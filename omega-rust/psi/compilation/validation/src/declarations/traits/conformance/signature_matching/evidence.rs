//! Universal evidence refinement uses the same declared-promise judgment as
//! ordinary forwarding. A later concrete implementation cannot establish a
//! stronger provider precondition on the public member's behalf.

use crate::conformance_applications::{
    applied_bound_argument_identities_in_region_scope, validate_forwarded_argument,
};
use diagnostics::Diagnostic;
use typed_trees::{
    data::{TypeParameter, TypeParameterKind},
    expression::StaticMachineArgument,
    machine::Machine,
    signature::StateSignature,
    trait_definition::AppliedTraitScope,
    type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument},
};

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    application: &AppliedTraitScope<'_>,
    requirement: &StateSignature,
    provider: &Machine,
    actual_parameters: &[TypeParameter],
    frame: &crate::borrow_contracts::CallableLifetimeFrame,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_parameters = program.state_signature_type_parameters(requirement);
    if !required_parameters
        .iter()
        .any(|parameter| parameter.kind == TypeParameterKind::Implementation)
    {
        return;
    }
    let actuals = actual_parameters
        .iter()
        .map(|parameter| StaticMachineArgument {
            symbol: parameter.symbol,
            path: vec![parameter.name.clone()].into_boxed_slice(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let operands = required_parameters
        .iter()
        .zip(&actuals)
        .map(|(parameter, actual)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(actual),
            )
        })
        .collect::<Vec<_>>();
    let required_regions = frame
        .required
        .iter()
        .map(|binding| (binding.parameter, binding.argument))
        .collect::<Vec<_>>();
    let actual_regions = frame
        .actual
        .iter()
        .map(|binding| (binding.parameter, binding.argument))
        .collect::<Vec<_>>();
    application.with_identity_scopes(program.typed_program(), |outer| {
        let mut scopes = outer.to_vec();
        scopes.push(AppliedTypeIdentityScope {
            declaration: requirement.symbol,
            arguments: &operands,
            lifetimes: &[],
        });
        for (required, actual) in required_parameters.iter().zip(actual_parameters) {
            if required.kind != TypeParameterKind::Implementation { continue; }
            let Some(demanded) = program.callable_conformance_bound(provider.symbol, actual.symbol) else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider `{}` evidence parameter `{}` has no exact declared bound",
                    provider.name, actual.name,
                )));
                continue;
            };
            let selected = StaticMachineArgument {
                symbol: required.symbol,
                path: vec![required.name.clone()].into_boxed_slice(),
                ..Default::default()
            };
            let expected = applied_bound_argument_identities_in_region_scope(
                program, demanded, provider.symbol, provider.symbol, &[], &actual_regions,
            );
            if let Err(reason) = validate_forwarded_argument(
                program, demanded, &selected, |promised| {
                    expected.as_ref().zip(
                        applied_bound_argument_identities_in_region_scope(
                            program, promised, requirement.symbol, provider.symbol, &scopes, &required_regions,
                        ).as_ref(),
                    ).is_some_and(|(demanded, promised)| demanded == promised)
                },
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "provider `{}` does not refine requirement `{}` evidence parameter `{}`: {reason}",
                    provider.name, requirement.name, required.name,
                )));
            }
        }
    });
}
