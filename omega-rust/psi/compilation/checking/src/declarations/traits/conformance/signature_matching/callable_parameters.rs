//! Partition the provider telescope by the exact contract being matched.
//!
//! `provider<T, U> satisfies Trait<Container<T>>::member` fixes T in that
//! trait application. U remains a fresh invocation binder and must match the
//! requirement's universal callable family. Only this exact edge captures
//! parameters: another edge mentioning U cannot erase that obligation here.
//! Inline named implementations additionally capture their own telescope.

use typed_trees::data::TypeParameter;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::trait_definition::AppliedTraitScope;

/// The authored family and its applied logical contract share one invocation
/// telescope. A concrete executable header consumes that telescope; it cannot
/// be compared to an unconsumed provider family as if they were one declaration.
/// This view retains source correspondence, not conformance or proof authority.
pub(crate) struct RequirementCallableCorrespondence<'program> {
    pub(crate) machine: &'program Machine,
    pub(crate) state: &'program State,
    pub(crate) requirement: &'program StateSignature,
    pub(crate) applied_requirement: &'program StateSignature,
    pub(crate) invocation_parameters: Vec<TypeParameter>,
}

pub(crate) fn requirement_callable_correspondence<'program>(
    program: &'program dyn crate::CheckingInput,
    realization: &'program Machine,
    state: &'program State,
    application: &AppliedTraitScope<'_>,
    requirement: &'program StateSignature,
) -> Result<RequirementCallableCorrespondence<'program>, diagnostics::Diagnostic> {
    let failure = || {
        diagnostics::Diagnostic::error(format!(
            "machine `{}` requirement correspondence lost its exact original callable family",
            realization.name,
        ))
    };
    let original = program
        .trait_machine_signature_by_symbol(application.declaration, requirement.symbol)
        .filter(|original| std::ptr::eq(*original, requirement))
        .ok_or_else(failure)?;
    let machine = super::authored_conformance_machine(program, realization)?;
    let state = if machine.symbol == realization.symbol {
        state
    } else {
        typed_trees::state::state_position(program.machine_states(realization), state.symbol)
            .and_then(|ordinal| program.machine_states(machine).get(ordinal))
            .ok_or_else(failure)?
    };
    let applied_requirement = if original.type_parameters.is_empty()
        && application.declaration.type_parameters.is_empty()
    {
        original
    } else {
        program
            .typed_program()
            .applied_callable_signature(application, original.symbol)
            .ok_or_else(failure)?
    };
    Ok(RequirementCallableCorrespondence {
        machine,
        state,
        requirement: original,
        applied_requirement,
        invocation_parameters: requirement_invocation_parameters(
            program,
            realization,
            machine,
            application,
        ),
    })
}

pub fn requirement_invocation_parameters(
    program: &dyn crate::CheckingInput,
    realization: &Machine,
    family: &Machine,
    application: &AppliedTraitScope<'_>,
) -> Vec<TypeParameter> {
    let mut captured = application.application_parameter_symbols(program);
    captured.extend(
        program
            .conformances()
            .iter()
            .filter(|conformance| {
                conformance.rows.iter().any(|row| {
                    (row.realization_machine == realization.symbol
                        || row.realization_machine == family.symbol)
                        && row.source.owns_realization()
                })
            })
            .flat_map(|conformance| program.conformance_type_parameters(conformance))
            .map(|parameter| parameter.symbol),
    );
    program
        .machine_type_parameters(family)
        .iter()
        .filter(|parameter| !captured.contains(&parameter.symbol))
        .cloned()
        .collect()
}
