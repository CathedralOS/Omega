//! Library coordinate shared by the native import contract and host execution.
pub const WINDOWS_INTERNET_PROVIDER_LIBRARY: &[u8] = b"omega_host.dll";

pub fn imports_windows_internet_provider(locator: target::NormalizedImportLocatorRef<'_>) -> bool {
    let target::NormalizedImportLocatorRef::Foreign(foreign) = locator else {
        return false;
    };
    foreign.target() == target::TargetProfile::WindowsX64
        && matches!(foreign.locator(),
            target::ForeignLocatorCandidate::PeByName { library, .. } |
            target::ForeignLocatorCandidate::PeByOrdinal { library, .. }
            if library == WINDOWS_INTERNET_PROVIDER_LIBRARY)
}
