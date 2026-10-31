//! Shared typed-expression queries for obligation construction and proof checking.
//!
//! Type lookup, callable returns and dehoisting use the same retained typed
//! program. Dehoisting checks for calls here before exposing an initializer;
//! neither these queries nor that effect scan depend on obligation collection.

use symbols::SymbolHandle;
use typed_trees::TypedTreeTables;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::StatementTable;
use typed_trees::types::TypeReferenceHandle;

/// Read view of a program's expression table: a chain that starts with this
/// call formats on the receiver's line instead of re-spelling the field path.
/// Signed on the table owner so `&TypedTrees`, `&dyn CheckingInput` and the
/// identity context all reach it through one deref chain.
pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
    &tables.expression_table
}

/// Read view of a program's statement table; see [`expression_table`].
pub(crate) fn statement_table(tables: &TypedTreeTables) -> &StatementTable {
    &tables.statement_table
}

pub(crate) fn expression_type_reference(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // A `self.field` place resolves through the machine's ATTACHED DATA. Without
    // this, `self.field` returned None here, so a bounded-assignment obligation
    // for a range-refined field target (`self.x = 9999` with `x: i32 [0..=100]`)
    // was never collected and the declared range went UNENFORCED on assignment
    // (the field range a later narrowing trusts must hold at every write).
    if let Some(field_type) =
        ::validation::field_domains::attached_data_field_type(program, machine, expression)
    {
        return Some(field_type);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
        }
        // Checking refuses a value name that resolved to no in-scope binder
        // before obligations are built; a same-spelled binder lends no type.
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            type_reference_for_symbol(program, machine, state, path.symbol)
        }
        ExpressionNode::Member(member) => {
            expression_type_reference(program, machine, state, member.receiver)
                .and_then(|receiver_type| {
                    data_field_type_reference(
                        program,
                        receiver_type,
                        member.member_symbol,
                        &member.member,
                    )
                })
                .or_else(|| {
                    type_reference_for_symbol(program, machine, state, member.member_symbol)
                })
        }
        // An INDEXED place (`self.cells[k]`, const or runtime k) is typed by
        // its collection's ELEMENT type. Without this arm, a range-refined
        // element (`cells: [i32 [0..=7]; 4]`) had NO bounded-assignment
        // obligation (writes went unenforced -- the declared element range was
        // a lie) and no read constraints. The element type carries its
        // constraints intact, so the same walk that enforces field ranges
        // enforces element ranges.
        ExpressionNode::Indexed(indexed) => {
            let collection_type =
                expression_type_reference(program, machine, state, indexed.collection)?;
            element_type_reference(program, collection_type)
        }
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        _ => None,
    }
}

/// The ELEMENT type of an array/slice type reference, through reference and
/// constraint shells: `[T; N]` / `[T]` / `&[T]` -> `T` (constraints intact).
fn element_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            element_type_reference(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            element_type_reference(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            Some(*element_type)
        }
        typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

/// Resolve a bare-name operand through its call-free LocalData initializer
/// when that initializer is a PLACE read (Indexed/Member/Name through
/// `Mutable`): `__hoist_N` -> `tallies[self.k]`. Anything else (calls,
/// arithmetic, non-locals) stays as-is.
pub(crate) fn dehoisted_operand(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    operand: ExpressionHandle,
) -> ExpressionHandle {
    match dehoisted_initializer(program, state, operand) {
        Some(initializer)
            if matches!(
                program.expression_table.expression(initializer),
                ExpressionNode::Indexed(_) | ExpressionNode::Member(_) | ExpressionNode::Name(_)
            ) =>
        {
            initializer
        }
        _ => operand,
    }
}

/// The CONDITION-level twin: a hoisted GUARD SUBJECT binds the whole boolean
/// comparison (`let __hoist_N = tallies[self.k] < 16; transition __hoist_N`),
/// so a bare-name condition resolves through any call-free initializer shape
/// (Binary comparisons included).
pub(crate) fn dehoisted_condition(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    dehoisted_initializer(program, state, condition).unwrap_or(condition)
}

/// The call-free LocalData initializer (peeling `Mutable`) of the local in
/// `state` a name is bound to, or None. Binding is by symbol: a read before a
/// later same-spelled local (a const binder `N` read ahead of `let N = ...`)
/// is not that local, and its initializer says nothing about the read.
fn dehoisted_initializer(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    operand: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let path = program.expression_table.name_path_node(operand)?;
    if !path.symbol.is_valid() {
        return None;
    }
    let (_, local) = program.state_local_declaration(state, path.symbol)?;
    if !local.initial_value.is_valid()
        || expression_contains_call_node(program, local.initial_value)
    {
        return None;
    }
    let mut initializer = local.initial_value;
    initializer = expression_table(program).peeled_expression_handle(initializer);
    Some(initializer)
}

fn type_reference_for_symbol(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameter_by_symbol(state, symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            // Value binders keep their declared carrier before specialization.
            // Its bounds are premises for the generic body, not facts inferred
            // from whichever concrete applications happen to be present.
            program
                .machine_type_parameters(machine)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .and_then(|parameter| match parameter.kind {
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                        Some(type_reference)
                    }
                    _ => None,
                })
        })
        .or_else(|| {
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local_data)| local_data.type_reference)
        })
        .or_else(|| {
            program
                .machine_owned_data_by_symbol(machine, symbol)
                .map(|owned_data| owned_data.type_reference)
        })
        .or_else(|| {
            program
                .data_definitions()
                .iter()
                .find_map(|data_definition| {
                    program
                        .data_members(data_definition)
                        .iter()
                        .find_map(|member| {
                            let field = member.as_field()?;

                            (field.symbol == symbol).then_some(field.type_reference)
                        })
                })
        })
}

fn data_field_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    ::validation::field_domains::data_field_type_reference_by_symbol_or_name(
        program,
        type_reference,
        member_symbol,
        member_name,
        ::validation::field_domains::data_definition_by_symbol_or_name,
    )
}

pub(crate) fn call_expression_return_type(
    program: &dyn ::validation::CheckingInput,
    _machine: &Machine,
    _state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    callable_return_type_by_symbol(program, call.target_symbol)
}

fn callable_return_type_by_symbol(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !target_symbol.is_valid() {
        return None;
    }

    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|candidate| candidate.symbol == target_symbol)
        .and_then(|candidate| {
            candidate
                .return_type
                .is_valid()
                .then_some(candidate.return_type)
        })
        .or_else(|| {
            program
                .machine_parameter_signature(target_symbol)
                .and_then(|(_, candidate)| {
                    candidate
                        .return_type
                        .is_valid()
                        .then_some(candidate.return_type)
                })
        })
}

/// Whether any `Call` node appears in the expression tree (an opaque effect:
/// a value-machine call may mutate fields through `&mut self`).
pub(crate) fn expression_contains_call_node(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            program.expression_table.match_deep_any(dispatch, |handle| {
                expression_contains_call_node(program, handle)
            })
        }
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call_node(program, binary.left)
                || expression_contains_call_node(program, binary.right)
        }
        ExpressionNode::Unary(unary) => expression_contains_call_node(program, unary.operand),
        ExpressionNode::Cast(cast) => expression_contains_call_node(program, cast.value),
        ExpressionNode::Borrow(inner) => expression_contains_call_node(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call_node(program, indexed.collection)
                || expression_contains_call_node(program, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_call_node(program, member.receiver),
        _ => false,
    }
}
