/*
Toolchain float-meaning projection contracts and invocations alongside
mathematical function and proposition applications
(`proof_contracts::{float_projection_bindings, float_projection_invocations,
mathematical_applications}`).
*/
pub mod float_projection_bindings {
    pub(crate) mod conversion_contracts {
        use crate::frozen_program::expression_table;
        use numerics::float_semantics_catalog::FloatSemanticOperation;
        use numerics::float_semantics_catalog::IntegerCarrier;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::operator::OperatorDefinition;
        use typed_trees::types::PrimitiveType;
        use typed_trees::types::TypeReferenceHandle;

        pub fn float_format_conversion_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(PrimitiveType, PrimitiveType)> {
            numeric_float_conversion_contract(program, operator)
                .filter(|(source, _)| matches!(source, PrimitiveType::F32 | PrimitiveType::F64))
        }

        pub fn numeric_float_conversion_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(PrimitiveType, PrimitiveType)> {
            selected_numeric_float_conversion_contract(program, operator)
                .map(|(source, target, _)| (source, target))
        }

        fn selected_numeric_float_conversion_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(
            PrimitiveType,
            PrimitiveType,
            &'static FloatSemanticOperation,
        )> {
            use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
            let [parameter] = program.operator_parameters(operator) else {
                return None;
            };
            let source = program.primitive_type_reference(parameter.type_reference)?;
            let target = program.primitive_type_reference(operator.return_type)?;
            if !(matches!(source, PrimitiveType::F32 | PrimitiveType::F64)
                || IntegerCarrier::from_spelling(source.name()).is_some())
                || !matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            let resolve =
                |program: &dyn crate::CheckingInput,
                 conversion: &typed_trees::expression::TableCallExpression| {
                    let source_kind = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                        FloatSemanticValueKind::Meaning
                    } else {
                        FloatSemanticValueKind::Integer(IntegerCarrier::from_spelling(
                            source.name(),
                        )?)
                    };
                    super::semantic_operations::exact_toolchain_float_semantic_application(
                        program,
                        conversion,
                        &[FloatSemanticValueKind::Format, source_kind],
                    )
                    .map(|(_, semantic, _)| (None, semantic))
                };
            scan_conversion_equalities(program, operator, resolve, |candidate| {
                let (result, result_primitive, meaning_type) =
                    projection(program, candidate.result)?;
                if result_primitive != target
                    || crate::reserved_result_owner(program, result).map(|owner| owner.0)
                        != Some(operator.symbol)
                    || !candidate.relation.relates_meanings(
                        program,
                        operator.symbol,
                        candidate.expression,
                        meaning_type,
                    )
                {
                    return None;
                }
                let semantic = candidate.semantic;
                let [format, value] =
                    expression_table(program).expression_handles(candidate.conversion.arguments)
                else {
                    return None;
                };
                let expected_format = match target {
                    PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                    PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                    _ => return None,
                };
                if super::semantic_values::exact_toolchain_float_format_const(program, *format)
                    != Some(expected_format)
                {
                    return None;
                }
                let value = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                    if semantic.name != "convert" {
                        return None;
                    }
                    let (value, source_primitive, _) = projection(program, *value)?;
                    if source_primitive != source {
                        return None;
                    }
                    value
                } else {
                    let [
                        FloatSemanticValueKind::Format,
                        FloatSemanticValueKind::Integer(carrier),
                    ] = semantic.parameters
                    else {
                        return None;
                    };
                    if !matches!(semantic.kernel(), FloatSemanticKernel::FromInteger(_))
                        || PrimitiveType::from_name(carrier.spelling()) != Some(source)
                    {
                        return None;
                    }
                    *value
                };
                let value = expression_table(program).name_path_node(value)?;
                if value.symbol.is_valid() && value.symbol == parameter.symbol {
                    Some((source, target, semantic))
                } else {
                    None
                }
            })
        }

        pub fn integer_float_conversion_bounds(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(
            PrimitiveType,
            PrimitiveType,
            numerics::float_semantics::FloatMeaning,
            numerics::float_semantics::FloatMeaning,
        )> {
            let (source, target, semantic) =
                selected_numeric_float_conversion_contract(program, operator)?;
            let carrier = IntegerCarrier::from_spelling(source.name())?;
            let format = match target {
                PrimitiveType::F32 => numerics::float_semantics::FloatFormat::BINARY32,
                PrimitiveType::F64 => numerics::float_semantics::FloatFormat::BINARY64,
                _ => return None,
            };
            let (minimum, maximum) = carrier.format().bounds();
            let (minimum, maximum) =
                semantic.integer_operand_image(format, carrier, &minimum, &maximum)?;
            Some((source, target, minimum, maximum))
        }

        pub(crate) struct ConversionEqualityCandidate<'program> {
            expression: ExpressionHandle,
            relation: FloatLawRelation,
            result: ExpressionHandle,
            conversion: &'program typed_trees::expression::TableCallExpression,
            selected: Option<&'program OperatorDefinition>,
            semantic: &'static FloatSemanticOperation,
        }

        /*
            The core library states a float law in one of two forms. A law whose
            subjects are scalars (`bool`, a class, an integer) uses the builtin
            `==`. A law whose subjects are `FloatMeaning` values uses the sealed
            `core::Identity<FloatMeaning>(left, right)`, because a mathematical
            meaning has no executable equality. Every recognizer of a float law
            reads it through this one reader, so a law keeps its role in either
            form instead of silently losing it when the library respells it.
        */
        #[derive(Clone, Copy)]
        pub(super) enum FloatLawRelation {
            BuiltinEquality,
            MeaningIdentity(TypeReferenceHandle),
        }

        pub(super) fn float_law_subjects(
            program: &dyn crate::CheckingInput,
            law: ExpressionHandle,
        ) -> Option<(FloatLawRelation, [ExpressionHandle; 2])> {
            if let Some(equality) = expression_table(program).binary_node(law) {
                return (equality.operator == BinaryOperator::Equal).then_some((
                    FloatLawRelation::BuiltinEquality,
                    [equality.left, equality.right],
                ));
            }
            let call = expression_table(program).call_node(law)?;
            let identity = crate::proof_contracts::mathematical_applications::identity_application(
                program, call,
            )
            .ok()?;
            Some((
                FloatLawRelation::MeaningIdentity(identity.carrier),
                identity.subjects,
            ))
        }

        impl FloatLawRelation {
            /*
                Whether this law relates two values of the exact `meaning_type`.
                The identity form must name the toolchain's own `FloatMeaning` as
                its carrier. An identity over any other type says nothing about
                float meanings and is refused here rather than read as one.
            */
            pub(super) fn relates_meanings(
                self,
                program: &dyn crate::CheckingInput,
                owner: symbols::SymbolHandle,
                law: ExpressionHandle,
                meaning_type: TypeReferenceHandle,
            ) -> bool {
                match self {
                    Self::BuiltinEquality => {
                        typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            owner,
                            law,
                            language_core::OperatorSpelling::Equal,
                            &[Some(meaning_type), Some(meaning_type)],
                        )
                    }
                    Self::MeaningIdentity(carrier) => {
                        super::is_exact_toolchain_float_meaning_type(program, carrier)
                            && super::is_exact_toolchain_float_meaning_type(program, meaning_type)
                            && program.normalized_type_identity(carrier)
                                == program.normalized_type_identity(meaning_type)
                    }
                }
            }
        }

        fn scan_conversion_equalities<'program, R>(
            program: &'program dyn crate::CheckingInput,
            operator: &OperatorDefinition,
            mut resolve: impl FnMut(
                &'program dyn crate::CheckingInput,
                &'program typed_trees::expression::TableCallExpression,
            ) -> Option<(
                Option<&'program OperatorDefinition>,
                &'static FloatSemanticOperation,
            )>,
            mut judge: impl FnMut(&ConversionEqualityCandidate<'_>) -> Option<R>,
        ) -> Option<R> {
            for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
                program,
                program.operator_contracts(operator),
            ) {
                let Some((relation, [left, right])) = float_law_subjects(program, expression)
                else {
                    continue;
                };
                for (result, converted) in [(left, right), (right, left)] {
                    let Some(conversion) = expression_table(program).call_node(converted) else {
                        continue;
                    };
                    let Some((selected, semantic)) = resolve(program, conversion) else {
                        continue;
                    };
                    if let Some(verdict) = judge(&ConversionEqualityCandidate {
                        expression,
                        relation,
                        result,
                        conversion,
                        selected,
                        semantic,
                    }) {
                        return Some(verdict);
                    }
                }
            }
            None
        }

        fn projection(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(ExpressionHandle, PrimitiveType, TypeReferenceHandle)> {
            let call = expression_table(program).call_node(expression)?;
            let (selected, primitive, _, _) =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program, call,
                )?;
            let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
            let [operand] = expression_table(program).expression_handles(call.arguments) else {
                return None;
            };
            Some((*operand, primitive, operator.return_type))
        }

        pub fn float_integer_conversion_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(PrimitiveType, &'static FloatSemanticOperation)> {
            use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
            let [parameter] = program.operator_parameters(operator) else {
                return None;
            };
            let source = program.primitive_type_reference(parameter.type_reference)?;
            if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                return None;
            }
            let target = program.primitive_type_reference(operator.return_type)?;
            fn resolve<'program>(
                program: &'program dyn crate::CheckingInput,
                conversion: &'program typed_trees::expression::TableCallExpression,
            ) -> Option<(
                Option<&'program OperatorDefinition>,
                &'static FloatSemanticOperation,
            )> {
                let selected =
                    typed_trees::operator::resolve_named_expression_call(program, conversion)?;
                super::semantic_operations::exact_toolchain_float_semantic_contract(
                    program, selected,
                )
                .map(|(semantic, _)| (Some(selected), semantic))
            }
            scan_conversion_equalities(program, operator, resolve, |candidate| {
                if crate::reserved_result_owner(program, candidate.result).map(|owner| owner.0)
                    != Some(operator.symbol)
                {
                    return None;
                }
                let selected = candidate.selected?;
                let semantic = candidate.semantic;
                let FloatSemanticValueKind::Integer(carrier) = semantic.result else {
                    return None;
                };
                if PrimitiveType::from_name(carrier.spelling()) != Some(target)
                    || !matches!(
                        semantic.kernel(),
                        FloatSemanticKernel::ToIntegerExact
                            | FloatSemanticKernel::ToIntegerSaturating
                    )
                    || !matches!(candidate.relation, FloatLawRelation::BuiltinEquality)
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        operator.symbol,
                        candidate.expression,
                        language_core::OperatorSpelling::Equal,
                        &[Some(operator.return_type), Some(selected.return_type)],
                    )
                {
                    return None;
                }
                let [value] =
                    expression_table(program).expression_handles(candidate.conversion.arguments)
                else {
                    return None;
                };
                let (value, primitive, _) = projection(program, *value)?;
                let value = expression_table(program).name_path_node(value)?;
                if primitive == source
                    && value.symbol.is_valid()
                    && value.symbol == parameter.symbol
                {
                    Some((source, semantic))
                } else {
                    None
                }
            })
        }

        pub fn float_integer_conversion_bounds(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
            minimum: f64,
            maximum: f64,
        ) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
            use numerics::float_semantics::FloatMeaning;
            use numerics::float_semantics_catalog::{FloatSemanticOperand, FloatSemanticResult};
            let (_, semantic) = float_integer_conversion_contract(program, operator)?;
            if minimum > maximum || minimum.is_nan() || maximum.is_nan() {
                return None;
            }
            let convert = |value| {
                let discharged = semantic.kernel_discharge(&[FloatSemanticOperand::Meaning(
                    FloatMeaning::from_f64(value),
                )])?;
                let FloatSemanticResult::Integer(value) = discharged.result else {
                    return None;
                };
                Some(value)
            };
            Some((convert(minimum)?, convert(maximum)?))
        }
    }
    pub(crate) mod result_contracts {
        use crate::frozen_program::expression_table;
        use numerics::float_semantics_catalog::FloatSemanticValueKind;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::operator::{OperatorDefinition, resolve_named_expression_call};
        use typed_trees::types::PrimitiveType;

        pub fn float_result_semantic_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<ExpressionHandle> {
            let target = program.primitive_type_reference(operator.return_type)?;
            if !matches!(target, PrimitiveType::F32 | PrimitiveType::F64) {
                return None;
            }
            for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
                program,
                program.operator_contracts(operator),
            ) {
                let Some((relation, [left, right])) =
                    super::conversion_contracts::float_law_subjects(program, expression)
                else {
                    continue;
                };
                for (result, meaning) in [(left, right), (right, left)] {
                    let Some(result_call) = expression_table(program).call_node(result) else {
                        continue;
                    };
                    let Some((projection, primitive, _, _)) =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program,
                    result_call,
                )
            else {
                continue;
            };
                    let [result_value] =
                        expression_table(program).expression_handles(result_call.arguments)
                    else {
                        continue;
                    };
                    let Some(projection) = program.operator_definition_by_symbol(projection) else {
                        continue;
                    };
                    if primitive != target
                        || crate::reserved_result_owner(program, *result_value).map(|owner| owner.0)
                            != Some(operator.symbol)
                        || !relation.relates_meanings(
                            program,
                            operator.symbol,
                            expression,
                            projection.return_type,
                        )
                    {
                        continue;
                    }
                    let Some(call) = expression_table(program).call_node(meaning) else {
                        continue;
                    };
                    let Some(semantic) =
                        super::semantic_operations::float_semantic_application_for_parameters(
                            program,
                            call,
                            operator.symbol,
                            program.operator_parameters(operator),
                        )
                        .map(|(_, row, _)| row)
                    else {
                        continue;
                    };
                    if semantic.result != FloatSemanticValueKind::Meaning {
                        continue;
                    }
                    let expected = match target {
                        PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                        PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                        _ => unreachable!("float result checked above"),
                    };
                    let arguments = expression_table(program).expression_handles(call.arguments);
                    if arguments.len() != semantic.parameters.len()
                        || arguments
                            .iter()
                            .zip(semantic.parameters)
                            .any(|(argument, kind)| {
                                *kind == FloatSemanticValueKind::Format
                                    && super::semantic_values::exact_toolchain_float_format_const(
                                        program, *argument,
                                    ) != Some(expected)
                            })
                    {
                        continue;
                    }
                    let mut remaining_work = 1024;
                    if semantic_term_uses_only_inputs(
                        program,
                        operator,
                        meaning,
                        &mut remaining_work,
                    ) {
                        return Some(meaning);
                    }
                }
            }
            None
        }

        pub fn float_saturation_divisor_parameter(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<Option<usize>> {
            let raw = float_result_semantic_contract(program, operator)?;
            let call = expression_table(program).call_node(raw)?;
            let selected = resolve_named_expression_call(program, call)?;
            let (row, _) = super::semantic_operations::exact_toolchain_float_semantic_contract(
                program, selected,
            )?;
            if !matches!(
                row.name,
                "divide"
                    | "divide_toward_zero"
                    | "divide_toward_positive"
                    | "divide_toward_negative"
            ) {
                return Some(None);
            }
            let divisor = *expression_table(program)
                .expression_handles(call.arguments)
                .last()?;
            let projection = expression_table(program).call_node(divisor)?;
            crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                program, projection,
            )?;
            let [source] = expression_table(program).expression_handles(projection.arguments)
            else {
                return None;
            };
            let ExpressionNode::Name(path) = expression_table(program).expression(*source) else {
                return None;
            };
            program
                .operator_parameters(operator)
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)
                .map(Some)
        }

        fn semantic_term_uses_only_inputs(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
            expression: ExpressionHandle,
            remaining_work: &mut usize,
        ) -> bool {
            let Some(next) = remaining_work.checked_sub(1) else {
                return false;
            };
            *remaining_work = next;
            if super::semantic_values::exact_toolchain_float_format_const(program, expression)
                .is_some()
            {
                return true;
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Float(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_) => true,
                ExpressionNode::Name(path) => {
                    program
                        .operator_parameters(operator)
                        .iter()
                        .any(|parameter| parameter.symbol == path.symbol)
                        || super::semantic_values::exact_toolchain_float_format_const(
                            program, expression,
                        )
                        .is_some()
                }
                ExpressionNode::Call(call) => {
                    let admitted =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program, call,
                )
                .is_some()
                    || super::semantic_operations::float_semantic_application_for_parameters(
                        program,
                        call,
                        operator.symbol,
                        program.operator_parameters(operator),
                    )
                    .is_some();
                    admitted
                        && expression_table(program)
                            .expression_handles(call.arguments)
                            .iter()
                            .all(|argument| {
                                semantic_term_uses_only_inputs(
                                    program,
                                    operator,
                                    *argument,
                                    remaining_work,
                                )
                            })
                }
                _ => false,
            }
        }

        pub fn float_boolean_semantic_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<ExpressionHandle> {
            (program.primitive_type_reference(operator.return_type) == Some(PrimitiveType::Bool))
                .then(|| float_scalar_semantic_contract(program, operator))
                .flatten()
        }

        pub(crate) fn float_scalar_semantic_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<ExpressionHandle> {
            let result_kind =
                super::semantic_operations::typed_value_kind(program, operator.return_type)?;
            if !matches!(
                result_kind,
                FloatSemanticValueKind::Bool
                    | FloatSemanticValueKind::Class
                    | FloatSemanticValueKind::Integer(_)
            ) {
                return None;
            }
            for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
                program,
                program.operator_contracts(operator),
            ) {
                let Some(equality) = expression_table(program).binary_node(expression) else {
                    continue;
                };
                if equality.operator != BinaryOperator::Equal
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        operator.symbol,
                        expression,
                        language_core::OperatorSpelling::Equal,
                        &[Some(operator.return_type), Some(operator.return_type)],
                    )
                {
                    continue;
                }
                for (result, predicate) in [
                    (equality.left, equality.right),
                    (equality.right, equality.left),
                ] {
                    if crate::reserved_result_owner(program, result).map(|owner| owner.0)
                        != Some(operator.symbol)
                    {
                        continue;
                    }
                    let Some(call) = expression_table(program).call_node(predicate) else {
                        continue;
                    };
                    let Some((_, row, _)) =
                        super::semantic_operations::float_semantic_application_for_parameters(
                            program,
                            call,
                            operator.symbol,
                            program.operator_parameters(operator),
                        )
                    else {
                        continue;
                    };
                    let mut remaining_work = 1024;
                    if row.result == result_kind
                        && semantic_term_uses_only_inputs(
                            program,
                            operator,
                            predicate,
                            &mut remaining_work,
                        )
                    {
                        return Some(predicate);
                    }
                }
            }
            None
        }
    }
    pub(crate) use result_contracts::float_scalar_semantic_contract;
    pub use result_contracts::{
        float_boolean_semantic_contract, float_result_semantic_contract,
        float_saturation_divisor_parameter,
    };
    pub(crate) mod semantic_operations {
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE;
        use numerics::float_semantics_catalog::{
            FLOAT_FORMAT_CORE_SOURCE, FLOAT_SEMANTICS_NAMESPACE, FloatSemanticContractIdentity,
            FloatSemanticOperation, FloatSemanticValueKind, IntegerCarrier,
        };
        use typed_trees::operator::OperatorDefinition;
        use typed_trees::types::PrimitiveType;

        pub(crate) fn exact_toolchain_float_semantic_application(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
            operand_kinds: &[FloatSemanticValueKind],
        ) -> Option<(
            symbols::SymbolHandle,
            &'static FloatSemanticOperation,
            FloatSemanticContractIdentity,
        )> {
            let candidates = typed_trees::operator::named_expression_call_candidates(program, call);
            let mut selected = candidates.iter().filter(|operator| {
                let parameters = program.operator_parameters(operator);
                parameters.len() == operand_kinds.len()
                    && parameters
                        .iter()
                        .zip(operand_kinds)
                        .all(|(parameter, kind)| {
                            typed_value_kind(program, parameter.type_reference) == Some(*kind)
                        })
            });
            let operator = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            exact_toolchain_float_semantic_contract(program, operator)
                .map(|(semantic, contract)| (operator.symbol, semantic, contract))
        }

        pub fn float_semantic_application_for_parameters(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
            owner: symbols::SymbolHandle,
            parameters: &[typed_trees::signature::StateParameter],
        ) -> Option<(
            symbols::SymbolHandle,
            &'static FloatSemanticOperation,
            FloatSemanticContractIdentity,
        )> {
            let references = expression_table(program)
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    crate::parameter_expression_result_type_reference(
                        program, owner, parameters, *argument,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            float_semantic_application(program, call, &references)
        }

        pub(crate) fn float_semantic_application(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
            references: &[typed_trees::types::TypeReferenceHandle],
        ) -> Option<(
            symbols::SymbolHandle,
            &'static FloatSemanticOperation,
            FloatSemanticContractIdentity,
        )> {
            let kinds = references
                .iter()
                .map(|reference| typed_value_kind(program, *reference))
                .collect::<Option<Vec<_>>>()?;
            exact_toolchain_float_semantic_application(program, call, &kinds)
        }

        pub(crate) fn symbol_is_declared_in_sealed_source(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
            relative_source: &str,
        ) -> bool {
            language_core::core_vocabulary::CoreSource::from_file(relative_source)
                .is_some_and(|core| program.symbols.core_source(symbol) == Some(core))
        }

        pub(crate) fn is_sealed_toolchain_float_operator(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> bool {
            typed_trees::operator::is_sealed_toolchain_float_operator(program, operator)
        }

        fn has_exact_toolchain_float_semantics_owner(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
            name: &str,
        ) -> bool {
            let expected_identity = format!("toolchain::{FLOAT_SEMANTICS_NAMESPACE}::{name}");
            program
                .normalized_hermetic_symbol_identity(symbol)
                .ok()
                .as_deref()
                == Some(expected_identity.as_str())
                && symbol_is_declared_in_sealed_source(
                    program,
                    symbol,
                    FLOAT_PROJECTION_CORE_SOURCE,
                )
        }

        fn is_exact_toolchain_float_data_type(
            program: &dyn crate::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
            name: &str,
            relative_source: &str,
        ) -> bool {
            let typed_trees::types::TypeReferenceNode::Named {
                symbol,
                name: spelled,
            } = program.type_reference_table.type_reference(type_reference)
            else {
                return false;
            };
            spelled.as_str() == name
                && program
                    .normalized_hermetic_symbol_identity(*symbol)
                    .ok()
                    .as_deref()
                    == Some(format!("toolchain::{name}").as_str())
                && symbol_is_declared_in_sealed_source(program, *symbol, relative_source)
                && program
                    .data_definitions()
                    .iter()
                    .any(|data| data.symbol == *symbol && data.name.as_str() == name)
        }

        pub(crate) fn typed_value_kind(
            program: &dyn crate::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<FloatSemanticValueKind> {
            if let Some(primitive) = program.primitive_type_reference(type_reference) {
                return Some(match primitive {
                    PrimitiveType::Bool => FloatSemanticValueKind::Bool,
                    PrimitiveType::I8 => FloatSemanticValueKind::Integer(IntegerCarrier::I8),
                    PrimitiveType::I16 => FloatSemanticValueKind::Integer(IntegerCarrier::I16),
                    PrimitiveType::I32 => FloatSemanticValueKind::Integer(IntegerCarrier::I32),
                    PrimitiveType::I64 => FloatSemanticValueKind::Integer(IntegerCarrier::I64),
                    PrimitiveType::U8 => FloatSemanticValueKind::Integer(IntegerCarrier::U8),
                    PrimitiveType::U16 => FloatSemanticValueKind::Integer(IntegerCarrier::U16),
                    PrimitiveType::U32 => FloatSemanticValueKind::Integer(IntegerCarrier::U32),
                    PrimitiveType::U64 => FloatSemanticValueKind::Integer(IntegerCarrier::U64),
                    PrimitiveType::F32
                    | PrimitiveType::F64
                    | PrimitiveType::Addr
                    | PrimitiveType::Selector => return None,
                });
            }
            if super::is_exact_toolchain_float_meaning_type(program, type_reference) {
                return Some(FloatSemanticValueKind::Meaning);
            }
            if is_exact_toolchain_float_data_type(
                program,
                type_reference,
                "FloatFormat",
                FLOAT_FORMAT_CORE_SOURCE,
            ) {
                return Some(FloatSemanticValueKind::Format);
            }
            if is_exact_toolchain_float_data_type(
                program,
                type_reference,
                "FloatClass",
                FLOAT_PROJECTION_CORE_SOURCE,
            ) {
                return Some(FloatSemanticValueKind::Class);
            }
            None
        }

        pub fn exact_toolchain_float_semantic_contract(
            program: &dyn crate::CheckingInput,
            operator: &OperatorDefinition,
        ) -> Option<(
            &'static FloatSemanticOperation,
            FloatSemanticContractIdentity,
        )> {
            let [namespace, name] = program.operator_path_members(operator.name) else {
                return None;
            };
            if !FloatSemanticOperation::namespace_matches(namespace.as_str())
                || !has_exact_toolchain_float_semantics_owner(
                    program,
                    operator.symbol,
                    name.as_str(),
                )
            {
                return None;
            }
            if !operator.is_public
                || operator.is_boundary
                || operator.spelling.is_some()
                || !operator.lifetime_parameters.is_empty()
                || !program.operator_type_parameters(operator).is_empty()
                || !program.operator_contracts(operator).is_empty()
            {
                return None;
            }
            let mut parameters = Vec::new();
            for parameter in program.operator_parameters(operator) {
                if parameter.is_const || parameter.is_mutable {
                    return None;
                }
                parameters.push(typed_value_kind(program, parameter.type_reference)?);
            }
            let result = typed_value_kind(program, operator.return_type)?;
            let row = FloatSemanticOperation::from_source_identity(
                namespace.as_str(),
                name.as_str(),
                &parameters,
                result,
            )?;
            Some((row, row.contract_identity()))
        }

        pub(crate) fn validate_float_semantic_operation_bindings(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for operator in program.operators() {
                let [namespace, name] = program.operator_path_members(operator.name) else {
                    continue;
                };
                if !FloatSemanticOperation::names_a_row(namespace.as_str(), name.as_str())
                    || !has_exact_toolchain_float_semantics_owner(
                        program,
                        operator.symbol,
                        name.as_str(),
                    )
                {
                    continue;
                }
                if exact_toolchain_float_semantic_contract(program, operator).is_none() {
                    diagnostics.push(Diagnostic::error(format!(
                "canonical float semantic definition `{FLOAT_SEMANTICS_NAMESPACE}::{}` from `{FLOAT_PROJECTION_CORE_SOURCE}` must be one public, contract-free ordinary tokenless declaration whose parameters and result are exactly one catalog row over the sealed toolchain `FloatFormat`, `FloatMeaning`, `FloatClass`, `bool` and integer carriers, with no lifetime/type parameters or receiver qualification",
                name.as_str()
            )));
                }
            }
        }
    }
    pub(crate) mod semantic_values {
        use super::semantic_operations::symbol_is_declared_in_sealed_source;
        use language_semantics::const_value::CanonicalConstValue;
        use numerics::float_projection::FloatProjectionOperation;
        use numerics::float_semantics::{FloatFormat, FloatMeaning};
        use numerics::float_semantics_catalog::{
            FLOAT_FORMAT_CORE_SOURCE, FloatSemanticOperand, FloatSemanticResult,
            FloatSemanticValueKind,
        };
        use semantic_vocabulary::IeeeFloatFormat;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

        pub(crate) struct ClosedFloatMeaning {
            format: IeeeFloatFormat,
            meaning: FloatMeaning,
            type_reference: typed_trees::types::TypeReferenceHandle,
        }

        pub(crate) fn closed_float_meaning_equality(
            program: &dyn crate::CheckingInput,
            machine_symbol: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<bool> {
            let binary = program.expression_table.binary_node(expression)?;
            let spelling = match binary.operator {
                BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
                BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
                _ => return None,
            };
            let mut remaining_work = 1024;
            let left = closed_meaning(program, machine_symbol, binary.left, &mut remaining_work)?;
            let right = closed_meaning(program, machine_symbol, binary.right, &mut remaining_work)?;
            if left.format != right.format
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine_symbol,
                    expression,
                    spelling,
                    &[Some(left.type_reference), Some(right.type_reference)],
                )
            {
                return None;
            }
            Some(if binary.operator == BinaryOperator::Equal {
                left.meaning == right.meaning
            } else {
                left.meaning != right.meaning
            })
        }

        fn closed_meaning(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            expression: ExpressionHandle,
            remaining_work: &mut usize,
        ) -> Option<ClosedFloatMeaning> {
            *remaining_work = remaining_work.checked_sub(1)?;
            let call = program.expression_table.call_node(expression)?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            if let Some((selected_operator, _, operation, _)) =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program, call,
                )
            {
                let [source] = arguments else { return None };
                let (format, meaning) =
                    closed_projection_expression(operation, program, *source, remaining_work)?;
                let operator = program.operator_definition_by_symbol(selected_operator)?;
                return Some(ClosedFloatMeaning {
                    format,
                    meaning,
                    type_reference: operator.return_type,
                });
            }
            let (selected, row, contract) =
                super::semantic_operations::float_semantic_application_for_parameters(
                    program,
                    call,
                    owner,
                    &[],
                )?;
            let operator = program.operator_definition_by_symbol(selected)?;
            if row.result != FloatSemanticValueKind::Meaning
                || arguments.len() != row.parameters.len()
            {
                return None;
            }
            let mut operands = Vec::with_capacity(arguments.len());
            let mut declared_format = None;
            let mut operand_format = None;
            for (argument, kind) in arguments.iter().zip(row.parameters) {
                match kind {
                    FloatSemanticValueKind::Format => {
                        let format = exact_toolchain_float_format_const(program, *argument)?;
                        declared_format = Some(format);
                        operands.push(FloatSemanticOperand::Format(match format {
                            IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                            IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                        }));
                    }
                    FloatSemanticValueKind::Meaning => {
                        let value = closed_meaning(program, owner, *argument, remaining_work)?;
                        if operand_format.is_some_and(|previous| previous != value.format) {
                            return None;
                        }
                        operand_format = Some(value.format);
                        operands.push(FloatSemanticOperand::Meaning(value.meaning));
                    }
                    FloatSemanticValueKind::Integer(carrier) => {
                        operands.push(FloatSemanticOperand::Integer(
                            closed_float_semantic_integer_operand(
                                program, owner, *argument, *carrier,
                            )?,
                        ));
                    }
                    _ => return None,
                }
            }
            let format = declared_format.or(operand_format)?;
            let discharge = row.kernel_discharge(&operands)?;
            if discharge.contract != contract {
                return None;
            }
            let FloatSemanticResult::Meaning(meaning) = discharge.result else {
                return None;
            };
            Some(ClosedFloatMeaning {
                format,
                meaning,
                type_reference: operator.return_type,
            })
        }

        pub fn closed_float_semantic_integer_operand(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            expression: ExpressionHandle,
            carrier: numerics::float_semantics_catalog::IntegerCarrier,
        ) -> Option<numerics::bignum::BigInt> {
            let value = program.closed_integer_value_in(expression, owner)?;
            (value.primitive.and_then(|primitive| {
                numerics::float_semantics_catalog::IntegerCarrier::from_spelling(primitive.name())
            }) == Some(carrier)
                && carrier.format().contains(&value.value))
            .then_some(value.value)
        }

        fn closed_projection_expression(
            operation: FloatProjectionOperation,
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            remaining_work: &mut usize,
        ) -> Option<(IeeeFloatFormat, FloatMeaning)> {
            *remaining_work = remaining_work.checked_sub(1)?;
            match program.expression_table.expression(expression) {
                ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                    closed_projection_expression(operation, program, cast.value, remaining_work)
                }
                source => closed_projection_source(operation, source),
            }
        }

        fn closed_projection_source(
            operation: FloatProjectionOperation,
            source: &ExpressionNode,
        ) -> Option<(IeeeFloatFormat, FloatMeaning)> {
            match (operation, source) {
                (
                    FloatProjectionOperation::Meaning32,
                    ExpressionNode::UndeterminedNaN(numerics::literals::FloatFormat::F32),
                ) => Some((IeeeFloatFormat::Binary32, FloatMeaning::NaN)),
                (
                    FloatProjectionOperation::Meaning64,
                    ExpressionNode::UndeterminedNaN(numerics::literals::FloatFormat::F64),
                ) => Some((IeeeFloatFormat::Binary64, FloatMeaning::NaN)),
                (FloatProjectionOperation::Meaning32, ExpressionNode::Float(literal)) => Some((
                    IeeeFloatFormat::Binary32,
                    FloatMeaning::from_f32(f32::from_bits(literal.f32_bits())),
                )),
                (FloatProjectionOperation::Meaning64, ExpressionNode::Float(literal)) => Some((
                    IeeeFloatFormat::Binary64,
                    FloatMeaning::from_f64(literal.landed_f64()),
                )),
                _ => None,
            }
        }

        pub fn exact_toolchain_float_format_const(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<IeeeFloatFormat> {
            const FORMAT_CONST_IDENTITY: [(IeeeFloatFormat, &str); 2] = [
                (
                    IeeeFloatFormat::Binary32,
                    "toolchain::FloatFormat::BINARY32",
                ),
                (
                    IeeeFloatFormat::Binary64,
                    "toolchain::FloatFormat::BINARY64",
                ),
            ];
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    if !path.symbol.is_valid()
                        || !symbol_is_declared_in_sealed_source(
                            program,
                            path.symbol,
                            FLOAT_FORMAT_CORE_SOURCE,
                        )
                    {
                        return None;
                    }
                    match program
                        .normalized_hermetic_symbol_identity(path.symbol)
                        .ok()?
                        .as_str()
                    {
                        "toolchain::FloatFormat::BINARY32" => Some(IeeeFloatFormat::Binary32),
                        "toolchain::FloatFormat::BINARY64" => Some(IeeeFloatFormat::Binary64),
                        _ => None,
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    if !literal.type_symbol.is_valid()
                        || program
                            .normalized_hermetic_symbol_identity(literal.type_symbol)
                            .ok()
                            .as_deref()
                            != Some("toolchain::FloatFormat")
                        || !symbol_is_declared_in_sealed_source(
                            program,
                            literal.type_symbol,
                            FLOAT_FORMAT_CORE_SOURCE,
                        )
                    {
                        return None;
                    }
                    FORMAT_CONST_IDENTITY
                        .iter()
                        .find(|(_, identity)| {
                            program.const_declarations().iter().any(|declaration| {
                                program
                                    .normalized_hermetic_symbol_identity(declaration.symbol)
                                    .ok()
                                    .as_deref()
                                    == Some(*identity)
                                    && symbol_is_declared_in_sealed_source(
                                        program,
                                        declaration.symbol,
                                        FLOAT_FORMAT_CORE_SOURCE,
                                    )
                                    && declaration
                                        .evaluated_value
                                        .as_ref()
                                        .and_then(|value| value.encoding())
                                        .and_then(|encoding| {
                                            CanonicalConstValue::new("", encoding, "")
                                                .decode_encoding()
                                        })
                                        .is_some_and(|decoded| {
                                            struct_literal_matches_decoded_const(
                                                program, expression, &decoded,
                                            )
                                        })
                            })
                        })
                        .map(|(format, _)| *format)
                }
                _ => None,
            }
        }

        fn struct_literal_matches_decoded_const(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            expected: &language_semantics::const_value::DecodedCanonicalConstValue,
        ) -> bool {
            use language_semantics::const_value::DecodedCanonicalConstValue as Decoded;
            match (program.expression_table.expression(expression), expected) {
                (ExpressionNode::Integer(literal), Decoded::Integer { value, .. }) => {
                    literal
                        .value_i64()
                        .map(i128::from)
                        .or_else(|| literal.value_u64().map(i128::from))
                        == Some(*value)
                }
                (ExpressionNode::Boolean(observed), Decoded::Boolean(expected)) => {
                    observed == expected
                }
                (ExpressionNode::StructLiteral(literal), Decoded::Record { fields, .. }) => {
                    let observed = program.expression_table.struct_fields(literal.fields);
                    observed.len() == fields.len()
                        && fields.iter().all(|(name, expected_field)| {
                            observed
                                .iter()
                                .find(|field| field.name.as_str() == name.as_str())
                                .is_some_and(|field| {
                                    struct_literal_matches_decoded_const(
                                        program,
                                        field.value,
                                        expected_field,
                                    )
                                })
                        })
                }
                _ => false,
            }
        }
    }
    pub use conversion_contracts::{
        float_format_conversion_contract, float_integer_conversion_bounds,
        float_integer_conversion_contract, integer_float_conversion_bounds,
        numeric_float_conversion_contract,
    };

    use diagnostics::Diagnostic;
    use numerics::float_projection::{FloatProjectionContractIdentity, FloatProjectionOperation};
    use numerics::float_semantics::FloatFormat;
    use typed_trees::operator::OperatorDefinition;
    use typed_trees::types::PrimitiveType;

    fn has_exact_toolchain_float_projection_owner(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        projection: FloatProjectionOperation,
    ) -> bool {
        let expected_identity = format!(
            "toolchain::{}::{}",
            projection.source_namespace(),
            projection.source_name(),
        );
        if program
            .normalized_hermetic_symbol_identity(symbol)
            .ok()
            .as_deref()
            != Some(expected_identity.as_str())
        {
            return false;
        }
        symbol_in_toolchain_source(
            program,
            symbol,
            std::path::Path::new(numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE),
        )
    }

    pub fn is_exact_toolchain_float_meaning_type(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        if name.as_str() != "FloatMeaning"
            || program
                .normalized_hermetic_symbol_identity(*symbol)
                .ok()
                .as_deref()
                != Some("toolchain::FloatMeaning")
        {
            return false;
        }
        symbol_in_toolchain_source(
            program,
            *symbol,
            std::path::Path::new(numerics::float_projection::FLOAT_MEANING_CORE_SOURCE),
        ) && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == *symbol && data.name.as_str() == "FloatMeaning")
    }

    pub fn exact_toolchain_float_projection_primitive(
        program: &dyn crate::CheckingInput,
        operator: &OperatorDefinition,
        projection: FloatProjectionOperation,
    ) -> Option<PrimitiveType> {
        exact_toolchain_float_projection_contract(program, operator, projection)
            .map(|(primitive, _)| primitive)
    }

    pub fn exact_toolchain_float_projection_contract(
        program: &dyn crate::CheckingInput,
        operator: &OperatorDefinition,
        projection: FloatProjectionOperation,
    ) -> Option<(PrimitiveType, FloatProjectionContractIdentity)> {
        if !has_exact_toolchain_float_projection_owner(program, operator.symbol, projection) {
            return None;
        }
        let expected_primitive = if projection.rule().source_format == FloatFormat::BINARY32 {
            PrimitiveType::F32
        } else {
            debug_assert_eq!(projection.rule().source_format, FloatFormat::BINARY64);
            PrimitiveType::F64
        };
        let [parameter] = program.operator_parameters(operator) else {
            return None;
        };
        (!operator.is_public
            && !operator.is_boundary
            && operator.spelling.is_none()
            && operator.lifetime_parameters.is_empty()
            && program.operator_type_parameters(operator).is_empty()
            && !parameter.is_const
            && !parameter.is_mutable
            && program.operator_contracts(operator).is_empty()
            && program.primitive_type_reference(parameter.type_reference)
                == Some(expected_primitive)
            && is_exact_toolchain_float_meaning_type(program, operator.return_type))
        .then_some((expected_primitive, projection.contract_identity()))
    }

    pub(crate) fn validate_float_projection_operator_bindings(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        semantic_operations::validate_float_semantic_operation_bindings(program, diagnostics);
        for operator in program.operators() {
            let path = program.operator_path_members(operator.name);
            let [namespace, name] = path else {
                continue;
            };
            let Some(projection) =
                FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
            else {
                continue;
            };

            let expected_primitive = if projection.rule().source_format == FloatFormat::BINARY32 {
                PrimitiveType::F32
            } else {
                debug_assert_eq!(projection.rule().source_format, FloatFormat::BINARY64);
                PrimitiveType::F64
            };
            if !has_exact_toolchain_float_projection_owner(program, operator.symbol, projection) {
                continue;
            }
            let signature_matches =
                exact_toolchain_float_projection_primitive(program, operator, projection)
                    == Some(expected_primitive);
            if !signature_matches {
                diagnostics.push(Diagnostic::error(format!(
                "canonical float projection `{}::{}` from `{}` must be one private, contract-free ordinary tokenless operator from `{}` to the sealed toolchain `FloatMeaning` from `{}`, with no lifetime/type parameters or receiver qualification",
                projection.source_namespace(),
                projection.source_name(),
                numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                match expected_primitive {
                    PrimitiveType::F32 => "f32",
                    PrimitiveType::F64 => "f64",
                    _ => unreachable!("closed float projection primitive"),
                },
                numerics::float_projection::FLOAT_MEANING_CORE_SOURCE,
            )));
            }
        }
    }

    fn symbol_in_toolchain_source(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        relative: &std::path::Path,
    ) -> bool {
        relative.to_str().is_some_and(|relative| {
            semantic_operations::symbol_is_declared_in_sealed_source(program, symbol, relative)
        })
    }
}

pub mod float_projection_invocations {
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use numerics::float_projection::FloatProjectionContractIdentity;
    use numerics::float_projection::FloatProjectionOperation;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
    };
    use typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedFloatMeaningProjectionInvocation {
        pub invocation: ExpressionHandle,
        pub source: ExpressionHandle,
        pub selected_operator_symbol: SymbolHandle,
        pub source_primitive: PrimitiveType,
        pub operation: FloatProjectionOperation,
        pub contract: FloatProjectionContractIdentity,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedFloatMeaningEqualityProposition {
        pub expression: ExpressionHandle,
        pub left: ExpressionHandle,
        pub right: ExpressionHandle,
    }

    pub fn exact_projection_operation(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<(
        SymbolHandle,
        PrimitiveType,
        FloatProjectionOperation,
        FloatProjectionContractIdentity,
    )> {
        let operator = resolve_projection_call(program, call)?;
        let [namespace, name] = program.operator_path_members(operator.name) else {
            return None;
        };
        let operation =
            FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())?;
        let (primitive, contract) =
        crate::proof_contracts::float_projection_bindings::exact_toolchain_float_projection_contract(
            program, operator, operation,
        )?;
        Some((operator.symbol, primitive, operation, contract))
    }

    fn requested_projection_operation(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<FloatProjectionOperation> {
        let path = expression_table(program).name_path_node(call.receiver)?;
        let [namespace] = expression_table(program).name_path_members(path.members) else {
            return None;
        };
        FloatProjectionOperation::from_source_identity(namespace.as_str(), call.target.as_str())
    }

    fn resolve_projection_call<'program>(
        program: &'program dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<&'program typed_trees::operator::OperatorDefinition> {
        resolve_named_expression_call(program, call)
            .or_else(|| float_static_call_resolution(program, call))
    }

    pub fn float_static_call_resolution<'program>(
        program: &'program dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<&'program typed_trees::operator::OperatorDefinition> {
        let path = expression_table(program).name_path_node(call.receiver)?;
        let [namespace] = expression_table(program).name_path_members(path.members) else {
            return None;
        };
        if namespace.as_str() != "Float" {
            return None;
        }
        let static_receiver = [namespace.as_str()];
        resolve_named_call(
            program,
            call.target_symbol,
            Some(&static_receiver),
            call.target.as_str(),
            expression_table(program)
                .expression_handles(call.arguments)
                .len(),
            false,
        )
    }

    fn walk_expression(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        visited: &mut Vec<ExpressionHandle>,
        projections: &mut Vec<ValidatedFloatMeaningProjectionInvocation>,
        equalities: &mut Vec<ValidatedFloatMeaningEqualityProposition>,
    ) -> Result<(), Diagnostic> {
        if !expression.is_valid() || visited.contains(&expression) {
            return Ok(());
        }
        visited.push(expression);
        let node = expression_table(program).expression(expression);
        match node {
            ExpressionNode::Binary(binary) => {
                walk_expression(program, binary.left, visited, projections, equalities)?;
                walk_expression(program, binary.right, visited, projections, equalities)?;
                let float_meaning_operands = (|| {
                    let left = projections
                        .iter()
                        .find(|projection| projection.invocation == binary.left)?;
                    let right = projections
                        .iter()
                        .find(|projection| projection.invocation == binary.right)?;
                    Some((left, right))
                })();
                if binary.operator == BinaryOperator::Equal
                    && let Some((left, right)) = float_meaning_operands
                {
                    if left.operation != right.operation || left.contract != right.contract {
                        return Err(Diagnostic::error(
                            "FloatMeaningEqual requires two projections with the same exact format and projection contract",
                        ));
                    }
                    equalities.push(ValidatedFloatMeaningEqualityProposition {
                        expression,
                        left: binary.left,
                        right: binary.right,
                    });
                }
                return Ok(());
            }
            ExpressionNode::Call(call) => {
                walk_expression(program, call.receiver, visited, projections, equalities)?;
                for argument in expression_table(program).expression_handles(call.arguments) {
                    walk_expression(program, *argument, visited, projections, equalities)?;
                }
                if let Some(requested_operation) = requested_projection_operation(program, call) {
                    let Some((selected_operator_symbol, source_primitive, operation, contract)) =
                        exact_projection_operation(program, call)
                    else {
                        return Err(Diagnostic::error(
                            "proof-only float-meaning projection call did not resolve one exact canonical operator signature",
                        ));
                    };
                    if operation != requested_operation {
                        return Err(Diagnostic::error(
                            "proof-only float-meaning projection call resolved a different canonical operation",
                        ));
                    }
                    let [source] = expression_table(program).expression_handles(call.arguments)
                    else {
                        return Err(Diagnostic::error(
                            "validated float-meaning projection did not retain one source operand",
                        ));
                    };
                    let operator = resolve_projection_call(program, call)
                        .expect("exact projection operation resolved its operator");
                    let [parameter] = program.operator_parameters(operator) else {
                        unreachable!("exact projection operation checked one parameter");
                    };
                    if !crate::value_custody::expression_types::argument_matches_type_reference_handle(
                    program,
                    *source,
                    parameter.type_reference,
                ) {
                    return Err(Diagnostic::error(
                        "validated float-meaning projection source no longer matches its exact format",
                    ));
                }
                    projections.push(ValidatedFloatMeaningProjectionInvocation {
                        invocation: expression,
                        source: *source,
                        selected_operator_symbol,
                        source_primitive,
                        operation,
                        contract,
                    });
                }
                return Ok(());
            }
            _ => {}
        }
        let mut failure = None;
        crate::queries::expression_traversal::children(program, node, |child| {
            if failure.is_none()
                && let Err(diagnostic) =
                    walk_expression(program, child, visited, projections, equalities)
            {
                failure = Some(diagnostic);
            }
        });
        match failure {
            Some(diagnostic) => Err(diagnostic),
            None => Ok(()),
        }
    }

    pub(crate) fn collect_float_meaning_projection_invocations(
        program: &dyn crate::CheckingInput,
    ) -> Result<
        (
            Vec<ValidatedFloatMeaningProjectionInvocation>,
            Vec<ValidatedFloatMeaningEqualityProposition>,
        ),
        Vec<Diagnostic>,
    > {
        let mut visited = Vec::new();
        let mut projections = Vec::new();
        let mut equalities = Vec::new();
        for (_, fact) in typed_trees::references::program_proof_facts(program) {
            let roots: &[ExpressionHandle] = match fact {
                ProofFact::Expression(expression) => std::slice::from_ref(expression),
                ProofFact::Membership(membership) => std::slice::from_ref(&membership.value),
                ProofFact::Proposition(application) => {
                    expression_table(program).expression_handles(application.arguments)
                }
            };
            for root in roots {
                walk_expression(
                    program,
                    *root,
                    &mut visited,
                    &mut projections,
                    &mut equalities,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
            }
        }
        projections.sort_by_key(|projection| projection.invocation.arena_index());
        equalities.sort_by_key(|proposition| proposition.expression.arena_index());
        Ok((projections, equalities))
    }
}

pub mod mathematical_applications {
    /*
    Source custody for mathematical applications, shared by signature
    elaboration, specification formation and fact substitution. A mathematical
    definition owns a checked dependent telescope and transparent term; it is
    never an executable machine, a runtime result occurrence or a purity claim.
    */

    pub(crate) mod fixed_core_vocabulary {
        use symbols::{FixedCoreDeclaration, SymbolHandle, SymbolTable};
        use typed_trees::name::Identifier;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum FixedCore {
            Level,
            Type,
            Strict,
            Squash,
            /// The sealed dependent identity relation.
            Identity,
        }

        impl FixedCore {
            pub const fn name(self) -> &'static str {
                match self {
                    Self::Level => "core::Level",
                    Self::Type => "core::Type",
                    Self::Strict => "core::Strict",
                    Self::Squash => "core::Squash",
                    Self::Identity => "core::Identity",
                }
            }

            pub fn universe_sort(
                self,
            ) -> Option<fn(proof_admission::Level) -> proof_admission::Sort> {
                match self {
                    Self::Type => Some(proof_admission::Sort::Type),
                    Self::Strict => Some(proof_admission::Sort::Strict),
                    Self::Level | Self::Squash | Self::Identity => None,
                }
            }

            const fn declaration(self) -> FixedCoreDeclaration {
                match self {
                    Self::Level => FixedCoreDeclaration::Level,
                    Self::Type => FixedCoreDeclaration::Type,
                    Self::Strict => FixedCoreDeclaration::Strict,
                    Self::Squash => FixedCoreDeclaration::Squash,
                    Self::Identity => FixedCoreDeclaration::Identity,
                }
            }
        }

        pub struct FixedCoreVocabulary {
            level: SymbolHandle,
            ty: SymbolHandle,
            strict: SymbolHandle,
            squash: SymbolHandle,
            identity: SymbolHandle,
        }

        impl FixedCoreVocabulary {
            pub fn resolve(symbols: &SymbolTable) -> Self {
                let find = |member: FixedCore| -> SymbolHandle {
                    symbols
                        .fixed_core_declaration_symbol(member.declaration())
                        .unwrap_or_else(SymbolHandle::invalid)
                };
                Self {
                    level: find(FixedCore::Level),
                    ty: find(FixedCore::Type),
                    strict: find(FixedCore::Strict),
                    squash: find(FixedCore::Squash),
                    identity: find(FixedCore::Identity),
                }
            }

            pub fn classify(&self, symbol: SymbolHandle, name: &Identifier) -> Option<FixedCore> {
                if symbol.is_valid() {
                    if symbol == self.level {
                        return Some(FixedCore::Level);
                    }
                    if symbol == self.ty {
                        return Some(FixedCore::Type);
                    }
                    if symbol == self.strict {
                        return Some(FixedCore::Strict);
                    }
                    if symbol == self.squash {
                        return Some(FixedCore::Squash);
                    }
                    if symbol == self.identity {
                        return Some(FixedCore::Identity);
                    }
                    return None;
                }
                FIXED_CORE_FALLBACK
                    .iter()
                    .find(|(_, spelling)| spelling == &name.as_str())
                    .map(|(member, _)| *member)
            }
        }

        const FIXED_CORE_FALLBACK: &[(FixedCore, &str)] = &[
            (FixedCore::Level, FixedCore::Level.name()),
            (FixedCore::Type, FixedCore::Type.name()),
            (FixedCore::Strict, FixedCore::Strict.name()),
            (FixedCore::Squash, FixedCore::Squash.name()),
        ];
    }
    mod identity {
        /*
        The sealed identity declaration retains its carrier and exact source subjects.

        This view forms a statement; it grants neither a witness nor a value law.
        Source elaboration checks its kernel Id, and consumers retain the same
        selected declaration instead of recognizing a call's displayed spelling.
        */

        use symbols::FixedCoreDeclaration;
        use typed_trees::expression::{ExpressionHandle, TableCallExpression};
        use typed_trees::types::TypeReferenceHandle;

        #[derive(Clone, Copy)]
        pub struct IdentityApplication {
            pub carrier: TypeReferenceHandle,
            pub subjects: [ExpressionHandle; 2],
        }

        /// Read a direct declaration use or its emitted checked value after ordinary
        /// transparent application. Only the authenticated Id row supplies meaning;
        /// the source occurrences come from that application's exact capture roster.
        pub fn identity_application_at(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Result<IdentityApplication, &'static str> {
            if let Some(call) = program.expression_table.call_node(expression)
                && is_identity_call(program, call)
            {
                return identity_application(program, call);
            }
            let view = program
                .prepared_mathematics()
                .ok_or("identity has no checked mathematical view")?;
            let application = view
                .application(expression)
                .ok_or("expression has no checked mathematical application")?;
            let term = match view.arena.get(application.term) {
                proof_admission::Term::Squash { ty } => ty,
                _ => application.term,
            };
            let (identity, subjects) = view
                .identity_value(application, term, 0)
                .ok_or("expression does not emit an authenticated typed Id")?;
            let captures = view
                .applications
                .captures
                .span_or_empty(application.captures);
            let capture = |term| {
                let proof_admission::Term::Variable(index) = view.arena.get(term) else {
                    return Err("canonical content Id subject has no retained source capture");
                };
                let position = captures
                    .len()
                    .checked_sub(index as usize + 1)
                    .ok_or("canonical content Id subject is outside its capture telescope")?;
                let value = &captures[position];
                let observed = super::subject_observation_type_reference(
                    program,
                    value.source_type,
                    value.observation_type,
                )
                .ok_or("canonical content Id subject has no readable source value")?;
                if identity.type_identity != program.normalized_type_identity(observed) {
                    return Err("canonical content Id subject lost its exact source carrier");
                }
                Ok((value, observed))
            };
            let left = capture(subjects[0])?;
            let right = capture(subjects[1])?;
            Ok(IdentityApplication {
                carrier: left.1,
                subjects: [left.0.expression, right.0.expression],
            })
        }

        pub fn is_identity_call(
            program: &dyn crate::CheckingInput,
            call: &TableCallExpression,
        ) -> bool {
            is_identity_declaration(program, call.target_symbol)
        }

        pub(super) fn is_identity_declaration(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
        ) -> bool {
            symbol.is_valid()
                && program
                    .symbols
                    .fixed_core_declaration_symbol(FixedCoreDeclaration::Identity)
                    == Some(symbol)
        }

        pub fn identity_application(
            program: &dyn crate::CheckingInput,
            call: &TableCallExpression,
        ) -> Result<IdentityApplication, &'static str> {
            if !is_identity_call(program, call) {
                return Err("identity requires the sealed core declaration");
            }
            if call.receiver.is_valid()
                || call.static_machine_parameter.is_valid()
                || !call.selects_only_nominal_route()
                || !call.evidence_arguments.is_empty()
                || call.operational_acknowledgement.acknowledges_suspend
                || call.operational_acknowledgement.acknowledges_block
            {
                return Err(
                    "core::Identity cannot carry a receiver, machine, evidence or operational argument",
                );
            }
            let [argument] = call.machine_arguments.as_ref() else {
                return Err("core::Identity requires exactly one type argument");
            };
            if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
                return Err("core::Identity requires a type, not a value or evidence argument");
            }
            let carrier = if argument.type_reference.is_valid() {
                argument.type_reference
            } else if argument.symbol.is_valid() {
                program
                    .type_reference_table
                    .find_named_type_reference(argument.symbol)
                    .ok_or("core::Identity type argument has no retained type")?
            } else {
                return Err("core::Identity type argument is unresolved");
            };
            if !program
                .type_reference_table
                .contains_type_reference(carrier)
            {
                return Err("core::Identity type argument is unavailable");
            }
            let [left, right] = program.expression_table.expression_handles(call.arguments) else {
                return Err("core::Identity requires exactly two subjects");
            };
            Ok(IdentityApplication {
                carrier,
                subjects: [*left, *right],
            })
        }
    }
    pub use fixed_core_vocabulary::{FixedCore, FixedCoreVocabulary};
    pub use identity::{
        IdentityApplication, identity_application, identity_application_at, is_identity_call,
    };
    use typed_trees::type_identity::TypeIdentityRequest;
    pub(crate) mod static_arguments {
        use super::{FixedCore, FixedCoreVocabulary};
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone, Copy)]
        pub enum MathematicalCarrierHead {
            Level,
            Type(TypeReferenceHandle),
            MalformedLevel,
            MalformedType(usize),
            Term,
        }

        pub fn mathematical_carrier_head(
            program: &dyn crate::CheckingInput,
            carrier: TypeReferenceHandle,
            vocabulary: &FixedCoreVocabulary,
        ) -> MathematicalCarrierHead {
            if !program
                .type_reference_table
                .contains_type_reference(carrier)
            {
                return MathematicalCarrierHead::Term;
            }
            match program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::Named { symbol, name } => {
                    match vocabulary.classify(*symbol, name) {
                        Some(FixedCore::Level) => MathematicalCarrierHead::Level,
                        Some(FixedCore::Type) => {
                            MathematicalCarrierHead::Type(TypeReferenceHandle::invalid())
                        }
                        _ => MathematicalCarrierHead::Term,
                    }
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => match vocabulary.classify(*base_symbol, base_name) {
                    Some(FixedCore::Level) => MathematicalCarrierHead::MalformedLevel,
                    Some(FixedCore::Type) => {
                        match program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                        {
                            [universe] => MathematicalCarrierHead::Type(*universe),
                            arguments => MathematicalCarrierHead::MalformedType(arguments.len()),
                        }
                    }
                    _ => MathematicalCarrierHead::Term,
                },
                _ => MathematicalCarrierHead::Term,
            }
        }

        #[derive(Clone, Copy)]
        pub enum MathematicalBinderKind {
            Level,
            Type,
            Term,
        }

        pub fn mathematical_binder_kind(
            program: &dyn crate::CheckingInput,
            binder: &TypeParameter,
            vocabulary: &FixedCoreVocabulary,
        ) -> Option<MathematicalBinderKind> {
            match binder.kind {
                TypeParameterKind::Type => Some(MathematicalBinderKind::Type),
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    match mathematical_carrier_head(program, type_reference, vocabulary) {
                        MathematicalCarrierHead::Level => Some(MathematicalBinderKind::Level),
                        MathematicalCarrierHead::Type(_) => Some(MathematicalBinderKind::Type),
                        MathematicalCarrierHead::Term => Some(MathematicalBinderKind::Term),
                        MathematicalCarrierHead::MalformedLevel
                        | MathematicalCarrierHead::MalformedType(_) => None,
                    }
                }
                _ => None,
            }
        }

        pub struct MathematicalGenericArgument<'source> {
            pub binder: &'source TypeParameter,
            pub kind: MathematicalBinderKind,
            pub argument: Option<&'source StaticMachineArgument>,
        }

        pub fn source_generic_argument_plan<'source>(
            program: &dyn crate::CheckingInput,
            binders: &'source [TypeParameter],
            arguments: &'source [StaticMachineArgument],
            vocabulary: &FixedCoreVocabulary,
        ) -> Option<Vec<MathematicalGenericArgument<'source>>> {
            let kinds = binders
                .iter()
                .map(|binder| mathematical_binder_kind(program, binder, vocabulary))
                .collect::<Option<Vec<_>>>()?;
            let level_count = kinds
                .iter()
                .filter(|kind| matches!(kind, MathematicalBinderKind::Level))
                .count();
            let omit_levels =
                arguments.len() != binders.len() && arguments.len() == binders.len() - level_count;
            if arguments.len() != binders.len() && !omit_levels {
                return None;
            }
            let mut arguments = arguments.iter();
            binders
                .iter()
                .zip(kinds)
                .map(|(binder, kind)| {
                    let argument = if omit_levels && matches!(kind, MathematicalBinderKind::Level) {
                        None
                    } else {
                        Some(arguments.next()?)
                    };
                    Some(MathematicalGenericArgument {
                        binder,
                        kind,
                        argument,
                    })
                })
                .collect()
        }
    }
    pub use static_arguments::{
        MathematicalBinderKind, MathematicalCarrierHead, mathematical_binder_kind,
        mathematical_carrier_head, source_generic_argument_plan,
    };
    pub(crate) mod expressions {
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use std::collections::HashSet;
        use std::sync::Arc;
        use symbols::SymbolHandle;

        use typed_trees::expression::ExpressionHandle;
        use typed_trees::mathematical::{MathematicalBody, MathematicalType};

        use crate::queries::expression_traversal::collect_expression_nodes;

        pub fn expression_nodes(program: &dyn CheckingInput) -> Vec<ExpressionHandle> {
            crate::frozen_program::frozen_memo(
                program,
                |memos| &memos.mathematical_expression_nodes,
                || {
                    let mut seen = HashSet::new();
                    scoped_expression_nodes(program)
                        .into_iter()
                        .filter_map(|(_, expression)| seen.insert(expression).then_some(expression))
                        .collect()
                },
            )
            .as_ref()
            .clone()
        }

        pub fn scoped_expression_nodes(
            program: &dyn CheckingInput,
        ) -> Vec<(SymbolHandle, ExpressionHandle)> {
            let mut expressions = Vec::new();
            for definition in program.mathematical_definitions() {
                let mut owned = Vec::new();
                let mut expressions_seen = HashSet::new();
                for root in definition_roots(program, definition) {
                    collect_expression_nodes(program, root, &mut owned, &mut expressions_seen);
                }
                expressions.extend(
                    owned
                        .into_iter()
                        .map(|expression| (definition.symbol, expression)),
                );
            }
            if expressions.is_empty() {
                return expressions;
            }
            let mut executable = Vec::new();
            let mut executable_seen = HashSet::new();
            for machine in program.machines() {
                for data in program.machine_owned_data(machine) {
                    collect_expression_nodes(
                        program,
                        data.initial_value,
                        &mut executable,
                        &mut executable_seen,
                    );
                }
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        for root in crate::machine_calls::calls::statement_value_expression_roots(
                            program, statement,
                        ) {
                            collect_expression_nodes(
                                program,
                                root,
                                &mut executable,
                                &mut executable_seen,
                            );
                        }
                    }
                }
            }
            expressions.retain(|(_, expression)| !executable_seen.contains(expression));
            expressions
        }

        pub fn definition_roots(
            program: &dyn CheckingInput,
            definition: &typed_trees::mathematical::MathematicalDefinition,
        ) -> Vec<ExpressionHandle> {
            let mut roots = Vec::new();
            if let MathematicalBody::Definition(body) = definition.body {
                roots.push(body);
            }
            let mut types = vec![definition.result];
            types.extend(
                program
                    .mathematical_parameters(definition.parameters)
                    .iter()
                    .map(|parameter| parameter.ty),
            );
            let mut visited = Vec::new();
            while let Some(handle) = types.pop() {
                if !handle.is_valid() || visited.contains(&handle) {
                    continue;
                }
                visited.push(handle);
                match program.mathematical_type(handle) {
                    MathematicalType::Ordinary(_) => {}
                    MathematicalType::Arrow {
                        domain, codomain, ..
                    } => {
                        types.push(*domain);
                        types.push(*codomain);
                    }
                    MathematicalType::Application { callee, arguments } => {
                        types.push(*callee);
                        roots.extend_from_slice(
                            expression_table(program).expression_handles(*arguments),
                        );
                    }
                }
            }
            roots
        }

        pub fn erased_initializer_nodes(program: &dyn CheckingInput) -> Arc<Vec<ExpressionHandle>> {
            crate::frozen_program::frozen_memo(
                program,
                |memos| &memos.erased_initializers,
                || {
                    let mut placements = InitializerAccesss::default();
                    for machine in program.machines() {
                        for data in program.machine_owned_data(machine) {
                            placements.visit(program, data.initial_value, false);
                        }
                        for state in program.machine_states(machine) {
                            for statement in
                                program.statement_table.statements(state.statement_nodes)
                            {
                                use typed_trees::statement::{
                                    StatementNode, TransitionGuardNode, TransitionTargetNode,
                                };
                                match statement {
                            StatementNode::LocalData(local) => placements.visit(
                                program,
                                local.initial_value,
                                local.relevance.is_erased(),
                            ),
                            StatementNode::Call(call) => {
                                for (position, argument) in program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .enumerate()
                                {
                                    placements.visit(
                                        program,
                                        *argument,
                                        erased_parameter(program, call.target_symbol, position),
                                    );
                                }
                            }
                            StatementNode::Transition(transition) => {
                                if let TransitionGuardNode::When(guard) = transition.guard {
                                    placements.visit(program, guard, false);
                                }
                                for target in transition.targets() {
                                    match program.statement_table.transition_target(target) {
                                        TransitionTargetNode::Named {
                                            path, arguments, ..
                                        } => {
                                            for (position, argument) in program
                                                .statement_table
                                                .expression_handles(*arguments)
                                                .iter()
                                                .enumerate()
                                            {
                                                placements.visit(
                                                    program,
                                                    *argument,
                                                    erased_parameter(
                                                        program,
                                                        path.symbol,
                                                        position,
                                                    ),
                                                );
                                            }
                                        }
                                        TransitionTargetNode::Value(value) => {
                                            placements.visit(program, *value, false)
                                        }
                                        TransitionTargetNode::Terminal => {}
                                    }
                                }
                            }
                            StatementNode::AssemblyFact(_) => {}
                            _ => {
                                for root in
                                    crate::machine_calls::calls::statement_value_expression_roots(
                                        program, statement,
                                    )
                                {
                                    placements.visit(program, root, false);
                                }
                            }
                        }
                            }
                        }
                    }
                    placements
                        .erased
                        .retain(|expression| !placements.runtime_seen.contains(expression));
                    placements.erased
                },
            )
        }

        fn erased_parameter(
            program: &dyn CheckingInput,
            target: SymbolHandle,
            position: usize,
        ) -> bool {
            program
                .state_by_symbol(target)
                .or_else(|| {
                    program
                        .machine_by_symbol(target)
                        .and_then(|machine| program.machine_states(machine).first())
                })
                .and_then(|state| program.state_parameters(state).get(position))
                .is_some_and(|parameter| parameter.relevance.is_erased())
        }

        #[derive(Default)]
        pub(crate) struct InitializerAccesss {
            erased: Vec<ExpressionHandle>,
            erased_seen: HashSet<ExpressionHandle>,
            runtime_seen: HashSet<ExpressionHandle>,
        }

        impl InitializerAccesss {
            fn visit(
                &mut self,
                program: &dyn CheckingInput,
                expression: ExpressionHandle,
                erased: bool,
            ) {
                if !expression_table(program).expression_is_valid(expression) {
                    return;
                }
                if erased {
                    if !self.erased_seen.insert(expression) {
                        return;
                    }
                    self.erased.push(expression);
                } else if !self.runtime_seen.insert(expression) {
                    return;
                }
                use typed_trees::expression::ExpressionNode;
                match expression_table(program).expression(expression) {
                    ExpressionNode::Call(call) => {
                        self.visit(program, call.receiver, erased);
                        for (position, argument) in expression_table(program)
                            .expression_handles(call.arguments)
                            .iter()
                            .enumerate()
                        {
                            self.visit(
                                program,
                                *argument,
                                erased || erased_parameter(program, call.target_symbol, position),
                            );
                        }
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        for field in expression_table(program).struct_fields(literal.fields) {
                            let field_erased = field.field_symbol.is_valid()
                                && program
                                    .any_field_owner(field.field_symbol)
                                    .is_some_and(|(_, declared)| declared.relevance.is_erased());
                            self.visit(program, field.value, erased || field_erased);
                        }
                    }
                    node => {
                        crate::queries::expression_traversal::children(program, node, |child| {
                            self.visit(program, child, erased)
                        })
                    }
                }
            }
        }
    }
    pub use expressions::{
        definition_roots, erased_initializer_nodes, expression_nodes, scoped_expression_nodes,
    };
    pub(crate) mod logical_bodies {
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::statement::{StatementNode, TransitionTargetNode};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub fn is_indexed_logical_machine(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
        ) -> bool {
            crate::frozen_program::frozen_memo(
                program,
                |memos| &memos.indexed_logical_machines,
                || {
                    let mut candidates = program
                        .machines()
                        .iter()
                        .filter(|machine| has_exact_theorem(program, machine))
                        .map(|machine| machine.symbol)
                        .collect::<Vec<_>>();
                    loop {
                        let mut retained = candidates
                            .iter()
                            .copied()
                            .filter(|symbol| {
                                program.machine_by_symbol(*symbol).is_some_and(|machine| {
                                    let mut stack = Vec::new();
                                    logical_body(program, machine, &candidates, &mut stack)
                                })
                            })
                            .collect::<Vec<_>>();
                        if retained.len() == candidates.len() {
                            let mut instances = program
                                .machine_specializations
                                .iter()
                                .filter(|row| retained.contains(&row.template))
                                .map(|row| row.instance)
                                .collect::<Vec<_>>();
                            instances.retain(|symbol| !retained.contains(symbol));
                            retained.extend(instances);
                            return retained;
                        }
                        candidates = retained;
                    }
                },
            )
            .contains(&machine.symbol)
        }

        pub fn indexed_contract_is_proved(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            expression: ExpressionHandle,
        ) -> bool {
            is_indexed_logical_machine(program, machine)
                && program.prepared_mathematics().is_some_and(|view| {
                    view.applications
                        .indexed_theorems
                        .iter()
                        .any(|(_, theorem)| {
                            theorem.machine == machine.symbol
                                && theorem.expression == expression
                                && theorem.theorem.is_valid()
                                && theorem.ty.is_valid()
                        })
                })
        }

        fn has_exact_theorem(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
            let Some(view) = program.prepared_mathematics() else {
                return false;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            view.applications.indexed_theorems.iter().any(|(_, theorem)| {
        theorem.machine == machine.symbol
            && theorem.theorem.is_valid() && theorem.ty.is_valid()
            && program.state_parameters(entry).iter().any(|formal| {
                formal.symbol == theorem.evidence_formal && formal.relevance.is_erased()
                    && formal.type_reference == theorem.source_type
                    && super::is_source_evidence_carrier(program, formal.type_reference)
            })
            && program.machine_contracts(machine).iter().chain(program.state_contracts(entry)).any(|contract| {
                matches!(contract.kind, SignatureContractKind::Ensures)
                    && program.proof_facts.span_or_empty(contract.facts).iter().any(|fact| {
                        matches!(fact, ProofFact::Expression(expression) if *expression == theorem.expression)
                    })
            })
            })
        }

        fn logical_carrier(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            super::is_source_evidence_carrier(program, reference)
                || program
                    .primitive_type_reference(reference)
                    .is_some_and(|primitive| {
                        primitive == PrimitiveType::Bool || primitive.integer_bounds().is_some()
                    })
        }

        fn target_machine(
            program: &dyn crate::CheckingInput,
            target: SymbolHandle,
        ) -> Option<&Machine> {
            program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))
        }

        fn logical_body(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            candidates: &[SymbolHandle],
            stack: &mut Vec<SymbolHandle>,
        ) -> bool {
            if stack.contains(&machine.symbol) {
                return true;
            }
            stack.push(machine.symbol);
            let admitted = logical_body_contents(program, machine, candidates, stack);
            stack.pop();
            admitted
        }

        fn logical_body_contents(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            candidates: &[SymbolHandle],
            stack: &mut Vec<SymbolHandle>,
        ) -> bool {
            if !machine.supply_mode.is_checked_body()
                || !machine.body_is_present
                || !crate::machine_calls::calls::machine_has_unit_result(program, machine)
                || machine.attached_data_symbol.is_valid()
                || machine.attached_data.is_some()
                || !program.machine_owned_data(machine).is_empty()
                || !machine.invokes.is_empty()
                || machine.suspends
                || machine.blocks
                || !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty()
                || program.machine_contracts(machine).iter().any(|contract| {
                    !matches!(
                        contract.kind,
                        SignatureContractKind::Requires | SignatureContractKind::Ensures
                    )
                })
            {
                return false;
            }
            program.machine_states(machine).iter().all(|state| {
                program.state_contracts(state).iter().all(|contract| {
                    matches!(
                        contract.kind,
                        SignatureContractKind::Requires | SignatureContractKind::Ensures
                    )
                }) && program.state_parameters(state).iter().all(|parameter| {
                    !parameter.is_mutable
                        && !parameter.is_const
                        && logical_carrier(program, parameter.type_reference)
                }) && program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .all(|statement| match statement {
                        StatementNode::Assignment(_) | StatementNode::Assembly(_) => false,
                        StatementNode::AssemblyFact(_) => true,
                        StatementNode::Call(call) => {
                            call.receiver.is_empty()
                                && !call.receiver_symbol.is_valid()
                                && !call.receiver_root_symbol.is_valid()
                                && call.static_requirement_dispatch.is_none()
                                && call.evidence_arguments.is_empty()
                                && !call.operational_acknowledgement.acknowledges_suspend
                                && !call.operational_acknowledgement.acknowledges_block
                                && target_machine(program, call.target_symbol).is_some_and(
                                    |callee| {
                                        candidates.contains(&callee.symbol)
                                            || logical_body(program, callee, candidates, stack)
                                    },
                                )
                                && program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .all(|argument| {
                                        logical_expression(
                                            program, machine, state, *argument, candidates, stack,
                                        )
                                    })
                        }
                        StatementNode::LocalData(local) => !local.is_mutable
                            && (crate::proof_contracts::contract_entailment::is_arm_pattern_marker(
                                statement,
                            ) || logical_carrier(program, local.type_reference)
                                || erased_evidence_initializer(
                                    program,
                                    state,
                                    local.initial_value,
                                ))
                            && logical_expression(
                                program,
                                machine,
                                state,
                                local.initial_value,
                                candidates,
                                stack,
                            ),
                        StatementNode::Expression(expression) => logical_expression(
                            program,
                            machine,
                            state,
                            *expression,
                            candidates,
                            stack,
                        ),
                        StatementNode::Transition(transition) => {
                            transition.exit == typed_trees::statement::TransitionExit::Ordinary
                                && program.tables.expression_roots(statement).iter().all(
                                    |(_, expression)| {
                                        logical_expression(
                                            program,
                                            machine,
                                            state,
                                            *expression,
                                            candidates,
                                            stack,
                                        )
                                    },
                                )
                                && transition.targets().all(|target| {
                                    match program.statement_table.transition_target(target) {
                                        TransitionTargetNode::Named {
                                            path,
                                            operational_acknowledgement,
                                            static_machine_parameter,
                                            evidence_arguments,
                                            ..
                                        } => {
                                            !operational_acknowledgement.acknowledges_suspend
                                                && !operational_acknowledgement.acknowledges_block
                                                && !static_machine_parameter.is_valid()
                                                && evidence_arguments.is_empty()
                                                && (program
                                                    .machine_states(machine)
                                                    .iter()
                                                    .any(|state| state.symbol == path.symbol)
                                                    || target_machine(program, path.symbol)
                                                        .is_some_and(|target| {
                                                            target.symbol == machine.symbol
                                                                || candidates
                                                                    .contains(&target.symbol)
                                                                || logical_body(
                                                                    program, target, candidates,
                                                                    stack,
                                                                )
                                                        }))
                                        }
                                        TransitionTargetNode::Terminal => true,
                                        TransitionTargetNode::Value(value) => logical_expression(
                                            program, machine, state, *value, candidates, stack,
                                        ),
                                    }
                                })
                        }
                    })
            })
        }

        fn erased_evidence_initializer(
            program: &dyn crate::CheckingInput,
            state: &typed_trees::state::State,
            mut expression: ExpressionHandle,
        ) -> bool {
            for _ in 0..8 {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Borrow(borrow) => expression = borrow.target,
                    ExpressionNode::Name(path) => {
                        if program.state_parameters(state).iter().any(|parameter| {
                            parameter.symbol == path.symbol
                                && parameter.relevance.is_erased()
                                && logical_carrier(program, parameter.type_reference)
                        }) {
                            return true;
                        }
                        let Some(initial_value) = program
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .find_map(|statement| match statement {
                                StatementNode::LocalData(local) if local.symbol == path.symbol => {
                                    Some(local.initial_value)
                                }
                                _ => None,
                            })
                        else {
                            return false;
                        };
                        expression = initial_value;
                    }
                    _ => return false,
                }
            }
            false
        }

        fn logical_expression(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
            candidates: &[SymbolHandle],
            stack: &mut Vec<SymbolHandle>,
        ) -> bool {
            if !expression.is_valid() {
                return true;
            }
            let node = program.expression_table.expression(expression);
            let admitted = match node {
        ExpressionNode::Atomic(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
        ExpressionNode::Borrow(borrow) => {
            erased_evidence_initializer(program, state, borrow.target)
        }
        ExpressionNode::Call(call) => {
            !call.receiver.is_valid()
                && call.selects_only_nominal_route()
                && call.evidence_arguments.is_empty()
                && !call.operational_acknowledgement.acknowledges_suspend
                && !call.operational_acknowledgement.acknowledges_block
                && call.static_requirement_dispatch.is_none()
                && (crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call)
                    || (super::selected_definition(program, call).is_some()
                        && program
                            .prepared_mathematics()
                            .is_some_and(|view| view.application(expression).is_some()))
                    || target_machine(program, call.target_symbol).is_some_and(|callee| {
                        candidates.contains(&callee.symbol)
                            || logical_body(program, callee, candidates, stack)
                    }))
        }
        ExpressionNode::Cast(cast) => {
            !cast.form.is_recast()
                && logical_carrier(program, cast.target_type)
                && program.arithmetic_domain_for_type_reference(cast.target_type)
                    != numerics::arithmetic::ArithmeticDomain::Trapping
        }
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => {
            (match node {
                ExpressionNode::Binary(_) => crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(program, machine, Some(state), expression),
                _ => crate::proof_contracts::bound_expression_meaning::has_builtin_bound_expression_meaning(program, machine, Some(state), expression),
            }) && crate::expression_result_type_reference(program, machine, state, expression).is_some_and(|reference| {
                    program.arithmetic_domain_for_type_reference(reference)
                        != numerics::arithmetic::ArithmeticDomain::Trapping
                })
        }
        ExpressionNode::Indexed(_) => false,
        ExpressionNode::Member(member) => {
            super::operand_type_reference(program, member.receiver)
                .is_some_and(|reference| super::is_source_evidence_carrier(program, reference))
                || erased_evidence_initializer(program, state, member.receiver)
        }
        _ => true,
            };
            if !admitted {
                return false;
            }
            let mut children_admitted = true;
            crate::queries::expression_traversal::children(program, node, |child| {
                children_admitted &=
                    logical_expression(program, machine, state, child, candidates, stack);
            });
            children_admitted
        }
    }
    pub use logical_bodies::{indexed_contract_is_proved, is_indexed_logical_machine};
    pub(crate) mod float_predicates {
        use super::{PreparedConstantMeaning, PreparedMathematicalView, application_spine};
        use proof_admission::{Term, TermHandle};
        use typed_trees::expression::ExpressionHandle;

        pub struct MathematicalFloatPredicate {
            pub subject: ExpressionHandle,
            pub reference: typed_trees::types::TypeReferenceHandle,
            pub predicate: facts::FloatPredicate,
            pub operator: typed_trees::expression::BinaryOperator,
            pub bound: numerics::bignum::ExactFloat,
        }

        pub fn float_predicates(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<Vec<MathematicalFloatPredicate>> {
            let view = program.prepared_mathematics()?;
            let application = view.application(expression)?;
            let captures = view
                .applications
                .captures
                .span_or_empty(application.captures)
                .iter()
                .map(|capture| capture.expression)
                .collect::<Vec<_>>();
            let mut predicates = Vec::new();
            collect(
                program,
                view,
                application.term,
                &captures,
                &mut predicates,
                0,
            )?;
            Some(predicates)
        }

        fn collect(
            program: &dyn crate::CheckingInput,
            view: PreparedMathematicalView<'_>,
            term: TermHandle,
            captures: &[ExpressionHandle],
            predicates: &mut Vec<MathematicalFloatPredicate>,
            depth: usize,
        ) -> Option<()> {
            if depth >= 128 {
                return None;
            }
            match view.arena.get(term) {
                Term::Squash { ty } => collect(program, view, ty, captures, predicates, depth + 1),
                Term::Sigma { domain, codomain }
                    if !matches!(view.arena.get(domain), Term::Two) =>
                {
                    collect(program, view, domain, captures, predicates, depth + 1)?;
                    let mut extended = captures.to_vec();
                    extended.push(ExpressionHandle::default());
                    collect(program, view, codomain, &extended, predicates, depth + 1)
                }
                _ => {
                    let (declaration, arguments) = application_spine(view, term)?;
                    let PreparedConstantMeaning::FloatRelation {
                        operator,
                        format,
                        reference,
                    } = view.constant(declaration)?
                    else {
                        return None;
                    };
                    let [left, right] = arguments.as_slice() else {
                        return None;
                    };
                    let literal = |term| {
                        if let Term::Variable(position) = view.arena.get(term) {
                            let expression = *captures
                                .get(captures.len().checked_sub(position as usize + 1)?)?;
                            if !expression.is_valid() {
                                return None;
                            }
                            let typed_trees::expression::ExpressionNode::Float(literal) =
                                program.expression_table.peeled_expression(expression)
                            else {
                                return None;
                            };
                            if literal.landing().is_some_and(|landing| landing != *format) {
                                return None;
                            }
                            return Some(match format {
                                numerics::literals::FloatFormat::F32 => {
                                    numerics::bignum::ExactFloat::from_f32(literal.value_f32())
                                }
                                numerics::literals::FloatFormat::F64 => {
                                    numerics::bignum::ExactFloat::from_f64(literal.value_f64())
                                }
                            });
                        }
                        let (declaration, arguments) = application_spine(view, term)?;
                        if !arguments.is_empty() {
                            return None;
                        }
                        let PreparedConstantMeaning::FloatLiteral {
                            format: retained,
                            bits,
                        } = view.constant(declaration)?
                        else {
                            return None;
                        };
                        if retained != format {
                            return None;
                        }
                        Some(match format {
                            numerics::literals::FloatFormat::F32 => {
                                numerics::bignum::ExactFloat::from_f32(f32::from_bits(*bits as u32))
                            }
                            numerics::literals::FloatFormat::F64 => {
                                numerics::bignum::ExactFloat::from_f64(f64::from_bits(*bits))
                            }
                        })
                    };
                    let (subject, bound, operator) = if let Some(bound) = literal(*right) {
                        (*left, bound, *operator)
                    } else {
                        (*right, literal(*left)?, operator.mirrored())
                    };
                    let Term::Variable(position) = view.arena.get(subject) else {
                        return None;
                    };
                    let subject =
                        *captures.get(captures.len().checked_sub(position as usize + 1)?)?;
                    if !subject.is_valid() {
                        return None;
                    }
                    let predicate = facts::FloatPredicate::comparison(*format, operator, &bound)?;
                    predicates.push(MathematicalFloatPredicate {
                        subject,
                        reference: *reference,
                        predicate,
                        operator,
                        bound,
                    });
                    Some(())
                }
            }
        }
    }
    pub(crate) mod prepared {
        use arena::{Arena, HandleSpan};
        use numerics::bignum::BigInt;
        use proof_admission::{Declaration, TermArena, TermHandle};
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        use crate::proof_contracts::arithmetic_domains::BigIntInterval;

        #[derive(Debug, Default)]
        pub struct PreparedMathematicalApplications {
            pub applications: Arena<PreparedApplication>,
            pub identity_values: Arena<PreparedIdentityValue>,
            pub captures: Arena<PreparedCapture>,
            pub constants: Arena<PreparedConstant>,
            pub source_evidence: Arena<PreparedSourceEvidence>,
            pub source_constructors: Arena<PreparedSourceConstructor>,
            pub source_carriers: Arena<PreparedSourceCarrier>,
            pub indexed_theorems: Arena<PreparedIndexedTheorem>,
            pub safety_assumptions: Vec<u32>,
        }

        #[derive(Debug, Default)]
        pub struct PreparedIndexedTheorem {
            pub level_arity: u32,
            pub machine: SymbolHandle,
            pub expression: ExpressionHandle,
            pub ensure_ordinal: u32,
            pub evidence_formal: SymbolHandle,
            pub source_type: TypeReferenceHandle,
            pub theorem: TermHandle,
            pub ty: TermHandle,
            pub assumption_closure: Vec<u32>,
        }

        #[derive(Debug, Default)]
        pub struct PreparedSourceEvidence {
            pub expression: ExpressionHandle,
            pub source_type: TypeReferenceHandle,
            pub type_identity: typed_trees::type_identity::TypeIdentityId,
            pub term: TermHandle,
            pub ty: TermHandle,
        }

        #[derive(Debug, Default, Clone)]
        pub struct PreparedSourceConstructor {
            pub level_arity: u32,
            pub expression: ExpressionHandle,
            pub constructor: ExpressionHandle,
            pub source_type: TypeReferenceHandle,
            pub type_identity: typed_trees::type_identity::TypeIdentityId,
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub schema: TermHandle,
            pub ty: TermHandle,
            pub parameters: Vec<SymbolHandle>,
            pub predicate_expressions: Vec<ExpressionHandle>,
        }

        #[derive(Debug, Default)]
        pub struct PreparedSourceCarrier {
            pub level_arity: u32,
            pub source_type: TypeReferenceHandle,
            pub type_identity: typed_trees::type_identity::TypeIdentityId,
            pub ty: TermHandle,
            pub domains: Vec<TermHandle>,
        }

        #[derive(Debug, Default)]
        pub struct PreparedApplication {
            pub level_arity: u32,
            pub expression: ExpressionHandle,
            pub owner: SymbolHandle,
            pub term: TermHandle,
            pub ty: TermHandle,
            pub domains: Vec<TermHandle>,
            pub captures: HandleSpan<PreparedCapture>,
            pub identity_values: HandleSpan<PreparedIdentityValue>,
        }

        #[derive(Debug, Default)]
        pub struct PreparedCapture {
            pub expression: ExpressionHandle,
            pub source_type: TypeReferenceHandle,
            /// The value domain selected during formation; the original source
            /// still supplies access, backing and storage version obligations.
            pub observation_type: TypeReferenceHandle,
        }

        /// Exact source carrier carried by one emitted checked identity value.
        #[derive(Debug, Default)]
        pub struct PreparedIdentityValue {
            pub term: TermHandle,
            pub ty: TermHandle,
            pub type_identity: typed_trees::type_identity::TypeIdentityId,
        }

        #[derive(Debug, Default)]
        pub struct PreparedConstant {
            pub declaration: u32,
            pub meaning: PreparedConstantMeaning,
        }

        /// One position in an opaque source value's checked dependency telescope.
        /// Type positions remain present even when they have no structural value.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PreparedSourceBindingKind {
            Type,
            Value,
        }

        #[derive(Debug, Clone)]
        pub struct PreparedSourceBinding {
            /// Original domain identity fences reuse across separately prepared scopes.
            pub source_domain: TermHandle,
            /// The same domain rebound over this dependency telescope's prefix.
            pub domain: TermHandle,
            pub kind: PreparedSourceBindingKind,
            pub symbol: SymbolHandle,
            pub source_type: TypeReferenceHandle,
            pub expressions: Vec<ExpressionHandle>,
        }

        impl PreparedSourceBinding {
            pub fn same_source_binding(&self, other: &Self) -> bool {
                self.kind == other.kind
                    && self.source_domain == other.source_domain
                    && self.symbol == other.symbol
                    && self.source_type == other.source_type
                    && self.expressions == other.expressions
            }
        }

        #[derive(Debug, Default)]
        pub enum PreparedConstantMeaning {
            #[default]
            Opaque,
            /// Authenticated transparent definition whose body is kernel Id.
            Identity,
            Integer(BigInt),
            Boolean(bool),
            IntegerCarrier,
            BooleanCarrier,
            AddressCarrier,
            AddressBound(SymbolHandle),
            IntegerOperation(BinaryOperator),
            IntegerRelation(BinaryOperator),
            FloatRelation {
                operator: BinaryOperator,
                format: numerics::literals::FloatFormat,
                reference: TypeReferenceHandle,
            },
            FloatLiteral {
                format: numerics::literals::FloatFormat,
                bits: u64,
            },
            Negation,
            IntegerEmbedding {
                primitive: PrimitiveType,
                interval: BigIntInterval,
            },
            SourceOperation {
                expression: ExpressionHandle,
                owner: SymbolHandle,
                source_type: TypeReferenceHandle,
                bindings: Vec<PreparedSourceBinding>,
            },
        }

        #[derive(Clone, Copy, Debug)]
        pub struct PreparedMathematicalView<'program> {
            pub arena: &'program TermArena,
            pub declarations: &'program [Declaration],
            pub applications: &'program PreparedMathematicalApplications,
        }

        impl<'program> PreparedMathematicalView<'program> {
            pub fn source_constructor(
                self,
                expression: ExpressionHandle,
            ) -> Option<&'program PreparedSourceConstructor> {
                self.applications
                    .source_constructors
                    .iter()
                    .find_map(|(_, row)| (row.expression == expression).then_some(row))
            }

            pub fn source_evidence(
                self,
                expression: ExpressionHandle,
            ) -> Option<&'program PreparedSourceEvidence> {
                self.applications
                    .source_evidence
                    .iter()
                    .find_map(|(_, evidence)| {
                        (evidence.expression == expression).then_some(evidence)
                    })
            }

            pub fn source_carrier(
                self,
                reference: TypeReferenceHandle,
            ) -> Option<&'program PreparedSourceCarrier> {
                self.applications
                    .source_carriers
                    .iter()
                    .find_map(|(_, carrier)| (carrier.source_type == reference).then_some(carrier))
            }

            pub fn application(
                self,
                expression: ExpressionHandle,
            ) -> Option<&'program PreparedApplication> {
                self.applications
                    .applications
                    .iter()
                    .find_map(|(_, application)| {
                        (application.expression == expression).then_some(application)
                    })
            }

            /// Read the checked value emitted by the sealed declaration, including
            /// through transparent mathematical application and capture substitution.
            pub fn identity_value(
                self,
                application: &PreparedApplication,
                term: TermHandle,
                inserted_binders: u32,
            ) -> Option<(&'program PreparedIdentityValue, [TermHandle; 2])> {
                let (declaration, arguments) = application_spine(self, term)?;
                if !matches!(
                    self.constant(declaration)?,
                    PreparedConstantMeaning::Identity
                ) {
                    return None;
                }
                let [_ty, left, right] = arguments.as_slice() else {
                    return None;
                };
                let mut matches = self
                    .applications
                    .identity_values
                    .span_or_empty(application.identity_values)
                    .iter()
                    .filter(|row| {
                        row.type_identity.is_valid()
                            && self.arena.structurally_equal_shifted(
                                row.term,
                                term,
                                inserted_binders,
                            )
                    });
                let carrier = matches.next()?;
                if matches.any(|other| other.type_identity != carrier.type_identity) {
                    return None;
                }
                Some((carrier, [*left, *right]))
            }

            pub fn constant(self, declaration: u32) -> Option<&'program PreparedConstantMeaning> {
                self.applications
                    .constants
                    .iter()
                    .find_map(|(_, constant)| {
                        (constant.declaration == declaration).then_some(&constant.meaning)
                    })
            }
        }

        pub(crate) fn application_spine(
            view: PreparedMathematicalView<'_>,
            mut term: TermHandle,
        ) -> Option<(u32, Vec<TermHandle>)> {
            let mut arguments = Vec::new();
            loop {
                match view.arena.get(term) {
                    proof_admission::Term::Apply { function, argument } => {
                        arguments.push(argument);
                        term = function;
                    }
                    proof_admission::Term::Constant { declaration, .. } => {
                        arguments.reverse();
                        return Some((declaration, arguments));
                    }
                    _ => return None,
                }
            }
        }

        pub fn integer_embedding_meaning(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<PreparedConstantMeaning> {
            let (_, argument) =
                crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                    program, expression,
                )?;
            let embedding =
                crate::proof_contracts::proof_embeddings::integer_embedding(program, argument)?;
            embedding.interval.low.as_ref()?;
            Some(PreparedConstantMeaning::IntegerEmbedding {
                primitive: embedding.primitive,
                interval: embedding.interval,
            })
        }
    }
    pub(crate) mod scalar_predicates {
        use super::{PreparedConstantMeaning, PreparedMathematicalView, application_spine};
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
            CheckedScalarExpression as Scalar,
        };
        use numerics::literals::FloatFormat;
        use proof_admission::{Term, TermHandle};
        use semantic_vocabulary::IeeeFloatValue;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::types::PrimitiveType;

        pub fn scalar_predicate(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            mut capture: impl FnMut(
                ExpressionHandle,
                typed_trees::types::TypeReferenceHandle,
            ) -> Option<Scalar>,
        ) -> Option<Boolean> {
            let view = program.prepared_mathematics()?;
            let application = view.application(expression)?;
            let captures = view
                .applications
                .captures
                .span_or_empty(application.captures)
                .iter()
                .map(|row| capture(row.expression, row.source_type).map(Some))
                .collect::<Option<Vec<_>>>()?;
            predicate(view, application.term, &captures, 0)
        }

        fn predicate(
            view: PreparedMathematicalView<'_>,
            term: TermHandle,
            captures: &[Option<Scalar>],
            depth: usize,
        ) -> Option<Boolean> {
            if depth >= 128 {
                return None;
            }
            match view.arena.get(term) {
                Term::Squash { ty } => predicate(view, ty, captures, depth + 1),
                Term::Id { ty, left, right } => {
                    if matches!(view.arena.get(ty), Term::Two) {
                        return match (view.arena.get(left), view.arena.get(right)) {
                            (Term::TwoZero, Term::TwoZero) | (Term::TwoOne, Term::TwoOne) => {
                                Some(Boolean::Constant(true))
                            }
                            (Term::TwoZero, Term::TwoOne) | (Term::TwoOne, Term::TwoZero) => {
                                Some(Boolean::Constant(false))
                            }
                            _ => None,
                        };
                    }
                    let Term::Constant { declaration, .. } = view.arena.get(ty) else {
                        return None;
                    };
                    if !matches!(
                        view.constant(declaration),
                        Some(PreparedConstantMeaning::IntegerCarrier)
                    ) {
                        return None;
                    }
                    comparison(view, BinaryOperator::Equal, left, right, captures)
                }
                Term::Sigma { domain, codomain } => {
                    let mut extended = captures.to_vec();
                    extended.push(None);
                    if matches!(view.arena.get(domain), Term::Two) {
                        let Term::CaseTwo {
                            zero_branch,
                            one_branch,
                            scrutinee,
                            ..
                        } = view.arena.get(codomain)
                        else {
                            return None;
                        };
                        if view.arena.get(scrutinee) != Term::Variable(0) {
                            return None;
                        }
                        Some(Boolean::Or {
                            left: Box::new(predicate(view, zero_branch, &extended, depth + 1)?),
                            right: Box::new(predicate(view, one_branch, &extended, depth + 1)?),
                        })
                    } else {
                        Some(Boolean::And {
                            left: Box::new(predicate(view, domain, captures, depth + 1)?),
                            right: Box::new(predicate(view, codomain, &extended, depth + 1)?),
                        })
                    }
                }
                _ => {
                    let (declaration, arguments) = application_spine(view, term)?;
                    match (view.constant(declaration)?, arguments.as_slice()) {
                        (PreparedConstantMeaning::Negation, [operand]) => Some(Boolean::Not(
                            Box::new(predicate(view, *operand, captures, depth + 1)?),
                        )),
                        (PreparedConstantMeaning::IntegerRelation(operator), [left, right]) => {
                            comparison(view, *operator, *left, *right, captures)
                        }
                        (
                            PreparedConstantMeaning::FloatRelation {
                                operator, format, ..
                            },
                            [left, right],
                        ) => {
                            let primitive = match format {
                                FloatFormat::F32 => PrimitiveType::F32,
                                FloatFormat::F64 => PrimitiveType::F64,
                            };
                            let left = scalar(view, *left, captures, Some(primitive))?;
                            let right = scalar(view, *right, captures, Some(primitive))?;
                            if left.primitive_type() != Some(primitive)
                                || right.primitive_type() != Some(primitive)
                            {
                                return None;
                            }
                            use checked_trees::CheckedIeeeFloatComparisonKind as FloatComparison;
                            let (kind, left, right) = match operator {
                                BinaryOperator::Equal => (FloatComparison::Equal, left, right),
                                BinaryOperator::NotEqual => {
                                    (FloatComparison::NotEqual, left, right)
                                }
                                BinaryOperator::Less => (FloatComparison::LessThan, left, right),
                                BinaryOperator::LessOrEqual => {
                                    (FloatComparison::LessOrEqual, left, right)
                                }
                                BinaryOperator::Greater => (FloatComparison::LessThan, right, left),
                                BinaryOperator::GreaterOrEqual => {
                                    (FloatComparison::LessOrEqual, right, left)
                                }
                                _ => return None,
                            };
                            Some(Boolean::ScalarIeeeFloatComparison {
                                kind,
                                left: Box::new(left),
                                right: Box::new(right),
                            })
                        }
                        _ => None,
                    }
                }
            }
        }

        fn scalar(
            view: PreparedMathematicalView<'_>,
            term: TermHandle,
            captures: &[Option<Scalar>],
            expected: Option<PrimitiveType>,
        ) -> Option<Scalar> {
            if let Term::Variable(index) = view.arena.get(term) {
                let value = captures
                    .get(captures.len().checked_sub(index as usize + 1)?)?
                    .clone()?;
                let primitive = value.primitive_type()?;
                return (primitive.is_fixed_integer()
                    || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
                .then_some(value);
            }
            let (declaration, arguments) = application_spine(view, term)?;
            if !arguments.is_empty() {
                return None;
            }
            if let PreparedConstantMeaning::FloatLiteral { format, bits } =
                view.constant(declaration)?
            {
                let (primitive, value) = match format {
                    FloatFormat::F32 => {
                        (PrimitiveType::F32, IeeeFloatValue::Binary32(*bits as u32))
                    }
                    FloatFormat::F64 => (PrimitiveType::F64, IeeeFloatValue::Binary64(*bits)),
                };
                if expected.is_some_and(|expected| expected != primitive) {
                    return None;
                }
                return Some(Scalar::IeeeFloatLiteral { value });
            }
            let PreparedConstantMeaning::Integer(value) = view.constant(declaration)? else {
                return None;
            };
            Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(value, expected?)?,
            })
        }

        fn comparison(
            view: PreparedMathematicalView<'_>,
            operator: BinaryOperator,
            left: TermHandle,
            right: TermHandle,
            captures: &[Option<Scalar>],
        ) -> Option<Boolean> {
            let left_value = scalar(view, left, captures, None);
            let right_value = scalar(view, right, captures, None);
            let primitive = left_value
                .as_ref()
                .or(right_value.as_ref())?
                .primitive_type()?;
            if !primitive.is_fixed_integer() {
                return None;
            }
            let left = left_value.or_else(|| scalar(view, left, captures, Some(primitive)))?;
            let right = right_value.or_else(|| scalar(view, right, captures, Some(primitive)))?;
            let (kind, left, right, negated) = match operator {
                BinaryOperator::Equal => (Comparison::Equal, left, right, false),
                BinaryOperator::NotEqual => (Comparison::Equal, left, right, true),
                BinaryOperator::Less => (Comparison::LessThan, left, right, false),
                BinaryOperator::LessOrEqual => (Comparison::LessOrEqual, left, right, false),
                BinaryOperator::Greater => (Comparison::LessThan, right, left, false),
                BinaryOperator::GreaterOrEqual => (Comparison::LessOrEqual, right, left, false),
                _ => return None,
            };
            let result = Boolean::IntegerComparison {
                kind,
                left: Box::new(left),
                right: Box::new(right),
            };
            Some(if negated {
                Boolean::Not(Box::new(result))
            } else {
                result
            })
        }
    }
    pub use float_predicates::{MathematicalFloatPredicate, float_predicates};
    pub(crate) use prepared::application_spine;
    pub use prepared::{
        PreparedApplication, PreparedCapture, PreparedConstant, PreparedConstantMeaning,
        PreparedIdentityValue, PreparedIndexedTheorem, PreparedMathematicalApplications,
        PreparedMathematicalView, PreparedSourceBinding, PreparedSourceBindingKind,
        PreparedSourceCarrier, PreparedSourceConstructor, PreparedSourceEvidence,
        integer_embedding_meaning,
    };
    pub use scalar_predicates::scalar_predicate;

    use symbols::SymbolHandle;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
    };
    use typed_trees::mathematical::{MathematicalDefinition, MathematicalType};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    /// Mathematical declarations own dependent formation and have no runtime call.
    pub fn is_mathematical_call(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> bool {
        is_mathematical_declaration(program, call.target_symbol)
    }

    /// These declarations have mathematical formation rules, not machine contracts.
    pub fn is_mathematical_declaration(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> bool {
        identity::is_identity_declaration(program, symbol)
            || selected_definition_by_symbol(program, symbol).is_some()
    }

    pub fn selected_definition<'program>(
        program: &'program dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<&'program MathematicalDefinition> {
        selected_definition_by_symbol(program, call.target_symbol)
    }

    fn selected_definition_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&MathematicalDefinition> {
        symbol.is_valid().then_some(())?;
        program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    pub fn parameter_type_reference(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        symbol.is_valid().then_some(())?;
        program
            .mathematical_definitions()
            .iter()
            .find_map(|definition| {
                let parameter = program
                    .mathematical_parameters(definition.parameters)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)?;
                match program.mathematical_type(parameter.ty) {
                    MathematicalType::Ordinary(reference) => Some(*reference),
                    _ => None,
                }
            })
    }

    pub fn operand_type_reference(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let owner = crate::reserved_result_place(program, expression)
            .map(|place| place.machine_symbol)
            .or_else(|| match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) if path.symbol.is_valid() => {
                    Some(program.symbols.get(path.symbol).parent)
                }
                _ => None,
            })
            .unwrap_or_else(|| program.symbols.root());
        operand_type_reference_in_declaration(program, owner, expression)
    }

    /// The number or Boolean read from a borrow has its own value type. The
    /// source capture still retains the borrow and must justify that exact read.
    pub fn scalar_observation_type_reference(
        program: &dyn crate::CheckingInput,
        declared: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let observed = crate::expression_types_and_fields::expression_types::scalar_observations::readable_observation_type_reference(program, declared)?;
        program.primitive_type_reference(observed)?;
        Some(observed)
    }

    /// A proof statement can name a borrowed subject of its exact receiving
    /// type. This forms the statement without copying or materializing the value.
    pub fn subject_observation_type_reference(
        program: &dyn crate::CheckingInput,
        declared: TypeReferenceHandle,
        expected: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let observed = crate::expression_types_and_fields::expression_types::scalar_observations::readable_observation_type_reference(program, declared)?;
        (program.type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
            observed,
        )) == program.type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
            expected,
        )))
        .then_some(observed)
    }

    /// Read an operand under its retained declaration telescope. Sibling trait
    /// operations and applied signatures can return types that belong to that
    /// exact owner; a context-free call query cannot export those local binders.
    pub fn operand_type_reference_in_declaration(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        crate::value_custody::expression_types::declaration_expression_result_type_reference(
            program,
            owner,
            None,
            &|expression| match program.expression_table.expression(expression) {
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                    crate::proof_contracts::proof_embeddings::expression_type_reference(
                        program, expression,
                    )
                }
                _ => None,
            },
            expression,
        )
    }

    pub fn result_type_reference(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<TypeReferenceHandle> {
        let definition = selected_definition(program, call)?;
        (call.arguments.len() == definition.parameters.len()).then_some(())?;
        match program.mathematical_type(definition.result) {
            MathematicalType::Ordinary(reference) => {
                applied_result_reference(program, definition, call, *reference)
            }
            _ => None,
        }
    }

    /// Complete ordinary receiving types of a mathematical call's value slots.
    /// These are formation inputs, not evidence that an actual inhabits its type.
    pub fn argument_type_references(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<Vec<Option<TypeReferenceHandle>>> {
        if is_identity_call(program, call) {
            let application = identity_application(program, call).ok()?;
            return Some(vec![Some(application.carrier); 2]);
        }
        let definition = selected_definition(program, call)?;
        let parameters = program.mathematical_parameters(definition.parameters);
        (parameters.len() == call.arguments.len()).then_some(())?;
        Some(
            parameters
                .iter()
                .map(|parameter| match program.mathematical_type(parameter.ty) {
                    MathematicalType::Ordinary(reference) => {
                        applied_result_reference(program, definition, call, *reference)
                    }
                    _ => None,
                })
                .collect(),
        )
    }

    /// Read the selected source telescope, independently of the caller's value
    /// operands. The mathematical application checker owns argument validity and
    /// universe inference; this projection only finds an existing ordinary result
    /// reference. Inserted actual references keep their supplying scope.
    fn applied_result_reference(
        program: &dyn crate::CheckingInput,
        definition: &MathematicalDefinition,
        call: &TableCallExpression,
        reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        program
            .type_reference_table
            .contains_type_reference(reference)
            .then_some(())?;
        let binders = program.data_type_parameters.span(definition.binders)?;
        let vocabulary = FixedCoreVocabulary::resolve(&program.symbols);
        let plan =
            source_generic_argument_plan(program, binders, &call.machine_arguments, &vocabulary)?;
        let dependencies =
            typed_trees::references::collect_type_symbol_dependencies(program, reference);
        let mut substitutions = Vec::new();
        for slot in plan {
            match slot.kind {
                MathematicalBinderKind::Level => continue,
                MathematicalBinderKind::Term => {
                    if !dependencies.contains(&slot.binder.symbol) {
                        continue;
                    }
                    let argument = slot.argument?;
                    let actual = value_index_reference(program, argument)?;
                    substitutions.push((slot.binder.symbol, actual));
                    continue;
                }
                MathematicalBinderKind::Type => {}
            }
            if !dependencies.contains(&slot.binder.symbol) {
                continue;
            }
            let argument = slot.argument?;
            if argument.application.is_some()
                || argument.evidence_projection.is_some()
                || argument.const_literal.is_some()
            {
                return None;
            }
            let actual = if program
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            {
                argument.type_reference
            } else {
                argument.symbol.is_valid().then_some(())?;
                program
                    .type_reference_table
                    .find_named_type_reference(argument.symbol)?
            };
            substitutions.push((slot.binder.symbol, actual));
        }
        let projected = typed_trees::static_applications::project_type_operand(
            program,
            reference,
            &substitutions,
        );
        if projected != reference || substitutions.is_empty() {
            return Some(projected);
        }
        let identity = program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: &substitutions,
            ..TypeIdentityRequest::ordinary(reference)
        });
        identity.is_valid().then_some(())?;
        if program.normalized_type_identity(reference) == identity {
            return Some(reference);
        }
        program
            .type_reference_table
            .type_reference_rows()
            .find_map(|(candidate, _)| {
                (program.normalized_type_identity(candidate) == identity).then_some(candidate)
            })
    }

    fn value_index_reference(
        program: &dyn crate::CheckingInput,
        argument: &StaticMachineArgument,
    ) -> Option<TypeReferenceHandle> {
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return None;
        }
        if program
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        {
            return Some(argument.type_reference);
        }
        if argument.symbol.is_valid() {
            return program
                .type_reference_table
                .find_named_type_reference(argument.symbol);
        }
        let literal = argument.const_literal.as_ref()?;
        program
            .type_reference_table
            .type_reference_rows()
            .find_map(|(handle, node)| {
                let TypeReferenceNode::ConstExpression(expression) = node else {
                    return None;
                };
                matches!(
                    program.expression_table.expression(*expression),
                    ExpressionNode::Integer(value) if value == literal
                )
                .then_some(handle)
            })
    }

    pub fn ordinary_parameters(
        program: &dyn crate::CheckingInput,
        definition: &MathematicalDefinition,
    ) -> Vec<typed_trees::signature::StateParameter> {
        program
            .mathematical_parameters(definition.parameters)
            .iter()
            .filter_map(|parameter| {
                let MathematicalType::Ordinary(reference) = program.mathematical_type(parameter.ty)
                else {
                    return None;
                };
                Some(typed_trees::signature::StateParameter {
                    symbol: parameter.symbol,
                    name: parameter.name.clone(),
                    type_reference: *reference,
                    ..Default::default()
                })
            })
            .collect()
    }

    pub fn source_operations_have_same_meaning(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let parameters = if let Some(definition) = program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == owner)
        {
            ordinary_parameters(program, definition)
        } else if let Some(machine) = program.machine_by_symbol(owner) {
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            program.state_parameters(entry).to_vec()
        } else {
            return false;
        };
        crate::proof_contracts::contract_entailment::source_operations_have_same_meaning(
            program,
            owner,
            &parameters,
            left,
            right,
        )
    }

    pub fn is_source_evidence_carrier(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        program.prepared_mathematics().is_some_and(|view| {
            view.source_carrier(reference).is_some_and(|carrier| {
                carrier.type_identity.is_valid()
                    && carrier.type_identity == program.normalized_type_identity(reference)
            })
        })
    }
}
