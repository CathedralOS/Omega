//! Top-level `boundary requirement` ProviderPlan execution bridge.
//!
//! Semantic checking and retained facts continue to name the public
//! requirement. Boundary-dispatch settlement associates each receiver-free
//! public requirement with its selected checked adapter
//! (`CheckedBoundaryAdapterDispatch`); this bridge is the execution half of
//! that association, the requirement analogue of the named boundary-operator
//! adapter rewrite: every direct call `Owner::name(...)` whose row is
//! settled — value position or statement position (`Owner::name(...);`,
//! including the `_ = call();` explicit discard) — redirects to the adapter's
//! entry state, journaled as a source edit, so the interpreter and Terminal
//! execute the ordinary checked body while the retained row, flow facts and
//! journal keep the requirement. A requirement satisfied by an external
//! `via` leaf settles no dispatch row and is deliberately not rewritten:
//! the call stays on the requirement, whose retained boundary seam is the
//! identity the native foreign-call join executes against.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequirementCallRewrite {
    /// The journaled call site: an expression-table call or a
    /// statement-table call (`Owner::name(...);`, including `_ = call();`).
    site: RequirementCallSite,
    /// The caller state and statement the flow occurrence and its checked
    /// scalar-argument facts are keyed on.
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    call_ordinal: u32,
    requirement_state: symbols::SymbolHandle,
    machine: String,
    entry_symbol: symbols::SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequirementCallSite {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

/// The settled realization rows keyed on top-level requirements: the row's
/// `requirement` is the entry state of a `TopLevelRequirement` machine.
fn top_level_rows(
    checked: &CheckedTrees,
) -> Vec<(
    &checked_trees::CheckedBoundaryAdapterDispatch,
    &typed_trees::machine::Machine,
)> {
    let typed = &checked.typed;
    checked
        .facts
        .boundary_adapter_dispatch
        .iter()
        .filter_map(|row| {
            let requirement = typed.machines().iter().find(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && typed
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == row.requirement)
            })?;
            Some((row, requirement))
        })
        .collect()
}

pub(super) fn plan_selected_requirement_rewrites(
    checked: &CheckedTrees,
) -> Result<Vec<RequirementCallRewrite>, Vec<Diagnostic>> {
    let rows = top_level_rows(checked);
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let typed = &checked.typed;
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();
    for (row, requirement) in &rows {
        if row.forward_receiver || !row.family_tuple.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "selected top-level boundary requirement `{}` settled a receiver-forwarding or family row; only an exact receiver-free row is executable",
                requirement.name,
            )));
            continue;
        }
        let realization = typed.machines().iter().find(|machine| {
            machine.supply_mode.is_checked_body()
                && typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == row.realization_state)
        });
        let Some(realization) = realization else {
            diagnostics.push(Diagnostic::error(format!(
                "selected top-level boundary requirement `{}` lost its exact checked realization entry",
                requirement.name,
            )));
            continue;
        };
        // Statement-position direct calls (`Owner::name(...);`, including
        // `_ = call();`) follow the same settled route. The site carries no
        // authored expression, so its flow occurrence is the caller state's
        // call at this statement index with an invalid authored expression;
        // the statement call takes its statement's first call ordinal ahead
        // of any nested argument calls, which keeps the match unique.
        for machine in typed.machines() {
            for state in typed.machine_states(machine) {
                for (statement_index, (handle, statement)) in typed
                    .statement_table
                    .iter_statements(state.statement_nodes)
                    .enumerate()
                {
                    let typed_trees::statement::StatementNode::Call(call) = statement else {
                        continue;
                    };
                    if call.target_symbol != row.requirement || call.receiver_symbol != row.receiver
                    {
                        continue;
                    }
                    let Some(flow_state) = checked
                        .facts
                        .flow
                        .control
                        .states
                        .iter()
                        .map(|(_, flow_state)| flow_state)
                        .find(|flow_state| flow_state.state_symbol == state.symbol)
                    else {
                        diagnostics.push(Diagnostic::error(format!(
                            "statement-position direct call to public boundary requirement `{}` lost its caller flow state",
                            requirement.name,
                        )));
                        continue;
                    };
                    let occurrences = checked
                        .facts
                        .flow
                        .control
                        .calls
                        .span_or_empty(flow_state.calls)
                        .iter()
                        .filter(|occurrence| {
                            occurrence.statement_index == statement_index
                                && !occurrence.authored_expression.is_valid()
                                && occurrence.target_symbol == row.requirement
                                && occurrence.receiver_symbol == row.receiver
                        })
                        .collect::<Vec<_>>();
                    let [occurrence] = occurrences.as_slice() else {
                        diagnostics.push(Diagnostic::error(format!(
                            "statement-position direct call to public boundary requirement `{}` retains {} flow occurrences; expected one",
                            requirement.name,
                            occurrences.len(),
                        )));
                        continue;
                    };
                    let (Ok(statement_ordinal), Ok(call_ordinal)) = (
                        u32::try_from(occurrence.statement_index),
                        u32::try_from(occurrence.call_ordinal),
                    ) else {
                        diagnostics.push(Diagnostic::error(format!(
                            "statement-position direct call to public boundary requirement `{}` has an unrepresentable occurrence coordinate",
                            requirement.name,
                        )));
                        continue;
                    };
                    rewrites.push(RequirementCallRewrite {
                        site: RequirementCallSite::Statement(handle),
                        caller_state: state.symbol,
                        statement_ordinal,
                        call_ordinal,
                        requirement_state: row.requirement,
                        machine: realization.name.as_str().to_owned(),
                        entry_symbol: row.realization_state,
                    });
                }
            }
        }
        for (handle, expression) in typed.expression_table.expression_entries() {
            let ExpressionNode::Call(call) = expression else {
                continue;
            };
            if call.target_symbol != row.requirement {
                continue;
            }
            let receiver = match typed.expression_table.expression(call.receiver) {
                ExpressionNode::Name(path) => path.symbol,
                _ => symbols::SymbolHandle::invalid(),
            };
            if receiver != row.receiver {
                continue;
            }
            // The flow occurrence is the exact custody coordinate of this
            // authored call; the scalar-argument facts share it.
            let occurrences = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .flat_map(|(_, state)| {
                    checked
                        .facts
                        .flow
                        .control
                        .calls
                        .span_or_empty(state.calls)
                        .iter()
                        .filter(|call| call.authored_expression == handle)
                        .map(move |call| (state.state_symbol, call))
                })
                .collect::<Vec<_>>();
            let [(caller_state, occurrence)] = occurrences.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "direct call to public boundary requirement `{}` retains {} flow occurrences; expected one",
                    requirement.name,
                    occurrences.len(),
                )));
                continue;
            };
            let (Ok(statement_ordinal), Ok(call_ordinal)) = (
                u32::try_from(occurrence.statement_index),
                u32::try_from(occurrence.call_ordinal),
            ) else {
                diagnostics.push(Diagnostic::error(format!(
                    "direct call to public boundary requirement `{}` has an unrepresentable occurrence coordinate",
                    requirement.name,
                )));
                continue;
            };
            rewrites.push(RequirementCallRewrite {
                site: RequirementCallSite::Expression(handle),
                caller_state: *caller_state,
                statement_ordinal,
                call_ordinal,
                requirement_state: row.requirement,
                machine: realization.name.as_str().to_owned(),
                entry_symbol: row.realization_state,
            });
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(rewrites)
}

/// Redirect each journaled direct call to the realization entry, exactly as
/// the named boundary-operator adapter rewrite does: receiver cleared, target
/// name and entry symbol replaced, arguments and static bindings retained.
/// The retained flow occurrence and the checked scalar-argument facts follow
/// the call into the ordinary-call custody roles, so the rebuilt Unit plans
/// and Terminal source custody see one coherent ordinary call to the checked
/// body; the authored span, the journaled edit and the settled row keep the
/// requirement.
pub(super) fn apply_selected_requirement_rewrites(
    checked: &mut CheckedTrees,
    rewrites: &[RequirementCallRewrite],
    source_edits: &mut crate::source_edits::SourceEditBuilder,
) {
    use checked_trees::CheckedScalarExpressionRole;
    for rewrite in rewrites {
        match rewrite.site {
            RequirementCallSite::Expression(expression) => {
                source_edits.expression(&checked.typed, expression);
                let ExpressionNode::Call(mut call) = checked
                    .typed
                    .expression_table
                    .expression(expression)
                    .clone()
                else {
                    unreachable!("planned requirement rewrite ceased to be a call")
                };
                debug_assert_eq!(call.target_symbol, rewrite.requirement_state);
                call.receiver = ExpressionHandle::invalid();
                call.target = typed_trees::name::Identifier::generated(rewrite.machine.clone());
                call.target_symbol = rewrite.entry_symbol;
                *checked.typed.expression_table.expression_mut(expression) =
                    ExpressionNode::Call(call);
            }
            RequirementCallSite::Statement(statement) => {
                source_edits.statement_call(&checked.typed, statement);
                let typed_trees::statement::StatementNode::Call(mut call) =
                    checked.typed.statement_table.statement(statement).clone()
                else {
                    unreachable!("planned requirement rewrite ceased to be a statement call")
                };
                debug_assert_eq!(call.target_symbol, rewrite.requirement_state);
                call.receiver_root_symbol = symbols::SymbolHandle::invalid();
                call.receiver_symbol = symbols::SymbolHandle::invalid();
                call.receiver = arena::HandleSpan::empty();
                call.target = typed_trees::name::Identifier::generated(rewrite.machine.clone());
                call.target_symbol = rewrite.entry_symbol;
                *checked.typed.statement_table.statement_mut(statement) =
                    typed_trees::statement::StatementNode::Call(call);
            }
        }

        // The retained flow occurrence is the caller state's call at the
        // recorded statement/call coordinate for both site shapes; a
        // statement site has no authored-expression handle to match on.
        let calls = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .find(|(_, state)| state.state_symbol == rewrite.caller_state)
            .map(|(_, state)| state.calls);
        if let Some(calls) = calls {
            for call in checked.facts.flow.control.calls.span_mut_or_empty(calls) {
                if call.statement_index == rewrite.statement_ordinal as usize
                    && call.call_ordinal == rewrite.call_ordinal as usize
                {
                    call.target_symbol = rewrite.entry_symbol;
                    call.receiver_symbol = symbols::SymbolHandle::invalid();
                    call.has_receiver = false;
                }
            }
        }

        let unit_role = |role: CheckedScalarExpressionRole| match role {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal,
                argument_ordinal,
            } if call_ordinal == rewrite.call_ordinal => {
                Some(CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal,
                })
            }
            _ => None,
        };
        let values = &mut checked.facts.values;
        for located in values.scalar_expressions.expressions.iter_mut() {
            if located.state == rewrite.caller_state
                && located.statement_ordinal == rewrite.statement_ordinal
                && let Some(role) = unit_role(located.role)
            {
                located.role = role;
            }
        }
        let bindings = values
            .scalar_expressions
            .source_bindings
            .iter()
            .filter(|(_, binding)| {
                binding.state == rewrite.caller_state
                    && binding.statement_ordinal == rewrite.statement_ordinal
                    && unit_role(binding.role).is_some()
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in bindings {
            let binding = values.scalar_expressions.source_bindings.get_mut(handle);
            if let Some(role) = unit_role(binding.role) {
                binding.role = role;
            }
        }
        let roots = values
            .scalar_computations
            .roots
            .iter()
            .filter(|(_, root)| {
                root.state == rewrite.caller_state
                    && root.statement_ordinal == rewrite.statement_ordinal
                    && unit_role(root.role).is_some()
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in roots {
            let root = values.scalar_computations.roots.get_mut(handle);
            if let Some(role) = unit_role(root.role) {
                root.role = role;
            }
        }
    }
}
