//! Family interfaces carry caller demands, not a selected body's telescope.
//! Ordinary arguments use the shared declared-callable projection, including
//! relevance and alpha-normalized types. Generic contract payloads reuse the
//! universal contract encoder. Nested callable carriers must agree, but their
//! demands remain separate: checking admits a portable application's selection
//! against every retained member through ordinary callable refinement.
//! Unjoined demands remain an implementation dependency,
//! never permission to silently adopt a weaker first-member interface.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::Machine;
use typed_trees::type_identity::TypeIdentityRequest;
use validation::contract_structure::{declared_callable, generic_binders};

mod callable_carriers;

pub(super) fn validate(
    program: &validation::CheckingContext<'_>,
    interface: &Machine,
    member: &Machine,
) -> Result<(), Diagnostic> {
    let required = program.machine_type_parameters(interface);
    let actual = program.machine_type_parameters(member);
    if required.len() != actual.len()
        || interface.lifetime_parameters.len() != member.lifetime_parameters.len()
    {
        return Err(incompatible(member, "generic/lifetime parameter count"));
    }
    let mut required_binders = generic_binders(required);
    let mut actual_binders = generic_binders(actual);
    if interface.conformance_bounds.len() != member.conformance_bounds.len() {
        return Err(unjoined(member, "conformance-bound conjunction"));
    }
    for (position, (required, actual)) in interface
        .conformance_bounds
        .iter()
        .zip(&member.conformance_bounds)
        .enumerate()
    {
        required_binders.push((required.binder, format!("$C{position}")));
        actual_binders.push((actual.binder, format!("$C{position}")));
    }
    for (required, actual) in interface
        .conformance_bounds
        .iter()
        .zip(&member.conformance_bounds)
    {
        if required.trait_symbol != actual.trait_symbol
            || required.arguments.len() != actual.arguments.len()
            || required
                .arguments
                .iter()
                .zip(&actual.arguments)
                .any(|(required, actual)| {
                    identity(program, *required, &required_binders)
                        != identity(program, *actual, &actual_binders)
                })
        {
            return Err(unjoined(
                member,
                "conformance-bound conjunction/alpha correspondence",
            ));
        }
    }
    let Some(required_state) = program.machine_states(interface).first() else {
        return Err(incompatible(member, "interface entry signature"));
    };
    let Some(actual_state) = program.machine_states(member).first() else {
        return Err(incompatible(member, "member entry signature"));
    };
    let required_callable = declared_callable(
        program,
        program.state_parameters(required_state),
        required_state.return_type,
        &[],
        &required_binders,
        &[],
        false,
    )?;
    let actual_callable = declared_callable(
        program,
        program.state_parameters(actual_state),
        actual_state.return_type,
        &[],
        &actual_binders,
        &[],
        false,
    )?;
    if required_callable != actual_callable {
        return Err(incompatible(
            member,
            "ordinary parameter relevance/calling shape or result",
        ));
    }
    for (required, actual) in required.iter().zip(actual) {
        if std::mem::discriminant(&required.kind) != std::mem::discriminant(&actual.kind) {
            return Err(incompatible(member, "generic parameter category"));
        }
        if typed_trees::data::type_parameter_demands_stronger_properties(required, actual) {
            return Err(unjoined(member, "generic property-demand conjunction"));
        }
        // Only generic contract encoding consumes conservation facts. The
        // shared immutable context derives them at most once for this append,
        // and ordinary target-family signatures do not request them at all.
        let content_conservation = validation::content_conservation_plans(program);
        if let (
            TypeParameterKind::Machine { contract: required },
            TypeParameterKind::Machine { contract: actual },
        ) = (&required.kind, &actual.kind)
        {
            if !callable_carriers::compatible_contracts(
                program,
                required,
                actual,
                &required_binders,
                &actual_binders,
                &content_conservation,
            ) {
                return Err(incompatible(member, "nested callable carrier telescope"));
            }
            // Calling shape is shared, not nominal authority. The contract
            // view substitutes authored trait arguments before comparison;
            // selection checking separately requires each member's exact
            // satisfaction application, even for phantom arguments or when
            // another member asks only for a structural contract.
            continue;
        }
        // Non-callable binders still require exact payload identity until
        // their distinct demand conjunction has an ordinary admission route.
        let required_identity = validation::contract_identities::generic_parameter_contract_bytes(
            program,
            required,
            &required_binders,
            &content_conservation,
        )?;
        let actual_identity = validation::contract_identities::generic_parameter_contract_bytes(
            program,
            actual,
            &actual_binders,
            &content_conservation,
        )?;
        if required_identity != actual_identity {
            return Err(unjoined(member, "generic contract refinement/conjunction"));
        }
    }
    Ok(())
}

fn identity(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> typed_trees::type_identity::NormalizedTypeIdentity {
    program.type_identity(TypeIdentityRequest {
        binders,
        ..TypeIdentityRequest::ordinary(reference)
    })
}

fn incompatible(member: &Machine, detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "machine `{}` has an incompatible signature for `{}` -- \
         every target's body of one machine shares its signature; incompatible {detail}",
        member.name,
        member
            .target
            .as_ref()
            .map_or("<missing target>", |target| target.as_str()),
    ))
}

fn unjoined(member: &Machine, detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "unimplemented: target-family member `{}` requires {detail}",
        member.name
    ))
}
