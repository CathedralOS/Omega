//! Transient resolved terms for source float predicate entailment.
//!
//! Public total Boolean queries expose sealed semantic equations. Observed
//! truth transports through the selected equation over exact source identities,
//! not rendered names. Quiet float results compose their selected denotations;
//! adapted results contribute no unconditional raw equation. Proof-position
//! calls are resolved in the fact's own typed frame: they have no executable
//! operator-use row or operand capture to fabricate.

use checked_trees::{CheckFacts, CheckedArithmeticPolicyAdapter};
use facts::{FactContextHandle, FactPayload, ProgramPoint};
use numerics::float_projection::FloatProjectionContractIdentity;
use numerics::float_semantics::FloatMeaning;
use numerics::float_semantics_catalog::FloatSemanticContractIdentity;
use semantic_vocabulary::IeeeFloatFormat;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::OperatorDefinition;
use typed_trees::signature::StateParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FloatSemanticTerm {
    Application {
        selected: SymbolHandle,
        contract: FloatSemanticContractIdentity,
        operands: Vec<FloatSemanticTerm>,
    },
    Projection {
        selected: SymbolHandle,
        contract: FloatProjectionContractIdentity,
        source: FloatSourceValue,
    },
    Format(IeeeFloatFormat),
    Literal {
        format: IeeeFloatFormat,
        meaning: FloatMeaning,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FloatSourceValue {
    Place(Vec<SymbolHandle>),
    Indexed {
        collection: Box<FloatSourceValue>,
        index: ExpressionHandle,
    },
    /// A non-place value without a retained semantic equation keeps its exact
    /// occurrence identity; another written expression cannot impersonate it.
    Occurrence(ExpressionHandle),
}

#[derive(Clone, Copy)]
struct Frame {
    machine: SymbolHandle,
    state: SymbolHandle,
}

pub(crate) fn context_proves_float_semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    context: FactContextHandle,
    required: &FloatSemanticTerm,
) -> bool {
    facts
        .semantic
        .context_view(facts.semantic.contexts.get(context))
        .facts()
        .any(|fact| {
            let expression = match fact.payload {
                FactPayload::BooleanExpression(expression)
                | FactPayload::BooleanValue {
                    expression,
                    value: true,
                } => expression,
                FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => expression,
                _ => return false,
            };
            positive_expression_proves(
                program,
                facts,
                expression,
                frame(program, fact.point),
                required,
            )
        })
}

fn positive_expression_proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    frame: Option<Frame>,
    required: &FloatSemanticTerm,
) -> bool {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == BinaryOperator::And
        && frame.is_some_and(|frame| {
            let Some(machine) = program.machine_by_symbol(frame.machine) else {
                return false;
            };
            validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                program.state_in_machine(machine, frame.state),
                expression,
            )
        })
    {
        return positive_expression_proves(program, facts, binary.left, frame, required)
            || positive_expression_proves(program, facts, binary.right, frame, required);
    }
    let Some((operator, operands, adapter)) =
        selected_expression(program, facts, expression, frame)
    else {
        return false;
    };
    if adapter != CheckedArithmeticPolicyAdapter::None {
        return false;
    }
    let Some(predicate) = validation::float_boolean_semantic_contract(program, operator) else {
        return false;
    };
    let inputs = inputs(program.operator_parameters(operator), &operands);
    let mut remaining_work = 1024;
    semantic_term(
        program,
        facts,
        predicate,
        &inputs,
        frame,
        &mut remaining_work,
    )
    .as_ref()
        == Some(required)
}

pub(crate) fn instantiated_float_semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
) -> Option<FloatSemanticTerm> {
    if parameters.len() != operands.len() {
        return None;
    }
    let mut remaining_work = 1024;
    semantic_term(
        program,
        facts,
        expression,
        &inputs(parameters, operands),
        None,
        &mut remaining_work,
    )
}

pub(crate) fn finite_float_semantic_term(
    program: &dyn ::validation::CheckingInput,
    raw: FloatSemanticTerm,
) -> Option<FloatSemanticTerm> {
    let mut finite = program.operators().iter().filter_map(|operator| {
        let (row, contract) =
            validation::exact_toolchain_float_semantic_contract(program, operator)?;
        (row.name == "is_finite").then_some((operator.symbol, contract))
    });
    let (selected, contract) = finite.next()?;
    if finite.next().is_some() {
        return None;
    }
    Some(FloatSemanticTerm::Application {
        selected,
        contract,
        operands: vec![raw],
    })
}

fn inputs(
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
) -> Vec<(SymbolHandle, ExpressionHandle)> {
    parameters
        .iter()
        .zip(operands)
        .map(|(parameter, operand)| (parameter.symbol, *operand))
        .collect()
}

fn semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    inputs: &[(SymbolHandle, ExpressionHandle)],
    frame: Option<Frame>,
    remaining_work: &mut usize,
) -> Option<FloatSemanticTerm> {
    *remaining_work = remaining_work.checked_sub(1)?;
    if let Some(format) = validation::exact_toolchain_float_format_const(program, expression) {
        return Some(FloatSemanticTerm::Format(format));
    }
    let call = program.expression_table.call_node(expression)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    if let Some((selected, primitive, _, contract)) =
        validation::exact_toolchain_float_projection_operation(program, call)
    {
        let [source] = arguments else {
            return None;
        };
        let source = substituted_source(program, *source, inputs);
        if let Some((selected, operands, CheckedArithmeticPolicyAdapter::None)) =
            selected_expression(program, facts, source, frame)
            && let Some(raw) = validation::float_result_semantic_contract(program, selected)
        {
            return semantic_term(
                program,
                facts,
                raw,
                &self::inputs(program.operator_parameters(selected), &operands),
                frame,
                remaining_work,
            );
        }
        let format = match primitive {
            typed_trees::types::PrimitiveType::F32 => IeeeFloatFormat::Binary32,
            typed_trees::types::PrimitiveType::F64 => IeeeFloatFormat::Binary64,
            _ => return None,
        };
        if let ExpressionNode::Float(literal) = program.expression_table.expression(source) {
            let meaning = match format {
                IeeeFloatFormat::Binary32 => FloatMeaning::from_f32(literal.value_f32()),
                IeeeFloatFormat::Binary64 => FloatMeaning::from_f64(literal.landed_f64()),
            };
            return Some(FloatSemanticTerm::Literal { format, meaning });
        }
        return Some(FloatSemanticTerm::Projection {
            selected,
            contract,
            source: source_value(program, source)?,
        });
    }
    let selected = typed_trees::operator::resolve_named_expression_call(program, call)?;
    let (row, contract) = validation::exact_toolchain_float_semantic_contract(program, selected)?;
    if arguments.len() != row.parameters.len() {
        return None;
    }
    let operands = arguments
        .iter()
        .map(|argument| semantic_term(program, facts, *argument, inputs, frame, remaining_work))
        .collect::<Option<Vec<_>>>()?;
    Some(FloatSemanticTerm::Application {
        selected: selected.symbol,
        contract,
        operands,
    })
}

fn substituted_source(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
    inputs: &[(SymbolHandle, ExpressionHandle)],
) -> ExpressionHandle {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some((_, operand)) = inputs
            .iter()
            .find(|(parameter, _)| *parameter == path.symbol)
    {
        expression = *operand;
    }
    while let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) {
        if cast.form.is_recast() {
            break;
        }
        expression = cast.value;
    }
    expression
}

fn source_value(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<FloatSourceValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let identities = if members.is_empty() {
                vec![path.symbol]
            } else {
                members.to_vec()
            };
            identities
                .iter()
                .all(|symbol| symbol.is_valid())
                .then_some(FloatSourceValue::Place(identities))
        }
        ExpressionNode::Member(member) => {
            let FloatSourceValue::Place(mut path) = source_value(program, member.receiver)? else {
                return None;
            };
            if !member.member_symbol.is_valid() {
                return None;
            }
            path.push(member.member_symbol);
            Some(FloatSourceValue::Place(path))
        }
        ExpressionNode::Indexed(indexed) => Some(FloatSourceValue::Indexed {
            collection: Box::new(source_value(program, indexed.collection)?),
            index: indexed.index,
        }),
        _ => Some(FloatSourceValue::Occurrence(expression)),
    }
}

fn selected_expression<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    frame: Option<Frame>,
) -> Option<(
    &'program OperatorDefinition,
    Vec<ExpressionHandle>,
    CheckedArithmeticPolicyAdapter,
)> {
    let mut selections = facts
        .operators
        .resolved_expression_selections()
        .filter(|(selected, _, _, _)| *selected == expression);
    if let Some((_, _, selected, adapter)) = selections.next() {
        if selections.next().is_some() {
            return None;
        }
        let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
        let operands = if let Some((_, selected_use)) =
            facts.operators.uses.iter().find(|(_, selected_use)| {
                selected_use.expression == expression
                    && selected_use.selected_operator_symbol == selected
            }) {
            selected_use.operands(program)?
        } else {
            crate::facts::operator_crashes::named_call_operands(
                program,
                program.expression_table.call_node(expression)?,
                program.operator_parameters(operator),
            )?
        };
        return (operands.len() == program.operator_parameters(operator).len())
            .then_some((operator, operands, adapter));
    }
    // Proof-position source calls have typed selections but no executable use
    // row. Resolve in the fact's actual frame and require quiet actual policy;
    // a missing executable capture never falls back to this path.
    let frame = frame?;
    let call = program.expression_table.call_node(expression)?;
    let operator = typed_trees::operator::resolve_named_expression_call(program, call)?;
    let operands = crate::facts::operator_crashes::named_call_operands(
        program,
        call,
        program.operator_parameters(operator),
    )?;
    let machine = program.machine_by_symbol(frame.machine)?;
    let state = program.state_in_machine(machine, frame.state)?;
    for operand in &operands {
        if matches!(
            program.expression_table.expression(*operand),
            ExpressionNode::Float(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
        ) {
            continue;
        }
        let reference =
            validation::expression_result_type_reference(program, machine, state, *operand)?;
        if program.arithmetic_domain_for_type_reference(reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return None;
        }
    }
    Some((operator, operands, CheckedArithmeticPolicyAdapter::None))
}

fn frame(program: &dyn ::validation::CheckingInput, point: ProgramPoint) -> Option<Frame> {
    match point {
        ProgramPoint::State {
            machine_symbol,
            state_symbol,
        }
        | ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        }
        | ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            ..
        }
        | ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            ..
        }
        | ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            ..
        }
        | ProgramPoint::TransitionArm {
            machine_symbol,
            state_symbol,
            ..
        }
        | ProgramPoint::Exit {
            machine_symbol,
            state_symbol,
            ..
        } => Some(Frame {
            machine: machine_symbol,
            state: state_symbol,
        }),
        ProgramPoint::Machine { machine_symbol } => Some(Frame {
            machine: machine_symbol,
            state: program
                .machine_states(program.machine_by_symbol(machine_symbol)?)
                .first()?
                .symbol,
        }),
        _ => None,
    }
}
