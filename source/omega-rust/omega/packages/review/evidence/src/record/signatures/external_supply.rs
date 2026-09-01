use super::*;

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalBinding {
    Import {
        library: String,
        symbol: String,
    },
    /// Ordinary typed `Binding::DllImport` evaluation. This remains distinct
    /// from the legacy string-backed import so review can never reinterpret
    /// two independently authored strings as one atomic physical locator.
    NormalizedImport(PackageReviewEvaluatedImport),
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
    pub(crate) producer_package: Option<psi_core::PackageKeyIdentity>,
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

    pub const fn producer_package(&self) -> Option<psi_core::PackageKeyIdentity> {
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

/// One trust-bearing association between an exact reviewed callable,
/// requirement application, and externally supplied executable mechanism.
/// This is not Terminal evidence and makes no implementation-correctness or
/// audit claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalRequirement {
    Trait(PackageReviewCallableConformance),
    Operator(PackageReviewOperatorCoordinate),
    TopLevelRequirement {
        identity: PackageReviewNominalIdentity,
        signature: PackageReviewExternalCallableSignature,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableParameter {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
    pub(crate) is_self: bool,
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

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalStaticParameter {
    Type {
        properties: PackageReviewDataProperties,
    },
    Const {
        type_identity: PackageReviewTypeIdentity,
    },
    Machine {
        contract: PackageReviewMachineParameterContract,
    },
}

impl PackageReviewExternalStaticParameter {
    pub const fn type_properties(&self) -> Option<PackageReviewDataProperties> {
        match self {
            Self::Type { properties } => Some(*properties),
            Self::Const { .. } | Self::Machine { .. } => None,
        }
    }

    pub const fn const_type_identity(&self) -> Option<&PackageReviewTypeIdentity> {
        match self {
            Self::Type { .. } => None,
            Self::Const { type_identity } => Some(type_identity),
            Self::Machine { .. } => None,
        }
    }

    pub const fn machine_contract(&self) -> Option<&PackageReviewMachineParameterContract> {
        match self {
            Self::Type { .. } | Self::Const { .. } => None,
            Self::Machine { contract } => Some(contract),
        }
    }
}

/// Self-contained callable shape for executable code supplied outside Omega.
/// The static telescope currently represents ordinary type parameters with
/// their exact property bounds, const parameters with their exact carrier, and
/// static-machine parameters with their complete recursive contract. The
/// adjacent conformance telescope retains exact structural generic bounds.
/// Projection rejects other static kinds until their exact structure has a
/// stable carrier here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) static_parameters: Vec<PackageReviewExternalStaticParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewExternalCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
}

impl PackageReviewExternalCallableSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn static_parameters(&self) -> &[PackageReviewExternalStaticParameter] {
        &self.static_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parameters(&self) -> &[PackageReviewExternalCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalExecutableSupply {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) signature: PackageReviewExternalCallableSignature,
    pub(crate) requirement: PackageReviewExternalRequirement,
    pub(crate) binding: PackageReviewExternalBinding,
}

impl PackageReviewExternalExecutableSupply {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn signature(&self) -> &PackageReviewExternalCallableSignature {
        &self.signature
    }

    pub const fn requirement(&self) -> &PackageReviewExternalRequirement {
        &self.requirement
    }

    pub const fn conformance(&self) -> Option<&PackageReviewCallableConformance> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(conformance) => Some(conformance),
            PackageReviewExternalRequirement::Operator(_)
            | PackageReviewExternalRequirement::TopLevelRequirement { .. } => None,
        }
    }

    pub const fn operator(&self) -> Option<&PackageReviewOperatorCoordinate> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::TopLevelRequirement { .. } => None,
            PackageReviewExternalRequirement::Operator(operator) => Some(operator),
        }
    }

    pub const fn top_level_requirement(&self) -> Option<&PackageReviewNominalIdentity> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::Operator(_) => None,
            PackageReviewExternalRequirement::TopLevelRequirement { identity, .. } => {
                Some(identity)
            }
        }
    }

    pub const fn top_level_requirement_signature(
        &self,
    ) -> Option<&PackageReviewExternalCallableSignature> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::Operator(_) => None,
            PackageReviewExternalRequirement::TopLevelRequirement { signature, .. } => {
                Some(signature)
            }
        }
    }

    pub const fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
}
