//! Inizializzazione EGL/GLES senza finestra, per il backend headless: gli shader di
//! `ossido-render` girano anche nei test e in CI (llvmpipe), non solo nella finestra winit.
//!
//! Unico modulo di Ossido dove `unsafe` è ammesso (AGENTS.md): i costruttori di smithay per
//! display e renderer sono `unsafe` per le condizioni scritte accanto a ogni blocco.
#![allow(unsafe_code)]

use smithay::backend::egl::{EGLContext, EGLDevice, EGLDisplay};
use smithay::backend::renderer::gles::GlesRenderer;
use tracing::{info, warn};

/// Un renderer GLES su un device EGL senza superficie. Con `prefer_gpu` prima le GPU,
/// altrimenti prima il device software di Mesa (llvmpipe): screenshot uguali su ogni
/// macchina e in CI. Errore se nessun device si inizializza.
pub fn headless_renderer(prefer_gpu: bool) -> Result<GlesRenderer, String> {
    let mut devices: Vec<EGLDevice> = EGLDevice::enumerate()
        .map_err(|e| format!("device EGL non elencabili: {e}"))?
        .collect();
    devices.sort_by_key(|device| device.is_software() == prefer_gpu);
    let mut errors = Vec::new();
    for device in devices {
        let software = device.is_software();
        match renderer_for(device) {
            Ok(renderer) => {
                info!("headless su GLES (device software: {software})");
                return Ok(renderer);
            }
            Err(err) => {
                warn!("device EGL scartato: {err}");
                errors.push(err);
            }
        }
    }
    Err(format!(
        "nessun device EGL utilizzabile: {}",
        errors.join("; ")
    ))
}

fn renderer_for(device: EGLDevice) -> Result<GlesRenderer, String> {
    // SAFETY: smithay tiene traccia dei display che crea; la condizione è che nessun altro
    // codice nel processo chiami eglGetPlatformDisplay/eglTerminate sullo stesso display.
    // Ossido crea i display EGL solo tramite smithay.
    let display = unsafe { EGLDisplay::new(device) }.map_err(|e| format!("display: {e}"))?;
    let context = EGLContext::new(&display).map_err(|e| format!("contesto: {e}"))?;
    // SAFETY: il contesto è appena creato e non è corrente su nessun altro thread; il
    // renderer resta sul thread del compositor, l'unico che lo usa.
    unsafe { GlesRenderer::new(context) }.map_err(|e| format!("renderer: {e}"))
}
