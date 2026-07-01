use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCastExpression,
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
            let value = hoist_operand_indexed_reads(lowerer, value, &mut hoisted);
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
                hoist_operand_indexed_reads(lowerer, initial_value, &mut hoisted)
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
                    hoist_operand_indexed_reads(lowerer, expression, &mut hoisted);
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
) -> ExpressionHandle {
    // The root itself stays as-is, but rewrite its children so any nested
    // operand-position indexed read is hoisted.
    rewrite_children(lowerer, value, hoisted);
    value
}

/// Rewrites the CHILDREN of `expression` in place, hoisting any child that is an
/// operand-position indexed read. Recurses so deeply nested cases work.
fn rewrite_children(
    lowerer: &mut Lowerer,
    expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
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
            let left = hoist_child(lowerer, binary.left, hoisted);
            let right = hoist_child(lowerer, binary.right, hoisted);
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
            let operand = hoist_child(lowerer, unary.operand, hoisted);
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
            let value = hoist_child(lowerer, cast.value, hoisted);
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
            let value = hoist_child(lowerer, membership.value, hoisted);
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
                hoist_child(lowerer, range.start, hoisted)
            } else {
                range.start
            };
            let end = if range.end.is_valid() {
                hoist_child(lowerer, range.end, hoisted)
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
            let index = hoist_child(lowerer, indexed.index, hoisted);
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
) -> ExpressionHandle {
    if is_runtime_indexed_read(lowerer, child) {
        // Rewrite the indexed read's OWN index first (nested `arr[other[i]]`),
        // then hoist the whole indexed read into a fresh temp.
        rewrite_children(lowerer, child, hoisted);
        return hoist_into_temp(lowerer, child, hoisted);
    }

    // Not a runtime-indexed read: descend so deeper operand-position runtime
    // indexed reads (`(a + arr[i]) * b`) are still hoisted.
    rewrite_children(lowerer, child, hoisted);
    child
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
