//! Token selection from a generic machine's authored conformance requirements.
use crate::declarations::traits::conformance::signature_matching::TraitTypeBinding;
use crate::declarations::traits::conformance::signature_matching::TraitTypeBindingTarget;
use crate::declarations::traits::conformance::signature_matching::type_references_match_with_trait_bindings;
use crate::declarations::traits::trait_definition_by_symbol;
use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::types::TypeReferenceHandle;

/// Select a requirement identity, not a concrete conformance realization.
/// Every known operand must satisfy the same declared subject/argument bindings.
pub fn generic_bound_operator_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    spelling: OperatorSpelling,
    operands: &[Option<TypeReferenceHandle>],
) -> Result<Option<&'program StateSignature>, String> {
    if operands.iter().any(Option::is_none) {
        return Ok(None);
    }
    let mut selected = None;
    for bound in &machine.conformance_bounds {
        let Some(trait_definition) = trait_definition_by_symbol(program, bound.carrier) else {
            continue;
        };
        for requirement in program.trait_machine_signatures(trait_definition) {
            if requirement.spelling != Some(spelling) {
                continue;
            }
            let parameters = program.state_signature_parameters(requirement);
            if parameters.len() != operands.len() {
                continue;
            }
            let mut bindings = vec![TraitTypeBinding {
                parameter_symbol: SymbolHandle::invalid(),
                parameter_name: "Self".to_owned(),
                target: TraitTypeBindingTarget::Parameter(bound.subject),
            }];
            bindings.extend(
                program
                    .trait_type_parameters(trait_definition)
                    .iter()
                    .zip(&bound.arguments)
                    .map(|(parameter, argument)| TraitTypeBinding {
                        parameter_symbol: parameter.symbol,
                        parameter_name: parameter.name.to_string(),
                        target: TraitTypeBindingTarget::Type(*argument),
                    }),
            );
            if !operands.iter().zip(parameters).all(|(actual, parameter)| {
                actual.is_some_and(|actual| {
                    type_references_match_with_trait_bindings(
                        program,
                        actual,
                        parameter.type_reference,
                        program.trait_type_parameters(trait_definition),
                        &mut bindings,
                    )
                })
            }) {
                continue;
            }
            if selected.is_some() {
                return Err(format!(
                    "machine `{}` has multiple declared conformance bounds selecting operator {:?}",
                    machine.name, spelling,
                ));
            }
            selected = Some(requirement);
        }
    }
    Ok(selected)
}
