//! Lowering call arguments into scalar plans: retained arguments, boundary
//! calls, direct call bindings and qualified call expressions.

use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::{
    CheckedLocatedProofTerm, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole,
    CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// The arguments a call site retains: scalar-lane expressions plus the
/// proof terms erased proof-only formals contribute to the contract lane.
pub(crate) struct LoweredCallArguments {
    pub(crate) scalar_arguments: Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>,
    pub(crate) proof_terms: Vec<CheckedLocatedProofTerm>,
}

pub(crate) fn retain_call_arguments(
    arguments: LoweredCallArguments,
    parameters: &[StateParameter],
    locals: &[ScalarLocal],
    expressions: &mut Vec<CheckedLocatedScalarExpression>,
    proof_terms: &mut Vec<CheckedLocatedProofTerm>,
    source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
    binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
) {
    proof_terms.extend(arguments.proof_terms);
    for (authored_argument, argument) in arguments.scalar_arguments {
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: argument.state,
            statement_ordinal: argument.statement_ordinal,
            role: argument.role,
            expression: authored_argument,
            symbols: binding_symbols.insert_many(
                parameters.iter().map(|parameter| parameter.symbol).chain(
                    locals
                        .iter()
                        .filter(|local| !local.is_mutable)
                        .map(|local| local.symbol),
                ),
            ),
        });
        expressions.push(argument);
    }
}

pub(crate) fn call_is_boundary(program: &TypedTrees, target_symbol: symbols::SymbolHandle) -> bool {
    let requirement_symbol = program
        .machine_parameter_signature(target_symbol)
        .map_or(target_symbol, |(_, signature)| signature.symbol);
    program.machines().iter().any(|machine| {
        machine.supply_mode.is_boundary_declaration()
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_symbol)
    }) || program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == requirement_symbol)
    }) || validation::exact_compiler_intrinsic_boundary_requirement(program, target_symbol)
        .is_some()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_call_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    call_ordinal: usize,
    call_site: &crate::semantic_calls::CallSite<'_>,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<LoweredCallArguments> {
    let target_symbol = match call_site {
        crate::semantic_calls::CallSite::Statement(call) => call.target_symbol,
        crate::semantic_calls::CallSite::Expression { call, .. } => call.target_symbol,
        crate::semantic_calls::CallSite::TransitionNamed { .. } => return None,
    };
    let is_boundary = call_is_boundary(program, target_symbol);

    let target_parameters = crate::semantic_calls::call_target_parameters(program, target_symbol)?;
    let explicit_arguments =
        crate::semantic_calls::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut scalar_index = 0usize;
    let mut structural_index = 0usize;
    let mut scalar_erased_index = 0usize;
    let mut proof_erased_index = 0usize;
    let proof_only = typed_trees::proof_only::classify(program);
    let mut proof_terms = Vec::new();
    let mut output = Vec::new();
    for target in target_parameters {
        if target.is_self && !explicit_self {
            continue;
        }
        let argument = *explicit_arguments.get(explicit_index)?;
        explicit_index = explicit_index.checked_add(1)?;
        // An erased position consumes its authored argument but owns neither a
        // scalar nor a structural ordinal. Its lowered expression stays under
        // an erased role so a Unit call can rebuild the proof-only actuals.
        if crate::execution::terminal_unit::strips_erased_parameter(target)? {
            // Erased formals split into two dense lanes: scalar carriers keep
            // the scalar lane while proof-only carriers reach the contract
            // term lane, each indexed by its own lane's ordinal.
            if !is_boundary {
                if let Some(expected_type) = program.primitive_type_reference(target.type_reference)
                {
                    if let Some(lowered) = lower_return_expression(
                        program,
                        operators,
                        argument,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        expected_type,
                        exact_integer_casts,
                    ) {
                        output.push((
                            argument,
                            CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedScalarExpressionRole::ErasedUnitCallArgument {
                                    call_ordinal: u32::try_from(call_ordinal).ok()?,
                                    erased_ordinal: u32::try_from(scalar_erased_index).ok()?,
                                },
                                expression: lowered,
                            },
                        ));
                    }
                    scalar_erased_index = scalar_erased_index.checked_add(1)?;
                } else if proof_only.contract_term_carrier(program, target.type_reference) {
                    if let Some(term) = lower_proof_term(
                        program,
                        operators,
                        argument,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                        &proof_only,
                    ) {
                        proof_terms.push(CheckedLocatedProofTerm {
                            state: state.symbol,
                            statement_ordinal,
                            role: CheckedProofTermRole::ErasedUnitCallArgument {
                                call_ordinal: u32::try_from(call_ordinal).ok()?,
                                erased_ordinal: u32::try_from(proof_erased_index).ok()?,
                            },
                            expression: argument,
                            term,
                        });
                    }
                    proof_erased_index = proof_erased_index.checked_add(1)?;
                } else {
                    return None;
                }
            }
            continue;
        }
        let Some(expected_type) = program.primitive_type_reference(target.type_reference) else {
            let argument_ordinal = u32::try_from(structural_index).ok()?;
            structural_index = structural_index.checked_add(1)?;
            let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument)
            else {
                continue;
            };
            let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            else {
                continue;
            };
            if range.end_inclusive
                || operators.expression_use(argument).is_some_and(|selected| {
                    selected.spelling != language_core::OperatorSpelling::Range
                        || selected.selected_operator_symbol.is_valid()
                        || selected.candidate_count != 0
                        || !matches!(
                            selected.status,
                            CheckedOperatorResolutionStatus::Missing
                                | CheckedOperatorResolutionStatus::BuiltinFallback
                        )
                })
            {
                continue;
            }
            let call_ordinal = u32::try_from(call_ordinal).ok()?;
            for (endpoint, role) in [
                (
                    range.start,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                        call_ordinal,
                        argument_ordinal,
                    },
                ),
                (
                    range.end,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                        call_ordinal,
                        argument_ordinal,
                    },
                ),
            ] {
                if !endpoint.is_valid() {
                    continue;
                }
                if let Some(expression) = lower_return_expression(
                    program,
                    operators,
                    endpoint,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    PrimitiveType::U64,
                    exact_integer_casts,
                ) {
                    output.push((
                        endpoint,
                        CheckedLocatedScalarExpression {
                            state: state.symbol,
                            statement_ordinal,
                            role,
                            expression,
                        },
                    ));
                }
            }
            continue;
        };
        if target.is_self
            || target.is_const
            || (target.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, target).is_none())
        {
            return None;
        }
        let lowered = lower_return_expression(
            program,
            operators,
            argument,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        );
        if let Some(lowered) = lowered {
            output.push((
                argument,
                CheckedLocatedScalarExpression {
                    state: state.symbol,
                    statement_ordinal,
                    role: if is_boundary {
                        CheckedScalarExpressionRole::BoundaryCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    } else {
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    },
                    expression: lowered,
                },
            ));
        }
        scalar_index = scalar_index.checked_add(1)?;
    }
    (explicit_index == explicit_arguments.len()).then_some(LoweredCallArguments {
        scalar_arguments: output,
        proof_terms,
    })
}

/// The declared type of one struct-literal field: the data member carrying
/// `field_symbol`, or the selected case's payload field when the literal
/// constructs a case.
fn proof_field_type(
    program: &TypedTrees,
    data_symbol: symbols::SymbolHandle,
    case_symbol: Option<symbols::SymbolHandle>,
    field_symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)?;
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if case_symbol.is_none() => {
                (field.symbol == field_symbol).then_some(field.type_reference)
            }
            typed_trees::data::DataMember::Variant(variant)
                if Some(variant.symbol) == case_symbol =>
            {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == field_symbol)
                    .map(|field| field.type_reference)
            }
            _ => None,
        })
}

/// Lower one erased contract-term actual: a construction (or a bare
/// reference to the caller's own erased proof-lane formal) becomes the
/// checked proof term the callee's contract lane carries. A construction
/// field whose declared type is primitive lowers through the scalar lane
/// into a `Scalar` leaf — the erased record carrier's runtime payload is
/// retained as a term, not a runtime operand.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_proof_term(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> Option<CheckedProofTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            if !literal.type_symbol.is_valid() {
                return None;
            }
            let mut fields = Vec::new();
            for field in program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
            {
                let declared = proof_field_type(
                    program,
                    literal.type_symbol,
                    literal.case_symbol,
                    field.field_symbol,
                )?;
                let term = if let Some(primitive) = program.primitive_type_reference(declared) {
                    CheckedProofTerm::Scalar(lower_return_expression(
                        program,
                        operators,
                        field.value,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        primitive,
                        exact_integer_casts,
                    )?)
                } else {
                    lower_proof_term(
                        program,
                        operators,
                        field.value,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                        proof_only,
                    )?
                };
                fields.push(CheckedProofTermField {
                    field_symbol: field.field_symbol,
                    term,
                });
            }
            let type_identity = program
                .type_reference_table
                .find_named_type_reference(literal.type_symbol)
                .map(|reference| {
                    program
                        .type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
                            reference,
                        ))
                        .into_string()
                })?;
            Some(CheckedProofTerm::Construction {
                data_symbol: literal.type_symbol,
                type_identity,
                case_symbol: literal.case_symbol,
                fields,
            })
        }
        ExpressionNode::Name(path) => {
            let position = crate::values::scalar::expression_facts::parameter_position(
                program,
                path,
                authored_parameters,
            )?;
            let parameter = authored_parameters.get(position)?;
            (parameter.relevance.is_erased()
                && proof_only.contract_term_carrier(program, parameter.type_reference))
            .then_some(CheckedProofTerm::Formal {
                parameter_symbol: parameter.symbol,
            })
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_direct_call_binding_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<LoweredCallArguments> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    crate::semantic_calls::find_machine_by_entry_state(program, call.target_symbol)?;
    let target_parameters =
        crate::semantic_calls::call_target_parameters(program, call.target_symbol)?;
    if target_parameters.iter().any(|parameter| {
        parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
    }) {
        return None;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    // Erased positions own no `CallArgument` ordinal; the retained ones are
    // numbered densely to match the callee's stripped scalar signature.
    if target_parameters.iter().any(|target_parameter| {
        crate::execution::terminal_unit::strips_erased_parameter(target_parameter).is_none()
    }) {
        return None;
    }
    let mut argument_ordinal = 0u32;
    let mut erased_ordinal = 0u32;
    let mut proof_erased_ordinal = 0u32;
    let proof_only = typed_trees::proof_only::classify(program);
    let mut scalar_arguments = Vec::new();
    let mut proof_terms = Vec::new();
    for (argument, target_parameter) in arguments.iter().zip(target_parameters) {
        let Some(expected_type) = program.primitive_type_reference(target_parameter.type_reference)
        else {
            // A contract-term erased carrier feeds the term lane with its own
            // dense ordinal; every other non-scalar callee shape stays
            // outside this direct-binding plan.
            if !target_parameter.relevance.is_erased()
                || !proof_only.contract_term_carrier(program, target_parameter.type_reference)
            {
                return None;
            }
            let ordinal = proof_erased_ordinal;
            proof_erased_ordinal = proof_erased_ordinal.checked_add(1)?;
            proof_terms.push(CheckedLocatedProofTerm {
                state,
                statement_ordinal,
                role: CheckedProofTermRole::ErasedCallArgument {
                    binding_ordinal,
                    erased_ordinal: ordinal,
                },
                expression: *argument,
                term: lower_proof_term(
                    program,
                    operators,
                    *argument,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    &proof_only,
                )?,
            });
            continue;
        };
        let role = if target_parameter.relevance.is_erased() {
            let ordinal = erased_ordinal;
            erased_ordinal = erased_ordinal.checked_add(1)?;
            CheckedScalarExpressionRole::ErasedCallArgument {
                binding_ordinal,
                erased_ordinal: ordinal,
            }
        } else {
            let ordinal = argument_ordinal;
            argument_ordinal = argument_ordinal.checked_add(1)?;
            CheckedScalarExpressionRole::CallArgument {
                binding_ordinal,
                argument_ordinal: ordinal,
            }
        };
        scalar_arguments.push((
            *argument,
            CheckedLocatedScalarExpression {
                state,
                statement_ordinal,
                role,
                expression: lower_return_expression(
                    program,
                    operators,
                    *argument,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    expected_type,
                    exact_integer_casts,
                )?,
            },
        ));
    }
    Some(LoweredCallArguments {
        scalar_arguments,
        proof_terms,
    })
}

/// Locate a call beneath only same-carrier integer qualifications. The actual
/// callee declaration supplies that carrier; no conversion result is guessed.
pub(crate) fn scalar_qualified_call_expression(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut targets = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
            {
                targets.push(program.primitive_type_reference(cast.target_type)?);
                expression = cast.value;
            }
            ExpressionNode::Call(call) => {
                let state = crate::semantic_calls::find_state(program, call.target_symbol)?;
                let primitive = program.primitive_type_reference(state.return_type)?;
                return (is_integer(primitive)
                    && primitive != PrimitiveType::Addr
                    && targets.iter().all(|target| *target == primitive))
                .then_some(expression);
            }
            _ => return None,
        }
    }
}
