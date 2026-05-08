use crate::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::runtime_dispatch::guards::{
    StateGuardOperand, StateGuardOperandKind, StateGuardOperandStorage,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

pub(in crate::runtime_dispatch::guards) struct GuardOperands {
    pub left: StateGuardOperand,
    pub right: StateGuardOperand,
}

impl GuardOperands {
    pub(in crate::runtime_dispatch::guards) fn insert_into(
        self,
        arena: &mut Arena<StateGuardOperand>,
    ) -> HandleSpan<StateGuardOperand> {
        arena.insert_many([self.left, self.right])
    }
}

pub(in crate::runtime_dispatch::guards) fn guard_operands(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    guard: &TransitionGuard,
) -> Option<GuardOperands> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };

    Some(GuardOperands {
        left: guard_operand(layouts, entry_machine, source_machine, binary.left.clone()),
        right: guard_operand(layouts, entry_machine, source_machine, binary.right.clone()),
    })
}

fn guard_operand(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: Expression,
) -> StateGuardOperand {
    let resolved_value = resolved_guard_operand_value(layouts, &expression);
    let operand_layout =
        resolve_guard_operand_layout(layouts, entry_machine, source_machine, &expression);
    StateGuardOperand {
        kind: classify_guard_operand(&expression),
        storage: operand_layout
            .as_ref()
            .map(|layout| layout.storage)
            .unwrap_or_default(),
        byte_offset: operand_layout
            .as_ref()
            .map(|layout| layout.byte_offset)
            .unwrap_or(0),
        byte_size: operand_layout
            .as_ref()
            .map(|layout| layout.layout.size)
            .unwrap_or(0),
        expression,
        resolved_value: resolved_value.unwrap_or(0),
        has_resolved_value: resolved_value.is_some(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedOperandLayout {
    storage: StateGuardOperandStorage,
    byte_offset: usize,
    layout: TypeLayout,
}

fn resolve_guard_operand_layout(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<ResolvedOperandLayout> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;

    resolve_nested_field_layout(layouts, root_field, suffix).map(|(byte_offset, layout)| {
        ResolvedOperandLayout {
            storage: StateGuardOperandStorage::MachineOwned,
            byte_offset: machine_base_offset + byte_offset,
            layout,
        }
    })
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let fields = layouts.fields.span(entry_layout.fields)?;

    fields
        .iter()
        .find(|field| field.type_name == source_machine)
        .map(|field| field.offset)
}

fn resolve_nested_field_layout(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for segment in suffix {
        let field_segment = parse_field_segment(segment)?;
        let data_layout = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == type_name)
            .map(|(_, data_layout)| data_layout)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, field_segment.name)?;
        byte_offset += field.offset;
        type_name = &field.type_name;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let array = parse_array_type_name(type_name)?;
            if index >= array.length {
                return None;
            }
            let element_layout = TypeLayout {
                size: layout.size / array.length,
                alignment: layout.alignment,
            };
            byte_offset += element_layout.size * index;
            type_name = array.element_type_name;
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

struct FieldSegment<'name> {
    name: &'name str,
    index: Option<usize>,
}

fn parse_field_segment(segment: &str) -> Option<FieldSegment<'_>> {
    let Some((field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment {
            name: segment,
            index: None,
        });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment {
        name: field_name,
        index: Some(index),
    })
}

struct ArrayTypeName<'name> {
    element_type_name: &'name str,
    length: usize,
}

fn parse_array_type_name(type_name: &str) -> Option<ArrayTypeName<'_>> {
    let inner = type_name.strip_prefix('[')?.strip_suffix(']')?;
    let (element_type_name, length) = inner.split_once(';')?;
    Some(ArrayTypeName {
        element_type_name: element_type_name.trim(),
        length: length.trim().parse::<usize>().ok()?,
    })
}

fn field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name == field_name)
}

fn resolved_guard_operand_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Boolean(value) => return Some(i64::from(*value)),
        Expression::Integer(value) => return Some(*value),
        _ => {}
    }

    let Expression::Name(path) = expression else {
        return None;
    };
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .and_then(|(_, data_layout)| match &data_layout.shape {
            DataShape::Enum { variants } => variants
                .iter()
                .position(|candidate| candidate == variant_name)
                .and_then(|index| i64::try_from(index).ok()),
            DataShape::Record { .. } => None,
        })
}

fn classify_guard_operand(expression: &Expression) -> StateGuardOperandKind {
    match expression {
        Expression::Name(path) if is_static_symbol_path(path) => {
            StateGuardOperandKind::StaticSymbol
        }
        Expression::Name(_) | Expression::Indexed(_) => StateGuardOperandKind::Place,
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => StateGuardOperandKind::Literal,
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Mutable(_)
        | Expression::StructLiteral(_) => StateGuardOperandKind::OtherExpression,
    }
}

fn is_static_symbol_path(path: &[ProgramName]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}
