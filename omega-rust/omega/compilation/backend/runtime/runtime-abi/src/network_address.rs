//! Native inet address value primitives for explicit trusted runtime supplies.
//! This owner knows C's four-byte `in_addr` ABI, never guest type names, selected
//! representations, source contracts or interpreter occurrence identities.
//!
//! Unix hosts call the platform `inet_makeaddr` and `inet_lnaof`. Windows has
//! neither export, so the same classful definition those libraries implement
//! (glibc `inet/inet_makeaddr.c` and `inet/inet_lnaof.c`) is the Windows
//! provider. `windows_provider` emits that definition as `omega_host.dll` for
//! the native loader; this module evaluates it for Terminal execution. The
//! two must agree: a physical copy of the four-byte result is not a second
//! semantic value, and the guest still cannot manufacture the representation.

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
            // SAFETY: the C arguments are exactly in_addr_t (unsigned 32-bit),
            // and repr(C) retains the single 32-bit member of struct in_addr.
            // Neither operation borrows or retains caller storage.
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
            // SAFETY: this is the actual owned struct returned by inet_makeaddr,
            // passed by value under the same native C ABI, without guest pointers.
            Some(unsafe { inet_lnaof(self) })
        }
        #[cfg(not(unix))]
        {
            Some(classful_local_address(self.address))
        }
    }
}

/// Host-order classful address, then numeric byte swap into network order.
///
/// The thresholds are the historical class A/B/C cuts, not the witness
/// constants. Class D and E fall through to `network | host`, matching glibc.
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

/// Inverse of `classful_internet_address`: network-order word back to host
/// order, then the class mask of that resulting address. The class is read
/// from the address, not from the network argument that produced it.
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
        }
    }
}
