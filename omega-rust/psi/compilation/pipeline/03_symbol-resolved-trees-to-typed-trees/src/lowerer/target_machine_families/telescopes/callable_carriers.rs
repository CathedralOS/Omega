//! Positional carrier correspondence, independent of callable supply demands.
//!
//! Recursing through the complete nested telescope prevents a changed callback
//! argument, result, relevance or generic category from becoming a different
//! target implementation of one signature. Preconditions, guarantees and
//! operational ceilings are not calling shape; selection checking discharges
//! those separately for every member. Exact nominal requirements stay nominal.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{MachineParameterContractView, TypeParameterKind};
use typed_trees::signature::StateSignature;
use validation::contract_structure::declared_callable;

pub(super) fn compatible(
    program: &TypedTrees,
    required: &StateSignature,
    actual: &StateSignature,
    required_outer: &[(SymbolHandle, String)],
    actual_outer: &[(SymbolHandle, String)],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> bool {
    let required_parameters = program.state_signature_type_parameters(required);
    let actual_parameters = program.state_signature_type_parameters(actual);
    if required_parameters.len() != actual_parameters.len()
        || required.lifetime_parameters.len() != actual.lifetime_parameters.len()
        || !native_callback_carriers_match(program, required, actual)
    {
        return false;
    }
    let mut required_binders = required_outer.to_vec();
    let mut actual_binders = actual_outer.to_vec();
    let scope = required_binders.len();
    for (position, (required, actual)) in required_parameters
        .iter()
        .zip(actual_parameters)
        .enumerate()
    {
        let identity = format!("$N{scope}G{position}");
        required_binders.push((required.symbol, identity.clone()));
        actual_binders.push((actual.symbol, identity));
    }
    for (required, actual) in required_parameters.iter().zip(actual_parameters) {
        if std::mem::discriminant(&required.kind) != std::mem::discriminant(&actual.kind) {
            return false;
        }
        match (&required.kind, &actual.kind) {
            (
                TypeParameterKind::Machine { contract: required },
                TypeParameterKind::Machine { contract: actual },
            ) => match (
                program.machine_parameter_contract_view(required),
                program.machine_parameter_contract_view(actual),
            ) {
                (
                    Some(MachineParameterContractView::Structural(required)),
                    Some(MachineParameterContractView::Structural(actual)),
                ) if compatible(
                    program,
                    required,
                    actual,
                    &required_binders,
                    &actual_binders,
                    content_conservation,
                ) =>
                {
                    continue;
                }
                (
                    Some(MachineParameterContractView::Nominal {
                        trait_definition: required_trait,
                        requirement: required_machine,
                    }),
                    Some(MachineParameterContractView::Nominal {
                        trait_definition: actual_trait,
                        requirement: actual_machine,
                    }),
                ) if required_trait.symbol == actual_trait.symbol
                    && required_machine.symbol == actual_machine.symbol => {}
                _ => return false,
            },
            _ => {}
        }
        // Named authority includes its trait application, not just the
        // requirement declaration. Until that conjunction has an applied
        // carrier model, nominal and non-callable slots retain exact identity.
        let required = validation::contract_identities::generic_parameter_contract_bytes(
            program,
            required,
            &required_binders,
            content_conservation,
        );
        let actual = validation::contract_identities::generic_parameter_contract_bytes(
            program,
            actual,
            &actual_binders,
            content_conservation,
        );
        if !matches!((required, actual), (Ok(required), Ok(actual)) if required == actual) {
            return false;
        }
    }
    declared_callable(
        program,
        program.state_signature_parameters(required),
        required.return_type,
        &[],
        &required_binders,
        &[],
        false,
    ) == declared_callable(
        program,
        program.state_signature_parameters(actual),
        actual.return_type,
        &[],
        &actual_binders,
        &[],
        false,
    )
}

/// Native callbacks occupy ABI positions and select declaration-local machine
/// binders. Their printed parameter and binder names are not carrier identity.
/// This legacy signature record stores binder identifiers, so resolve each
/// against its own telescope and compare positions, rejecting missing or
/// ambiguous associations rather than discarding the binder relationship.
fn native_callback_carriers_match(
    program: &TypedTrees,
    required: &StateSignature,
    actual: &StateSignature,
) -> bool {
    if required.native_callback_parameters.len() != actual.native_callback_parameters.len() {
        return false;
    }
    let binder_position = |signature: &StateSignature, binder: &typed_trees::name::Identifier| {
        let mut positions = program
            .state_signature_type_parameters(signature)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                parameter.name == *binder
                    && matches!(parameter.kind, TypeParameterKind::Machine { .. })
            })
            .map(|(position, _)| position);
        let position = positions.next()?;
        positions.next().is_none().then_some(position)
    };
    required
        .native_callback_parameters
        .iter()
        .zip(&actual.native_callback_parameters)
        .all(|(required_callback, actual_callback)| {
            required_callback.native_ordinal == actual_callback.native_ordinal
                && matches!(
                    (
                        binder_position(required, &required_callback.binder),
                        binder_position(actual, &actual_callback.binder),
                    ),
                    (Some(required), Some(actual)) if required == actual
                )
        })
}
