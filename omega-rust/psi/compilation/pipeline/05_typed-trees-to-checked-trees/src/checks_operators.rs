/*
    Checked operator resolution: we report inadmissible and ambiguous operator
    selections, check destination and float-semantic operator contracts, and
    prove an operator's authored `requires` clauses at each use.
*/

use checked_trees::{CheckFacts, CheckedOperatorResolutionIssue};
use diagnostics::Diagnostic;

use crate::flow::expressions;
use crate::published::labels::symbol_name;

mod destinations {
    /*
        Destination compatibility consumes finalized operator selection. Operand
        formats describe a builtin result only after selection retained that builtin.
    */
    use crate::flow::{indexed_statements, resolved_operator_statement_symbol};
    use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus, CheckedValueOrigin};
    use diagnostics::Diagnostic;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    pub(super) fn check(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    let mut check = |value, destination| {
                        check_destination(
                            program,
                            facts,
                            value,
                            destination,
                            state.symbol,
                            statement_index,
                            &mut diagnostics,
                        );
                    };
                    match statement {
                        StatementNode::Call(call) => {
                            if let Some(symbol) = resolved_operator_statement_symbol(program, call)
                                && let Some(operator) =
                                    typed_trees::operator::declaration_by_symbol(program, symbol)
                            {
                                let arguments =
                                    program.statement_table.expression_handles(call.arguments);
                                for (argument, parameter) in
                                    arguments.iter().zip(positional_operator_parameters(
                                        program.operator_parameters(operator),
                                        arguments.len(),
                                    ))
                                {
                                    check(*argument, parameter.type_reference);
                                }
                            }
                        }
                        StatementNode::LocalData(local) => {
                            check(local.initial_value, local.type_reference)
                        }
                        StatementNode::Assignment(assignment) => {
                            if let Some(destination) = validation::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            ) {
                                let destination = match program
                                    .type_reference_table
                                    .type_reference(destination)
                                {
                                    TypeReferenceNode::Reference { referee, .. } => *referee,
                                    _ => destination,
                                };
                                check(assignment.value, destination);
                            }
                        }
                        StatementNode::Expression(value) => check(*value, state.return_type),
                        StatementNode::Transition(transition) => {
                            for target in transition.targets() {
                                if let TransitionTargetNode::Value(value) =
                                    program.statement_table.transition_target(target)
                                {
                                    check(*value, state.return_type);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        for (_, state) in facts.flow.control.states.iter() {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let Some(site) = validation::find_call_site(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let Some(parameters) =
                    validation::call_target_parameters(program, call.target_symbol)
                else {
                    continue;
                };
                for (argument, parameter) in
                    validation::call_site_argument_expressions(program, &site)
                        .iter()
                        .zip(parameters.iter())
                {
                    check_destination(
                        program,
                        facts,
                        *argument,
                        parameter.type_reference,
                        state.state_symbol,
                        call.statement_index,
                        &mut diagnostics,
                    );
                }
            }
        }
        for (_, selected) in facts.operators.named_uses.iter() {
            let CheckedValueOrigin::StateStatement {
                state_symbol,
                statement_index,
                ..
            } = selected.origin
            else {
                continue;
            };
            let Some(call) = program.expression_table.call_node(selected.expression) else {
                continue;
            };
            let Some(operator) = typed_trees::operator::declaration_by_symbol(
                program,
                selected.selected_operator_symbol,
            ) else {
                continue;
            };
            let arguments = program.expression_table.expression_handles(call.arguments);
            for (argument, parameter) in arguments.iter().zip(positional_operator_parameters(
                program.operator_parameters(operator),
                arguments.len(),
            )) {
                check_destination(
                    program,
                    facts,
                    *argument,
                    parameter.type_reference,
                    state_symbol,
                    statement_index,
                    &mut diagnostics,
                );
            }
        }
        diagnostics
    }

    fn positional_operator_parameters(
        parameters: &[typed_trees::signature::StateParameter],
        argument_count: usize,
    ) -> impl Iterator<Item = &typed_trees::signature::StateParameter> {
        let receiver = (parameters.len().checked_sub(argument_count) == Some(1)).then_some(0);
        parameters
            .iter()
            .enumerate()
            .filter_map(move |(index, parameter)| (Some(index) != receiver).then_some(parameter))
    }

    fn check_destination(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        value: ExpressionHandle,
        destination: TypeReferenceHandle,
        state: symbols::SymbolHandle,
        statement: usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !value.is_valid() || !destination.is_valid() {
            return;
        }
        let destination = match program.type_reference_table.type_reference(destination) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => destination,
        };
        if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
            && let Some(data) = program.data_definition_by_symbol(literal.type_symbol)
        {
            for field in program.expression_table.struct_fields(literal.fields) {
                let field_type =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(declared)
                                if declared.symbol == field.field_symbol =>
                            {
                                Some(declared.type_reference)
                            }
                            typed_trees::data::DataMember::Variant(variant)
                                if Some(variant.symbol) == literal.case_symbol =>
                            {
                                program
                                    .data_payload_field_by_symbol(variant, field.field_symbol)
                                    .map(|declared| declared.type_reference)
                            }
                            _ => None,
                        });
                if let Some(field_type) = field_type {
                    check_destination(
                        program,
                        facts,
                        field.value,
                        field_type,
                        state,
                        statement,
                        diagnostics,
                    );
                }
            }
            return;
        }
        if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
            && let TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(destination)
        {
            for element in program.expression_table.expression_handles(*elements) {
                check_destination(
                    program,
                    facts,
                    *element,
                    *element_type,
                    state,
                    statement,
                    diagnostics,
                );
            }
            return;
        }
        let Some(target @ (PrimitiveType::F32 | PrimitiveType::F64)) =
            primitive(program, destination)
        else {
            return;
        };
        let mut expression = value;
        loop {
            expression = match program.expression_table.expression(expression) {
                ExpressionNode::Atomic(atomic) => atomic.value,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => break,
            };
        }
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Binary(_)
        ) {
            return;
        }
        if let Some(source @ (PrimitiveType::F32 | PrimitiveType::F64)) =
            result_primitive(program, facts, expression, state, statement)
            && source != target
        {
            diagnostics.push(Diagnostic::error(format!(
                "operator expression `{}` delivers a `{}` result to a `{}` destination; changing a landed float's format requires an explicit conversion",
                program.expression_table.display_name(expression), source.name(), target.name(),
            )));
        }
    }

    fn result_primitive(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        expression: ExpressionHandle,
        state: symbols::SymbolHandle,
        statement: usize,
    ) -> Option<PrimitiveType> {
        let origin = CheckedValueOrigin::StateStatement {
            machine_symbol: program.symbols.get(state).parent,
            state_symbol: state,
            statement_index: statement,
            role: checked_trees::CheckedValueStatementRole::Expression,
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                result_primitive(program, facts, atomic.value, state, statement)
            }
            ExpressionNode::Unary(unary) => {
                result_primitive(program, facts, unary.operand, state, statement)
            }
            ExpressionNode::Binary(binary) => {
                let selected = facts.operators.uses.iter().map(|(_, operator)| operator).find(|operator| {
                    operator.expression == expression && matches!(operator.origin, CheckedValueOrigin::StateStatement { state_symbol, statement_index, .. } if state_symbol == state && statement_index == statement)
                });
                if let Some(selected) = selected {
                    match selected.status {
                        CheckedOperatorResolutionStatus::Resolved => {
                            return facts
                                .operators
                                .selected_candidate(selected)
                                .and_then(|candidate| primitive(program, candidate.return_type));
                        }
                        CheckedOperatorResolutionStatus::BuiltinFallback => {}
                        _ => return None,
                    }
                }
                match binary.operator {
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => {
                        let left = result_primitive(program, facts, binary.left, state, statement);
                        let right =
                            result_primitive(program, facts, binary.right, state, statement);
                        match (left, right) {
                            (Some(left), Some(right)) if left == right => Some(left),
                            (Some(value), None) | (None, Some(value)) => Some(value),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
            ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
                numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
            }),
            ExpressionNode::Call(call) => {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
                    .or_else(|| {
                        program
                            .state_by_symbol(call.target_symbol)
                            .map(|state| state.return_type)
                    })
                    .or_else(|| {
                        program
                            .machine_parameter_signature(call.target_symbol)
                            .map(|(_, signature)| signature.return_type)
                    })
                    .or_else(|| {
                        program
                            .traits()
                            .iter()
                            .flat_map(|definition| program.trait_machine_signatures(definition))
                            .find(|signature| signature.symbol == call.target_symbol)
                            .map(|signature| signature.return_type)
                    })
                    .and_then(|reference| primitive(program, reference))
            }
            _ => validation::expression_type_reference_for_origin(program, expression, origin)
                .and_then(|reference| primitive(program, reference)),
        }
    }

    fn primitive(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => primitive(program, *base_type),
            _ => program.primitive_type_reference(reference),
        }
    }
}
mod float_semantic {
    /*
        Transient resolved terms for source float predicate entailment.

        Public total Boolean queries expose sealed semantic equations. Observed
        truth transports through the selected equation over exact source identities,
        not rendered names. Quiet float results compose their selected denotations;
        adapted results contribute no unconditional raw equation. Proof-position
        operations are resolved in the fact's own typed frame: they have no executable
        operator-use row or operand capture to fabricate.
        Integer catalog inputs have their own carrier and completed-value authority;
        they never occupy a FloatMeaning projection. The selected equation retains
        its formal telescope while captured actuals retain their independent caller
        owner, so composing a conversion cannot reinterpret a source operation in
        the callee's scope or replace its arithmetic policy.
    */
    use crate::flow::expressions;
    use crate::published::facts::operator_crashes::named_call_operands;
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
        /*
            An admitted captured Integer value keeps its own policy and source
            frame. Its carrier image can prove a classification consequence without
            asserting a value or replaying the producer's arithmetic as mathematics.
        */
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
        /*
            A non-place value without a retained semantic equation keeps its exact
            occurrence identity; another written expression cannot impersonate it.
        */
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
        boolean_semantic_term(program, facts, expression, frame, None, frame).as_ref()
            == Some(required)
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

    /*
        Closed caller obligations share the observed-predicate term owner. Actuals
        are explicit source bindings, separate from a selected public query's own
        formals; composing a raw equation cannot erase the outer call telescope.
        Open Float storage stays a projection. Integer sources keep their own
        bounded carrier and captured caller frame.
    */
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

    /*
        Decide a selected Boolean denotation from closed values or a kernel-owned
        ordered image. A symbolic Integer source supplies its own finite carrier,
        not a literal or a fact about arbitrary Float storage.
    */
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

    /*
        The admitted source's image under the selected monotone Integer conversion.
        This is an enclosure query over resolved terms, not another source evaluator.
    */
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

    /*
        The selected equation owns its formal types as well as their substitutions.
        The outer call telescope is independent: replacing this scope during raw
        result composition must not erase bindings of captured caller arguments.
    */
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
                    let reference = validation::expression_result_type_reference(
                        program, machine, state, source,
                    )?;
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
                    let Some(operand) = actuals.and_then(|actuals| actuals.operand(path.symbol))
                    else {
                        return Some(expression);
                    };
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

    /*
        Integer carrier qualification participates in its closed operations. Unlike
        float projection erasure, substitute the exact source without stripping its
        casts; the shared numeric evaluator owns their denotation and result carrier.
    */
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
                let members = expressions(program).name_path_member_symbols(path.member_symbols);
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
                let FloatSourceValue::Place(mut path) = source_value(program, member.receiver)?
                else {
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
                named_call_operands(
                    program,
                    program.expression_table.call_node(expression)?,
                    program.operator_parameters(operator),
                )?
            };
            return (operands.len() == program.operator_parameters(operator).len())
                .then_some((operator, operands, adapter));
        }
        let frame = frame?;
        let (operator, operands) = match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                let operator = typed_trees::operator::resolve_named_expression_call(program, call)
                    .or_else(|| {
                        crate::shared::authored_selections::checked_named_operator_call(
                            program,
                            facts,
                            expression,
                            call,
                            program.expression_table.source_span(expression),
                            None,
                        )
                    })?;
                let operands =
                    named_call_operands(program, call, program.operator_parameters(operator))?;
                (operator, operands)
            }
            node @ ExpressionNode::Binary(binary) => {
                let crate::shared::authored_selections::CheckedContractOperatorResolution::Declaration(
                    selected,
                ) = crate::shared::authored_selections::checked_operator_resolution(
                    program, facts, expression, node, None,
                )?
                else {
                    return None;
                };
                let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
                (operator, vec![binary.left, binary.right])
            }
            ExpressionNode::Unary(_) => return None,
            _ => return None,
        };
        if matches!(
            program.primitive_type_reference(operator.return_type),
            Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
        ) {
            let machine = program.machine_by_symbol(frame.machine)?;
            let state = program.state_in_machine(machine, frame.state)?;
            for operand in &operands {
                if matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Float(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Boolean(_)
                ) {
                    continue;
                }
                let reference = validation::expression_result_type_reference(
                    program, machine, state, *operand,
                )?;
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
}
pub(crate) use float_semantic::{
    context_proves_float_semantic_term, finite_float_semantic_term, float_boolean_term_value,
    instantiated_float_semantic_term, selected_float_boolean_requirement,
};
mod requires {
    /*
        Use-site discharge for `requires` contracts carried by the SELECTED meaning
        of a spelled binary operator, an implicit Match equality, or a named
        `Namespace::requirement(...)` operator call.

        Model: the bounds-from-`requires` seam for `[]`/`[..]`
        (checks/ranges/indexes/validation.rs) sources the access obligation from
        the spelled operator's `requires` contract and discharges it with the
        bounds prover. Spelled binary uses have no special-purpose prover, so the
        obligation is INSTANTIATED over the actual operands (parameter -> operand,
        the call-`requires` instantiation precedent in
        checks/contracts/labels/calls.rs) and proven against the semantic contexts
        at the exact invocation after operand effects, for both spelled operators
        and implicit Match comparisons. Statement-entry facts can describe overwritten
        storage or omit guarantees established by earlier operands. Copied scalars
        and by-value carriers with stable observable contents retain operand-time
        facts — the bound operand is a detached copy a later write cannot reach;
        view carriers such as references and slices additionally require those same
        facts to remain live at invocation. This conservative intersection prevents
        a later guarantee or reference rebind from impersonating a captured value.
        This query checks the already
        selected operator's precondition; it never participates in selecting the
        operator meaning itself.

        Named operator calls carry the same obligation. They produce no `uses`
        row, so their operand-time capture keys on the `named_use` handle and
        their operand list is reconstructed in operator-parameter order by
        `named_call_operands` — including an `is_self` or leading value receiver
        and skipping a static namespace receiver, which is a path, not an operand.
        A reference formal still reads its referent in the predicate, so operand
        labels come from `named_call_operand_labels`. A call whose evaluation
        emitted no capture row, or whose captured operand expressions do not match
        exactly, selects no contexts — the same fail-closed shape a spelled use
        gets for missing, duplicated, or substituted custody.

        An unproven obligation reports the operator-contract attribution shape the
        indexed seam established: name the instantiated clause, the operator that
        declares it, and the spelling that resolved to it, so the user can browse
        to the operator declaration and read the governing contract.
    */
    use arena::Handle;
    use std::cmp::Ordering;
    use typed_trees::data::TypeParameter;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::proposition::ProofSubstitutions;
    use validation::canonical_place_from_expression;
    use validation::field_domains::domain_membership_implies;

    use checked_trees::{
        CheckFacts, CheckedNamedOperatorUseFact, CheckedOperatorFacts, CheckedOperatorOccurrence,
        CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedValueOrigin, FlowFacts,
    };
    use diagnostics::Diagnostic;
    use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan, ProgramPoint};
    use language_core::operator_spelling::OperatorSpelling;
    use numerics::bignum::{BigInt, BigRational, ExactFloat, IeeeRounding};
    use numerics::literals::{FloatFormat, FloatLiteral};
    use symbols::SymbolHandle;
    use typed_trees::domain::{ProofFact, has_symbol_only_proof};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableNamePath,
        UnaryOperator,
    };
    use typed_trees::operator::{OperatorDefinition, declaration_by_symbol};
    use typed_trees::signature::{SignatureContractKind, StateParameter, state_parameter_position};
    use typed_trees::types::PrimitiveType;

    use crate::flow::expressions;
    use crate::published::labels::{
        context_proves_boolean_label, instantiate_operator_contract_expression_label, symbol_name,
    };

    mod invocation {
        /*
            A copied scalar's contract refers to its operand-time value, whereas a
            reference still denotes its captured referent's live storage at invocation.
            A by-value formal whose type has stable observable contents is an equally
            detached carrier: the bound operand is a copy no loan or interior authority
            can retarget, so its operand-time facts describe the invocation value
            outright and need no invocation-time liveness. Reference and slice formals
            are views onto shared storage: the operand's evaluated value denotes a
            fixed referent at its own completion, so exact referent custody transports
            the facts live on that storage at invocation — including newer facts a
            later operand established — instead of intersecting context identities.
            Operands without exact custody keep the capture ∩ invocation intersection.
            In particular, rebinding a source reference must not retarget an earlier
            copy. Select facts per clause
            leaf, intersecting exact context identities when a relation uses several
            operands. Unioning their snapshots could combine facts about different
            versions of the same source place. Conjunctions are proved leaf by leaf by
            the ordinary requires checker; they do not need all premises at one time.
        */
        use crate::published::facts::contract_occurrences::append_expression_occurrences;
        use arena::Handle;
        use checked_trees::{
            CheckFacts, CheckedNamedOperatorUseFact, CheckedOperatorUseFact, FlowCallFact,
            FlowFacts, FlowOperatorInvocationFact,
        };
        use facts::{FactContextHandle, FactPlan};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::StateParameter;

        pub(crate) struct InvocationContexts<'facts> {
            flow: &'facts FlowFacts,
            semantic: &'facts FactPlan,
            invocation: Option<CapturedInvocation<'facts>>,
        }

        enum CapturedInvocation<'facts> {
            Operator(&'facts FlowOperatorInvocationFact),
            Call {
                call: &'facts FlowCallFact,
                receiver_offset: usize,
            },
        }

        impl<'facts> InvocationContexts<'facts> {
            /*
                Ordinary state and machine calls share the scalar snapshot law with
                operator calls. The caller supplies the authenticated explicit telescope;
                a namespace receiver is evaluated but is not a formal parameter.
            */
            pub(crate) fn from_call(
                facts: &'facts CheckFacts,
                call: &'facts FlowCallFact,
                arguments: &[ExpressionHandle],
            ) -> Self {
                let operands = facts.flow.control.operands.span_or_empty(call.operands);
                let receiver_offset = operands.len().checked_sub(arguments.len());
                let invocation = receiver_offset
                    .filter(|offset| *offset <= 1)
                    .filter(|offset| {
                        call.authored_source_custody_valid
                            && operands[*offset..]
                                .iter()
                                .map(|operand| operand.expression)
                                .eq(arguments.iter().copied())
                    })
                    .map(|receiver_offset| CapturedInvocation::Call {
                        call,
                        receiver_offset,
                    });
                Self {
                    flow: &facts.flow,
                    semantic: &facts.semantic,
                    invocation,
                }
            }

            pub(crate) fn new(
                facts: &'facts CheckFacts,
                operator_use: Handle<CheckedOperatorUseFact>,
                operands: &[ExpressionHandle],
            ) -> Self {
                Self::from_flow(&facts.flow, &facts.semantic, operator_use, operands)
            }

            pub(crate) fn from_flow(
                flow: &'facts FlowFacts,
                semantic: &'facts FactPlan,
                operator_use: Handle<CheckedOperatorUseFact>,
                operands: &[ExpressionHandle],
            ) -> Self {
                Self::from_matching_invocation(flow, semantic, operands, |invocation| {
                    invocation.operator_use == operator_use
                })
            }

            fn from_matching_invocation(
                flow: &'facts FlowFacts,
                semantic: &'facts FactPlan,
                operands: &[ExpressionHandle],
                matches: impl Fn(&FlowOperatorInvocationFact) -> bool,
            ) -> Self {
                let mut matching = flow
                    .control
                    .operator_invocations
                    .iter()
                    .filter_map(|(_, invocation)| matches(invocation).then_some(invocation));
                let invocation = match (matching.next(), matching.next()) {
                    (Some(invocation), None)
                        if operand_expressions_match(flow, invocation, operands) =>
                    {
                        Some(CapturedInvocation::Operator(invocation))
                    }
                    _ => None,
                };
                Self {
                    flow,
                    semantic,
                    invocation,
                }
            }

            /*
                The operand-time capture of one named `Namespace::requirement(...)`
                call. Named uses produce no `uses` row, so their capture rows key on
                the `named_use` handle instead. The caller distinguishes "no capture
                was emitted" (entry-context fallback may apply) from present-but-bad
                custody: a mismatched or duplicated row selects no contexts, leaving
                only context-free truths.
            */
            pub(crate) fn from_named_use(
                flow: &'facts FlowFacts,
                semantic: &'facts FactPlan,
                named_use: Handle<CheckedNamedOperatorUseFact>,
                operands: &[ExpressionHandle],
            ) -> Self {
                Self::from_matching_invocation(flow, semantic, operands, |invocation| {
                    invocation.named_use == named_use
                })
            }

            /*
                Select the exact capture of a resolved expression occurrence. Spelled,
                named operator and ordinary requirement calls retain their own source
                identities while sharing the same detached-value/live-view context law.
            */
            pub(crate) fn for_selected_expression(
                program: &dyn ::validation::CheckingInput,
                facts: &'facts CheckFacts,
                expression: ExpressionHandle,
                selected: symbols::SymbolHandle,
                operands: &[ExpressionHandle],
            ) -> Self {
                Self::for_selected_expression_in_flow(
                    program,
                    &facts.operators,
                    &facts.flow,
                    &facts.semantic,
                    expression,
                    selected,
                    operands,
                )
            }

            /*
                The same exact occurrence binding while the checked facts are being
                assembled. Readers borrow the published owners independently; they do
                not manufacture a temporary aggregate or re-evaluate source operands.
            */
            pub(crate) fn for_selected_expression_in_flow(
                program: &dyn ::validation::CheckingInput,
                operators: &'facts checked_trees::CheckedOperatorFacts,
                flow: &'facts FlowFacts,
                semantic: &'facts FactPlan,
                expression: ExpressionHandle,
                selected: symbols::SymbolHandle,
                operands: &[ExpressionHandle],
            ) -> Self {
                let mut spelled = Vec::new();
                operators.for_each_expression_use(expression, |handle, fact| {
                    if fact.selected_operator_symbol == selected
                        && fact.status == checked_trees::CheckedOperatorResolutionStatus::Resolved
                        && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                    {
                        spelled.push((handle, fact));
                    }
                });
                let named = operators
                    .named_uses
                    .iter()
                    .filter(|(_, fact)| {
                        fact.expression == expression && fact.selected_operator_symbol == selected
                    })
                    .collect::<Vec<_>>();
                if let ([(handle, _)], []) = (spelled.as_slice(), named.as_slice()) {
                    return Self::from_flow(flow, semantic, *handle, operands);
                }
                if let ([], [(handle, _)]) = (spelled.as_slice(), named.as_slice()) {
                    return Self::from_named_use(flow, semantic, *handle, operands);
                }
                let mut calls = flow.control.calls.iter().filter_map(|(_, call)| {
                    (call.authored_expression == expression
                        && validation::find_machine_by_entry_state(program, call.authored_target)
                            .map(|(machine, _)| machine.symbol)
                            .unwrap_or(call.authored_target)
                            == selected
                        && call.authored_source_custody_valid
                        && flow
                            .control
                            .operands
                            .span_or_empty(call.operands)
                            .iter()
                            .map(|operand| operand.expression)
                            .eq(operands.iter().copied()))
                    .then_some(call)
                });
                let invocation = match (
                    spelled.is_empty(),
                    named.is_empty(),
                    calls.next(),
                    calls.next(),
                ) {
                    (true, true, Some(call), None) => Some(CapturedInvocation::Call {
                        call,
                        receiver_offset: 0,
                    }),
                    _ => None,
                };
                Self {
                    flow,
                    semantic,
                    invocation,
                }
            }

            pub(crate) fn entry_constraints(
                &self,
            ) -> Option<arena::HandleSpan<checked_trees::FlowConstraintRef>> {
                Some(match self.invocation.as_ref()? {
                    CapturedInvocation::Operator(invocation) => invocation.requires_constraints,
                    CapturedInvocation::Call { call, .. } => call.entry_constraints,
                })
            }

            pub(crate) fn for_expressions(
                &self,
                program: &dyn ::validation::CheckingInput,
                parameters: &[StateParameter],
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> Vec<FactContextHandle> {
                let Some(invocation) = &self.invocation else {
                    return Vec::new();
                };
                let mut occurrences = Vec::new();
                for expression in expressions {
                    append_expression_occurrences(program, expression, &mut occurrences);
                }
                let (operands, requires_constraints, receiver_offset) = match invocation {
                    CapturedInvocation::Operator(invocation) => {
                        (invocation.operands, invocation.requires_constraints, 0)
                    }
                    CapturedInvocation::Call {
                        call,
                        receiver_offset,
                    } => (call.operands, call.entry_constraints, *receiver_offset),
                };
                let operands = self.flow.control.operands.span_or_empty(operands);
                let mut selected = None::<Vec<FactContextHandle>>;
                for (ordinal, parameter) in parameters.iter().enumerate() {
                    if !occurrences.iter().any(|expression| {
                        occurrence_root_symbol(program, *expression) == parameter.symbol
                    }) {
                        continue;
                    }
                    let Some(operand) = operands.get(ordinal + receiver_offset) else {
                        return Vec::new();
                    };
                    let operand_contexts = crate::flow::operand_contexts(
                        program,
                        &self.flow.contexts,
                        &self.flow.control,
                        self.semantic,
                        operand,
                        parameter.type_reference,
                        requires_constraints,
                    );
                    if let Some(selected) = &mut selected {
                        selected.retain(|candidate| operand_contexts.contains(candidate));
                    } else {
                        selected = Some(operand_contexts);
                    }
                }
                selected.unwrap_or_else(|| {
                    self.flow
                        .semantic_constraint_contexts(requires_constraints)
                        .collect()
                })
            }

            pub(crate) fn flow(&self) -> &'facts FlowFacts {
                self.flow
            }

            /*
                A selected expression call shares ordinary calls' operation-entry
                proof owner. Its completed operands and saved Boolean observations can
                establish a requirement absent from the smaller semantic-context
                vocabulary. That query stops before this call's result and guarantees.
                Missing, duplicate or substituted capture still establishes nothing.
            */
            pub(crate) fn proves_selected_requirement(
                &self,
                program: &dyn ::validation::CheckingInput,
                origin: checked_trees::CheckedValueOrigin,
                source_expression: ExpressionHandle,
                required_expression: ExpressionHandle,
            ) -> bool {
                let Some(invocation) = &self.invocation else {
                    return false;
                };
                let checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                } = origin
                else {
                    return false;
                };
                let operands = match invocation {
                    CapturedInvocation::Operator(invocation) => invocation.operands,
                    CapturedInvocation::Call { call, .. } => call.operands,
                };
                let Some(operands) = self.flow.control.operands.span(operands) else {
                    return false;
                };
                if operands.iter().any(|operand| {
                    operand.state_symbol != state_symbol
                        || operand.statement_index != statement_index
                }) {
                    return false;
                }
                let Some((machine, state)) =
                    validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
                else {
                    return false;
                };
                let Some(call) = program.expression_table.call_node(source_expression) else {
                    return false;
                };
                validation::selected_value_call_requirement_entailed(
                    program,
                    machine,
                    state,
                    statement_index,
                    validation::CallSite::Expression {
                        expression: source_expression,
                        call,
                    },
                    required_expression,
                    false,
                    None,
                )
            }

            pub(crate) fn operand_at(
                &self,
                ordinal: usize,
            ) -> Option<&'facts checked_trees::FlowOperandFact> {
                let invocation = self.invocation.as_ref()?;
                let (operands, receiver_offset) = match invocation {
                    CapturedInvocation::Operator(invocation) => (invocation.operands, 0),
                    CapturedInvocation::Call {
                        call,
                        receiver_offset,
                    } => (call.operands, *receiver_offset),
                };
                self.flow
                    .control
                    .operands
                    .span_or_empty(operands)
                    .get(ordinal + receiver_offset)
            }
        }

        /*
            The capture row names its exact operand expressions in evaluation order;
            a row whose operands differ is substituted custody, not this invocation's.
        */
        fn operand_expressions_match(
            flow: &FlowFacts,
            invocation: &FlowOperatorInvocationFact,
            operands: &[ExpressionHandle],
        ) -> bool {
            flow.control
                .operands
                .span_or_empty(invocation.operands)
                .iter()
                .map(|operand| operand.expression)
                .eq(operands.iter().copied())
        }

        fn occurrence_root_symbol(
            program: &dyn ::validation::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> symbols::SymbolHandle {
            loop {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Name(path) => return path.symbol,
                    ExpressionNode::Member(member) => expression = member.receiver,
                    ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                    ExpressionNode::Borrow(borrow) => expression = borrow.target,
                    _ => return symbols::SymbolHandle::invalid(),
                }
            }
        }
    }
    pub(crate) use invocation::InvocationContexts;

    /*
        Crash refinement needs proof of falsity, not failure to prove a precondition.
        Only Boolean structure and exact evaluated predicate facts supply polarity;
        selected comparisons have no inferred arithmetic/complement laws here.
    */
    pub(crate) fn operator_route_is_false(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        flow: &FlowFacts,
        semantic: &FactPlan,
        operator_use: Handle<CheckedOperatorUseFact>,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> bool {
        let contexts = InvocationContexts::from_flow(flow, semantic, operator_use, operands);
        expression_has_polarity(
            program,
            point,
            semantic,
            &contexts,
            parameters,
            &operand_labels(program, operands),
            expression,
            false,
        )
    }

    /*
        The same discharge for a named `Namespace::requirement(...)` call whose
        evaluation emitted an operand-time capture row keyed by its `named_use`
        handle. The selection rules are identical to the spelled path: operand
        expressions must match exactly, scalar carriers use their operand-time
        snapshots, and other carriers intersect those snapshots with facts live
        at invocation. The caller supplies the operand labels: a reference formal
        used as a predicate value reads its referent, so a `&x` operand names
        `x`. A named use whose statement produced no capture has no row here at
        all; the caller applies its entry-context fallback instead.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn named_operator_route_is_false(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        flow: &FlowFacts,
        semantic: &FactPlan,
        named_use: Handle<CheckedNamedOperatorUseFact>,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        operand_labels: &[String],
        expression: ExpressionHandle,
    ) -> bool {
        let contexts = InvocationContexts::from_named_use(flow, semantic, named_use, operands);
        expression_has_polarity(
            program,
            point,
            semantic,
            &contexts,
            parameters,
            operand_labels,
            expression,
            false,
        )
    }

    /*
        Only an executable occurrence supplies a caller payload namespace. Other
        origins have no invocation field premises to expand.
    */
    fn operator_consumer_point(origin: CheckedValueOrigin) -> ProgramPoint {
        match origin {
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } => ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            _ => ProgramPoint::Global,
        }
    }

    /*
        Operand labels as the spelled surface renders them. The named-call seam
        supplies its own referent-naming labels instead.
    */
    fn operand_labels(
        program: &dyn ::validation::CheckingInput,
        operands: &[ExpressionHandle],
    ) -> Vec<String> {
        operands
            .iter()
            .map(|operand| program.render_proof_expression(*operand, ProofSubstitutions::None))
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn expression_has_polarity(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        semantic: &FactPlan,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        operand_labels: &[String],
        expression: ExpressionHandle,
        polarity: bool,
    ) -> bool {
        crate::published::labels::boolean_expression_has_polarity(
            program,
            expression,
            polarity,
            &mut |expression, polarity| {
                leaf_has_polarity(
                    program,
                    point,
                    semantic,
                    contexts,
                    parameters,
                    operand_labels,
                    expression,
                    polarity,
                )
            },
        )
    }

    fn leaf_has_polarity(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        semantic: &FactPlan,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        operand_labels: &[String],
        expression: ExpressionHandle,
        polarity: bool,
    ) -> bool {
        if polarity {
            return contexts_prove_boolean_leaf(
                program,
                point,
                semantic,
                contexts,
                parameters,
                operand_labels,
                expression,
            );
        }
        let required = instantiate_operator_contract_expression_label(
            program,
            parameters,
            operand_labels,
            expression,
        );
        contexts
            .for_expressions(program, parameters, [expression])
            .iter()
            .any(|context| {
                semantic
                    .context_view(semantic.contexts.get(*context))
                    .facts()
                    .any(|fact| {
                        matches!(fact.payload, FactPayload::BooleanValue { expression, value: false }
                    if program.expression_table.display_name(expression) == required)
                    })
            })
    }

    /*
        Checks selected binary, implicit comparison, and named-call preconditions
        against their available invocation facts, reporting each unproven clause.
        Slice `[]`/`[..]` uses discharge through the ranges seam and are
        deliberately excluded — including a named call whose selected operator
        carries that spelling.
    */
    pub(super) fn selected_binary_requires_diagnostics(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (operator_use_handle, operator_use) in facts.operators.uses.iter() {
            if operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved {
                continue;
            }
            if let CheckedOperatorOccurrence::MatchEquality { source_arm } = operator_use.occurrence
                && let ExpressionNode::Match(dispatch) =
                    program.expression_table.expression(operator_use.expression)
                && let Some(ordinal) = source_arm
                    .arena_index()
                    .checked_sub(dispatch.arms.start().arena_index())
                && expressions(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .take(ordinal as usize)
                    .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard))
            {
                continue;
            }
            if matches!(
                operator_use.spelling,
                OperatorSpelling::Index | OperatorSpelling::Range
            ) && match program.expression_table.expression(operator_use.expression) {
                ExpressionNode::Indexed(indexed) => {
                    crate::checks::ranges::ranges_seam_owns(program, indexed, operator_use.origin)
                }
                _ => true,
            } {
                continue;
            }
            let Some(selected) = facts.operators.selected_candidate(operator_use) else {
                continue;
            };
            let requires_facts: Vec<&ProofFact> = program
                .signature_contracts
                .span_or_empty(selected.contracts)
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
                .collect();
            if requires_facts.is_empty() {
                continue;
            }

            let operator = operator_definition(program, selected.operator_symbol);
            let parameters = operator
                .map(|operator| program.operator_parameters(operator))
                .unwrap_or(&[]);
            let type_parameters = operator
                .map(|operator| program.operator_type_parameters(operator))
                .unwrap_or(&[]);
            let operands = operator_use.operands(program).unwrap_or_default();
            let operand_labels = operand_labels(program, &operands);
            let invocation_contexts =
                InvocationContexts::new(facts, operator_use_handle, &operands);
            let point = operator_consumer_point(operator_use.origin);

            check_requires_facts(
                program,
                point,
                facts,
                &invocation_contexts,
                operator_use.origin,
                operator_use.expression,
                parameters,
                type_parameters,
                &operands,
                &operand_labels,
                requires_facts,
                &mut diagnostics,
                |fact| {
                    Diagnostic::error(format!(
                        "cannot prove `{}` — the `requires` of `{}` (spelled `{}`)",
                        requires_clause_label(program, parameters, &operand_labels, fact),
                        operator_path_label(
                            program,
                            operator,
                            selected.domain_symbol,
                            selected.operator_symbol
                        ),
                        operator_use.spelling.symbol(),
                    ))
                },
            );
        }

        for (named_use_handle, named_use) in facts.operators.named_uses.iter() {
            let Some(operator) = declaration_by_symbol(program, named_use.selected_operator_symbol)
            else {
                continue;
            };
            if matches!(
                operator.spelling,
                Some(OperatorSpelling::Index | OperatorSpelling::Range)
            ) {
                continue;
            }
            let requires_facts: Vec<&ProofFact> = program
                .signature_contracts
                .span_or_empty(operator.contracts)
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
                .collect();
            if requires_facts.is_empty() {
                continue;
            }
            let invalid = || {
                Diagnostic::error(
                    "selected named operator call requires exact selected meaning and ordered operand capture",
                )
            };
            let Some(call) = program.expression_table.call_node(named_use.expression) else {
                diagnostics.push(invalid());
                continue;
            };
            let parameters = program.operator_parameters(operator);
            let type_parameters = program.operator_type_parameters(operator);
            let Some(operands) = crate::published::facts::operator_crashes::named_call_operands(
                program, call, parameters,
            ) else {
                diagnostics.push(invalid());
                continue;
            };
            let operand_labels =
                crate::published::facts::operator_crashes::named_call_operand_labels(
                    program, parameters, &operands,
                );
            let invocation_contexts = InvocationContexts::from_named_use(
                &facts.flow,
                &facts.semantic,
                named_use_handle,
                &operands,
            );
            let point = operator_consumer_point(named_use.origin);

            check_requires_facts(
                program,
                point,
                facts,
                &invocation_contexts,
                named_use.origin,
                named_use.expression,
                parameters,
                type_parameters,
                &operands,
                &operand_labels,
                requires_facts,
                &mut diagnostics,
                |fact| {
                    Diagnostic::error(format!(
                        "cannot prove `{}` — the `requires` of `{}` (called `{}`)",
                        requires_clause_label(program, parameters, &operand_labels, fact),
                        operator_path_label(
                            program,
                            Some(operator),
                            operator.home_domain,
                            operator.symbol
                        ),
                        program.expression_table.display_name(named_use.expression),
                    ))
                },
            );
        }

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in
                    crate::flow::state_statements(program, state.statement_nodes).iter()
                {
                    let Some(call) = statement.as_call() else {
                        continue;
                    };
                    let Some(operator) =
                        crate::flow::resolved_operator_statement_symbol(program, call)
                            .and_then(|symbol| declaration_by_symbol(program, symbol))
                    else {
                        continue;
                    };
                    if !program
                        .signature_contracts
                        .span_or_empty(operator.contracts)
                        .iter()
                        .any(|contract| contract.kind == SignatureContractKind::Requires)
                    {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "statement call to named operator `{}` carries `requires` obligations no checked capture can prove",
                        operator_path_label(
                            program,
                            Some(operator),
                            operator.home_domain,
                            operator.symbol
                        ),
                    )));
                }
            }
        }

        diagnostics
    }

    /*
        The selected operator's definition; candidates record only the symbol, so
        resolve it back to the declaration (root surface or domain-owned).
    */
    fn operator_definition(
        program: &dyn ::validation::CheckingInput,
        operator_symbol: SymbolHandle,
    ) -> Option<&OperatorDefinition> {
        program
            .operators()
            .iter()
            .chain(
                program
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| program.domain_operators(domain).iter()),
            )
            .find(|operator| operator.symbol == operator_symbol)
    }

    /*
        The browsable path naming the operator in a failed-contract attribution,
        e.g. `Quantity::Additive::add` for a domain-owned meaning or `Slice::index`
        for a root surface declaration.
    */
    fn operator_path_label(
        program: &dyn ::validation::CheckingInput,
        operator: Option<&OperatorDefinition>,
        domain_symbol: SymbolHandle,
        operator_symbol: SymbolHandle,
    ) -> String {
        let path = operator
            .map(|operator| program.operator_path_string(operator.name))
            .unwrap_or_default();
        let path = if path.is_empty() {
            symbol_name(program, operator_symbol)
        } else {
            path
        };
        if domain_symbol.is_valid() {
            format!("{}::{path}", symbol_name(program, domain_symbol))
        } else {
            path
        }
    }

    /*
        Whether one instantiated `requires` fact is proven by any context entering
        the invocation, or is a closed-literal claim decidable on its own.
    */

    /* Push one diagnostic per requires fact the invocation does not prove. */
    #[allow(clippy::too_many_arguments)]
    fn check_requires_facts(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        facts: &CheckFacts,
        invocation_contexts: &InvocationContexts<'_>,
        origin: CheckedValueOrigin,
        source_expression: ExpressionHandle,
        parameters: &[StateParameter],
        type_parameters: &[TypeParameter],
        operands: &[ExpressionHandle],
        operand_labels: &[String],
        requires_facts: Vec<&ProofFact>,
        diagnostics: &mut Vec<Diagnostic>,
        report: impl Fn(&ProofFact) -> Diagnostic,
    ) {
        for fact in requires_facts {
            let proven = requires_fact_proven(
                program,
                point,
                &facts.semantic,
                &facts.operators,
                invocation_contexts,
                parameters,
                type_parameters,
                operands,
                operand_labels,
                fact,
            ) || fact.as_expression().is_some_and(|required| {
                invocation_contexts.proves_selected_requirement(
                    program,
                    origin,
                    source_expression,
                    *required,
                )
            });
            if !proven {
                diagnostics.push(report(fact));
            }
        }
    }

    fn requires_fact_proven(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        type_parameters: &[TypeParameter],
        operands: &[ExpressionHandle],
        operand_labels: &[String],
        fact: &ProofFact,
    ) -> bool {
        if let Some(expression) = fact.as_expression()
            && reflexive_comparison_holds(program, parameters, type_parameters, *expression)
            && closed_operators_are_builtin(program, operators, *expression)
        {
            return true;
        }
        match fact {
            ProofFact::Membership(membership) => {
                if !membership.domain_arguments.is_empty() {
                    return false;
                }
                let value_label = instantiate_operator_contract_expression_label(
                    program,
                    parameters,
                    operand_labels,
                    membership.value,
                );
                let contexts = contexts.for_expressions(program, parameters, [membership.value]);
                contexts.iter().any(|context| {
                    context_proves_membership_label(
                        program,
                        semantic,
                        *context,
                        &value_label,
                        membership.domain_symbol,
                    )
                })
            }
            ProofFact::Expression(expression) => contexts_prove_boolean_expression(
                program,
                point,
                semantic,
                operators,
                contexts,
                parameters,
                operands,
                operand_labels,
                *expression,
            ),
            ProofFact::Proposition(application) => {
                let contexts = contexts.for_expressions(
                    program,
                    parameters,
                    expressions(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .copied(),
                );
                let (binder_labels, argument_handles) =
                    crate::published::labels::proposition_label_sources(application, program);
                let argument_labels = argument_handles
                    .iter()
                    .map(|argument| {
                        instantiate_operator_contract_expression_label(
                            program,
                            parameters,
                            operand_labels,
                            *argument,
                        )
                    })
                    .collect::<Vec<_>>();
                let Some(required_label) = program.normalized_identity_label(
                    application,
                    &binder_labels,
                    &argument_labels,
                ) else {
                    return false;
                };
                crate::checks::contracts::contexts_prove_proposition_label(
                    semantic,
                    program,
                    &contexts,
                    &required_label,
                )
            }
        }
    }

    /*
        Boolean `requires` clauses decompose like the call-`requires` prover:
        conjunctions need both sides, disjunctions either side, and a leaf is
        proven by a matching instantiated fact (directly, or derived from a domain
        membership whose body states the clause) or is a closed-literal claim.
    */
    fn contexts_prove_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        semantic: &FactPlan,
        operators: &CheckedOperatorFacts,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        operand_labels: &[String],
        expression: ExpressionHandle,
    ) -> bool {
        macro_rules! prove_child {
            ($expression:expr $(,)?) => {
                contexts_prove_boolean_expression(
                    program,
                    point,
                    semantic,
                    operators,
                    contexts,
                    parameters,
                    operands,
                    operand_labels,
                    $expression,
                )
            };
        }
        let leaf_is_proven = |expression| {
            contexts_prove_boolean_leaf(
                program,
                point,
                semantic,
                contexts,
                parameters,
                operand_labels,
                expression,
            ) || (closed_clause_value(program, parameters, operands, expression) == Some(true)
                && closed_operators_are_builtin(program, operators, expression))
                || crate::checks::float_implications::contexts_prove_comparison(
                    program,
                    semantic,
                    operators,
                    contexts,
                    &contexts.for_expressions(program, parameters, [expression]),
                    parameters,
                    operands,
                    expression,
                )
                || contexts_prove_case_membership(
                    program, semantic, contexts, parameters, operands, expression,
                )
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => prove_child!(inner.target),
            ExpressionNode::Boolean(true) => true,
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => prove_child!(binary.left) && prove_child!(binary.right),
                BinaryOperator::Or => prove_child!(binary.left) || prove_child!(binary.right),
                _ => leaf_is_proven(expression),
            },
            _ => leaf_is_proven(expression),
        }
    }

    fn contexts_prove_boolean_leaf(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        semantic: &FactPlan,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        operand_labels: &[String],
        expression: ExpressionHandle,
    ) -> bool {
        let required_label = instantiate_operator_contract_expression_label(
            program,
            parameters,
            operand_labels,
            expression,
        );
        let contexts = contexts.for_expressions(program, parameters, [expression]);
        contexts.iter().any(|context| {
            context_proves_boolean_label(
                program,
                semantic,
                *context,
                &contexts,
                point,
                &required_label,
            )
        })
    }

    /*
        An instantiated membership obligation (`<operand> in Domain`) is proven by
        a context fact placing the same value in a domain that implies the required
        one.
    */
    fn context_proves_membership_label(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: FactContextHandle,
        value_label: &str,
        required_domain: SymbolHandle,
    ) -> bool {
        if !has_symbol_only_proof(program, required_domain) {
            return false;
        }
        let context = semantic.contexts.get(context);
        semantic.context_view(context).facts().any(|fact| {
            let (fact_domain, fact_value) = match fact.payload {
                FactPayload::DomainMembership {
                    domain_symbol,
                    value,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain_symbol,
                    value,
                    ..
                } => (domain_symbol, value),
                _ => return false,
            };
            if !has_symbol_only_proof(program, fact_domain) {
                return false;
            }
            if !semantic.domain_implies(fact_domain, required_domain)
                && !domain_membership_implies(program, fact_domain, required_domain)
            {
                return false;
            }
            let place_matches = matches!(fact.place, FactPlace::Place(place)
                if semantic.place_label(program, place) == value_label);
            place_matches || program.expression_table.display_name(fact_value) == value_label
        })
    }

    /*
        The unproven clause in caller terms for the attribution diagnostic. Both
        use kinds pass their own operand labels: a spelled use renders operands as
        written, while a named call names the referent under a reference formal.
    */
    fn requires_clause_label(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operand_labels: &[String],
        fact: &ProofFact,
    ) -> String {
        match fact {
            ProofFact::Expression(expression) => instantiate_operator_contract_expression_label(
                program,
                parameters,
                operand_labels,
                *expression,
            ),
            ProofFact::Membership(membership) => format!(
                "{} in {}",
                instantiate_operator_contract_expression_label(
                    program,
                    parameters,
                    operand_labels,
                    membership.value,
                ),
                symbol_name(program, membership.domain_symbol)
            ),
            ProofFact::Proposition(application) => format!(
                "{}({})",
                application.name.as_str(),
                expressions(program)
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|argument| {
                        instantiate_operator_contract_expression_label(
                            program,
                            parameters,
                            operand_labels,
                            *argument,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /*
        A scalar atom whose value is fixed by the source text alone. A float
        literal keeps its authored landing; the comparison site picks the common
        format from a substituted formal's declared type when one is available.
    */
    enum ClosedScalar {
        Integer(BigInt),
        Float(FloatLiteral),
        Boolean(bool),
    }

    /*
        An instantiated `requires` leaf whose atoms are all closed literals is a
        concrete claim: `70 == 70` needs no context fact because the operand
        written at the call is its own evidence. The same holds for the float
        conversion contracts — `value == value && value > -129.0 && value < 128.0`
        on a `-8.75f32` operand is an exact finite-range check, decided here at the
        literal's own format. This grants no operator law: the leaf evaluates only
        when every operator inside it resolved to a builtin meaning, so a
        comparison that selected a checked or boundary-operator meaning keeps the
        label/fact path. The channel is context-free — operand custody is about
        which facts describe the operands, and a literal atom carries no premise.
        `x == x`, `x <= x` and `x >= x` hold for every value of a non-float
        scalar under the builtin comparison: one symbol names one value on both
        sides, whether it is a formal (bound to one captured operand) or the
        operator's own const or value type parameter (`requires Count == Count`).
        The caller also requires the comparison itself to be builtin
        (`closed_operators_are_builtin`), since a selected user operator carries
        no reflexivity law. Floats are excluded because NaN is not equal to
        itself, and any other name keeps the ordinary discharge.
    */
    fn reflexive_comparison_holds(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        type_parameters: &[TypeParameter],
        expression: ExpressionHandle,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::LessOrEqual | BinaryOperator::GreaterOrEqual
        ) {
            return false;
        }
        let (ExpressionNode::Name(left), ExpressionNode::Name(right)) = (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) else {
            return false;
        };
        let single =
            |path: &TableNamePath| expressions(program).name_path_members(path.members).len() == 1;
        if !left.symbol.is_valid() || left.symbol != right.symbol || !single(left) || !single(right)
        {
            return false;
        }
        let declared = parameters
            .iter()
            .find(|parameter| parameter.symbol == left.symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                type_parameters
                    .iter()
                    .find(|parameter| parameter.symbol == left.symbol)
                    .and_then(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => Some(type_reference),
                        _ => None,
                    })
            });
        declared
            .and_then(|reference| program.type_reference_table.primitive_type(reference))
            .is_some_and(|primitive| !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
    }

    /*
        Evaluate a contract-side Boolean leaf (or its Boolean structure) to a
        closed value. Formal `Name` leaves substitute their operand expression;
        anything not reducible to literals yields `None`.
    */
    fn closed_clause_value(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> Option<bool> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Borrow(borrow) => {
                closed_clause_value(program, parameters, operands, borrow.target)
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                closed_clause_value(program, parameters, operands, unary.operand)
                    .map(|value| !value)
            }
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => {
                    let left = closed_clause_value(program, parameters, operands, binary.left)?;
                    let right = closed_clause_value(program, parameters, operands, binary.right)?;
                    Some(left && right)
                }
                BinaryOperator::Or => {
                    let left = closed_clause_value(program, parameters, operands, binary.left)?;
                    let right = closed_clause_value(program, parameters, operands, binary.right)?;
                    Some(left || right)
                }
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => {
                    let left = closed_scalar_value(program, parameters, operands, binary.left)?;
                    let right = closed_scalar_value(program, parameters, operands, binary.right)?;
                    let format = clause_comparison_format(
                        program,
                        parameters,
                        operands,
                        binary.left,
                        binary.right,
                        &left,
                        &right,
                    );
                    closed_compare(&left, &right, binary.operator, format)
                }
                _ => None,
            },
            _ => match closed_scalar_value(program, parameters, operands, expression) {
                Some(ClosedScalar::Boolean(value)) => Some(value),
                _ => None,
            },
        }
    }

    /*
        Reduce one atom of a closed clause to a literal value. A contract formal's
        name binds its operand expression and the recursion continues there; the
        operand's own names are caller-local symbols, which never match a formal —
        keeping substitution structurally finite and caller names uninterpreted.
    */
    fn closed_scalar_value(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> Option<ClosedScalar> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => value.value_bignum().map(ClosedScalar::Integer),
            ExpressionNode::Float(value) => Some(ClosedScalar::Float(value.clone())),
            ExpressionNode::Boolean(value) => Some(ClosedScalar::Boolean(*value)),
            ExpressionNode::Borrow(borrow) => {
                closed_scalar_value(program, parameters, operands, borrow.target)
            }
            ExpressionNode::Name(path) => {
                let operand = operand_for_parameter(program, parameters, operands, path)?;
                closed_scalar_value(program, parameters, operands, operand)
            }
            _ => None,
        }
    }

    /*
        Bind a contract-side formal name to its operand expression. Self-alignment
        mirrors the label instantiation: a captured receiver operand rides its
        formal's ordinal; without one, operands count only the non-self formals.
    */
    fn operand_for_parameter(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        path: &TableNamePath,
    ) -> Option<ExpressionHandle> {
        if !path.symbol.is_valid()
            || expressions(program).name_path_members(path.members).len() != 1
        {
            return None;
        }
        let operands_include_self = operands.len() == parameters.len();
        let mut positional_operand_index = 0usize;
        for (parameter_index, parameter) in parameters.iter().enumerate() {
            let operand = if operands_include_self {
                operands.get(parameter_index)
            } else {
                let operand = operands.get(positional_operand_index);
                positional_operand_index += 1;
                operand
            };
            if path.symbol == parameter.symbol || path.head_symbol == parameter.symbol {
                return operand.copied();
            }
        }
        None
    }

    /*
        The float format a comparison reads at: a substituted formal's declared
        type is authoritative (an unsuffixed literal reads at its operand type),
        an explicitly suffixed literal decides otherwise, and f64 is the default
        when no float context exists at all.
    */
    fn clause_comparison_format(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        left: ExpressionHandle,
        right: ExpressionHandle,
        left_scalar: &ClosedScalar,
        right_scalar: &ClosedScalar,
    ) -> FloatFormat {
        for side in [left, right] {
            let Some(path) = program.expression_table.name_path_node(side) else {
                continue;
            };
            let Some(parameter) = parameters.iter().find(|parameter| {
                path.symbol == parameter.symbol || path.head_symbol == parameter.symbol
            }) else {
                continue;
            };
            if let Some(format) = program
                .type_reference_table
                .primitive_type(parameter.type_reference)
                .and_then(|primitive| match primitive {
                    PrimitiveType::F32 => Some(FloatFormat::F32),
                    PrimitiveType::F64 => Some(FloatFormat::F64),
                    _ => None,
                })
            {
                return format;
            }
            if let Some(operand) = operand_for_parameter(program, parameters, operands, path)
                && let ExpressionNode::Float(literal) = program.expression_table.expression(operand)
                && let Some(format) = literal.landing()
            {
                return format;
            }
        }
        [left_scalar, right_scalar]
            .iter()
            .find_map(|scalar| match scalar {
                ClosedScalar::Float(literal) => literal.landing(),
                _ => None,
            })
            .unwrap_or(FloatFormat::F64)
    }

    fn closed_compare(
        left: &ClosedScalar,
        right: &ClosedScalar,
        operator: BinaryOperator,
        format: FloatFormat,
    ) -> Option<bool> {
        match (left, right) {
            (ClosedScalar::Boolean(left), ClosedScalar::Boolean(right)) => match operator {
                BinaryOperator::Equal => Some(left == right),
                BinaryOperator::NotEqual => Some(left != right),
                _ => None,
            },
            (ClosedScalar::Integer(left), ClosedScalar::Integer(right)) => {
                operator.evaluate_comparison(&left, &right)
            }
            (ClosedScalar::Boolean(_), _) | (_, ClosedScalar::Boolean(_)) => None,
            _ => {
                let left = closed_exact_float(left, format)?;
                let right = closed_exact_float(right, format)?;
                Some(match operator {
                    BinaryOperator::Equal => left.equal_value(&right),
                    BinaryOperator::NotEqual => !left.equal_value(&right),
                    BinaryOperator::Less => left.partial_cmp_value(&right) == Some(Ordering::Less),
                    BinaryOperator::LessOrEqual => matches!(
                        left.partial_cmp_value(&right),
                        Some(Ordering::Less | Ordering::Equal)
                    ),
                    BinaryOperator::Greater => {
                        left.partial_cmp_value(&right) == Some(Ordering::Greater)
                    }
                    BinaryOperator::GreaterOrEqual => matches!(
                        left.partial_cmp_value(&right),
                        Some(Ordering::Greater | Ordering::Equal)
                    ),
                    _ => return None,
                })
            }
        }
    }

    /*
        Read one atom at the comparison's float format. An integer literal in a
        float comparison rounds exactly to that format — the same landing a
        written `0` takes against an f32 operand — and a float literal is decoded
        from its exact IEEE meaning so NaN orderings stay unordered.
    */
    fn closed_exact_float(scalar: &ClosedScalar, format: FloatFormat) -> Option<ExactFloat> {
        match scalar {
            ClosedScalar::Float(literal) => Some(match format {
                FloatFormat::F32 => ExactFloat::from_f32(literal.value_f32()),
                FloatFormat::F64 => ExactFloat::from_f64(literal.value_f64()),
            }),
            ClosedScalar::Integer(value) => {
                let rational = BigRational::from_integer(value.clone());
                Some(match format {
                    FloatFormat::F32 => ExactFloat::from_f32(
                        rational.to_f32_with_rounding(IeeeRounding::NearestTiesToEven),
                    ),
                    FloatFormat::F64 => ExactFloat::from_f64(
                        rational.to_f64_with_rounding(IeeeRounding::NearestTiesToEven),
                    ),
                })
            }
            ClosedScalar::Boolean(_) => None,
        }
    }

    /*
        Every operator occurrence inside a closed leaf must resolve to a builtin
        meaning — builtin fallback, or a body-less `boundary machine` realized by
        the target (such as `Float::equal`) — before its literal atoms may be
        evaluated. A checked or boundary-operator selection keeps the label/fact
        path: its meaning is not inferred from the spelling here.
    */
    pub(crate) fn closed_operators_are_builtin(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
    ) -> bool {
        let mut has_non_builtin = false;
        operators.for_each_expression_use(expression, |_, operator_use| {
            if !operator_use_is_builtin_meaning(program, operators, operator_use) {
                has_non_builtin = true;
            }
        });
        if has_non_builtin {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                closed_operators_are_builtin(program, operators, binary.left)
                    && closed_operators_are_builtin(program, operators, binary.right)
            }
            ExpressionNode::Unary(unary) => {
                closed_operators_are_builtin(program, operators, unary.operand)
            }
            ExpressionNode::Borrow(borrow) => {
                closed_operators_are_builtin(program, operators, borrow.target)
            }
            _ => true,
        }
    }

    fn operator_use_is_builtin_meaning(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        operator_use: &CheckedOperatorUseFact,
    ) -> bool {
        match operator_use.status {
            CheckedOperatorResolutionStatus::BuiltinFallback => true,
            CheckedOperatorResolutionStatus::Resolved => operators
                .selected_candidate(operator_use)
                .is_some_and(|selected| {
                    program.machine_token_bindings().iter().any(|binding| {
                        binding.symbol == selected.operator_symbol && binding.is_boundary
                    })
                }),
            _ => false,
        }
    }

    /*
        A `subject in T::C` leaf -- lowered to `subject == T::C` -- is proven by
        the installed case at the operand place the subject names. The assigned
        tag, not a value comparison, is the evidence.
    */
    fn contexts_prove_case_membership(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &InvocationContexts<'_>,
        parameters: &[StateParameter],
        operands: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> bool {
        let Some((subject, case)) =
            crate::published::proof::exact_outcome_case_test(program, expression)
        else {
            return false;
        };
        let Some(formal) = canonical_place_from_expression(program, subject) else {
            return false;
        };
        let Some(symbol) = formal.root.as_symbol() else {
            return false;
        };
        let Some(position) = state_parameter_position(parameters, symbol) else {
            return false;
        };
        let Some(mut place) = operands
            .get(position)
            .and_then(|operand| canonical_place_from_expression(program, *operand))
        else {
            return false;
        };
        place.segments.extend(formal.segments);
        let contexts = contexts.for_expressions(program, parameters, [expression]);
        contexts.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    crate::flow::zero_case_at(program, semantic, &fact, &place) == Some(case)
                        || matches!(fact.payload, FactPayload::CaseValue { variant, value: true } if variant == case)
                        && matches!(fact.place, FactPlace::Place(candidate) if
                        crate::flow::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(candidate),
                        )
                        .is_some_and(|candidate| {
                            candidate.root == place.root
                                && candidate.segments == place.segments
                        }))
                })
        })
    }
}

pub(crate) use requires::{
    InvocationContexts, closed_operators_are_builtin, named_operator_route_is_false,
    operator_route_is_false,
};

pub(crate) fn check_operator_resolution(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = facts
        .operators
        .resolution_issues()
        .filter_map(|issue| {
            if issue.is_ambiguous() {
                Some(ambiguous_operator_diagnostic(program, issue))
            } else if issue.is_inadmissible() {
                Some(inadmissible_operator_diagnostic(program, issue))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    diagnostics.extend(requires::selected_binary_requires_diagnostics(
        program, facts,
    ));
    diagnostics.extend(destinations::check(program, facts));
    for (_, operator_use) in facts.operators.uses.iter() {
        let selected = facts.operators.selected_candidate(operator_use);
        if operator_use.occurrence == checked_trees::CheckedOperatorOccurrence::Expression {
            continue;
        }
        let Some(selected) = selected else {
            diagnostics.push(Diagnostic::error(
                "floating Match equality requires one selected operator meaning",
            ));
            continue;
        };
        if operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved
            || operator_use.operands(program).is_none()
            || selected.parameter_count != 2
            || program.primitive_type_reference(selected.return_type)
                != Some(typed_trees::types::PrimitiveType::Bool)
        {
            diagnostics.push(Diagnostic::error(
                "selected Match equality must have exact operands and return Bool",
            ));
        }
    }

    crate::checks::finish(diagnostics)
}

/*
    The binding-site rule (chapter 8): a domain-owned meaning participates only
    when an operand declaration, mint, or signature `requires` selects its
    semantic role. This use has domain candidates, none selected, and no
    builtin/root fallback.
*/
fn inadmissible_operator_diagnostic(
    program: &dyn ::validation::CheckingInput,
    issue: CheckedOperatorResolutionIssue<'_>,
) -> Diagnostic {
    let operand = expressions(program).display_name(issue.operator_use.expression);
    let required_facts = issue
        .candidates
        .iter()
        .filter(|candidate| candidate.is_domain_owned())
        .map(|candidate| {
            format!(
                "a binding declared, minted, or signature-qualified in {}",
                symbol_name(program, candidate.domain_symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(" or ");
    Diagnostic::error(format!(
        "operator spelling `{}` in `{operand}` has no admissible meaning: \
         the domain operator meaning needs {required_facts}, \
         and no builtin meaning exists for the operand type",
        issue.spelling().symbol(),
    ))
}

fn ambiguous_operator_diagnostic(
    program: &dyn ::validation::CheckingInput,
    issue: CheckedOperatorResolutionIssue<'_>,
) -> Diagnostic {
    let candidates = issue
        .candidates
        .iter()
        .map(|candidate| {
            let operator = symbol_name(program, candidate.operator_symbol);
            if candidate.is_trait_backed() {
                format!(
                    "`{operator}` selected by proof-static conformance `{}`",
                    symbol_name(program, candidate.conformance_symbol)
                )
            } else if candidate.domain_symbol.is_valid() {
                format!(
                    "`{operator}` owned by domain `{}`",
                    symbol_name(program, candidate.domain_symbol)
                )
            } else {
                format!("`{operator}` (builtin/root)")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::error(format!(
        "ambiguous operator spelling `{}` has {} viable candidates: {candidates}. The static \
         operand-domain tuple does not uniquely select one -- narrow the binding's semantic \
         qualification or choose a clearer operation.",
        issue.spelling().symbol(),
        issue.candidate_count(),
    ))
}
