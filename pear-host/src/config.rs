use std::path::PathBuf;

use clap::Parser;
use objc2_virtualization::VZVirtualMachineConfiguration;

#[derive(Parser)]
// #[command(name = "pear-host")]
struct Args {
    #[arg(long)]
    cpus: Option<usize>,
    #[arg(long)]
    memory: Option<u64>,
    #[arg(long, default_value = "vm-assets/linux")]
    kernel: PathBuf,
    #[arg(long, default_value = "vm-assets/initrd.gz")]
    initrd: PathBuf,
    #[arg(long, default_value = "vm-data/root.img")]
    disk: PathBuf,

    #[arg(long, default_value_t = 8)]
    disk_gib: u64,
}

const MIB: u64 = 1024 * 1024;

pub struct VMSettings {
    pub cpus: usize,
    pub memory: u64,

    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub disk: PathBuf,
    pub disk_gib: u64,
}

impl VMSettings {
    pub fn from_args() -> Self {
        let args = Args::parse();

        if !args.kernel.exists() {
            eprintln!("Kernel path not found at {:?}", args.kernel);
            std::process::exit(1);
        }
        if !args.initrd.exists() {
            eprintln!("Initrd path not found at {:?}", args.initrd);
            std::process::exit(1);
        }

        unsafe {
            let min_cpus = VZVirtualMachineConfiguration::minimumAllowedCPUCount();
            let max_cpus = VZVirtualMachineConfiguration::maximumAllowedCPUCount();
            let min_memory = VZVirtualMachineConfiguration::minimumAllowedMemorySize();
            let max_memory = VZVirtualMachineConfiguration::maximumAllowedMemorySize();

            let default_cpus = 2usize.clamp(min_cpus, max_cpus);
            let default_memory = (1024 * MIB).clamp(min_memory, max_memory);

            let cpus = args.cpus.unwrap_or(default_cpus);
            let memory = args.memory.map(|m| m * MIB).unwrap_or(default_memory);

            if cpus < min_cpus || cpus > max_cpus {
                eprintln!("Invalid CPU count: {cpus}");
                eprintln!("This Mac supports {min_cpus} to {max_cpus} virtual CPUs");
                std::process::exit(1);
            }
            if memory < min_memory || memory > max_memory {
                eprintln!("Invalid memory: {} MiB", memory / MIB);
                eprintln!(
                    "This Mac supports {} to {} MiB",
                    min_memory / MIB,
                    max_memory / MIB
                );
                std::process::exit(1);
            }

            if memory % MIB != 0 {
                eprintln!("Memory must be a multiple of 1 MiB");
                std::process::exit(1);
            }

            if args.disk_gib == 0 {
                eprintln!("Disk must be at least of 1 GiB");
                std::process::exit(1);
            }

            Self {
                cpus,
                memory: memory / MIB,
                kernel: args.kernel,
                initrd: args.initrd,
                disk: args.disk,
                disk_gib: args.disk_gib,
            }
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.memory * MIB
    }
}
