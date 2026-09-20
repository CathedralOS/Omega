//! One-to-one lowering of statement nodes, transition guards and targets.

use crate::lowering::expression::lower_private_expression_into_table;
use crate::lowering::statement::guarded_arm_rewrites::{
    rewrite_guarded_call_arm, rewrite_guarded_transition_argument_calls,
};
use crate::lowering::statement::indexed_read_hoisting::{
    OperandHoisting, hoist_child, hoist_into_temp, hoist_operand_indexed_reads,
    hoist_target_computed_indices,
};
use crate::lowering::statement::match_subject_hoisting::{
    guard_hoists_operands, hoist_comparison_match_subject, hoist_membership_match_subject,
    is_reference_struct_parameter_member,
};
use crate::lowering::statement::value_call_hoisting::{
    hoist_scalar_value_call_comparison, hoist_terminal_value_machine_call,
    is_scalar_return_computation,
};
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{
    AssemblyFact, AssemblyFactKind, Assignment, Call, CallStorage, LocalData, LocalDataStorage,
    NamedTransitionTarget, NamedTransitionTargetStorage, Statement, Transition, TransitionExit,
    TransitionGuard, TransitionTarget,
};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_statement_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: &syntax::statement::StatementNode,
    statement_index: usize,
    has_preceding_transition: bool,
) -> Result<Vec<Statement>, Diagnostic> {
    match statement {
        syntax::statement::StatementNode::RootBinding(binding) => Ok(vec![Statement::RootBinding(
            symbol_resolved_trees::statement::RootBinding {
                receiver: lower_statement_expression(lowerer, syntax_trees, binding.receiver)?,
                slot: binding
                    .slot
                    .iter()
                    .map(crate::lowering::name::lower_name)
                    .collect(),
                implementation: binding
                    .implementation
                    .iter()
                    .map(crate::lowering::name::lower_name)
                    .collect(),
                // An ambiguous bare name is lowered outside the authored-
                // selection exposure so a name that resolves to a
                // declaration cannot mint a cross-package selection record.
                // Routing clears the operand in that case and the lexical
                // `implementation` path remains the only declaration channel.
                // Computed descriptions instead retain ordinary authored call
                // selections; their bodies execute in the build context.
                implementation_operand: if binding.implementation_operand.is_valid() {
                    if !matches!(
                        syntax_trees
                            .expressions
                            .expression(binding.implementation_operand),
                        syntax::expression::ExpressionNode::Name(_)
                    ) {
                        lower_statement_expression(
                            lowerer,
                            syntax_trees,
                            binding.implementation_operand,
                        )?
                    } else {
                        let exposure = lowerer.current_authored_expression_exposure.take();
                        let lowered = crate::lowering::expression::lower_expression_into_table(
                            lowerer,
                            syntax_trees,
                            binding.implementation_operand,
                        );
                        lowerer.current_authored_expression_exposure = exposure;
                        lowered?
                    }
                } else {
                    symbol_resolved_trees::expression::ExpressionHandle::invalid()
                },
                source_span: binding.source_span,
            },
        )]),
        syntax::statement::StatementNode::AssemblyFact(fact) => {
            Ok(vec![Statement::AssemblyFact(AssemblyFact {
                kind: match fact.kind {
                    syntax::statement::AssemblyFactKind::Requires => AssemblyFactKind::Requires,
                    syntax::statement::AssemblyFactKind::Ensures => AssemblyFactKind::Ensures,
                },
                expression: lower_statement_expression(lowerer, syntax_trees, fact.expression)?,
            })])
        }
        syntax::statement::StatementNode::Assignment(assignment) => {
            if let Some((target, source)) = evidence_forwarding_names(
                lowerer,
                syntax_trees,
                assignment.target,
                assignment.value,
            ) {
                lowerer.symbol_resolved_trees.evidence_forwardings.push(
                    symbol_resolved_trees::statement::EvidenceForwarding {
                        machine_root_index: lowerer
                            .current_machine_root_index
                            .unwrap_or(usize::MAX),
                        machine_name: DiagnosticName::generated(
                            lowerer.current_machine_name.as_deref().unwrap_or_default(),
                        ),
                        state_name: DiagnosticName::generated(
                            lowerer.current_state_name.as_deref().unwrap_or_default(),
                        ),
                        machine_symbol: SymbolHandle::invalid(),
                        state_symbol: SymbolHandle::invalid(),
                        statement_index,
                        target: crate::lowering::name::lower_name(target),
                        source: crate::lowering::name::lower_name(source),
                        source_conformance: None,
                    },
                );
                return Ok(Vec::new());
            }
            let target = lower_statement_expression(lowerer, syntax_trees, assignment.target)?;
            let value = lower_statement_expression(lowerer, syntax_trees, assignment.value)?;
            let mut hoisted = Vec::new();
            // A computed index in the write TARGET (`arr[k + 1] = v`) hoists
            // the same way as in value positions: the temp indexes as a
            // slotted plain place.
            hoist_target_computed_indices(lowerer, target, &mut hoisted);
            let value = hoist_operand_indexed_reads(
                lowerer,
                value,
                &mut hoisted,
                OperandHoisting::Computation,
            );
            // A BARE ref-param member as the whole RHS (`self.c = table.con_out`)
            // is not an operand, so the rewrite above leaves it -- and the flat
            // machine-write path would read frame garbage. Hoist the root into a
            // `let`, which lowers through the pointee-deref path.
            let value = if is_reference_struct_parameter_member(lowerer, value) {
                hoist_child(lowerer, value, &mut hoisted, OperandHoisting::Value)
            } else {
                value
            };
            hoisted.push(Statement::Assignment(Assignment { target, value }));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Call(call) => {
            let receiver = lower_statement_path_members(lowerer, syntax_trees, call.receiver);
            if call.target_is_static && !receiver.is_empty() {
                let mut path = lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .statement_path_members
                    .span_or_empty(receiver)
                    .to_vec();
                path.push(crate::lowering::name::lower_name(&call.target));
                lowerer
                    .pending_static_module_statement_calls
                    .push((call.target.source_span(), path));
            }
            let arguments = lower_statement_expressions(lowerer, syntax_trees, call.arguments)?;
            // A ref-param member as a CALL ARGUMENT (`out.output_string(
            // table.con_out, ..)`) folds flat -- slot+field frame read, silent
            // garbage into the callee. Hoist each such argument into a `let`
            // (the pointee-deref path) and pass the temp. Indexed-read args are
            // deliberately NOT hoisted here (their call-arg substitution is
            // correct); only ref-param members, whose substitution is the bug.
            let mut hoisted = Vec::new();
            for offset in 0..arguments.count() {
                let argument = lowerer
                    .symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(arguments)[offset as usize];
                if is_reference_struct_parameter_member(lowerer, argument) {
                    let temp = hoist_into_temp(lowerer, argument, &mut hoisted);
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .bodies
                        .expressions
                        .set_expression_handle_at_offset(arguments, offset, temp);
                }
            }
            hoisted.push(Statement::Call(Call {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: SymbolHandle::invalid(),
                target: crate::lowering::name::lower_name(&call.target),
                storage: CallStorage {
                    receiver_root_symbol: SymbolHandle::invalid(),
                    receiver,
                    receiver_starts_at_self: call.receiver_starts_at_self,
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(|argument| {
                            crate::lowering::expression::lower_static_machine_argument(
                                lowerer,
                                syntax_trees,
                                argument,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                    arguments,
                    evidence_arguments: call
                        .evidence_arguments
                        .iter()
                        .map(crate::lowering::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
                    authored_call_selection: None,
                },
            }));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::ProofOutputBindingStatement(binding) => {
            let call = lower_statement_expression(lowerer, syntax_trees, binding.call)?;
            let runtime_value = binding.bindings.iter().find(|binding| {
                binding.output_field.as_str() == "value" && binding.binding.as_str() != "_"
            });
            let bindings = binding
                .bindings
                .iter()
                .map(|binding| {
                    if binding.output_field.as_str() != "value" && binding.binding.as_str() != "_" {
                        lowerer
                            .current_evidence_term_names
                            .push(binding.binding.as_str().to_owned());
                    }
                    symbol_resolved_trees::statement::ProofOutputSelector {
                        output_field: crate::lowering::name::lower_name(&binding.output_field),
                        binding: crate::lowering::name::lower_name(&binding.binding),
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let mut lowered = Vec::with_capacity(usize::from(runtime_value.is_some()) + 1);
            if let Some(runtime_value) = runtime_value {
                // `value` is the sole runtime representation of the generated
                // package. Reusing this exact resolved call handle in the
                // erased metadata does not execute it twice; only this ordinary
                // local enters the typed runtime statement stream.
                lowered.push(Statement::LocalData(LocalData {
                    symbol: SymbolHandle::invalid(),
                    name: crate::lowering::name::lower_name(&runtime_value.binding),
                    storage: LocalDataStorage {
                        type_reference: TypeReference::Unit,
                        initial_value: call,
                        is_mutable: false,
                        type_is_inferred: true,
                        relevance: language_core::BindingRelevance::Relevant,
                    },
                }));
            }
            lowered.push(Statement::ProofOutputBindingStatement(
                symbol_resolved_trees::statement::ProofOutputBindingStatement {
                    machine_symbol: SymbolHandle::invalid(),
                    state_symbol: SymbolHandle::invalid(),
                    statement_index,
                    bindings,
                    call,
                },
            ));
            Ok(lowered)
        }
        syntax::statement::StatementNode::Expression(expression) => {
            // A bare trailing expression is a VALUE machine's implicit return
            // (`state go(..) -> i64 { ..; self.buf[j] as i64 }`). Its
            // operand-position runtime-indexed reads need the SAME hoist the
            // assignment-value / let-initializer / transition-value paths apply
            // -- otherwise `self.buf[j]` reaches selection as a raw machine
            // runtime-indexed read with no value-operand lowering and falls to
            // the place resolver, which drops the index and reads the wrong base
            // (native only; the interpreter masks it). Root left whole,
            // matching the transition-value target.
            let mut hoisted = Vec::new();
            let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
            let expression = hoist_operand_indexed_reads(
                lowerer,
                expression,
                &mut hoisted,
                OperandHoisting::Computation,
            );
            // A free or direct-self value-machine call as the trailing return
            // (`state go(..) -> f64 { ..; self.sin(x) }`) hoists into the
            // let-bound spelling, exactly as the transition-value face.
            // Trailing returns are unconditional, so no guard gate applies.
            let expression = if matches!(
                lowerer.current_state_return_type,
                None | Some(TypeReference::Unit)
            ) {
                // Preserve the authored terminal expression in a Unit context.
                // This does not classify the callee or discard its result:
                // checking must still require an actual Unit-producing call.
                // A synthetic local would instead require value storage and
                // return-type inference for a call that may have no value.
                expression
            } else {
                hoist_terminal_value_machine_call(lowerer, expression, &mut hoisted)
            };
            hoisted.push(Statement::Expression(expression));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::LocalData(local_data) => {
            // Parse-time desugars (destructure lets) mint TYPELESS locals;
            // the Unit sentinel defers typing to the initializer (the
            // hoist rule the resolved->typed layer already serves).
            let type_reference = if local_data.type_reference.is_valid() {
                lower_type_reference_handle(lowerer, syntax_trees, local_data.type_reference)?
            } else {
                TypeReference::Unit
            };
            let capturable_local = (!matches!(type_reference, TypeReference::Unit)).then(|| {
                (
                    local_data.name.as_str().to_owned(),
                    type_reference.clone(),
                    local_data.is_mutable,
                )
            });
            let initial_value = if local_data.initial_value.is_valid() {
                lower_statement_expression(lowerer, syntax_trees, local_data.initial_value)?
            } else {
                ExpressionHandle::invalid()
            };
            let mut hoisted = Vec::new();
            let initial_value = if initial_value.is_valid() {
                hoist_operand_indexed_reads(
                    lowerer,
                    initial_value,
                    &mut hoisted,
                    OperandHoisting::Computation,
                )
            } else {
                initial_value
            };
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(&local_data.name),
                storage: LocalDataStorage {
                    type_reference,
                    initial_value,
                    is_mutable: local_data.is_mutable,
                    type_is_inferred: !local_data.type_reference.is_valid(),
                    relevance: local_data.relevance,
                },
            }));
            if let Some(local) = capturable_local {
                lowerer.current_state_locals.push(local);
            }
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Transition(transition) => {
            let mut hoisted = Vec::new();
            // An Always fallback is selected only after earlier arms fail.
            // Its target work cannot join the pre-dispatch hoist prefix.
            let unconditional = !has_preceding_transition
                && matches!(
                    transition.guard,
                    syntax::statement::TransitionGuardNode::Always
                );
            let mut target = lower_transition_target_node(
                lowerer,
                syntax_trees,
                transition.target,
                &mut hoisted,
                unconditional,
            )?;
            // Free scalar return calls keep their exact destination for checked
            // computation lowering. Other unconditional calls retain the older
            // let-bound route; guarded calls must stay behind arm selection.
            if unconditional {
                if let TransitionTarget::Value(expression) = target
                    && !is_scalar_return_computation(lowerer, expression)
                {
                    let rewritten =
                        hoist_terminal_value_machine_call(lowerer, expression, &mut hoisted);
                    if rewritten != expression {
                        target = TransitionTarget::Value(rewritten);
                    }
                }
            } else {
                // GUARDED-ARM DEEP FIX (task #45): a guarded arm's value call
                // cannot hoist above the transition (the callee would run when
                // the arm is not taken), so rewrite `cond -> (call(a, b))`
                // into `cond -> __arm_k_N(a, b)` plus a synthesized
                // continuation state whose Always terminal hoists the call --
                // the mul_comm/mc_step shape the language already serves,
                // automated. V1 gates the arguments to enclosing-parameter
                // NAMES (the synthesized state's parameter types copy over).
                target = rewrite_guarded_call_arm(lowerer, target);
                target = rewrite_guarded_transition_argument_calls(lowerer, target);
            }
            let continuation = if transition.continuation.is_valid() {
                // A continuation arm is conditional by construction (it runs
                // only when the guard fails) -- same rewrite, never a hoist.
                let lowered = lower_transition_target_node(
                    lowerer,
                    syntax_trees,
                    transition.continuation,
                    &mut hoisted,
                    unconditional,
                )?;
                if unconditional {
                    // A lone wildcard arm is represented as the continuation
                    // of an Always transition. It is unconditional, so keep
                    // its existing target intact (including ownership-call
                    // ordinals) instead of synthesizing an arm-local state.
                    Some(lowered)
                } else {
                    let lowered = rewrite_guarded_call_arm(lowerer, lowered);
                    Some(rewrite_guarded_transition_argument_calls(lowerer, lowered))
                }
            } else {
                None
            };
            let guard = lower_transition_guard_node(lowerer, syntax_trees, transition.guard)?;
            // Hoist runtime-indexed reads out of the guard's OPERAND positions, exactly as for
            // assignment values and let initializers above, so `transition self.arr[i] > 5`
            // becomes `let __hoist = self.arr[i]; transition __hoist > 5`. Without this the guard
            // subject keeps a raw runtime-indexed read that has no valid static byte offset, and
            // the compare silently reads element 0. Binding to a local first is the sound idiom;
            // this makes it automatic. A bare match subject (`transition self.arr[i] { .. }`) is
            // the guard ROOT, which `hoist_operand_indexed_reads` leaves whole -- so match
            // exhaustiveness (which needs a single shared subject across arms) is unaffected.
            if let TransitionGuard::When(expression) = guard {
                // A USER value-machine call on one side of a comparison
                // (`transition self.next() == expected`) is hoisted into a `let`
                // temp FIRST -- the direct shape has no guard lowering (the
                // callee body is never spliced for guard-role calls; the
                // emission blocker rejects it), while the let-bound call is
                // the fully working assignment path. The rewritten guard
                // (`__hoist == expected`) then flows through the match-subject /
                // operand hoists below unchanged. A comparison with user calls
                // on both sides stays explicit until both evaluation results
                // can be materialized without reordering them.
                hoist_scalar_value_call_comparison(
                    lowerer,
                    syntax_trees,
                    transition.guard,
                    expression,
                    &mut hoisted,
                );
                if guard_hoists_operands(lowerer, expression) {
                    // A bool-arm dispatch (`{ true -> .. false -> .. }`) over a
                    // hoistable comparison subject shares ONE temp across arms so
                    // the true/false pair still pairs for exhaustiveness (each arm
                    // otherwise re-lowers the subject to its own temp). If that
                    // fires it rewrites the guard to `__hoist == <bool>`, leaving
                    // nothing for the operand hoist.
                    if !hoist_comparison_match_subject(
                        lowerer,
                        syntax_trees,
                        transition.guard,
                        expression,
                        &mut hoisted,
                    ) {
                        // In GUARD position also hoist a pure-builtin subject
                        // (`transition min(self.a, self.b) == 3`) into a temp, so the
                        // guard compares a materialized local -- the sound idiom the
                        // "bind it to a local first" diagnostic asks for, made
                        // automatic. Scoped to guards: assignment
                        // and let values above already lower builtin calls directly.
                        hoist_operand_indexed_reads(
                            lowerer,
                            expression,
                            &mut hoisted,
                            OperandHoisting::Guard,
                        );
                    }
                } else {
                    // A `Membership` root is an enum-variant match arm (`grid[i] { Wall -> .. }`);
                    // the comparison hoist above skips it (not a `Binary`). Hoist its runtime-indexed
                    // subject into a SHARED temp so all arms of the match test one plain local.
                    hoist_membership_match_subject(
                        lowerer,
                        syntax_trees,
                        transition.guard,
                        expression,
                        &mut hoisted,
                    );
                }
            }
            hoisted.push(Statement::Transition(Transition {
                target,
                continuation,
                guard,
                proof_selectors: syntax_trees
                    .statements
                    .outcome_proof_selectors(transition.proof_selectors)
                    .iter()
                    .map(
                        |selector| symbol_resolved_trees::statement::OutcomeProofSelector {
                            output_field: crate::lowering::name::lower_name(&selector.output_field),
                            binding: crate::lowering::name::lower_name(&selector.binding),
                        },
                    )
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                exit: match transition.exit {
                    syntax::statement::TransitionExit::Ordinary => TransitionExit::Ordinary,
                    syntax::statement::TransitionExit::Crash(cause) => {
                        TransitionExit::Crash(match cause {
                            syntax::item::CrashCause::Trap => {
                                symbol_resolved_trees::signature::CrashCause::Trap
                            }
                            syntax::item::CrashCause::Abort => {
                                symbol_resolved_trees::signature::CrashCause::Abort
                            }
                        })
                    }
                },
                source_span: transition.source_span,
            }));
            Ok(hoisted)
        }
    }
}

fn evidence_forwarding_names<'syntax>(
    lowerer: &Lowerer,
    syntax_trees: &'syntax SyntaxTrees,
    target: syntax::expression::ExpressionHandle,
    source: syntax::expression::ExpressionHandle,
) -> Option<(
    &'syntax syntax::identifier::Identifier,
    &'syntax syntax::identifier::Identifier,
)> {
    let target = bare_syntax_name(syntax_trees, target)?;
    let source = bare_syntax_name(syntax_trees, source)?;
    (lowerer
        .current_evidence_term_names
        .iter()
        .any(|name| name == target.as_str() || name == source.as_str()))
    .then_some((target, source))
}

fn bare_syntax_name(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Option<&syntax::identifier::Identifier> {
    let syntax::expression::ExpressionNode::Name(path) =
        syntax_trees.expressions.expression(expression)
    else {
        return None;
    };
    let [name] = syntax_trees.expressions.identifier_path_members(*path) else {
        return None;
    };
    Some(name)
}

pub(crate) fn set_expression(
    lowerer: &mut Lowerer,
    handle: ExpressionHandle,
    node: ExpressionNode,
) {
    *lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression_mut(handle) = node;
}

fn lower_statement_expression(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<symbol_resolved_trees::expression::ExpressionHandle, Diagnostic> {
    lower_private_expression_into_table(lowerer, syntax_trees, expression)
}

fn lower_statement_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
    let span = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_expression_handles(expressions.count());

    for (offset, expression) in syntax_trees
        .statements
        .expression_handles(expressions)
        .iter()
        .enumerate()
    {
        let expression = lower_private_expression_into_table(lowerer, syntax_trees, *expression)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
    }

    Ok(span)
}

fn lower_transition_guard_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    guard: syntax::statement::TransitionGuardNode,
) -> Result<TransitionGuard, Diagnostic> {
    match guard {
        syntax::statement::TransitionGuardNode::Always => Ok(TransitionGuard::Always),
        syntax::statement::TransitionGuardNode::When(expression) => Ok(TransitionGuard::When(
            lower_statement_expression(lowerer, syntax_trees, expression)?,
        )),
    }
}

fn lower_transition_target_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    target: syntax::statement::TransitionTargetHandle,
    hoisted: &mut Vec<Statement>,
    unconditional: bool,
) -> Result<TransitionTarget, Diagnostic> {
    match syntax_trees.statements.transition_target(target) {
        syntax::statement::TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
            evidence_arguments,
            source_span,
        } => {
            let arguments = lower_statement_expressions(lowerer, syntax_trees, *arguments)?;
            // A runtime-indexed read in OPERAND position inside a transition
            // ARGUMENT (`-> dot(.., acc + a[i][k] * b[k][j])`) has no value
            // operand and SILENTLY read 0 (native; the interpreter was right)
            // -- the same gap the assignment-value/let/guard hoists close.
            // Hoist each argument's operand-position indexed reads into
            // `let __hoist_N` temps (the root is left whole: a BARE indexed
            // arg already delivers through the frame-slot arm). The hoisted
            // reads and calls must not escape a selected arm. Guarded targets
            // retain their operands for arm-local normalization and checking.
            for offset in 0..if unconditional { arguments.count() } else { 0 } {
                let argument = lowerer
                    .symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(arguments)[offset as usize];
                let rewritten = hoist_operand_indexed_reads(
                    lowerer,
                    argument,
                    hoisted,
                    OperandHoisting::Computation,
                );
                if rewritten != argument {
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .bodies
                        .expressions
                        .set_expression_handle_at_offset(arguments, offset, rewritten);
                }
            }
            Ok(TransitionTarget::Named(NamedTransitionTarget {
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
                storage: NamedTransitionTargetStorage {
                    path: lower_statement_path_members(lowerer, syntax_trees, *path),
                    path_starts_at_self: *path_starts_at_self,
                    arguments,
                    evidence_arguments: evidence_arguments
                        .iter()
                        .map(crate::lowering::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    source_span: *source_span,
                    authored_call_selection: None,
                },
            }))
        }
        syntax::statement::TransitionTargetNode::Value(expression) => {
            let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
            // Same hoist for an unconditional VALUE result (`-> (arr[i] + 5)`).
            let expression = if unconditional {
                hoist_operand_indexed_reads(
                    lowerer,
                    expression,
                    hoisted,
                    OperandHoisting::Computation,
                )
            } else {
                expression
            };
            Ok(TransitionTarget::Value(expression))
        }
        syntax::statement::TransitionTargetNode::SelfTarget => Ok(TransitionTarget::SelfTarget),
        syntax::statement::TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
    }
}

fn lower_statement_path_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.statements.identifier_path_members(members) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .statement_path_members
            .append_to_span(&mut span, crate::lowering::name::lower_name(member));
    }

    span
}
