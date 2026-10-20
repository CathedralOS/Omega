//! Native inet address value primitives for explicit trusted runtime supplies.
//! This owner knows C's four-byte `in_addr` ABI, never guest type names, selected
//! representations, source contracts or interpreter occurrence identities.

#[repr(C)]
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
            // SAFETY: the C arguments are exactly in_addr_t (unsigned 32-bit),
            // and repr(C) retains the single 32-bit member of struct in_addr.
            // Neither operation borrows or retains caller storage.
            Some(unsafe { inet_makeaddr(network, host) })
        }
        #[cfg(not(unix))]
        {
            let _ = (network, host);
            None
        }
    }

    pub fn host_number(self) -> Option<u32> {
        #[cfg(unix)]
        {
            // SAFETY: this is the actual owned struct returned by inet_makeaddr,
            // passed by value under the same native C ABI, without guest pointers.
            Some(unsafe { inet_lnaof(self) })
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::HostInternetAddress;

    #[test]
    fn native_inet_struct_return_and_parameter_preserve_distinct_host_numbers() {
        for (network, host) in [(10, 66051), (44048, 513), (10, 1027)] {
            assert_eq!(
                HostInternetAddress::make(network, host)
                    .unwrap()
                    .host_number(),
                Some(host)
            );
        }
    }
}
