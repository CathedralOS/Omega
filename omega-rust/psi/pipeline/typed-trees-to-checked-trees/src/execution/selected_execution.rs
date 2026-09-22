//! Settle checked execution after exact provider settlement.
//!
//! `settle_checked_execution` is the checked->checked settlement link: it
//! applies the requirement, operator-adapter and float-intrinsic rewrites the
//! orchestration owner planned to Psi's own typed program and facts,
//! re-infers the state write frames a rewrite changed, and builds the
//! Terminal plan lanes exactly once against the selected applications.
//! Omega plans the settlement from checked facts; Psi applies it, so no
//! previous-stage object is cloned or edited outside its owner.

use crate::execution::execution_plans::{ExecutionPlans, build_execution_plans};
use checked_trees::{CheckedScalarExpressionRole, CheckedTrees, CheckedValueOrigin};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementHandle, StatementNode};

/// Exact compiler-owned join from one authored operator use to the checked
/// machine selected to realize it. Selected execution supplies these rows only
/// after ProviderPlan settlement; ordinary checking supplies none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOperatorApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// One compiler-intrinsic nearest IEEE FMA selected for an attached Unit
/// local initializer. This is intentionally disjoint from checked-body
/// operator adapters: no bodyless call or fabricated realization machine is
/// introduced into checked Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIeeeFloatFmaUnitApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub format: semantic_vocabulary::IeeeFloatFormat,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// The authored call site a settlement rewrites: an expression-table call
/// or a statement-table call (`Owner::name(...);`, `_ = call();`, and
/// member calls such as `token.consume();`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettledCallSite {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

/// One settled direct call to a top-level boundary requirement, retargeted
/// to the exact checked body the selected provider row realizes it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledRequirementCall {
    pub site: SettledCallSite,
    /// The caller state and statement the flow occurrence and its checked
    /// scalar-argument facts are keyed on.
    pub caller_state: symbols::SymbolHandle,
    pub statement_ordinal: u32,
    pub call_ordinal: u32,
    pub requirement_state: symbols::SymbolHandle,
    /// A `self` row forwards the call's receiver place as the realization's
    /// leading argument instead of only clearing it.
    pub forward_receiver: bool,
    /// Statement sites only: the exact field symbol of each projected member
    /// after the receiver-path root (the leaf repeats the receiver symbol).
    /// Expression sites reuse the authored receiver expression and carry none.
    pub receiver_member_symbols: Vec<symbols::SymbolHandle>,
    pub machine: String,
    pub entry_symbol: symbols::SymbolHandle,
}

/// Where a settled operator adapter call takes its operands from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettledOperatorAdapterSource {
    /// The authored named operator call keeps its argument list.
    NamedCall,
    /// A spelled operator expression is replaced by a synthesized call over
    /// these operand expressions.
    Spelled(Box<[ExpressionHandle]>),
}

/// One authored operator use retargeted to its selected checked adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledOperatorAdapterCall {
    pub expression: ExpressionHandle,
    pub machine: String,
    pub entry_symbol: symbols::SymbolHandle,
    pub source: SettledOperatorAdapterSource,
}

/// The realization a compiler-intrinsic float call settles to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettledFloatIntrinsicExecution {
    /// The authored call is retargeted to a builtin function.
    Builtin {
        function: symbols::BuiltinFunction,
        symbol: symbols::SymbolHandle,
    },
    /// The authored call becomes `operand * -1.0` landed at the format.
    Negate(numerics::literals::FloatFormat),
    /// The authored call becomes a value cast of its operand.
    Convert {
        domain: numerics::arithmetic::ArithmeticDomain,
        target_type: typed_trees::types::TypeReferenceHandle,
    },
}

/// One bodyless float boundary call replaced by its intrinsic realization
/// in the authored body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettledFloatIntrinsic {
    pub expression: ExpressionHandle,
    /// The flow occurrence of the authored call, retired once the settled
    /// body no longer makes it.
    pub origin: CheckedValueOrigin,
    pub execution: SettledFloatIntrinsicExecution,
}

/// Everything one provider settlement changes about a checked program. The
/// rewrites are applied in this order; the applications are the exact joins
/// the rebuilt Terminal plan lanes plan against.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionSettlement<'a> {
    pub requirement_calls: &'a [SettledRequirementCall],
    pub operator_adapter_calls: &'a [SettledOperatorAdapterCall],
    pub float_intrinsics: &'a [SettledFloatIntrinsic],
    pub operator_applications: &'a [SelectedOperatorApplication],
    pub ieee_float_fma_unit_applications: &'a [SelectedIeeeFloatFmaUnitApplication],
}

impl ExecutionSettlement<'_> {
    fn rewrites_bodies(&self) -> bool {
        !self.requirement_calls.is_empty()
            || !self.operator_adapter_calls.is_empty()
            || !self.float_intrinsics.is_empty()
    }
}

/// Apply one provider settlement to a checked program: rewrite the settled
/// call sites in the typed program and the facts keyed on them, re-infer the
/// state write frames the rewrites changed, then build the Terminal plan
/// lanes once against the selected applications. The three lanes are
/// published together; a failure publishes nothing.
pub fn settle_checked_execution(
    mut checked: CheckedTrees,
    settlement: &ExecutionSettlement<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    for call in settlement.requirement_calls {
        apply_requirement_call(&mut checked, call);
    }
    for call in settlement.operator_adapter_calls {
        apply_operator_adapter_call(&mut checked, call);
    }
    for intrinsic in settlement.float_intrinsics {
        apply_float_intrinsic(&mut checked, intrinsic);
    }
    if settlement.rewrites_bodies() {
        // A rewrite replaces a bodyless boundary call with its realization in
        // the authored body, so the write frames retained at checking (opaque
        // across that call) no longer describe the settled body. Refresh them
        // before planning so Unit store planning sees one write frame.
        refresh_settled_state_write_frames(&mut checked)?;
    }
    let call_frames = validation::CallFrameResolver::new(&checked.typed);
    let ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    } = build_execution_plans(
        &checked.typed,
        &checked.facts,
        settlement.operator_applications,
        settlement.ieee_float_fma_unit_applications,
        call_frames.as_ref(),
    );
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    checked.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    checked.facts.flow.terminal_unit_effects = unit_effects;
    checked.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(checked)
}

/// Retarget one direct requirement call to its realization entry. The
/// authored span, the orchestration owner's edit journal and the settled
/// dispatch row keep the requirement; the flow call row and the checked
/// scalar-argument roles follow the call to its Unit target.
fn apply_requirement_call(checked: &mut CheckedTrees, rewrite: &SettledRequirementCall) {
    match rewrite.site {
        SettledCallSite::Expression(expression) => {
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
                // `recv.name(args)` becomes `Provider::entry(recv, args)`: the
                // authored receiver expression is spliced in as argument 0,
                // where the adapter's ordinary leading parameter binds it; the
                // caller's member-call custody claim on the receiver place is
                // preserved.
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
            *checked.typed.expression_table.expression_mut(expression) = ExpressionNode::Call(call);
        }
        SettledCallSite::Statement(statement) => {
            let StatementNode::Call(mut call) =
                checked.typed.statement_table.statement(statement).clone()
            else {
                unreachable!("planned requirement rewrite ceased to be a statement call")
            };
            debug_assert_eq!(call.target_symbol, rewrite.requirement_state);
            if rewrite.forward_receiver {
                // `token.consume();` becomes `Provider::entry(token);` and
                // `holder.inner.token.consume();` becomes
                // `Provider::entry(holder.inner.token);`: the root place is
                // reified as a Name expression and each projected member as a
                // Member expression carrying the exact field symbol planning
                // re-derived; the result is spliced in as argument 0.
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
                checked
                    .typed
                    .expression_table
                    .push_name_path_member_symbol(&mut member_symbols, call.receiver_root_symbol);
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
            *checked.typed.statement_table.statement_mut(statement) = StatementNode::Call(call);
        }
    }

    // The retained flow occurrence is the caller state's call at the recorded
    // statement/call coordinate for both site shapes; a statement site has no
    // authored-expression handle to match on.
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

/// Retarget one authored operator use to its selected checked adapter.
fn apply_operator_adapter_call(checked: &mut CheckedTrees, rewrite: &SettledOperatorAdapterCall) {
    let replacement = match &rewrite.source {
        SettledOperatorAdapterSource::NamedCall => {
            let ExpressionNode::Call(mut call) = checked
                .typed
                .expression_table
                .expression(rewrite.expression)
                .clone()
            else {
                unreachable!("validated named operator rewrite ceased to be a call")
            };
            call.receiver = ExpressionHandle::invalid();
            call.target = typed_trees::name::Identifier::generated(rewrite.machine.clone());
            call.target_symbol = rewrite.entry_symbol;
            ExpressionNode::Call(call)
        }
        SettledOperatorAdapterSource::Spelled(operands) => {
            let arguments = checked
                .typed
                .expression_table
                .insert_expression_handles(operands.iter().copied());
            ExpressionNode::Call(typed_trees::expression::TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: rewrite.entry_symbol,
                target: typed_trees::name::Identifier::generated(rewrite.machine.clone()),
                static_machine_parameter: symbols::SymbolHandle::invalid(),
                static_requirement_dispatch: None,
                machine_arguments: Box::new([]),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::new([]),
                operational_acknowledgement: language_core::CallOperationalAcknowledgement {
                    origin:
                        language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    acknowledges_suspend: false,
                    acknowledges_block: false,
                },
            })
        }
    };
    *checked
        .typed
        .expression_table
        .expression_mut(rewrite.expression) = replacement;
}

/// Replace one bodyless float boundary call with its intrinsic realization
/// and retire the flow call row the settled body no longer makes.
fn apply_float_intrinsic(checked: &mut CheckedTrees, rewrite: &SettledFloatIntrinsic) {
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(rewrite.expression)
        .clone()
    else {
        unreachable!("validated named-float rewrite ceased to be a call before publication");
    };
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    let replacement = match rewrite.execution {
        SettledFloatIntrinsicExecution::Builtin { function, symbol } => {
            let mut call = call;
            call.receiver = ExpressionHandle::invalid();
            call.target = typed_trees::name::Identifier::generated(function.name());
            call.target_symbol = symbol;
            ExpressionNode::Call(call)
        }
        SettledFloatIntrinsicExecution::Negate(format) => {
            let negative_one = checked.typed.expression_table.insert(ExpressionNode::Float(
                numerics::literals::FloatLiteral::from_f64(-1.0).with_landing(format),
            ));
            ExpressionNode::Binary(typed_trees::expression::TableBinaryExpression {
                left: arguments[0],
                operator: typed_trees::expression::BinaryOperator::Multiply,
                right: negative_one,
            })
        }
        SettledFloatIntrinsicExecution::Convert {
            domain,
            target_type,
        } => ExpressionNode::Cast(typed_trees::expression::TableCastExpression {
            value: arguments[0],
            target_type,
            result_type: typed_trees::types::TypeReferenceHandle::invalid(),
            target_label: arena::HandleSpan::empty(),
            domain,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: symbols::SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: language_core::CastForm::Value,
        }),
    };
    *checked
        .typed
        .expression_table
        .expression_mut(rewrite.expression) = replacement;
    retire_rewritten_call_row(checked, rewrite.origin, rewrite.expression);
}

/// The flow call row captured for the authored call no longer names a call
/// the settled body makes; retire it so execution planning skips the row
/// while the certificates holding its handle stay valid.
fn retire_rewritten_call_row(
    checked: &mut CheckedTrees,
    origin: CheckedValueOrigin,
    expression: ExpressionHandle,
) {
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = origin
    else {
        return;
    };
    let statement_root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            checked
                .typed
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
                .map(|state| state.statement_nodes)
        })
        .and_then(|statements| {
            checked
                .typed
                .statement_table
                .statements(statements)
                .get(statement_index)
        })
        .is_some_and(
            |statement| matches!(statement, StatementNode::Expression(root) if *root == expression),
        );
    checked.facts.flow.control.retire_calls(
        machine_symbol,
        state_symbol,
        statement_index,
        expression,
        statement_root,
    );
}

/// Re-derive the retained state write frames from the settled typed program
/// after settlement rewrote authored bodies in place.
///
/// Checking retains an opaque frame for a state whose body calls a bodyless
/// boundary declaration; settlement replaces such a call with its selected
/// realization (an adapter call, a builtin, or an arithmetic expression), so
/// the settled body carries a derivable frame that the retained one no longer
/// describes. Terminal store planning compares the retained frame with the one
/// re-inferred from the settled body, so the retained facts follow the
/// settled program. Every refreshed frame must equal the retained frame or
/// refine a retained opaque frame; any other change is a settlement fault.
fn refresh_settled_state_write_frames(
    program: &mut CheckedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let Some(call_frames) = validation::CallFrameResolver::new(&program.typed) else {
        return Err(vec![diagnostics::Diagnostic::error(
            "selected execution settlement cannot re-infer state write frames: the settled typed program has no call frame resolver",
        )]);
    };
    let refreshed = settled_mutation_facts(&program.typed, &call_frames);
    let mut diagnostics = Vec::new();
    for (retained, refreshed) in program
        .facts
        .mutation
        .machines
        .iter()
        .zip(refreshed.machines.iter())
    {
        for (retained_state, refreshed_state) in retained
            .state_write_frames
            .iter()
            .zip(refreshed.state_write_frames.iter())
        {
            let retained_opaque =
                retained_state.frame.completeness() == facts::WriteFrameCompleteness::Opaque;
            if retained_state.frame == refreshed_state.frame || retained_opaque {
                continue;
            }
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected execution settlement changed the complete write frame retained for state `{}`: retained {:?}, settled {:?}",
                program.typed.symbols.display_path(retained_state.state, "::"),
                retained_state.frame.paths(),
                refreshed_state.frame.paths(),
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    program.facts.mutation = refreshed;
    Ok(())
}

/// The settled program's write frames in the retained fact shape: one entry
/// per machine in machine-table order, one frame per state in state order.
fn settled_mutation_facts(
    program: &typed_trees::TypedTrees,
    call_frames: &validation::CallFrameResolver<'_>,
) -> checked_trees::MutationFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| checked_trees::MachineMutationFact {
            machine: machine.symbol,
            state_write_frames: program
                .machine_states(machine)
                .iter()
                .zip(call_frames.inferred_machine_state_write_frames(machine))
                .map(|(state, frame)| checked_trees::StateWriteFramePlan {
                    state: state.symbol,
                    frame,
                })
                .collect(),
        })
        .collect();
    checked_trees::MutationFacts { machines }
}

#[cfg(test)]
mod tests {
    use super::refresh_settled_state_write_frames;
    use crate::{CheckingRequest, lower_typed_trees};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> checked_trees::CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed, &CheckingRequest::settled()).expect("check")
    }

    #[test]
    fn write_frame_refresh_accepts_agreement_and_rejects_a_changed_complete_frame() {
        let mut program = checked(
            "data Counter { value: u64 }
             machine Counter::reset(&mut self) { self.value = 0u64; }",
        );
        let retained = program.facts.mutation.clone();
        refresh_settled_state_write_frames(&mut program)
            .expect("an unrewritten body derives the frames checking retained");
        assert_eq!(program.facts.mutation, retained);

        let plan = program
            .facts
            .mutation
            .machines
            .iter_mut()
            .flat_map(|machine| machine.state_write_frames.iter_mut())
            .find(|plan| plan.frame.completeness() == facts::WriteFrameCompleteness::Complete)
            .expect("a state with a complete retained frame");
        plan.frame = facts::NormalizedWriteFrame::complete(vec!["self.drifted".to_owned()]);
        let diagnostics = refresh_settled_state_write_frames(&mut program)
            .expect_err("a retained complete frame the settled body does not derive is a fault");
        assert!(
            diagnostics[0]
                .message
                .contains("changed the complete write frame retained for state"),
            "{:?}",
            diagnostics[0].message
        );
    }
}
