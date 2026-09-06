mod config;
mod console;
mod vm;

use crate::config::VMSettings;

fn main() {
    let settings = VMSettings::from_args();

    println!("VM configuration");
    println!("----------------");
    println!("CPUs:       {}", settings.cpus);
    println!("RAM:        {} MiB", settings.memory);
    println!("Kernel:     {}", settings.kernel.display());
    println!("Initramfs:  {}", settings.initrd.display());
    println!("Cmdline:    console=hvc0");
    println!();

    let vm = vm::create(&settings);
    vm::start(&vm);
}
