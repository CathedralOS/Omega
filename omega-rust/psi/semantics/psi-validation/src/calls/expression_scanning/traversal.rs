//! Recursive statement and expression traversal for value-position calls.
//!
//! This owner preserves source-order diagnostics while finding value calls,
//! malformed bare names, and unsupported nested indexed reads. Per-call target,
//! argument, and bound validation remains in the parent module.

use super::validate_expression_call_bounds;
use crate::arithmetic_domains::ValueEnv;
use crate::locals::WritableRoots;
use crate::struct_literals::data_declares_field;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// FROZEN DECISION 13 residue (value-position complement of `validate_call_node`).
///
/// Walk every expression in every statement of `state` and enforce
/// machine-call type-parameter bounds for VALUE-position calls
/// (`let r = self.pick(&self.h)`).  These never reach `validate_call_node`
/// because they appear as `ExpressionNode::Call` inside expression trees,
/// not as top-level `StatementNode::Call` nodes.
///
/// Scope: covers all expression positions that feed into statements
/// (LocalData initializers, assignment values/targets, guard expressions,
/// transition arguments, terminal expressions) and recurses into nested
/// call arguments.  Enforces the type-parameter BOUND check plus, for a
/// RESOLVED callee, argument arity (`report_argument_count_mismatch`) and the
/// per-argument class/narrowing/nominal checks (`validate_value_call_argument_classes`).
/// The remaining frontier is the UNRESOLVED callee: a value call whose target
/// no branch resolves falls through silently (the nonexistent-value-call gap),
/// which needs the complete value-call target resolver.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_position_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                assignment.value,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                assignment.target,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            // Statement-position call arguments may themselves be value calls.
            for argument in program.statement_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *expression,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            crate::locals::StateValueScope {
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                prior_statements: writable_roots.statements,
                context: "local type bound",
            }
            .type_reference(local_data.type_reference, diagnostics);
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                local_data.initial_value,
                boundary_operator_applications,
                diagnostics,
            );
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    guard,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                let target = program.statement_table.transition_target(target_handle);
                match target {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            scan_expression_calls(
                                program,
                                machine,
                                state,
                                machine_symbols,
                                symbols,
                                writable_roots,
                                value_env,
                                *argument,
                                boundary_operator_applications,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        scan_expression_calls(
                            program,
                            machine,
                            state,
                            machine_symbols,
                            symbols,
                            writable_roots,
                            value_env,
                            *expression,
                            boundary_operator_applications,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

/// Reject READING an array element with a RUNTIME SCALAR index whose collection is
/// itself reached THROUGH a RUNTIME array index -- `grid[i][j]` with BOTH indices
/// runtime. The lowering op carries ONE runtime index, so a runtime index above
/// another runtime index has no computable offset: the backend would resolve it to
/// the collection BASE and SILENTLY READ 0 instead of the element. Lowerable shapes
/// are untouched: any CONST index level (`grid[i][0]` -- suffix walk; `grid[1][j]`,
/// `rows[2].data[j]` -- const levels fold into the collection resolution's fixed
/// offset, landed 2026-07-07), a single index over a non-indexed base (`arr[i]`,
/// `self.field[j]`), and a RANGE index (`sub[1..][1..]`, a nested subslice). The
/// real fix for both-runtime is a two-index lowering op (backend feature).
fn report_nested_runtime_indexed_read(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return;
    };
    // The index must be a RUNTIME SCALAR: not a constant integer (a fixed offset,
    // lowerable) and not a RANGE (`sub[1..][1..]` is a nested SUBSLICE that lowers).
    let mut index = indexed.index;
    while let ExpressionNode::Borrow(inner) = program.expression_table.expression(index) {
        index = inner.target;
    }
    if matches!(
        program.expression_table.expression(index),
        ExpressionNode::Integer(_) | ExpressionNode::Range(_)
    ) {
        return;
    }
    // The collection's place chain (through Member receivers and Mutable) must reach
    // a RUNTIME array index -- `grid[c][j]` with BOTH indices runtime. A base with no
    // index in its chain (`arr`, `self.field`) is a plain place whose element offset
    // IS computable; a CONST-indexed base (`grid[1][j]`, `rows[2].data[j]`) folds into
    // the machine-owned collection resolution's fixed offset and lowers (2026-07-07:
    // walk-boundary threading in instruction selection + the root-element-index
    // ordering fix in resolve_machine_owned_collection_in_table; value-validated
    // differential probes). Only a runtime index ABOVE another RUNTIME index remains
    // unlowerable -- the op carries one runtime index.
    let mut collection = indexed.collection;
    let base_is_runtime_indexed = loop {
        match program.expression_table.expression(collection) {
            ExpressionNode::Indexed(inner) => {
                let mut inner_index = inner.index;
                while let ExpressionNode::Borrow(next) =
                    program.expression_table.expression(inner_index)
                {
                    inner_index = next.target;
                }
                if !matches!(
                    program.expression_table.expression(inner_index),
                    ExpressionNode::Integer(_) | ExpressionNode::Range(_)
                ) {
                    break true;
                }
                collection = inner.collection;
            }
            ExpressionNode::Member(member) => collection = member.receiver,
            ExpressionNode::Borrow(inner) => collection = inner.target,
            _ => break false,
        }
    };
    if !base_is_runtime_indexed {
        return;
    }
    // The DIRECTLY-nested two-level machine-owned read (`self.grid[i][j]`,
    // both indices runtime, the base a `self` field behind only const-indexed
    // /member links) lowers since 2026-07-07 via the double-indexed copy op
    // (resolve_runtime_machine_double_indexed_source_in_table -- this
    // predicate mirrors its shape gates exactly). Everything else stays
    // fenced: 3+ runtime levels, a member BETWEEN the two indices
    // (`rows[i].data[j]`), and frame/local/param collections. Faces the op's
    // consumers do not cover (writes, member suffixes above the element,
    // oversized elements) reject LOUDLY downstream -- the write classifier
    // refuses the fast path and the storage blockers report unlowered
    // statements, so nothing falls back to the legacy silent paths.
    if double_indexed_machine_read_is_lowerable(program, indexed) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` reads an array element with a runtime index whose base is itself \
         indexed by another RUNTIME value (`grid[i][j]`, both indices runtime), which the backend \
         cannot lower yet -- it silently reads 0 instead of the element. Make one index a constant, \
         or use a flat array with a computed index",
        machine.name.as_str(),
        state.name.as_str(),
    )));
}

/// Whether a both-runtime nested read matches the DOUBLE-INDEXED lowering's
/// shape: `<self-field base>[i][j]` -- the outer collection (Mutable-peeled)
/// is DIRECTLY another `Indexed` with a runtime index, and that inner
/// collection's chain reaches a `self` member through only CONST-indexed /
/// member / mutable links (the const-prefix peel folds those). Mirrors
/// `resolve_runtime_machine_double_indexed_source_in_table`.
fn double_indexed_machine_read_is_lowerable(
    program: &TypedTrees,
    indexed: &psi_typed_trees::expression::TableIndexedExpression,
) -> bool {
    // A member chain BETWEEN the two indices (`rows[i].data[j]`) is a fixed
    // offset the resolver folds into the op's field_byte_offset (2026-07-07),
    // so peel Member links too.
    let mut inner = indexed.collection;
    loop {
        match program.expression_table.expression(inner) {
            ExpressionNode::Borrow(next) => inner = next.target,
            ExpressionNode::Member(member) => inner = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(inner_indexed) = program.expression_table.expression(inner) else {
        return false;
    };
    let mut inner_index = inner_indexed.index;
    while let ExpressionNode::Borrow(next) = program.expression_table.expression(inner_index) {
        inner_index = next.target;
    }
    if matches!(
        program.expression_table.expression(inner_index),
        ExpressionNode::Integer(_) | ExpressionNode::Range(_)
    ) {
        return false;
    }
    // Below the two runtime levels: only const-indexed / member / mutable
    // links, ending at a `self.<field>` head (machine-owned storage; a bare
    // local/param array stays fenced -- the frame op family does not cover
    // both-runtime yet).
    let mut place = inner_indexed.collection;
    loop {
        match program.expression_table.expression(place) {
            ExpressionNode::Indexed(below) => {
                let mut below_index = below.index;
                while let ExpressionNode::Borrow(next) =
                    program.expression_table.expression(below_index)
                {
                    below_index = next.target;
                }
                if !matches!(
                    program.expression_table.expression(below_index),
                    ExpressionNode::Integer(_)
                ) {
                    return false;
                }
                place = below.collection;
            }
            ExpressionNode::Member(member) => {
                let receiver = member.receiver;
                if matches!(
                    program.expression_table.expression(receiver),
                    ExpressionNode::Name(path)
                        if program
                            .expression_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self")
                ) {
                    return true;
                }
                place = receiver;
            }
            ExpressionNode::Borrow(next) => place = next.target,
            ExpressionNode::Name(path) => {
                // A multi-member Name path starting at `self` (`self.grid`
                // resolved as one path) is machine-owned. A BARE single name
                // (a by-value param or local 2D array, `g[i][j]`) is the
                // FRAME flavor, lowered since 2026-07-07 by
                // CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage -- but
                // only for the DIRECTLY-nested member-free shape, which is
                // exactly what reaching this arm through the loop implies
                // for the frame case (member links between the indices stay
                // fenced by the resolver and report loudly downstream).
                let members = program.expression_table.name_path_members(path.members);
                return members.first().is_some_and(|name| name.as_str() == "self")
                    || members.len() == 1;
            }
            _ => return false,
        }
    }
}

/// Recursively scan `expression` for `ExpressionNode::Call` nodes and
/// validate machine-call type-parameter bounds for each one found.
#[allow(clippy::too_many_arguments)]
fn scan_expression_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    expression: ExpressionHandle,
    boundary_operator_applications: &mut Vec<crate::ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    // Unknown-field READ: a direct `self.<field>` read of a nonexistent field (a typo)
    // gets a clear error instead of silently passing type-check. Mirrors the
    // assignment-target write check (places.rs): scoped to a direct `self.<field>`
    // against the machine's top-level data fields, versioned data excluded. Nested
    // `self.a.b` (checked at `a` when the recursion reaches the receiver) and non-self
    // members are left alone. The recursion continues afterward.
    if let Some(field_name) = crate::places::direct_self_field_member(program, expression)
        && let Some(data) = crate::places::machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` reads `self.{field_name}`, but data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
            data.name.as_str()
        )));
    }
    // Unknown data-typed local/parameter member READ, plus nested `self` reads:
    // `task.continuation`, `self.o.inner.nonexistent` (final missing), and
    // `self.o.bogus.value` (intermediate missing) must not become silent ZII
    // reads. The walker reports only a provably-missing member on a
    // provably-plain container -- versioned containers (fields in wire version
    // blocks), contained-machine/owned-data roots, and non-data hops all skip.
    // The recursion below may revisit the same missing hop, so an identical
    // message is deduplicated rather than reported per level.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Name(_)
    ) && let Some((container, member)) =
        crate::places::first_unknown_nested_field(program, machine, Some(state), expression)
    {
        let message = format!(
            "machine `{}` state `{}` reads a nested member `{member}`, but data `{container}` \
             has no field `{member}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
        );
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.to_string().contains(&message))
        {
            diagnostics.push(Diagnostic::error(message));
        }
    }
    // Member / index access on a PRIMITIVE receiver. A number or bool has no fields,
    // no `.len`, and is not indexable, so `x.field` / `x.len` / `x[0]` on an `i32`
    // local silently reads a ZII 0 -- reject any such access. `String` is the one
    // exception, and only for MEMBER access: text carries a `.len` view, so `s.len`
    // is legal. An UNRESOLVED receiver or a struct / array / slice receiver is
    // left alone (those accesses are separate).
    let primitive_access = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => Some((
            member.receiver,
            format!("member `{}`", member.member.as_str()),
        )),
        ExpressionNode::Indexed(indexed) => Some((indexed.collection, "an index".to_owned())),
        _ => None,
    };
    if let Some((receiver, access)) = primitive_access
        && let Some(receiver_type) =
            crate::places::declared_place_type(program, machine, Some(state), receiver)
        && let Some(primitive) = program.primitive_type_reference(receiver_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` accesses {access} of a `{}` value, but a primitive \
             scalar has no members or elements",
            machine.name.as_str(),
            state.name.as_str(),
            primitive.name(),
        )));
    }
    // Unknown BARE NAME: a SINGLE-segment name (`undeclared_var`, not `self.x` or
    // `Type::Case`) that resolves to nothing is otherwise silently accepted and reads
    // as 0/garbage. Reject it when it is none of the legal bare-name forms. The scope
    // is the current state; retained foreign binding identities cannot be
    // replaced by a coincidentally matching spelling. Qualified paths check
    // their root too, so member access cannot hide an implicit capture.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some(first) = program
            .expression_table
            .name_path_members(path.members)
            .first()
    {
        let name = first.as_str();
        let root = if path.head_symbol.is_valid() {
            path.head_symbol
        } else {
            path.symbol
        };
        let bare_identity_matches = program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
            || !path.head_symbol.is_valid()
            || path.head_symbol == path.symbol;
        if name != "true"
            && name != "false"
            && (!bare_identity_matches
                || !crate::locals::state_value_root_is_known(
                    program,
                    machine,
                    state,
                    writable_roots.statements,
                    machine_symbols,
                    symbols,
                    root,
                    name,
                ))
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, parameter, field, or type in this state (check the spelling)",
                machine.name.as_str(), state.name.as_str(),
            )));
        }
    }
    // Unknown TWO-segment path (`Type::Case`, `Trait::NAME`): the sibling of the
    // bare-name check above. A LEGITIMATE qualified case (`Signal::Green`) or a
    // substituted const resolves to a valid symbol before this stage; an
    // unresolved `Scope::tail` -- a bogus case (`Signal::Blue`) or a typo -- keeps BOTH the
    // head and leaf symbols invalid and would silently read 0 (ZII) in value
    // position (native AND interpreter agree, so the differential cannot catch
    // it). Reject exactly that shape: both symbols unresolved.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let [scope, tail] = program.expression_table.name_path_members(path.members)
        && !path.head_symbol.is_valid()
        && !path.symbol.is_valid()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` uses `{}::{}`, which resolves to no case or \
             constant -- it would silently read 0 (ZII). Check the spelling and \
             ensure the declaration is imported",
            machine.name.as_str(),
            state.name.as_str(),
            scope.as_str(),
            tail.as_str(),
        )));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => scan_expression_calls(
            program,
            machine,
            state,
            machine_symbols,
            symbols,
            writable_roots,
            value_env,
            atomic.value,
            boundary_operator_applications,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            let call = call.clone();
            if crate::proof_embeddings::is_exact_embed_call(program, &call) {
                crate::arithmetic_domains::validate_total_specification_arithmetic(
                    program,
                    machine,
                    Some(state),
                    expression,
                    value_env,
                    "`embed` source in a proof body",
                    diagnostics,
                );
            }
            validate_expression_call_bounds(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                expression,
                &call,
                boundary_operator_applications,
                diagnostics,
            );
            // Recurse into the receiver and arguments (nested calls).
            if call.receiver.is_valid()
                && !crate::locals::expression_call_has_operator_namespace(program, &call)
            {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    call.receiver,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // Every binary-operand TYPE check (operator applied to operands it is
            // not defined for) lives behind one dispatcher.
            crate::expression_types::validate_binary_operand_types(
                program,
                machine,
                Some(state),
                binary.operator,
                binary.left,
                binary.right,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.left,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.right,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
            crate::locals::StateValueScope {
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                prior_statements: writable_roots.statements,
                context: "cast type bound",
            }
            .type_reference(cast.target_type, diagnostics);
            // All cast TARGET/SOURCE type validation lives behind one dispatcher.
            crate::expression_types::validate_cast_types(
                program,
                machine,
                Some(state),
                cast,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                cast.value,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            // Reading a nested array element with a RUNTIME COLUMN index (`grid[..][j]`)
            // silently reads 0 (the backend cannot lower it yet); the sibling WRITE is
            // already fenced, this fences the READ to match.
            report_nested_runtime_indexed_read(program, machine, state, expression, diagnostics);
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.collection,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.index,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                member.receiver,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                inner.target,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            match unary.operator {
                psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
                    crate::expression_types::report_non_integer_bitwise_not(
                        program,
                        machine,
                        Some(state),
                        unary.operand,
                        diagnostics,
                    );
                }
                psi_typed_trees::expression::UnaryOperator::LogicalNot => {
                    crate::expression_types::report_non_bool_logical_not(
                        program,
                        machine,
                        Some(state),
                        unary.operand,
                        diagnostics,
                    );
                }
            }
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                unary.operand,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = *elements;
            for element in program.expression_table.expression_handles(elements) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *element,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = literal.fields;
            for field in program.expression_table.struct_fields(fields) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    field.value,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.start,
                boundary_operator_applications,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.end,
                boundary_operator_applications,
                diagnostics,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::ZeroValue(type_reference) => {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `zero_value<{}>()` in executable code; \
                 `zero_value<T>()` is a proof-only representation observation and may \
                 appear only in contracts",
                machine.name.as_str(),
                state.name.as_str(),
                program.display_type_reference(*type_reference),
            )));
        }
    }
}
