//! Queries over the typed program the collection needs: expression and
//! field types, data definitions, callable returns and dehoisted operands.

use crate::obligations::collection::expression_contains_call_node;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn expression_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // A `self.field` place resolves through the machine's ATTACHED DATA. Without
    // this, `self.field` returned None here, so a bounded-assignment obligation
    // for a range-refined field target (`self.x = 9999` with `x: i32 [0..=100]`)
    // was never collected and the declared range went UNENFORCED on assignment
    // (the field range a later narrowing trusts must hold at every write).
    if let Some(field_type) = attached_data_field_type(program, machine, expression) {
        return Some(field_type);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
        }
        ExpressionNode::Name(path) => {
            if path.symbol.is_valid() {
                return type_reference_for_symbol(program, machine, state, path.symbol);
            }

            let name = match program.expression_table.name_path_members(path.members) {
                [name] => name,
                [receiver, name] if receiver.as_str() == "self" => name,
                _ => return None,
            };

            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name == *name)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    local_data_by_name(program, state, name)
                        .map(|local_data| local_data.type_reference)
                })
                .or_else(|| {
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned_data| owned_data.name == *name)
                        .map(|owned_data| owned_data.type_reference)
                })
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
    program: &TypedTrees,
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

/// Resolve a `self.a.b.c` field place (ONE level `self.f` or NESTED) to the
/// final field's DECLARED type reference (constraints intact) via the machine's
/// attached data, descending into each intermediate field's data type. `None` for
/// any other expression shape. Mirrors `typed-trees-to-checked-trees`
/// `field_domain::attached_data_field_type` -- both sides must agree so a nested
/// domained field is trusted at reads exactly where it is enforced at writes.
fn attached_data_field_type(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let path = self_field_path(program, expression)?;
    let (last, parents) = path.split_last()?;

    let attached = machine.attached_data.as_ref()?;
    let mut data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    for segment in parents {
        let field_type = data_field_type_by_name(program, data, segment)?;
        let next = type_reference_data_name(program, field_type)?;
        data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == next.as_str())?;
    }
    data_field_type_by_name(program, data, last)
}

/// The segments of a `self.a.b.c` field-access path AFTER `self`, or `None` if
/// not a `self`-rooted field access. Handles the nested `Member` chain and a flat
/// `Name` path alike.
fn self_field_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<Vec<String>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let mut path = self_field_path(program, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(name) => {
            match program.expression_table.name_path_members(name.members) {
                [first, rest @ ..] if first.as_str() == "self" => Some(
                    rest.iter()
                        .map(|segment| segment.as_str().to_owned())
                        .collect(),
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn data_field_type_by_name(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// The data-type name a field's type reference names (peeling `&`/`&mut` and a
/// domain `Constrained` wrapper), for descending a nested field path.
fn type_reference_data_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
        TypeReferenceNode::Reference { referee, .. } => type_reference_data_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_data_name(program, *base_type)
        }
        _ => None,
    }
}

/// Resolve a bare-name operand through its call-free LocalData initializer
/// when that initializer is a PLACE read (Indexed/Member/Name through
/// `Mutable`): `__hoist_N` -> `tallies[self.k]`. Anything else (calls,
/// arithmetic, non-locals) stays as-is.
pub(crate) fn dehoisted_operand(
    program: &TypedTrees,
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
    program: &TypedTrees,
    state: &State,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    dehoisted_initializer(program, state, condition).unwrap_or(condition)
}

/// A bare single-segment name's call-free LocalData initializer in `state`
/// (peeling `Mutable`), or None.
fn dehoisted_initializer(
    program: &TypedTrees,
    state: &State,
    operand: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(operand) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    let local = local_data_by_name(program, state, name)?;
    if !local.initial_value.is_valid()
        || expression_contains_call_node(program, local.initial_value)
    {
        return None;
    }
    let mut initializer = local.initial_value;
    while let ExpressionNode::Borrow(inner) = program.expression_table.expression(initializer) {
        initializer = inner.target;
    }
    Some(initializer)
}

fn local_data_by_name<'program>(
    program: &'program TypedTrees,
    state: &State,
    name: &Identifier,
) -> Option<&'program TableLocalData> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::LocalData(local_data) = statement else {
                return None;
            };

            (local_data.name == *name).then_some(local_data)
        })
}

fn type_reference_for_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
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
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local_data) = statement else {
                        return None;
                    };

                    (local_data.symbol == symbol).then_some(local_data.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned_data| owned_data.symbol == symbol)
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
                            let typed_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };

                            (field.symbol == symbol).then_some(field.type_reference)
                        })
                })
        })
}

fn data_field_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            data_field_type_reference(program, *referee, member_symbol, member_name)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_field_type_reference(program, *base_type, member_symbol, member_name)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReferenceNode::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<&'program typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

pub(crate) fn data_field_in_definition(
    program: &TypedTrees,
    data_definition: &typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}

pub(crate) fn call_expression_return_type(
    program: &TypedTrees,
    _machine: &Machine,
    _state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    callable_return_type_by_symbol(program, call.target_symbol)
}

fn callable_return_type_by_symbol(
    program: &TypedTrees,
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
