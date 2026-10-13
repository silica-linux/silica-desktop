//! Il renderer Vulkan dell'engine (Impeller): una swapchain `VK_KHR_wayland_surface` sulla
//! `wl_surface` dell'app. Il driver (Mesa, NVIDIA) passa i buffer al compositor come
//! farebbe EGL.
//!
//! L'engine chiama i callback dal thread raster: `next_image` prende un'immagine della
//! swapchain (rifatta se la dimensione cambia), Impeller ci disegna, `present` la porta allo
//! stato di presentazione e la presenta. La presentazione non aspetta il frame callback
//! (MAILBOX o IMMEDIATE se ci sono): il ritmo lo dà il vsync dell'engine, come con EGL.
//!
//! Si preferisce la GPU integrata (consuma meno e su un portatile tiene spenta la discreta);
//! `SILICA_VULKAN_GPU=discrete` sceglie la discreta.

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

/// Estensioni dell'istanza e del dispositivo, da dichiarare anche all'engine.
const INSTANCE_EXTENSIONS: [&CStr; 2] = [ash::khr::surface::NAME, ash::khr::wayland_surface::NAME];
const DEVICE_EXTENSIONS: [&CStr; 1] = [ash::khr::swapchain::NAME];

/// La swapchain del momento e quello che serve per presentarne le immagini.
struct Swap {
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    extent: vk::Extent2D,
    format: vk::Format,
    /// Comandi che portano ogni immagine da COLOR_ATTACHMENT_OPTIMAL a PRESENT_SRC_KHR.
    transitions: Vec<vk::CommandBuffer>,
    /// L'immagine presa con `next_image` e non ancora presentata.
    current: Option<u32>,
}

/// Istanza, dispositivo e swapchain dell'app. Vive in [`super::Callbacks`] finché l'engine
/// gira.
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

// SAFETY: istanza, dispositivo, coda e oggetti Vulkan sono handle che l'API consente di usare
// da qualsiasi thread purché l'accesso alla coda e alla swapchain sia sincronizzato
// dall'esterno: l'engine li usa solo dal thread raster, e lo stato che cambia (swapchain,
// immagine corrente) sta dietro il Mutex. I puntatori delle estensioni puntano a costanti
// statiche.
unsafe impl Send for VulkanSurface {}
// SAFETY: vedi sopra.
unsafe impl Sync for VulkanSurface {}

impl VulkanSurface {
    /// Istanza, dispositivo e swapchain per `surface`, di `size` pixel fisici.
    pub fn new(conn: &Connection, surface: &WlSurface, size: (u32, u32)) -> Result<Self, Error> {
        // SAFETY: carica la libvulkan di sistema (il loader Khronos), che espone l'API
        // standard verificata da ash.
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
        // SAFETY: create è una struttura valida con puntatori a dati vivi per la chiamata.
        let instance = unsafe { entry.create_instance(&create, None) }
            .map_err(vk_error("vkCreateInstance"))?;

        let wayland_fn = ash::khr::wayland_surface::Instance::new(&entry, &instance);
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        let display = conn.backend().display_ptr().cast::<vk::wl_display>();
        let wl_surface = surface.id().as_ptr().cast::<vk::wl_surface>();
        let surface_info = vk::WaylandSurfaceCreateInfoKHR::default()
            .display(display)
            .surface(wl_surface);
        // SAFETY: display e superficie sono gli oggetti libwayland della connessione e
        // dell'app, vivi più a lungo della superficie Vulkan (distrutta in Drop).
        let vk_surface = unsafe { wayland_fn.create_wayland_surface(&surface_info, None) }
            .map_err(vk_error("vkCreateWaylandSurfaceKHR"))?;

        let (physical, queue_family, name) = pick_device(&instance, &surface_fn, vk_surface)
            .ok_or(Error::Gl(
                "vkEnumeratePhysicalDevices",
                "nessuna GPU Vulkan presenta su questa superficie".to_owned(),
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
        // SAFETY: physical viene da questa istanza; device_info è valida per la chiamata.
        let device = unsafe { instance.create_device(physical, &device_info, None) }
            .map_err(vk_error("vkCreateDevice"))?;
        // SAFETY: la famiglia è stata creata con una coda (indice 0).
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: dispositivo valido, struttura valida.
        let pool = unsafe { device.create_command_pool(&pool_info, None) }
            .map_err(vk_error("vkCreateCommandPool"))?;
        // SAFETY: come sopra.
        let acquired = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }
            .map_err(vk_error("vkCreateFence"))?;
        // SAFETY: come sopra.
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

    /// La GPU scelta, per il log e le misure.
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

    /// `vkGetInstanceProcAddr` per l'engine.
    pub(super) fn proc_address(&self, instance: *mut c_void, name: &CStr) -> *mut c_void {
        let instance = vk::Instance::from_raw(instance as u64);
        // SAFETY: name è una stringa C valida; instance è nulla o quella creata qui (l'engine
        // passa quella che gli abbiamo dato).
        let f =
            unsafe { (self._entry.static_fn().get_instance_proc_addr)(instance, name.as_ptr()) };
        f.map_or(std::ptr::null_mut(), |f| f as *mut c_void)
    }

    /// Rifà la swapchain per `size` pixel fisici (la vecchia diventa `old_swapchain`).
    fn rebuild(&self, swap: &mut Swap, size: (u32, u32)) -> Result<(), Error> {
        // SAFETY: le chiamate usano dispositivo e superficie validi di questa istanza.
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
                "nessun formato".to_owned(),
            ))?;
        // Il ritmo lo dà il vsync dell'engine: niente attesa del frame callback.
        let mode = [vk::PresentModeKHR::MAILBOX, vk::PresentModeKHR::IMMEDIATE]
            .into_iter()
            .find(|m| modes.contains(m))
            .unwrap_or(vk::PresentModeKHR::FIFO);
        // Le superfici della shell sono trasparenti: alfa premoltiplicata.
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
        // SAFETY: nessun lavoro in sospeso usa la swapchain vecchia (si aspetta la coda);
        // la struttura è valida per la chiamata.
        let swapchain = unsafe {
            self.device
                .queue_wait_idle(self.queue)
                .map_err(vk_error("vkQueueWaitIdle"))?;
            self.swapchain_fn
                .create_swapchain(&info, None)
                .map_err(vk_error("vkCreateSwapchainKHR"))?
        };
        // SAFETY: la vecchia swapchain e i suoi comandi non sono più in uso (coda ferma).
        unsafe {
            if !swap.transitions.is_empty() {
                self.device
                    .free_command_buffers(self.pool, &swap.transitions);
            }
            if swap.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(swap.swapchain, None);
            }
        }
        // SAFETY: swapchain appena creata da questo dispositivo.
        let images = unsafe { self.swapchain_fn.get_swapchain_images(swapchain) }
            .map_err(vk_error("vkGetSwapchainImagesKHR"))?;
        let alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(u32::try_from(images.len()).unwrap_or(0));
        // SAFETY: pool valido, struttura valida.
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
            // SAFETY: cmd appena allocato da questo pool; la registrazione usa oggetti validi.
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

    /// Un'immagine della swapchain per un frame di `size` pixel (null se non si può).
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
            // SAFETY: swapchain e fence validi; si aspetta la fence prima di riusarla.
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
                    // SAFETY: la fence è stata passata all'acquisizione appena riuscita.
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

    /// Porta l'immagine corrente allo stato di presentazione e la presenta.
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
        // SAFETY: il comando di transizione è registrato per quest'immagine; la coda è usata
        // solo da questo thread (raster); il semaforo ordina la presentazione dopo la
        // transizione, e la sottomissione dopo il disegno dell'engine sulla stessa coda.
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

/// La GPU che presenta su questa superficie: integrata, poi discreta (o il contrario con
/// `SILICA_VULKAN_GPU=discrete`), mai quella sulla CPU se c'è altro.
fn pick_device(
    instance: &ash::Instance,
    surface_fn: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Option<(vk::PhysicalDevice, u32, String)> {
    let discrete_first = std::env::var("SILICA_VULKAN_GPU").is_ok_and(|v| v == "discrete");
    // SAFETY: istanza valida.
    let devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    let mut best: Option<(u32, vk::PhysicalDevice, u32, String)> = None;
    for device in devices {
        // SAFETY: device viene da questa istanza.
        let props = unsafe { instance.get_physical_device_properties(device) };
        if props.api_version < vk::API_VERSION_1_1 {
            continue;
        }
        // SAFETY: come sopra.
        let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
        let family = families.iter().enumerate().find_map(|(i, f)| {
            let i = u32::try_from(i).ok()?;
            // SAFETY: indice di famiglia di questo dispositivo, superficie di questa istanza.
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
        // L'engine è già spento: nessuno usa più coda, immagini e comandi.
        // SAFETY: tutti gli oggetti sono stati creati qui e si distruggono una volta, in
        // ordine inverso, dopo che il dispositivo ha finito il lavoro.
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
