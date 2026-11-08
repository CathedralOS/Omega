/*
We evaluate native IPv4 classful internet-address operations (`inet_makeaddr`
and `inet_lnaof`) over C's 4-byte `in_addr` representation
(`HostInternetAddress`).

Unix hosts call libc's `inet_makeaddr` and `inet_lnaof` directly; non-Unix
hosts evaluate the identical class A/B/C bit-packing and network-order byte
swap that `windows_provider.rs` emits into `omega_host.dll`.
*/

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostInternetAddress {
    address: u32,
}

#[cfg(unix)]
unsafe extern "C" {
    fn inet_makeaddr(network: u32, host: u32) -> HostInternetAddress;
    fn inet_lnaof(address: HostInternetAddress) -> u32;
}

impl HostInternetAddress {
    pub fn make(network: u32, host: u32) -> Option<Self> {
        #[cfg(unix)]
        {
            Some(unsafe { inet_makeaddr(network, host) })
        }
        #[cfg(not(unix))]
        {
            Some(Self {
                address: classful_internet_address(network, host),
            })
        }
    }

    pub fn host_number(self) -> Option<u32> {
        #[cfg(unix)]
        {
            Some(unsafe { inet_lnaof(self) })
        }
        #[cfg(not(unix))]
        {
            Some(classful_local_address(self.address))
        }
    }
}

#[cfg(any(not(unix), test))]
pub(super) fn classful_internet_address(network: u32, host: u32) -> u32 {
    let host_order = if network < 128 {
        (network << 24) | (host & 0x00ff_ffff)
    } else if network < 65_536 {
        (network << 16) | (host & 0x0000_ffff)
    } else if network < 16_777_216 {
        (network << 8) | (host & 0x0000_00ff)
    } else {
        network | host
    };
    host_order.swap_bytes()
}

#[cfg(any(not(unix), test))]
pub(super) fn classful_local_address(stored: u32) -> u32 {
    let value = stored.swap_bytes();
    if value & 0x8000_0000 == 0 {
        value & 0x00ff_ffff
    } else if value & 0xc000_0000 == 0x8000_0000 {
        value & 0x0000_ffff
    } else {
        value & 0x0000_00ff
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::HostInternetAddress;
    use super::{classful_internet_address, classful_local_address};

    #[test]
    fn classful_definition_preserves_the_witness_host_numbers() {
        for (network, host) in [(10, 66051), (44048, 513), (10, 1027)] {
            let stored = classful_internet_address(network, host);
            assert_eq!(classful_local_address(stored), host);
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_inet_struct_return_and_parameter_preserve_distinct_host_numbers() {
        for (network, host) in [(10, 66051), (44048, 513), (10, 1027)] {
            let made = HostInternetAddress::make(network, host).unwrap();
            assert_eq!(made.address, classful_internet_address(network, host));
            assert_eq!(made.host_number(), Some(host));
            assert_eq!(made.host_number(), Some(host));
        }
    }
}
