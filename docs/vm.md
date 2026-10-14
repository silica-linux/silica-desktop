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
   session into `/opt/silica` and starts `seatd-launch -- ossido --backend drm` with the shell
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
