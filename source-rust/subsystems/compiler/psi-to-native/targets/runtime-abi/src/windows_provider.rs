//! Select the bundled Windows x64 Internet provider for compatible foreign imports.
pub const WINDOWS_INTERNET_PROVIDER_LIBRARY: &[u8] = b"omega_host.dll";

pub fn imports_windows_internet_provider(locator: target::NormalizedImportLocatorRef<'_>) -> bool {
    needs_windows_internet_companion([locator])
}

pub fn needs_windows_internet_companion<'a>(
    imports: impl IntoIterator<Item = target::NormalizedImportLocatorRef<'a>>,
) -> bool {
    let mut requested = false;
    for import in imports {
        let target::NormalizedImportLocatorRef::Foreign(foreign) = import else {
            continue;
        };
        if foreign.target() != target::TargetProfile::WindowsX64 {
            continue;
        }
        let (library, exported) = match foreign.locator() {
            target::ForeignLocatorCandidate::PeByName { library, export } => (
                library,
                matches!(export.as_slice(), b"inet_lnaof" | b"inet_makeaddr"),
            ),
            target::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                (library, matches!(ordinal, 1 | 2))
            }
            _ => continue,
        };
        if !library.eq_ignore_ascii_case(WINDOWS_INTERNET_PROVIDER_LIBRARY) {
            continue;
        }
        // All imports of this filename will resolve to the same DLL at load time.
        if !exported {
            return false;
        }
        requested |= library == WINDOWS_INTERNET_PROVIDER_LIBRARY;
    }
    requested
}
