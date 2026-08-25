use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
    TableIndexedExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableUnaryExpression,
};
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::statement::{
    AssemblyFact, AssemblyFactKind, Assignment, Call, CallStorage, LocalData, LocalDataStorage,
    NamedTransitionTarget, NamedTransitionTargetStorage, Statement, Transition, TransitionExit,
    TransitionGuard, TransitionTarget,
};
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

/// Lowers a syntax statement to one or more symbol-resolved statements.
///
/// Most statements lower one-to-one. Assignments and local-data declarations
/// can yield EXTRA statements first: every runtime-indexed read `arr[i]` used
/// as a SUB-EXPRESSION OPERAND (a child of a binary/cast/etc., not the root of
/// the value) is hoisted into a synthetic `let __hoist_N = arr[i];` placed
/// before the rewritten statement, and the operand is replaced with a name
/// referencing that temp. The hoisted `let` is itself a root-level whole-value
/// indexed read, which already lowers natively; the rewritten parent then only
/// reads a plain local. The hoisted statements come first so later passes seed
/// and resolve the temps' symbols (locals are bound by ORDER + NAME).
pub(crate) fn lower_statement_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
    statement_index: usize,
) -> Result<Vec<Statement>, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
        statement_index,
    )
}

fn lower_statement_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: &syntax::statement::StatementNode,
    statement_index: usize,
) -> Result<Vec<Statement>, Diagnostic> {
    match statement {
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
                    psi_symbol_resolved_trees::statement::EvidenceForwarding {
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
                        target: crate::name::lower_name(target),
                        source: crate::name::lower_name(source),
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
            let value = hoist_operand_indexed_reads(lowerer, value, &mut hoisted, false);
            // A BARE ref-param member as the whole RHS (`self.c = table.con_out`)
            // is not an operand, so the rewrite above leaves it -- and the flat
            // machine-write path would read frame garbage. Hoist the root into a
            // `let`, which lowers through the pointee-deref path.
            let value = if is_reference_struct_parameter_member(lowerer, value) {
                hoist_child(lowerer, value, &mut hoisted, false)
            } else {
                value
            };
            hoisted.push(Statement::Assignment(Assignment { target, value }));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Call(call) => {
            let receiver = lower_statement_path_members(lowerer, syntax_trees, call.receiver);
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
                target: crate::name::lower_name(&call.target),
                storage: CallStorage {
                    receiver,
                    receiver_starts_at_self: call.receiver_starts_at_self,
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(crate::expression::lower_static_machine_argument)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    arguments,
                    evidence_arguments: call
                        .evidence_arguments
                        .iter()
                        .map(crate::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
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
                    psi_symbol_resolved_trees::statement::ProofOutputSelector {
                        output_field: crate::name::lower_name(&binding.output_field),
                        binding: crate::name::lower_name(&binding.binding),
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
                    name: crate::name::lower_name(&runtime_value.binding),
                    storage: LocalDataStorage {
                        type_reference: TypeReference::Unit,
                        initial_value: call,
                        is_mutable: false,
                    },
                }));
            }
            lowered.push(Statement::ProofOutputBindingStatement(
                psi_symbol_resolved_trees::statement::ProofOutputBindingStatement {
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
            // (native only; the interpreter masks it). Root left whole (`false`),
            // matching the transition-value target.
            let mut hoisted = Vec::new();
            let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
            let expression = hoist_operand_indexed_reads(lowerer, expression, &mut hoisted, false);
            // A free or direct-self value-machine call as the trailing return
            // (`state go(..) -> f64 { ..; self.sin(x) }`) hoists into the
            // let-bound spelling, exactly as the transition-value face.
            // Trailing returns are unconditional, so no guard gate applies.
            let expression = hoist_terminal_value_machine_call(lowerer, expression, &mut hoisted);
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
                hoist_operand_indexed_reads(lowerer, initial_value, &mut hoisted, false)
            } else {
                initial_value
            };
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&local_data.name),
                storage: LocalDataStorage {
                    type_reference,
                    initial_value,
                    is_mutable: local_data.is_mutable,
                },
            }));
            if let Some(local) = capturable_local {
                lowerer.current_state_locals.push(local);
            }
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Transition(transition) => {
            let mut hoisted = Vec::new();
            let mut target = lower_transition_target_node(
                lowerer,
                syntax_trees,
                transition.target,
                &mut hoisted,
            )?;
            // A free or direct-self value-machine call as the TERMINAL value
            // of an ALWAYS-guard arm (`transition { _ -> (self.sin(x + k)) }`) hoists
            // into the let-bound spelling -- the only served route. Guarded
            // arms keep the honest fence: their hoist would run the callee
            // even when the arm is not taken.
            if matches!(
                transition.guard,
                syntax::statement::TransitionGuardNode::Always
            ) {
                if let TransitionTarget::Value(expression) = target {
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
                )?;
                if matches!(
                    transition.guard,
                    syntax::statement::TransitionGuardNode::Always
                ) {
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
                        // automatic. Scoped to guards (the `true` flag): assignment
                        // and let values above already lower builtin calls directly.
                        hoist_operand_indexed_reads(lowerer, expression, &mut hoisted, true);
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
                exit: match transition.exit {
                    syntax::statement::TransitionExit::Ordinary => TransitionExit::Ordinary,
                    syntax::statement::TransitionExit::Crash(cause) => {
                        TransitionExit::Crash(match cause {
                            syntax::item::CrashCause::Trap => {
                                psi_symbol_resolved_trees::signature::CrashCause::Trap
                            }
                            syntax::item::CrashCause::Abort => {
                                psi_symbol_resolved_trees::signature::CrashCause::Abort
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

/// Hoists every runtime-indexed read in OPERAND position out of `value`.
///
/// `value` is the ROOT of an assignment value or local initializer. A root-level
/// `Indexed` is left in place (the existing whole-value copy path handles it).
/// For every OTHER node, each child that is an `Indexed` (transitively) is
/// hoisted into a fresh `let __hoist_N = <indexed>;` appended to `hoisted`, and
/// the child is replaced by a `Name` referencing that temp. Returns the
/// (possibly identical) root handle to use in the rewritten statement.
fn hoist_operand_indexed_reads(
    lowerer: &mut Lowerer,
    value: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    hoist_builtin_calls: bool,
) -> ExpressionHandle {
    // The root itself stays as-is, but rewrite its children so any nested
    // operand-position indexed read is hoisted.
    rewrite_children(lowerer, value, hoisted, hoist_builtin_calls);
    value
}

/// Rewrites the CHILDREN of `expression` in place, hoisting any child that is an
/// operand-position indexed read. Recurses so deeply nested cases work.
fn rewrite_children(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    hoist_builtin_calls: bool,
) {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    match node {
        ExpressionNode::Binary(binary) => {
            let left = hoist_child(lowerer, binary.left, hoisted, hoist_builtin_calls);
            let right = hoist_child(lowerer, binary.right, hoisted, hoist_builtin_calls);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }),
            );
        }
        ExpressionNode::Unary(unary) => {
            let operand = hoist_child(lowerer, unary.operand, hoisted, hoist_builtin_calls);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }),
            );
        }
        ExpressionNode::Cast(cast) => {
            // A §5b RECAST re-views its operand PLACE's bytes -- hoisting a
            // runtime-indexed operand into a value temp would destroy the
            // place (the view must address `buf[k]`, not a copied byte;
            // rung C1's runtime-offset judgment reads the raw Indexed
            // shape). Deeper hoistables INSIDE the index still hoist via
            // the index rewrite; only the top-level indexed READ is kept.
            let value = if cast.form.is_recast()
                && matches!(
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .bodies
                        .expressions
                        .expression(cast.value),
                    ExpressionNode::Indexed(_)
                ) {
                cast.value
            } else if !cast.form.is_recast() && is_hoistable_value_cast_call(lowerer, cast.value) {
                // A value-machine call directly beneath a value cast
                // (including erased domain qualification) must first
                // materialize through the ordinary let-bound call-result
                // route. Otherwise Cast(Call(..)) reaches native selection as
                // one compound operand and surrounding arithmetic can consume
                // the call scratch slot instead of the delivered result. This
                // is exactly the authored equivalent:
                //
                //   (convert(x) as T in Policy)
                //     -> let __hoist = convert(x);
                //        (__hoist as T in Policy)
                //
                // The callee's declared return types the synthetic local in
                // `infer_hoist_temp_type`.
                hoist_into_temp(lowerer, cast.value, hoisted)
            } else {
                hoist_child(lowerer, cast.value, hoisted, hoist_builtin_calls)
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    form: cast.form,
                }),
            );
        }
        ExpressionNode::Membership(membership) => {
            let value = hoist_child(lowerer, membership.value, hoisted, hoist_builtin_calls);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
                    case_type_symbol: membership.case_type_symbol,
                    case_symbol: membership.case_symbol,
                }),
            );
        }
        // `&mut <place>` / `&<place>` borrows a PLACE, not a value. An indexed
        // read inside (`&mut self.entries[0]`) is the borrow TARGET, not an
        // operand to materialize -- hoisting it into a temp would borrow the
        // temp instead and silently change aliasing. Leave the whole subtree
        // untouched.
        ExpressionNode::Borrow(_) => {}
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                hoist_child(lowerer, range.start, hoisted, hoist_builtin_calls)
            } else {
                range.start
            };
            let end = if range.end.is_valid() {
                hoist_child(lowerer, range.end, hoisted, hoist_builtin_calls)
            } else {
                range.end
            };
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }),
            );
        }
        ExpressionNode::Indexed(indexed) => {
            // A NON-root indexed node reached via recursion would already have
            // been hoisted by `hoist_child`; reaching here means this is the
            // value root. Leave it whole (the root whole-value copy path), but
            // still rewrite its index sub-expression so a runtime-indexed read
            // INSIDE the index (`arr[other[i]]`) is hoisted.
            let index = hoist_index(lowerer, indexed.index, hoisted, hoist_builtin_calls);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Indexed(TableIndexedExpression {
                    collection: indexed.collection,
                    index,
                }),
            );
        }
        // Array literals, calls, member accesses, struct literals, and the leaf
        // nodes (names, integers, ...) are left untouched: an operand-position
        // indexed read appears only as a direct child handled above, and these
        // forms either do not surface the blocker or are out of scope.
        _ => {}
    }
}

/// Whether a direct call beneath a non-recast value cast can use the let-bound
/// value-call result path. Pure compiler builtins keep their existing
/// expression lowering because their synthetic result type is inferred from an
/// operand rather than a declared machine return.
fn is_hoistable_value_cast_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return false;
    };
    !call.receiver.is_valid() && !matches!(call.target.as_str(), "min" | "max" | "sqrt")
}

/// Rewrites an `Indexed` node's INDEX position. A hoistable COMPUTED index
/// (`arr[k + 1]` -- see `index_is_hoistable_computed`) is hoisted into a
/// `let __hoist_N = k + 1;` temp so the access indexes a slotted plain place:
/// the checker proves the temp's bounds from its env interval, state-storage
/// keeps its slot (the runtime-index carve-out), and simplify never folds a
/// computed binding back into an index position. Anything else goes through
/// `hoist_child` unchanged (a runtime-indexed read INSIDE the index,
/// `arr[other[i]]`, still hoists there).
fn hoist_index(
    lowerer: &mut Lowerer,
    index: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    hoist_builtin_calls: bool,
) -> ExpressionHandle {
    if index_is_hoistable_computed(lowerer, index) {
        return hoist_into_temp(lowerer, index, hoisted);
    }
    hoist_child(lowerer, index, hoisted, hoist_builtin_calls)
}

/// A `Binary` index up to TWO levels deep (`k + 1`, `2 * k`, and the
/// row-major idiom `y * 4 + x`) whose leaf operands are each TYPEABLE at the
/// hoist-temp layer: an integer literal, a `self.<field>` member, or a
/// state-PARAMETER name -- with at least one non-literal (a pure-const binary
/// is left for the const fold, which resolves it to a fixed index without a
/// temp). A LOCAL leaf is refused: `infer_hoist_temp_type` resolves
/// self-fields and params only, and an untypeable temp would mint a Unit
/// layout error where the checker's computed-index fence gives a clear
/// message today. Deeper nests (`(a+b)*(c+d)` and beyond) are likewise left
/// fenced -- the interval synthesis in `computed_index_interval` composes to
/// the same depth, so hoistable-here and range-synthesizable-there stay in
/// lockstep (R0 of the dependent-types ladder: the two-level shape was
/// neither hoisted NOR fenced, and silently read ZII natively).
fn index_is_hoistable_computed(lowerer: &Lowerer, index: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(index) else {
        return false;
    };
    let leaf_is_typeable = |operand: ExpressionHandle| -> bool {
        match expressions.expression(operand) {
            ExpressionNode::Integer(_) => true,
            ExpressionNode::Member(member) => matches!(
                expressions.expression(member.receiver),
                ExpressionNode::Name(path)
                    if expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| name.as_str() == "self")
            ),
            ExpressionNode::Name(path) => {
                let members = expressions.name_path_members(path.members);
                let [only] = members else {
                    return false;
                };
                lowerer
                    .current_state_parameter_names
                    .iter()
                    .any(|name| name == only.as_str())
            }
            _ => false,
        }
    };
    let operand_is_typeable = |operand: ExpressionHandle| -> bool {
        if leaf_is_typeable(operand) {
            return true;
        }
        // One nested level: a binary of typeable LEAVES (`y * 4` inside
        // `y * 4 + x`).
        match expressions.expression(operand) {
            ExpressionNode::Binary(inner) => {
                leaf_is_typeable(inner.left) && leaf_is_typeable(inner.right)
            }
            _ => false,
        }
    };
    let contains_non_literal = |operand: ExpressionHandle| -> bool {
        match expressions.expression(operand) {
            ExpressionNode::Integer(_) => false,
            ExpressionNode::Binary(inner) => {
                !matches!(
                    expressions.expression(inner.left),
                    ExpressionNode::Integer(_)
                ) || !matches!(
                    expressions.expression(inner.right),
                    ExpressionNode::Integer(_)
                )
            }
            _ => true,
        }
    };
    if !contains_non_literal(binary.left) && !contains_non_literal(binary.right) {
        return false;
    }
    operand_is_typeable(binary.left) && operand_is_typeable(binary.right)
}

/// Hoists computed indices inside an assignment TARGET's place chain
/// (`self.arr[self.k + 1] = v`), walking `Member` receivers and `Indexed`
/// collections. Only INDEX positions are rewritten -- the place structure
/// itself is never hoisted (it is a write target, not a value).
fn hoist_target_computed_indices(
    lowerer: &mut Lowerer,
    target: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(target)
        .clone();
    match node {
        ExpressionNode::Indexed(indexed) => {
            hoist_target_computed_indices(lowerer, indexed.collection, hoisted);
            if index_is_hoistable_computed(lowerer, indexed.index) {
                let index = hoist_into_temp(lowerer, indexed.index, hoisted);
                set_expression(
                    lowerer,
                    target,
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: indexed.collection,
                        index,
                    }),
                );
            }
        }
        ExpressionNode::Member(member) => {
            hoist_target_computed_indices(lowerer, member.receiver, hoisted);
        }
        ExpressionNode::Borrow(inner) => {
            hoist_target_computed_indices(lowerer, inner.target, hoisted);
        }
        _ => {}
    }
}

/// Hoists `child` if it is an `Indexed` node, otherwise recurses into it.
///
/// Returns the handle to use in the parent: a `Name` referencing the new temp
/// when hoisted, or the original handle (with its children rewritten) otherwise.
fn hoist_child(
    lowerer: &mut Lowerer,
    child: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
    hoist_builtin_calls: bool,
) -> ExpressionHandle {
    // A member read through a shared reference-to-struct PARAM
    // (`table.con_out`) must dereference the pointer slot; left in operand or
    // guard position it folds flat (slot + field offset in the FRAME) and
    // silently reads garbage -- the entry-ref-param face. Hoisting it into a
    // `let` routes it through the boot-verified pointee-copy path. The param's
    // `&Named` type is DECLARED on the state signature, so this predicate is
    // not type-blind.
    if is_reference_struct_parameter_member(lowerer, child) {
        return hoist_into_temp(lowerer, child, hoisted);
    }

    if is_runtime_indexed_read(lowerer, child) {
        // Rewrite the indexed read's OWN index first (nested `arr[other[i]]`),
        // then hoist the whole indexed read into a fresh temp.
        rewrite_children(lowerer, child, hoisted, hoist_builtin_calls);
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // A FIELD read of a runtime-indexed ELEMENT (`cells[k].v`) is the same
    // operand shape one field deeper: unhoisted, it reaches state-values with
    // no operand lowering and blocks ("needs runtime value lowering"). Hoist
    // the WHOLE member chain -- the temp's materialization resolves the
    // element field through the machine-indexed copy (field_byte_offset), the
    // same path a transition argument uses. VALUE positions ONLY
    // (`hoist_builtin_calls` marks the guard path): a guard's comparison
    // subject is hoisted ONCE and SHARED across arms by
    // `hoist_comparison_match_subject`, and a per-arm hoist here would split
    // the subject into distinct temps, un-pairing the `true`/`false` arms
    // (exhaustiveness then reports a fall-through on working guards).
    if !hoist_builtin_calls && is_member_of_runtime_indexed_read(lowerer, child) {
        rewrite_children(lowerer, child, hoisted, hoist_builtin_calls);
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // In guard position, a pure-builtin call subject (`min(self.a, self.b)`) is
    // hoisted whole into a temp so the guard compares a materialized local. The
    // builtins are effect-free, so this never changes an effectful evaluation
    // count (unlike a general value-call hoist). Only calls whose first argument
    // is a `self.<field>` place are hoisted -- the symbol-resolved->typed lowering
    // types the temp from that field (`infer_hoist_temp_type`); a non-place first
    // argument (a nested call, a literal) is left for the "bind to a local first"
    // diagnostic, unchanged.
    if hoist_builtin_calls && is_hoistable_builtin_guard_call(lowerer, child) {
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // Not a runtime-indexed read: descend so deeper operand-position runtime
    // indexed reads (`(a + arr[i]) * b`) are still hoisted.
    rewrite_children(lowerer, child, hoisted, hoist_builtin_calls);
    child
}

/// Hoists a USER value-machine call out of a guard comparison:
/// `self.next() == expected` becomes
/// `let __hoist_N = self.next(); __hoist_N == expected`. Fires only when the
/// guard ROOT is a comparison with one user Call side and one non-user-Call
/// side (builtin min/max/sqrt calls keep their own dedicated hoist below). The
/// temp's type is resolved from the callee's DECLARED return by the
/// symbol-resolved -> typed lowering
/// (`infer_hoist_temp_type`); an inferred-return callee gets a clear
/// annotate-or-bind diagnostic there.
fn hoist_scalar_value_call_comparison(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let syntax::statement::TransitionGuardNode::When(mut syntax_cmp) = syntax_guard else {
        return;
    };
    // Peel bool-arm wrappers in LOCKSTEP on both trees: a bool-arm dispatch
    // wraps the subject per arm (`(dbl(5) == 11) == true`), and a match-over-
    // call arm arrives directly as `roll(..) == <arm literal>` with the SAME
    // syntax subject handle shared across arms.
    let mut resolved_cmp = guard_expression;
    let binary = loop {
        let node = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(resolved_cmp)
            .clone();
        let ExpressionNode::Binary(binary) = node else {
            return;
        };
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) && matches!(
            lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(binary.right),
            ExpressionNode::Boolean(_)
        ) {
            let syntax::expression::ExpressionNode::Binary(syntax_outer) =
                syntax_trees.expressions.expression(syntax_cmp)
            else {
                return;
            };
            resolved_cmp = binary.left;
            syntax_cmp = syntax_outer.left;
            continue;
        }
        break binary;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return;
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let side_is_user_call = |handle: ExpressionHandle| match expressions.expression(handle) {
        ExpressionNode::Call(call) => !matches!(call.target.as_str(), "min" | "max" | "sqrt"),
        _ => false,
    };
    let call_is_left = side_is_user_call(binary.left) && !side_is_user_call(binary.right);
    let call_is_right = side_is_user_call(binary.right) && !side_is_user_call(binary.left);
    if !call_is_left && !call_is_right {
        return;
    }
    let call_side = if call_is_left {
        binary.left
    } else {
        binary.right
    };

    // The memo key is the CALL's SYNTAX handle: a match over a call subject
    // (`transition self.roll(t) { 1 -> .. 2 -> .. }`) lowers one comparison
    // PER ARM over the SAME syntax subject, and every arm must share ONE temp
    // -- per-arm temps re-run the callee once per attempted arm (the
    // effectful-subject single-evaluation tripwire).
    let syntax_call = match syntax_trees.expressions.expression(syntax_cmp) {
        syntax::expression::ExpressionNode::Binary(syntax_binary) => {
            if call_is_left {
                syntax_binary.left
            } else {
                syntax_binary.right
            }
        }
        // A match-over-call arm whose SYNTAX guard is the bare subject (the
        // arm value is synthesized): the subject itself is the call.
        _ => syntax_cmp,
    };
    let subject_key = syntax_call.arena_index();

    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    // Unit is the inference sentinel; the symbol-resolved ->
                    // typed lowering types the temp from the callee's DECLARED
                    // return (`infer_hoist_temp_type`'s Call branch).
                    type_reference: TypeReference::Unit,
                    initial_value: call_side,
                    is_mutable: false,
                },
            }));
            name
        }
    };

    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    let member_symbols = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_name_path_member_symbols(members.count());
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
    let rewritten = TableBinaryExpression {
        left: if call_is_left {
            name_reference
        } else {
            binary.left
        },
        operator: binary.operator,
        right: if call_is_right {
            name_reference
        } else {
            binary.right
        },
    };
    set_expression(lowerer, resolved_cmp, ExpressionNode::Binary(rewritten));
}

/// A free or direct-self value-machine call (`sin(x)` or `self.finish()`; not a
/// pure builtin) as a transition's TERMINAL VALUE (or a state's trailing
/// implicit return) has no dispatch return route: an acyclic callee poisons at the
/// unlowered-terminal fence and a cyclic one refuses at the
/// binding-substitution depth cap (neither ever lowered -- probed 2026-07-11,
/// so this rewrite cannot regress a served shape). The let-bound spelling is
/// the fully served path, so make it automatic: hoist the call into a
/// `let __hoist_N` temp (typed from the callee's DECLARED return by
/// `infer_hoist_temp_type`'s Call branch) and deliver the local. Callers gate
/// transition targets to ALWAYS-guard arms: a hoisted statement runs whenever
/// control reaches it, and hoisting out of a guarded arm would run an
/// effectful callee even when the arm is not taken (trailing returns are
/// unconditional, so they hoist unconditionally).
fn hoist_terminal_value_machine_call(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return expression;
    };
    if matches!(call.target.as_str(), "min" | "max" | "sqrt") {
        return expression;
    }
    if call.receiver.is_valid() {
        let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
            return expression;
        };
        if !matches!(expressions.name_path_members(path.members), [member] if member.as_str() == "self")
        {
            // Contained/dynamic/proof receivers already have their own return
            // routes, and their selected requirement is not a concrete state
            // whose declared result `infer_hoist_temp_type` can copy. Only a
            // direct `self.machine()` sibling call uses this normalization.
            return expression;
        }
    }
    let name = DiagnosticName::generated(lowerer.next_hoist_name());
    hoisted.push(Statement::LocalData(LocalData {
        symbol: SymbolHandle::invalid(),
        name: name.clone(),
        storage: LocalDataStorage {
            // Unit is the inference sentinel; the symbol-resolved -> typed
            // lowering types the temp from the callee's declared return.
            type_reference: TypeReference::Unit,
            initial_value: expression,
            is_mutable: false,
        },
    }));
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, name);
    let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        is_self_value: false,
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
    }))
}

/// GUARDED-ARM VALUE-CALL REWRITE (task #45): when a guarded (or
/// continuation) arm's target is a free user value-machine call whose every
/// argument is a bare NAME of an enclosing state parameter, rewrite the arm
/// to a Named target on a synthesized continuation state (recorded on the
/// lowerer; the machine lowering appends it after the authored states). The
/// synthesized state re-declares the SAME-named parameters, so the original
/// call expression's Names resolve against them verbatim. Anything outside
/// the gate returns unchanged and keeps the honest backend fence.
fn rewrite_guarded_call_arm(lowerer: &mut Lowerer, target: TransitionTarget) -> TransitionTarget {
    let TransitionTarget::Value(expression) = target else {
        return target;
    };
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return TransitionTarget::Value(expression);
    };
    if call.receiver.is_valid() || matches!(call.target.as_str(), "min" | "max" | "sqrt") {
        return TransitionTarget::Value(expression);
    }
    let argument_handles = expressions.expression_handles(call.arguments).to_vec();
    let mut parameters: Vec<(String, TypeReference)> = Vec::new();
    for argument in &argument_handles {
        let ExpressionNode::Name(path) = expressions.expression(*argument) else {
            return TransitionTarget::Value(expression);
        };
        let members = expressions.name_path_members(path.members);
        let [single] = members else {
            return TransitionTarget::Value(expression);
        };
        let Some((name, type_reference, _)) = lowerer
            .current_state_parameters
            .iter()
            .find(|(name, _, _)| name == single.as_str())
        else {
            return TransitionTarget::Value(expression);
        };
        // Dedup: `call(x, x)` declares ONE parameter x; both body Names
        // resolve to it, and the target passes it once.
        if !parameters.iter().any(|(existing, _)| existing == name) {
            parameters.push((name.clone(), type_reference.clone()));
        }
    }
    let Some(return_type) = lowerer.current_state_return_type.clone() else {
        return TransitionTarget::Value(expression);
    };
    let state_name = lowerer.next_arm_state_name();
    lowerer
        .pending_synthesized_states
        .push(crate::lowerer::SynthesizedArmState {
            name: state_name.clone(),
            parameters: parameters.clone(),
            return_type,
            call: expression,
        });
    let mut path = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .statement_path_members
        .append_to_span(&mut path, DiagnosticName::generated(state_name));
    // FRESH Name nodes for the target's arguments: the original handles
    // stay inside the synthesized state's call (where they resolve against
    // ITS parameters); sharing one node across two scopes would let the
    // second resolution overwrite the first's symbol.
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut arguments = HandleSpan::empty();
    for (name, _) in &parameters {
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, DiagnosticName::generated(name.clone()));
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        let fresh = expressions.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
        expressions.push_expression_handle(&mut arguments, fresh);
    }
    TransitionTarget::Named(NamedTransitionTarget {
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
        storage: NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments,
            evidence_arguments: Box::default(),
        },
    })
}

/// Move direct free/`self` value calls used as guarded named-target arguments
/// behind the selected arm. The generated state receives the source state's
/// referenced parameters, evaluates calls left-to-right into locals, then
/// performs the original transition. Unsupported captures (notably source
/// locals whose types are not retained here) keep the existing honest fence.
fn rewrite_guarded_transition_argument_calls(
    lowerer: &mut Lowerer,
    target: TransitionTarget,
) -> TransitionTarget {
    let TransitionTarget::Named(named) = target else {
        return target;
    };
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let argument_handles = expressions.expression_handles(named.storage.arguments);
    let mut calls = Vec::new();
    for argument in argument_handles {
        collect_synthesizable_argument_calls(lowerer, *argument, &mut calls);
    }
    if calls.is_empty() {
        return TransitionTarget::Named(named);
    }
    // Evidence identifiers are scoped to the authored source state. Moving
    // this edge behind a synthesized runtime-argument state would orphan a
    // state-arrival term, so keep the exact edge intact and let the existing
    // downstream call-in-argument fence decide whether its runtime shape is
    // supported.
    if !named.evidence_arguments.is_empty() {
        return TransitionTarget::Named(named);
    }

    let mut captured_names = Vec::new();
    let mut uses_self = false;
    if !argument_handles.iter().all(|argument| {
        collect_synthesized_argument_captures(
            lowerer,
            *argument,
            &mut captured_names,
            &mut uses_self,
        )
    }) {
        return TransitionTarget::Named(named);
    }
    let self_parameter = uses_self
        .then(|| lowerer.current_state_self_parameter.clone())
        .flatten();
    if uses_self && self_parameter.is_none() {
        return TransitionTarget::Named(named);
    }
    let parameters = lowerer
        .current_state_parameters
        .iter()
        .filter(|(name, _, _)| captured_names.contains(name))
        .cloned()
        .chain(
            lowerer
                .current_state_locals
                .iter()
                .filter(|(name, _, _)| captured_names.contains(name))
                .cloned(),
        )
        .collect::<Vec<_>>();
    if parameters.len() != captured_names.len() {
        return TransitionTarget::Named(named);
    }

    let state_name = lowerer.next_arm_state_name();
    lowerer.pending_synthesized_transition_argument_states.push(
        crate::lowerer::SynthesizedTransitionArgumentState {
            name: state_name.clone(),
            self_parameter,
            parameters: parameters.clone(),
            return_type: lowerer.current_state_return_type.clone(),
            target: named,
            calls,
        },
    );

    let mut path = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .statement_path_members
        .append_to_span(&mut path, DiagnosticName::generated(state_name));
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut arguments = HandleSpan::empty();
    for (name, _, _) in &parameters {
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, DiagnosticName::generated(name.clone()));
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        let fresh = expressions.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
        expressions.push_expression_handle(&mut arguments, fresh);
    }
    TransitionTarget::Named(NamedTransitionTarget {
        head_symbol: SymbolHandle::invalid(),
        symbol: SymbolHandle::invalid(),
        storage: NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments,
            evidence_arguments: Box::default(),
        },
    })
}

/// Collect direct free/`self` machine calls in evaluation order. Children are
/// visited first so a nested call result is materialized before its enclosing
/// call, and sibling operands retain left-to-right order.
fn collect_synthesizable_argument_calls(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    calls: &mut Vec<ExpressionHandle>,
) {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut visit = |child| collect_synthesizable_argument_calls(lowerer, child, calls);
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => visit(atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in expressions.expression_handles(*values) {
                visit(*value);
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left);
            visit(binary.right);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                visit(call.receiver);
            }
            for argument in expressions.expression_handles(call.arguments) {
                visit(*argument);
            }
            if matches!(call.target.as_str(), "min" | "max" | "sqrt") {
                return;
            }
            if call.receiver.is_valid() {
                let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
                    return;
                };
                if !matches!(expressions.name_path_members(path.members), [member] if member.as_str() == "self")
                {
                    return;
                }
            }
            calls.push(expression);
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection);
            visit(indexed.index);
        }
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::Membership(membership) => visit(membership.value),
        ExpressionNode::Borrow(inner) => visit(inner.target),
        ExpressionNode::Range(range) => {
            visit(range.start);
            visit(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in expressions.struct_fields(literal.fields) {
                visit(field.value);
            }
        }
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn collect_synthesized_argument_captures(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    captured_names: &mut Vec<String>,
    uses_self: &mut bool,
) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut visit =
        |child| collect_synthesized_argument_captures(lowerer, child, captured_names, uses_self);
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => visit(atomic.value),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .all(|value| visit(*value)),
        ExpressionNode::Binary(binary) => visit(binary.left) && visit(binary.right),
        ExpressionNode::Call(call) => {
            (!call.receiver.is_valid() || visit(call.receiver))
                && expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| visit(*argument))
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) && visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::Membership(membership) => visit(membership.value),
        ExpressionNode::Borrow(inner) => visit(inner.target),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            if members
                .first()
                .is_some_and(|member| member.as_str() == "self")
            {
                *uses_self = true;
                return true;
            }
            let [name] = members else {
                return false;
            };
            let name = name.as_str();
            if !lowerer
                .current_state_parameters
                .iter()
                .any(|(parameter, _, _)| parameter == name)
                && !lowerer
                    .current_state_locals
                    .iter()
                    .any(|(local, _, _)| local == name)
            {
                return false;
            }
            if !captured_names.iter().any(|captured| captured == name) {
                captured_names.push(name.to_owned());
            }
            true
        }
        ExpressionNode::Range(range) => visit(range.start) && visit(range.end),
        ExpressionNode::StructLiteral(literal) => expressions
            .struct_fields(literal.fields)
            .iter()
            .all(|field| visit(field.value)),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
    }
}

/// Whether `expression` is a pure-builtin call (`min`/`max`/`sqrt`; `abs`/`clamp`
/// are already desugared to these) that Phase-1 guard hoisting materializes: a
/// free call (no receiver) whose FIRST argument is a `self.<field>` place, so the
/// synthetic temp's type is resolvable from that field. `abs(self.x)` desugars to
/// `max(self.x, 0 - self.x)` (first arg `self.x`, hoisted); `clamp(self.x, ..)`
/// desugars to `min(max(self.x, ..), ..)` whose first arg is a call, so it is
/// left alone (not hoisted) -- the temp would be untypeable.
fn is_hoistable_builtin_guard_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return false;
    };
    if call.receiver.is_valid() {
        return false; // a method call, not a free builtin
    }
    if !matches!(call.target.as_str(), "min" | "max" | "sqrt") {
        return false;
    }
    let arguments = expressions.expression_handles(call.arguments);
    let Some(&first) = arguments.first() else {
        return false;
    };
    // The first argument must be a `self.<field>` member access -- the only place
    // shape `infer_hoist_temp_type` can type the temp from.
    let ExpressionNode::Member(member) = expressions.expression(first) else {
        return false;
    };
    matches!(
        expressions.expression(member.receiver),
        ExpressionNode::Name(path)
            if expressions
                .name_path_members(path.members)
                .first()
                .is_some_and(|name| name.as_str() == "self")
    )
}

/// Whether `expression` is an `Indexed` read whose index is NOT a constant
/// integer -- the RUNTIME-indexed case that needs operand hoisting. A
/// constant-index read (`arr[0]`) lowers as a plain place path and is left
/// alone (so existing whole-value copies / borrows are untouched).
/// Whether `expression` is a MEMBER chain whose receiver bottoms out at a
/// runtime-indexed read of a MACHINE-owned collection (`self.cells[k].v`).
/// Restricted to `self.<field>` collections because the hoist temp's type is
/// inferred from the machine's attached data (`infer_hoist_temp_type`); a
/// LOCAL array's element field would mint an untypeable Unit temp AND break
/// the local-array RMW write path that pattern-matches the unhoisted read.
fn is_member_of_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return false;
    };
    let mut receiver = member.receiver;
    loop {
        match expressions.expression(receiver) {
            ExpressionNode::Member(inner) => receiver = inner.receiver,
            _ => break,
        }
    }
    if !is_runtime_indexed_read(lowerer, receiver) {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = expressions.expression(receiver) else {
        return false;
    };
    // The collection must be a `self.<field>` place (typeable from attached data).
    match expressions.expression(indexed.collection) {
        ExpressionNode::Member(collection_member) => matches!(
            expressions.expression(collection_member.receiver),
            ExpressionNode::Name(path)
                if expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name.as_str() == "self")
        ),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            members.len() == 2 && members[0].as_str() == "self"
        }
        _ => false,
    }
}

fn is_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Indexed(indexed) = expressions.expression(expression) else {
        return false;
    };
    !matches!(
        expressions.expression(indexed.index),
        ExpressionNode::Integer(_)
    )
}

/// Emits `let __hoist_N = <indexed_value>;` and returns a `Name` referencing it.
fn hoist_into_temp(
    lowerer: &mut Lowerer,
    indexed_value: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let name = DiagnosticName::generated(lowerer.next_hoist_name());

    hoisted.push(Statement::LocalData(LocalData {
        symbol: SymbolHandle::invalid(),
        name: name.clone(),
        storage: LocalDataStorage {
            // No annotation here. The element type (with its arithmetic domain)
            // is filled in by the symbol-resolved -> typed lowering, which has
            // the resolved data-field types available
            // (`statement::infer_hoist_temp_type`). Until then it is `Unit`,
            // the inference sentinel.
            type_reference: TypeReference::Unit,
            initial_value: indexed_value,
            is_mutable: false,
        },
    }));

    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    let member_symbols = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_name_path_member_symbols(members.count());
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }))
}

fn set_expression(lowerer: &mut Lowerer, handle: ExpressionHandle, node: ExpressionNode) {
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
) -> Result<psi_symbol_resolved_trees::expression::ExpressionHandle, Diagnostic> {
    lower_expression_into_table(lowerer, syntax_trees, expression)
}

fn lower_statement_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<psi_symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
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
        let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
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

/// Hoists a runtime-indexed ENUM-VARIANT MATCH subject into a SINGLE shared temp so every arm of
/// the match tests one plain local. A match `transition self.grid[self.i] { Cell::Wall -> .. }`
/// lowers (in the parser) to one `When(subject is Variant)` -- an `ExpressionNode::Membership` --
/// per arm, and every arm's Membership references the SAME syntax subject handle. Naively hoisting
/// each arm's subject (as the comparison hoist does for `Binary` guards) would mint a DISTINCT temp
/// per arm, and the exhaustiveness checker -- which groups the arms by a shared subject -- would then
/// report "match does not cover Variant". Instead this keys a memo on the shared syntax subject
/// handle: the FIRST arm mints `let __hoist_N = self.grid[self.i]` and records the name; the siblings
/// reuse it. All arms end up testing `__hoist_N`, so exhaustiveness still groups them, and the temp
/// is a plain local with a correct offset (a raw runtime-indexed guard subject silently reads
/// element 0). Const-index / field / string-slice subjects are not runtime-indexed and are left
/// untouched.
fn hoist_membership_match_subject(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    // The LOWERED guard must be a Membership over a runtime-indexed subject.
    let ExpressionNode::Membership(membership) = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(guard_expression)
        .clone()
    else {
        return;
    };
    if !is_runtime_indexed_read(lowerer, membership.value) {
        return;
    }
    // The SYNTAX guard carries the subject handle shared across every arm of this match.
    let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
        return;
    };
    let syntax::expression::ExpressionNode::Membership(syntax_membership) =
        syntax_trees.expressions.expression(syntax_expression)
    else {
        return;
    };
    let subject_key = syntax_membership.value.arena_index();

    // Reuse the sibling arm's temp if the first arm already minted one; otherwise mint it here and
    // emit the single `let __hoist_N = <subject>;` (reusing this arm's lowered indexed read as the
    // initializer -- later arms' lowered reads are simply left orphaned).
    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            // The subject's INDEX may itself be a hoistable COMPUTED
            // expression (`grid[r * 6 + c]` as a match subject -- the
            // dungeon_render shape): hoist it into its own temp FIRST,
            // exactly as operand positions do, so the emitted subject let
            // indexes a slotted plain place (otherwise the #40 computed-
            // index fence refuses the initializer). Only on the minting
            // arm: sibling arms' lowered reads are orphaned anyway.
            if let ExpressionNode::Indexed(indexed) = lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(membership.value)
                .clone()
            {
                let index = hoist_index(lowerer, indexed.index, hoisted, false);
                set_expression(
                    lowerer,
                    membership.value,
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: indexed.collection,
                        index,
                    }),
                );
            }
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: TypeReference::Unit,
                    initial_value: membership.value,
                    is_mutable: false,
                },
            }));
            name
        }
    };

    // Rewrite this arm's Membership to test the shared temp instead of the raw indexed read.
    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    let member_symbols = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_name_path_member_symbols(members.count());
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
    set_expression(
        lowerer,
        guard_expression,
        ExpressionNode::Membership(TableMembershipExpression {
            value: name_reference,
            domain: membership.domain,
            domain_symbol: membership.domain_symbol,
            case_type_symbol: membership.case_type_symbol,
            case_symbol: membership.case_symbol,
        }),
    );
}

/// Hoists a bool-arm dispatch SUBJECT that contains a hoistable read/call into a
/// SINGLE shared temp, so a `{ true -> .. false -> .. }` pair still shares one
/// subject and exhaustiveness pairs the arms.
///
/// A `transition min(self.a, self.b) == 3 { true -> A false -> B }` lowers (per
/// arm) to the wrapper `(min(self.a, self.b) == 3) == <bool>`. The operand hoist
/// would pull `min(self.a, self.b)` into a DISTINCT temp per arm (each arm
/// re-lowers the subject to its own handle), so the two arms no longer test a
/// structurally-equal subject and the dispatch is rejected as non-exhaustive
/// (`arr[i] > 5 { true/false }` fails identically -- this is not builtin-specific).
/// Instead this keys the shared `match_subject_temps` memo on the SYNTAX subject
/// handle (the parser reuses `subject[0]` across every arm's guard, guards.rs):
/// the first arm mints `let __hoist_N: bool = <subject>` and the siblings reuse
/// it, so all arms test one local. Only fires when the subject is a COMPARISON
/// CONTAINING a builtin/indexed read (exactly the shapes the operand hoist would
/// otherwise break); a bare-place subject (`self.flag`, `self.a > self.b`) is
/// structurally equal across arms already and is left untouched. Returns whether
/// it hoisted, so the caller skips the operand hoist.
///
/// Handles both a pure-BUILTIN subject (min/max/sqrt, which lowers directly in the
/// let value) and a runtime-INDEXED subject (`arr[i] > 5 { true/false }`, whose
/// read is hoisted INSIDE the shared temp -- `let __t = arr[i]; let __b = __t >
/// 5`; `__t` keeps its slot as a compare operand, so it reads correctly).
fn hoist_comparison_match_subject(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> bool {
    // The LOWERED guard must be the bool-arm wrapper `SUBJECT ==/!= <bool>`.
    let ExpressionNode::Binary(outer) = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(guard_expression)
        .clone()
    else {
        return false;
    };
    if !matches!(
        outer.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return false;
    }
    if !matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(outer.right),
        ExpressionNode::Boolean(_)
    ) {
        return false;
    }
    // The SUBJECT must be a comparison that CONTAINS a hoistable read/call.
    let subject_is_comparison = matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(outer.left),
        ExpressionNode::Binary(inner) if is_comparison_operator(inner.operator)
    );
    if !subject_is_comparison || !subject_contains_hoistable(lowerer, outer.left) {
        return false;
    }

    // The SHARED syntax subject handle: `subject[0]`, the left of every arm's
    // wrapper (guards.rs builds `Binary { left: subject[0], ==, right: arm }`).
    let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
        return false;
    };
    let syntax::expression::ExpressionNode::Binary(syntax_outer) =
        syntax_trees.expressions.expression(syntax_expression)
    else {
        return false;
    };
    let subject_key = syntax_outer.left.arena_index();

    // Reuse the sibling arm's temp, or mint `let __hoist_N: bool = <subject>`.
    // The subject is a comparison, so the temp is `bool` -- known here (unlike the
    // indexed/builtin operand hoists, whose element type needs the resolved field
    // types via `infer_hoist_temp_type`). A pure-builtin subject lowers directly in
    // the let value (`let __b = min(a, b) == 3`); a runtime-INDEXED subject needs
    // its read hoisted INSIDE the temp first (`let __t = arr[i]; let __b = __t >
    // 5`), which the let-value operand hoist does -- `__t` (an indexed-read local
    // used as a compare operand) keeps its slot, so `__t > 5` reads correctly.
    let name = match lowerer.match_subject_temp(subject_key) {
        Some(existing) => DiagnosticName::generated(existing),
        None => {
            hoist_operand_indexed_reads(lowerer, outer.left, hoisted, false);
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: DiagnosticName::generated("bool"),
                    },
                    initial_value: outer.left,
                    is_mutable: false,
                },
            }));
            name
        }
    };

    // Rewrite the guard to test the shared temp: `__hoist_N ==/!= <bool>`.
    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    let member_symbols = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_name_path_member_symbols(members.count());
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
    set_expression(
        lowerer,
        guard_expression,
        ExpressionNode::Binary(TableBinaryExpression {
            left: name_reference,
            operator: outer.operator,
            right: outer.right,
        }),
    );
    true
}

fn is_comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

/// Whether `expression` contains (transitively through comparison/arith/cast
/// operands) a hoistable pure-builtin call OR a runtime-indexed read -- the shapes
/// the shared-subject hoist handles (both would otherwise be pulled into per-arm
/// temps by the operand hoist, breaking the true/false pairing).
/// Whether `expression` is a member read through a SHARED reference-to-struct
/// parameter of the current state (`table.con_out` with `table:
/// &EfiSystemTable`) -- the shape whose flat fold reads frame garbage. The
/// receiver must be a bare single-segment name matching one of the recorded
/// `&Named` params.
fn is_reference_struct_parameter_member(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    if lowerer.reference_struct_parameters.is_empty() {
        return false;
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return false;
    };
    let ExpressionNode::Name(path) = expressions.expression(member.receiver) else {
        return false;
    };
    let members = expressions.name_path_members(path.members);
    let [only] = members else {
        return false;
    };
    lowerer
        .reference_struct_parameters
        .iter()
        .any(|name| name == only.as_str())
}

fn subject_contains_hoistable(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    if is_hoistable_builtin_guard_call(lowerer, expression)
        || is_runtime_indexed_read(lowerer, expression)
        || is_reference_struct_parameter_member(lowerer, expression)
    {
        return true;
    }
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    match node {
        ExpressionNode::Binary(binary) => {
            subject_contains_hoistable(lowerer, binary.left)
                || subject_contains_hoistable(lowerer, binary.right)
        }
        ExpressionNode::Unary(unary) => subject_contains_hoistable(lowerer, unary.operand),
        ExpressionNode::Cast(cast) => subject_contains_hoistable(lowerer, cast.value),
        _ => false,
    }
}

/// Whether a `When` guard is a COMPARISON/boolean guard whose runtime-indexed operands should be
/// hoisted. A match arm lowers to a `When(subject is Variant)` -- an `ExpressionNode::Membership`
/// root -- and hoisting each arm's subject into a distinct temp would break match exhaustiveness
/// (all arms of one match must share a single subject). Only a `Binary` root (`arr[i] > 5`,
/// `arr[i] == 66`, `a && b`) is hoisted; membership/pattern guards are left for the separate
/// shared-subject match rewrite.
fn guard_hoists_operands(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    matches!(
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(expression),
        ExpressionNode::Binary(_)
    )
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
) -> Result<TransitionTarget, Diagnostic> {
    match syntax_trees.statements.transition_target(target) {
        syntax::statement::TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
            evidence_arguments,
        } => {
            let arguments = lower_statement_expressions(lowerer, syntax_trees, *arguments)?;
            // A runtime-indexed read in OPERAND position inside a transition
            // ARGUMENT (`-> dot(.., acc + a[i][k] * b[k][j])`) has no value
            // operand and SILENTLY read 0 (native; the interpreter was right)
            // -- the same gap the assignment-value/let/guard hoists close.
            // Hoist each argument's operand-position indexed reads into
            // `let __hoist_N` temps (the root is left whole: a BARE indexed
            // arg already delivers through the frame-slot arm). The hoisted
            // reads are pure loads, so evaluating them before the guard --
            // even for a not-taken arm -- has no observable effect.
            for offset in 0..arguments.count() {
                let argument = lowerer
                    .symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(arguments)[offset as usize];
                let rewritten = hoist_operand_indexed_reads(lowerer, argument, hoisted, false);
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
                        .map(crate::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            }))
        }
        syntax::statement::TransitionTargetNode::Value(expression) => {
            let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
            // Same hoist for a VALUE result (`-> (arr[i] + 5)`).
            let expression = hoist_operand_indexed_reads(lowerer, expression, hoisted, false);
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
            .append_to_span(&mut span, crate::name::lower_name(member));
    }

    span
}
