# Pear

A Docker-like container engine built from scratch in **Rust**, targeting **macOS**.

The goal of Pear is to understand and implement the core pieces behind Docker-style containers rather than simply wrapping Docker, containerd, or an existing runtime.

We are building:

* Linux virtualization on macOS
* Linux namespaces
* cgroups
* container filesystems
* OverlayFS
* container networking
* OCI images
* registry pulling
* process isolation
* container lifecycle management
* macOS ↔ Linux communication
* a user-facing container CLI

The eventual experience should look like:

```bash
pear run alpine echo "Hello from Pear"
```

and produce:

```text
Hello from Pear
```

---

# Why Pear Needs a Linux VM on macOS

Containers rely heavily on Linux kernel functionality such as:

* PID namespaces
* mount namespaces
* network namespaces
* UTS namespaces
* IPC namespaces
* cgroups
* OverlayFS
* capabilities
* `pivot_root`
* Linux networking primitives

macOS does not provide these Linux kernel interfaces.

Therefore Pear uses a small Linux VM underneath macOS.

```text
macOS
│
├── pear
│     user-facing CLI
│
├── pear-host
│     macOS VM manager
│
│     Apple Virtualization.framework
│
└────────────── Linux VM ──────────────
                      │
                      ▼
                pear-runtime
                      │
              ┌───────┼────────┐
              │       │        │
         namespaces cgroups OverlayFS
              │       │        │
              └───────┼────────┘
                      │
                      ▼
                  containers
```

On native Linux, Pear would not need `pear-host`.

The architecture could simply be:

```text
Linux
  │
  ▼
pear
  │
  ▼
pear-runtime
  │
  ├── namespaces
  ├── cgroups
  ├── OverlayFS
  ├── networking
  └── processes
```

`pear-host` exists because macOS needs a Linux kernel before Linux containers can run.

---

# Workspace Structure

Current planned workspace:

```text
pear/
│
├── Cargo.toml
├── README.md
│
├── pear/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
│
├── pear-host/
│   ├── Cargo.toml
│   ├── host.entitlements
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── console.rs
│       ├── storage.rs
│       └── vm.rs
│
├── pear-runtime/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
│
├── vm-assets/
│   ├── linux
│   ├── initrd.gz
│   ├── vmlinuz-6.12.107+deb13-arm64
│   └── initrd.img-6.12.107+deb13-arm64
│
└── vm-data/
    └── root.img
```

The root Cargo workspace will eventually look approximately like:

```toml
[workspace]
resolver = "2"

members = [
    "pear",
    "pear-host",
    "pear-runtime",
]
```

---

# Components

## `pear`

`pear` is the user-facing CLI.

Eventually:

```bash
pear run alpine echo hello
```

```bash
pear run -it debian /bin/bash
```

```bash
pear ps
```

```bash
pear images
```

```bash
pear pull alpine
```

The CLI should not need to understand the implementation details of namespaces, cgroups, or Apple's virtualization APIs.

Its job is to express what the user wants.

```text
user
 │
 ▼
pear
 │
 ▼
pear-host
 │
 ▼
pear-runtime
```

---

# `pear-host`

`pear-host` runs on **macOS**.

Its responsibility is managing the Linux environment required by Pear.

```text
pear-host
│
├── VM lifecycle
├── CPU configuration
├── memory configuration
├── Linux boot
├── virtual disks
├── virtual networking
├── VirtIO console
├── VSOCK
└── communication with pear-runtime
```

It uses Apple's:

```text
Virtualization.framework
```

through Rust bindings provided by:

```text
objc2-virtualization
```

The host should eventually become mostly invisible to users.

A normal user should run:

```bash
pear run alpine
```

rather than manually launching:

```bash
pear-host
```

---

# `pear-runtime`

`pear-runtime` runs **inside the Linux VM**.

This is where the actual container implementation lives.

Eventually it will handle:

```text
pear-runtime
│
├── process creation
├── PID namespaces
├── mount namespaces
├── network namespaces
├── UTS namespaces
├── IPC namespaces
├── cgroups v2
├── root filesystem setup
├── pivot_root
├── OverlayFS
├── capabilities
├── container networking
└── container lifecycle
```

This is the most important part of Pear from a container-runtime perspective.

The VM exists mainly to provide the Linux kernel that `pear-runtime` needs.

---

# Current VM Architecture

Our VM boot path currently looks like:

```text
pear-host
    │
    ▼
VZVirtualMachineConfiguration
    │
    ├── CPU
    ├── RAM
    ├── Linux bootloader
    ├── VirtIO console
    └── VirtIO disk
    │
    ▼
Virtualization.framework
    │
    ▼
Linux kernel
    │
    ▼
Debian initramfs
    │
    ▼
Debian userspace
```

---

# Linux Distribution

Pear currently uses **Debian** to bootstrap its Linux VM.

The default VM boot assets are copied from the installed system's `/boot`:

```text
vm-assets/
├── vmlinuz-6.12.107+deb13-arm64
└── initrd.img-6.12.107+deb13-arm64
```

The older `vm-assets/linux` and `vm-assets/initrd.gz` files are preserved
installer assets, but are no longer the defaults. An installer initramfs starts
the installer even when an installed system exists on the attached disk.

The default boot path is now:

```text
Installed Debian kernel
  │
  ▼
Matching installed-system initramfs
  │
  ▼
Debian on root.img (/dev/vda3)
```

The local `vm-data/root.img` contains an installed **Debian 13.7 (trixie)**
system. Booting it through Pear has been verified through the serial console.
From the workspace root, run:

```bash
make run-host
# Optional resource settings:
make run-host ARGS="--cpus 4 --memory 2048"
```

`run-host` builds, signs with `pear-host/host.entitlements`, and launches the VM.
Use `make sign-host` to build and sign without launching. Re-sign after rebuilding
to retain the virtualization entitlement.

These boot assets and the disk image are local, gitignored files; a fresh
checkout must provision them before launching. See [Installed Debian and boot
assets](#installed-debian-and-boot-assets) for the inspected disk layout.

---

# Dynamic VM Resources

Pear does not hardcode one CPU or memory configuration.

`pear-host` asks Virtualization.framework for the limits supported by the current Mac:

```text
minimumAllowedCPUCount()
maximumAllowedCPUCount()

minimumAllowedMemorySize()
maximumAllowedMemorySize()
```

We can currently launch the host with values such as:

```bash
./target/debug/pear-host \
    --cpus 4 \
    --memory 2048
```

`--memory` is expressed in MiB. The current binary does not accept `--memory-mib`.

Conceptually this will later become:

```bash
pear system start \
    --cpus 4 \
    --memory 2G
```

Invalid configurations are rejected before creating the VM.

For example:

```bash
pear-host --cpus 9999
```

might result in:

```text
invalid CPU count: 9999
supported range: 1..=10
```

The exact limits depend on the Mac.

---

# VirtIO Console

The Linux kernel currently boots with:

```text
console=hvc0 root=UUID=d08ea5d6-3242-4110-843e-e6a52d335173 ro rootwait
```

Pear connects that virtual console to the macOS terminal.

```text
Mac stdin
    │
    ▼
NSFileHandle
    │
    ▼
VirtIO serial console
    │
    ▼
hvc0
    │
    ▼
Linux
    │
    ▼
VirtIO serial console
    │
    ▼
Mac stdout
```

This lets us watch Linux boot directly from `pear-host`.

---

# Persistent Storage

Pear uses:

```text
vm-data/root.img
```

as the VM's persistent virtual disk.

The host exposes it to Linux using VirtIO:

```text
vm-data/root.img
       │
       ▼
VZDiskImageStorageDeviceAttachment
       │
       ▼
VZVirtioBlockDeviceConfiguration
       │
════════ Linux VM boundary ════════
       │
       ▼
VirtIO block driver
       │
       ▼
/dev/vda
```

The important distinction is:

```text
vm-assets/vmlinuz-6.12.107+deb13-arm64
    installed Debian kernel

vm-assets/initrd.img-6.12.107+deb13-arm64
    matching early userspace with storage drivers

vm-data/root.img
    persistent Linux filesystem
```

The local `root.img` contains Debian and boots through Pear. Deploying
`pear-runtime` is still pending. Do not recreate or format this image to resolve
the missing-driver issue in the old installer environment.

## Installed Debian and boot assets

Read-only inspection of the local image confirmed a GPT partition table, Debian
13.7 release files, and an installed-package database. These findings describe
the current local image, not an image provisioned automatically by Pear.

| Partition | Contents confirmed by inspection |
| --- | --- |
| `/dev/vda1` | 16 MiB partition; purpose not established |
| `/dev/vda2` | ext4 filesystem for `/boot` |
| `/dev/vda3` | ext4 root filesystem (`/`) |
| `/dev/vda4` | Swap, recorded in the installed `/etc/fstab` |

The root filesystem UUID is `d08ea5d6-3242-4110-843e-e6a52d335173`.
The disk has a capacity of **20 GiB** (21,474,836,480 bytes), or
**41,943,040 sectors** of 512 bytes each. The host's printed `Disk size` setting
is a creation default, not a measurement of an existing image's capacity.

The installed `/boot` contains a matching pair:

```text
/boot/vmlinuz-6.12.107+deb13-arm64
/boot/initrd.img-6.12.107+deb13-arm64
```

The kernel has a raw ARM64 Image header. Its configuration enables VirtIO PCI
and the VirtIO console as built-ins, with VirtIO block and ext4 as modules. The
matching initramfs was inspected and contains both `virtio_blk.ko.xz` and
`ext4.ko.xz`, along with module dependency metadata. It contains an initial cpio
archive followed by a zstd-compressed archive; it is not a gzip-only image.

### Verified installed-system boot

The installed kernel and matching initramfs were copied read-only from the
`/boot` partition into the versioned host-side paths above. SHA256 checks
confirmed that both copies match their source files; the installer assets were
left untouched.

`pear-host/src/config.rs` selects those files by default.
`pear-host/src/vm.rs` supplies the root UUID via `COMMAND_LINE`, and the host
prints that same command line. Boot verification showed the correct disk
capacity, mounting of `/dev/vda3`, and normal Debian startup through the serial
login service. No login credentials were used during verification.

The root UUID is specific to this local disk image and is currently hardcoded;
there is no kernel-command-line CLI option. For another image, update the root
argument and select that system's matching kernel/initramfs. Keep their versions
matched; do not combine the installed 6.12.107 kernel with the older installer
initramfs. After kernel upgrades inside Debian, refresh the host-side copies
and update the selected paths together.

---

# `pear-host` Modules

The host has been split into separate responsibilities.

```text
pear-host/src/
├── main.rs
├── config.rs
├── console.rs
├── storage.rs
└── vm.rs
```

## `config.rs`

Responsible for:

```text
CPU configuration
memory configuration
kernel path
initramfs path
disk path
disk size
host capability validation
```

Example:

```bash
pear-host \
    --cpus 4 \
    --memory 2048 \
    --kernel vm-assets/vmlinuz-6.12.107+deb13-arm64 \
    --initrd vm-assets/initrd.img-6.12.107+deb13-arm64
```

---

## `console.rs`

Responsible for:

```text
macOS terminal
       ↕
VirtIO serial console
       ↕
Linux hvc0
```

---

## `storage.rs`

Responsible for:

```text
creating root.img
opening root.img
attaching the disk
creating a VirtIO block device
```

---

## `vm.rs`

Responsible for:

```text
VZVirtualMachineConfiguration
Linux bootloader
kernel
initramfs
console attachment
storage attachment
configuration validation
VZVirtualMachine creation
VM startup
```

---

## `main.rs`

`main.rs` should remain mostly orchestration.

Conceptually:

```rust
let settings = VmSettings::from_args();

prepare_storage(&settings);

let vm = vm::create(&settings);

vm::start(&vm);

dispatch_main();
```

Implementation details belong in modules rather than accumulating inside `main.rs`.

---

# Development Progress

## ✅ Phase 1 — Rust Workspace

Created the main workspace components:

```text
pear
pear-host
pear-runtime
```

Status:

```text
COMPLETE
```

---

# Phase 2 — macOS Virtualization

## ✅ Step 2A — Virtualization.framework

Connected Rust to Apple's:

```text
Virtualization.framework
```

Added:

```text
objc2
objc2-foundation
objc2-virtualization
```

Added the required entitlement:

```text
com.apple.security.virtualization
```

Verified:

```rust
VZVirtualMachine::isSupported()
```

Status:

```text
COMPLETE
```

---

## ✅ Step 2B — CPU and Memory Configuration

Created:

```text
VZVirtualMachineConfiguration
```

Added CPU and RAM configuration.

Queried the current Mac for supported limits.

Status:

```text
COMPLETE
```

---

## ✅ Step 2C — Debian Kernel + Initramfs

Added:

```text
vm-assets/linux
vm-assets/initrd.gz
```

Created:

```text
VZLinuxBootLoader
```

Configured:

```text
console=hvc0
```

Status:

```text
COMPLETE
```

---

## ✅ Step 2C.1 — Configurable CPU and RAM

Added:

```text
--cpus
--memory
```

Example:

```bash
pear-host \
    --cpus 4 \
    --memory 2048
```

Status:

```text
COMPLETE
```

---

## ✅ Step 2D — VirtIO Console

Connected:

```text
macOS stdin/stdout
        ↕
VirtIO console
        ↕
Linux hvc0
```

Status:

```text
COMPLETE
```

---

## ✅ Step 2E — Boot Linux

Created:

```text
VZVirtualMachine
```

and started it from Rust.

`pear-host` can now boot a Linux kernel and display its console.

Status:

```text
COMPLETE
```

---

## ✅ Step 2F — Refactor `pear-host`

Split the host into:

```text
config.rs
console.rs
vm.rs
main.rs
```

Status:

```text
COMPLETE
```

---

# Phase 3 — Persistent Storage

## ✅ Step 3 — VirtIO Disk

Host attachment and guest detection verified:

```text
vm-data/root.img
       │
       ▼
VirtIO block device
       │
       ▼
/dev/vda
```

The original installer boot was checked through the VirtIO console:

* `/proc/partitions` and `/sys/block` were empty; `/dev/vda` was absent.
* A VirtIO block device (`0x0002`) was present on the guest bus, with no bound
  driver.
* `virtio_blk` was unavailable in the running `6.12.94+deb13-arm64` installer
  environment. This is a guest-driver issue, not evidence that the image is blank.
* Read-only host inspection confirmed an installed Debian system and a newer,
  matching kernel/initramfs pair with storage support. See
  [Installed Debian and boot assets](#installed-debian-and-boot-assets).

Switching to the installed system's matching kernel/initramfs resolved the
driver issue. Actual guest boot output included:

```text
virtio_blk virtio2: [vda] 41943040 512-byte logical blocks (21.5 GB/20.0 GiB)
vda: vda1 vda2 vda3 vda4
EXT4-fs (vda3): mounted filesystem d08ea5d6-3242-4110-843e-e6a52d335173 ro with ordered data mode. Quota mode: none.
Welcome to Debian GNU/Linux 13 (trixie)!
```

Debian reached its normal serial login service rather than the installer menu.
To repeat the checks after logging in:

```sh
cat /proc/partitions
ls -l /dev/vda
cat /sys/block/vda/size
findmnt /
```

The detection checkpoint passed: Debian recognized `vda` with the expected
`41943040` sectors and mounted the installed root filesystem on `/dev/vda3`.

Status:

```text
COMPLETE
```

---

# Remaining Roadmap

## Step 4 — VM Networking

Add a VirtIO network interface.

```text
Linux VM
   │
   │ eth0
   ▼
VirtIO network
   │
   ▼
Virtualization.framework
   │
   ▼
macOS NAT
   │
   ▼
Internet
```

Goal:

```bash
apt update
```

and normal network connectivity from the VM.

---

## Step 5 — Permanent Minimal Linux System

The installed Debian system now boots from the persistent disk. Further work is
to prepare and minimize it as Pear's runtime environment:

```text
vm-data/root.img
```

Eventually:

```text
Linux kernel
     │
     ▼
root.img
     │
     ├── /bin
     ├── /etc
     ├── /lib
     ├── /usr
     └── /sbin
```

This environment should stay minimal because its primary purpose is running Pear.

---

## Step 6 — Build `pear-runtime` for Linux

`pear-runtime` must be compiled for Linux rather than macOS.

Conceptually:

```text
macOS
  │
  │ cargo build
  ▼
Linux executable
  │
  ▼
VM
  │
  ▼
pear-runtime
```

Eventually the VM might contain:

```text
/usr/local/bin/pear-runtime
```

---

## Step 7 — Host ↔ Runtime Communication

We need communication between:

```text
pear-host
    ↕
pear-runtime
```

The likely transport is VirtIO sockets:

```text
macOS
┌────────────────┐
│ pear-host      │
└───────┬────────┘
        │
      VSOCK
        │
════════╪════════ Linux
        │
┌───────▼────────┐
│ pear-runtime   │
└────────────────┘
```

A future command:

```bash
pear run alpine echo hello
```

will become:

```text
pear
 │
 ▼
pear-host
 │
 ▼
VSOCK
 │
 ▼
pear-runtime
 │
 ▼
container
```

---

## Step 8 — Remote Process Execution

Before building containers, Pear should be able to ask the runtime to execute a normal process inside the VM.

For example:

```bash
pear vm exec uname -a
```

Flow:

```text
pear
  ↓
pear-host
  ↓
pear-runtime
  ↓
execve()
```

This proves the communication protocol works.

---

# Container Runtime Phase

At this point, most macOS-specific virtualization work should be finished.

Development moves primarily into:

```text
pear-runtime
```

---

## Step 9 — Linux Namespaces

Implement namespace isolation using Linux syscalls.

We will use:

```text
CLONE_NEWUTS
CLONE_NEWPID
CLONE_NEWNS
CLONE_NEWIPC
CLONE_NEWNET
```

These isolate:

```text
hostname
process IDs
mounts
IPC
networking
```

This will be the first true container primitive.

---

## Step 10 — Container Root Filesystem

Implement:

```text
mount namespaces
bind mounts
pivot_root
/proc
/dev
/sys
```

A container should see its own:

```text
/
├── bin
├── etc
├── lib
├── proc
├── sys
├── usr
└── var
```

instead of the VM's root filesystem.

---

## Step 11 — cgroups v2

Implement resource controls using:

```text
/sys/fs/cgroup/
```

Including:

```text
memory.max
cpu.max
pids.max
cgroup.procs
```

Eventually:

```bash
pear run \
    --memory 256M \
    --cpus 1 \
    alpine
```

---

## Step 12 — Container Networking

Implement:

* network namespaces
* veth pairs
* bridge networking
* IP allocation
* routing
* NAT
* DNS

Architecture:

```text
                  Linux VM

                   internet
                      │
                      ▼
                     NAT
                      │
                      ▼
                  pear0
                10.0.0.1
                 /      \
                /        \
             veth        veth
              │            │
              ▼            ▼
        container A   container B
         10.0.0.2      10.0.0.3
```

---

## Step 13 — OverlayFS

Container images should remain read-only while containers receive writable layers.

```text
image layers
     │
     ▼
 lowerdir
     │
     ├──────────────┐
     │              │
 upperdir         workdir
     │              │
     └───────┬──────┘
             ▼
           merged
             │
             ▼
        container /
```

---

## Step 14 — OCI Images

Implement support for the OCI image format.

Learn and implement:

```text
manifest
config
layers
digests
content-addressable storage
```

Pear's local image store will eventually contain blobs identified by hashes such as:

```text
sha256:...
```

---

## Step 15 — Registry Pulling

Implement:

```bash
pear pull alpine
```

Flow:

```text
OCI registry
    │
    ▼
manifest
    │
    ▼
config
    │
    ▼
layers
    │
    ▼
Pear content store
```

---

## Step 16 — Container Lifecycle

Implement:

```text
create
start
run
stop
kill
remove
list
```

Eventually:

```bash
pear run alpine
pear ps
pear stop <container>
pear rm <container>
```

---

## Step 17 — Complete Pear CLI

Connect all components into the final user experience.

Target commands:

```bash
pear pull alpine
```

```bash
pear run alpine echo hello
```

```bash
pear run -it debian /bin/bash
```

```bash
pear ps
```

```bash
pear images
```

```bash
pear stop <container>
```

```bash
pear rm <container>
```

---

# Overall Progress

Current high-level roadmap:

```text
 1. Rust workspace                    ✅
 2. macOS virtualization              ✅
 3. Persistent storage                ✅
 4. VM networking                     ⬜
 5. Permanent Linux VM                ⬜
 6. pear-runtime Linux binary         ⬜
 7. Host/runtime communication        ⬜
 8. Remote process execution          ⬜
 9. Linux namespaces                  ⬜
10. Container rootfs                  ⬜
11. cgroups v2                        ⬜
12. Container networking              ⬜
13. OverlayFS                         ⬜
14. OCI images                        ⬜
15. Registry pulling                  ⬜
16. Container lifecycle               ⬜
17. Complete pear CLI                 ⬜
```

Current count:

```text
Major phases:         17
Completed:             3
In progress:           0
Remaining afterward:  14
```

Phase 2 itself contained several significant milestones:

```text
2A   Virtualization.framework       ✅
2B   CPU/RAM configuration          ✅
2C   Debian bootloader              ✅
2C1  Dynamic CPU/RAM                ✅
2D   VirtIO console                 ✅
2E   Boot Linux                     ✅
2F   Host refactor                  ✅
```

A rough implementation-progress estimate is:

```text
██████░░░░░░░░░░░░░░   ~25–30%
```

The largest remaining portion is the actual Linux container runtime.

---

# Final Architecture

The target architecture for Pear is:

```text
                         macOS

                  ┌────────────────┐
                  │      pear      │
                  │      CLI       │
                  └───────┬────────┘
                          │
                          ▼
                  ┌────────────────┐
                  │   pear-host    │
                  │                │
                  │ VM lifecycle   │
                  │ disks          │
                  │ networking     │
                  │ VSOCK          │
                  └───────┬────────┘
                          │
                        VSOCK
                          │

════════════════════ Linux VM ════════════════════

                          │
                          ▼
                ┌────────────────────┐
                │    pear-runtime    │
                │                    │
                │ namespaces         │
                │ cgroups            │
                │ OverlayFS          │
                │ networking         │
                │ process lifecycle  │
                └─────────┬──────────┘
                          │
                ┌─────────┴─────────┐
                │                   │
                ▼                   ▼
          ┌───────────┐       ┌───────────┐
          │ Container │       │ Container │
          │     A     │       │     B     │
          │           │       │           │
          │  Alpine   │       │  Debian   │
          └───────────┘       └───────────┘
```

---

# Current Position

We are currently here:

```text
macOS
  │
  ▼
pear-host
  │
  ▼
Virtualization.framework
  │
  ├── configurable CPU ✅
  ├── configurable RAM ✅
  ├── Debian kernel ✅
  ├── Debian initramfs ✅
  ├── VirtIO console ✅
  ├── Linux boot ✅
  └── persistent VirtIO disk ✅
```

The storage checkpoint is verified:

```text
vm-data/root.img
       │
       ▼
VirtIO
       │
       ▼
Debian
       │
       ▼
/dev/vda
```

The next major step is networking verification:

```text
Step 4

pear-host
    │
    ▼
VirtIO network adapter
    │
    ▼
Linux eth0
    │
    ▼
macOS NAT
    │
    ▼
Internet
```

From there we can validate and minimize the installed Linux environment that
will run `pear-runtime`.

---

# End Goal

The project succeeds when this works:

```bash
pear run alpine echo "Hello from Pear"
```

and Pear itself performs the entire path:

```text
pear
 ↓
pear-host
 ↓
Linux VM
 ↓
pear-runtime
 ↓
OCI image
 ↓
namespaces
 ↓
cgroups
 ↓
OverlayFS
 ↓
container process
```

without depending on Docker to run the container.
