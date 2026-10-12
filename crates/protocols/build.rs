//! I binding si generano dagli XML con le macro di wayland-scanner, che non dicono a cargo
//! di ricompilare quando un XML cambia: lo dice questo script.

fn main() {
    println!("cargo:rerun-if-changed=xml");
}
