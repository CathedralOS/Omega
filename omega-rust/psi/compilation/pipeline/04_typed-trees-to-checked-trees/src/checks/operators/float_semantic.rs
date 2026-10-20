//! Transient resolved terms for source float predicate entailment.
//!
//! Public total Boolean queries expose sealed semantic equations. Observed
//! truth transports through the selected equation over exact source identities,
//! not rendered names. Quiet float results compose their selected denotations;
//! adapted results contribute no unconditional raw equation. Proof-position
//! operations are resolved in the fact's own typed frame: they have no executable
//! operator-use row or operand capture to fabricate.
//! Integer catalog inputs have their own carrier and completed-value authority;
//! they never occupy a FloatMeaning projection. The selected equation retains
//! its formal telescope while captured actuals retain their independent caller
//! owner, so composing a conversion cannot reinterpret a source operation in
//! the callee's scope or replace its arithmetic policy.

use checked_trees::{CheckFacts, CheckedArithmeticPolicyAdapter};
use facts::{FactContextHandle, FactPayload, ProgramPoint};
use numerics::float_projection::FloatProjectionContractIdentity;
use numerics::float_semantics::{FloatFormat, FloatMeaning};
use numerics::float_semantics_catalog::{
    FloatSemanticContractIdentity, FloatSemanticOperand, FloatSemanticOperation,
    FloatSemanticResult, FloatSemanticValueKind, IntegerCarrier,
};
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
        operation: &'static FloatSemanticOperation,
        operands: Vec<FloatSemanticTerm>,
    },
    Projection {
        selected: SymbolHandle,
        contract: FloatProjectionContractIdentity,
        source: FloatSourceValue,
    },
    Format(IeeeFloatFormat),
    Integer {
        carrier: IntegerCarrier,
        value: numerics::bignum::BigInt,
    },
    /// An admitted captured Integer value keeps its own policy and source
    /// frame. Its carrier image can prove a classification consequence without
    /// asserting a value or replaying the producer's arithmetic as mathematics.
    IntegerSource {
        carrier: IntegerCarrier,
        domain: numerics::arithmetic::ArithmeticDomain,
        frame: Frame,
        source: FloatSourceValue,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Frame {
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
    boolean_semantic_term(program, facts, expression, frame, None, frame).as_ref() == Some(required)
}

fn boolean_semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    frame: Option<Frame>,
    actuals: Option<&SemanticBindings<'_>>,
    source_frame: Option<Frame>,
) -> Option<FloatSemanticTerm> {
    let (operator, operands, adapter) = selected_expression(program, facts, expression, frame)?;
    if adapter != CheckedArithmeticPolicyAdapter::None {
        return None;
    }
    let predicate = validation::float_boolean_semantic_contract(program, operator)?;
    let inputs = SemanticBindings {
        owner: operator.symbol,
        parameters: program.operator_parameters(operator),
        operands: &operands,
    };
    let mut remaining_work = 1024;
    semantic_term(
        program,
        facts,
        predicate,
        &inputs,
        actuals,
        frame,
        source_frame,
        &mut remaining_work,
    )
}

/// Closed caller obligations share the observed-predicate term owner. Actuals
/// are explicit source bindings, separate from a selected public query's own
/// formals; composing a raw equation cannot erase the outer call telescope.
/// Open Float storage stays a projection. Integer sources keep their own
/// bounded carrier and captured caller frame.
pub(crate) fn selected_float_boolean_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    source_owner: SymbolHandle,
    source_state: SymbolHandle,
) -> Option<bool> {
    if parameters.len() != operands.len() {
        return None;
    }
    let term = boolean_semantic_term(
        program,
        facts,
        expression,
        Some(Frame { machine, state }),
        Some(&SemanticBindings {
            owner: machine,
            parameters,
            operands,
        }),
        Some(Frame {
            machine: source_owner,
            state: source_state,
        }),
    )?;
    float_boolean_term_value(&term)
}

/// Decide a selected Boolean denotation from closed values or a kernel-owned
/// ordered image. A symbolic Integer source supplies its own finite carrier,
/// not a literal or a fact about arbitrary Float storage.
pub(crate) fn float_boolean_term_value(term: &FloatSemanticTerm) -> Option<bool> {
    let mut remaining_work = 1024;
    if let Some(FloatSemanticResult::Bool(value)) =
        closed_semantic_result(term, &mut remaining_work)
    {
        return Some(value);
    }
    let FloatSemanticTerm::Application {
        operation,
        operands,
        ..
    } = term
    else {
        return None;
    };
    let [meaning] = operands.as_slice() else {
        return None;
    };
    let (minimum, maximum) = ordered_meaning_image(meaning, &mut remaining_work)?;
    operation.ordered_interval_predicate(&minimum, &maximum)
}

/// The admitted source's image under the selected monotone Integer conversion.
/// This is an enclosure query over resolved terms, not another source evaluator.
fn ordered_meaning_image(
    term: &FloatSemanticTerm,
    remaining_work: &mut usize,
) -> Option<(FloatMeaning, FloatMeaning)> {
    *remaining_work = remaining_work.checked_sub(1)?;
    match term {
        FloatSemanticTerm::Literal { meaning, .. } => Some((meaning.clone(), meaning.clone())),
        FloatSemanticTerm::Application {
            operation,
            operands,
            ..
        } => {
            let [FloatSemanticTerm::Format(format), integer] = operands.as_slice() else {
                return None;
            };
            let (carrier, minimum, maximum) = match integer {
                FloatSemanticTerm::Integer { carrier, value } => {
                    (*carrier, value.clone(), value.clone())
                }
                FloatSemanticTerm::IntegerSource { carrier, .. } => {
                    let (minimum, maximum) = carrier.format().bounds();
                    (*carrier, minimum, maximum)
                }
                _ => return None,
            };
            let format = match format {
                IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
            };
            operation.integer_operand_image(format, carrier, &minimum, &maximum)
        }
        _ => None,
    }
}

fn closed_semantic_result(
    term: &FloatSemanticTerm,
    remaining_work: &mut usize,
) -> Option<FloatSemanticResult> {
    *remaining_work = remaining_work.checked_sub(1)?;
    match term {
        FloatSemanticTerm::Literal { meaning, .. } => {
            Some(FloatSemanticResult::Meaning(meaning.clone()))
        }
        FloatSemanticTerm::Integer { value, .. } => {
            Some(FloatSemanticResult::Integer(value.clone()))
        }
        FloatSemanticTerm::Application {
            operation,
            operands,
            ..
        } => {
            let operands = operands
                .iter()
                .map(|operand| match operand {
                    FloatSemanticTerm::Format(format) => {
                        Some(FloatSemanticOperand::Format(match format {
                            IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                            IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                        }))
                    }
                    _ => match closed_semantic_result(operand, remaining_work)? {
                        FloatSemanticResult::Meaning(meaning) => {
                            Some(FloatSemanticOperand::Meaning(meaning))
                        }
                        FloatSemanticResult::Integer(integer) => {
                            Some(FloatSemanticOperand::Integer(integer))
                        }
                        _ => None,
                    },
                })
                .collect::<Option<Vec<_>>>()?;
            operation
                .kernel_discharge(&operands)
                .map(|discharge| discharge.result)
        }
        FloatSemanticTerm::Projection { .. }
        | FloatSemanticTerm::IntegerSource { .. }
        | FloatSemanticTerm::Format(_) => None,
    }
}

pub(crate) fn instantiated_float_semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    selected: SymbolHandle,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    source_owner: SymbolHandle,
    source_state: SymbolHandle,
) -> Option<FloatSemanticTerm> {
    if parameters.len() != operands.len() {
        return None;
    }
    let mut remaining_work = 1024;
    semantic_term(
        program,
        facts,
        expression,
        &SemanticBindings {
            owner: selected,
            parameters,
            operands,
        },
        None,
        None,
        Some(Frame {
            machine: source_owner,
            state: source_state,
        }),
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
        matches!(
            row.kernel(),
            numerics::float_semantics_catalog::FloatSemanticKernel::SinglePredicate(
                numerics::float_semantics_catalog::FloatSemanticPredicate::Finite
            )
        )
        .then_some((operator.symbol, row, contract))
    });
    let (selected, operation, contract) = finite.next()?;
    if finite.next().is_some() {
        return None;
    }
    Some(FloatSemanticTerm::Application {
        selected,
        contract,
        operation,
        operands: vec![raw],
    })
}

/// The selected equation owns its formal types as well as their substitutions.
/// The outer call telescope is independent: replacing this scope during raw
/// result composition must not erase bindings of captured caller arguments.
struct SemanticBindings<'scope> {
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    operands: &'scope [ExpressionHandle],
}

impl SemanticBindings<'_> {
    fn operand(&self, symbol: SymbolHandle) -> Option<ExpressionHandle> {
        self.parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .and_then(|position| self.operands.get(position).copied())
    }
}

fn semantic_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    inputs: &SemanticBindings<'_>,
    actuals: Option<&SemanticBindings<'_>>,
    frame: Option<Frame>,
    source_frame: Option<Frame>,
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
        let source = substituted_source(program, *source, inputs, actuals, remaining_work)?;
        if let Some((selected, operands, CheckedArithmeticPolicyAdapter::None)) =
            selected_expression(program, facts, source, source_frame.or(frame))
            && let Some(raw) = validation::float_result_semantic_contract(program, selected)
        {
            return semantic_term(
                program,
                facts,
                raw,
                &SemanticBindings {
                    owner: selected.symbol,
                    parameters: program.operator_parameters(selected),
                    operands: &operands,
                },
                actuals,
                frame,
                source_frame,
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
        if matches!(
            program.expression_table.expression(source),
            ExpressionNode::UndeterminedNaN(_)
        ) {
            return Some(FloatSemanticTerm::Literal {
                format,
                meaning: FloatMeaning::NaN,
            });
        }
        return Some(FloatSemanticTerm::Projection {
            selected,
            contract,
            source: source_value(program, source)?,
        });
    }
    let (selected, row, contract) = validation::float_semantic_application_for_parameters(
        program,
        call,
        inputs.owner,
        inputs.parameters,
    )?;
    if arguments.len() != row.parameters.len() {
        return None;
    }
    let operands = arguments
        .iter()
        .zip(row.parameters)
        .map(|(argument, kind)| match kind {
            FloatSemanticValueKind::Integer(carrier) => {
                let source = substituted_operand(program, *argument, inputs, actuals)?;
                let owner = source_frame.map_or(SymbolHandle::invalid(), |frame| frame.machine);
                if let Some(value) = validation::closed_float_semantic_integer_operand(
                    program, owner, source, *carrier,
                ) {
                    return Some(FloatSemanticTerm::Integer {
                        carrier: *carrier,
                        value,
                    });
                }
                let frame = source_frame?;
                let machine = program.machine_by_symbol(frame.machine)?;
                let state = program.state_in_machine(machine, frame.state)?;
                let reference =
                    validation::expression_result_type_reference(program, machine, state, source)?;
                let primitive = program.primitive_type_reference(reference)?;
                if IntegerCarrier::from_spelling(primitive.name()) != Some(*carrier) {
                    return None;
                }
                Some(FloatSemanticTerm::IntegerSource {
                    carrier: *carrier,
                    domain: program.arithmetic_domain_for_type_reference(reference),
                    frame,
                    source: source_value(program, source)?,
                })
            }
            _ => semantic_term(
                program,
                facts,
                *argument,
                inputs,
                actuals,
                frame,
                source_frame,
                remaining_work,
            ),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(FloatSemanticTerm::Application {
        selected,
        contract,
        operation: row,
        operands,
    })
}

fn substituted_source(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
    inputs: &SemanticBindings<'_>,
    actuals: Option<&SemanticBindings<'_>>,
    remaining_work: &mut usize,
) -> Option<ExpressionHandle> {
    // A selected equation substitutes its formal once. Its actual belongs to
    // source scope, so only the independent outer call bindings apply there.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some(operand) = inputs.operand(path.symbol)
    {
        expression = operand;
    }
    loop {
        *remaining_work = remaining_work.checked_sub(1)?;
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) if !cast.form.is_recast() => expression = cast.value,
            ExpressionNode::Name(path) => {
                let Some(operand) = actuals.and_then(|actuals| actuals.operand(path.symbol)) else {
                    return Some(expression);
                };
                // An actual is in caller scope, never another callee formal.
                expression = operand;
                while let ExpressionNode::Cast(cast) =
                    program.expression_table.expression(expression)
                {
                    *remaining_work = remaining_work.checked_sub(1)?;
                    if cast.form.is_recast() {
                        break;
                    }
                    expression = cast.value;
                }
                return Some(expression);
            }
            _ => return Some(expression),
        }
    }
}

/// Integer carrier qualification participates in its closed operations. Unlike
/// float projection erasure, substitute the exact source without stripping its
/// casts; the shared numeric evaluator owns their denotation and result carrier.
fn substituted_operand(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
    inputs: &SemanticBindings<'_>,
    actuals: Option<&SemanticBindings<'_>>,
) -> Option<ExpressionHandle> {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some(operand) = inputs.operand(path.symbol)
    {
        expression = operand;
    }
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some(operand) = actuals.and_then(|actuals| actuals.operand(path.symbol))
    {
        expression = operand;
    }
    Some(expression)
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
    let mut first_selection = None;
    let mut multiple_selections = false;
    facts.operators.for_each_resolved_expression_selection(
        expression,
        |_, selected, adapter, use_fact| {
            if first_selection.is_none() {
                first_selection = Some((selected, adapter, use_fact));
            } else {
                multiple_selections = true;
            }
        },
    );
    if let Some((selected, adapter, selected_use)) = first_selection {
        if multiple_selections {
            return None;
        }
        let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
        let operands = if let Some(selected_use) = selected_use {
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
    // Proof-position operations have exact typed owners but no executable use
    // row. Reuse contract selection for every authored dispatch surface, then
    // require quiet actual policy. Missing executable custody cannot take this
    // path because only genuine source facts carry a frame.
    let frame = frame?;
    let (operator, operands) = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            let operator = typed_trees::operator::resolve_named_expression_call(program, call)
                .or_else(|| {
                    crate::authored_selections::checked_named_operator_call(
                        program,
                        facts,
                        expression,
                        call,
                        program.expression_table.source_span(expression),
                        None,
                    )
                })?;
            let operands = crate::facts::operator_crashes::named_call_operands(
                program,
                call,
                program.operator_parameters(operator),
            )?;
            (operator, operands)
        }
        node @ ExpressionNode::Binary(binary) => {
            let crate::authored_selections::CheckedContractOperatorResolution::Declaration(
                selected,
            ) = crate::authored_selections::checked_operator_resolution(
                program, facts, expression, node, None,
            )?
            else {
                return None;
            };
            let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
            (operator, vec![binary.left, binary.right])
        }
        // The typed unary forms are logical/bitwise negation, with no authored
        // operator dispatch surface or float result semantic equation.
        ExpressionNode::Unary(_) => return None,
        _ => return None,
    };
    // Qualification adapts float-producing operations. A total Boolean query
    // observes its argument's denotation without adapting that argument; its
    // public semantic equation remains valid for qualified source values.
    if matches!(
        program.primitive_type_reference(operator.return_type),
        Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
    ) {
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
        | facts::call_point_triple_pattern!(machine_symbol, state_symbol, ..)
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
