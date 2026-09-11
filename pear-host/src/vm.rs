use block2::RcBlock;
use dispatch2::dispatch_main;
use objc2::{AnyThread, rc::Retained};
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_virtualization::{VZLinuxBootLoader, VZVirtualMachine, VZVirtualMachineConfiguration};

use crate::{config::VMSettings, console, network, storage};

pub fn create(settings: &VMSettings) -> Retained<VZVirtualMachine> {
    unsafe {
        if !VZVirtualMachine::isSupported() {
            eprintln!("Virtualization.framework is not supported");
            std::process::exit(1);
        }

        let kernel_url = NSURL::from_file_path(&settings.kernel).unwrap();
        let initrd_url = NSURL::from_file_path(&settings.initrd).unwrap();

        let config = VZVirtualMachineConfiguration::new();

        config.setCPUCount(settings.cpus);
        config.setMemorySize(settings.memory_bytes());

        let bootloader =
            VZLinuxBootLoader::initWithKernelURL(VZLinuxBootLoader::alloc(), &kernel_url);
        bootloader.setInitialRamdiskURL(Some(&initrd_url));
        let command_line = NSString::from_str("console=hvc0");
        bootloader.setCommandLine(&command_line);
        config.setBootLoader(Some(&bootloader));

        console::attach(&config);
        network::attach(&config);
        storage::attach(&config, settings);

        if let Err(error) = config.validateWithError() {
            println!("Configuration is invalid");
            println!("{error:?}");
            std::process::exit(1);
        }
        println!("Configuration is valid");

        VZVirtualMachine::initWithConfiguration(VZVirtualMachine::alloc(), &config)
    }
}

pub fn start(vm: &Retained<VZVirtualMachine>) {
    println!("Starting vm...");

    let completion = RcBlock::new(|error: *mut NSError| {
        if error.is_null() {
            println!("VM started successfully");
        } else {
            let error = unsafe { &*error };
            eprintln!("Failed to start VM");
            eprintln!("{error:}");
            std::process::exit(1);
        }
    });
    unsafe { vm.startWithCompletionHandler(&completion) };
    dispatch_main();
}
