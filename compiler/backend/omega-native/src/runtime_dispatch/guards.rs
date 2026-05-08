use crate::control_flow::StateKey;
use crate::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::runtime_dispatch::states::{DispatchEdge, StateDispatchPlan};
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub guards: Arena<StateGuard>,
    pub operands: Arena<StateGuardOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuard {
    pub source: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub source_dispatch_index: u32,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub statement_order: usize,
    pub kind: StateGuardKind,
    pub operator: StateGuardOperator,
    pub lowering: StateGuardLowering,
    pub expression: Expression,
    pub operands: HandleSpan<StateGuardOperand>,
    pub has_expression: bool,
    pub forms_cycle: bool,
}

impl Default for StateGuard {
    fn default() -> Self {
        Self {
            source: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            source_dispatch_index: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            statement_order: 0,
            kind: StateGuardKind::Always,
            operator: StateGuardOperator::None,
            lowering: StateGuardLowering::NoOp,
            expression: Expression::Boolean(true),
            operands: HandleSpan::empty(),
            has_expression: false,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardKind {
    #[default]
    Always,
    RuntimeEquality,
    RuntimeInequality,
    RuntimeOrdering,
    RuntimeExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperator {
    #[default]
    None,
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Add,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardLowering {
    NoOp,
    CompareStaticValue,
    CompareRuntimeValue,
    #[default]
    NeedsRuntimeExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuardOperand {
    pub expression: Expression,
    pub kind: StateGuardOperandKind,
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub resolved_value: i64,
    pub has_resolved_value: bool,
}

impl Default for StateGuardOperand {
    fn default() -> Self {
        Self {
            expression: Expression::Boolean(true),
            kind: StateGuardOperandKind::OtherExpression,
            storage: StateGuardOperandStorage::Unknown,
            byte_offset: 0,
            byte_size: 0,
            resolved_value: 0,
            has_resolved_value: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperandKind {
    Place,
    StaticSymbol,
    Literal,
    #[default]
    OtherExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperandStorage {
    MachineOwned,
    #[default]
    Unknown,
}

pub fn build_state_guard_plan(
    state_dispatch: &StateDispatchPlan,
    layouts: &LayoutPlan,
    entry_machine: &str,
) -> StateGuardPlan {
    let mut plan = StateGuardPlan::default();

    for (_, state) in state_dispatch.states.iter() {
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            plan.guards.insert(build_state_guard(
                &mut plan.operands,
                layouts,
                entry_machine,
                state.key,
                &state.machine,
                &state.state,
                state.dispatch_index,
                statement_order,
                edge,
            ));
        }
    }

    plan
}

pub fn classify_transition_guard(guard: &TransitionGuard) -> StateGuardKind {
    match guard {
        TransitionGuard::Always => StateGuardKind::Always,
        TransitionGuard::When(expression) => match expression {
            Expression::Binary(binary) => match binary.operator {
                BinaryOperator::Equal => StateGuardKind::RuntimeEquality,
                BinaryOperator::NotEqual => StateGuardKind::RuntimeInequality,
                BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual => StateGuardKind::RuntimeOrdering,
                BinaryOperator::Add | BinaryOperator::And | BinaryOperator::Or => {
                    StateGuardKind::RuntimeExpression
                }
            },
            _ => StateGuardKind::RuntimeExpression,
        },
    }
}

fn build_state_guard(
    operand_arena: &mut Arena<StateGuardOperand>,
    layouts: &LayoutPlan,
    entry_machine: &str,
    source: StateKey,
    source_machine: &ProgramName,
    source_state: &ProgramName,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let (kind, operator, expression, has_expression) = guard_data(&edge.guard);
    let guard_operands =
        guard_operands(layouts, entry_machine, source_machine.as_str(), &edge.guard);
    let lowering = guard_lowering(kind, operator, guard_operands.as_ref());
    let operands = guard_operands
        .map(|operands| operands.insert_into(operand_arena))
        .unwrap_or_default();

    StateGuard {
        source,
        source_machine: source_machine.clone(),
        source_state: source_state.clone(),
        source_dispatch_index,
        target: edge.target.clone(),
        target_dispatch_index: edge.target_dispatch_index,
        continuation: edge.continuation.clone(),
        continuation_dispatch_index: edge.continuation_dispatch_index,
        statement_order,
        kind,
        operator,
        lowering,
        expression,
        operands,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
}

fn guard_lowering(
    kind: StateGuardKind,
    operator: StateGuardOperator,
    operands: Option<&GuardOperands>,
) -> StateGuardLowering {
    if kind == StateGuardKind::Always {
        return StateGuardLowering::NoOp;
    }

    if !matches!(
        operator,
        StateGuardOperator::Equal | StateGuardOperator::NotEqual
    ) {
        return StateGuardLowering::NeedsRuntimeExpression;
    }

    let Some(operands) = operands else {
        return StateGuardLowering::NeedsRuntimeExpression;
    };
    let left = &operands.left;
    let right = &operands.right;

    if left.kind == StateGuardOperandKind::Place && right.has_resolved_value {
        return StateGuardLowering::CompareStaticValue;
    }

    if left.kind == StateGuardOperandKind::Place && right.kind == StateGuardOperandKind::Place {
        return StateGuardLowering::CompareRuntimeValue;
    }

    StateGuardLowering::NeedsRuntimeExpression
}

fn guard_data(guard: &TransitionGuard) -> (StateGuardKind, StateGuardOperator, Expression, bool) {
    match guard {
        TransitionGuard::Always => (
            StateGuardKind::Always,
            StateGuardOperator::None,
            Expression::Boolean(true),
            false,
        ),
        TransitionGuard::When(expression) => (
            classify_transition_guard(guard),
            guard_operator(expression),
            expression.clone(),
            true,
        ),
    }
}

fn guard_operator(expression: &Expression) -> StateGuardOperator {
    let Expression::Binary(binary) = expression else {
        return StateGuardOperator::None;
    };

    match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        BinaryOperator::Greater => StateGuardOperator::Greater,
        BinaryOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqual,
        BinaryOperator::Less => StateGuardOperator::Less,
        BinaryOperator::LessOrEqual => StateGuardOperator::LessOrEqual,
        BinaryOperator::Add => StateGuardOperator::Add,
        BinaryOperator::And => StateGuardOperator::And,
        BinaryOperator::Or => StateGuardOperator::Or,
    }
}

struct GuardOperands {
    left: StateGuardOperand,
    right: StateGuardOperand,
}

impl GuardOperands {
    fn insert_into(self, arena: &mut Arena<StateGuardOperand>) -> HandleSpan<StateGuardOperand> {
        arena.insert_many([self.left, self.right])
    }
}

fn guard_operands(
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
