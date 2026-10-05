use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalBinding {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvaluatedBindingUsage {
    pub(crate) usage_schema_version: u32,
    pub(crate) step_schedule_marker: u32,
    pub(crate) fuel_units: u64,
    pub(crate) fuel_ceiling: u64,
    pub(crate) build_log_bytes: u64,
    pub(crate) filesystem_operation_attempts: u64,
    pub(crate) peak_live_cells: u64,
    pub(crate) peak_live_text_bytes: u64,
    pub(crate) result_cells: u64,
    pub(crate) result_text_bytes: u64,
}

impl PackageReviewEvaluatedBindingUsage {
    pub const fn usage_schema_version(self) -> u32 {
        self.usage_schema_version
    }

    pub const fn step_schedule_marker(self) -> u32 {
        self.step_schedule_marker
    }

    pub const fn fuel_units(self) -> u64 {
        self.fuel_units
    }

    pub const fn fuel_ceiling(self) -> u64 {
        self.fuel_ceiling
    }

    pub const fn build_log_bytes(self) -> u64 {
        self.build_log_bytes
    }

    pub const fn filesystem_operation_attempts(self) -> u64 {
        self.filesystem_operation_attempts
    }

    pub const fn peak_live_cells(self) -> u64 {
        self.peak_live_cells
    }

    pub const fn peak_live_text_bytes(self) -> u64 {
        self.peak_live_text_bytes
    }

    pub const fn result_cells(self) -> u64 {
        self.result_cells
    }

    pub const fn result_text_bytes(self) -> u64 {
        self.result_text_bytes
    }
}

/// Stable, source-handle-free review identity for one evaluated import.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvaluatedImport {
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

impl PackageReviewEvaluatedImport {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn locator(&self) -> &PackageReviewForeignLocator {
        &self.locator
    }

    pub const fn locator_identity_digest(&self) -> [u8; 32] {
        self.locator_identity_digest
    }

    pub const fn producer(&self) -> &PackageReviewNominalIdentity {
        &self.producer
    }

    pub const fn producer_package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.producer_package
    }

    pub fn producer_callable_identity(&self) -> &str {
        &self.producer_callable_identity
    }

    pub const fn producer_closure_digest(&self) -> [u8; 32] {
        self.producer_closure_digest
    }

    pub const fn evaluator_semantics_marker(&self) -> u32 {
        self.evaluator_semantics_marker
    }

    pub const fn evaluation_usage(&self) -> PackageReviewEvaluatedBindingUsage {
        self.evaluation_usage
    }

    pub const fn evaluation_digest(&self) -> [u8; 32] {
        self.evaluation_digest
    }

    pub const fn materializer_schema_version(&self) -> u32 {
        self.materializer_schema_version
    }

    pub const fn materialization_digest(&self) -> [u8; 32] {
        self.materialization_digest
    }

    pub const fn receipt_locator_identity_digest(&self) -> [u8; 32] {
        self.receipt_locator_identity_digest
    }

    pub const fn receipt_identity_digest(&self) -> [u8; 32] {
        self.receipt_identity_digest
    }
}

/// Stable, source-handle-free review identity for one evaluated syscall.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvaluatedSyscall {
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

impl PackageReviewEvaluatedSyscall {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn number(&self) -> i64 {
        self.number
    }

    pub const fn binding_identity_digest(&self) -> [u8; 32] {
        self.binding_identity_digest
    }

    pub const fn producer(&self) -> &PackageReviewNominalIdentity {
        &self.producer
    }

    pub const fn producer_package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.producer_package
    }

    pub fn producer_callable_identity(&self) -> &str {
        &self.producer_callable_identity
    }

    pub const fn producer_closure_digest(&self) -> [u8; 32] {
        self.producer_closure_digest
    }

    pub const fn evaluator_semantics_marker(&self) -> u32 {
        self.evaluator_semantics_marker
    }

    pub const fn evaluation_usage(&self) -> PackageReviewEvaluatedBindingUsage {
        self.evaluation_usage
    }

    pub const fn evaluation_digest(&self) -> [u8; 32] {
        self.evaluation_digest
    }

    pub const fn materializer_schema_version(&self) -> u32 {
        self.materializer_schema_version
    }

    pub const fn materialization_digest(&self) -> [u8; 32] {
        self.materialization_digest
    }

    pub const fn receipt_binding_identity_digest(&self) -> [u8; 32] {
        self.receipt_binding_identity_digest
    }

    pub const fn receipt_identity_digest(&self) -> [u8; 32] {
        self.receipt_identity_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableParameter {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewExternalCallableParameter {
    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }
}
