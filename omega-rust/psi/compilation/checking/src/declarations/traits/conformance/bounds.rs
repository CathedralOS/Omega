//! Generic and trait conformance bounds: the requirement a bound states,
//! whether an argument meets it, and the validation of every bound a
//! conformance declares.

use crate::declarations::traits::conformance::signature_matching::{
    TraitTypeBinding, TraitTypeBindingTarget, type_references_match_with_trait_bindings,
};
use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
use crate::declarations::traits::trait_definition_by_symbol;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct GenericBoundRequirement<'program> {
    pub(crate) signature: &'program StateSignature,
    pub(crate) trait_definition: &'program TraitDefinition,
    pub(crate) bound: &'program typed_trees::machine::GenericConformanceBound,
}

/// Whether an actual argument type matches a requirement parameter of the
/// binder's trait. The binder's trait arguments substitute the trait's own
/// parameters; there is no subject binding.
pub(crate) fn named_conformance_argument_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    let mut bindings = program
        .trait_type_parameters(requirement.trait_definition)
        .iter()
        .zip(requirement.bound.arguments.iter())
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
        })
        .collect::<Vec<_>>();
    type_references_match_with_trait_bindings(
        program,
        actual,
        required,
        program.trait_type_parameters(requirement.trait_definition),
        &mut bindings,
    )
}

pub(crate) fn validate_generic_conformance_bounds(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "machine",
        machine.name.as_str(),
        &machine.conformance_bounds,
        diagnostics,
    );
}

pub(crate) fn validate_trait_conformance_bounds(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "trait",
        trait_definition.name.as_str(),
        &trait_definition.conformance_bounds,
        diagnostics,
    );
}

fn validate_conformance_bounds(
    program: &TypedTrees,
    owner_kind: &str,
    owner_name: &str,
    bounds: &[typed_trees::machine::GenericConformanceBound],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in bounds {
        if !bound.binder.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` evidence binder `{}` was not declared",
                bound.binder_name
            )));
            continue;
        }

        let Some(trait_definition) = trait_definition_by_symbol(program, bound.trait_symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` evidence binder `{}` names unknown trait `{}`",
                bound.binder_name, bound.trait_name
            )));
            continue;
        };
        let expected = program.trait_type_parameters(trait_definition).len();
        if bound.arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` evidence binder `{}` for trait `{}` expects {expected} generic argument(s), got {}",
                bound.binder_name,
                bound.trait_name,
                bound.arguments.len(),
            )));
            continue;
        }

        validate_trait_application_obligations(
            program,
            trait_definition,
            &bound.arguments,
            &format!(
                "{owner_kind} `{owner_name}` evidence binder `{} satisfies {}`",
                bound.binder_name, bound.trait_name
            ),
            diagnostics,
        );
    }
}
