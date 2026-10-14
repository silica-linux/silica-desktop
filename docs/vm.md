# The real session in a virtual machine

`cargo xtask vm` runs Silica the way a real session does, without a host compositor: Ossido
on DRM/KMS with libinput and libseat (`--backend drm`, `crates/ossido/src/drm.rs`), the
shell on top, inside a QEMU/KVM virtual machine. It is the first step of M9 (real session,
in a VM first) and runs in CI on every push (`.github/workflows/vm.yml`).

## What it does

1. Builds Ossido, the embedder and `silica-daemon` in release, downloads the debug Flutter
   engine and builds the shell's debug bundle (no engine build from source is needed: the
   debug engine runs the JIT bundle).
2. Packs them with `run-session.sh` into `target/vm/payload.tar`.
3. Downloads the Ubuntu cloud image of the **same release as the host**
   (`VERSION_CODENAME` in `/etc/os-release`, or `SILICA_VM_RELEASE`): the binaries are built
   on the host and need the same glibc. The image is kept in `target/vm/` and the guest boots
   from a throwaway qcow2 overlay.
4. Serves the cloud-init configuration (NoCloud, `-smbios type=1,serial=ds=nocloud;s=…`) and
   the tarball over HTTP from xtask itself: QEMU's user network reaches the host's loopback at
   `10.0.2.2`. No ISO tools or 9p shares are needed.
5. The guest installs the runtime libraries (seatd, Mesa, libinput, libseat, …), unpacks the
   session into `/opt/silica` and starts `ossido --backend drm` on the guest's seatd service with the shell
   components (embedder with `--renderer software`).
6. Ossido's control channel listens on the guest's loopback (7991); socat exposes it on 7990,
   which QEMU forwards to the host (default 7990, never 7979/7980, used by the sessions on the
   host).
7. xtask waits for the channel, checks that the serial console shows `drm backend ready`
   (Ossido really runs on DRM), takes `ui.screenshot` and fails if the frame is black.
   The screenshot and the serial console are in `target/vm/` (`screenshot.png`,
   `console.log`).

Choice of image: an Ubuntu cloud image with cloud-init, rather than mkosi or debootstrap,
because it needs no root on the host (mkosi and debootstrap do), it is a single download, and
its release can match the build host, which CI's Ubuntu runner and the development machines
already are. A Fedora image is the natural next step with the D-track packages
(`silica-packages`), when the session is installed from RPMs instead of a tarball.

The GPU is `virtio-vga` without 3D: in the guest Mesa renders with llvmpipe into GBM dumb
buffers (`kms_swrast`), slow but deterministic, like headless CI.

## Options

```text
cargo xtask vm [--port N] [--timeout SECONDS] [--out PATH] [--show] [--keep] [--payload-only]
```

- `--show`: a GTK window with the guest's screen, to watch or use the session (keyboard and
  tablet are virtio devices, read by libinput).
- `--keep`: leaves the VM running after the check.
- `--payload-only`: only builds `payload.tar` (no QEMU needed).
- `--timeout`: seconds to wait for the control channel (default 900: the first boot installs
  packages).

## Host requirements

- `qemu-system-x86_64` and `qemu-img`: on Ubuntu `sudo apt install qemu-system-x86 qemu-utils`.
- KVM: `/dev/kvm` readable and writable by the user, on Ubuntu
  `sudo usermod -aG kvm $USER` and a new login. Without KVM the VM is emulated (TCG), many
  times slower.
- Network access for the image download and the guest's packages.
- To build Ossido with the `drm` feature (default): `libudev-dev libinput-dev libseat-dev
  libgbm-dev` (already in CI's dependencies).

## Development loop: `cargo xtask os`

All development and testing happens inside the real Silica OS, not only in the nested session
on the host: build here, check in the VM, then push to CI. `cargo xtask os` runs a released
Silica OS disk (the qcow2 of a silica-os release, in `~/silica-vm/`) under QEMU/KVM, in a
window on the user's screen, so the work can be watched while it happens. Keep one VM running
for the whole session: `deploy` restarts the Silica session inside it.

```bash
gh release download -R silica-linux/silica-os -p '*.qcow2' -D ~/silica-vm   # once
cargo xtask os start          # the VM in a full-screen window ("Silica OS (dev)")
cargo xtask os deploy         # build Ossido, the embedder, the services and the shell, run them
cargo xtask os deploy --apps  # the apps too (mustache, mail, terminal, clock, calculator, settings)
cargo xtask os deploy --apps=terminal,mail  # only some of them
cargo xtask os shot out.png   # screenshot through Ossido's control channel
cargo xtask os cmd window.list            # any silica-cli command
cargo xtask os ssh 'journalctl -b -p warning'   # a root shell in the guest
cargo xtask os log            # the deployed session's log (Ossido, the embedders)
cargo xtask os restart        # restart the session (automatic login again)
cargo xtask os status | stop
```

`start` options: `--image PATH`, `--fresh` (a new overlay disk), `--size WxH` (default
1920x1200), `--headless` (no window, for CI), and the graphics:

| Option | Device | Ossido | Shell and apps |
| --- | --- | --- | --- |
| (default) | `virtio-vga-gl` with virgl and Venus | GLES on the host's GPU | Impeller on Vulkan (Venus) |
| `--virgl` | `virtio-vga-gl` with virgl | GLES on the host's GPU | Impeller on GLES |
| `--software` | `virtio-vga` | llvmpipe | software |

How it works, without root on the host and without changing the image:

- **Credentials.** systemd credentials passed through SMBIOS (`-smbios type=11`) give the
  guest root's SSH key, `/etc/silica/session.env.d/50-dev.env` (Ossido's control channel on
  the guest's loopback, the embedder's renderer, `RUST_LOG`) and a service that mounts the
  share. Nothing persists in the image: the overlay disk holds the guest's state.
- **Ports.** SSH on 127.0.0.1:7998 (guest 22), Ossido's control channel on 127.0.0.1:7993
  through an SSH tunnel (guest 7991: SELinux keeps socket proxies from binding a port),
  QEMU's monitor on 127.0.0.1:7996. Never 7979/7980, the sessions on the host.
- **The share.** `~/build/silica-os-dev/share` is mounted read-only with 9p on
  `/run/silica-dev`: `bin/` (Ossido, embedder, services, CLI), `engine/`, `shell/`, `apps/`,
  `share/applications/` (the apps' `.desktop` entries, before the installed ones), `icons/`
  (silica-icons) and `session.sh`. The hook in `50-dev.env` runs `session.sh` instead of the
  installed session when it exists; silica-session also has `SILICA_PREFIX` for the same.
- **Binaries.** Built on the host: its glibc must not be newer than Fedora 44's (2.43),
  which `deploy` checks.
- **Restart.** greetd logs in automatically only once per boot (its run file in `/run`):
  `deploy` and `restart` remove it and restart greetd.
- **The window.** QEMU runs with `GDK_BACKEND=x11`; with `wmctrl` installed `start` moves it
  to the second workspace.

The work directory is `~/build/silica-os-dev` (`SILICA_OS_DIR`).

## Looking at animations and frames

- `SILICA_TIME_DILATION=10` in the shell's environment slows every animation of the shell
  tenfold (Flutter's `timeDilation`), to take screenshots through a transition.
- `RUST_LOG=ossido::handlers=trace,ossido::layers=trace` logs every commit of every surface:
  the layer namespace, whether it carries a new buffer and its glass regions (count and
  highest opacity), and the layers whose buffer does not have the configured size (not
  drawn yet). A glass change on a commit without a new buffer is a frame early or late: the
  embedder commits the glass regions with the frame they were measured on (`GlassSync` in
  `crates/embedder/src/glass.rs`).
