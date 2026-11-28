//! Close the callable signatures needed by the declarations and refined calls in this typing.

use crate::source_state::{
    TypedTrees,
    references::{ProgramApplicationOccurrences, collect_program_application_occurrences},
    trait_definition::{AppliedTraitScope, TraitApplicationArguments, TraitApplicationEnvironment},
};

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), &'static str> {
    let occurrences = collect_program_application_occurrences(program);
    let (invocation_scopes, added_types) = refinement_invocation_scopes(program, &occurrences)?;
    // Reuse the occurrence walk when collecting the scopes changed
    // nothing it could have seen; otherwise walk the changed program.
    let mut roots = if added_types {
        program.applied_trait_roots()
    } else {
        program.applied_trait_roots_from(occurrences)
    };
    roots.extend(
        invocation_scopes
            .iter()
            .filter_map(|scope| scope.scope(program)),
    );
    let pending = program.applied_callable_signature_requests(roots);
    pending.apply(program)
}

/// Return the refined scopes and whether forming them added operand types.
fn refinement_invocation_scopes(
    program: &mut TypedTrees,
    occurrences: &ProgramApplicationOccurrences,
) -> Result<(Vec<TraitApplicationEnvironment>, bool), &'static str> {
    use crate::source_state::static_applications::StaticMachineBindingValue;
    let mut scopes = Vec::new();
    let mut changed = false;
    for &invocation in &occurrences.invocation_applications {
        let record = program
            .expression_table
            .static_machine_application(invocation)
            .clone();
        let Some(machine) = program.machine_by_symbol(record.binding_declaration) else {
            continue;
        };
        // Any refinement clause changes the callable the bound supplies:
        // contracts, termination, suspension, blocking or reach. A
        // selected requirement call reads its contract from this
        // invocation scope, so each refined bound needs one prepared.
        let bounds = machine
            .conformance_bounds
            .iter()
            .filter(|bound| {
                program
                    .trait_definition_by_symbol(bound.trait_symbol)
                    .is_some_and(|declaration| !declaration.refinement_clauses.is_empty())
            })
            .cloned()
            .collect::<Vec<_>>();
        if bounds.is_empty() {
            continue;
        }
        let mut bindings = program
            .expression_table
            .static_machine_application_bindings(invocation)
            .to_vec();
        let mut arguments = Vec::with_capacity(bindings.len());
        let mut retained_types = false;
        for binding in &mut bindings {
            let actual = match &mut binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => *reference,
                StaticMachineBindingValue::Machine(argument) => {
                    if !argument.type_reference.is_valid() {
                        argument.type_reference = program.static_operand_type_reference_in_scope(
                            argument,
                            record.lexical_owner,
                        )?;
                        retained_types = true;
                    }
                    argument.type_reference
                }
                StaticMachineBindingValue::Conformance(argument) => {
                    if !argument.type_reference.is_valid() {
                        return Err(
                            "refinement invocation lost its retained implementation operand",
                        );
                    }
                    argument.type_reference
                }
                StaticMachineBindingValue::Proof(_) => {
                    return Err("refinement invocation needs its retained logical evidence");
                }
                StaticMachineBindingValue::RuntimeValue { .. } => {
                    return Err(
                        "refinement postcondition invocation has an unresolved value argument",
                    );
                }
            };
            arguments.push((binding.parameter, actual));
        }
        if retained_types {
            changed = true;
            program
                .expression_table
                .set_static_machine_application_bindings(invocation, &bindings);
        }
        let lifetimes = program
            .static_application_lifetimes_for_declaration(invocation, record.binding_declaration)?;
        for bound in bounds {
            let declaration = program
                .trait_definition_by_symbol(bound.trait_symbol)
                .ok_or("refinement invocation lost its bound declaration")?;
            let scope = AppliedTraitScope::new(
                program,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: record.binding_declaration,
                    enclosing_data_applications: &[],
                    arguments: &bound.arguments,
                    lifetimes: &bound.lifetime_arguments,
                },
            )
            .and_then(|scope| {
                scope.under_invocation(
                    program,
                    record.binding_declaration,
                    record.lexical_owner,
                    &arguments,
                    &lifetimes,
                )
            })
            .ok_or("refinement invocation lost its complete bound scope")?;
            scopes.push(scope.environment());
        }
    }
    Ok((scopes, changed))
}
