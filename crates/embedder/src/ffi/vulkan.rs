//! The engine's Vulkan renderer (Impeller): a `VK_KHR_wayland_surface` swapchain on the
//! app's `wl_surface`. The driver (Mesa, NVIDIA) passes the buffers to the compositor as
//! EGL would.
//!
//! The engine calls the callbacks from the raster thread: `next_image` takes an image of the
//! swapchain (rebuilt if the size changes), Impeller draws on it, `present` brings it to the
//! presentation state and presents it. Presentation does not wait for the frame callback
//! (MAILBOX or IMMEDIATE if available): the pace comes from the engine's vsync, as with EGL.
//!
//! The integrated GPU is preferred (it uses less power and on a laptop keeps the discrete one
//! off); `SILICA_VULKAN_GPU=discrete` chooses the discrete one.

use std::ffi::{CStr, c_char, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ash::vk::{self, Handle};
use wayland_client::Connection;
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;

use super::Error;

fn vk_error(call: &'static str) -> impl Fn(vk::Result) -> Error {
    move |err| Error::Gl(call, format!("{err:?}"))
}

/// Instance and device extensions, to be declared to the engine too.
const INSTANCE_EXTENSIONS: [&CStr; 2] = [ash::khr::surface::NAME, ash::khr::wayland_surface::NAME];
const DEVICE_EXTENSIONS: [&CStr; 1] = [ash::khr::swapchain::NAME];

/// The current swapchain and what is needed to present its images.
struct Swap {
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    extent: vk::Extent2D,
    format: vk::Format,
    /// Commands that bring every image from COLOR_ATTACHMENT_OPTIMAL to PRESENT_SRC_KHR.
    transitions: Vec<vk::CommandBuffer>,
    /// The image taken with `next_image` and not yet presented.
    current: Option<u32>,
}

/// The app's instance, device and swapchain. Lives in [`super::Callbacks`] while the engine
/// runs.
pub struct VulkanSurface {
    _entry: ash::Entry,
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue_family: u32,
    queue: vk::Queue,
    surface_fn: ash::khr::surface::Instance,
    swapchain_fn: ash::khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    pool: vk::CommandPool,
    acquired: vk::Fence,
    transitioned: vk::Semaphore,
    swap: Mutex<Swap>,
    presented: AtomicBool,
    instance_extensions: Vec<*const c_char>,
    device_extensions: Vec<*const c_char>,
    name: String,
}

// SAFETY: instance, device, queue and Vulkan objects are handles the API allows to use from
// any thread as long as access to the queue and the swapchain is synchronized externally:
// the engine uses them only from the raster thread, and the state that changes (swapchain,
// current image) sits behind the Mutex. The extension pointers point to static
// constants.
unsafe impl Send for VulkanSurface {}
// SAFETY: see above.
unsafe impl Sync for VulkanSurface {}

impl VulkanSurface {
    /// Instance, device and swapchain for `surface`, of `size` physical pixels.
    pub fn new(conn: &Connection, surface: &WlSurface, size: (u32, u32)) -> Result<Self, Error> {
        // SAFETY: loads the system libvulkan (the Khronos loader), which exposes the standard
        // API checked by ash.
        let entry =
            unsafe { ash::Entry::load() }.map_err(|err| Error::Gl("libvulkan", err.to_string()))?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"silica-embedder")
            .api_version(vk::API_VERSION_1_1);
        let extensions: Vec<*const c_char> =
            INSTANCE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();
        let create = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(&extensions);
        // SAFETY: create is a valid structure with pointers to data alive for the call.
        let instance = unsafe { entry.create_instance(&create, None) }
            .map_err(vk_error("vkCreateInstance"))?;

        let wayland_fn = ash::khr::wayland_surface::Instance::new(&entry, &instance);
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        let display = conn.backend().display_ptr().cast::<vk::wl_display>();
        let wl_surface = surface.id().as_ptr().cast::<vk::wl_surface>();
        let surface_info = vk::WaylandSurfaceCreateInfoKHR::default()
            .display(display)
            .surface(wl_surface);
        // SAFETY: display and surface are the libwayland objects of the connection and of the
        // app, alive longer than the Vulkan surface (destroyed in Drop).
        let vk_surface = unsafe { wayland_fn.create_wayland_surface(&surface_info, None) }
            .map_err(vk_error("vkCreateWaylandSurfaceKHR"))?;

        let (physical, queue_family, name) = pick_device(&instance, &surface_fn, vk_surface)
            .ok_or(Error::Gl(
                "vkEnumeratePhysicalDevices",
                "no Vulkan GPU presents on this surface".to_owned(),
            ))?;
        let priorities = [1.0f32];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let device_extensions: Vec<*const c_char> =
            DEVICE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&device_extensions);
        // SAFETY: physical comes from this instance; device_info is valid for the call.
        let device = unsafe { instance.create_device(physical, &device_info, None) }
            .map_err(vk_error("vkCreateDevice"))?;
        // SAFETY: the family was created with one queue (index 0).
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: valid device, valid structure.
        let pool = unsafe { device.create_command_pool(&pool_info, None) }
            .map_err(vk_error("vkCreateCommandPool"))?;
        // SAFETY: as above.
        let acquired = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }
            .map_err(vk_error("vkCreateFence"))?;
        // SAFETY: as above.
        let transitioned =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .map_err(vk_error("vkCreateSemaphore"))?;

        let empty = Swap {
            swapchain: vk::SwapchainKHR::null(),
            images: Vec::new(),
            extent: vk::Extent2D::default(),
            format: vk::Format::UNDEFINED,
            transitions: Vec::new(),
            current: None,
        };
        let this = Self {
            _entry: entry,
            instance,
            physical,
            device,
            queue_family,
            queue,
            surface_fn,
            swapchain_fn,
            surface: vk_surface,
            pool,
            acquired,
            transitioned,
            swap: Mutex::new(empty),
            presented: AtomicBool::new(false),
            instance_extensions: extensions,
            device_extensions,
            name,
        };
        if let Ok(mut swap) = this.swap.lock() {
            this.rebuild(&mut swap, size)?;
        }
        Ok(this)
    }

    /// The chosen GPU, for the log and the measurements.
    pub fn renderer(&self) -> &str {
        &self.name
    }

    pub fn presented(&self) -> bool {
        self.presented.load(Ordering::Acquire)
    }

    pub(super) fn handles(&self) -> (*mut c_void, *mut c_void, *mut c_void, u32, *mut c_void) {
        (
            self.instance.handle().as_raw() as *mut c_void,
            self.physical.as_raw() as *mut c_void,
            self.device.handle().as_raw() as *mut c_void,
            self.queue_family,
            self.queue.as_raw() as *mut c_void,
        )
    }

    pub(super) fn instance_extensions(&self) -> &[*const c_char] {
        &self.instance_extensions
    }

    pub(super) fn device_extensions(&self) -> &[*const c_char] {
        &self.device_extensions
    }

    /// `vkGetInstanceProcAddr` for the engine.
    pub(super) fn proc_address(&self, instance: *mut c_void, name: &CStr) -> *mut c_void {
        let instance = vk::Instance::from_raw(instance as u64);
        // SAFETY: name is a valid C string; instance is null or the one created here (the engine
        // passes the one we gave it).
        let f =
            unsafe { (self._entry.static_fn().get_instance_proc_addr)(instance, name.as_ptr()) };
        f.map_or(std::ptr::null_mut(), |f| f as *mut c_void)
    }

    /// Rebuilds the swapchain for `size` physical pixels (the old one becomes `old_swapchain`).
    fn rebuild(&self, swap: &mut Swap, size: (u32, u32)) -> Result<(), Error> {
        // SAFETY: the calls use a valid device and surface of this instance.
        let (caps, formats, modes) = unsafe {
            (
                self.surface_fn
                    .get_physical_device_surface_capabilities(self.physical, self.surface)
                    .map_err(vk_error("vkGetPhysicalDeviceSurfaceCapabilitiesKHR"))?,
                self.surface_fn
                    .get_physical_device_surface_formats(self.physical, self.surface)
                    .map_err(vk_error("vkGetPhysicalDeviceSurfaceFormatsKHR"))?,
                self.surface_fn
                    .get_physical_device_surface_present_modes(self.physical, self.surface)
                    .map_err(vk_error("vkGetPhysicalDeviceSurfacePresentModesKHR"))?,
            )
        };
        let format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
            .or_else(|| {
                formats
                    .iter()
                    .find(|f| f.format == vk::Format::R8G8B8A8_UNORM)
            })
            .or(formats.first())
            .copied()
            .ok_or(Error::Gl(
                "vkGetPhysicalDeviceSurfaceFormatsKHR",
                "no format".to_owned(),
            ))?;
        // The pace comes from the engine's vsync: no waiting for the frame callback.
        let mode = [vk::PresentModeKHR::MAILBOX, vk::PresentModeKHR::IMMEDIATE]
            .into_iter()
            .find(|m| modes.contains(m))
            .unwrap_or(vk::PresentModeKHR::FIFO);
        // Shell surfaces are transparent: premultiplied alpha.
        let alpha = [
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::INHERIT,
            vk::CompositeAlphaFlagsKHR::OPAQUE,
        ]
        .into_iter()
        .find(|a| caps.supported_composite_alpha.contains(*a))
        .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);
        let extent = if caps.current_extent.width == u32::MAX {
            vk::Extent2D {
                width: size.0.clamp(
                    caps.min_image_extent.width.max(1),
                    caps.max_image_extent.width.max(1),
                ),
                height: size.1.clamp(
                    caps.min_image_extent.height.max(1),
                    caps.max_image_extent.height.max(1),
                ),
            }
        } else {
            caps.current_extent
        };
        let mut count = (caps.min_image_count + 1).max(3);
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | (caps.supported_usage_flags
                & (vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED));
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
            .composite_alpha(alpha)
            .present_mode(mode)
            .clipped(true)
            .old_swapchain(swap.swapchain);
        // SAFETY: no pending work uses the old swapchain (the queue is waited on);
        // the structure is valid for the call.
        let swapchain = unsafe {
            self.device
                .queue_wait_idle(self.queue)
                .map_err(vk_error("vkQueueWaitIdle"))?;
            self.swapchain_fn
                .create_swapchain(&info, None)
                .map_err(vk_error("vkCreateSwapchainKHR"))?
        };
        // SAFETY: the old swapchain and its commands are no longer in use (queue idle).
        unsafe {
            if !swap.transitions.is_empty() {
                self.device
                    .free_command_buffers(self.pool, &swap.transitions);
            }
            if swap.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(swap.swapchain, None);
            }
        }
        // SAFETY: swapchain just created by this device.
        let images = unsafe { self.swapchain_fn.get_swapchain_images(swapchain) }
            .map_err(vk_error("vkGetSwapchainImagesKHR"))?;
        let alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(u32::try_from(images.len()).unwrap_or(0));
        // SAFETY: valid pool, valid structure.
        let transitions = unsafe { self.device.allocate_command_buffers(&alloc) }
            .map_err(vk_error("vkAllocateCommandBuffers"))?;
        for (&cmd, &image) in transitions.iter().zip(&images) {
            let barrier = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                );
            // SAFETY: cmd just allocated from this pool; the recording uses valid objects.
            unsafe {
                self.device
                    .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                    .map_err(vk_error("vkBeginCommandBuffer"))?;
                self.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );
                self.device
                    .end_command_buffer(cmd)
                    .map_err(vk_error("vkEndCommandBuffer"))?;
            }
        }
        *swap = Swap {
            swapchain,
            images,
            extent,
            format: format.format,
            transitions,
            current: None,
        };
        Ok(())
    }

    /// An image of the swapchain for a frame of `size` pixels (null if not possible).
    pub(super) fn next_image(&self, size: (u32, u32)) -> (u64, u32) {
        let Ok(mut swap) = self.swap.lock() else {
            return (0, 0);
        };
        if (swap.extent.width, swap.extent.height) != size
            && size.0 > 0
            && size.1 > 0
            && let Err(err) = self.rebuild(&mut swap, size)
        {
            tracing::warn!("swapchain: {err}");
            return (0, 0);
        }
        for _ in 0..2 {
            // SAFETY: valid swapchain and fence; the fence is waited on before reusing it.
            let acquired = unsafe {
                self.swapchain_fn.acquire_next_image(
                    swap.swapchain,
                    u64::MAX,
                    vk::Semaphore::null(),
                    self.acquired,
                )
            };
            match acquired {
                Ok((index, _suboptimal)) => {
                    // SAFETY: the fence was passed to the acquisition that just succeeded.
                    let waited = unsafe {
                        self.device
                            .wait_for_fences(&[self.acquired], true, u64::MAX)
                            .and_then(|()| self.device.reset_fences(&[self.acquired]))
                    };
                    if let Err(err) = waited {
                        tracing::warn!("vkWaitForFences: {err:?}");
                        return (0, 0);
                    }
                    swap.current = Some(index);
                    let image = swap
                        .images
                        .get(index as usize)
                        .map_or(0, |image| image.as_raw());
                    return (image, swap.format.as_raw() as u32);
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    if let Err(err) = self.rebuild(&mut swap, size) {
                        tracing::warn!("swapchain: {err}");
                        return (0, 0);
                    }
                }
                Err(err) => {
                    tracing::warn!("vkAcquireNextImageKHR: {err:?}");
                    return (0, 0);
                }
            }
        }
        (0, 0)
    }

    /// Brings the current image to the presentation state and presents it.
    pub(super) fn present(&self) -> bool {
        let Ok(mut swap) = self.swap.lock() else {
            return false;
        };
        let Some(index) = swap.current.take() else {
            return false;
        };
        let Some(&cmd) = swap.transitions.get(index as usize) else {
            return false;
        };
        let commands = [cmd];
        let signal = [self.transitioned];
        let submit = vk::SubmitInfo::default()
            .command_buffers(&commands)
            .signal_semaphores(&signal);
        let swapchains = [swap.swapchain];
        let indices = [index];
        let present = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal)
            .swapchains(&swapchains)
            .image_indices(&indices);
        // SAFETY: the transition command is recorded for this image; the queue is used only by
        // this thread (raster); the semaphore orders the presentation after the transition,
        // and the submission after the engine's drawing on the same queue.
        let result = unsafe {
            self.device
                .queue_submit(self.queue, &[submit], vk::Fence::null())
                .and_then(|()| self.swapchain_fn.queue_present(self.queue, &present))
        };
        match result {
            Ok(_) => {
                self.presented.store(true, Ordering::Release);
                true
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(err) => {
                tracing::warn!("vkQueuePresentKHR: {err:?}");
                false
            }
        }
    }
}

/// The GPU that presents on this surface: integrated, then discrete (or the other way round
/// with `SILICA_VULKAN_GPU=discrete`), never the one on the CPU if there is another.
fn pick_device(
    instance: &ash::Instance,
    surface_fn: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Option<(vk::PhysicalDevice, u32, String)> {
    let discrete_first = std::env::var("SILICA_VULKAN_GPU").is_ok_and(|v| v == "discrete");
    // SAFETY: valid instance.
    let devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    let mut best: Option<(u32, vk::PhysicalDevice, u32, String)> = None;
    for device in devices {
        // SAFETY: device comes from this instance.
        let props = unsafe { instance.get_physical_device_properties(device) };
        if props.api_version < vk::API_VERSION_1_1 {
            continue;
        }
        // SAFETY: as above.
        let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
        let family = families.iter().enumerate().find_map(|(i, f)| {
            let i = u32::try_from(i).ok()?;
            // SAFETY: family index of this device, surface of this instance.
            let present =
                unsafe { surface_fn.get_physical_device_surface_support(device, i, surface) }
                    .unwrap_or(false);
            (f.queue_flags.contains(vk::QueueFlags::GRAPHICS) && present).then_some(i)
        });
        let Some(family) = family else {
            continue;
        };
        let score = match props.device_type {
            vk::PhysicalDeviceType::INTEGRATED_GPU if !discrete_first => 3,
            vk::PhysicalDeviceType::DISCRETE_GPU if discrete_first => 3,
            vk::PhysicalDeviceType::INTEGRATED_GPU | vk::PhysicalDeviceType::DISCRETE_GPU => 2,
            vk::PhysicalDeviceType::CPU => 0,
            _ => 1,
        };
        let name = props
            .device_name_as_c_str()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if best.as_ref().is_none_or(|(s, ..)| score > *s) {
            best = Some((score, device, family, name));
        }
    }
    best.map(|(_, device, family, name)| (device, family, name))
}

impl Drop for VulkanSurface {
    fn drop(&mut self) {
        // The engine is already shut down: nobody uses the queue, images and commands anymore.
        // SAFETY: every object was created here and is destroyed once, in reverse order,
        // after the device has finished its work.
        unsafe {
            let _ = self.device.device_wait_idle();
            if let Ok(swap) = self.swap.get_mut() {
                if !swap.transitions.is_empty() {
                    self.device
                        .free_command_buffers(self.pool, &swap.transitions);
                }
                if swap.swapchain != vk::SwapchainKHR::null() {
                    self.swapchain_fn.destroy_swapchain(swap.swapchain, None);
                }
            }
            self.device.destroy_semaphore(self.transitioned, None);
            self.device.destroy_fence(self.acquired, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
