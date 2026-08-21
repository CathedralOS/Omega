//! Value-position expression-call scanning and bound validation.
//!
//! The parent call validator owns shared argument/type rules. This child
//! owns recursive expression traversal, value-callee resolution, and the
//! diagnostics emitted at those scan sites.

use super::{
    free_machine_entry_state, machine_state_by_symbol, user_asm_contract,
    validate_asm_operand_constraint, validate_call_arguments_handles,
    validate_call_arguments_handles_with_policy_retention, validate_generic_bound_argument_types,
    validate_machine_call_type_parameter_bounds, validate_value_call_argument_classes,
    validate_value_call_argument_classes_with_receiver,
};
use crate::arithmetic_domains::{self, ValueEnv};
use crate::locals::WritableRoots;
use crate::struct_literals::data_declares_field;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataMember, TypeParameterKind};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

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
                diagnostics,
            );
            // target is a place (Name/Member/Indexed), no calls to validate
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
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                local_data.initial_value,
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
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

/// Is `name` a legal single-segment BARE name in the body of `machine`?
///
/// A bare `Name` resolves to a field (implicit `self`), a top-level symbol (type
/// / machine / platform / trait), an enum case constant (`Red`), or a local/
/// parameter. The binding scope for locals and parameters is the WHOLE machine,
/// not one state: a sub-state legitimately reads a parameter or `let` declared on
/// the machine's entry (or an ancestor) state (`state nonpos` reading the entry
/// state's `n`). We therefore scan every state's parameters and `LocalData`.
///
/// The allow-list is deliberately GENEROUS -- scanning ALL states over-approximates
/// the true lexical scope, so an out-of-scope-but-declared name is accepted (an
/// UNDER-rejection, never a false rejection of a real name). The sole goal is to
/// catch a name that exists NOWHERE (a typo reading as 0/garbage).
fn is_known_bare_name(
    program: &TypedTrees,
    machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    name: &str,
) -> bool {
    // Field of the receiver data (bare `fld` == `self.fld`), owned data, or a
    // callable field.
    if machine_symbols.has_member(name)
        || machine_symbols.has_owned_data(name)
        || machine_symbols.callable_field_type(name).is_some()
    {
        return true;
    }
    // Top-level symbol: a type, machine, or trait spelled bare.
    if symbols.has_type(name)
        || symbols.machine(name).is_some()
        || symbols.trait_definition(name).is_some()
    {
        return true;
    }
    // A generic attached method may use its container's const parameter as a
    // value in the authored template. Instance desugaring replaces this name
    // with the concrete integer before the executable clone is validated.
    if program
        .machine_type_parameters(machine)
        .iter()
        .any(|parameter| {
            parameter.name.as_str() == name
                && matches!(&parameter.kind, TypeParameterKind::Const { .. })
        })
    {
        return true;
    }
    if let Some(attached_data) = &machine.attached_data
        && program.data_definitions().iter().any(|definition| {
            definition.name == *attached_data
                && program
                    .data_type_parameters(definition)
                    .iter()
                    .any(|parameter| {
                        parameter.name.as_str() == name
                            && matches!(&parameter.kind, TypeParameterKind::Const { .. })
                    })
        })
    {
        return true;
    }
    // Enum case constant used bare (`let s: Signal = Red`).
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            if let DataMember::Variant(variant) = member
                && variant.name.as_str() == name
            {
                return true;
            }
        }
    }
    // Parameter or local declared on ANY state of this machine (whole-machine
    // scope -- see the doc comment).
    for other in program.machine_states(machine) {
        for parameter in program.state_parameters(other) {
            if parameter.name.as_str() == name {
                return true;
            }
        }
        for statement in program.statement_table.statements(other.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && local.name.as_str() == name
            {
                return true;
            }
        }
    }
    false
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
    while let ExpressionNode::Mutable(inner) = program.expression_table.expression(index) {
        index = *inner;
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
                while let ExpressionNode::Mutable(next) =
                    program.expression_table.expression(inner_index)
                {
                    inner_index = *next;
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
            ExpressionNode::Mutable(inner) => collection = *inner,
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
            ExpressionNode::Mutable(next) => inner = *next,
            ExpressionNode::Member(member) => inner = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(inner_indexed) = program.expression_table.expression(inner) else {
        return false;
    };
    let mut inner_index = inner_indexed.index;
    while let ExpressionNode::Mutable(next) = program.expression_table.expression(inner_index) {
        inner_index = *next;
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
                while let ExpressionNode::Mutable(next) =
                    program.expression_table.expression(below_index)
                {
                    below_index = *next;
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
            ExpressionNode::Mutable(next) => place = *next,
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
    // is the whole MACHINE (a sub-state may read the entry state's params/locals), and
    // `true`/`false` are single-segment `Name` nodes here, so they are skipped. The
    // allow-list is deliberately GENEROUS -- an unrecognised valid form only
    // UNDER-rejects (misses a typo), never falsely rejects a real name.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let [only] = program.expression_table.name_path_members(path.members)
    {
        let name = only.as_str();
        if name != "self"
            && name != "true"
            && name != "false"
            && !is_known_bare_name(program, machine, machine_symbols, symbols, name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, \
                 parameter, field, or type (check the spelling)",
                machine.name.as_str(),
                state.name.as_str(),
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
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            let call = call.clone();
            validate_expression_call_bounds(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                &call,
                diagnostics,
            );
            // Recurse into the receiver and arguments (nested calls).
            if call.receiver.is_valid() {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    call.receiver,
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
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
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
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *inner,
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

/// Enforce machine-call type-parameter bounds for a single VALUE-position
/// `ExpressionNode::Call`.  The receiver name path is extracted from the
/// receiver expression (must be a `Name` node with identifier segments).
/// Other receiver shapes (member chains, indexed, etc.) are beyond this
/// scope and stand down silently, consistent with the statement-path's
/// handling of unrecognised receivers.
/// A VALUE-position call from emitted concrete code may not retain a GENERIC
/// callee: its result slot has no concrete layout. Uninstantiated generic
/// templates are different—they are checked modularly but never emitted, and
/// their symbolic calls are resolved by fixed-point specialization once a
/// concrete outer call selects them.
fn fence_generic_value_callee(
    program: &TypedTrees,
    caller_machine: &Machine,
    callee_machine: &Machine,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // An uninstantiated generic template is checked modularly but is never
    // emitted. Its symbolic value calls become concrete when an outer call
    // specializes the template. Keep the fence for concrete callers whose
    // selected callee somehow remains generic: that is still an incomplete
    // lowering and must fail loudly.
    if !program.machine_type_parameters(caller_machine).is_empty()
        || program.machine_type_parameters(callee_machine).is_empty()
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "a value call to generic machine `{target}` cannot derive a complete \
         type/const/machine/conformance specialization tuple from its argument and result types; add a \
         concrete destination annotation or provide concrete argument type evidence",
    )));
}

/// The receiver's spelled member chain, root -> leaf (`["self", "p",
/// "second"]` for `self.p.second.stored()`). `None` for non-place receivers
/// (calls, literals). Mirrors the state-call plan's `append_receiver_path`
/// walk at the typed layer.
pub(super) fn receiver_member_chain(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        ExpressionNode::Mutable(inner) => receiver_member_chain(program, *inner),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression_call_bounds(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if (call.target.as_str() == "asm#pushfq"
        || psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        )
        .is_some())
        && !call.receiver.is_valid()
    {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes 0 operands, found {}",
                call.target,
                arguments.len()
            )));
        }
        return;
    }

    if matches!(call.target.as_str(), "asm#port_in" | "asm#rdmsr") && !call.receiver.is_valid() {
        let (intrinsic, instruction, operand_index) = if call.target.as_str() == "asm#port_in" {
            ("asm#port_in", "in", 1)
        } else {
            ("asm#rdmsr", "rdmsr", 1)
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{intrinsic}` takes 1 operand, found {}",
                arguments.len()
            )));
            return;
        }
        let contract = user_asm_contract(instruction);
        validate_asm_operand_constraint(
            program,
            current_machine,
            Some(current_state),
            instruction,
            arguments[0],
            contract.operands[operand_index],
            diagnostics,
        );
        return;
    }

    if let Some(operator) = psi_typed_trees::operator::resolve_named_expression_call(program, call)
    {
        let explicit_arguments = program.expression_table.expression_handles(call.arguments);
        let parameters = program.operator_parameters(operator);
        let has_self_parameter = parameters.iter().any(|parameter| parameter.is_self);
        let mut arguments = Vec::with_capacity(explicit_arguments.len() + 1);
        if call.receiver.is_valid()
            && !has_self_parameter
            && parameters.len() == explicit_arguments.len() + 1
        {
            arguments.push(call.receiver);
        }
        arguments.extend_from_slice(explicit_arguments);
        let retains_arithmetic_policy = matches!(
            (
                program
                    .operator_path_members(operator.name)
                    .first()
                    .map(|name| name.as_str()),
                program.primitive_type_reference(operator.return_type),
            ),
            (Some("F32"), Some(PrimitiveType::F32)) | (Some("F64"), Some(PrimitiveType::F64))
        );
        validate_call_arguments_handles_with_policy_retention(
            program,
            current_machine,
            Some(current_state),
            value_env,
            &arguments,
            call.target.as_str(),
            parameters,
            None,
            writable_roots,
            retains_arithmetic_policy,
            diagnostics,
        );
        validate_named_float_to_integer_call(
            program,
            current_machine,
            current_state,
            value_env,
            operator,
            explicit_arguments,
            diagnostics,
        );
        return;
    }

    // Resolve the receiver: is this a self-call, and if not, the name of the
    // receiver object (field/local). A self-call has no receiver (`call.receiver`
    // invalid) or an explicit `self`. A `Name`-path receiver names the object via
    // its last member; a `Member` receiver (`self.host.method(..)`, where
    // `self.host` is a member access, NOT a name path) names it via that member —
    // WITHOUT this, a member receiver fell through as an empty path and the call
    // was misrouted into the self-call branch (resolving to a same-named sibling
    // state instead of the field's boundary/machine type).
    let (call_is_self, external_receiver_name): (bool, Option<&str>) = if !call.receiver.is_valid()
    {
        (true, None)
    } else {
        match program.expression_table.expression(call.receiver) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if members.is_empty() || matches!(members, [r] if r.as_str() == "self") {
                    (true, None)
                } else {
                    (false, members.last().map(Identifier::as_str))
                }
            }
            ExpressionNode::Member(member) => (false, Some(member.member.as_str())),
            _ => (true, None),
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);

    // Self-call or `self`-prefixed call: the callee is a state of the
    // current machine, an attached-data sibling machine, or a free machine.
    // Mirrors the same three-way fallback in `validate_call_node`.
    if call_is_self {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            if !signature.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}`: machine parameter `{}` does not return a value but is used in a VALUE position",
                    current_machine.name,
                    current_state.name,
                    signature.name,
                )));
            }
            validate_call_arguments_handles(
                program,
                current_machine,
                Some(current_state),
                value_env,
                arguments,
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                None,
                writable_roots,
                diagnostics,
            );
            return;
        }

        if let Some((callee_machine, callee_state)) =
            machine_state_by_symbol(program, call.target_symbol)
            && callee_machine.symbol != current_machine.symbol
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }

        if let Some(callee_state) = machine_symbols.state(call.target.as_str()) {
            report_void_value_callee(
                program,
                current_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                current_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                current_machine,
                callee_state,
                diagnostics,
            );
            return;
        }

        // A self-call can also target a SIBLING machine that shares the same
        // attached data (`machine Main::pick<T [copy]>` called from
        // `machine Main::main`). The statement-position path uses
        // `symbols.attached_machine_state(program, attached_data, call.target)`.
        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            });

        if let Some((callee_machine, callee_state)) = attached_state {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }

        // Free machine call (`compute(item)` -- no `self.`, no receiver).
        if let Some((callee_machine, callee_state)) =
            free_machine_entry_state(program, symbols, call.target.as_str())
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            None,
            None,
            call,
            diagnostics,
        );
        return;
    }

    let receiver_name = external_receiver_name.unwrap_or_default();
    // Direct field/local receivers resolve by bare name. A NESTED self-rooted
    // VALUE-position member chain (`self.p.a.get()`) resolves by walking the
    // chain's declared field types to the leaf type (receiver-place staircase,
    // rung 3). The full arc is now sound: symbol resolution stamps the nested
    // symbols (rung 2b) so the state-call plan records the call; the backend
    // storage walk descends plain-DATA intermediates (rung 2a/D1) so the
    // callee's `self` base resolves; and the emission-planning
    // contained-receiver blocker rejects an ambiguous nested receiver (a
    // same-type sibling that the by-type walk would misresolve) loudly instead
    // of binding 0. STATEMENT-position nested calls are validated separately
    // (`validate_call_node`) and remain unsupported -- see TASKS D2.
    let receiver_type_reference = crate::places::declared_place_type(
        program,
        current_machine,
        Some(current_state),
        call.receiver,
    );
    let receiver_type = machine_symbols
        .callable_field_type(receiver_name)
        .or_else(|| {
            let chain = receiver_member_chain(program, call.receiver)?;
            if chain.len() < 3 || chain.first().map(String::as_str) != Some("self") {
                return None;
            }
            crate::places::nested_receiver_type_name(
                program,
                current_machine,
                Some(current_state),
                &chain,
            )
        })
        .or_else(|| {
            receiver_type_reference
                .and_then(|type_reference| named_type_reference_name(program, type_reference))
        });

    if let Some(error) = receiver_type_reference.and_then(|type_reference| {
        crate::traits::dynamic_requirement_call_error(
            program,
            type_reference,
            call.target.as_str(),
            call.target_symbol,
        )
    }) {
        diagnostics.push(Diagnostic::error(error));
        return;
    }

    if let Some(type_reference) = receiver_type_reference {
        match crate::traits::generic_bound_requirement_call(
            program,
            current_machine,
            type_reference,
            call.target.as_str(),
        ) {
            Ok(Some(requirement)) => {
                let signature = requirement.signature;
                if !signature.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(signature.return_type),
                        TypeReferenceNode::Unit
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait requirement `{}::{}` does not return a value but is used in a VALUE position",
                        requirement.trait_definition.name,
                        signature.name,
                    )));
                }
                validate_generic_bound_argument_types(
                    program,
                    current_machine,
                    Some(current_state),
                    type_reference,
                    arguments,
                    &requirement,
                    diagnostics,
                );
                validate_call_arguments_handles(
                    program,
                    current_machine,
                    Some(current_state),
                    value_env,
                    arguments,
                    signature.name.as_str(),
                    program.state_signature_parameters(signature),
                    None,
                    writable_roots,
                    diagnostics,
                );
                return;
            }
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::error(error));
                return;
            }
        }
    }

    // N6 attached lift: a quotient has no runtime representative or attached
    // methods of its own, but a checked pure operation attached to its carrier
    // may be used proof-side when a structural respect certificate covers the
    // receiver and every explicit carrier argument. Resolve that projection
    // only from the receiver's exact quotient type; no method is installed on
    // the quotient and no runtime dispatch target is minted.
    if let Some(receiver_type_reference) = receiver_type_reference
        && let Some((callee_machine, callee_state)) =
            crate::quotients::representative_operation_for_quotient(
                program,
                receiver_type_reference,
                call.target.as_str(),
            )
    {
        report_void_value_callee(
            program,
            callee_machine,
            current_machine,
            current_state,
            callee_state,
            call.target.as_str(),
            diagnostics,
        );
        fence_generic_value_callee(
            program,
            current_machine,
            callee_machine,
            call.target.as_str(),
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            callee_state,
            callee_state.name.as_str(),
            arguments,
            current_machine,
            Some(current_state),
            diagnostics,
        );
        validate_value_call_argument_classes_with_receiver(
            program,
            current_machine,
            current_state,
            value_env,
            Some(receiver_type_reference),
            arguments,
            callee_machine,
            callee_state,
            diagnostics,
        );
        return;
    }

    // External machine receiver.
    if let Some(callee_machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver_name))
    {
        if let Some(callee_state) = program
            .machine_states(callee_machine)
            .iter()
            .find(|s| s.name == call.target)
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            Some(receiver_name),
            receiver_type,
            call,
            diagnostics,
        );
        return;
    }

    // Attached-data machine receiver.
    if let Some((callee_machine, callee_state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        report_void_value_callee(
            program,
            callee_machine,
            current_machine,
            current_state,
            callee_state,
            call.target.as_str(),
            diagnostics,
        );
        fence_generic_value_callee(
            program,
            current_machine,
            callee_machine,
            call.target.as_str(),
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            callee_state,
            callee_state.name.as_str(),
            arguments,
            current_machine,
            Some(current_state),
            diagnostics,
        );
        validate_value_call_argument_classes(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            callee_machine,
            callee_state,
            diagnostics,
        );
        let _ = writable_roots;
        return;
    }
    report_unresolved_value_call(
        program,
        current_machine,
        current_state,
        symbols,
        Some(receiver_name),
        receiver_type,
        call,
        diagnostics,
    );

    let _ = writable_roots;
}

fn validate_named_float_to_integer_call(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    value_env: &ValueEnv,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = program.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return;
    };
    if !matches!(requirement.as_str(), "from_f32" | "from_f64")
        || program
            .type_reference_table
            .arithmetic_domain(operator.return_type)
            != psi_numerics::arithmetic::ArithmeticDomain::Exact
    {
        return;
    }
    let target = match namespace.as_str() {
        "I8" => PrimitiveType::I8,
        "I16" => PrimitiveType::I16,
        "I32" => PrimitiveType::I32,
        "I64" => PrimitiveType::I64,
        "U8" => PrimitiveType::U8,
        "U16" => PrimitiveType::U16,
        "U32" => PrimitiveType::U32,
        "U64" => PrimitiveType::U64,
        _ => return,
    };
    let [value] = arguments else {
        return;
    };
    if arithmetic_domains::float_source_proves_int_cast(
        program,
        current_machine,
        Some(current_state),
        value_env,
        *value,
        target,
    ) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` cannot prove unqualified `{}::{}` operand `{}` is finite and truncates into `{}`; constrain it with a declared range or dominating non-NaN/range guard, or select result type `{} in Trapping` or `{} in Saturating`",
        current_machine.name,
        current_state.name,
        namespace,
        requirement,
        program.expression_table.display_name(*value),
        target.name(),
        target.name(),
        target.name(),
    )));
}

/// A value call on a LET-BOUND LOCAL receiver (`let p: Pair = ..; p.total()`)
/// reads ZII natively: receiver resolution reaches machine FIELDS and state
/// PARAMETERS only, so the callee's `self.field` reads bind to nothing and
/// the result silently zeroes when the caller is itself an inlined value
/// callee (Main-state spellings hit the emission backstop instead). Fence it
/// loudly until local receiver resolution lands (TASKS.md "local-receiver
/// value calls"). Field receivers, `self`, and state-parameter receivers are
/// the supported (canaried) forms.
pub(crate) fn report_local_receiver_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    if !call.receiver.is_valid() {
        return;
    }
    // BUILTIN view/operand methods (`view.bytes()`, `arr.as_slice()`, min/max
    // shapes) compose on locals through the operand machinery -- the same
    // exemption list as the nested-argument fence above.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    // Only a BARE single-member NAME receiver can be a local; `self.x` and
    // deeper member paths route through the supported field machinery.
    let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver) else {
        return;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [receiver_name] = members else {
        return;
    };
    let receiver = receiver_name.as_str();
    if receiver == "self" {
        return;
    }
    // A state PARAMETER (whole-machine scope) is a supported receiver.
    let is_parameter = program.machine_states(machine).iter().any(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.name.as_str() == receiver)
    });
    if is_parameter {
        return;
    }
    // A machine FIELD read as a bare name cannot happen (fields spell
    // `self.x`), but keep the check total: owned-data names pass through.
    let is_field = program
        .machine_owned_data(machine)
        .iter()
        .any(|owned| owned.name.as_str() == receiver);
    if is_field {
        return;
    }
    // A LOCAL-DATA binding anywhere in the machine (bindings are
    // whole-machine scope).
    let local = program.machine_states(machine).iter().find_map(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                (local.name.as_str() == receiver).then_some(local)
            })
    });
    let Some(local) = local else {
        return;
    };
    // A checked local dynamic coercion is not an ordinary local receiver:
    // closed-row lowering devirtualizes the call onto the coercion's retained
    // source place. Invalid, missing, or ambiguous conformance selection is
    // rejected independently by `collect_dynamic_conformance_selections`.
    if type_reference_contains_dynamic_trait(program, local.type_reference)
        && matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Cast(cast)
                if type_reference_contains_dynamic_trait(program, cast.target_type)
        )
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{state_name}`: value call `{}.{}(..)` uses a LET-bound          local as its receiver, which reads ZII (zeroes) natively -- receiver          resolution reaches machine fields and state parameters only. Store the          value in a data field (`self.{} = {}; self.{}.{}(..)`) or pass it as a          state parameter.",
        machine.name,
        receiver,
        call.target.as_str(),
        receiver,
        receiver,
        receiver,
        call.target.as_str(),
    )));
}

fn type_reference_contains_dynamic_trait(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_dynamic_trait(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_dynamic_trait(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. } => true,
        _ => false,
    }
}

/// A LET/ASSIGNMENT-bound value call whose ARGUMENT nests another machine call
/// (`let out = self.double(self.inc(3))`) reads a garbage inner result: the
/// inner callee's frame locals cannot materialize inside the outer call's
/// argument context in the VALUE sink (some consumer shapes fence loudly with
/// "needs stack/local storage lowering", but the guard-consumer shape slipped
/// through and natively bound 0). STATEMENT-call arguments take a different
/// materialization path and legitimately nest (the dungeon's
/// `self.append_exit(.., self.direction_command(self.opposite(d)), ..)`), so
/// this check runs ONLY on the value expression of a local/assignment.
pub(crate) fn report_nested_call_in_bound_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    // A BUILTIN outer call (`let v = max(self.range(..), floor)`) composes:
    // builtin arguments materialize as operands through the call-result-local
    // machinery (canaried). Only a MACHINE outer call's argument context is
    // broken.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        if let Some(inner) = first_non_builtin_call(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}`: a value-call argument cannot itself \
                 be a machine call yet (`{inner}(..)` nested in `{}(..)` would read a \
                 garbage result) -- bind the inner call to a local first, then pass \
                 the local.",
                machine.name,
                call.target.as_str(),
            )));
            return;
        }
    }
}

/// The first NON-BUILTIN machine call nested anywhere inside `expression`
/// (its target name, for the diagnostic), or None. Reserved value builtins
/// (`min`/`max`/`sqrt`) and the view builtins (`as_slice`/`as_mut_slice`/
/// `as_view`/`bytes`) compose in arguments and are exempt.
fn first_non_builtin_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => first_non_builtin_call(program, atomic.value),
        ExpressionNode::Call(call) => {
            if !matches!(
                call.target.as_str(),
                "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
            ) {
                return Some(call.target.clone());
            }
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| first_non_builtin_call(program, *argument))
        }
        ExpressionNode::Binary(binary) => first_non_builtin_call(program, binary.left)
            .or_else(|| first_non_builtin_call(program, binary.right)),
        ExpressionNode::Unary(unary) => first_non_builtin_call(program, unary.operand),
        ExpressionNode::Cast(cast) => first_non_builtin_call(program, cast.value),
        ExpressionNode::Mutable(inner) => first_non_builtin_call(program, *inner),
        ExpressionNode::Indexed(indexed) => first_non_builtin_call(program, indexed.collection)
            .or_else(|| first_non_builtin_call(program, indexed.index)),
        ExpressionNode::Member(member) => first_non_builtin_call(program, member.receiver),
        _ => None,
    }
}

/// A VOID callee in VALUE position used to compile and silently bind 0 (ZII)
/// -- and native/interp DIVERGED on the bound value. "Void" means: no declared
/// return type on the resolved state (the parser now lands `-> T` written
/// after the machine clauses too; it used to be silently dropped) AND no state
/// of the callee machine produces a value through a transition VALUE arm --
/// undeclared-return value machines (`transition r > 0 { true -> self.f(r-1)
/// false -> 0 }`, the termination-canary surface) stay callable.
fn report_void_value_callee(
    program: &TypedTrees,
    callee_machine: &Machine,
    current_machine: &Machine,
    current_state: &State,
    callee_state: &State,
    callee_display: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if callee_state.return_type.is_valid() {
        return;
    }
    let produces_value = program.machine_states(callee_machine).iter().any(|state| {
        state.return_type.is_valid()
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .iter()
                        .any(|handle| {
                            handle.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(*handle),
                                    TransitionTargetNode::Value(_)
                                )
                        })
                })
    });
    if produces_value {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: `{callee_display}(..)` does not return a value but is \
         used in a VALUE position -- it would silently bind 0 (ZII) at runtime. Declare \
         a return type on the callee (`-> T`) or call it as a statement.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}

/// The declared TYPE NAME of a receiver that is a state param, state local,
/// whole-machine param, or machine-owned field -- walked through reference/
/// constraint shells to the Named/Generic/DynamicTrait head. `None` for
/// primitives, arrays, slices, and unknown names.
fn receiver_declared_type_name<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<&'program str> {
    let handle = declared_receiver_type_reference(program, machine, state, receiver)?;
    named_type_reference_name(program, handle)
}

pub(crate) fn declared_receiver_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == receiver)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.name.as_str() == receiver).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.name.as_str() == receiver)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // State bindings are whole-machine scope: a param declared on any
            // state of this machine is readable everywhere in it.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == receiver)
                    .map(|parameter| parameter.type_reference)
            })
        })
        .or_else(|| {
            let attached_data = machine.attached_data.as_ref()?;
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| &definition.name == attached_data)?;
            program.data_members(definition).iter().find_map(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == receiver).then_some(field.type_reference)
            })
        })
}

fn named_type_reference_name<'program>(
    program: &'program TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&'program str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_name(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_name(program, *base_type)
        }
        TypeReferenceNode::Named { name, .. }
        | TypeReferenceNode::Generic {
            base_name: name, ..
        }
        | TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// True when `type_name` resolves the value-call target through any of the
/// channels the LOWERING understands: a boundary-trait machine signature, a
/// machine's local state, or a machine attached to that data type.
fn type_name_resolves_value_call(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_name: &str,
    target: &str,
) -> bool {
    if let Some(trait_definition) = symbols.trait_definition(type_name)
        && program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.name.as_str() == target)
    {
        return true;
    }
    if let Some(machine) = symbols.machine(type_name)
        && program
            .machine_states(machine)
            .iter()
            .any(|state| state.name.as_str() == target)
    {
        return true;
    }
    symbols
        .attached_machine_state(program, type_name, target)
        .is_some()
}

/// Decision layer for the value-call fall-through: everything the partial
/// bounds resolver above recognizes has already returned; anything the
/// LOWERING can still resolve is checked here (builtins, platform/trait
/// receivers, declared receiver types, type-name receivers). A target that
/// resolves through NONE of these names nothing anywhere -- it would silently
/// bind a ZII 0 at runtime, so it is a compile error.
#[allow(clippy::too_many_arguments)]
fn report_unresolved_value_call(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    symbols: &TopLevelSymbols<'_>,
    receiver_name: Option<&str>,
    receiver_type: Option<&str>,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = call.target.as_str();
    // Named operators are callable requirements too. Use the same exact-symbol
    // or unique path+arity resolution as checked-flow analysis; a leaf spelling
    // alone is never enough to suppress this fail-closed fence.
    if psi_typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
        return;
    }
    let Some(receiver) = receiver_name else {
        // Receiverless: the three machine channels missed; only the reserved
        // value builtins remain. `asm#port_in` is the value-position asm
        // intrinsic (`asm { in dest, port }` desugars to `dest =
        // asm#port_in(port)`); the name is unnameable from source.
        if matches!(
            target,
            "min" | "max" | "sqrt" | "asm#port_in" | "asm#pushfq" | "asm#rdmsr"
        ) || psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            target,
        )
        .is_some()
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: value call `{target}(..)` does not resolve to a \
             state of this machine, an attached sibling machine, or a free machine -- \
             it would silently bind 0 (ZII) at runtime. Check the name.",
            current_machine.name,
            current_state.name.as_str(),
        )));
        return;
    };

    // Collection/text view builtins: `arr.as_slice()` / `.as_mut_slice()`,
    // the text view `text.as_view()` (the borrow layer's own builtin list,
    // borrow/loans.rs), and the view byte accessor `view.bytes()`.
    if matches!(target, "as_slice" | "as_mut_slice" | "as_view" | "bytes") {
        return;
    }
    // Wire-schema synthesized codecs (`Schema::encode(..)` / `::decode(..)`)
    // are not user machines; a data-definition receiver resolves them.
    if matches!(target, "encode" | "decode")
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == receiver)
    {
        return;
    }

    let declared_type =
        receiver_declared_type_name(program, current_machine, current_state, receiver);
    let resolves = receiver_type
        .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        || declared_type
            .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        // The receiver may BE a type name (`Real.from(..)`, `Worker.run(..)`).
        || type_name_resolves_value_call(program, symbols, receiver, target);
    if resolves {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: value call `{receiver}.{target}(..)` does not resolve \
         to any machine state, attached machine, platform state, or boundary-trait \
         method -- it would silently bind 0 (ZII) at runtime. Check the receiver and \
         method names.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
