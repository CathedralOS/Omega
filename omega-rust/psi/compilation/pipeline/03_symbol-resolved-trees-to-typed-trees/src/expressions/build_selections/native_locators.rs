//! Close native-locator operands before ordinary const and literal formation.
//! The exact core telescope owns coordinate inference; target policy stays in Omega.

mod constructors;

use crate::tables::expression_table;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{BuildOperation, BuildSelectionRequest};
use std::collections::HashSet;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument},
};

pub(crate) fn normalize(program: &mut TypedTrees, frontier: usize) -> Result<(), Diagnostic> {
    let pending = expression_table(program)
        .expression_entries()
        .filter_map(|(handle, node)| {
            if handle.arena_index() as usize <= frontier {
                return None;
            }
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            let Some(BuildSelectionRequest::Declaration(request)) =
                expression_table(program).build_selection_request(call.build_selection)
            else {
                return None;
            };
            (request.operation == BuildOperation::NativeBinding
                && call.target.as_str() == "bind_native")
                .then_some(handle)
        })
        .collect::<HashSet<_, symbols::BuildSymbolHasher>>();
    if pending.is_empty() {
        return Ok(());
    }
    let vocabulary =
        validation::native_locators::vocabulary(program, |_| true).map_err(|errors| {
            errors.into_iter().next().unwrap_or_else(|| {
                Diagnostic::error("native locator vocabulary admission returned no diagnostic")
            })
        })?;
    // Resolve each occurrence in its own state; nesting supplies no implicit
    // data or inherited argument context. Borrow one immutable owner per group.
    let mut groups = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut selected = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let mut expressions = Vec::new();
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                selected.extend(
                    expressions
                        .into_iter()
                        .filter(|expression| pending.contains(expression)),
                );
            }
            selected.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
            selected.dedup();
            if !selected.is_empty() {
                groups.push((machine.clone(), state.clone(), selected));
            }
        }
    }
    let mut completed = HashSet::<ExpressionHandle, symbols::BuildSymbolHasher>::default();
    for (machine, state, sites) in groups {
        for site in sites {
            let call = expression_table(program)
                .call_node(site)
                .cloned()
                .ok_or_else(|| Diagnostic::error("native binding lost its ordinary call"))?;
            let [_, _, locator] = program.expression_table.expression_handles(call.arguments)
            else {
                return Err(Diagnostic::error(
                    "native binding requires its Build, target and locator operands",
                ));
            };
            let locator = *locator;
            let carrier = constructors::infer(program, &machine, &state, vocabulary, locator)?;
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(site) else {
                unreachable!()
            };
            call.machine_arguments = vec![StaticMachineArgument {
                type_reference: carrier,
                ..Default::default()
            }]
            .into_boxed_slice();
            completed.insert(site);
        }
    }
    if completed.len() != pending.len() {
        return Err(Diagnostic::error(
            "native binding occurrence has no exact executable owner",
        ));
    }
    Ok(())
}
