//! Generic and trait conformance bounds: the requirement a bound states,
//! whether an argument meets it, and the validation of every bound a
//! conformance declares.

use crate::declarations::traits::conformance::signature_matching::{
    trait_type_bindings, type_references_match_with_trait_bindings,
};
use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
use crate::declarations::traits::trait_definition_by_symbol;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
pub struct GenericBoundRequirement<'program> {
    pub signature: &'program StateSignature,
    pub trait_definition: &'program TraitDefinition,
    pub bound: &'program typed_trees::machine::GenericConformanceBound,
}

impl GenericBoundRequirement<'_> {
    /// The public signature and its applied trait telescope are inseparable.
    /// Inherited requirements use their declaring trait's arguments, not the
    /// outer binder's parameter order. These borrowed type identities close
    /// structural queries without manufacturing instantiated type nodes.
    pub fn type_bindings(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> Option<Vec<(symbols::SymbolHandle, TypeReferenceHandle)>> {
        let carrier = trait_definition_by_symbol(program, self.bound.trait_symbol)?;
        let carrier = match &carrier.refines {
            Some(base) => trait_definition_by_symbol(program, base.symbol)?,
            None => carrier,
        };
        let arguments = crate::declarations::traits::arguments_for_declaring_trait(
            program,
            carrier,
            &self.bound.arguments,
            self.trait_definition.symbol,
            &mut Vec::new(),
        )?;
        let parameters = program.trait_type_parameters(self.trait_definition);
        if parameters.len() != arguments.len() {
            return None;
        }
        Some(
            parameters
                .iter()
                .zip(arguments)
                .filter_map(|(parameter, argument)| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                        .then_some((parameter.symbol, argument))
                })
                .collect(),
        )
    }
}

/// Whether an actual argument type matches a requirement parameter of the
/// binder's trait. The binder's trait arguments substitute the trait's own
/// parameters; there is no subject binding.
pub(crate) fn named_conformance_argument_matches(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    let mut bindings = trait_type_bindings(
        program
            .trait_type_parameters(requirement.trait_definition)
            .iter(),
        requirement.bound.arguments.iter(),
    );
    type_references_match_with_trait_bindings(
        program,
        actual,
        required,
        program.trait_type_parameters(requirement.trait_definition),
        &mut bindings,
    )
}

pub(crate) fn validate_generic_conformance_bounds(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
