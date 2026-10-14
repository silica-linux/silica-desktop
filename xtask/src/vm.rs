//! `cargo xtask vm`: the real session in a QEMU/KVM virtual machine (docs/vm.md).
//!
//! The guest is an Ubuntu cloud image of the same release as the host (the binaries are
//! built here and must find the same glibc), configured by cloud-init (NoCloud) served over
//! HTTP by this process together with a tarball of the session: Ossido, the embedder,
//! silica-daemon, the Flutter engine and the shell bundle. Inside the guest Ossido runs on
//! DRM/KMS (virtio-gpu) under seatd; its control channel is forwarded to the host, where
//! `ui.screenshot` checks that the first frame is not black.
//!
//! ```text
//! cargo xtask vm [--port N] [--timeout SECONDS] [--out PATH] [--show] [--keep] [--payload-only]
//! ```

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use silica_control::Client;
use silica_control::commands::{PERF_FRAMES, UI_SCREENSHOT};

use crate::{Result, cargo_command, flutter, root, status};

/// The host port forwarded to the guest's control channel (not 7979/7980, used by the
/// sessions on the host).
const DEFAULT_PORT: u16 = 7990;
/// The control channel inside the guest: Ossido listens on loopback, socat exposes it on
/// the guest's network interface for QEMU's forward.
const GUEST_CONTROL: u16 = 7991;
const GUEST_FORWARD: u16 = 7990;
/// The guest's first boot installs packages: minutes, not seconds.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(900);
/// After the first answer of the control channel, time for the shell to draw.
const SETTLE: Duration = Duration::from_secs(20);
/// The log line of Ossido's real-session backend, looked for on the serial console.
const DRM_READY: &str = "drm backend ready";

struct Options {
    port: u16,
    timeout: Duration,
    out: Option<PathBuf>,
    show: bool,
    keep: bool,
    /// Only build the session tarball (no QEMU needed).
    payload_only: bool,
}

fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options {
        port: DEFAULT_PORT,
        timeout: DEFAULT_TIMEOUT,
        out: None,
        show: false,
        keep: false,
        payload_only: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--port" => {
                let text = value()?;
                options.port = text
                    .parse()
                    .ok()
                    .filter(|port| ![0, 7979, 7980].contains(port))
                    .ok_or(format!("invalid port (not 7979/7980): {text}"))?;
            }
            "--timeout" => {
                let text = value()?;
                options.timeout = Duration::from_secs(
                    text.parse()
                        .map_err(|_| format!("invalid timeout: {text}"))?,
                );
            }
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--show" => options.show = true,
            "--keep" => options.keep = true,
            "--payload-only" => options.payload_only = true,
            other => {
                return Err(format!(
                    "usage: cargo xtask vm [--port N] [--timeout SECONDS] [--out PATH] [--show] [--keep] [--payload-only], not {other}"
                ));
            }
        }
    }
    Ok(options)
}

pub fn vm(args: &[String]) -> Result<()> {
    let options = parse(args)?;
    let work = target_dir()?.join("vm");
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    if options.payload_only {
        let payload = build_payload(&work)?;
        eprintln!("xtask: session tarball in {}", payload.display());
        return Ok(());
    }
    let tools = Tools::find()?;

    let payload = build_payload(&work)?;
    let release = host_release()?;
    let base = cloud_image(&work, &release)?;
    let disk = work.join("disk.qcow2");
    let _ = std::fs::remove_file(&disk);
    let mut overlay = Command::new(&tools.qemu_img);
    overlay
        .args(["create", "-q", "-f", "qcow2", "-F", "qcow2", "-b"])
        .arg(&base)
        .arg(&disk)
        .arg("12G");
    status(overlay)?;

    let server = Server::start(payload)?;
    let console = work.join("console.log");
    let _ = std::fs::remove_file(&console);
    let mut qemu = Qemu::start(&tools, &disk, &console, server.port, &options)?;
    let result = drive(&options, &console, &mut qemu, &work);
    if options.keep {
        eprintln!(
            "xtask: --keep: the VM is still running (pid {})",
            qemu.0.id()
        );
        std::mem::forget(qemu);
    }
    result
}

/// The tools on the host: QEMU and, if usable, KVM.
struct Tools {
    qemu: PathBuf,
    qemu_img: PathBuf,
    kvm: bool,
}

impl Tools {
    fn find() -> Result<Self> {
        let qemu = which("qemu-system-x86_64").ok_or(
            "qemu-system-x86_64 not found: install qemu-system-x86 and qemu-utils (docs/vm.md)",
        )?;
        let qemu_img =
            which("qemu-img").ok_or("qemu-img not found: install qemu-utils (docs/vm.md)")?;
        let kvm = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok();
        if !kvm {
            eprintln!(
                "xtask: /dev/kvm not usable (group kvm?): emulation without KVM, much slower"
            );
        }
        Ok(Self {
            qemu,
            qemu_img,
            kvm,
        })
    }
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// The cargo target directory (`CARGO_TARGET_DIR` or `target/`).
fn target_dir() -> Result<PathBuf> {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => Ok(root()?.join("target")),
    }
}

/// The host's Ubuntu release (`VERSION_CODENAME`), or `SILICA_VM_RELEASE`.
fn host_release() -> Result<String> {
    if let Ok(release) = std::env::var("SILICA_VM_RELEASE") {
        return Ok(release);
    }
    let text =
        std::fs::read_to_string("/etc/os-release").map_err(|e| format!("/etc/os-release: {e}"))?;
    os_release_codename(&text).ok_or_else(|| {
        "no VERSION_CODENAME in /etc/os-release: set SILICA_VM_RELEASE (an Ubuntu codename)"
            .to_owned()
    })
}

fn os_release_codename(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("VERSION_CODENAME="))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
}

/// The cloud image of `release`, downloaded once into the work directory.
fn cloud_image(work: &Path, release: &str) -> Result<PathBuf> {
    let image = work.join(format!("{release}-server-cloudimg-amd64.img"));
    if image.is_file() {
        return Ok(image);
    }
    let partial = image.with_extension("part");
    let mut curl = Command::new("curl");
    curl.args(["-sSfL", "-o"]).arg(&partial).arg(format!(
        "https://cloud-images.ubuntu.com/{release}/current/{release}-server-cloudimg-amd64.img"
    ));
    status(curl)?;
    std::fs::rename(&partial, &image).map_err(|e| format!("{}: {e}", image.display()))?;
    Ok(image)
}

/// Builds the session and packs it: `payload.tar` with `bin/`, `engine/`, `shell/` and
/// `run-session.sh`.
fn build_payload(work: &Path) -> Result<PathBuf> {
    status(cargo_command(&[
        "build",
        "--release",
        "--package",
        "ossido",
        "--package",
        "silica-embedder",
        "--package",
        "silica-daemon",
    ]))?;
    // The debug engine (downloaded) runs the debug (JIT) bundle: no engine build from source.
    flutter::engine()?;
    flutter::bundle("shell")?;

    let stage = work.join("payload");
    let _ = std::fs::remove_dir_all(&stage);
    let release = target_dir()?.join("release");
    for binary in ["ossido", "silica-embedder", "silica-daemon"] {
        copy(&release.join(binary), &stage.join("bin").join(binary))?;
    }
    let engine = flutter::engine_dir()?;
    for file in ["libflutter_engine.so", "icudtl.dat"] {
        copy(&engine.join(file), &stage.join("engine").join(file))?;
    }
    let mut assets = Command::new("cp");
    assets
        .arg("-r")
        .arg(root()?.join("shell/build/flutter_assets"))
        .arg(stage.join("shell"));
    std::fs::create_dir_all(stage.join("shell")).map_err(|e| format!("{e}"))?;
    status(assets)?;
    let script = stage.join("run-session.sh");
    std::fs::write(&script, run_session_script()).map_err(|e| format!("{e}"))?;

    let tar = work.join("payload.tar");
    let mut pack = Command::new("tar");
    pack.arg("-C").arg(&stage).arg("-cf").arg(&tar).arg(".");
    status(pack)?;
    Ok(tar)
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("{} → {}: {e}", from.display(), to.display()))
}

/// The shell components, as in `cargo xtask run --shell`, with the guest's paths.
fn run_session_script() -> String {
    let components = crate::shell_component_args(
        "/opt/silica/bin/silica-embedder --engine /opt/silica/engine --renderer software",
        "/opt/silica/shell/flutter_assets",
    );
    let spawn: String = components
        .chunks(2)
        .filter_map(|pair| match pair {
            [flag, command] => Some(format!("  {flag} '{command}' \\\n")),
            _ => None,
        })
        .collect();
    format!(
        r#"#!/bin/bash
# Started by cloud-init: Ossido on DRM/KMS under seatd, with the shell.
set -u
# Everything on the serial console, which the host reads.
exec > /dev/ttyS0 2>&1
export XDG_RUNTIME_DIR=/run/silica
mkdir -p -m 0700 "$XDG_RUNTIME_DIR"
export RUST_LOG=info
# The control channel stays on the guest's loopback; socat exposes it for QEMU's forward.
socat TCP-LISTEN:{GUEST_FORWARD},fork,reuseaddr TCP:127.0.0.1:{GUEST_CONTROL} &
# The seatd package starts its own daemon (seatd-launch would refuse the existing socket):
# use it; the session runs as root, which seatd lets in.
systemctl start seatd.service || true
export LIBSEAT_BACKEND=seatd
exec /opt/silica/bin/ossido --backend drm --control {GUEST_CONTROL} \
  --session-bus \
{spawn}
"#
    )
}

/// cloud-init's configuration: packages, the session tarball, the session.
fn user_data(port: u16) -> String {
    format!(
        r#"#cloud-config
hostname: silica-vm
ssh_pwauth: false
package_update: true
packages:
  - seatd
  - socat
  - dbus
  - curl
  - fontconfig
  - fonts-dejavu-core
  - libegl1
  - libegl-mesa0
  - libgbm1
  - libgl1-mesa-dri
  - libgles2
  - libinput10
  - libpixman-1-0
  - libseat1
  - libwayland-client0
  - libwayland-egl1
  - libwayland-server0
  - libxkbcommon0
runcmd:
  - modprobe virtio_gpu || (apt-get install -y linux-modules-extra-$(uname -r) && modprobe virtio_gpu)
  - mkdir -p /opt/silica
  - curl -sSf http://10.0.2.2:{port}/payload.tar | tar -x -C /opt/silica
  - ldd /opt/silica/bin/ossido /opt/silica/bin/silica-embedder | grep 'not found' > /dev/ttyS0 || true
  - ls -l /dev/dri > /dev/ttyS0 2>&1
  - setsid bash /opt/silica/run-session.sh < /dev/null > /dev/null 2>&1 &
"#
    )
}

/// A minimal HTTP server for the NoCloud seed and the tarball (QEMU's user network reaches
/// the host's loopback at 10.0.2.2).
struct Server {
    port: u16,
}

impl Server {
    fn start(payload: PathBuf) -> Result<Self> {
        let listener =
            TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("HTTP server: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("HTTP server: {e}"))?
            .port();
        let files = Arc::new((payload, user_data(port)));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let files = Arc::clone(&files);
                std::thread::spawn(move || {
                    if let Err(err) = serve(stream, &files.0, &files.1) {
                        eprintln!("xtask: HTTP: {err}");
                    }
                });
            }
        });
        Ok(Self { port })
    }
}

fn serve(stream: TcpStream, payload: &Path, user_data: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    // The headers are not needed: read them to the empty line.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let mut stream = stream;
    match path.trim_end_matches('/').rsplit('/').next().unwrap_or("") {
        "user-data" => respond(&mut stream, user_data.as_bytes()),
        "meta-data" => respond(
            &mut stream,
            b"instance-id: silica-vm\nlocal-hostname: silica-vm\n",
        ),
        "vendor-data" | "network-config" => respond(&mut stream, b""),
        "payload.tar" => {
            let mut file = std::fs::File::open(payload)?;
            let length = file.metadata()?.len();
            write!(
                stream,
                "HTTP/1.0 200 OK\r\nContent-Length: {length}\r\nContent-Type: application/x-tar\r\n\r\n"
            )?;
            std::io::copy(&mut file, &mut stream).map(|_| ())
        }
        _ => stream.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n"),
    }
}

fn respond(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

/// The QEMU process, killed when dropped.
struct Qemu(Child);

impl Qemu {
    fn start(
        tools: &Tools,
        disk: &Path,
        console: &Path,
        http_port: u16,
        options: &Options,
    ) -> Result<Self> {
        let mut command = Command::new(&tools.qemu);
        command
            .args(["-m", "4096", "-smp", "4", "-no-reboot"])
            .args(["-machine", "q35"])
            .arg("-drive")
            .arg(format!("file={},if=virtio,format=qcow2", disk.display()))
            .args(["-vga", "none", "-device", "virtio-vga"])
            .args(["-netdev"])
            .arg(format!(
                "user,id=net0,hostfwd=tcp:127.0.0.1:{}-:{GUEST_FORWARD}",
                options.port
            ))
            .args(["-device", "virtio-net-pci,netdev=net0"])
            .args([
                "-device",
                "virtio-keyboard-pci",
                "-device",
                "virtio-tablet-pci",
            ])
            .arg("-smbios")
            .arg(format!(
                "type=1,serial=ds=nocloud;s=http://10.0.2.2:{http_port}/"
            ))
            .arg("-serial")
            .arg(format!("file:{}", console.display()))
            .args(["-display", if options.show { "gtk" } else { "none" }])
            .stdin(Stdio::null());
        if tools.kvm {
            command.args(["-enable-kvm", "-cpu", "host"]);
        }
        eprintln!("xtask: starting the VM (console in {})", console.display());
        command.spawn().map(Qemu).map_err(|e| format!("qemu: {e}"))
    }
}

impl Drop for Qemu {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Waits for the session, takes the screenshot and checks it.
fn drive(options: &Options, console: &Path, qemu: &mut Qemu, work: &Path) -> Result<()> {
    let deadline = Instant::now() + options.timeout;
    let address = ("127.0.0.1", options.port);
    loop {
        if let Ok(Some(exit)) = qemu.0.try_wait() {
            return Err(format!(
                "the VM stopped ({exit}): see {}",
                console.display()
            ));
        }
        if Instant::now() > deadline {
            return Err(format!(
                "no control channel after {}s: see {}",
                options.timeout.as_secs(),
                console.display()
            ));
        }
        // QEMU accepts the connection even when nothing listens in the guest: only an
        // answer counts.
        if let Ok(mut client) = Client::connect(address)
            && client.call(PERF_FRAMES, serde_json::json!({})).is_ok()
        {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    eprintln!("xtask: control channel up, waiting for the shell");
    std::thread::sleep(SETTLE);

    let log = std::fs::read_to_string(console).unwrap_or_default();
    if !log.contains(DRM_READY) {
        return Err(format!(
            "Ossido answers but not on DRM (no \"{DRM_READY}\"): see {}",
            console.display()
        ));
    }
    let mut client = Client::connect(address).map_err(|e| format!("control channel: {e}"))?;
    let result = client
        .call(UI_SCREENSHOT, serde_json::json!({}))
        .map_err(|e| format!("ui.screenshot: {e}"))?;
    let png = result
        .get("png_base64")
        .and_then(|value| value.as_str())
        .ok_or("ui.screenshot without png_base64")?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(png)
        .map_err(|e| format!("screenshot: {e}"))?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| work.join("screenshot.png"));
    std::fs::write(&out, &png).map_err(|e| format!("{}: {e}", out.display()))?;
    let lit = lit_fraction(&png)?;
    eprintln!(
        "xtask: screenshot {} ({:.1}% non-black pixels)",
        out.display(),
        lit * 100.0
    );
    if lit < MIN_LIT {
        return Err(format!("the first frame is black: {}", out.display()));
    }
    eprintln!("xtask: vm ok");
    Ok(())
}

/// Below this share of non-black pixels the frame counts as black.
const MIN_LIT: f64 = 0.5;

/// The share of pixels that are not black (any channel above 8/255).
fn lit_fraction(png_bytes: &[u8]) -> Result<f64> {
    let mut decoder = png::Decoder::new(png_bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("PNG: {e}"))?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|e| format!("PNG: {e}"))?;
    let channels = info.color_type.samples();
    let color = channels.min(3);
    let data = pixels.get(..info.buffer_size()).unwrap_or_default();
    let total = data.len() / channels.max(1);
    if total == 0 {
        return Err("empty screenshot".to_owned());
    }
    let lit = data
        .chunks_exact(channels.max(1))
        .filter(|pixel| pixel.iter().take(color).any(|&value| value > 8))
        .count();
    Ok(lit as f64 / total as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
        drop(writer);
        out
    }

    #[test]
    fn a_black_frame_has_no_lit_pixels() {
        let black = png(2, 1, &[0, 0, 0, 255, 0, 0, 0, 255]);
        assert_eq!(lit_fraction(&black).unwrap(), 0.0);
        let half = png(2, 1, &[0, 0, 0, 255, 122, 123, 128, 255]);
        assert_eq!(lit_fraction(&half).unwrap(), 0.5);
    }

    #[test]
    fn the_release_comes_from_os_release() {
        let text = "NAME=\"Ubuntu\"\nVERSION_CODENAME=noble\nID=ubuntu\n";
        assert_eq!(os_release_codename(text).as_deref(), Some("noble"));
        assert_eq!(os_release_codename("ID=arch\n"), None);
    }

    #[test]
    fn ports_of_the_host_sessions_are_refused() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(parse(&args(&["--port", "7979"])).is_err());
        assert_eq!(parse(&args(&["--port", "7995"])).unwrap().port, 7995);
        assert_eq!(parse(&[]).unwrap().port, DEFAULT_PORT);
    }

    #[test]
    fn the_session_script_starts_ossido_on_drm_with_the_shell() {
        let script = run_session_script();
        assert!(script.contains("--backend drm --control 7991"));
        assert!(script.contains("--spawn-shell '/opt/silica/bin/silica-embedder"));
        assert!(script.contains("--namespace dock"));
    }

    fn get(port: u16, path: &str) -> String {
        use std::io::Read;
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: x\r\n\r\n").unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    }

    #[test]
    fn the_server_hands_out_the_seed_and_the_payload() {
        let dir = std::env::temp_dir().join(format!("xtask-vm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let payload = dir.join("payload.tar");
        std::fs::write(&payload, b"tarball").unwrap();
        let server = Server::start(payload).unwrap();
        let seed = get(server.port, "/user-data");
        assert!(seed.starts_with("HTTP/1.0 200 OK"));
        assert!(seed.contains(&format!("10.0.2.2:{}/payload.tar", server.port)));
        assert!(get(server.port, "/meta-data").contains("instance-id: silica-vm"));
        assert!(get(server.port, "/payload.tar").ends_with("\r\n\r\ntarball"));
        assert!(get(server.port, "/etc/passwd").starts_with("HTTP/1.0 404"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_seed_installs_the_session_libraries() {
        let seed = user_data(8000);
        assert!(seed.starts_with("#cloud-config\n"));
        assert!(seed.contains("libseat1"));
        assert!(seed.contains("http://10.0.2.2:8000/payload.tar"));
    }
}
