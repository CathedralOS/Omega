use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
    TableIndexedExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableUnaryExpression,
};
use omega_symbol_resolved_trees::name::DiagnosticName;
use omega_symbol_resolved_trees::statement::{
    Assignment, Call, CallStorage, LocalData, LocalDataStorage, NamedTransitionTarget,
    NamedTransitionTargetStorage, Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_symbol_resolved_trees::types::TypeReference;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

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
) -> Result<Vec<Statement>, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
    )
}

fn lower_statement_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: &syntax::statement::StatementNode,
) -> Result<Vec<Statement>, Diagnostic> {
    match statement {
        syntax::statement::StatementNode::Assignment(assignment) => {
            let target = lower_statement_expression(lowerer, syntax_trees, assignment.target)?;
            let value = lower_statement_expression(lowerer, syntax_trees, assignment.value)?;
            let mut hoisted = Vec::new();
            let value = hoist_operand_indexed_reads(lowerer, value, &mut hoisted, false);
            hoisted.push(Statement::Assignment(Assignment { target, value }));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Call(call) => Ok(vec![Statement::Call(Call {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            target: crate::name::lower_name(&call.target),
            storage: CallStorage {
                receiver: lower_statement_path_members(lowerer, syntax_trees, call.receiver),
                receiver_starts_at_self: call.receiver_starts_at_self,
                arguments: lower_statement_expressions(lowerer, syntax_trees, call.arguments)?,
                discards_result: call.discards_result,
            },
        })]),
        syntax::statement::StatementNode::Expression(expression) => Ok(vec![Statement::Expression(
            lower_statement_expression(lowerer, syntax_trees, *expression)?,
        )]),
        syntax::statement::StatementNode::LocalData(local_data) => {
            let type_reference =
                lower_type_reference_handle(lowerer, syntax_trees, local_data.type_reference)?;
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
                },
            }));
            Ok(hoisted)
        }
        syntax::statement::StatementNode::Relax(relax) => Ok(vec![Statement::Expression(
            lower_statement_expression(lowerer, syntax_trees, relax.target)?,
        )]),
        syntax::statement::StatementNode::Transition(transition) => {
            let target = lower_transition_target_node(lowerer, syntax_trees, transition.target)?;
            let continuation = if transition.continuation.is_valid() {
                Some(lower_transition_target_node(
                    lowerer,
                    syntax_trees,
                    transition.continuation,
                )?)
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
            let mut hoisted = Vec::new();
            if let TransitionGuard::When(expression) = guard {
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
            }));
            Ok(hoisted)
        }
    }
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
            let value = hoist_child(lowerer, cast.value, hoisted, hoist_builtin_calls);
            set_expression(
                lowerer,
                expression,
                ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    domain: cast.domain,
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
                }),
            );
        }
        // `&mut <place>` / `&<place>` borrows a PLACE, not a value. An indexed
        // read inside (`&mut self.entries[0]`) is the borrow TARGET, not an
        // operand to materialize -- hoisting it into a temp would borrow the
        // temp instead and silently change aliasing. Leave the whole subtree
        // untouched.
        ExpressionNode::Mutable(_) => {}
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
            let index = hoist_child(lowerer, indexed.index, hoisted, hoist_builtin_calls);
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
    if is_runtime_indexed_read(lowerer, child) {
        // Rewrite the indexed read's OWN index first (nested `arr[other[i]]`),
        // then hoist the whole indexed read into a fresh temp.
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
        },
    }));

    let mut members = HandleSpan::empty();
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .push_name_path_member(&mut members, name);
    lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            is_self_value: false,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }))
}

fn set_expression(
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
) -> Result<omega_symbol_resolved_trees::expression::ExpressionHandle, Diagnostic> {
    lower_expression_into_table(
        syntax_trees,
        &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
        expression,
    )
}

fn lower_statement_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
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
        let expression = lower_expression_into_table(
            syntax_trees,
            &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
            *expression,
        )?;
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
    let syntax::expression::ExpressionNode::Membership(syntax_membership) = syntax_trees
        .expressions
        .expression(syntax_expression)
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
            let fresh = lowerer.next_hoist_name();
            lowerer.record_match_subject_temp(subject_key, fresh.clone());
            let name = DiagnosticName::generated(fresh);
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    type_reference: TypeReference::Unit,
                    initial_value: membership.value,
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
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
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
/// Scoped to pure-BUILTIN subjects (min/max/sqrt): those lower directly in a let
/// value, so the shared temp is a clean `let __b = min(a, b) == 3`. A runtime-
/// INDEXED subject (`arr[i] > 5 { true/false }`) is deliberately NOT handled here
/// -- it needs the indexed read hoisted INSIDE the shared temp, which miscompiled
/// in testing; it stays on the (unchanged) operand-hoist path, so its true/false
/// pair remains the pre-existing gap (task #41) rather than a regression.
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
    // types via `infer_hoist_temp_type`). A pure-builtin subject lowers directly
    // in the let value (`let __b = min(a, b) == 3`), so no inner hoist is needed.
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
                    type_reference: TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: DiagnosticName::generated("bool"),
                    },
                    initial_value: outer.left,
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
    let name_reference = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Name(TableNamePath {
            members,
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
/// operands) a hoistable pure-builtin call -- the shape the shared-subject hoist
/// handles. Runtime-INDEXED reads are intentionally excluded (see
/// `hoist_comparison_match_subject`): they need the read hoisted inside the
/// shared temp, which miscompiled, so they stay on the operand-hoist path.
fn subject_contains_hoistable(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    if is_hoistable_builtin_guard_call(lowerer, expression) {
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
) -> Result<TransitionTarget, Diagnostic> {
    match syntax_trees.statements.transition_target(target) {
        syntax::statement::TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
        } => Ok(TransitionTarget::Named(NamedTransitionTarget {
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
            storage: NamedTransitionTargetStorage {
                path: lower_statement_path_members(lowerer, syntax_trees, *path),
                path_starts_at_self: *path_starts_at_self,
                arguments: lower_statement_expressions(lowerer, syntax_trees, *arguments)?,
            },
        })),
        syntax::statement::TransitionTargetNode::Value(expression) => Ok(TransitionTarget::Value(
            lower_statement_expression(lowerer, syntax_trees, *expression)?,
        )),
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
