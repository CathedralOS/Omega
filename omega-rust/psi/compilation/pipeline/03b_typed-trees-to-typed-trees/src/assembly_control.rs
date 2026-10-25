//! Admission for the currently executable lexical assembly control domain.
//! Normalize explicit scratch versions and conditional edges first. Remaining
//! lexical paths preserve their original frame and must have a finite order;
//! no cycle is disguised as a finite instruction sequence or an assumed fact.

mod normal_exit_contracts;
mod normalization;
mod register_versions;
pub(crate) use normalization::normalize;

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) fn admit(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if !program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Assembly(_)
                    )
                })
            {
                continue;
            }
            let order = program
                .statement_table
                .finite_execution_order(state.statement_nodes)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "invalid assembly control in `{}` state `{}`: {reason}",
                        machine.name, state.name,
                    ))]
                })?;
            if order.is_none() {
                return Err(vec![Diagnostic::error(format!(
                    "cyclic assembly label control in `{}` state `{}` requires checked loop invariants; this control domain is not yet implemented",
                    machine.name, state.name,
                ))]);
            }
        }
    }
    Ok(())
}

/// Compiler-owned catalog operations cannot acquire an authored operator's
/// meaning merely because their implementation reuses ordinary numeric nodes.
/// This gate preserves the current builtin-only cohort; it neither forces a
/// selected builtin nor changes executable operator dispatch.
pub(crate) fn validate_primitive_meanings(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let typed_trees::statement::StatementNode::Assembly(
                    typed_trees::statement::AssemblyControlPoint::Entry(handle),
                ) = statement
                else {
                    continue;
                };
                let graph = program.statement_table.assembly(*handle);
                for expression in program.statement_table.expression_handles(graph.primitives) {
                    if !validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        *expression,
                    ) {
                        return Err(vec![Diagnostic::error(format!(
                            "catalog assembly arithmetic/zero-flag operation in `{}` state `{}` requires builtin operator meaning; an authored operator cannot redefine an instruction",
                            machine.name, state.name
                        ))]);
                    }
                }
            }
        }
    }
    Ok(())
}
