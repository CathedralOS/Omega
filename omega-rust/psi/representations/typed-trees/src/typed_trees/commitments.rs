//! Boundary calling plan, machine specialization, template and closed
//! conformance commitments.

use crate::{data, types};
use arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanIdentity {
    pub boundary_trait: symbols::SymbolHandle,
    /// Concrete boundary trait argument tuple. These handles are internal
    /// lookup identity only and never enter the published contract hash.
    /// Empty for a non-generic boundary declaration.
    pub boundary_arguments: Vec<crate::types::TypeReferenceHandle>,
    pub requirement_machine: symbols::SymbolHandle,
    /// Compact compatibility/report coordinate. Exact plan authority uses the
    /// adjacent strong commitment.
    pub report_fingerprint: u64,
    pub commitment: BoundaryCallingPlanCommitment,
}

/// Exact selected-plan custody that licenses one routed `Binding<R>`
/// carrier to enter the existing provider-attachment specialization path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusedServiceErasureAuthorization {
    pub requirement: symbols::SymbolHandle,
    pub provider_plan_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallingPlanCommitment([u8; 32]);

impl BoundaryCallingPlanCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

/// One compile-time machine specialization. Const arguments are canonical
/// proof-static identities and static machine arguments are symbols; neither
/// becomes a runtime value. `report_fingerprint` is a compact diagnostic/cache
/// coordinate only; authority-bearing consumers must replay `commitment` from
/// the retained exact specialization inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineSpecialization {
    pub template: symbols::SymbolHandle,
    /// Original binder arena range, retained after the live template is closed.
    /// Rewritten calls use its exact machine-binder identities and declaration
    /// order to rejoin the selected entries in this specialization.
    pub template_parameters: HandleSpan<data::TypeParameter>,
    /// Concrete machine instance produced for this substitution. The first
    /// specialization may reuse `template`; later specializations clone it
    /// under a fresh symbol. Consumers must key executable/elaborated work by
    /// this symbol rather than guessing from specialization order.
    pub instance: symbols::SymbolHandle,
    /// Readable source-oriented spellings retained for diagnostics only.
    pub type_arguments: Vec<String>,
    pub const_arguments: Vec<String>,
    /// Ordered canonical static identities used to construct the
    /// specialization fingerprint. These are retained independently from the
    /// readable spellings above and are never reconstructed from them.
    pub type_argument_identities: Vec<String>,
    pub const_argument_identities: Vec<String>,
    pub machine_arguments: Vec<symbols::SymbolHandle>,
    /// Exact package-scoped conformances selected for explicit proof-static
    /// evidence binders. Separate from callable machine arguments.
    pub conformance_arguments: Vec<symbols::SymbolHandle>,
    /// Exact package-scoped conformances selected by unique generic-bound
    /// inference rather than named proof-static evidence at the call site.
    pub inferred_conformance_arguments: Vec<symbols::SymbolHandle>,
    /// Closed, argument-sensitive identities for the selected conformance
    /// family members. Two applications of the same declaration with
    /// different telescopes are distinct even though `conformance_arguments`
    /// contains the same package-scoped declaration symbol.
    pub conformance_applications: Vec<ClosedConformanceApplication>,
    /// Exact operator requirements realized by this concrete checked-body
    /// specialization. Each row is reconstructed from the substituted entry
    /// signature and retains the declaration-ordered closed application; it
    /// is distinct from provider-authored conformance arguments and from
    /// checked use-site demand.
    pub operator_realizations: Vec<crate::operator::ClosedOperatorRealizationApplication>,
    /// The normalized authored template identity captured before in-place
    /// substitution consumes its generic parameter declarations.
    pub template_contract_report_fingerprint: u64,
    /// Domain-separated commitment to the same exact canonical universal-
    /// template bytes represented by the adjacent compact report coordinate.
    pub template_contract_commitment: MachineTemplateCommitment,
    /// Exact canonical universal-template encoding captured before in-place
    /// substitution consumes its generic parameter declarations. This is
    /// retained so later checked-to-terminal review can independently replay
    /// the specialization commitment rather than trusting the compact
    /// template coordinate above.
    pub canonical_template_contract_bytes: Vec<u8>,
    /// Package-qualified normalized overload identity of the authored generic
    /// template before substitution. The concrete `instance` identity is
    /// rederived from the retained typed program during commitment replay.
    pub normalized_template_identity: String,
    /// The one accepted-fact commitment this instance relies upon. Every
    /// instance points at the same template commitment; none spends a new
    /// grant. `None` for checked templates.
    pub accepted_template_commitment: Option<String>,
    /// Checked contract identities of the selected static machine arguments.
    /// Compact report coordinates populated after contract-plan construction.
    /// Authority uses the adjacent exact commitments.
    pub machine_argument_contract_report_fingerprints: Vec<u64>,
    /// Exact checked public-contract commitments of the selected static
    /// machine owners, in specialization argument order.
    /// Stored as the exact 32-byte digest because the typed representation
    /// cannot depend cyclically on the checked representation that owns the
    /// `MachineContractCommitment` newtype.
    pub machine_argument_contract_commitments: Vec<[u8; 32]>,
    /// Semantic identities of the selected closed conformance maps. These
    /// compact values are report coordinates only; exact authority is retained
    /// by each `conformance_applications[*].commitment`.
    pub conformance_argument_report_fingerprints: Vec<u64>,
    /// Historical compact cache/report coordinate for the complete
    /// specialization. It is never sufficient authority without `commitment`.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the canonical template, exact
    /// argument identities, selected machine contracts, closed conformances,
    /// and accepted-template grant relied upon by this instance.
    pub commitment: MachineSpecializationCommitment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineSpecializationCommitment([u8; 32]);

impl MachineSpecializationCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineTemplateCommitment([u8; 32]);

impl MachineTemplateCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConformanceApplication {
    pub declaration: symbols::SymbolHandle,
    /// Exact ordered source-resolved arguments retained for structural policy
    /// projection. This added tree does not participate in the historical
    /// application report or commitment; equal legacy coordinates cannot
    /// substitute for joining its exact resolved symbols.
    pub arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub lifetime_arguments: Vec<String>,
    pub type_arguments: Vec<String>,
    pub const_arguments: Vec<ClosedConformanceConstArgument>,
    pub machine_arguments: Vec<symbols::SymbolHandle>,
    pub subject_identity: Option<String>,
    pub trait_definition: symbols::SymbolHandle,
    /// Concrete target-trait lifetime application after substituting the
    /// declaration's checked lifetime ordinals through this application.
    pub trait_lifetime_arguments: Vec<String>,
    pub trait_arguments: Vec<String>,
    pub rows: Vec<ClosedConformanceRowIdentity>,
    /// Historical compact coordinate retained for diagnostics and local
    /// indexing. Authority-bearing joins must also replay `commitment`.
    pub report_fingerprint: u64,
    /// Domain-separated commitment to the historical closed-application
    /// encoding. The separately retained resolved argument tree is not hashed.
    pub commitment: ClosedConformanceApplicationCommitment,
}

/// One exact const argument retained by a checked conformance application.
/// Evaluated values exclude diagnostic display; caller binders retain both
/// sides of the checked carrier match until final substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedConformanceConstArgument {
    Evaluated {
        parameter_carrier: types::TypeReferenceHandle,
        declared_carrier: types::TypeReferenceHandle,
        value: language_semantics::const_value::CanonicalConstIdentity,
    },
    CallerBinder {
        parameter_carrier: types::TypeReferenceHandle,
        binder: symbols::SymbolHandle,
        binder_carrier: types::TypeReferenceHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedConformanceApplicationCommitment([u8; 32]);

impl ClosedConformanceApplicationCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConformanceRowIdentity {
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
}

/// One requirement call resolved through an exact closed static conformance.
///
/// Monomorphization rewrites the executable target to `realization_state`, but
/// the public erased contract remains owned by `declaring_trait` and
/// `requirement`.  Keeping both identities on the call prevents later proof
/// lowering from mistaking satisfier-local evidence for public witness
/// identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticRequirementDispatch {
    /// Non-authoritative compatibility coordinate for the selected
    /// application. `application_commitment` is the authoritative join.
    pub application_report_fingerprint: u64,
    pub application_commitment: ClosedConformanceApplicationCommitment,
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
}
