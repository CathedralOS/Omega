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
//! journal keep the requirement. An owned-`self` requirement settles the same
//! route through a member call (`token.consume();`, or a projected receiver
//! place like `holder.inner.token.consume();`): its row forwards the receiver
//! place, which the rewrite splices in as the adapter's leading argument. A requirement
//! satisfied by an external
//! `via` leaf settles no dispatch row and is deliberately not rewritten:
//! the call stays on the requirement, whose retained boundary seam is the
//! identity the native foreign-call join executes against.

use crate::boundary_dispatch::boundary_fields::named_type_symbol;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequirementCallRewrite {
    /// The journaled call site: an expression-table call or a
    /// statement-table call (`Owner::name(...);`, including `_ = call();`,
    /// and member calls `token.consume();` on a `self` requirement).
    site: RequirementCallSite,
    /// The caller state and statement the flow occurrence and its checked
    /// scalar-argument facts are keyed on.
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    call_ordinal: u32,
    requirement_state: symbols::SymbolHandle,
    /// The `self` row forwards the call's receiver place as the adapter's
    /// leading argument: the rewrite splices the receiver into argument
    /// position 0 instead of only clearing it.
    forward_receiver: bool,
    /// Statement sites only: the exact field symbol of each projected member
    /// after the receiver-path root (the leaf repeats `receiver_symbol`),
    /// re-derived by name inside the already-exact owner types at plan time.
    /// Expression sites reuse the authored receiver expression and carry none.
    receiver_member_symbols: Vec<symbols::SymbolHandle>,
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

/// The exact field symbol each projected member of a statement receiver path
/// resolves to: the statement table retains only the root and leaf member
/// symbols, so the intermediate members re-derive by name inside the previous
/// member's already-exact named owner — the same authority the
/// settlement-side receiver-place type walk in `boundary_dispatch` uses.
/// `members` carries the projections after the root place; the last must
/// resolve to `leaf`, or the path does not re-derive and stays fenced.
fn statement_receiver_member_symbols(
    typed: &TypedTrees,
    root: symbols::SymbolHandle,
    members: &[typed_trees::name::Identifier],
    leaf: symbols::SymbolHandle,
) -> Option<Vec<symbols::SymbolHandle>> {
    if members.is_empty() {
        return Some(Vec::new());
    }
    // The root place's declared type, wherever it is bound: place symbols are
    // unique across the program, so more than one binding means the path does
    // not re-derive and stays fenced.
    let mut root_types = Vec::new();
    // A `self.`-rooted receiver names its enclosing machine or state as the
    // root place, whose type is the machine's attached data rather than a
    // parameter or local declaration.
    let mut self_attached_data = Vec::new();
    for machine in typed.machines() {
        if machine.symbol == root
            || typed
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == root)
        {
            self_attached_data.push(machine.attached_data_symbol);
        }
        for state in typed.machine_states(machine) {
            for parameter in typed.state_parameters(state) {
                if parameter.symbol == root {
                    root_types.push(parameter.type_reference);
                }
            }
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::LocalData(local) = statement
                    && local.symbol == root
                {
                    root_types.push(local.type_reference);
                }
            }
        }
    }
    let mut data_symbol = match root_types.as_slice() {
        [type_reference] => named_type_symbol(typed, *type_reference)?,
        [] => match self_attached_data.as_slice() {
            [data_symbol] if data_symbol.is_valid() => *data_symbol,
            _ => return None,
        },
        _ => return None,
    };
    let mut member_symbols = Vec::with_capacity(members.len());
    for member in members {
        let owner = typed
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)?;
        let mut matching =
            typed
                .data_members(owner)
                .iter()
                .filter_map(|member_node| match member_node {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member.as_str() =>
                    {
                        Some((field.symbol, field.type_reference))
                    }
                    _ => None,
                });
        let (symbol, next_type) = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        member_symbols.push(symbol);
        data_symbol = named_type_symbol(typed, next_type)?;
    }
    (member_symbols.last().copied() == Some(leaf)).then_some(member_symbols)
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
        if !row.family_tuple.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "selected top-level boundary requirement `{}` settled a family row; only an exact nongeneric row is executable",
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
                    // A `self` row rewrites `token.consume()` into
                    // `Provider::consume(token)`: the receiver place is
                    // spliced as argument 0. The statement-table path retains
                    // only the root and leaf member symbols, so each
                    // projected member's exact field symbol re-derives by
                    // name inside the previous member's named owner -- the
                    // same authority the settlement-side receiver-place type
                    // walk uses; a path that cannot re-derive stays fenced.
                    let mut receiver_member_symbols = Vec::new();
                    if row.forward_receiver {
                        let path_members = typed.statement_table.name_path_members(call.receiver);
                        let derived = if path_members.is_empty()
                            || !call.receiver_root_symbol.is_valid()
                            || !call.receiver_symbol.is_valid()
                            || (path_members.len() == 1
                                && call.receiver_root_symbol != call.receiver_symbol)
                        {
                            None
                        } else {
                            statement_receiver_member_symbols(
                                typed,
                                call.receiver_root_symbol,
                                &path_members[1..],
                                call.receiver_symbol,
                            )
                        };
                        let Some(symbols) = derived else {
                            diagnostics.push(Diagnostic::error(format!(
                                "member call on receiver-bearing public boundary requirement `{}` forwards only a receiver place path whose projected members re-derive to exact field symbols inside their owner types",
                                requirement.name,
                            )));
                            continue;
                        };
                        receiver_member_symbols = symbols;
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
                        forward_receiver: row.forward_receiver,
                        receiver_member_symbols: std::mem::take(&mut receiver_member_symbols),
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
                ExpressionNode::Member(member) => member.member_symbol,
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
                forward_receiver: row.forward_receiver,
                receiver_member_symbols: Vec::new(),
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
/// A `self` row first splices the receiver place (a single place, or one
/// projection hop on it) into argument 0, where the adapter's ordinary
/// leading parameter binds it; the flow occurrence's receiver fields clear
/// with the call node's, matching the receiver-free rewrite.
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
                if rewrite.forward_receiver {
                    // `recv.name(args)` becomes `Provider::entry(recv, args)`:
                    // the authored receiver expression is spliced in as
                    // argument 0, where the adapter's ordinary leading
                    // parameter binds it; the caller's member-call custody
                    // claim on the receiver place is preserved.
                    let mut arguments = vec![call.receiver];
                    arguments.extend(
                        checked
                            .typed
                            .expression_table
                            .expression_handles(call.arguments)
                            .iter()
                            .copied(),
                    );
                    call.arguments = checked
                        .typed
                        .expression_table
                        .insert_expression_handles(arguments);
                }
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
                if rewrite.forward_receiver {
                    // `token.consume();` becomes `Provider::entry(token);`
                    // and `holder.inner.token.consume();` becomes
                    // `Provider::entry(holder.inner.token);`: the root place
                    // is reified as a Name expression and each projected
                    // member as a Member expression carrying the exact field
                    // symbol planning re-derived; the result is spliced in as
                    // argument 0. Planning fenced receiver paths whose
                    // members do not re-derive.
                    let path_members = checked
                        .typed
                        .statement_table
                        .name_path_members(call.receiver)
                        .to_vec();
                    debug_assert_eq!(
                        rewrite.receiver_member_symbols.len(),
                        path_members.len().saturating_sub(1)
                    );
                    let mut members = arena::HandleSpan::empty();
                    let mut member_symbols = arena::HandleSpan::empty();
                    checked
                        .typed
                        .expression_table
                        .push_name_path_member(&mut members, path_members[0].clone());
                    checked.typed.expression_table.push_name_path_member_symbol(
                        &mut member_symbols,
                        call.receiver_root_symbol,
                    );
                    let mut receiver_expression = checked.typed.expression_table.insert(
                        ExpressionNode::Name(typed_trees::expression::TableNamePath {
                            members,
                            member_symbols,
                            head_symbol: call.receiver_root_symbol,
                            symbol: call.receiver_root_symbol,
                        }),
                    );
                    for (member, member_symbol) in path_members[1..]
                        .iter()
                        .zip(rewrite.receiver_member_symbols.iter())
                    {
                        receiver_expression =
                            checked
                                .typed
                                .expression_table
                                .insert(ExpressionNode::Member(
                                    typed_trees::expression::TableMemberExpression {
                                        receiver: receiver_expression,
                                        member_symbol: *member_symbol,
                                        member: member.clone(),
                                        case_variant: None,
                                    },
                                ));
                    }
                    checked
                        .typed
                        .expression_table
                        .set_source_span(receiver_expression, call.source_span);
                    let mut arguments = vec![receiver_expression];
                    arguments.extend(
                        checked
                            .typed
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .copied(),
                    );
                    call.arguments = checked
                        .typed
                        .statement_table
                        .insert_expression_handles(arguments);
                }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use typed_trees::expression::ExpressionNode;

    const PROJECTED_RECEIVER_SOURCE: &str = r#"
        pub data Token {}
        pub boundary requirement Token::consume(self) -> i32;

        data TokenProvider {}
        machine TokenProvider::consume_impl(token: Token) -> i32
        satisfies Token::consume
        {
            transition { _ -> (41) }
        }

        data Holder { token: Token }
        data Client {}
        machine Client::run(&mut self, holder: Holder) -> i32 {
            _ = holder.token.consume();
            transition { _ -> (7) }
        }
        machine Client::value(&mut self, holder: Holder) -> i32 {
            transition { _ -> (holder.token.consume()) }
        }
    "#;

    fn requirement_fixture(
        source: &str,
    ) -> (
        checked_trees::CheckedTrees,
        Vec<effects::provider_plan::ProviderPlan>,
    ) {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize requirement fixture");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse requirement fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve requirement fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type requirement fixture");
        let plans = provider_planning::derive_satisfies_plans(
            &typed,
            provider_planning::ProviderPlanDerivation::unevaluated(None),
        )
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("check requirement fixture");
        (checked, plans)
    }

    fn selected_all(
        plans: &[effects::provider_plan::ProviderPlan],
    ) -> effects::SelectedProviderPlanFacts {
        let names = plans
            .iter()
            .map(|plan| plan.name.clone())
            .collect::<Vec<_>>();
        effects::SelectedProviderPlanFacts::from_selection(plans, &names)
            .expect("select every plan")
    }

    fn entry_symbol(
        checked: &checked_trees::CheckedTrees,
        machine_name: &str,
    ) -> symbols::SymbolHandle {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
        checked
            .typed
            .machine_states(machine)
            .first()
            .expect("entry state")
            .symbol
    }

    fn field_symbol(
        checked: &checked_trees::CheckedTrees,
        data_name: &str,
        field_name: &str,
    ) -> symbols::SymbolHandle {
        let data = checked
            .typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == data_name)
            .unwrap_or_else(|| panic!("missing data `{data_name}`"));
        checked
            .typed
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    Some(field.symbol)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing field `{data_name}::{field_name}`"))
    }

    /// A member call on a projected receiver place (`holder.token.consume()`)
    /// settles a receiver-place-keyed forwarding row on the leaf field symbol
    /// and the rewrite splices the projected place in as argument 0.
    #[test]
    fn a_projected_receiver_member_call_forwards_the_place_as_argument_zero() {
        let (checked, plans) = requirement_fixture(PROJECTED_RECEIVER_SOURCE);
        let selected = selected_all(&plans);
        let requirement = entry_symbol(&checked, "Token::consume");
        let realization = entry_symbol(&checked, "TokenProvider::consume_impl");
        let token_field = field_symbol(&checked, "Holder", "token");
        let holder = checked
            .typed
            .machines()
            .iter()
            .flat_map(|machine| checked.typed.machine_states(machine))
            .flat_map(|state| checked.typed.state_parameters(state))
            .find(|parameter| parameter.name.as_str() == "holder")
            .expect("the holder parameter")
            .symbol;

        let mut settled = Arc::new(checked);
        let edits =
            crate::settle_selected_execution_dispatch_with_source_edits(&mut settled, &selected)
                .expect("a projected receiver member call settles");
        let rows = &settled.facts.boundary_adapter_dispatch;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].receiver, token_field);
        assert_eq!(rows[0].requirement, requirement);
        assert!(rows[0].forward_receiver);

        // The statement call `_ = holder.token.consume();` redirects to the
        // adapter with the projected place reified as `holder.token` at
        // argument 0.
        let call = settled
            .typed
            .machines()
            .iter()
            .flat_map(|machine| settled.typed.machine_states(machine))
            .flat_map(|state| {
                settled
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)
            })
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() != "consume" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("the rewritten statement call");
        assert_eq!(call.target_symbol, realization);
        assert_eq!(call.target.as_str(), "TokenProvider::consume_impl");
        assert!(call.receiver.is_empty() && !call.receiver_symbol.is_valid());
        let arguments = settled
            .typed
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        assert_eq!(arguments.len(), 1);
        let ExpressionNode::Member(member) =
            settled.typed.expression_table.expression(arguments[0])
        else {
            panic!("the forwarded receiver is a member projection");
        };
        assert_eq!(member.member_symbol, token_field);
        let ExpressionNode::Name(root) = settled.typed.expression_table.expression(member.receiver)
        else {
            panic!("the projection root is a place name");
        };
        assert_eq!(root.symbol, holder);

        // The value-position call `holder.token.consume()` in the transition
        // arm redirects the same way, splicing the authored receiver
        // expression as argument 0.
        let value_call = settled
            .typed
            .expression_table
            .expression_entries()
            .find_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call)
                    if call.target.as_str() == "TokenProvider::consume_impl" =>
                {
                    Some((handle, call))
                }
                _ => None,
            })
            .expect("the rewritten value call");
        assert_eq!(value_call.1.target_symbol, realization);
        assert!(!value_call.1.receiver.is_valid());
        let value_arguments = settled
            .typed
            .expression_table
            .expression_handles(value_call.1.arguments)
            .to_vec();
        assert_eq!(value_arguments.len(), 1);
        let ExpressionNode::Member(value_receiver) = settled
            .typed
            .expression_table
            .expression(value_arguments[0])
        else {
            panic!("the forwarded value receiver is the authored projection");
        };
        assert_eq!(value_receiver.member_symbol, token_field);

        // The journal restores the authored requirement statement.
        let statement = settled
            .typed
            .machines()
            .iter()
            .flat_map(|machine| settled.typed.machine_states(machine))
            .flat_map(|state| {
                settled
                    .typed
                    .statement_table
                    .iter_statements(state.statement_nodes)
            })
            .find_map(|(handle, statement)| match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() == "TokenProvider::consume_impl" =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .expect("the rewritten statement handle");
        let source = edits
            .source_trees(&settled.typed)
            .expect("restore the journaled source");
        let typed_trees::statement::StatementNode::Call(authored) =
            source.statement_table.statement(statement)
        else {
            panic!("the restored statement is a call");
        };
        assert_eq!(authored.target_symbol, requirement);
        assert_eq!(authored.target.as_str(), "consume");
        assert_eq!(authored.receiver_symbol, token_field);
        assert!(authored.discards_result);
    }

    const DEEP_PROJECTED_RECEIVER_SOURCE: &str = r#"
        pub data Token {}
        pub boundary requirement Token::consume(self) -> i32;

        data TokenProvider {}
        machine TokenProvider::consume_impl(token: Token) -> i32
        satisfies Token::consume
        {
            transition { _ -> (41) }
        }

        data Inner { token: Token }
        data Holder { inner: Inner }
        data Client {}
        machine Client::run(&mut self, holder: Holder) -> i32 {
            _ = holder.inner.token.consume();
            transition { _ -> (7) }
        }
        machine Client::value(&mut self, holder: Holder) -> i32 {
            transition { _ -> (holder.inner.token.consume()) }
        }
    "#;

    /// A member call on a deeper projected receiver place
    /// (`holder.inner.token.consume()`) settles the same
    /// receiver-place-keyed forwarding row on the leaf field symbol; the
    /// rewrite re-derives the intermediate member's exact field symbol and
    /// reifies the whole place as argument 0 in both call positions.
    #[test]
    fn a_deeper_projected_receiver_member_call_forwards_the_place_as_argument_zero() {
        let (checked, plans) = requirement_fixture(DEEP_PROJECTED_RECEIVER_SOURCE);
        let selected = selected_all(&plans);
        let requirement = entry_symbol(&checked, "Token::consume");
        let realization = entry_symbol(&checked, "TokenProvider::consume_impl");
        let inner_token_field = field_symbol(&checked, "Inner", "token");
        let holder_inner_field = field_symbol(&checked, "Holder", "inner");
        let holder = checked
            .typed
            .machines()
            .iter()
            .flat_map(|machine| checked.typed.machine_states(machine))
            .flat_map(|state| checked.typed.state_parameters(state))
            .find(|parameter| parameter.name.as_str() == "holder")
            .expect("the holder parameter")
            .symbol;

        let mut settled = Arc::new(checked);
        let edits =
            crate::settle_selected_execution_dispatch_with_source_edits(&mut settled, &selected)
                .expect("a deeper projected receiver member call settles");
        let rows = &settled.facts.boundary_adapter_dispatch;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].receiver, inner_token_field);
        assert_eq!(rows[0].requirement, requirement);
        assert!(rows[0].forward_receiver);

        // The statement call `_ = holder.inner.token.consume();` redirects to
        // the adapter with the whole projected place reified as
        // `holder.inner.token` at argument 0.
        let call = settled
            .typed
            .machines()
            .iter()
            .flat_map(|machine| settled.typed.machine_states(machine))
            .flat_map(|state| {
                settled
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)
            })
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() != "consume" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("the rewritten statement call");
        assert_eq!(call.target_symbol, realization);
        assert_eq!(call.target.as_str(), "TokenProvider::consume_impl");
        assert!(call.receiver.is_empty() && !call.receiver_symbol.is_valid());
        let arguments = settled
            .typed
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        assert_eq!(arguments.len(), 1);
        let ExpressionNode::Member(leaf_member) =
            settled.typed.expression_table.expression(arguments[0])
        else {
            panic!("the forwarded receiver is a member projection");
        };
        assert_eq!(leaf_member.member_symbol, inner_token_field);
        let ExpressionNode::Member(inner_member) = settled
            .typed
            .expression_table
            .expression(leaf_member.receiver)
        else {
            panic!("the intermediate member is a member projection");
        };
        assert_eq!(inner_member.member_symbol, holder_inner_field);
        let ExpressionNode::Name(root) = settled
            .typed
            .expression_table
            .expression(inner_member.receiver)
        else {
            panic!("the projection root is a place name");
        };
        assert_eq!(root.symbol, holder);

        // The value-position call `holder.inner.token.consume()` redirects
        // the same way, splicing the authored receiver expression as
        // argument 0.
        let value_call = settled
            .typed
            .expression_table
            .expression_entries()
            .find_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call)
                    if call.target.as_str() == "TokenProvider::consume_impl" =>
                {
                    Some((handle, call))
                }
                _ => None,
            })
            .expect("the rewritten value call");
        assert_eq!(value_call.1.target_symbol, realization);
        assert!(!value_call.1.receiver.is_valid());
        let value_arguments = settled
            .typed
            .expression_table
            .expression_handles(value_call.1.arguments)
            .to_vec();
        assert_eq!(value_arguments.len(), 1);
        let ExpressionNode::Member(value_receiver) = settled
            .typed
            .expression_table
            .expression(value_arguments[0])
        else {
            panic!("the forwarded value receiver is the authored projection");
        };
        assert_eq!(value_receiver.member_symbol, inner_token_field);

        // The journal restores the authored requirement statement.
        let statement = settled
            .typed
            .machines()
            .iter()
            .flat_map(|machine| settled.typed.machine_states(machine))
            .flat_map(|state| {
                settled
                    .typed
                    .statement_table
                    .iter_statements(state.statement_nodes)
            })
            .find_map(|(handle, statement)| match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() == "TokenProvider::consume_impl" =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .expect("the rewritten statement handle");
        let source = edits
            .source_trees(&settled.typed)
            .expect("restore the journaled source");
        let typed_trees::statement::StatementNode::Call(authored) =
            source.statement_table.statement(statement)
        else {
            panic!("the restored statement is a call");
        };
        assert_eq!(authored.target_symbol, requirement);
        assert_eq!(authored.target.as_str(), "consume");
        assert_eq!(authored.receiver_symbol, inner_token_field);
        assert!(authored.discards_result);
    }
}
