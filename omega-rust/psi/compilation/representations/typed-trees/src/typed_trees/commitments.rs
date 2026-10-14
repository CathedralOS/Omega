//! Boundary calling plan, machine specialization, template and closed
//! conformance commitments.

use crate::{data, types};
use arena::HandleSpan;

/// `[u8; 32]` digest newtype — one nominal commitment identity per
/// contract domain, sharing the `from_digest`/`as_bytes`/`is_zero`
/// surface.
macro_rules! digest_commitment {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        pub struct $name([u8; 32]);

        impl $name {
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
    };
}

/// One compile-time machine specialization. Const arguments are canonical
/// proof-static identities and static machine arguments are symbols; neither
/// becomes a runtime value. Preparation records the template and arguments;
/// checking retains the specialization's portable identity in checked facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineSpecialization {
    pub template: symbols::SymbolHandle,
    /// Original binder arena range, retained after the live template is closed.
    /// Rewritten calls use its exact machine-binder identities and declaration
    /// order to rejoin the selected entries in this specialization.
    pub template_parameters: HandleSpan<data::TypeParameter>,
    /// Exact typed substitution, independent of diagnostic and hash strings.
    pub binding_application: crate::static_applications::StaticMachineApplicationHandle,
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
    /// Package-qualified normalized overload identity of the authored generic
    /// template before substitution. ProviderPlan checked-adapter joins and
    /// Terminal Psi closed reach applications name the template by it.
    pub normalized_template_identity: String,
    /// The one accepted-fact commitment this instance relies upon. Every
    /// instance points at the same template commitment; none spends a new
    /// grant. `None` for checked templates.
    pub accepted_template_commitment: Option<String>,
}

digest_commitment! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    MachineSpecializationCommitment
}

digest_commitment! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    MachineTemplateCommitment
}

#[derive(Debug, Clone, Default)]
pub struct ClosedConformanceApplication {
    pub declaration: symbols::SymbolHandle,
    /// Derived exact source application, prepared before checking. The zero
    /// scope is unavailable, not permission to recover operands from report
    /// strings. Local arena handles do not contribute portable authority.
    pub trait_scope: crate::trait_definition::RetainedTraitScope,
    /// Occurrence-owned map telescope used to admit every captured callable.
    pub callable_application: crate::static_applications::StaticMachineApplicationHandle,
    /// Exact ordered source-resolved arguments retained for structural policy
    /// projection. This added tree does not participate in the historical
    /// application report or commitment; equal legacy coordinates cannot
    /// substitute for joining its exact resolved symbols.
    pub arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub lifetime_arguments: Vec<String>,
    pub type_arguments: Vec<String>,
    pub const_arguments: Vec<ClosedConformanceConstArgument>,
    pub machine_arguments: Vec<symbols::SymbolHandle>,
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

impl PartialEq for ClosedConformanceApplication {
    fn eq(&self, other: &Self) -> bool {
        // Preparation attaches an arena-local scope to the same application.
        // A freshly reconstructed selection must still match its prepared row.
        self.declaration == other.declaration
            && self.arguments == other.arguments
            && self.lifetime_arguments == other.lifetime_arguments
            && self.type_arguments == other.type_arguments
            && self.const_arguments == other.const_arguments
            && self.machine_arguments == other.machine_arguments
            && self.trait_definition == other.trait_definition
            && self.trait_lifetime_arguments == other.trait_lifetime_arguments
            && self.trait_arguments == other.trait_arguments
            && self.rows == other.rows
            && self.report_fingerprint == other.report_fingerprint
            && self.commitment == other.commitment
    }
}

impl Eq for ClosedConformanceApplication {}

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

digest_commitment! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    ClosedConformanceApplicationCommitment
}

/// The closed conformance applications retained on `instance`'s
/// specializations carrying the dispatch's fingerprint and commitment.
impl MachineSpecialization {
    /// The specialization carries no residual type, machine, or conformance
    /// arguments; only the const identities name the tuple it instantiates.
    pub fn is_const_only(&self) -> bool {
        self.type_argument_identities.is_empty()
            && self.machine_arguments.is_empty()
            && self.conformance_arguments.is_empty()
            && self.conformance_applications.is_empty()
    }
}

pub fn closed_conformance_applications_of<'a>(
    specializations: &'a [MachineSpecialization],
    instance: symbols::SymbolHandle,
    application_report_fingerprint: u64,
    application_commitment: ClosedConformanceApplicationCommitment,
) -> Vec<&'a ClosedConformanceApplication> {
    specializations
        .iter()
        .filter(|specialization| specialization.instance == instance)
        .flat_map(|specialization| &specialization.conformance_applications)
        .filter(|application| {
            application.report_fingerprint == application_report_fingerprint
                && application.commitment == application_commitment
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConformanceRowIdentity {
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
}

/// Exact storage coordinates for complete derived application views. Invocation
/// and direct occurrence sources remain distinct even when their tuples agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedConformanceApplicationSite {
    Invocation {
        specialization: usize,
        application: usize,
    },
    Occurrence(crate::static_applications::StaticMachineApplicationHandle),
}

/// An owned map row with its capture telescope applied. The method's own
/// invocation telescope remains independently authored on the instance.
/// This relation is substitution custody, never conformance admission.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedMachineApplication {
    pub template: symbols::SymbolHandle,
    pub instance: symbols::SymbolHandle,
    pub binding_application: crate::static_applications::StaticMachineApplicationHandle,
    /// Exact source invocation that supplied the selected evidence operand.
    pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
    pub conformance_application: ClosedConformanceApplication,
    pub normalized_template_identity: String,
    pub accepted_template_commitment: Option<String>,
    pub states: HandleSpan<CapturedMachineState>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapturedMachineState {
    pub template: symbols::SymbolHandle,
    pub instance: symbols::SymbolHandle,
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
    /// Direct selections retain their exact authored occurrence. Evidence-bound
    /// dispatch keeps its independent invocation-specialization source.
    pub selection_application: crate::static_applications::StaticMachineApplicationHandle,
    /// Non-authoritative compatibility coordinate for the selected
    /// application. `application_commitment` is the authoritative join.
    pub application_report_fingerprint: u64,
    pub application_commitment: ClosedConformanceApplicationCommitment,
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    /// Applied public contract, distinct from the authored selection-row key.
    /// Zero until mutable preparation joins its complete committed application.
    pub contract_signature: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
}

impl StaticRequirementDispatch {
    pub fn contract_symbol(&self) -> symbols::SymbolHandle {
        if self.contract_signature.is_valid() {
            self.contract_signature
        } else {
            self.requirement
        }
    }
}

/// Whether `specialization` was made from the template that a ProviderPlan's
/// split checked-adapter identity names. The recorded identity is
/// `<declaration>|<callable identity>`; the declaration half derives from the
/// template symbol alone and contains no `|`, so the join compares the
/// template's package and the recorded callable half.
pub fn machine_specialization_matches_template_identity(
    typed: &crate::TypedTrees,
    specialization: &MachineSpecialization,
    machine_identity: &str,
    machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> bool {
    typed
        .symbols
        .symbol_package_identity(specialization.template)
        == machine_package_identity
        && specialization
            .normalized_template_identity
            .split_once('|')
            .is_some_and(|(_, callable)| callable == machine_identity)
}
