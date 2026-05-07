use crate::ir::expression::{BinaryOperator, Expression};
use crate::ir::statement::TransitionGuard;
use crate::native::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::native::runtime_dispatch::states::{DispatchEdge, StateDispatchPlan};
use crate::native::runtime_flow::RuntimeTransitionTarget;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub guards: Arena<StateGuard>,
    pub operands: Arena<StateGuardOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuard {
    pub source_machine: String,
    pub source_state: String,
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
            source_machine: String::new(),
            source_state: String::new(),
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
    operands: &mut Arena<StateGuardOperand>,
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    source_state: &str,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let (kind, operator, expression, has_expression) = guard_data(&edge.guard);
    let guard_operands = guard_operands(layouts, entry_machine, source_machine, &edge.guard);
    let lowering = guard_lowering(kind, operator, &guard_operands);
    let operands = operands.insert_many(guard_operands);

    StateGuard {
        source_machine: source_machine.to_owned(),
        source_state: source_state.to_owned(),
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
    operands: &[StateGuardOperand],
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

    let [left, right] = operands else {
        return StateGuardLowering::NeedsRuntimeExpression;
    };

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

fn guard_operands(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    guard: &TransitionGuard,
) -> Vec<StateGuardOperand> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return Vec::new();
    };

    [binary.left.clone(), binary.right.clone()]
        .into_iter()
        .map(|expression| {
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
        })
        .collect()
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
    suffix: &[String],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for segment in suffix {
        let data_layout = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == type_name)
            .map(|(_, data_layout)| data_layout)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, segment)?;
        byte_offset += field.offset;
        type_name = &field.type_name;
        layout = field.layout;
    }

    Some((byte_offset, layout))
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

fn is_static_symbol_path(path: &[String]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}
