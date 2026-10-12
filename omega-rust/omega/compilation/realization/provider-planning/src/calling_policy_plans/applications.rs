//! Collect exact closed calling applications from source and produced headers.
//!
//! Authored conformance applications carry type substitutions. A produced
//! public header already owns its substituted formals and keeps the full
//! original binding application separately. Its retained reachable interface
//! roster supplies an inherited policy owner's tuple while the selected Child
//! remains the presentation owner. Both feed the same selector and evaluator;
//! neither source spelling nor a same-shaped ABI selects a row.

use super::boundary_signatures::{BoundarySignatureInstance, collect_boundary_signatures};
use super::policy_implementations::{boundary_policy_instances, policy_static_argument};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::calling_vocabulary::boundary_has_explicit_calling_policy;
use checked_trees::typed_trees::data::TypeParameterKind;
use checked_trees::typed_trees::static_applications::{
    StaticApplicationIdentity, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue, static_application_identity, static_binding_identity,
};
use checked_trees::typed_trees::trait_definition::TraitDefinition;
use checked_trees::typed_trees::types::TypeReferenceHandle;

/// One original declared requirement and its exact compiler-local tuple.
/// Presentation handles select consumers; they do not define application equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryCallingApplicationIdentity {
    pub(crate) policy_owner: symbols::SymbolHandle,
    pub(crate) requirement: symbols::SymbolHandle,
    pub(crate) bindings: StaticApplicationIdentity,
}

pub(super) struct BoundaryPolicyApplication<'program> {
    pub(super) identity: BoundaryCallingApplicationIdentity,
    pub(super) selected_interface: symbols::SymbolHandle,
    pub(super) boundary_arguments: Vec<TypeReferenceHandle>,
    pub(super) argument_owner: symbols::SymbolHandle,
    pub(super) binding_application: StaticMachineApplicationHandle,
    pub(super) policy_argument: TypeReferenceHandle,
    pub(super) policy_bindings: Vec<StaticMachineBinding>,
    pub(super) signature: BoundarySignatureInstance<'program>,
}

pub(super) fn collect<'program>(
    typed: &'program TypedTrees,
    boundary: &'program TraitDefinition,
    policy_argument: TypeReferenceHandle,
) -> Result<Vec<BoundaryPolicyApplication<'program>>, String> {
    let mut applications = Vec::new();
    for source in boundary_policy_instances(typed, boundary, policy_argument)? {
        let boundary_arguments = source.arguments;
        let mut policy_bindings = Vec::new();
        for (parameter, actual) in typed
            .trait_type_parameters(boundary)
            .iter()
            .zip(&boundary_arguments)
        {
            let value = match &parameter.kind {
                TypeParameterKind::Type => StaticMachineBindingValue::Type(*actual),
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    StaticMachineBindingValue::Const(*actual)
                }
                TypeParameterKind::Implementation => StaticMachineBindingValue::Conformance(
                    policy_static_argument(typed, *actual, &[])?,
                ),
                TypeParameterKind::Machine { .. } => {
                    StaticMachineBindingValue::Machine(policy_static_argument(typed, *actual, &[])?)
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err(
                        "calling application has an unsupported proposition binder".to_owned()
                    );
                }
            };
            policy_bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value,
            });
        }
        let mut signatures = Vec::new();
        collect_boundary_signatures(
            typed,
            boundary,
            &boundary_arguments,
            &mut Vec::new(),
            &mut signatures,
        )?;
        let bindings = static_binding_identity(
            typed,
            boundary.symbol,
            &policy_bindings,
            checked_trees::typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(
                source.lexical_owner,
            ),
            source.lifetime_arguments,
        )
        .map_err(|diagnostic| {
            format!("calling application identity is unavailable: {diagnostic:?}")
        })?;
        for signature in signatures {
            // A nominal callback binder is a declared ABI surface, materialized
            // independently of its later selected callback. Other open static
            // formals need their genuine produced application before layout.
            if typed
                .state_signature_type_parameters(signature.public_signature.declaration)
                .iter()
                .any(|parameter| {
                    !matches!(
                        &parameter.kind,
                        TypeParameterKind::Machine {
                            contract: checked_trees::typed_trees::data::MachineParameterContract::Nominal { .. }
                        }
                    )
                })
            {
                continue;
            }
            applications.push(BoundaryPolicyApplication {
                identity: BoundaryCallingApplicationIdentity {
                    policy_owner: boundary.symbol,
                    requirement: signature.public_signature.signature.symbol,
                    bindings: bindings.clone(),
                },
                selected_interface: boundary.symbol,
                boundary_arguments: boundary_arguments.clone(),
                argument_owner: source.lexical_owner,
                binding_application: StaticMachineApplicationHandle::invalid(),
                policy_argument: source.policy,
                policy_bindings: policy_bindings.clone(),
                signature,
            });
        }
    }
    for (_, row) in typed.instantiated_requirement_signatures.iter() {
        let signature = typed.trait_machine_signatures.get(row.signature);
        if header_calling_policy_owner(typed, row)? != Some(boundary.symbol) {
            continue;
        }
        let policy_scope = typed
            .requirement_interface_applications
            .span_or_empty(row.interface_applications)
            .iter()
            .find(|scope| scope.interface == boundary.symbol)
            .ok_or_else(|| {
                "produced Calling header lost its original policy interface".to_owned()
            })?;
        if !policy_scope.binding_application.is_valid() {
            return Err(
                "inherited Calling policy lacks original interface evidence correspondence"
                    .to_owned(),
            );
        }
        let declaring = typed
            .trait_definition_by_symbol(row.declaring_trait)
            .ok_or_else(|| "produced calling header lost its declaring trait".to_owned())?;
        let authored = typed
            .trait_machine_signature_by_symbol(declaring, row.requirement_template)
            .ok_or_else(|| "produced calling header lost its original requirement".to_owned())?;
        if !row.binding_application.is_valid() {
            return Err("produced calling header has no retained binding application".to_owned());
        }
        let public_signature = typed
            .selected_public_trait_requirement_signature(signature.symbol, row.binding_application)
            .filter(|selected| {
                selected.owner.symbol == row.declaring_trait
                    && std::ptr::eq(selected.declaration, authored)
                    && std::ptr::eq(selected.signature, signature)
            })
            .ok_or_else(|| {
                "produced Calling header lost its original application correspondence".to_owned()
            })?;
        // No substitution remains to materialize this header. The exact full
        // tuple, including non-type binders and captured regions, stays in its
        // original program-owned application; an empty substitution list is
        // not an assertion that the source declaration had no static operands.
        applications.push(BoundaryPolicyApplication {
            identity: BoundaryCallingApplicationIdentity {
                policy_owner: boundary.symbol,
                requirement: row.requirement_template,
                bindings: static_application_identity(typed, row.binding_application).map_err(
                    |diagnostic| {
                        format!(
                            "produced calling application identity is unavailable: {diagnostic:?}"
                        )
                    },
                )?,
            },
            selected_interface: row.selected_interface,
            boundary_arguments: Vec::new(),
            argument_owner: typed
                .expression_table
                .static_machine_application(row.binding_application)
                .lexical_owner,
            binding_application: row.binding_application,
            policy_argument,
            policy_bindings: typed
                .expression_table
                .static_machine_application_bindings(policy_scope.binding_application)
                .to_vec(),
            signature: BoundarySignatureInstance {
                declaring_trait: row.declaring_trait,
                public_signature,
                requirement_identity: typed
                    .trait_requirement_callable_identity_text(declaring, authored),
                bindings: Vec::new(),
            },
        });
    }
    Ok(applications)
}

/// Consume the original header's applied-interface roster, rather than discover
/// policy through descendants or reconstruct ancestor tuples during realization.
/// Direct selected-owner Calling keeps its existing authored route. Otherwise
/// only a unique inherited owner can provide a presentation; ambiguity remains
/// unavailable until an explicit language selection rule exists.
pub(crate) fn calling_policy_owner(
    typed: &TypedTrees,
    selected_interface: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
) -> Result<Option<symbols::SymbolHandle>, String> {
    let selected = typed
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| "Calling presentation lost its original selected interface".to_owned())?;
    if boundary_has_explicit_calling_policy(typed, selected) {
        return Ok(Some(selected_interface));
    }
    let mut headers = typed
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, header)| {
            typed.trait_machine_signatures.get(header.signature).symbol == requirement_machine
        });
    let Some((_, header)) = headers.next() else {
        // An ordinary authored direct requirement has no produced header. Its
        // direct declaration already supplies the existing no-policy meaning.
        // A selected inherited application always needs its produced owner.
        if typed
            .trait_machine_signature(requirement_machine)
            .is_some_and(|(declaring, _)| declaring.symbol == selected_interface)
        {
            return Ok(None);
        }
        return Err(
            "inherited Calling presentation is missing its original public header".to_owned(),
        );
    };
    if headers.next().is_some() || header.selected_interface != selected_interface {
        return Err("Calling presentation does not name one exact selected header".to_owned());
    }
    header_calling_policy_owner(typed, header)
}

fn header_calling_policy_owner(
    typed: &TypedTrees,
    header: &checked_trees::typed_trees::static_applications::InstantiatedRequirementSignature,
) -> Result<Option<symbols::SymbolHandle>, String> {
    let selected_interface = header.selected_interface;
    let selected = typed
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| "Calling header lost its original selected interface".to_owned())?;
    if boundary_has_explicit_calling_policy(typed, selected) {
        return Ok(Some(selected_interface));
    }
    let scopes = typed
        .requirement_interface_applications
        .span_or_empty(header.interface_applications);
    if scopes.is_empty()
        || !scopes
            .iter()
            .any(|scope| scope.interface == selected_interface)
    {
        return Err("Calling header lost its original reachable interface roster".to_owned());
    }
    let mut owner = None;
    for scope in scopes {
        let definition = typed
            .trait_definition_by_symbol(scope.interface)
            .ok_or_else(|| "Calling scope lost its original interface declaration".to_owned())?;
        if !boundary_has_explicit_calling_policy(typed, definition) {
            continue;
        }
        if owner.is_some_and(|known| known != scope.interface) {
            return Err(
                "inherited Calling presentation has no unique authored policy owner".to_owned(),
            );
        }
        owner = Some(scope.interface);
    }
    Ok(owner)
}
