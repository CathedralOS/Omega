use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewExternalBinding {
    /// Ordinary typed `ForeignBinding::DllImport` evaluation: the only import row.
    /// The string-backed `Import { library, symbol }` row is retired, as is
    /// the typed identity behind it; review never reinterprets two
    /// independently authored strings as one atomic physical locator.
    NormalizedImport(PackageReviewEvaluatedImport),
    /// Ordinary typed `ForeignBinding::Syscall` evaluation. This remains distinct
    /// from the legacy integer syscall carrier and retains its exact producer,
    /// target, evaluator, and materializer receipt.
    NormalizedSyscall(PackageReviewEvaluatedSyscall),
    Syscall {
        number: i64,
    },
    CompilerIntrinsic,
    VtableSlot {
        index: i64,
    },
    VtableField {
        field: String,
    },
    TableFunction {
        field: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewForeignLocator {
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
    MachODylibSymbol {
        install_name: Vec<u8>,
        symbol: Vec<u8>,
    },
}

impl PackageReviewForeignLocator {
    /// The review-record locator for one provider-plan candidate: a
    /// byte-for-byte carry of the candidate's coordinates into the review
    /// vocabulary.
    pub(crate) fn from_candidate(candidate: &target::ForeignLocatorCandidate) -> Self {
        match candidate {
            target::ForeignLocatorCandidate::PeByName { library, export } => Self::PeByName {
                library: library.clone(),
                export: export.clone(),
            },
            target::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                Self::PeByOrdinal {
                    library: library.clone(),
                    ordinal: *ordinal,
                }
            }
            target::ForeignLocatorCandidate::ElfVersioned {
                object,
                symbol,
                version,
            } => Self::ElfVersioned {
                object: object.clone(),
                symbol: symbol.clone(),
                version: version.clone(),
            },
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name,
                symbol,
            } => Self::MachODylibSymbol {
                install_name: install_name.clone(),
                symbol: symbol.clone(),
            },
        }
    }
}

/// The review record retains the provider's evaluated-usage carrier verbatim.
pub(crate) type PackageReviewEvaluatedBindingUsage = effects::provider_plan::EvaluatedBindingUsage;

/// Stable, source-handle-free review identity for one evaluated import.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewEvaluatedImport {
    pub(crate) target: String,
    pub(crate) locator: PackageReviewForeignLocator,
    pub(crate) locator_identity_digest: [u8; 32],
    pub(crate) producer: PackageReviewNominalIdentity,
    pub(crate) producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) producer_callable_identity: String,
    pub(crate) producer_closure_digest: [u8; 32],
    pub(crate) evaluator_semantics_marker: u32,
    pub(crate) evaluation_usage: PackageReviewEvaluatedBindingUsage,
    pub(crate) evaluation_digest: [u8; 32],
    pub(crate) materializer_schema_version: u32,
    pub(crate) materialization_digest: [u8; 32],
    pub(crate) receipt_locator_identity_digest: [u8; 32],
    pub(crate) receipt_identity_digest: [u8; 32],
}

/// Stable, source-handle-free review identity for one evaluated syscall.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewEvaluatedSyscall {
    pub(crate) target: String,
    pub(crate) number: i64,
    pub(crate) binding_identity_digest: [u8; 32],
    pub(crate) producer: PackageReviewNominalIdentity,
    pub(crate) producer_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) producer_callable_identity: String,
    pub(crate) producer_closure_digest: [u8; 32],
    pub(crate) evaluator_semantics_marker: u32,
    pub(crate) evaluation_usage: PackageReviewEvaluatedBindingUsage,
    pub(crate) evaluation_digest: [u8; 32],
    pub(crate) materializer_schema_version: u32,
    pub(crate) materialization_digest: [u8; 32],
    pub(crate) receipt_binding_identity_digest: [u8; 32],
    pub(crate) receipt_identity_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableParameter {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewExternalCallableParameter {
    field_projection_accessors!(borrowed type_identity: &PackageReviewTypeIdentity);
    field_projection_accessors!(copied is_const: bool, is_mutable: bool);
}
