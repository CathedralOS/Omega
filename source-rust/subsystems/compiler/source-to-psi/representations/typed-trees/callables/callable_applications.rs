#[path = "specialization_regions.rs"]
mod specialization_regions;
//! Typed callable applications retain the selected declaration, its scope and its substituted signature.

#[path = "arguments/canonical_values.rs"]
mod canonical_arguments;
#[path = "arguments/closed_operands.rs"]
pub(crate) mod closed_operands;
#[path = "arguments/selected_operands.rs"]
pub(crate) mod selected_operands;

#[path = "requirements/instances.rs"]
pub(crate) mod requirement_instances;

#[path = "scopes/conformances.rs"]
pub(crate) mod conformance_scopes;
#[path = "scopes/domains.rs"]
pub(crate) mod domain_scopes;
#[path = "scopes/namespaces.rs"]
pub(crate) mod namespaces;
#[path = "scopes/traits.rs"]
pub(crate) mod trait_scopes;

#[path = "selection/call_targets.rs"]
pub(crate) mod call_targets;
#[path = "selection/declaration_invocations.rs"]
mod declaration_invocations;
#[path = "selection/machines.rs"]
pub(crate) mod machine_selections;

#[path = "signatures/contract_views.rs"]
mod contract_views;
#[path = "signatures/substitution.rs"]
pub(crate) mod signatures;

#[path = "static_callback_contracts.rs"]
mod static_callback_contracts;

#[path = "storage/application_rows.rs"]
pub(crate) mod table;

#[path = "substitution/callable.rs"]
mod callable_substitution;
#[path = "substitution/payloads.rs"]
pub(crate) mod payloads;
#[path = "substitution/types.rs"]
mod type_substitution;

#[cfg(test)]
#[path = "tests/canonical_record_arguments.rs"]
mod canonical_record_argument_tests;
#[cfg(test)]
#[path = "tests/invocation_owners.rs"]
mod invocation_owner_walk_tests;

pub use static_callback_contracts::AppliedStaticCallbackRequirement;

pub use super::machine_instances::{
    MachineInstantiation, NamedCaptureError, NamedConformanceCaptureIndex,
};

pub(crate) use table::AppliedCallableTable;

pub use requirement_instances::{ClosedRequirementApplication, RequirementLocalArgument};

use crate::source_state::static_applications::StaticMachineLifetimeBinding;
use crate::source_state::type_identity::lifetime_parameter_identity;
use crate::source_state::{TypedTrees, data, signature, types};
use symbols::SymbolHandle;

/// A retained callable telescope and the exact outer scope that produced it.
/// This is source typing context, never a conformance or checked proof.
#[derive(Clone, Copy)]
pub struct CallableContractScope<'a> {
    pub signature: &'a signature::StateSignature,
    pub trait_scope: Option<crate::source_state::trait_definition::RetainedTraitScope>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub(crate) struct AppliedCallableSignature {
    owner: AppliedCallableOwner,
    signature: signature::StateSignature,
}

impl AppliedCallableSignature {
    pub(crate) fn requirement_instance_symbol(&self) -> Option<SymbolHandle> {
        matches!(self.owner, AppliedCallableOwner::RequirementInstance { .. })
            .then_some(self.signature.symbol)
    }

    pub(crate) fn requirement_template_symbol(&self) -> Option<SymbolHandle> {
        match self.owner {
            AppliedCallableOwner::RequirementInstance { template, .. } => Some(template),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub(crate) enum AppliedCallableOwner {
    #[default]
    Absent,
    Realization {
        declaration: SymbolHandle,
        application: crate::source_state::type_identity::NormalizedTypeIdentity,
        lexical_owner: SymbolHandle,
    },
    Trait {
        declaration: SymbolHandle,
        application: crate::source_state::type_identity::NormalizedTypeIdentity,
        scope: crate::source_state::trait_definition::RetainedTraitScope,
    },
    RequirementInvocation {
        declaration: SymbolHandle,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        lexical_owner: SymbolHandle,
        source: signature::StateSignature,
        bindings: arena::HandleSpan<crate::source_state::static_applications::StaticMachineBinding>,
        lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
        arguments: arena::HandleSpan<crate::source_state::type_identity::NormalizedTypeIdentity>,
    },
    RequirementInstance {
        declaration: SymbolHandle,
        template: SymbolHandle,
        trait_application_identity: crate::source_state::type_identity::NormalizedTypeIdentity,
        portable_trait_application_identity:
            crate::source_state::type_identity::NormalizedTypeIdentity,
        portable_local_arguments:
            arena::HandleSpan<crate::source_state::type_identity::NormalizedTypeIdentity>,
        trait_scope: crate::source_state::trait_definition::RetainedTraitScope,
        trait_arguments: arena::HandleSpan<types::TypeReferenceHandle>,
        local_arguments: arena::HandleSpan<RequirementLocalArgument>,
        invocation_contract_parameters: arena::HandleSpan<data::TypeParameter>,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
    },
    MachineSelection {
        invocation: crate::source_state::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        demand: SymbolHandle,
        selected: SymbolHandle,
        provider: crate::source_state::static_applications::StaticMachineApplicationHandle,
        required: Option<signature::StateSignature>,
        source_required: signature::StateSignature,
        caller_operands: arena::HandleSpan<CallableSubstitution>,
        caller_bindings:
            arena::HandleSpan<crate::source_state::static_applications::StaticMachineBinding>,
        caller_lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
        caller_lifetime_declaration: SymbolHandle,
        caller_lifetime_owner: SymbolHandle,
        provider_lifetime_declaration: SymbolHandle,
        provider_lifetime_owner: SymbolHandle,
        provider_lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
    },
    StaticCallback {
        parameter: SymbolHandle,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        lexical_owner: SymbolHandle,
        source: signature::StateSignature,
        bindings: arena::HandleSpan<crate::source_state::static_applications::StaticMachineBinding>,
        lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct CallableSubstitution {
    pub parameter: SymbolHandle,
    pub actual: types::TypeReferenceHandle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppliedMachineCallableId(arena::Handle<AppliedCallableSignature>);

impl AppliedMachineCallableId {
    pub fn is_valid(self) -> bool {
        self.0.is_valid()
    }
}

pub struct AppliedMachineCallable<'a> {
    pub identity: AppliedMachineCallableId,
    pub invocation: crate::source_state::static_applications::StaticMachineApplicationHandle,
    pub ordinal: usize,
    pub demand: SymbolHandle,
    pub lexical_owner: SymbolHandle,
    pub provider: crate::source_state::static_applications::StaticMachineApplicationHandle,
    pub selected_entry: SymbolHandle,
    pub required: &'a signature::StateSignature,
    pub selected: &'a signature::StateSignature,
    pub source_required: &'a signature::StateSignature,
    pub caller_operands: arena::RootRows<'a, CallableSubstitution>,
    pub caller_lifetimes: arena::RootRows<'a, StaticMachineLifetimeBinding>,
}

/// All expressions newly allocated for a copied machine and the statements
/// it owns, including statements inside block values.
pub struct CopiedCallableBody {
    pub first_expression: usize,
    pub expression_count: usize,
    pub statements: Vec<crate::source_state::statement::StatementHandle>,
}

#[derive(Default)]
/// Substitutes the bindings of one declaration while copying typed records.
/// Reuse the same application for types, facts and static arguments that share a scope.
pub struct CallableApplication {
    specialized_regions: std::collections::VecDeque<specialization_regions::RegionUse>,
    owned_signature: SymbolHandle,
    runtime_values: Vec<SymbolHandle>,
    region_scope: SymbolHandle,
    trait_region_application: Option<(
        crate::source_state::trait_definition::TraitApplicationEnvironment,
        usize,
    )>,
    lifetime_declaration: SymbolHandle,
    lifetimes: Vec<CallableLifetimeBinding>,
    elided_lifetimes: Vec<crate::source_state::name::Identifier>,
    invocation_elision: Option<(
        SymbolHandle,
        usize,
        crate::source_state::type_identity::identity_context::SignatureElision,
    )>,
    types: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
    static_arguments: Vec<(
        SymbolHandle,
        crate::source_state::expression::StaticMachineArgument,
    )>,
    static_argument_owner: SymbolHandle,
    selected_providers: Vec<(
        SymbolHandle,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    )>,
    symbols: Vec<(SymbolHandle, SymbolHandle)>,
    declaration_symbols: crate::source_state::PreparedSymbolRebinding,
    active_requirements: Vec<(SymbolHandle, SymbolHandle)>,
    source_binders: Vec<(crate::source_state::name::Identifier, SymbolHandle)>,
    active_runtime_contracts: Vec<(
        crate::source_state::callable_values::RuntimeCallableContractHandle,
        crate::source_state::callable_values::RuntimeCallableContractHandle,
    )>,
}

struct CallableLifetimeBinding {
    parameter: crate::source_state::name::Identifier,
    argument: crate::source_state::name::Identifier,
    region: crate::source_state::type_identity::LifetimeParameterIdentity,
    origin: CallableLifetimeOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallableLifetimeOrigin {
    Specialized,
    Lexical,
    Rigid,
    Captured,
}

struct ScopedCallableLifetime {
    name: crate::source_state::name::Identifier,
    region: crate::source_state::type_identity::LifetimeParameterIdentity,
    origin: CallableLifetimeOrigin,
}
