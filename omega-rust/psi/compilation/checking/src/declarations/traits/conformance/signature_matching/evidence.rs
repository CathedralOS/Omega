//! Universal evidence refinement uses the same declared-promise judgment as
//! ordinary forwarding. A later concrete implementation cannot establish a
//! stronger provider precondition on the public member's behalf.

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
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_parameters = program.state_signature_type_parameters(requirement);
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
    let lifetimes = requirement
        .lifetime_parameters
        .iter()
        .zip(&provider.lifetime_parameters)
        .map(|(required, actual)| (required.as_str().to_owned(), actual.as_str().to_owned()))
        .collect::<Vec<_>>();
    application.with_identity_scopes(program.typed_program(), |outer| {
        let mut scopes = outer.to_vec();
        scopes.push(AppliedTypeIdentityScope {
            declaration: requirement.symbol,
            arguments: &operands,
            lifetimes: &lifetimes,
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
            let expected = crate::conformance_applications::applied_bound_argument_identities(
                program, demanded, provider.symbol, provider.symbol, &[],
            );
            if let Err(reason) = crate::conformance_applications::validate_forwarded_argument(
                program, demanded, &selected, |promised| {
                    expected.as_ref().zip(
                        crate::conformance_applications::applied_bound_argument_identities(
                            program, promised, requirement.symbol, provider.symbol, &scopes,
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
