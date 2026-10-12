//! linux-dmabuf: i client disegnano con la GPU e passano il buffer senza copie.
//!
//! Il feedback (versione 4) indica ai client il render node della GPU di Ossido: senza,
//! Mesa non sa quale device usare per una superficie Wayland e ricade su llvmpipe.
//! Solo con il backend winit; in headless (pixman) il protocollo non c'è.

use std::os::unix::fs::MetadataExt;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::egl::EGLDevice;
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::delegate_dmabuf;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::dmabuf::{
    DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier,
};
use tracing::{info, warn};

use crate::backend::Backend;
use crate::state::State;

/// Il device (`dev_t`) del render node su cui gira il renderer di Ossido.
fn main_device(renderer: &GlesRenderer) -> Result<u64, String> {
    let device = EGLDevice::device_for_display(renderer.egl_context().display())
        .map_err(|e| format!("device EGL non trovato: {e}"))?;
    let path = device
        .render_device_path()
        .map_err(|e| format!("render node non trovato: {e}"))?;
    let metadata = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    info!("render node per dmabuf: {}", path.display());
    Ok(metadata.rdev())
}

/// Crea il global `zwp_linux_dmabuf_v1` con i formati che il renderer sa importare.
/// Senza render node (driver che non lo espone) offre la versione 3, senza feedback.
pub fn create_global(
    state: &mut DmabufState,
    dh: &DisplayHandle,
    renderer: &GlesRenderer,
) -> DmabufGlobal {
    let formats = renderer.dmabuf_formats();
    let feedback = main_device(renderer).and_then(|device| {
        DmabufFeedbackBuilder::new(device, formats.clone())
            .build()
            .map_err(|e| format!("feedback dmabuf: {e}"))
    });
    match feedback {
        Ok(feedback) => state.create_global_with_default_feedback::<State>(dh, &feedback),
        Err(err) => {
            warn!("dmabuf senza feedback: {err}");
            state.create_global::<State>(dh, formats)
        }
    }
}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        let imported = match &mut self.backend {
            Backend::Winit(backend) => backend
                .graphics
                .renderer()
                .import_dmabuf(&dmabuf, None)
                .is_ok(),
            Backend::Headless(_) => false,
        };
        if imported {
            // Se il client è già sparito non c'è nessuno da avvisare.
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

delegate_dmabuf!(State);
