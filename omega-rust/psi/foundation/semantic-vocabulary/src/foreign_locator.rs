//! Owned native-locator coordinates before target validation or admission.
//! These data grant neither execution authority nor foreign contract evidence.

/// Untrusted owned coordinates produced by future ordinary `Binding` value
/// evaluation. Vector lengths retain the fixed-array widths of that value;
/// unused coordinates are absent by construction rather than normalized from
/// an independently supplied field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignLocatorCandidate {
    PeByName {
        library: Vec<u8>,
        export: Vec<u8>,
    },
    PeByOrdinal {
        library: Vec<u8>,
        ordinal: u16,
    },
    ElfVersioned {
        object: Vec<u8>,
        symbol: Vec<u8>,
        version: Vec<u8>,
    },
    /// One exact two-level-namespace Mach-O import. `install_name` is the raw
    /// `LC_LOAD_DYLIB` identity; `symbol` is the raw dyld symbol spelling. The
    /// image-local dylib ordinal is derived from load-command order and is not
    /// locator identity.
    MachODylibSymbol {
        install_name: Vec<u8>,
        symbol: Vec<u8>,
    },
}

impl ForeignLocatorCandidate {
    pub const fn case_name(&self) -> &'static str {
        match self {
            Self::PeByName { .. } => "PeByName",
            Self::PeByOrdinal { .. } => "PeByOrdinal",
            Self::ElfVersioned { .. } => "ElfVersioned",
            Self::MachODylibSymbol { .. } => "MachODylibSymbol",
        }
    }
}

/// A typed native binding value; syscall applicability and import-format
/// validation remain with target realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBindingLocator {
    Import(ForeignLocatorCandidate),
    Syscall { number: u64 },
}
