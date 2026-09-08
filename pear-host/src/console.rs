use objc2::AnyThread;
use objc2_foundation::{NSArray, NSFileHandle};
use objc2_virtualization::{
    VZFileHandleSerialPortAttachment, VZVirtioConsoleDeviceSerialPortConfiguration,
    VZVirtualMachineConfiguration,
};

pub fn attach(config: &VZVirtualMachineConfiguration) {
    let stdin = NSFileHandle::fileHandleWithStandardInput();
    let stdout = NSFileHandle::fileHandleWithStandardError();

    unsafe {
        let console_attachment =
            VZFileHandleSerialPortAttachment::initWithFileHandleForReading_fileHandleForWriting(
                VZFileHandleSerialPortAttachment::alloc(),
                Some(&stdin),
                Some(&stdout),
            )
            .into_super();

        let serial_port = VZVirtioConsoleDeviceSerialPortConfiguration::new();
        serial_port.setAttachment(Some(&console_attachment));
        let serial_port = serial_port.into_super();
        let serial_ports = NSArray::from_retained_slice(&[serial_port]);
        config.setSerialPorts(&serial_ports);
    }
}
