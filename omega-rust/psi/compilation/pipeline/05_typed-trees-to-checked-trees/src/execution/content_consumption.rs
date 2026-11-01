//! Retain completed content transactions as ordinary owned value operands.
//!
//! The permission producer has already proved and committed each transaction.
//! Its fresh payload consumes those parents even when no structural field
//! contains them. Publish that roster on the value before Unit disposition
//! planning, so a call result and a construction use the same custody rule.
//! Terminal lowering follows the exact retained transaction, never a scan that
//! could backdate authority onto a captured copy or a later alias.

use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

pub(crate) fn attach(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if facts.qualifications.content.body_transformations.is_empty() {
        return Ok(());
    }
    let nodes = facts
        .values
        .structural_values
        .nodes
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in nodes {
        let (machine, state, statement_index, expression) = {
            let node = facts.values.structural_values.nodes.get(handle);
            (
                node.machine,
                node.state,
                node.statement_index,
                node.expression,
            )
        };
        let content = &facts.qualifications.content;
        let mut matching = content
            .body_transformations
            .iter()
            .filter(|(_, transaction)| {
                let completed = facts.flow.control.operands.get(transaction.completed_value);
                transaction.machine_symbol == machine
                    && transaction.state_symbol == state
                    && completed.statement_index == statement_index as usize
                    && completed.expression == expression
            });
        let Some((transaction_handle, transaction)) = matching.next() else {
            continue;
        };
        // A copied observation can share an authored expression with its
        // original producer, but never repeats that producer's transaction.
        if facts
            .values
            .structural_values
            .roots
            .iter()
            .any(|(_, root)| root.root == handle && root.captured_copy.is_valid())
        {
            continue;
        }
        let error = |reason: &str| {
            vec![diagnostics::Diagnostic::error(format!(
                "cannot retain content consumption in `{}`, state `{}` at statement {}: {reason}",
                crate::labels::symbol_name(program, machine),
                crate::labels::symbol_name(program, state),
                statement_index,
            ))]
        };
        if matching.next().is_some() {
            return Err(error("completion has ambiguous checked transactions"));
        }
        let mut parents = Vec::new();
        for derived in content
            .body_derivations
            .span_or_empty(transaction.derivations)
        {
            let equation = content.body_conservations.get(derived.conservation);
            for available in content
                .body_available_inputs
                .span_or_empty(equation.available_inputs)
            {
                let place = validation::CanonicalPlace {
                    root: available.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(available.segments)
                        .to_vec(),
                };
                let source = match place.root {
                    facts::PlaceRoot::Symbol(symbol) => {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                    }
                    facts::PlaceRoot::Expression(expression) => {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression }
                    }
                    _ => return Err(error("parent has no executable owned source")),
                };
                let (reference, path) = super::terminal_unit::calls::projected_argument_path(
                    program,
                    state,
                    statement_index as usize,
                    &place,
                    Some(machine),
                    None,
                )
                .ok_or_else(|| error("parent has no exact typed finite path"))?;
                let argument = CheckedUnitStructuralArgumentPlan {
                    source,
                    path,
                    projection_evaluations: Default::default(),
                    type_identity: validation::stored_structural_type_identity(
                        program,
                        reference,
                        &[],
                        &[],
                    ),
                    access: CheckedStructuralAccess::Owned,
                };
                crate::collections::push_unique(&mut parents, argument);
            }
        }
        if parents.is_empty() {
            return Err(error("transaction has no owned parents"));
        }
        let inputs = facts
            .values
            .structural_values
            .content_inputs
            .insert_many(parents);
        let node = facts.values.structural_values.nodes.get_mut(handle);
        node.content_transformation = transaction_handle;
        node.content_inputs = inputs;
    }
    Ok(())
}
