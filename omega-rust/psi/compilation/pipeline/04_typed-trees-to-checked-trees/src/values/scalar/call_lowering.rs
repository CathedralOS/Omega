//! Lowering call arguments into scalar plans: retained arguments, boundary
//! calls, direct call bindings and qualified call expressions.

use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::expression_plans::{ScalarLocal, located_binding_symbols};
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::{
    CheckedLocatedProofTerm, CheckedLocatedScalarExpression, CheckedOperatorFacts,
    CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole, CheckedScalarExpression,
    CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLanding;
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
            symbols: binding_symbols.insert_many(located_binding_symbols(parameters, locals)),
        });
        expressions.push(argument);
    }
}

pub(crate) fn call_is_boundary(
    program: &dyn ::validation::CheckingInput,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    let requirement_symbol = program
        .machine_parameter_signature(target_symbol)
        .map_or(target_symbol, |(_, signature)| signature.symbol);
    program.machines().iter().any(|machine| {
        machine.has_bodyless_signature()
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_symbol)
    }) || program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signature_by_symbol(definition, requirement_symbol)
                .is_some()
    }) || program
        .operators()
        .iter()
        .any(|operator| operator.is_boundary && operator.symbol == target_symbol)
        || validation::exact_external_leaf_boundary_requirement(program, target_symbol).is_some()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_call_arguments(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    call_ordinal: usize,
    call_site: &validation::CallSite<'_>,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<LoweredCallArguments> {
    let target_symbol = match call_site {
        validation::CallSite::Statement(call) => call.target_symbol,
        validation::CallSite::Expression { call, .. } => {
            validation::call_target_symbol(program, call)
        }
        validation::CallSite::TransitionNamed { .. } => return None,
    };
    let is_boundary = call_is_boundary(program, target_symbol);

    if let Some(operation) = program
        .symbols
        .builtin_function_for_symbol(target_symbol)
        .and_then(|builtin| {
            language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                builtin.name(),
            )
        })
    {
        let arguments = validation::call_site_argument_expressions(program, call_site);
        if arguments.len() != operation.input_carriers().len() {
            return None;
        }
        let mut scalar_arguments = Vec::new();
        for (ordinal, (argument, carrier)) in
            arguments.iter().zip(operation.input_carriers()).enumerate()
        {
            if let Some(expression) = lower_return_expression(
                program,
                operators,
                *argument,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                PrimitiveType::from_name(carrier.name())?,
                exact_integer_casts,
            ) {
                scalar_arguments.push((
                    *argument,
                    CheckedLocatedScalarExpression {
                        state: state.symbol,
                        statement_ordinal,
                        role: CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        expression,
                    },
                ));
            }
        }
        return Some(LoweredCallArguments {
            scalar_arguments,
            proof_terms: Vec::new(),
        });
    }

    let target_parameters = validation::call_target_parameters(program, target_symbol)?;
    let explicit_arguments = validation::call_site_argument_expressions(program, call_site);
    let mut explicit_index = 0usize;
    let mut scalar_index = 0usize;
    let mut scalar_erased_index = 0usize;
    let mut proof_erased_index = 0usize;
    let proof_only = validation::proof_only_classification(program);
    let mut proof_terms = Vec::new();
    let mut output = Vec::new();
    for target in target_parameters {
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
                if let Some(expected_type) =
                    program.erased_scalar_observation_type(target.type_reference)
                {
                    if let Some(lowered) = lower_erased_scalar_argument(
                        state.symbol,
                        statement_ordinal,
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
                        target.type_reference,
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
            continue;
        };
        if (target.is_const)
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
    if explicit_index != explicit_arguments.len() {
        return None;
    }
    // A dynamic-family call's generic actuals are row-selection operands,
    // not callee formals: each roster binder's actual is evaluated beside the
    // call under its own family ordinal so lowering can compare it against
    // the declared tuple identities. Only a slot whose requirement declares a
    // finite family accepts a nonempty machine-argument roster here; any
    // other generic argument retains its own admission path.
    let machine_arguments: &[typed_trees::expression::StaticMachineArgument] = match call_site {
        validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
        validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
        validation::CallSite::TransitionNamed { .. } => &[],
    };
    if machine_arguments.is_empty() {
        return Some(LoweredCallArguments {
            scalar_arguments: output,
            proof_terms,
        });
    }
    let slot = program.dynamic_slot_by_symbol(target_symbol)?;
    let requirement = program.dynamic_slot_requirement(slot)?;
    let typed_trees::finite_family::FamilyProbe::Finite { arity, .. } =
        program.finite_signature_family(requirement)
    else {
        return None;
    };
    let binders = program
        .state_signature_type_parameters(requirement)
        .iter()
        .filter(|parameter| parameter.kind.is_const_or_value())
        .collect::<Vec<_>>();
    if arity != binders.len() || machine_arguments.len() != arity {
        return None;
    }
    for (family_ordinal, (argument, binder)) in
        machine_arguments.iter().zip(binders.iter()).enumerate()
    {
        let expected_type = program.primitive_type_reference(match binder.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => type_reference,
            _ => return None,
        })?;
        let expression =
            family_argument_expression(program, argument, parameters, locals, expected_type)?;
        output.push((
            typed_trees::expression::ExpressionHandle::default(),
            CheckedLocatedScalarExpression {
                state: state.symbol,
                statement_ordinal,
                role: CheckedScalarExpressionRole::DynamicFamilyArgument {
                    call_ordinal: u32::try_from(call_ordinal).ok()?,
                    family_ordinal: u32::try_from(family_ordinal).ok()?,
                },
                expression,
            },
        ));
    }
    Some(LoweredCallArguments {
        scalar_arguments: output,
        proof_terms,
    })
}

/// The evaluated operand one finite-family actual contributes: a closed
/// const lands at the binder's declared primitive type, and an ordinary
/// runtime subject reads its dense scalar parameter, value-binding, or
/// storage position -- the same reading the subject's name would produce as
/// an explicit argument. Anything else owns no row-selection operand.
fn family_argument_expression(
    program: &dyn ::validation::CheckingInput,
    argument: &typed_trees::expression::StaticMachineArgument,
    scalar_parameters: &[StateParameter],
    locals: &[ScalarLocal],
    expected_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    if let Some(literal) = &argument.const_literal {
        let landed_type = expected_type.landed_landing()?;
        return Some(CheckedScalarExpression::IntegerLiteral {
            literal: literal.clone().with_landing(IntegerLanding {
                landed_type,
                domain: ArithmeticDomain::Exact,
            }),
        });
    }
    if !validation::static_arguments::is_runtime_value_subject(program, argument)
        || !argument.symbol.is_valid()
    {
        return None;
    }
    if let Some(position) = scalar_parameters
        .iter()
        .position(|parameter| parameter.symbol == argument.symbol)
    {
        return Some(CheckedScalarExpression::Parameter {
            position,
            primitive_type: expected_type,
        });
    }
    let mut value_position = 0usize;
    for local in locals {
        if !local.is_value_binding() {
            if local.symbol == argument.symbol {
                return Some(CheckedScalarExpression::StorageRead {
                    symbol: local.symbol,
                    primitive_type: expected_type,
                });
            }
            continue;
        }
        if local.symbol == argument.symbol {
            return Some(CheckedScalarExpression::Local {
                position: value_position,
                primitive_type: expected_type,
            });
        }
        value_position += 1;
    }
    None
}

/// The declared type of one struct-literal field: the data member carrying
/// `field_symbol`, or the selected case's payload field when the literal
/// constructs a case.
fn proof_field_type(
    program: &dyn ::validation::CheckingInput,
    data_symbol: symbols::SymbolHandle,
    case_symbol: Option<symbols::SymbolHandle>,
    field_symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let definition = program.data_definition_by_symbol(data_symbol)?;
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
                    .data_payload_field_by_symbol(variant, field_symbol)
                    .map(|field| field.type_reference)
            }
            _ => None,
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_call_scalar_argument(
    erased: bool,
    state_symbol: symbols::SymbolHandle,
    before_statement: u32,
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    expected_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    if erased {
        lower_erased_scalar_argument(
            state_symbol,
            before_statement,
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        )
    } else {
        lower_return_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        )
    }
}

/// Form a proof-side observation contingent on final source admission of the
/// actual's type, scope, borrow and callee precondition. A borrow names its
/// exact referent expression here; it never forms an executable descriptor or
/// load. Located argument rows retain the original authored expression.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_erased_scalar_argument(
    state_symbol: symbols::SymbolHandle,
    before_statement: u32,
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    expected_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    let observation = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            borrow.target
        }
        _ => expression,
    };
    let observation =
        super::erased_observations::resolve(program, state_symbol, before_statement, observation)?;
    lower_return_expression(
        program,
        operators,
        observation,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        expected_type,
        exact_integer_casts,
    )
}

/// Lower one erased contract-term actual: a construction (or a bare
/// reference to the caller's own erased proof-lane formal) becomes the
/// checked proof term the callee's contract lane carries. A construction
/// field whose declared type is primitive lowers through the scalar lane
/// into a `Scalar` leaf — the erased record carrier's runtime payload is
/// retained as a term, not a runtime operand.

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_proof_term(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    expected_type: typed_trees::types::TypeReferenceHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> Option<CheckedProofTerm> {
    // The checked destination provides an array's element type and exact extent.
    // Recursive payload lowering uses the same operation for record fields and
    // array elements; neither creates a runtime value or a second call.
    if let Some(primitive) = program.primitive_type_reference(expected_type) {
        return Some(CheckedProofTerm::Scalar(lower_return_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            primitive,
            exact_integer_casts,
        )?));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(elements) => {
            let typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(expected_type)
            else {
                return None;
            };
            let elements = program.expression_table.expression_handles(*elements);
            if elements.len() != *length {
                return None;
            }
            Some(CheckedProofTerm::Array {
                type_identity: program.type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(expected_type),
                ),
                elements: elements
                    .iter()
                    .map(|element| {
                        lower_proof_term(
                            program,
                            operators,
                            *element,
                            *element_type,
                            parameters,
                            authored_parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                            proof_only,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
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
                let term = lower_proof_term(
                    program,
                    operators,
                    field.value,
                    declared,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    proof_only,
                )?;
                fields.push(CheckedProofTermField {
                    field_symbol: field.field_symbol,
                    term,
                });
            }
            let type_identity = program
                .type_reference_table
                .find_named_type_reference(literal.type_symbol)
                .map(|reference| {
                    program.type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(reference),
                    )
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
    program: &dyn ::validation::CheckingInput,
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
    let call = program.expression_table.call_node(expression)?;
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    validation::find_machine_by_entry_state(program, call.target_symbol)?;
    let target_parameters = validation::call_target_parameters(program, call.target_symbol)?;
    if target_parameters.iter().any(|parameter| {
        (parameter.is_const)
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
    let proof_only = validation::proof_only_classification(program);
    let mut scalar_arguments = Vec::new();
    let mut proof_terms = Vec::new();
    for (argument, target_parameter) in arguments.iter().zip(target_parameters) {
        let Some(expected_type) = (if target_parameter.relevance.is_erased() {
            program.erased_scalar_observation_type(target_parameter.type_reference)
        } else {
            program.primitive_type_reference(target_parameter.type_reference)
        }) else {
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
                    target_parameter.type_reference,
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
                expression: lower_call_scalar_argument(
                    target_parameter.relevance.is_erased(),
                    state,
                    statement_ordinal,
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

/// The call a value expression evaluates, whether spelled directly or reached
/// beneath same-carrier integer casts — the two custody spellings a statement
/// can write for `CallSite::Expression` argument rows.
pub(crate) fn value_call_expression(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
) -> Option<ExpressionHandle> {
    scalar_qualified_call_expression(program, value).or_else(|| {
        matches!(
            program.expression_table.expression(value),
            ExpressionNode::Call(_)
        )
        .then_some(value)
    })
}

/// Locate a call beneath only same-carrier integer qualifications. The actual
/// callee declaration supplies that carrier; no conversion result is guessed.
pub(crate) fn scalar_qualified_call_expression(
    program: &dyn ::validation::CheckingInput,
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
                let state = program.state_by_symbol(call.target_symbol)?;
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
