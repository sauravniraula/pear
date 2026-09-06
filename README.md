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
│   └── initrd.gz
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

The VM boot assets are:

```text
vm-assets/
├── linux
└── initrd.gz
```

`linux` is the Debian Linux kernel.

`initrd.gz` is the Debian installer initramfs.

For now:

```text
linux
  │
  ▼
initrd.gz
  │
  ▼
Debian installer/userspace
```

Later we will boot into our own persistent minimal Linux environment instead of depending on the installer initramfs.

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
    --memory-mib 2048
```

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
console=hvc0
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
vm-assets/linux
    Linux kernel

vm-assets/initrd.gz
    temporary early userspace

vm-data/root.img
    persistent Linux filesystem
```

Eventually `root.img` will contain the Linux system that runs `pear-runtime`.

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
    --memory-mib 2048 \
    --kernel vm-assets/linux \
    --initrd vm-assets/initrd.gz
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
--memory-mib
```

Example:

```bash
pear-host \
    --cpus 4 \
    --memory-mib 2048
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

## 🚧 Step 3 — VirtIO Disk

Added/planned:

```text
vm-data/root.img
       │
       ▼
VirtIO block device
       │
       ▼
/dev/vda
```

The next verification is making sure Debian sees the disk as a block device.

Status:

```text
IN PROGRESS
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

Replace the temporary installer environment with a persistent Linux filesystem living on:

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
 3. Persistent storage                🚧
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
Completed:             2
In progress:           1
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
  └── persistent VirtIO disk 🚧
```

The immediate next checkpoint is:

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

Once that is verified, the next major step is:

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

From there we can build the permanent Linux environment that will run `pear-runtime`.

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
