use objc2_foundation::NSArray;
use objc2_virtualization::{
    VZNATNetworkDeviceAttachment, VZVirtioNetworkDeviceConfiguration, VZVirtualMachineConfiguration,
};

pub fn attach(config: &VZVirtualMachineConfiguration) {
    unsafe {
        let attachment = VZNATNetworkDeviceAttachment::new().into_super();

        let network_device = VZVirtioNetworkDeviceConfiguration::new();
        network_device.setAttachment(Some(&attachment));

        let network_device = network_device.into_super();
        let network_devices = NSArray::from_retained_slice(&[network_device]);

        config.setNetworkDevices(&network_devices);
    }
}
