/*
Expression result types, reference correspondence, and match dispatch
(`expression_types`) alongside record and array field domain checks
(`field_domains`).
*/
pub mod expression_types {

    use crate::frozen_program::expression_table;
    use checked_trees::CheckedByteSequenceCarrier;
    use diagnostics::Diagnostic;
    use std::fmt;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod cast_validation {
        use super::expression_type_name_handle;
        use super::shape_validation::value_shape_is_array;
        use super::value_classification::{ValueClass, value_class, value_concrete_data_name};
        use diagnostics::Diagnostic;
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub(crate) fn validate_cast_types(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            occurrence: ExpressionHandle,
            cast: &TableCastExpression,
            environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if cast.form.is_recast() {
                return;
            }
            if let Some(state) = state {
                crate::value_custody::cleanup::validate_cleanup_delivery(
                    program,
                    machine,
                    state,
                    cast.value,
                    cast.target_type,
                    "explicit qualification cast",
                    diagnostics,
                );
                super::validate_exact_float_coercion(
                    program,
                    machine,
                    state,
                    occurrence,
                    cast.value,
                    cast.target_type,
                    environment,
                    diagnostics,
                );
            }
            crate::value_custody::type_references::validate_indexed_qualification_arguments(
                program,
                machine,
                cast,
                diagnostics,
            );
            let target_name = program.named_type_reference(cast.target_type);
            if crate::proof_contracts::contract_entailment::proof_nat_cast(program, cast) {
                let hypotheses: Vec<_> = program
                    .contract_expressions(
                        program.machine_contracts(machine).iter().chain(
                            state
                                .into_iter()
                                .flat_map(|state| program.state_contracts(state)),
                        ),
                        typed_trees::signature::SignatureContractKind::Requires,
                    )
                    .collect();
                if !crate::proof_contracts::contract_entailment::proof_integer_nonnegative(
                    program,
                    state.map_or(machine.symbol, |state| state.symbol),
                    cast.value,
                    &hypotheses,
                    &[],
                ) {
                    diagnostics.push(Diagnostic::error(
                "Exact proof Int-to-Nat conversion requires a previously proven nonnegative value",
            ));
                }
                return;
            }
            if target_name.is_some_and(|target| PrimitiveType::from_name(target.as_str()).is_none())
                && (match state {
                    Some(state) => {
                        super::expression_result_type_reference(program, machine, state, cast.value)
                    }
                    None => super::declaration_expression_result_type_reference(
                        program,
                        machine.symbol,
                        None,
                        &|expression| {
                            crate::value_custody::places::declared_place_type(
                                program, machine, state, expression,
                            )
                        },
                        cast.value,
                    ),
                })
                .is_some_and(|actual| {
                    crate::stored_structural_type_identity(program, actual, &[], &[])
                        == crate::stored_structural_type_identity(
                            program,
                            cast.target_type,
                            &[],
                            &[],
                        )
                })
            {
                return;
            }
            if let Some(target_name) = target_name
                && let Some(quotient_data) = program.data_definitions().iter().find(|definition| {
                    definition.name.as_str() == target_name.as_str()
                        && definition.quotient.is_some()
                })
            {
                validate_quotient_mint(program, machine, state, cast, quotient_data, diagnostics);
                return;
            }
            if let Some(target) = target_name
                && PrimitiveType::from_name(target.as_str()).is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` casts with `as {target}`, but `{target}` is not a scalar \
             type; non-scalar `as` only erases qualifications from the same data carrier",
                    machine.name.as_str(),
                    state.map(|state| state.name.as_str()).unwrap_or(""),
                )));
            }
            match target_name.and_then(|target| PrimitiveType::from_name(target.as_str())) {
                Some(PrimitiveType::Bool) => {
                    report_number_to_bool_cast(program, machine, state, cast.value, diagnostics);
                }
                Some(primitive) => {
                    report_invalid_numeric_cast_source(
                        program,
                        machine,
                        state,
                        cast.value,
                        primitive.name(),
                        diagnostics,
                    );
                }
                _ => {}
            }
        }

        fn validate_quotient_mint(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            cast: &TableCastExpression,
            quotient_data: &typed_trees::data::DataDefinition,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let quotient = quotient_data
                .quotient
                .as_ref()
                .expect("quotient target was selected by quotient metadata");
            let expected = base_data_symbol(program, quotient.carrier);
            let actual = expression_data_symbol(program, machine, state, cast.value);
            if expected.is_none() || actual != expected {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` constructs quotient `{}` from `{}`, but quotient construction is carrier-only: expected `{}`",
            machine.name,
            state.map(|state| state.name.as_str()).unwrap_or(""),
            quotient_data.name,
            expression_type_name_handle(program, cast.value),
            program.display_type_reference_with_constraints(quotient.carrier),
        )));
            }
            if cast.semantic_domain.count() != 0 || cast.domain != ArithmeticDomain::Exact {
                diagnostics.push(Diagnostic::error(format!(
            "quotient construction `as {}` cannot carry an arithmetic or semantic-domain policy suffix",
            quotient_data.name,
        )));
            }
        }

        fn base_data_symbol(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<symbols::SymbolHandle> {
            if !type_reference.is_valid() {
                return None;
            }
            program.type_reference_table.type_head_symbol(
                program
                    .type_reference_table
                    .unconstrained_type_reference(type_reference),
            )
        }

        fn expression_data_symbol(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            expression: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            if let Some(type_reference) = crate::value_custody::places::declared_place_type(
                program, machine, state, expression,
            )
            .or_else(|| match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    crate::proof_contracts::arithmetic_domains::call_return_type(program, call)
                }
                _ => None,
            }) {
                return base_data_symbol(program, type_reference);
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => literal
                    .type_symbol
                    .is_valid()
                    .then_some(literal.type_symbol),
                ExpressionNode::Borrow(inner) => {
                    expression_data_symbol(program, machine, state, inner.target)
                }
                ExpressionNode::Atomic(atomic) => {
                    expression_data_symbol(program, machine, state, atomic.value)
                }
                ExpressionNode::Cast(cast) => base_data_symbol(program, cast.target_type),
                _ => None,
            }
        }

        fn report_number_to_bool_cast(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let class = value_class(program, Some(machine), state, value);
            if !matches!(class, Some(ValueClass::Numeric) | Some(ValueClass::Text)) {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` casts {} to `bool`, but `as` has no number-to-bool conversion \
         (a `bool` is {{0, 1}}; write an explicit comparison like `n != 0`)",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
                class.unwrap().describe(),
            )));
            true
        }

        fn report_invalid_numeric_cast_source(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            source: ExpressionHandle,
            target_name: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let mut reject = |reason: String| {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` {reason} to `{target_name}`, but `as` converts between \
             scalar types only",
                    machine.name.as_str(),
                    state.map(|state| state.name.as_str()).unwrap_or(""),
                )));
                true
            };
            if value_class(program, Some(machine), state, source) == Some(ValueClass::Text) {
                return reject("casts text".to_owned());
            }
            if value_shape_is_array(program, machine, state, source) == Some(true) {
                return reject("casts an array value".to_owned());
            }
            if let Some(name) = value_concrete_data_name(program, machine, state, source) {
                return reject(format!("casts a `{name}` value"));
            }
            false
        }
    }
    pub(crate) mod float_coercions {
        use diagnostics::Diagnostic;
        use numerics::bignum::BigRational;
        use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_exact_float_coercion(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            occurrence: typed_trees::expression::ExpressionHandle,
            operand: typed_trees::expression::ExpressionHandle,
            target: TypeReferenceHandle,
            environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let source = super::expression_result_type_reference(program, machine, state, operand)
                .and_then(|reference| program.primitive_type_reference(reference));
            let Some(source) = source else {
                return true;
            };
            let Some(target) = program.primitive_type_reference(target) else {
                return true;
            };
            let proven = float_coercion_preserves_denotation(
                source,
                target,
                || {
                    crate::proof_contracts::arithmetic_domains::float_source_bounds(
                        program,
                        machine.symbol,
                        environment,
                        operand,
                        &|value| {
                            Some(state).and_then(|state| {
                crate::value_custody::expression_types::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    value,
                )
            })
            .or_else(|| {
                crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    value,
                )
            })
                        },
                    )
                },
                || {
                    crate::proof_contracts::arithmetic_domains::integer_source_bounds(
                        program,
                        machine,
                        state,
                        operand,
                        source,
                        environment,
                        diagnostics,
                    )
                },
            );
            let Some(proven) = proven else {
                return true;
            };
            let source_reference =
                super::expression_result_type_reference(program, machine, state, operand);
            let proven = proven
        || (matches!(source, PrimitiveType::F32 | PrimitiveType::F64)
            && source_reference.is_some_and(|reference| {
                crate::proof_contracts::contract_entailment::exact_float_integer_cast_fits_at_occurrence(
                    program, machine, state, occurrence, reference, target,
                )
            }))
        || (source.is_fixed_integer()
            && matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
            && source_reference.is_some_and(|reference| {
                crate::proof_contracts::contract_entailment::exact_integer_float_cast_fits_at_occurrence(
                    program, machine, state, occurrence, reference, target,
                )
            }));
            if !proven {
                let message = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                    format!(
                        "machine `{}` state `{}` cannot prove Exact float-to-`{}` cast operand `{}` \
                 preserves its denotation; prove an integral representable value for an integer \
                 target, or exact format representability; use a named conversion to round or truncate",
                        machine.name,
                        state.name,
                        target.name(),
                        program.expression_table.display_name(operand),
                    )
                } else {
                    format!(
                        "machine `{}` state `{}` cannot prove integer-to-`{}` cast operand `{}` \
                 preserves its denotation; prove exact format representability or use a named conversion to round",
                        machine.name,
                        state.name,
                        target.name(),
                        program.expression_table.display_name(operand),
                    )
                };
                diagnostics.push(
                    Diagnostic::error(message)
                        .with_source_span(program.expression_table.source_span(operand)),
                );
            }
            proven
        }

        pub(crate) fn float_coercion_preserves_denotation(
            source: PrimitiveType,
            target: PrimitiveType,
            float_bounds: impl FnOnce() -> Option<
                crate::proof_contracts::arithmetic_domains::FloatSourceBounds,
            >,
            integer_bounds: impl FnOnce() -> Option<(i128, i128)>,
        ) -> Option<bool> {
            if numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())
                .is_some()
                && matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
            {
                let format = if target == PrimitiveType::F32 {
                    FloatFormat::BINARY32
                } else {
                    FloatFormat::BINARY64
                };
                return Some(integer_bounds().is_some_and(|(minimum, maximum)| {
                    let exact_limit = 1i128 << format.precision;
                    if minimum >= -exact_limit && maximum <= exact_limit {
                        true
                    } else if minimum == maximum {
                        let integer = numerics::bignum::BigInt::from_i128(minimum);
                        FloatSemantics::from_integer(format, &integer)
                            == FloatMeaning::FiniteNonZero(BigRational::from_integer(integer))
                    } else {
                        false
                    }
                }));
            }
            if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                return None;
            }
            if source == target || (source == PrimitiveType::F32 && target == PrimitiveType::F64) {
                return Some(true);
            }
            if !target.accepts_integer_literal() && target != PrimitiveType::F32 {
                return None;
            }
            Some(float_bounds().is_some_and(|bounds| {
                if target.accepts_integer_literal() {
                    bounds.is_exact_integer(target)
                } else if bounds.minimum == bounds.maximum {
                    let meaning = FloatMeaning::from_f64(bounds.minimum);
                    FloatSemantics::convert(FloatFormat::BINARY32, &meaning) == meaning
                } else {
                    false
                }
            }))
        }
    }
    pub(crate) mod float_operations {
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub(crate) fn validate_binary_float_formats(
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
            expression: ExpressionHandle,
            operands: &[Option<TypeReferenceHandle>; 2],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) {
                return;
            }
            let [Some(left), Some(right)] = *operands else {
                return;
            };
            let (Some(left), Some(right)) = (
                program.primitive_type_reference(left),
                program.primitive_type_reference(right),
            ) else {
                return;
            };
            if !matches!(
                (left, right),
                (PrimitiveType::F32, PrimitiveType::F64) | (PrimitiveType::F64, PrimitiveType::F32)
            ) {
                return;
            }
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                return;
            };
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program, owner, expression, spelling, operands,
            ) {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
        "builtin `{}` cannot combine `{}` and `{}`; a landed float retains its format, so changing format requires an explicit conversion",
        spelling.symbol(), left.name(), right.name(),
            )).with_source_span(program.expression_table.source_span(expression)));
        }

        pub(crate) fn validate_float_expression_formats(
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
            expression: ExpressionHandle,
            result_type: &impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            crate::queries::expression_traversal::visit_pre_order(
                program,
                vec![expression],
                |handle, node| {
                    if let typed_trees::expression::ExpressionNode::Binary(binary) = node {
                        let operands = [binary.left, binary.right].map(result_type);
                        validate_binary_float_formats(
                            program,
                            owner,
                            handle,
                            &operands,
                            diagnostics,
                        );
                    }
                    false
                },
            );
        }
    }
    pub(crate) mod match_dispatch {
        use super::expression_result_type_reference as declared_value_type;
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
        };
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::PrimitiveType;

        pub(crate) mod structural_patterns {

            use crate::frozen_program::expression_table;
            use diagnostics::Diagnostic;
            use symbols::{SymbolHandle, SymbolKind};
            use typed_trees::data::DataMember;
            use typed_trees::data::DataVariant;
            use typed_trees::expression::{
                ExpressionNode, MatchFieldAction, MatchPattern, TableMatchExpression,
            };
            use typed_trees::machine::Machine;
            use typed_trees::state::State;
            use typed_trees::types::TypeReferenceHandle;

            pub fn structural_match_subject_type(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                dispatch: &TableMatchExpression,
            ) -> Option<TypeReferenceHandle> {
                structural_match_subject_type_for_subject(program, machine, state, dispatch.subject)
            }

            pub(crate) fn structural_match_subject_type_for_subject(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                subject: typed_trees::expression::ExpressionHandle,
            ) -> Option<TypeReferenceHandle> {
                let reference = super::declared_value_type(program, machine, state, subject)?;
                crate::value_custody::places::data_definition_for_type(program, reference)?;
                Some(reference)
            }

            pub fn structural_match_pattern_case(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                dispatch: &TableMatchExpression,
                pattern: MatchPattern,
            ) -> Option<SymbolHandle> {
                structural_match_pattern_case_for_subject(
                    program,
                    machine,
                    state,
                    dispatch.subject,
                    pattern,
                )
            }

            pub(crate) fn structural_match_pattern_case_for_subject(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                subject: typed_trees::expression::ExpressionHandle,
                pattern: MatchPattern,
            ) -> Option<SymbolHandle> {
                program.pattern_component(subject)?;
                let reference =
                    structural_match_subject_type_for_subject(program, machine, state, subject)?;
                let data =
                    crate::value_custody::places::data_definition_for_type(program, reference)?;
                let head = match pattern {
                    MatchPattern::Value(value) => value,
                    MatchPattern::Destructure(pattern) => pattern.head,
                    MatchPattern::Wildcard => return Some(SymbolHandle::invalid()),
                };
                if let Some(name) = expression_table(program).name_path_node(head) {
                    if name.symbol.is_valid()
                        && program.symbols.get(name.symbol).kind == SymbolKind::Variant
                    {
                        let owner = crate::exact_case_reference_owner(program, head)?;
                        return program
                            .data_constructor_case(
                                program
                                    .type_reference_table
                                    .peeled_type_reference(reference),
                                owner.symbol,
                                name.symbol,
                            )
                            .map(|variant| variant.symbol);
                    }
                    if matches!(pattern, MatchPattern::Destructure(_))
                        && program
                            .data_constructor_definition(
                                program
                                    .type_reference_table
                                    .peeled_type_reference(reference),
                                name.symbol,
                            )
                            .is_some_and(|selected| selected.symbol == data.symbol)
                        && program.pattern_component(subject)
                            == Some(language_core::PatternComponent::Fields)
                    {
                        return Some(SymbolHandle::invalid());
                    }
                    if matches!(pattern, MatchPattern::Destructure(_))
                        && let Some(domain) = program.domain_definition(name.symbol)
                        && let Some(subject) =
                            crate::value_custody::places::unwrapped_type_reference(
                                program, reference,
                            )
                        && let Some(target) = crate::value_custody::places::unwrapped_type_reference(
                            program,
                            domain.target_type,
                        )
                        && crate::value_custody::type_references::type_references_match(
                            program, subject, target,
                        )
                    {
                        return Some(SymbolHandle::invalid());
                    }
                }
                if let ExpressionNode::StructLiteral(literal) =
                    expression_table(program).expression(head)
                    && expression_table(program)
                        .struct_fields(literal.fields)
                        .is_empty()
                    && let Some(case) = literal.case_symbol
                    && let Some(selected) = program.data_constructor_case(
                        program
                            .type_reference_table
                            .peeled_type_reference(reference),
                        literal.type_symbol,
                        case,
                    )
                    && selected.payload.is_empty()
                {
                    return Some(selected.symbol);
                }

                None
            }

            pub fn structural_match_pattern_is_total(
                program: &dyn crate::CheckingInput,
                pattern: MatchPattern,
            ) -> bool {
                match pattern {
                    MatchPattern::Value(_) => true,
                    MatchPattern::Wildcard => false,
                    MatchPattern::Destructure(pattern) => {
                        !pattern.guard.is_valid()
                            && !expression_table(program)
                                .name_path_node(pattern.head)
                                .is_some_and(|head| {
                                    program.domain_definition(head.symbol).is_some()
                                })
                            && !expression_table(program)
                                .match_fields(pattern.fields)
                                .iter()
                                .any(|field| matches!(field.action, MatchFieldAction::Equal(_)))
                    }
                }
            }

            pub(crate) fn validate(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                dispatch: &TableMatchExpression,
                pattern: MatchPattern,
                source_span: source::SourceSpan,
                diagnostics: &mut Vec<Diagnostic>,
            ) -> Option<(SymbolHandle, bool)> {
                validate_for_subject(
                    program,
                    machine,
                    state,
                    dispatch.subject,
                    pattern,
                    source_span,
                    diagnostics,
                )
            }

            fn quotient_subject<'program>(
                program: &'program dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                subject: typed_trees::expression::ExpressionHandle,
            ) -> Option<&'program typed_trees::data::DataDefinition> {
                structural_match_subject_type_for_subject(program, machine, state, subject)
                    .and_then(|reference| {
                        crate::value_custody::places::data_definition_for_type(program, reference)
                    })
                    .filter(|data| data.quotient.is_some())
            }

            pub(crate) fn validate_for_subject(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                subject: typed_trees::expression::ExpressionHandle,
                pattern: MatchPattern,
                source_span: source::SourceSpan,
                diagnostics: &mut Vec<Diagnostic>,
            ) -> Option<(SymbolHandle, bool)> {
                if !matches!(pattern, MatchPattern::Wildcard)
                    && let Some(data) = quotient_subject(program, machine, state, subject)
                {
                    diagnostics.push(
            Diagnostic::error(format!(
                "destructure arm in machine `{}` state `{}` cannot pattern-match quotient `{}`: retained representatives are opaque and pattern matching requires a named lifted operation",
                machine.name, state.name, data.name,
            ))
            .with_source_span(source_span),
                    );
                    return None;
                }
                let Some(case) = structural_match_pattern_case_for_subject(
                    program, machine, state, subject, pattern,
                ) else {
                    let head = match pattern {
                        MatchPattern::Value(head) => head,
                        MatchPattern::Destructure(pattern) => pattern.head,
                        MatchPattern::Wildcard => Default::default(),
                    };
                    let selected = expression_table(program).name_path_node(head).map(|path| {
                        (
                            expression_table(program)
                                .name_path_members(path.members)
                                .iter()
                                .map(|name| name.as_str())
                                .collect::<Vec<_>>()
                                .join("::"),
                            path.symbol,
                        )
                    });
                    let actual = super::declared_value_type(program, machine, state, subject)
                        .map(|reference| program.type_reference_table.type_reference(reference));
                    diagnostics.push(
            Diagnostic::error(format!(
                "match pattern does not select a record or case of its subject's exact carrier in `{}::{}`: selected {selected:?}, subject {subject:?} {:?}, type {actual:?}; parameters {:?}; locals {:?}",
                machine.name, state.name,
                expression_table(program).expression(subject),
                program.state_parameters(state).iter().map(|parameter| (&parameter.name, parameter.symbol, parameter.type_reference)).collect::<Vec<_>>(),
                program.statement_table.statements(state.statement_nodes).iter().filter_map(|statement| statement.as_local_data()).map(|local| (&local.name, local.symbol, local.type_reference, local.initial_value)).collect::<Vec<_>>(),
            ))
            .with_source_span(source_span),
                    );
                    return None;
                };
                let MatchPattern::Destructure(pattern) = pattern else {
                    return Some((case, true));
                };
                let reference =
                    structural_match_subject_type_for_subject(program, machine, state, subject)?;
                if pattern.application_type.is_valid() {
                    let matches =
                        crate::value_custody::places::unwrapped_type_reference(program, reference)
                            .zip(crate::value_custody::places::unwrapped_type_reference(
                                program,
                                pattern.application_type,
                            ))
                            .is_some_and(|(subject, application)| {
                                crate::value_custody::type_references::type_references_match(
                                    program,
                                    subject,
                                    application,
                                )
                            });
                    if !matches {
                        diagnostics.push(
                            Diagnostic::error(
                                "pattern type application differs from its subject's exact carrier",
                            )
                            .with_source_span(source_span),
                        );
                        return None;
                    }
                }
                let data =
                    crate::value_custody::places::data_definition_for_type(program, reference)?;
                let declared = if case.is_valid() {
                    let variant = program
                        .data_members(data)
                        .iter()
                        .filter_map(DataMember::as_variant)
                        .find(|variant| variant.symbol == case)?;
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .collect::<Vec<_>>()
                } else {
                    program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                };
                let has_cases = program
                    .data_members(data)
                    .iter()
                    .any(|member| member.as_variant().is_some());
                let fields = expression_table(program).match_fields(pattern.fields);
                let total =
                    structural_match_pattern_is_total(program, MatchPattern::Destructure(pattern));
                let selected_field_symbol = |field_symbol: SymbolHandle| {
                    if declared
                        .iter()
                        .any(|declared| declared.symbol == field_symbol)
                    {
                        return Some(field_symbol);
                    }
                    program
                        .data_constructor_field(
                            program
                                .type_reference_table
                                .peeled_type_reference(reference),
                            data.symbol,
                            field_symbol,
                        )
                        .map(|selected| selected.symbol)
                };
                for (ordinal, field) in fields.iter().enumerate() {
                    if field.component == language_core::PatternComponent::Case {
                        if case.is_valid()
                            || !has_cases
                            || program.pattern_component(subject)
                                != Some(language_core::PatternComponent::Fields)
                            || fields[..ordinal].iter().any(|prior| {
                                prior.component == language_core::PatternComponent::Case
                            })
                            || matches!(field.action, MatchFieldAction::Equal(_))
                        {
                            diagnostics.push(Diagnostic::error("case component is missing, duplicated, or selected outside its outer data pattern").with_source_span(source_span));
                        }
                        continue;
                    }
                    let selected = selected_field_symbol(field.field_symbol);
                    let exact = selected.and_then(|selected| {
                        declared.iter().find(|declared| {
                            declared.symbol == selected
                                && declared.name.as_str() == field.member.as_str()
                        })
                    });
                    if exact.is_none()
                        || fields[..ordinal]
                            .iter()
                            .filter(|prior| {
                                prior.component == language_core::PatternComponent::Fields
                            })
                            .any(|prior| {
                                selected_field_symbol(prior.field_symbol)
                                    .zip(selected)
                                    .is_some_and(|(prior, selected)| prior == selected)
                            })
                    {
                        diagnostics.push(
                Diagnostic::error(format!(
                    "match field `{}` is missing, duplicated, or belongs to another record/case",
                    field.member
                ))
                .with_source_span(source_span),
            );
                    }
                    if let MatchFieldAction::Equal(value) = field.action
                        && let Some(declared) = exact
                    {
                        let carrier = crate::value_custody::places::assignment_value_type(
                            program,
                            declared.type_reference,
                        );
                        if !super::super::argument_matches_carrier(
                            program,
                            machine.symbol,
                            value,
                            carrier,
                            &[],
                        ) {
                            diagnostics.push(Diagnostic::error(format!("match equality operand for field `{}` is incompatible with the field's declared carrier", field.member)).with_source_span(source_span));
                        }
                        crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    carrier,
                    value,
                    "match field equality",
                    diagnostics,
                );
                    }
                }
                if !pattern.has_rest
                    && (declared.iter().any(|declared| {
                        !fields.iter().any(|field| {
                            field.component == language_core::PatternComponent::Fields
                                && selected_field_symbol(field.field_symbol)
                                    == Some(declared.symbol)
                        })
                    }) || (!case.is_valid()
                        && has_cases
                        && !fields
                            .iter()
                            .any(|field| field.component == language_core::PatternComponent::Case)))
                {
                    diagnostics.push(
            Diagnostic::error(
                "match structural pattern must mention every declared field or explicitly use `..`",
            )
            .with_source_span(source_span),
                    );
                }
                if pattern.guard.is_valid() {
                    let guard_type =
                        super::declared_value_type(program, machine, state, pattern.guard)
                            .and_then(|reference| {
                                crate::value_custody::places::unwrapped_type_reference(
                                    program, reference,
                                )
                            })
                            .and_then(|reference| program.primitive_type_reference(reference));
                    let boolean = guard_type == Some(typed_trees::types::PrimitiveType::Bool)
                        || super::super::value_classification::value_class(
                            program,
                            Some(machine),
                            Some(state),
                            pattern.guard,
                        ) == Some(super::super::ValueClass::Boolean);
                    if !boolean {
                        diagnostics.push(
                            Diagnostic::error("match structural pattern guard must produce bool")
                                .with_source_span(source_span),
                        );
                    }
                }
                Some((case, total))
            }

            pub fn structural_match_cases_are_covered(
                program: &dyn crate::CheckingInput,
                reference: TypeReferenceHandle,
                expression: typed_trees::expression::ExpressionHandle,
                cases: &[SymbolHandle],
            ) -> bool {
                let Some(data) =
                    crate::value_custody::places::data_definition_for_type(program, reference)
                else {
                    return false;
                };
                if cases.contains(&SymbolHandle::invalid()) {
                    return true;
                }
                let mut variants = program
                    .data_members(data)
                    .iter()
                    .filter_map(DataMember::as_variant)
                    .peekable();
                if variants.peek().is_none() {
                    return cases.contains(&SymbolHandle::invalid());
                }
                variants
                    .filter(|variant| {
                        !variant_is_impossible(program, variant)
                            && program
                                .admitted_match_cases(expression)
                                .is_none_or(|cases| cases.contains(&variant.symbol))
                    })
                    .all(|variant| cases.contains(&variant.symbol))
            }

            fn variant_is_impossible(
                program: &dyn crate::CheckingInput,
                variant: &DataVariant,
            ) -> bool {
                program
                    .proof_facts
                    .span_or_empty(variant.requires_facts)
                    .iter()
                    .any(|fact| match fact.as_expression() {
                        Some(expression) => match expression_table(program).expression(*expression)
                        {
                            ExpressionNode::Integer(literal) => {
                                literal.decimal_value_i128() == Some(0)
                            }
                            ExpressionNode::Boolean(value) => !*value,
                            _ => false,
                        },
                        None => false,
                    })
            }

            pub(crate) fn validate_transition_patterns(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
                state: &State,
                transition: &typed_trees::statement::TableTransition,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                for retained in program
                    .statement_table
                    .transition_patterns(transition.patterns)
                {
                    let is_case = expression_table(program)
                        .name_path_node(retained.pattern.head)
                        .is_some_and(|head| {
                            head.symbol.is_valid()
                                && program.symbols.get(head.symbol).kind == SymbolKind::Variant
                        });
                    if retained.is_structural
                        || is_case
                        || (retained.pattern.head.is_valid()
                            && quotient_subject(program, machine, state, retained.subject)
                                .is_some())
                    {
                        let pattern = if retained.is_structural {
                            MatchPattern::Destructure(retained.pattern)
                        } else {
                            MatchPattern::Value(retained.pattern.head)
                        };
                        validate_for_subject(
                            program,
                            machine,
                            state,
                            retained.subject,
                            pattern,
                            transition.source_span,
                            diagnostics,
                        );
                    }
                }
            }
        }
        pub(crate) use structural_patterns::validate_transition_patterns;
        pub use structural_patterns::{
            structural_match_cases_are_covered, structural_match_pattern_case,
            structural_match_pattern_is_total, structural_match_subject_type,
        };

        pub fn match_subject_primitive_type(
            program: &dyn crate::CheckingInput,
            dispatch: &TableMatchExpression,
        ) -> Option<PrimitiveType> {
            retained_primitive(program, dispatch.subject).or_else(|| {
                expression_table(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .find_map(|arm| {
                        if let MatchPattern::Value(value) = arm.pattern {
                            retained_primitive(program, value)
                        } else {
                            None
                        }
                    })
            })
        }

        fn retained_primitive(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<PrimitiveType> {
            let reference = match expression_table(program).expression(expression) {
                ExpressionNode::Boolean(_) => return Some(PrimitiveType::Bool),
                ExpressionNode::Integer(_) => {
                    crate::declarations::operators::landed_integer_literal_type_reference(
                        program, expression,
                    )?
                }
                ExpressionNode::Name(path) => super::named_value_type_reference(program, path)?,
                ExpressionNode::Call(call) => crate::machine_calls::calls::call_result_type(
                    program, expression,
                )
                .or_else(|| {
                    typed_trees::operator::resolve_named_expression_call(program, call)
                        .map(|operator| operator.return_type)
                })?,
                ExpressionNode::Cast(cast) => cast.target_type,
                ExpressionNode::Match(dispatch) => {
                    return expression_table(program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .find_map(|arm| retained_primitive(program, arm.value));
                }
                _ => return None,
            };
            crate::value_custody::places::unwrapped_type_reference(program, reference)
                .and_then(|reference| program.primitive_type_reference(reference))
        }

        pub fn validate_match_coverage(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let arms = expression_table(program).match_arms(dispatch.arms);
            let structural = structural_match_subject_type(program, machine, state, dispatch);
            let mut covered_cases = Vec::new();
            let mut wildcard = false;
            let mut boolean_values = [false; 2];
            let subject_type = declared_value_type(program, machine, state, dispatch.subject);
            let subject_class = super::value_classification::value_class(
                program,
                Some(machine),
                Some(state),
                dispatch.subject,
            );
            let subject_boolean = subject_type
                .and_then(|reference| {
                    crate::value_custody::places::unwrapped_type_reference(program, reference)
                })
                .and_then(|reference| program.primitive_type_reference(reference))
                == Some(PrimitiveType::Bool)
                || subject_class == Some(super::ValueClass::Boolean)
                || match_subject_primitive_type(program, dispatch) == Some(PrimitiveType::Bool);
            for arm in arms {
                match arm.pattern {
                    MatchPattern::Wildcard => wildcard = true,
                    MatchPattern::Destructure(_) | MatchPattern::Value(_)
                        if structural.is_some() =>
                    {
                        if let Some((case, total)) = structural_patterns::validate(
                            program,
                            machine,
                            state,
                            dispatch,
                            arm.pattern,
                            arm.source_span,
                            diagnostics,
                        ) && total
                            && !covered_cases.contains(&case)
                        {
                            covered_cases.push(case);
                        }
                    }
                    MatchPattern::Destructure(_) => diagnostics.push(
                        Diagnostic::error(
                            "structural match pattern requires a record or case-bearing subject",
                        )
                        .with_source_span(arm.source_span),
                    ),
                    MatchPattern::Value(pattern) => {
                        if let Some(value) = expression_table(program).boolean_node(pattern) {
                            boolean_values[usize::from(*value)] = true;
                        }
                    }
                }
            }
            if !wildcard
                && !(subject_boolean && boolean_values == [true, true])
                && !structural.is_some_and(|reference| {
                    structural_match_cases_are_covered(
                        program,
                        reference,
                        expression,
                        &covered_cases,
                    )
                })
            {
                diagnostics.push(Diagnostic::error(
            "match does not cover its subject; cover every admitted case, both Boolean alternatives, or add a wildcard arm",
        ).with_source_span(expression_table(program).source_span(expression)));
            }
        }

        pub fn validate_match_dispatch(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            validate_match_coverage(program, machine, state, expression, dispatch, diagnostics);
            let arms = expression_table(program).match_arms(dispatch.arms);
            if structural_match_subject_type(program, machine, state, dispatch).is_none() {
                for arm in arms {
                    let MatchPattern::Value(pattern) = arm.pattern else {
                        continue;
                    };
                    if matches!(
                        expression_table(program).expression(pattern),
                        ExpressionNode::ArrayLiteral(_)
                            | ExpressionNode::StructLiteral(_)
                            | ExpressionNode::Range(_)
                            | ExpressionNode::Borrow(_)
                    ) {
                        diagnostics.push(Diagnostic::error(
                    "match structural, range, and binding patterns are not supported by value dispatch yet",
                ).with_source_span(arm.source_span));
                        continue;
                    }
                    if !is_scalar_value(program, machine, state, dispatch.subject)
                        || !is_scalar_value(program, machine, state, pattern)
                    {
                        diagnostics.push(Diagnostic::error(
                    "match value patterns require scalar subjects and scalar pattern values; structural, domain, and case patterns are not supported yet",
                ).with_source_span(arm.source_span));
                        continue;
                    }
                    check_compatible_values(
                        program,
                        machine,
                        state,
                        dispatch.subject,
                        pattern,
                        "match pattern is incompatible with its subject",
                        diagnostics,
                    );
                }
            }
            if arms.is_empty() {
                return;
            }
            let live_arms = arms
                .iter()
                .filter(|arm| !program.static_type_context_is_unreachable(arm.value))
                .collect::<Vec<_>>();
            let declared_arms = live_arms
                .iter()
                .map(|arm| {
                    declared_value_type(program, machine, state, arm.value)
                        .map(|reference| (arm.value, reference))
                })
                .collect::<Option<Vec<_>>>();
            if declared_arms
                .as_deref()
                .and_then(|values| program.common_static_type_reference(values))
                .is_some()
            {
                return;
            }
            let Some(peer) = live_arms
                .iter()
                .find(|arm| declared_value_type(program, machine, state, arm.value).is_some())
                .copied()
                .or_else(|| live_arms.first().copied())
            else {
                return;
            };
            let peer = peer.value;
            if let Some(destination) = declared_value_type(program, machine, state, peer) {
                for arm in &live_arms {
                    crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
                        program,
                        destination,
                        arm.value,
                        "typed match result",
                        diagnostics,
                    );
                }
            }
            for arm in &live_arms {
                if !program.static_type_contexts_agree(peer, arm.value) {
                    continue;
                }
                check_compatible_values(
                    program,
                    machine,
                    state,
                    peer,
                    arm.value,
                    "match arms produce incompatible values",
                    diagnostics,
                );
            }
        }

        fn is_scalar_value(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> bool {
            if let Some(reference) = declared_value_type(program, machine, state, expression) {
                return crate::value_custody::places::unwrapped_type_reference(program, reference)
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some();
            }
            if let Some(class) = super::value_classification::value_class(
                program,
                Some(machine),
                Some(state),
                expression,
            ) {
                return matches!(
                    class,
                    super::ValueClass::Numeric | super::ValueClass::Boolean
                );
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let arms = expression_table(program).match_arms(dispatch.arms);
                    !arms.is_empty()
                        && arms
                            .iter()
                            .all(|arm| is_scalar_value(program, machine, state, arm.value))
                }
                ExpressionNode::Binary(_) => crate::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ),
                _ => false,
            }
        }

        fn check_compatible_values(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            peer: ExpressionHandle,
            value: ExpressionHandle,
            message: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if let Some(dispatch) = expression_table(program).match_node(value) {
                for arm in expression_table(program).match_arms(dispatch.arms) {
                    check_compatible_values(
                        program,
                        machine,
                        state,
                        peer,
                        arm.value,
                        message,
                        diagnostics,
                    );
                }
                return;
            }
            if let Some(dispatch) = expression_table(program).match_node(peer) {
                for arm in expression_table(program).match_arms(dispatch.arms) {
                    check_compatible_values(
                        program,
                        machine,
                        state,
                        arm.value,
                        value,
                        message,
                        diagnostics,
                    );
                }
                return;
            }
            let data_symbol = |expression| match expression_table(program).expression(expression) {
                ExpressionNode::StructLiteral(literal) if literal.type_symbol.is_valid() => {
                    Some(literal.type_symbol)
                }
                _ => None,
            };
            let peer_data = data_symbol(peer);
            let value_data = data_symbol(value);
            let peer_declared = declared_value_type(program, machine, state, peer);
            let value_declared = declared_value_type(program, machine, state, value);
            let peer_type = peer_declared.map(|reference| {
                crate::value_custody::places::assignment_value_type(program, reference)
            });
            let value_type = value_declared.map(|reference| {
                crate::value_custody::places::assignment_value_type(program, reference)
            });
            let mismatch = if let (Some(peer_type), Some(value_type)) = (peer_type, value_type) {
                program.normalized_type_identity(peer_type)
                    != program.normalized_type_identity(value_type)
                    && !peer_declared
                        .zip(value_declared)
                        .is_some_and(|(peer, value)| {
                            borrowed_arrays_share_element_view(program, peer, value)
                        })
                    && !matches!(
                        (super::result_type::arithmetic_carrier(program, peer_type),
                         super::result_type::arithmetic_carrier(program, value_type)),
                        (Some(peer), Some(value)) if peer == value
                    )
            } else if let Some(reference) = peer_type.or(value_type) {
                let anonymous = if peer_type.is_some() { value } else { peer };
                crate::value_custody::literals::validate_suffix_landing(
                    program,
                    anonymous,
                    reference,
                    diagnostics,
                );
                !super::argument_matches_type_reference_handle(program, anonymous, reference)
            } else {
                let peer_class = super::value_classification::value_class(
                    program,
                    Some(machine),
                    Some(state),
                    peer,
                );
                let value_class = super::value_classification::value_class(
                    program,
                    Some(machine),
                    Some(state),
                    value,
                );
                let class_mismatch = peer_class
                    .zip(value_class)
                    .is_some_and(|(peer, value)| peer != value);
                let data_mismatch = match (peer_data, value_data) {
                    (Some(peer), Some(value)) => peer != value,
                    (Some(_), None) => {
                        value_class.is_some()
                            || matches!(
                                expression_table(program).expression(value),
                                ExpressionNode::ArrayLiteral(_)
                            )
                    }
                    (None, Some(_)) => {
                        peer_class.is_some()
                            || matches!(
                                expression_table(program).expression(peer),
                                ExpressionNode::ArrayLiteral(_)
                            )
                    }
                    (None, None) => false,
                };
                class_mismatch || data_mismatch
            };
            if mismatch {
                diagnostics.push(
                    Diagnostic::error(message)
                        .with_source_span(expression_table(program).source_span(value)),
                );
            }
        }

        fn borrowed_arrays_share_element_view(
            program: &dyn crate::CheckingInput,
            left: typed_trees::types::TypeReferenceHandle,
            right: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            use typed_trees::types::TypeReferenceNode;
            let borrowed_array =
                |reference| match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    } => match program.type_reference_table.type_reference(*referee) {
                        TypeReferenceNode::FixedArray { element_type, .. } => {
                            Some((*access, lifetime, *element_type))
                        }
                        _ => None,
                    },
                    _ => None,
                };
            let (
                Some((left_access, left_lifetime, left_element)),
                Some((right_access, right_lifetime, right_element)),
            ) = (borrowed_array(left), borrowed_array(right))
            else {
                return false;
            };
            left_access == right_access
                && left_lifetime == right_lifetime
                && program.normalized_type_identity(left_element)
                    == program.normalized_type_identity(right_element)
        }
    }
    pub(crate) mod operator_validation {
        use super::value_classification::{ValueClass, concrete_data_type_name, value_class};
        use crate::value_custody::places::collection_operations::type_reference_is_collection;
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_binary_operand_types(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            expression: ExpressionHandle,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            if let Some(state) = state {
                let operands = [left, right].map(|operand| {
                    super::expression_result_type_reference(program, machine, state, operand)
                });
                super::float_operations::validate_binary_float_formats(
                    program,
                    state.symbol,
                    expression,
                    &operands,
                    diagnostics,
                );
                if crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        ) {
            validate_builtin_comparison_operand_landing(
                program,
                operator,
                [(left, operands[0]), (right, operands[1])],
                &format!("machine `{}` state `{}`", machine.name, state.name),
                &mut |peer, target| {
                    crate::proof_contracts::contract_entailment::anonymous_comparison_operand_fits_at_occurrence(
                        program, machine, state, expression, peer, target,
                    ).retain(expression, machine.symbol, "anonymous address comparison operand", formation_obligations)
                },
                diagnostics,
            );
        }
            }
            report_cross_class_binary_operands(program, machine, state, left, right, diagnostics);
            report_invalid_text_operator(
                program,
                machine,
                state,
                operator,
                left,
                right,
                diagnostics,
            );
            report_non_bool_logical_operands(
                program,
                machine,
                state,
                operator,
                left,
                right,
                diagnostics,
            );
            report_array_operator_operands(
                program,
                machine,
                state,
                operator,
                left,
                right,
                diagnostics,
            );
            report_undeclared_struct_operator(
                program,
                machine,
                state,
                operator,
                left,
                right,
                diagnostics,
            );
            report_float_bitwise_operator(
                program,
                machine,
                state,
                operator,
                left,
                right,
                diagnostics,
            );
        }

        pub(crate) fn validate_builtin_comparison_operand_landing(
            program: &dyn crate::CheckingInput,
            operator: typed_trees::expression::BinaryOperator,
            operands: [(ExpressionHandle, Option<TypeReferenceHandle>); 2],
            owner: &str,
            target_fit: &mut dyn FnMut(ExpressionHandle, PrimitiveType) -> bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            use typed_trees::expression::BinaryOperator;
            if !matches!(
                operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) {
                return;
            }
            for ((_, reference), (peer, peer_reference)) in
                [(operands[0], operands[1]), (operands[1], operands[0])]
            {
                if peer_reference.is_some() {
                    continue;
                }
                let Some(primitive) = reference
                    .and_then(|reference| {
                        super::scalar_observations::readable_observation_type_reference(
                            program, reference,
                        )
                    })
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .filter(|primitive| {
                        primitive.is_integer() || *primitive == PrimitiveType::Addr
                    })
                else {
                    continue;
                };
                let before = diagnostics.len();
                crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_primitive_range(
            program,
            primitive,
            peer,
            owner,
            diagnostics,
        );
                if primitive == PrimitiveType::Addr
                    && diagnostics.len() == before
                    && crate::value_custody::literals::has_anonymous_numeric_results(program, peer)
                    && !target_fit(peer, primitive)
                {
                    diagnostics.push(Diagnostic::error(format!(
                "anonymous address comparison operand cannot prove fit in `addr::Bound` in {owner}",
            )).with_source_span(program.expression_table.source_span(peer)));
                }
            }
        }

        pub(crate) fn expression_is_float_typed(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operand: ExpressionHandle,
        ) -> bool {
            match program.expression_table.expression(operand) {
                ExpressionNode::UndeterminedNaN(_) => true,
                ExpressionNode::Float(literal) => literal.landing().is_some(),
                ExpressionNode::Borrow(inner) => {
                    expression_is_float_typed(program, machine, state, inner.target)
                }
                ExpressionNode::Binary(binary) => {
                    expression_is_float_typed(program, machine, state, binary.left)
                        || expression_is_float_typed(program, machine, state, binary.right)
                }
                ExpressionNode::Unary(unary) => {
                    expression_is_float_typed(program, machine, state, unary.operand)
                }
                ExpressionNode::Cast(cast) => program
                    .primitive_type_reference(cast.target_type)
                    .is_some_and(|primitive| primitive.accepts_float_literal()),
                ExpressionNode::Call(call) => {
                    crate::proof_contracts::arithmetic_domains::call_return_type(program, call)
                        .and_then(|return_type| program.primitive_type_reference(return_type))
                        .is_some_and(|primitive| primitive.accepts_float_literal())
                }
                _ => crate::value_custody::places::declared_place_type(
                    program, machine, state, operand,
                )
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some_and(|primitive| {
                    matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                }),
            }
        }

        fn report_float_bitwise_operator(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            use typed_trees::expression::BinaryOperator;
            if !matches!(
                operator,
                BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::Modulo
            ) {
                return false;
            }
            if !expression_is_float_typed(program, machine, state, left)
                && !expression_is_float_typed(program, machine, state, right)
            {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a float operand, but bitwise, shift, \
         and modulo operators are defined for integers only",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
            true
        }

        fn report_cross_class_binary_operands(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let (Some(left_class), Some(right_class)) = (
                value_class(program, Some(machine), state, left),
                value_class(program, Some(machine), state, right),
            ) else {
                return false;
            };
            if left_class == right_class {
                return false;
            }
            let detail = if matches!(left_class, ValueClass::Text)
                || matches!(right_class, ValueClass::Text)
            {
                "text and non-text operands cannot be compared or combined"
            } else {
                "Omega does not coerce a boolean to a number -- compare booleans directly \
             (`b == true`) or convert a number explicitly (`n != 0`)"
            };
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` applies an operator to {} and {} -- {detail}",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
                left_class.describe(),
                right_class.describe(),
            )));
            true
        }

        pub(crate) fn type_reference_is_text_carrier(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
        ) -> bool {
            let Some(carrier) =
                crate::value_custody::reference_shells::carrier_beneath_borrows(program, handle)
            else {
                return false;
            };
            let element_type = match program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => *element_type,
                _ => return false,
            };
            program.primitive_type_reference(element_type) == Some(PrimitiveType::U8)
        }

        fn report_non_bool_logical_operands(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            use typed_trees::expression::BinaryOperator;
            if !matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                return false;
            }
            let non_bool = |value| {
                matches!(
                    value_class(program, Some(machine), state, value),
                    Some(ValueClass::Numeric) | Some(ValueClass::Text)
                )
            };
            if !non_bool(left) && !non_bool(right) {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` applies logical `{operator:?}` to a non-bool operand, but \
         `&&`/`||` require `bool` operands",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
            true
        }

        fn report_invalid_text_operator(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            use typed_trees::expression::BinaryOperator;
            if !matches!(
                operator,
                BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) {
                return false;
            }
            let is_text =
                |value| value_class(program, Some(machine), state, value) == Some(ValueClass::Text);
            if !is_text(left) || !is_text(right) {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to text operands, but text supports only \
         concatenation (`+`), `==`, and `!=`",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
            true
        }

        pub(crate) fn report_non_bool_logical_not(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operand: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let class = value_class(program, Some(machine), state, operand);
            if !matches!(class, Some(ValueClass::Numeric) | Some(ValueClass::Text)) {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies logical `!` to {}, but `!` requires a `bool` operand \
         (bitwise-not is spelled `~`)",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        class.unwrap().describe(),
            )));
            true
        }

        pub(crate) fn report_non_integer_bitwise_not(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operand: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let class = value_class(program, Some(machine), state, operand);
            let invalid = matches!(class, Some(ValueClass::Boolean | ValueClass::Text))
                || expression_is_float_typed(program, machine, state, operand);
            if !invalid {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies bitwise `~` to {}, but `~` requires a fixed-width integer operand",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        class.map_or("a non-integer value", ValueClass::describe),
            )));
            true
        }

        pub(crate) fn report_non_integer_index(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            indexed: &typed_trees::expression::TableIndexedExpression,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            use language_core::operator_spelling::OperatorSpelling;
            let (spelling, index_operands) =
                match program.expression_table.expression(indexed.index) {
                    ExpressionNode::Range(range) => {
                        (OperatorSpelling::Range, vec![range.start, range.end])
                    }
                    _ => (OperatorSpelling::Index, vec![indexed.index]),
                };
            let operand_type = |operand: ExpressionHandle| {
                super::result_type::expression_result_type_reference(
                    program, machine, state, operand,
                )
            };
            let index_types = index_operands
                .iter()
                .map(|operand| operand_type(*operand))
                .collect::<Vec<_>>();
            let non_integer = index_operands
                .iter()
                .zip(&index_types)
                .filter_map(|(operand, index_type)| {
                    let primitive = crate::value_custody::places::unwrapped_type_reference(
                        program,
                        (*index_type)?,
                    )
                    .and_then(|unwrapped| program.primitive_type_reference(unwrapped))?;
                    (!primitive.accepts_integer_literal()).then_some((*operand, primitive))
                })
                .collect::<Vec<_>>();
            if non_integer.is_empty() {
                return false;
            }
            let operand_types = std::iter::once(operand_type(indexed.collection))
                .chain(index_types)
                .collect::<Vec<_>>();
            if !typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                spelling,
                &operand_types,
                program.expression_table.source_span(indexed.index),
            )
            .is_empty()
                || !typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    machine.symbol,
                    spelling,
                    &operand_types,
                )
                .is_empty()
            {
                return false;
            }
            for (operand, primitive) in non_integer {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` indexes `{}` with `{}` of type `{}`, but an index or range \
             endpoint must be an integer",
            machine.name.as_str(),
            state.name.as_str(),
            program.diagnostic_expression_name(indexed.collection),
            program.diagnostic_expression_name(operand),
            primitive.name(),
        )));
            }
            true
        }

        fn binary_operator_spelling(
            operator: typed_trees::expression::BinaryOperator,
        ) -> Option<language_core::operator_spelling::OperatorSpelling> {
            use language_core::operator_spelling::OperatorSpelling;
            use typed_trees::expression::BinaryOperator;
            Some(match operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                _ => return None,
            })
        }

        fn report_undeclared_struct_operator(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let Some(spelling) = binary_operator_spelling(operator) else {
                return false;
            };
            let operand_types = [left, right].map(|operand| {
                state.and_then(|state| {
                    super::result_type::expression_result_type_reference(
                        program, machine, state, operand,
                    )
                })
            });
            let Some(type_name) = operand_types
                .iter()
                .flatten()
                .find_map(|reference| concrete_data_type_name(program, *reference))
            else {
                return false;
            };
            if !typed_trees::operator::resolve_spelling_for_operands(
                program,
                spelling,
                &operand_types,
                program.expression_table.source_span(left),
            )
            .is_empty()
            {
                return false;
            }
            if !typed_trees::operator::selected_trait_operator_meanings(
                program,
                machine.symbol,
                spelling,
                &operand_types,
            )
            .is_empty()
            {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{type_name}` value, but no such \
         operator is declared for these operand types; declare a matching token-bearing \
         machine or select its declared trait meaning",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
            true
        }

        fn report_array_operator_operands(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            operator: typed_trees::expression::BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            use typed_trees::expression::BinaryOperator;
            if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                return false;
            }
            let is_equality = matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual);
            for operand in [left, right] {
                if let Some(operand_type) = crate::value_custody::places::declared_place_type(
                    program, machine, state, operand,
                ) && type_reference_is_collection(program, operand_type)
                    && !type_reference_is_text_carrier(program, operand_type)
                {
                    let detail = if is_equality {
                        "arrays do not support `==` / `!=` yet (element-wise array equality is not \
                 synthesized -- compare elements individually)"
                    } else {
                        "ordering, arithmetic, and bitwise operators are not defined for arrays"
                    };
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` applies `{operator:?}` to an array operand, but {detail}",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
            )));
                    return true;
                }
            }
            false
        }
    }
    pub(crate) use float_operations::validate_float_expression_formats;

    pub(crate) mod float_destinations {
        use crate::CheckingInput;
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub(crate) fn report_mismatch(
            program: &dyn CheckingInput,
            machine: Option<&Machine>,
            state: Option<&State>,
            value: ExpressionHandle,
            target: PrimitiveType,
            slot_context: &str,
            slot_noun: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let mut direct = value;
            direct = program.expression_table.peeled_expression_handle(direct);
            if matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
                && matches!(
                    program.expression_table.expression(direct),
                    ExpressionNode::Float(_)
                )
            {
                return false;
            }

            let source = source_primitive(program, machine, state, direct);
            if let Some(source @ (PrimitiveType::F32 | PrimitiveType::F64)) = source
                && source != target
            {
                diagnostics.push(Diagnostic::error(format!(
                "{slot_context} delivers a `{}` value to a `{}` {slot_noun}; a landed float retains its format, so changing format requires an explicit conversion",
                source.name(), target.name(),
            )));
                return true;
            }
            false
        }

        fn source_primitive(
            program: &dyn CheckingInput,
            machine: Option<&Machine>,
            state: Option<&State>,
            value: ExpressionHandle,
        ) -> Option<PrimitiveType> {
            match program.expression_table.expression(value) {
                ExpressionNode::UndeterminedNaN(format) => Some(match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                }),
                ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                }),
                ExpressionNode::Cast(cast) => primitive(program, cast.target_type),
                ExpressionNode::Call(call) => {
                    crate::machine_calls::calls::call_result_type(program, value)
                        .or_else(|| {
                            typed_trees::operator::resolve_named_expression_call(program, call)
                                .map(|operator| operator.return_type)
                        })
                        .and_then(|reference| primitive(program, reference))
                }
                ExpressionNode::Name(path) => super::named_value_type_reference(program, path)
                    .or_else(|| {
                        machine.and_then(|machine| {
                            crate::value_custody::places::declared_place_type(
                                program, machine, state, value,
                            )
                        })
                    })
                    .and_then(|reference| primitive(program, reference)),
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => machine
                    .and_then(|machine| {
                        crate::value_custody::places::declared_place_type(
                            program, machine, state, value,
                        )
                    })
                    .and_then(|reference| primitive(program, reference)),
                ExpressionNode::Binary(binary)
                    if crate::value_custody::literals::has_anonymous_operator_meaning(
                        program, value,
                    ) =>
                {
                    [binary.left, binary.right]
                        .into_iter()
                        .filter_map(|operand| source_primitive(program, machine, state, operand))
                        .find(|primitive| {
                            matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                        })
                }
                _ => None,
            }
        }

        fn primitive(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            let reference =
                crate::value_custody::places::unwrapped_type_reference(program, reference)?;
            program.primitive_type_reference(reference)
        }
    }
    pub(crate) mod shape_validation {
        use super::operator_validation::type_reference_is_text_carrier;
        use super::value_classification::{
            ValueClass, concrete_data_type_name, value_class, value_concrete_data_name,
        };
        use crate::value_custody::places::collection_operations::type_reference_is_collection;
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn value_shape_is_array(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
        ) -> Option<bool> {
            match program.expression_table.expression(value) {
                ExpressionNode::ArrayLiteral(_) => Some(true),
                ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::String(_)
                | ExpressionNode::StructLiteral(_) => Some(false),
                ExpressionNode::Borrow(inner) => {
                    value_shape_is_array(program, machine, state, inner.target)
                }
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                    crate::value_custody::places::declared_place_type(
                        program, machine, state, value,
                    )
                    .map(|type_reference| type_reference_is_collection(program, type_reference))
                }
                _ => None,
            }
        }

        pub(crate) fn report_array_scalar_shape_mismatch(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            target_type: TypeReferenceHandle,
            slot_context: &str,
            slot_noun: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if crate::value_custody::literals::validate_constant_projection_destination(
                program,
                machine.symbol,
                value,
                target_type,
                diagnostics,
            ) {
                return true;
            }
            if type_reference_is_text_carrier(program, target_type)
                || value_class(program, Some(machine), state, value) == Some(ValueClass::Text)
                || crate::value_custody::places::declared_place_type(program, machine, state, value)
                    .is_some_and(|value_type| type_reference_is_text_carrier(program, value_type))
            {
                return false;
            }
            let Some(value_is_array) = value_shape_is_array(program, machine, state, value) else {
                return false;
            };
            if value_is_array == type_reference_is_collection(program, target_type) {
                return false;
            }
            diagnostics.push(Diagnostic::error(if value_is_array {
                format!("{slot_context} binds an ARRAY value into a non-array {slot_noun}")
            } else {
                format!("{slot_context} binds a non-array value into an ARRAY {slot_noun}")
            }));
            true
        }

        pub(crate) fn report_scalar_data_shape_mismatch(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            target_type: TypeReferenceHandle,
            slot_context: &str,
            slot_noun: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if let Some(target_name) = concrete_data_type_name(program, target_type)
                && let Some(value_scalar_class) = value_class(program, Some(machine), state, value)
            {
                diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds {} into the `{target_name}` data {slot_noun}; a scalar value \
             cannot fill a struct or enum slot",
            value_scalar_class.describe(),
        )));
                return true;
            }
            if program.primitive_type_reference(target_type).is_some()
                && let Some(value_name) = value_concrete_data_name(program, machine, state, value)
            {
                diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds a `{value_name}` value into a scalar {slot_noun}; a struct or \
             enum value cannot fill a scalar slot",
        )));
                return true;
            }
            false
        }

        fn slice_view_element(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            let mut handle = type_reference;
            loop {
                match program.type_reference_table.type_reference(handle) {
                    TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                    _ => return None,
                }
            }
        }

        fn collection_element(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Slice { element_type }
                | TypeReferenceNode::FixedArray { element_type, .. } => Some(*element_type),
                _ => None,
            }
        }

        fn argument_collection_element(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            argument: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            let mut handle = argument;
            handle = program.expression_table.peeled_expression_handle(handle);
            if let Some(indexed) = program.expression_table.indexed_node(handle)
                && matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                return crate::value_custody::places::declared_place_type(
                    program,
                    machine,
                    state,
                    indexed.collection,
                )
                .and_then(|type_reference| collection_element(program, type_reference));
            }
            crate::value_custody::places::declared_place_type(program, machine, state, handle)
                .and_then(|type_reference| collection_element(program, type_reference))
        }

        fn element_is_concrete(
            program: &dyn crate::CheckingInput,
            element: TypeReferenceHandle,
        ) -> bool {
            let handle = program
                .type_reference_table
                .unconstrained_type_reference(element);
            !matches!(
                program.type_reference_table.type_reference(handle),
                TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            ) && !matches!(
                program.type_reference_table.type_reference(handle),
                TypeReferenceNode::Generic { .. } | TypeReferenceNode::DynamicTrait { .. }
            )
        }

        pub(crate) fn report_view_element_argument_mismatch(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            argument: ExpressionHandle,
            target_type: TypeReferenceHandle,
            slot_context: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let Some(required_element) = slice_view_element(program, target_type) else {
                return false;
            };
            let Some(actual_element) =
                argument_collection_element(program, machine, state, argument)
            else {
                return false;
            };
            if !element_is_concrete(program, required_element)
                || !element_is_concrete(program, actual_element)
                || program.normalized_type_identity(actual_element)
                    == program.normalized_type_identity(required_element)
            {
                return false;
            }
            diagnostics.push(Diagnostic::error(format!(
                "{slot_context} expects element type `{}`, but the argument supplies `{}`",
                program.display_type_reference_with_constraints(required_element),
                program.display_type_reference_with_constraints(actual_element),
            )));
            true
        }
    }
    pub(crate) mod value_classification {
        use crate::declarations::data_queries::data_definition_by_symbol;
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ValueClass {
            Boolean,
            Text,
            Numeric,
        }

        impl ValueClass {
            pub(crate) fn describe(self) -> &'static str {
                match self {
                    Self::Boolean => "a boolean",
                    Self::Text => "text",
                    Self::Numeric => "a numeric value",
                }
            }

            fn of_literal(
                program: &dyn crate::CheckingInput,
                value: ExpressionHandle,
            ) -> Option<Self> {
                match program.expression_table.expression(value) {
                    ExpressionNode::Boolean(_) => Some(Self::Boolean),
                    ExpressionNode::String(_) => Some(Self::Text),
                    ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_) => Some(Self::Numeric),
                    ExpressionNode::Borrow(inner) => Self::of_literal(program, inner.target),
                    _ => None,
                }
            }

            fn of_primitive(primitive: PrimitiveType) -> Self {
                match primitive {
                    PrimitiveType::Bool => Self::Boolean,
                    _ => Self::Numeric,
                }
            }
        }

        pub(crate) fn value_class(
            program: &dyn crate::CheckingInput,
            machine: Option<&typed_trees::machine::Machine>,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
        ) -> Option<ValueClass> {
            if let Some(class) = ValueClass::of_literal(program, value) {
                return Some(class);
            }
            if let Some(binary) = program.expression_table.binary_node(value) {
                use typed_trees::expression::BinaryOperator;
                if binary.operator.is_comparison()
                    || matches!(
                        binary.operator,
                        BinaryOperator::And | BinaryOperator::Or | BinaryOperator::CaseMembership
                    )
                {
                    let (machine, state) = (machine?, state?);
                    let primitive = super::result_type::expression_result_type_reference(
                        program, machine, state, value,
                    )
                    .and_then(|handle| program.primitive_type_reference(handle))?;
                    return Some(ValueClass::of_primitive(primitive));
                }
                let left = value_class(program, machine, state, binary.left);
                let right = value_class(program, machine, state, binary.right);
                if crate::proof_contracts::arithmetic_domains::is_arithmetic(binary.operator) {
                    return match (left, right) {
                        (Some(ValueClass::Text), _) | (_, Some(ValueClass::Text)) => {
                            Some(ValueClass::Text)
                        }
                        (Some(_), _) | (_, Some(_)) => Some(ValueClass::Numeric),
                        (None, None) => None,
                    };
                }
                if matches!(
                    binary.operator,
                    BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseOr
                        | BinaryOperator::BitwiseXor
                ) {
                    return match (left, right) {
                        (Some(ValueClass::Numeric), _) | (_, Some(ValueClass::Numeric)) => {
                            Some(ValueClass::Numeric)
                        }
                        _ => None,
                    };
                }
            }
            if let Some(unary) = program.expression_table.unary_node(value) {
                return match unary.operator {
                    typed_trees::expression::UnaryOperator::BitwiseNot => Some(ValueClass::Numeric),
                    typed_trees::expression::UnaryOperator::LogicalNot => Some(ValueClass::Boolean),
                };
            }
            if let Some(call) = program.expression_table.call_node(value) {
                let reference = match (machine, state) {
                    (Some(machine), Some(state)) => {
                        super::result_type::expression_result_type_reference(
                            program, machine, state, value,
                        )
                    }
                    _ => {
                        crate::proof_contracts::arithmetic_domains::call_return_type(program, call)
                    }
                };
                if let Some(primitive) =
                    reference.and_then(|handle| program.primitive_type_reference(handle))
                {
                    return Some(ValueClass::of_primitive(primitive));
                }
                return None;
            }
            let machine = machine?;
            let primitive =
                crate::value_custody::places::declared_place_type(program, machine, state, value)
                    .and_then(|handle| program.primitive_type_reference(handle))?;
            Some(ValueClass::of_primitive(primitive))
        }

        pub(crate) fn cross_class_conflict(
            program: &dyn crate::CheckingInput,
            machine: Option<&typed_trees::machine::Machine>,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            target: PrimitiveType,
        ) -> Option<(ValueClass, ValueClass)> {
            let value_class = value_class(program, machine, state, value)?;
            let target_class = ValueClass::of_primitive(target);
            (value_class != target_class).then_some((value_class, target_class))
        }

        pub(crate) fn report_cross_class_store(
            program: &dyn crate::CheckingInput,
            machine: Option<&typed_trees::machine::Machine>,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            target: PrimitiveType,
            slot_context: &str,
            slot_noun: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if program.static_type_context_is_unreachable(value) {
                return false;
            }
            if let Some(dispatch) = program.expression_table.match_node(value) {
                let mut rejected = false;
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    rejected |= report_cross_class_store(
                        program,
                        machine,
                        state,
                        arm.value,
                        target,
                        slot_context,
                        slot_noun,
                        diagnostics,
                    );
                }
                return rejected;
            }
            if let Some(machine) = machine
                && let Some(actual) =
                    crate::builtin_constant_array_projection_type(program, machine.symbol, value)
                && let Some(actual) = program.primitive_type_reference(actual)
                && actual != target
            {
                diagnostics.push(Diagnostic::error(format!(
            "{slot_context} stores a constant array projection of type `{}` into a `{}` {slot_noun}; use an explicit conversion",
            actual.name(), target.name(),
        )));
                return true;
            }
            if let Some(call) = program.expression_table.call_node(value)
                && let Some(operator) =
                    typed_trees::operator::resolve_named_expression_call(program, call)
                && let Some(source) = program.primitive_type_reference(operator.return_type)
                && source != target
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{slot_context} stores the `{}` result of named operator `{}` into a `{}` \
             {slot_noun}; numeric representation changes require an explicit named conversion",
                    source.name(),
                    call.target,
                    target.name(),
                )));
                return true;
            }

            if super::float_destinations::report_mismatch(
                program,
                machine,
                state,
                value,
                target,
                slot_context,
                slot_noun,
                diagnostics,
            ) {
                return true;
            }

            let Some((value_class, target_class)) =
                cross_class_conflict(program, machine, state, value, target)
            else {
                return false;
            };
            diagnostics.push(Diagnostic::error(format!(
                "{slot_context} stores {} into a `{}` {slot_noun}, which holds {}",
                value_class.describe(),
                target.name(),
                target_class.describe(),
            )));
            true
        }

        fn concrete_data_type_symbol(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
        ) -> Option<symbols::SymbolHandle> {
            if !handle.is_valid() {
                return None;
            }
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Reference { referee, .. } => {
                    concrete_data_type_symbol(program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    concrete_data_type_symbol(program, *base_type)
                }
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    (data_definition_by_symbol(program, *symbol).is_some()).then_some(*symbol)
                }
                _ => None,
            }
        }

        pub(crate) fn concrete_data_type_name(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
        ) -> Option<&str> {
            let symbol = concrete_data_type_symbol(program, handle)?;
            data_definition_by_symbol(program, symbol).map(|definition| definition.name.as_str())
        }

        fn constructed_data_symbol(
            program: &dyn crate::CheckingInput,
            value: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            let symbol = match program.expression_table.expression(value) {
                ExpressionNode::StructLiteral(literal) => literal.type_symbol,
                ExpressionNode::Name(path)
                    if program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
                {
                    program.symbols.get(path.symbol).parent
                }
                ExpressionNode::Borrow(inner) => {
                    return constructed_data_symbol(program, inner.target);
                }
                _ => return None,
            };
            data_definition_by_symbol(program, symbol)
                .filter(|definition| definition.type_parameters.is_empty())
                .map(|definition| definition.symbol)
        }

        pub(crate) fn value_concrete_data_symbol(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            constructed_data_symbol(program, value)
                .or_else(|| {
                    crate::value_custody::places::declared_place_type(
                        program, machine, state, value,
                    )
                    .and_then(|reference| concrete_data_type_symbol(program, reference))
                })
                .or_else(|| {
                    crate::machine_calls::calls::call_result_type(program, value)
                        .and_then(|reference| concrete_data_type_symbol(program, reference))
                })
        }

        pub(crate) fn value_concrete_data_name<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
        ) -> Option<&'program str> {
            let symbol = value_concrete_data_symbol(program, machine, state, value)?;
            data_definition_by_symbol(program, symbol).map(|definition| definition.name.as_str())
        }

        pub(crate) fn report_data_type_conflict(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
            value: ExpressionHandle,
            expected_type: TypeReferenceHandle,
            slot_context: &str,
            slot_noun: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if let Some(dispatch) = program.expression_table.match_node(value) {
                let mut rejected = false;
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    rejected |= report_data_type_conflict(
                        program,
                        machine,
                        state,
                        arm.value,
                        expected_type,
                        slot_context,
                        slot_noun,
                        diagnostics,
                    );
                }
                return rejected;
            }
            let Some(expected) = concrete_data_type_symbol(program, expected_type) else {
                return false;
            };
            let Some(got) = value_concrete_data_symbol(program, machine, state, value) else {
                return false;
            };
            if expected == got
                || program
                    .normalized_nominal_type_identity(expected)
                    .is_some_and(|expected| {
                        Some(expected) == program.normalized_nominal_type_identity(got)
                    })
            {
                return false;
            }
            let expected = program.symbols.display_path(expected, "::");
            let got = program.symbols.display_path(got, "::");
            diagnostics.push(Diagnostic::error(format!(
        "{slot_context} expects the `{expected}` data type but got `{got}` in the `{slot_noun}` \
         position; these are incompatible data types (a place is accepted structurally, but its \
         declared type must match)",
            )));
            true
        }
    }

    pub(crate) mod call_carriers {

        use super::*;

        pub(crate) fn argument_matches_call_carrier(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            argument: ExpressionHandle,
            required: TypeReferenceHandle,
            required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            if argument.is_valid()
                && program.pattern_component(argument)
                    != Some(language_core::PatternComponent::Fields)
            {
                return false;
            }
            if argument_matches_carrier(program, scope, argument, required, required_substitutions)
            {
                return true;
            }
            let raw_required = required;
            let (required, required_substitutions) =
                reference_values::substituted_required_reference(
                    program,
                    required,
                    required_substitutions,
                );
            if required != raw_required {
                return false;
            }
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            let borrow = program.expression_table.borrow_expression(argument);
            if borrow.is_some_and(|borrow| borrow.access != *access) {
                return false;
            }
            let source = borrow.map_or(argument, |borrow| borrow.target);
            let mut substitutions = Vec::new();
            let Some(mut actual) =
                reference_values::declared_value_type(program, source, &mut substitutions)
            else {
                return false;
            };
            if let TypeReferenceNode::Reference {
                referee,
                access: actual_access,
                ..
            } = program.type_reference_table.type_reference(actual)
            {
                if actual_access != access {
                    return false;
                }
                actual = *referee;
            } else if borrow.is_none() {
                return false;
            }
            call_value_index_carriers_match(
                program,
                actual,
                &substitutions,
                *referee,
                required_substitutions,
                0,
            )
        }

        fn call_value_index_carriers_match(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            required: TypeReferenceHandle,
            required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            depth: usize,
        ) -> bool {
            if depth >= 128 {
                return false;
            }
            if substituted_identity(program, actual, substitutions)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required)
                })
            {
                return true;
            }
            let Some(actual) =
                reference_values::substituted_reference(program, actual, substitutions)
            else {
                return false;
            };
            let raw_required = required;
            let (required, required_substitutions) =
                reference_values::substituted_required_reference(
                    program,
                    required,
                    required_substitutions,
                );
            if required != raw_required {
                return false;
            }
            let actual = delivered_types::application_form(program, actual);
            let required = delivered_types::application_form(program, required);
            let (
                TypeReferenceNode::Generic {
                    base_symbol: left_base,
                    arguments: left,
                    lifetime_arguments: left_lifetimes,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_base,
                    arguments: right,
                    lifetime_arguments: right_lifetimes,
                    ..
                },
            ) = (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            )
            else {
                return false;
            };
            if left_base != right_base || !left_lifetimes.is_empty() || !right_lifetimes.is_empty()
            {
                return false;
            }
            let Some(data) = program.data_definition_by_symbol(*left_base) else {
                return false;
            };
            let parameters = program.data_type_parameters(data);
            let left = program.type_reference_table.type_reference_handles(*left);
            let right = program.type_reference_table.type_reference_handles(*right);
            parameters.len() == left.len()
                && left.len() == right.len()
                && parameters.iter().zip(left.iter().zip(right)).all(
                    |(parameter, (left, right))| match parameter.kind {
                        typed_trees::data::TypeParameterKind::Value { .. } => true,
                        typed_trees::data::TypeParameterKind::Type => {
                            call_value_index_carriers_match(
                                program,
                                *left,
                                substitutions,
                                *right,
                                required_substitutions,
                                depth + 1,
                            )
                        }
                        _ => {
                            substituted_identity(program, *left, substitutions)
                                == program.type_identity_with_simultaneous_substitutions(
                                    TypeIdentityRequest {
                                        substitutions: required_substitutions,
                                        ..TypeIdentityRequest::ordinary(*right)
                                    },
                                )
                        }
                    },
                )
        }
    }
    pub(crate) mod delivered_types {

        use super::{
            ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypeReferenceNode,
            named_value_type_reference, reference_values,
        };
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::types::PrimitiveType;

        pub(crate) struct DeliveredType {
            pub(crate) reference: TypeReferenceHandle,
            pub(crate) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        }

        pub(crate) fn delivered_value_type(
            program: &dyn crate::CheckingInput,
            value: ExpressionHandle,
        ) -> Option<DeliveredType> {
            if let Some(declaration) = program.expression_const_declaration(value) {
                return declaration
                    .declared_type
                    .is_valid()
                    .then_some(DeliveredType {
                        reference: declaration.declared_type,
                        substitutions: Vec::new(),
                    });
            }
            let mut substitutions = Vec::new();
            let reference = match expression_table(program).expression(value) {
                ExpressionNode::Call(_) => {
                    expression_table(program).constructed_value_type_reference(value)
                }
                ExpressionNode::Name(path) => named_value_type_reference(program, path)?,
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                    reference_values::declared_value_type(program, value, &mut substitutions)?
                }
                ExpressionNode::Atomic(_) => {
                    let load = crate::atomic_load_carrier(program, value)?;
                    let cell = reference_values::declared_value_type(
                        program,
                        load.place,
                        &mut substitutions,
                    )?;
                    let cell =
                        reference_values::substituted_reference(program, cell, &substitutions)?;
                    substitutions.clear();
                    crate::atomic_resident_type_reference(program, cell)?
                }
                _ => return None,
            };
            reference.is_valid().then_some(DeliveredType {
                reference,
                substitutions,
            })
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Position {
            Stored,
            Applied,
        }

        pub(crate) fn application_form(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let symbol = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => symbol,
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if arguments.count() == 0 => base_symbol,
                _ => return reference,
            };
            if let Some(origin) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
                && origin.is_valid()
            {
                program
                    .type_reference_table
                    .unconstrained_type_reference(origin)
            } else {
                reference
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Carrier {
            Parameter(SymbolHandle),
            Data(SymbolHandle),
            Implementation(SymbolHandle),
            Primitive(PrimitiveType),
        }

        fn carrier(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<Carrier> {
            if let Some(primitive) = program.primitive_type_reference(reference) {
                return Some(Carrier::Primitive(primitive));
            }
            let symbol = program.type_reference_table.type_head_symbol(reference)?;
            if !symbol.is_valid() {
                return None;
            }
            match program.symbols.get(symbol).kind {
                symbols::SymbolKind::TypeParameter | symbols::SymbolKind::ConformanceParameter => {
                    Some(Carrier::Parameter(symbol))
                }
                symbols::SymbolKind::Data => Some(Carrier::Data(symbol)),
                symbols::SymbolKind::Conformance => Some(Carrier::Implementation(symbol)),
                _ => None,
            }
        }

        fn parameter_is_in_scope(
            program: &dyn crate::CheckingInput,
            parameter: SymbolHandle,
            mut scope: SymbolHandle,
        ) -> bool {
            let owner = program.symbols.get(parameter).parent;
            while scope.is_valid() {
                if scope == owner || program.lifetime_declaration_owner(scope) == owner {
                    return true;
                }
                scope = program.declaration_scope_parent(scope);
            }
            false
        }

        pub(crate) fn delivered_carrier_conflict(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            context: ExpressionHandle,
            actual: TypeReferenceHandle,
            actual_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            required: TypeReferenceHandle,
            required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            scope.is_valid()
                && carrier_conflict(
                    program,
                    scope,
                    context,
                    actual,
                    actual_substitutions,
                    required,
                    required_substitutions,
                    Position::Stored,
                )
        }

        fn carrier_conflict(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            context: ExpressionHandle,
            actual: TypeReferenceHandle,
            actual_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            required: TypeReferenceHandle,
            required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            position: Position,
        ) -> bool {
            let (required, required_substitutions) =
                reference_values::substituted_required_reference(
                    program,
                    required,
                    required_substitutions,
                );
            let unconstrained = |reference| {
                program
                    .type_reference_table
                    .unconstrained_type_reference(reference)
            };
            let Some(actual) = reference_values::substituted_reference(
                program,
                unconstrained(actual),
                actual_substitutions,
            ) else {
                return false;
            };
            let actual = program.statically_refined_type_reference(context, unconstrained(actual));
            let required =
                program.statically_refined_type_reference(context, unconstrained(required));
            let rigid = |carrier: Carrier| {
                matches!(carrier, Carrier::Parameter(parameter)
            if parameter_is_in_scope(program, parameter, scope))
            };
            match (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            ) {
                (
                    TypeReferenceNode::FixedArray {
                        element_type: actual,
                        ..
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: required,
                        ..
                    },
                ) => {
                    return carrier_conflict(
                        program,
                        scope,
                        context,
                        *actual,
                        actual_substitutions,
                        *required,
                        required_substitutions,
                        position,
                    );
                }
                (TypeReferenceNode::Reference { .. }, _)
                | (_, TypeReferenceNode::Reference { .. }) => {
                    return false;
                }
                _ => {}
            }
            let (Some(actual_carrier), Some(required_carrier)) =
                (carrier(program, actual), carrier(program, required))
            else {
                return false;
            };
            let foreign =
                |carrier: Carrier| matches!(carrier, Carrier::Parameter(_)) && !rigid(carrier);
            if foreign(actual_carrier) || foreign(required_carrier) {
                return false;
            }
            if actual_carrier != required_carrier
                && (rigid(actual_carrier) || rigid(required_carrier))
            {
                return true;
            }
            let declared = |reference| {
                matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { .. }
                )
            };
            if position == Position::Stored && declared(actual) && declared(required) {
                return false;
            }
            let actual = application_form(program, actual);
            let required = application_form(program, required);
            let (Some(actual_carrier), Some(required_carrier)) =
                (carrier(program, actual), carrier(program, required))
            else {
                return false;
            };
            if actual_carrier != required_carrier {
                return true;
            }
            let (
                TypeReferenceNode::Generic {
                    arguments: actual_arguments,
                    ..
                },
                TypeReferenceNode::Generic {
                    arguments: required_arguments,
                    ..
                },
            ) = (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            )
            else {
                return false;
            };
            let actual_arguments = program
                .type_reference_table
                .type_reference_handles(*actual_arguments);
            let required_arguments = program
                .type_reference_table
                .type_reference_handles(*required_arguments);
            let implementation_parameters = match actual_carrier {
                Carrier::Implementation(symbol) => program
                    .conformance_by_symbol(symbol)
                    .map(|declaration| program.conformance_type_parameters(declaration)),
                _ => None,
            };
            actual_arguments.len() == required_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(required_arguments.iter())
                    .enumerate()
                    .any(|(ordinal, (actual, required))| {
                        if let Some(parameter) = implementation_parameters
                            .and_then(|parameters| parameters.get(ordinal))
                            .filter(|parameter| {
                                !matches!(
                                    parameter.kind,
                                    typed_trees::data::TypeParameterKind::Type
                                        | typed_trees::data::TypeParameterKind::Implementation
                                )
                            })
                        {
                            return static_implementation_operand_conflict(
                                program,
                                scope,
                                &parameter.kind,
                                (*actual, actual_substitutions),
                                (*required, required_substitutions),
                            );
                        }
                        carrier_conflict(
                            program,
                            scope,
                            context,
                            *actual,
                            actual_substitutions,
                            *required,
                            required_substitutions,
                            Position::Applied,
                        )
                    })
        }

        fn static_implementation_operand_conflict(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            kind: &typed_trees::data::TypeParameterKind,
            actual: (TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)]),
            required: (TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)]),
        ) -> bool {
            use typed_trees::data::TypeParameterKind;
            use typed_trees::type_identity::{IndexIdentityArgument, StaticOperandIdentityRequest};
            let compare = |actual_operand, required_operand| {
                let actual_identity =
                    program.static_operand_identity(StaticOperandIdentityRequest {
                        operand: actual_operand,
                        lexical_owner: scope,
                        substitutions: actual.1,
                    });
                let required_identity = program
                    .static_operand_identity_with_simultaneous_substitutions(
                        StaticOperandIdentityRequest {
                            operand: required_operand,
                            lexical_owner: scope,
                            substitutions: required.1,
                        },
                    );
                !matches!((actual_identity, required_identity), (Some(actual), Some(required)) if actual == required)
            };
            match kind {
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => false,
                TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {
                    let Ok(actual_operand) = program.static_operand_from_type_reference(actual.0)
                    else {
                        return true;
                    };
                    let Ok(required_operand) =
                        program.static_operand_from_type_reference(required.0)
                    else {
                        return true;
                    };
                    compare(
                        IndexIdentityArgument::StaticArgument(&actual_operand),
                        IndexIdentityArgument::StaticArgument(&required_operand),
                    )
                }
                TypeParameterKind::Type | TypeParameterKind::Implementation => false,
            }
        }

        fn delivered_type_conflict(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            value: ExpressionHandle,
            destination: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            if program.static_type_context_is_unreachable(value) {
                return None;
            }
            if let Some(dispatch) = expression_table(program).match_node(value) {
                return expression_table(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .find_map(|arm| {
                        delivered_type_conflict(program, scope, arm.value, destination)
                    });
            }
            let DeliveredType {
                reference: delivered,
                substitutions,
            } = delivered_value_type(program, value)?;
            let conflict = if matches!(
                program.type_reference_table.type_reference(destination),
                TypeReferenceNode::Reference { .. }
            ) {
                matches!(
                    program.type_reference_table.type_reference(delivered),
                    TypeReferenceNode::Reference { .. }
                ) && !reference_values::value_carrier_matches_reference(
                    program,
                    delivered,
                    destination,
                    &substitutions,
                    &[],
                )
            } else {
                delivered_carrier_conflict(
                    program,
                    scope,
                    value,
                    delivered,
                    &substitutions,
                    destination,
                    &[],
                )
            };
            conflict.then(|| {
                reference_values::substituted_reference(program, delivered, &substitutions)
                    .unwrap_or(delivered)
            })
        }

        pub(crate) fn report_delivered_type_mismatch(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            value: ExpressionHandle,
            destination: TypeReferenceHandle,
            owner: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            report_delivered_type_mismatch_with_component(
                program,
                scope,
                value,
                destination,
                Some(language_core::PatternComponent::Fields),
                owner,
                diagnostics,
            );
        }

        pub(crate) fn report_delivered_type_mismatch_with_component(
            program: &dyn crate::CheckingInput,
            scope: SymbolHandle,
            value: ExpressionHandle,
            destination: TypeReferenceHandle,
            component: Option<language_core::PatternComponent>,
            owner: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if crate::value_custody::pattern_components::report_delivery(
                program,
                value,
                destination,
                component,
                owner,
                diagnostics,
            ) {
                return;
            }
            if let Some(delivered) = delivered_type_conflict(program, scope, value, destination) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} expects `{}`, but the value is delivered as `{}`",
                    program.display_type_reference_with_constraints(destination),
                    program.display_type_reference_with_constraints(delivered),
                )));
            }
        }
    }
    pub(crate) use call_carriers::argument_matches_call_carrier;
    pub(crate) mod reference_values {
        use super::{
            ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypeReferenceNode,
            named_value_type_reference,
        };
        use crate::frozen_program::expression_table;
        use crate::value_custody::places::declared_reference_access;
        use crate::value_custody::type_references::substituted_identity;
        use language_semantics::ReferenceAccess;

        use typed_trees::data::TypeParameterKind;
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::{FixedArrayLength, TypeConstraintNode};

        pub(crate) fn reference_type_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::Reference {
                access: actual_access,
                referee: actual_referee,
                ..
            } = program.type_reference_table.type_reference(actual)
            else {
                return false;
            };
            let TypeReferenceNode::Reference {
                access: required_access,
                referee: required_referee,
                ..
            } = program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            let actual_identity =
                |reference| substituted_identity(program, reference, substitutions);
            if actual_identity(actual)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required)
                })
            {
                return true;
            }
            if *actual_access == ReferenceAccess::WriteOnly
                || *required_access == ReferenceAccess::WriteOnly
                || (*actual_access != *required_access
                    && !(*actual_access == ReferenceAccess::Mutable
                        && *required_access == ReferenceAccess::Shared))
            {
                return false;
            }
            actual_identity(*actual_referee)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(*required_referee)
                })
                || ((*required_access == ReferenceAccess::Shared
                    || matches!(
                        program.type_reference_table.type_reference(*actual_referee),
                        TypeReferenceNode::FixedArray { .. }
                    ))
                    && owned_array_projects_to_slice(
                        program,
                        *actual_referee,
                        *required_referee,
                        substitutions,
                        required_substitutions,
                    ))
                || owned_array_satisfies_constrained_referee(
                    program,
                    *actual_referee,
                    *required_referee,
                    substitutions,
                    required_substitutions,
                )
                || (*required_access == ReferenceAccess::Shared
                    && constrained_referee_forgets_qualification(
                        program,
                        *actual_referee,
                        *required_referee,
                        substitutions,
                        required_substitutions,
                    ))
                || fixed_array_binds_open_length(
                    program,
                    *actual_referee,
                    *required_referee,
                    substitutions,
                    required_substitutions,
                )
        }

        pub(crate) type ReferenceCorrespondence = fn(
            &dyn crate::CheckingInput,
            TypeReferenceHandle,
            TypeReferenceHandle,
            &[(symbols::SymbolHandle, TypeReferenceHandle)],
            &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool;

        pub(crate) fn value_carrier_matches_reference(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::Reference {
                access: ReferenceAccess::Shared,
                referee: required_referee,
                ..
            } = program.type_reference_table.type_reference(required)
            else {
                return declared_value_matches_reference(
                    program,
                    actual,
                    required,
                    substitutions,
                    required_substitutions,
                );
            };
            let actual_referee = match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::Reference {
                    access: ReferenceAccess::WriteOnly,
                    ..
                } => return false,
                TypeReferenceNode::Reference { referee, .. } => *referee,
                _ => actual,
            };
            shared_referee_matches(
                program,
                actual_referee,
                *required_referee,
                substitutions,
                required_substitutions,
            )
        }

        fn shared_referee_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            if substituted_identity(program, actual, substitutions)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required)
                })
            {
                return true;
            }
            let actual = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            let required = declared_domain_base_reference(program, required);
            substituted_identity(program, actual, substitutions)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required)
                })
                || owned_array_projects_to_slice(
                    program,
                    actual,
                    required,
                    substitutions,
                    required_substitutions,
                )
                || fixed_array_binds_open_length(
                    program,
                    actual,
                    required,
                    substitutions,
                    required_substitutions,
                )
        }

        fn declared_domain_base_reference(
            program: &dyn crate::CheckingInput,
            mut required: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(required)
            {
                if !program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| {
                        matches!(constraint, TypeConstraintNode::Domain(domain)
                if domain.symbol.is_valid()
                    && domain.subject == typed_trees::types::DomainConstraintSubject::Declared)
                    })
                {
                    break;
                }
                required = *base_type;
            }
            required
        }

        pub(crate) fn fixed_array_binds_open_length(
            program: &dyn crate::CheckingInput,
            mut actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::FixedArray {
                element_type: required_element,
                length:
                    FixedArrayLength::ConstParameter {
                        symbol: required_symbol,
                        ..
                    },
            } = program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            actual = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            let TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            } = program.type_reference_table.type_reference(actual)
            else {
                return false;
            };
            if substituted_identity(program, *actual_element, substitutions)
                != program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(*required_element)
                })
            {
                return false;
            }
            let Some(required_declared) = const_length_declared_type(program, *required_symbol)
            else {
                return false;
            };
            match actual_length {
                FixedArrayLength::Literal(length) => {
                    literal_satisfies_declared_const(program, *length, required_declared)
                }
                FixedArrayLength::ConstParameter {
                    symbol: actual_symbol,
                    ..
                } => const_length_declared_type(program, *actual_symbol).is_some_and(
                    |actual_declared| {
                        crate::value_custody::type_references::type_references_match(
                            program,
                            actual_declared,
                            required_declared,
                        )
                    },
                ),
                FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => false,
            }
        }

        fn const_length_declared_type(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            if !symbol.is_valid() {
                return None;
            }
            program
                .data_type_parameters
                .iter()
                .find(|(_, parameter)| parameter.symbol == symbol)
                .and_then(|(_, parameter)| match parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => Some(type_reference),
                    _ => None,
                })
        }

        fn literal_satisfies_declared_const(
            program: &dyn crate::CheckingInput,
            length: usize,
            declared: TypeReferenceHandle,
        ) -> bool {
            let Some(primitive) = program.primitive_type_reference(declared) else {
                return false;
            };
            let Ok(value) = i128::try_from(length) else {
                return false;
            };
            primitive.accepts_integer_literal()
                && crate::value_custody::type_references::const_integer_value_fits_primitive(
                    primitive, value,
                )
        }

        pub(crate) fn subslice_view_matches_reference(
            program: &dyn crate::CheckingInput,
            indexed: &typed_trees::expression::TableIndexedExpression,
            required: TypeReferenceHandle,
            correspondence: ReferenceCorrespondence,
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let mut substitutions = Vec::new();
            let Some(collection) = selected_value_type(
                program,
                indexed.collection,
                &mut substitutions,
                None,
                &mut ProjectionContext::default(),
            ) else {
                return false;
            };
            let Some(collection) = substituted_reference(program, collection, &substitutions)
            else {
                return false;
            };
            matches!(
                program.type_reference_table.type_reference(collection),
                TypeReferenceNode::Reference { .. }
            ) && correspondence(
                program,
                collection,
                required,
                &substitutions,
                required_substitutions,
            )
        }

        pub(crate) fn projected_matches_reference(
            program: &dyn crate::CheckingInput,
            scope: symbols::SymbolHandle,
            expression: ExpressionHandle,
            required: TypeReferenceHandle,
            correspondence: ReferenceCorrespondence,
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let mut substitutions = Vec::new();
            let Some(actual) = selected_value_type(
                program,
                expression,
                &mut substitutions,
                None,
                &mut ProjectionContext {
                    owner_symbol: scope,
                    ..ProjectionContext::default()
                },
            ) else {
                return false;
            };
            let Some(actual) = substituted_reference(program, actual, &substitutions) else {
                return false;
            };
            correspondence(
                program,
                actual,
                required,
                &substitutions,
                required_substitutions,
            )
        }

        pub(crate) fn declared_value_matches_reference(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::Reference {
                access: required_access,
                referee: required_referee,
                ..
            } = program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            if matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            ) {
                return reference_type_matches(
                    program,
                    actual,
                    required,
                    substitutions,
                    required_substitutions,
                );
            }
            *required_access == ReferenceAccess::Shared
                && owned_value_matches_referee(
                    program,
                    actual,
                    *required_referee,
                    substitutions,
                    required_substitutions,
                )
        }

        pub(crate) fn owned_value_matches_referee(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required_referee: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            if owned_referee_carrier_matches(
                program,
                actual,
                required_referee,
                substitutions,
                required_substitutions,
            ) {
                return true;
            }
            let deferred_actual = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            let deferred_referee = declared_domain_base_reference(program, required_referee);
            (deferred_actual != actual || deferred_referee != required_referee)
                && owned_referee_carrier_matches(
                    program,
                    deferred_actual,
                    deferred_referee,
                    substitutions,
                    required_substitutions,
                )
        }

        fn owned_referee_carrier_matches(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required_referee: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            substituted_identity(program, actual, substitutions)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required_referee)
                })
                || owned_array_projects_to_slice(
                    program,
                    actual,
                    required_referee,
                    substitutions,
                    required_substitutions,
                )
                || owned_array_satisfies_constrained_referee(
                    program,
                    actual,
                    required_referee,
                    substitutions,
                    required_substitutions,
                )
                || fixed_array_binds_open_length(
                    program,
                    actual,
                    required_referee,
                    substitutions,
                    required_substitutions,
                )
        }

        fn owned_array_projects_to_slice(
            program: &dyn crate::CheckingInput,
            mut actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::Slice {
                element_type: required_element,
            } = program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            actual = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            let TypeReferenceNode::FixedArray {
                element_type: actual_element,
                ..
            } = program.type_reference_table.type_reference(actual)
            else {
                return false;
            };
            program
                .type_reference_table
                .contains_type_reference(*actual_element)
                && program
                    .type_reference_table
                    .contains_type_reference(*required_element)
                && substituted_identity(program, *actual_element, substitutions)
                    == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(*required_element)
                    })
        }

        fn owned_array_satisfies_constrained_referee(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let mut required_carrier = required;
            let mut required_constraints = Vec::new();
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program
                .type_reference_table
                .type_reference(required_carrier)
            {
                required_constraints.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter(),
                );
                required_carrier = *base_type;
            }
            if required_constraints.is_empty() {
                return false;
            }
            let TypeReferenceNode::Slice {
                element_type: required_element,
            } = program
                .type_reference_table
                .type_reference(required_carrier)
            else {
                return false;
            };
            let mut actual_carrier = actual;
            let mut actual_domains = Vec::new();
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(actual_carrier)
            {
                actual_domains.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => Some(domain.symbol),
                            _ => None,
                        }),
                );
                actual_carrier = *base_type;
            }
            let TypeReferenceNode::FixedArray {
                element_type: actual_element,
                ..
            } = program.type_reference_table.type_reference(actual_carrier)
            else {
                return false;
            };
            if !program
                .type_reference_table
                .contains_type_reference(*actual_element)
                || !program
                    .type_reference_table
                    .contains_type_reference(*required_element)
                || substituted_identity(program, *actual_element, substitutions)
                    != program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(*required_element)
                    })
            {
                return false;
            }
            required_constraints
                .iter()
                .all(|constraint| match constraint {
                    TypeConstraintNode::Domain(target) => actual_domains.iter().any(|source| {
                        crate::field_domains::domain_membership_implies(
                            program,
                            *source,
                            target.symbol,
                        )
                    }),
                    _ => false,
                })
        }

        fn constrained_referee_forgets_qualification(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let peeled = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            let constrained = peeled != actual;
            constrained
                && substituted_identity(program, peeled, substitutions)
                    == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(required)
                    })
        }

        pub(crate) fn substituted_reference(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeReferenceHandle> {
            for _ in 0..=substitutions.len() {
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return Some(reference);
                };
                let Some((_, replacement)) = substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    return Some(reference);
                };
                reference = *replacement;
            }
            None
        }

        pub(crate) fn declared_value_type(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
        ) -> Option<TypeReferenceHandle> {
            selected_value_type(
                program,
                expression,
                substitutions,
                None,
                &mut ProjectionContext::default(),
            )
        }

        pub fn borrow_selects_reference_storage(
            program: &dyn crate::CheckingInput,
            source: ExpressionHandle,
            required: TypeReferenceHandle,
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            if let Some(place) = crate::canonical_place_from_expression(program, source)
                && place.segments.is_empty()
                && let Some(symbol) = place.root.as_symbol().filter(|symbol| symbol.is_valid())
                && let Some(state) = program.state_by_symbol(program.symbols.get(symbol).parent)
                && crate::value_custody::places::assignment_targets_parameter_referent(
                    program, state, source,
                )
            {
                return false;
            }
            let (required, required_substitutions) =
                substituted_required_reference(program, required, required_substitutions);
            let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(required)
            else {
                return false;
            };
            let mut substitutions = Vec::new();
            let Some(actual) = declared_value_type(program, source, &mut substitutions)
                .and_then(|actual| substituted_reference(program, actual, &substitutions))
            else {
                return false;
            };
            matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            ) && substituted_identity(program, actual, &substitutions)
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(*referee)
                })
        }

        pub(crate) fn owned_rvalue_exclusive_reference_access(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<ReferenceAccess> {
            if !projection_ends_at_owned_rvalue(program, expression) {
                return None;
            }
            let mut substitutions = Vec::new();
            let selected = selected_value_type(
                program,
                expression,
                &mut substitutions,
                None,
                &mut ProjectionContext::requiring(ReferenceAccess::Mutable),
            )?;
            let mut selected = substituted_reference(program, selected, &substitutions)?;
            selected = program
                .type_reference_table
                .unconstrained_type_reference(selected);
            declared_reference_access(program, selected).filter(|access| access.is_exclusive())
        }

        fn projection_ends_at_owned_rvalue(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> bool {
            loop {
                match expression_table(program).expression(expression) {
                    ExpressionNode::Member(member) => expression = member.receiver,
                    ExpressionNode::Indexed(indexed)
                        if !expression_table(program).indexed_on_range(indexed) =>
                    {
                        expression = indexed.collection;
                    }
                    ExpressionNode::Match(dispatch) => {
                        return expression_table(program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .all(|arm| projection_ends_at_owned_rvalue(program, arm.value));
                    }
                    ExpressionNode::Call(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::ArrayLiteral(_) => return true,
                    _ => return false,
                }
            }
        }

        pub(crate) fn place_forwarded_exclusive_reference_access(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            root: facts::PlaceRoot,
            root_type: TypeReferenceHandle,
        ) -> Option<ReferenceAccess> {
            let mut substitutions = Vec::new();
            let selected = selected_value_type(
                program,
                expression,
                &mut substitutions,
                Some((root, root_type)),
                &mut ProjectionContext::requiring(ReferenceAccess::Mutable),
            )?;
            let selected = substituted_reference(program, selected, &substitutions)?;
            declared_reference_access(program, selected).filter(|access| access.is_exclusive())
        }

        pub(crate) fn place_permits_reference_borrow(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
            root: facts::PlaceRoot,
            root_type: TypeReferenceHandle,
            owned_root_is_writable: bool,
            access: ReferenceAccess,
        ) -> bool {
            if let Some(indexed) = expression_table(program).indexed_node(expression)
                && matches!(
                    expression_table(program).expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                expression = indexed.collection;
            }
            let mut substitutions = Vec::new();
            let mut projection = ProjectionContext::requiring(access);
            let Some(selected) = selected_value_type(
                program,
                expression,
                &mut substitutions,
                Some((root, root_type)),
                &mut projection,
            ) else {
                return false;
            };
            let Some(mut selected) = substituted_reference(program, selected, &substitutions)
            else {
                return false;
            };
            selected = program
                .type_reference_table
                .unconstrained_type_reference(selected);
            match program.type_reference_table.type_reference(selected) {
                TypeReferenceNode::Reference {
                    access: source_access,
                    ..
                } => reference_access_permits(*source_access, access),
                _ => {
                    !access.is_exclusive() || owned_root_is_writable || projection.crossed_reference
                }
            }
        }

        fn reference_access_permits(source: ReferenceAccess, required: ReferenceAccess) -> bool {
            matches!(
                (source, required),
                (ReferenceAccess::Shared, ReferenceAccess::Shared)
                    | (
                        ReferenceAccess::Mutable,
                        ReferenceAccess::Shared
                            | ReferenceAccess::Mutable
                            | ReferenceAccess::WriteOnly
                    )
                    | (ReferenceAccess::WriteOnly, ReferenceAccess::WriteOnly)
            )
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum StorageWriteAccess {
            OwnedStorage,
            BorrowedStorage,
        }

        pub(crate) fn place_storage_write_access(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            root: facts::PlaceRoot,
            root_type: TypeReferenceHandle,
        ) -> Option<StorageWriteAccess> {
            place_storage_borrow_access(
                program,
                expression,
                root,
                root_type,
                ReferenceAccess::WriteOnly,
            )
        }

        pub(crate) fn place_storage_borrow_access(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            root: facts::PlaceRoot,
            root_type: TypeReferenceHandle,
            access: ReferenceAccess,
        ) -> Option<StorageWriteAccess> {
            let mut projection = ProjectionContext::requiring(access);
            selected_value_type(
                program,
                expression,
                &mut Vec::new(),
                Some((root, root_type)),
                &mut projection,
            )?;
            Some(if projection.crossed_reference {
                StorageWriteAccess::BorrowedStorage
            } else {
                StorageWriteAccess::OwnedStorage
            })
        }

        #[derive(Default)]
        pub(crate) struct ProjectionContext {
            owner_symbol: symbols::SymbolHandle,
            required: Option<ReferenceAccess>,
            crossed_reference: bool,
        }

        impl ProjectionContext {
            fn requiring(access: ReferenceAccess) -> Self {
                Self {
                    owner_symbol: symbols::SymbolHandle::invalid(),
                    required: Some(access),
                    crossed_reference: false,
                }
            }
        }

        fn projection_receiver_type(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            projection: &mut ProjectionContext,
        ) -> Option<TypeReferenceHandle> {
            for _ in 0..128 {
                reference = substituted_reference(program, reference, substitutions)?;
                reference = match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } => {
                        if projection
                            .required
                            .is_some_and(|required| !reference_access_permits(*access, required))
                        {
                            return None;
                        }
                        projection.crossed_reference = true;
                        *referee
                    }
                    _ => return Some(reference),
                };
            }
            None
        }

        fn is_builtin_indexed_shape(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> bool {
            loop {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
                        return true;
                    }
                    _ => return false,
                }
            }
        }

        fn selected_value_type(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            substitutions: &mut Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
            forwarding_root: Option<(facts::PlaceRoot, TypeReferenceHandle)>,
            projection: &mut ProjectionContext,
        ) -> Option<TypeReferenceHandle> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(name) => {
                    let [spelling] = expression_table(program).name_path_members(name.members)
                    else {
                        return None;
                    };
                    if !(name.symbol.is_valid() && name.symbol == name.head_symbol) {
                        return None;
                    }
                    if let Some((root, reference)) = forwarding_root {
                        return (root == facts::PlaceRoot::Symbol(name.symbol))
                            .then_some(reference);
                    }
                    (program.symbols.name(name.symbol) == spelling.as_str())
                        .then(|| named_value_type_reference(program, name))?
                }
                ExpressionNode::SavedSubject(saved) => {
                    let source = program.saved_value_source(saved)?;
                    let anchor = expression_table(program)
                        .value_capture(saved.capture)
                        .value_anchor;
                    if let Some((root, reference)) = forwarding_root {
                        return (root == facts::PlaceRoot::Expression(anchor)).then_some(reference);
                    }
                    declared_value_type(program, source, substitutions)
                }
                ExpressionNode::Call(_) if forwarding_root.is_none() => {
                    crate::machine_calls::calls::call_result_type(program, expression)
                }
                ExpressionNode::Match(dispatch) if forwarding_root.is_none() => {
                    let mut joined = None;
                    for arm in expression_table(program).match_arms(dispatch.arms) {
                        let mut arm_substitutions = substitutions.clone();
                        let arm_type = selected_value_type(
                            program,
                            arm.value,
                            &mut arm_substitutions,
                            None,
                            projection,
                        )?;
                        let arm_type =
                            substituted_reference(program, arm_type, &arm_substitutions)?;
                        match joined {
                            None => joined = Some(arm_type),
                            Some(existing)
                                if program
                                    .type_identity(TypeIdentityRequest::ordinary(existing))
                                    == program
                                        .type_identity(TypeIdentityRequest::ordinary(arm_type)) => {
                            }
                            Some(_) => return None,
                        }
                    }
                    joined
                }
                ExpressionNode::Member(member) => {
                    if forwarding_root.is_none()
                        && let ExpressionNode::Name(root) =
                            expression_table(program).expression(member.receiver)
                        && root.head_symbol.is_valid()
                        && let Some(machine) = program.machines_with_symbol(root.head_symbol).next()
                    {
                        return crate::value_custody::places::exact_self_field(
                            program, machine, expression,
                        )
                        .map(|field| field.type_reference);
                    }
                    let receiver = selected_value_type(
                        program,
                        member.receiver,
                        substitutions,
                        forwarding_root,
                        projection,
                    )?;
                    let receiver =
                        projection_receiver_type(program, receiver, substitutions, projection)?;
                    if program.data_field_definition(receiver).is_some() {
                        return crate::value_custody::places::applied_data_member_field(
                            program, receiver, member,
                        )
                        .map(|field| field.type_reference);
                    }
                    let (symbol, arguments) =
                        match program.type_reference_table.type_reference(receiver) {
                            TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..]),
                            TypeReferenceNode::Generic {
                                base_symbol,
                                arguments,
                                ..
                            } => (
                                *base_symbol,
                                program
                                    .type_reference_table
                                    .type_reference_handles(*arguments),
                            ),
                            _ => return None,
                        };
                    let mut definitions = program
                        .data_definitions()
                        .iter()
                        .filter(|definition| symbol.is_valid() && definition.symbol == symbol);
                    let definition = definitions.next()?;
                    if definitions.next().is_some()
                        || program.data_type_parameters(definition).len() != arguments.len()
                    {
                        return None;
                    }
                    for (parameter, argument) in program
                        .data_type_parameters(definition)
                        .iter()
                        .zip(arguments)
                    {
                        if let Some((_, existing)) = substitutions
                            .iter()
                            .find(|(symbol, _)| *symbol == parameter.symbol)
                        {
                            if substituted_identity(program, *existing, substitutions)
                                != substituted_identity(program, *argument, substitutions)
                            {
                                return None;
                            }
                        } else {
                            substitutions.push((parameter.symbol, *argument));
                        }
                    }
                    let field = crate::value_custody::places::exact_data_member_field(
                        program,
                        definition,
                        member.member_symbol,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|variant| variant.as_str()),
                    );
                    let field = match (field, forwarding_root) {
                        (Some(field), _) => Some(field),
                        (None, None) => None,
                        (None, Some(_)) => crate::value_custody::places::exact_data_member_field(
                            program,
                            definition,
                            symbols::SymbolHandle::invalid(),
                            member.member.as_str(),
                            member.case_variant.as_ref().map(|variant| variant.as_str()),
                        ),
                    };
                    field.map(|field| field.type_reference)
                }
                ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
                    if forwarding_root.is_none() =>
                {
                    let reference =
                        expression_table(program).constructed_value_type_reference(expression);
                    reference
                        .is_valid()
                        .then_some(reference)
                        .or_else(|| crate::declared_constant_array_type(program, expression))
                }
                ExpressionNode::Indexed(indexed) => {
                    if expression_table(program).indexed_on_range(indexed) {
                        return None;
                    }
                    let collection = selected_value_type(
                        program,
                        indexed.collection,
                        substitutions,
                        forwarding_root,
                        projection,
                    )?;
                    if !substitutions.is_empty()
                        && !is_builtin_indexed_shape(program, collection)
                        && program.normalized_type_identity(collection)
                            != substituted_identity(program, collection, substitutions)
                    {
                        return None;
                    }
                    let index_type = declared_value_type(program, indexed.index, &mut Vec::new());
                    let owner = projection
                        .owner_symbol
                        .is_valid()
                        .then_some(projection.owner_symbol)
                        .or_else(|| place_machine_symbol(program, indexed.collection));
                    let operands = [Some(collection), index_type];
                    let meaning_is_builtin = if let Some(machine) = owner {
                        typed_trees::operator::builtin_index_read_is_spelled(
                            program, machine, expression, &operands,
                        )
                    } else {
                        crate::value_custody::places::has_retained_builtin_index_meaning(
                            program, expression,
                        ) && typed_trees::operator::resolve_indexed_spelling_for_operands(
                            program,
                            language_core::OperatorSpelling::Index,
                            &operands,
                            expression_table(program).source_span(expression),
                        )
                        .is_empty()
                    };
                    if !meaning_is_builtin {
                        return None;
                    }
                    let collection =
                        projection_receiver_type(program, collection, substitutions, projection)?;
                    crate::value_custody::places::collection_element_type(program, collection)
                }
                _ => None,
            }
        }

        fn place_machine_symbol(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            for _ in 0..128 {
                match expression_table(program).expression(expression) {
                    ExpressionNode::Member(member) => expression = member.receiver,
                    ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                    ExpressionNode::Name(path)
                        if path.symbol.is_valid() && path.symbol == path.head_symbol =>
                    {
                        let mut symbol = path.symbol;
                        for _ in 0..128 {
                            if program.machine(symbol).is_some() {
                                return Some(symbol);
                            }
                            let parent = program.symbols.get(symbol).parent;
                            if !parent.is_valid() || parent == symbol {
                                return None;
                            }
                            symbol = parent;
                        }
                        return None;
                    }
                    _ => return None,
                }
            }
            None
        }

        pub(crate) fn substituted_required_reference<'bindings>(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            substitutions: &'bindings [(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> (
            TypeReferenceHandle,
            &'bindings [(symbols::SymbolHandle, TypeReferenceHandle)],
        ) {
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
            {
                (*actual, &[])
            } else {
                (reference, substitutions)
            }
        }
    }
    pub(crate) mod result_type {
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::bound_expression_meaning::builtin_type_reference;
        use language_core::OperatorSpelling;
        use numerics::arithmetic::ArithmeticDomain;
        use symbols::BuiltinTypeAtom;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
        };
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{
            PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };

        pub fn expression_result_type_reference(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            result_type(
                program,
                ExpressionOwner::State { machine, state },
                expression,
                &mut Vec::new(),
            )
        }

        pub fn parameter_expression_result_type_reference(
            program: &dyn crate::CheckingInput,
            owner_symbol: symbols::SymbolHandle,
            parameters: &[typed_trees::signature::StateParameter],
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            parameter_expression_result_type_reference_in_scope(
                program,
                owner_symbol,
                parameters,
                program
                    .closed_requirement_application(owner_symbol)
                    .map(|application| application.trait_scope),
                expression,
            )
        }

        /// A copied callable retains its supplying application as well as its fresh
        /// formal roster. This selects principal results of siblings, including
        /// nullary operations; no destination or operand-count inference supplies it.
        pub fn parameter_expression_result_type_reference_in_scope(
            program: &dyn crate::CheckingInput,
            owner_symbol: symbols::SymbolHandle,
            parameters: &[typed_trees::signature::StateParameter],
            trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if !owner_symbol.is_valid() {
                return result_type(
                    program,
                    ExpressionOwner::Declaration {
                        owner_symbol,
                        trait_scope,
                        place_type: &|_| None,
                    },
                    expression,
                    &mut Vec::new(),
                );
            }
            let telescope =
                crate::frozen_program::frozen_program_memos(program).and_then(|memos| {
                    let roster = parameters
                        .iter()
                        .map(|parameter| (parameter.symbol, parameter.type_reference))
                        .collect::<Vec<_>>();
                    let mut telescopes = memos.parameter_telescopes.lock().ok()?;
                    let next = u32::try_from(telescopes.len()).ok()?;
                    Some(
                        *telescopes
                            .entry((owner_symbol, roster, trait_scope))
                            .or_insert(next),
                    )
                });
            let Some(telescope) = telescope else {
                let place_type = |expression| {
                    telescope_place_type(program, owner_symbol, parameters, expression)
                };
                return result_type(
                    program,
                    ExpressionOwner::Declaration {
                        owner_symbol,
                        trait_scope,
                        place_type: &place_type,
                    },
                    expression,
                    &mut Vec::new(),
                );
            };
            result_type(
                program,
                ExpressionOwner::Telescope {
                    owner_symbol,
                    parameters,
                    trait_scope,
                    telescope,
                },
                expression,
                &mut Vec::new(),
            )
        }

        fn telescope_place_type(
            program: &dyn crate::CheckingInput,
            owner_symbol: symbols::SymbolHandle,
            parameters: &[typed_trees::signature::StateParameter],
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            crate::reserved_result_place(program, expression)
                .filter(|place| place.machine_symbol == owner_symbol)
                .map(|place| place.type_reference)
                .or_else(|| {
                    crate::value_custody::places::parameter_scoped_type_reference(
                        program,
                        owner_symbol,
                        parameters,
                        expression,
                    )
                })
                .or_else(|| {
                    let symbol = crate::queries::expression_traversal::single_segment_name_symbol(
                        program, expression,
                    )?;
                    if let Some(reference) =
                        crate::value_custody::places::declaration_type_reference(program, symbol)
                    {
                        return Some(reference);
                    }
                    let parameter = program.data_type_parameter_by_symbol(symbol)?;
                    match parameter.kind {
                        typed_trees::data::TypeParameterKind::Const { type_reference }
                        | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                            Some(type_reference)
                        }
                        _ => None,
                    }
                })
        }

        fn constructor_result_type_reference(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            let node = expression_table(program).expression(expression);
            if !matches!(
                node,
                ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
            ) {
                return None;
            }
            let reference = expression_table(program).constructed_value_type_reference(expression);
            reference
                .is_valid()
                .then_some(reference)
                .or_else(|| match node {
                    ExpressionNode::StructLiteral(literal) if literal.type_symbol.is_valid() => {
                        program
                            .type_reference_table
                            .find_named_type_reference(literal.type_symbol)
                    }
                    ExpressionNode::ArrayLiteral(_) => {
                        crate::declared_constant_array_type(program, expression)
                    }
                    _ => None,
                })
        }

        pub(crate) fn declaration_expression_result_type_reference(
            program: &dyn crate::CheckingInput,
            owner_symbol: symbols::SymbolHandle,
            trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
            place_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if !owner_symbol.is_valid() {
                return None;
            }
            result_type(
                program,
                ExpressionOwner::Declaration {
                    owner_symbol,
                    trait_scope,
                    place_type,
                },
                expression,
                &mut Vec::new(),
            )
        }

        pub(crate) fn domain_expression_result_type_reference(
            program: &dyn crate::CheckingInput,
            domain: &typed_trees::domain::DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            result_type(
                program,
                ExpressionOwner::Domain(domain),
                expression,
                &mut Vec::new(),
            )
        }

        #[derive(Clone, Copy)]
        pub(crate) enum ExpressionOwner<'program> {
            State {
                machine: &'program Machine,
                state: &'program State,
            },
            Domain(&'program typed_trees::domain::DomainDefinition),
            Declaration {
                owner_symbol: symbols::SymbolHandle,
                trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
                place_type: &'program dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
            },
            Telescope {
                owner_symbol: symbols::SymbolHandle,
                parameters: &'program [typed_trees::signature::StateParameter],
                trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
                telescope: u32,
            },
        }

        pub(crate) enum ResultTypeMemoKey {
            State((u32, u32, ExpressionHandle)),
            Telescope((u32, ExpressionHandle)),
        }

        impl ExpressionOwner<'_> {
            fn symbol(self) -> symbols::SymbolHandle {
                match self {
                    Self::State { machine, .. } => machine.symbol,
                    Self::Domain(domain) => domain.symbol,
                    Self::Declaration { owner_symbol, .. }
                    | Self::Telescope { owner_symbol, .. } => owner_symbol,
                }
            }
        }

        fn result_type(
            program: &dyn crate::CheckingInput,
            owner: ExpressionOwner<'_>,
            expression: ExpressionHandle,
            active: &mut Vec<ExpressionHandle>,
        ) -> Option<TypeReferenceHandle> {
            if !program.expression_table.expression_is_valid(expression)
                || active.contains(&expression)
            {
                return None;
            }
            let memo = crate::frozen_program::frozen_program_memos(program).and_then(|memos| {
                let key = match owner {
                    ExpressionOwner::State { machine, state } => ResultTypeMemoKey::State(
                        crate::proof_contracts::bound_expression_meaning::meaning_key(
                            program,
                            machine,
                            Some(state),
                            expression,
                        )?,
                    ),
                    ExpressionOwner::Telescope { telescope, .. } => {
                        ResultTypeMemoKey::Telescope((telescope, expression))
                    }
                    _ => return None,
                };
                Some((memos, key))
            });
            let cached = memo.as_ref().and_then(|(memos, key)| match key {
                ResultTypeMemoKey::State(key) => {
                    memos.expression_result_types.read().ok()?.get(key).copied()
                }
                ResultTypeMemoKey::Telescope(key) => {
                    memos.parameter_result_types.read().ok()?.get(key).copied()
                }
            });
            if let Some(cached) = cached {
                return cached;
            }
            if let Some(declaration) = program.expression_const_declaration(expression) {
                return Some(declaration.declared_type);
            }
            if let ExpressionNode::Name(path) = expression_table(program).expression(expression)
                && let Some(reference) = super::builtin_member_type_reference(program, path.symbol)
            {
                return Some(reference);
            }
            active.push(expression);
            let result = match expression_table(program).expression(expression) {
                ExpressionNode::SavedSubject(saved) => program
                    .saved_value_source(saved)
                    .and_then(|subject| result_type(program, owner, subject, active)),
                ExpressionNode::Atomic(atomic) => {
                    let operand = result_type(program, owner, atomic.value, active);
                    if crate::atomic_load_carrier(program, expression).is_some() {
                        operand
                            .and_then(|cell| crate::atomic_resident_type_reference(program, cell))
                    } else {
                        operand
                    }
                }
                ExpressionNode::Match(dispatch) => {
                    let arms = expression_table(program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .filter(|arm| !program.static_type_context_is_unreachable(arm.value))
                        .collect::<Vec<_>>();
                    let references = arms
                        .iter()
                        .map(|arm| result_type(program, owner, arm.value, active))
                        .collect::<Vec<_>>();
                    join_result_type_references(
                        program,
                        &references,
                        &arms
                            .iter()
                            .zip(&references)
                            .map(|(arm, reference)| {
                                reference.is_none()
                                    && crate::value_custody::literals::has_anonymous_numeric_results(
                                        program, arm.value,
                                    )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .or_else(|| {
                        let values = arms
                            .iter()
                            .zip(&references)
                            .map(|(arm, reference)| reference.map(|reference| (arm.value, reference)))
                            .collect::<Option<Vec<_>>>()?;
                        program.common_static_type_reference(&values)
                    })
                }
                ExpressionNode::Call(call) => (|| {
                    if crate::compiler_operations::storage_address_operand(program, call).is_some()
                    {
                        return builtin_type_reference(program, BuiltinTypeAtom::Address);
                    }
                    if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call)
                    {
                        return crate::proof_contracts::proof_embeddings::expression_type_reference(
                            program, expression,
                        );
                    }
                    if let Some(reference) =
                        crate::proof_contracts::proof_embeddings::embedding_result_type_reference(
                            program, call,
                        )
                    {
                        return Some(reference);
                    }
                    if typed_trees::byte_predicates::byte_predicate_call(program, expression)
                        .is_some()
                    {
                        return builtin_type_reference(program, BuiltinTypeAtom::Bool);
                    }
                    if let Some(reference) =
                        crate::value_custody::content_conservation::intrinsic_result_type_reference(
                            program,
                            call,
                            |argument| result_type(program, owner, argument, active),
                        )
                    {
                        return Some(reference);
                    }
                    if let Some(reference) =
                        crate::mathematical_applications::result_type_reference(program, call)
                    {
                        return Some(reference);
                    }
                    let selected_operator =
                        typed_trees::operator::resolve_named_expression_call(program, call)
                            .or_else(|| {
                                crate::machine_calls::calls::callee_entry_contract(
                                    program,
                                    call.target_symbol,
                                )
                                .and_then(|contract| {
                                    typed_trees::operator::declaration_by_symbol(
                                        program,
                                        contract.callable,
                                    )
                                })
                            });
                    // Inside an abstract trait contract, a sibling requirement's
                    // generic result already belongs to this exact telescope. Runtime
                    // callers cannot borrow that raw result: they need its applied
                    // signature. Keep this owner distinction instead of making the
                    // context-free call query guess a generic instantiation.
                    let trait_scope = match owner {
                        ExpressionOwner::Declaration { trait_scope, .. }
                        | ExpressionOwner::Telescope { trait_scope, .. } => trait_scope,
                        _ => None,
                    };
                    let scoped_requirement = match trait_scope {
                        Some(scope) => {
                            program.requirement_call_signature_in_scope(owner.symbol(), call, scope)
                        }
                        None => program.requirement_call_signature(owner.symbol(), call),
                    };
                    let scoped_requirement_result = scoped_requirement
                        .filter(|signature| signature.return_type.is_valid())
                        .map(|signature| signature.return_type)
                        .or_else(|| match owner {
                            ExpressionOwner::Declaration { owner_symbol, .. }
                            | ExpressionOwner::Telescope { owner_symbol, .. }
                                if trait_scope.is_none() && call.target_symbol.is_valid() =>
                            {
                                let parent = program.symbols.get(owner_symbol).parent;
                                (program.symbols.get(call.target_symbol).parent == parent)
                                    .then(|| program.trait_definition_by_symbol(parent))
                                    .flatten()
                                    .and_then(|definition| {
                                        program.trait_machine_signature_by_symbol(
                                            definition,
                                            call.target_symbol,
                                        )
                                    })
                                    .filter(|signature| {
                                        signature.type_parameters.is_empty()
                                            && signature.return_type.is_valid()
                                    })
                                    .map(|signature| signature.return_type)
                            }
                            _ => None,
                        });
                    let retained =
                        expression_table(program).constructed_value_type_reference(expression);
                    let declared_result = retained
                        .is_valid()
                        .then_some(retained)
                        .or(scoped_requirement_result)
                        .or_else(|| {
                            crate::machine_calls::calls::resolved_call_result_type(program, call)
                        });
                    if let Some(result) = declared_result
                && (call.exact_coercion_target.is_valid()
                    || selected_operator.is_none_or(|operator| !crate::machine_calls::calls::selected_float_operation_retains_argument_policy(program, operator.symbol)))
            { return Some(result); }
                    let operands: Vec<_> = expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| result_type(program, owner, *argument, active))
                        .collect();
                    let selected_operator = selected_operator.or_else(|| {
                let references = operands.iter().copied().collect::<Option<Vec<_>>>()?;
                let (symbol, _, _) = crate::proof_contracts::float_projection_bindings::semantic_operations::float_semantic_application(program, call, &references)?;
                typed_trees::operator::declaration_by_symbol(program, symbol)
            });
                    declared_result
                        .or_else(|| {
                            selected_operator.and_then(|operator| {
                                bound_result_type_parameter(program, operator, &operands).or_else(
                                    || {
                                        (!result_mentions_type_parameters(
                                            program,
                                            operator,
                                            operator.return_type,
                                        ))
                                        .then_some(operator.return_type)
                                    },
                                )
                            })
                        })
                        .and_then(|result| {
                            if call.exact_coercion_target.is_valid() {
                                return Some(result);
                            }
                            let Some(operator) = selected_operator else {
                                return Some(result);
                            };
                            selected_operator_result_type_reference(
                                program,
                                operator.symbol,
                                result,
                                &operands,
                            )
                        })
                })(),
                ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                    constructor_result_type_reference(program, expression)
                }
                ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                    let reference =
                        expression_table(program).constructed_value_type_reference(expression);
                    reference.is_valid().then_some(reference)
                }
                ExpressionNode::Cast(cast) => cast_result_type_reference(program, cast),
                ExpressionNode::Indexed(indexed)
                    if matches!(
                        expression_table(program).expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) =>
                {
                    (|| {
                        let range = expression_table(program).range_node(indexed.index)?;
                        if range.end_inclusive {
                            return None;
                        }
                        let collection = result_type(program, owner, indexed.collection, active)?;
                        let operands = [
                            Some(collection),
                            range
                                .start
                                .is_valid()
                                .then(|| result_type(program, owner, range.start, active))
                                .flatten(),
                            range
                                .end
                                .is_valid()
                                .then(|| result_type(program, owner, range.end, active))
                                .flatten(),
                        ];
                        if !typed_trees::operator::resolve_indexed_spelling_for_operands(
                            program,
                            OperatorSpelling::Range,
                            &operands,
                            expression_table(program).source_span(expression),
                        )
                        .is_empty()
                            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                                program,
                                owner.symbol(),
                                expression,
                                OperatorSpelling::Range,
                                &operands,
                            )
                        {
                            return None;
                        }
                        let carrier = program
                            .type_reference_table
                            .unconstrained_type_reference(collection);
                        let (access, carrier) =
                            match program.type_reference_table.type_reference(carrier) {
                                TypeReferenceNode::Reference {
                                    access, referee, ..
                                } => (*access, *referee),
                                _ => (language_core::ReferenceAccess::Shared, carrier),
                            };
                        if access == language_core::ReferenceAccess::WriteOnly {
                            return None;
                        }
                        let carrier = program
                            .type_reference_table
                            .unconstrained_type_reference(carrier);
                        let element = match program.type_reference_table.type_reference(carrier) {
                            TypeReferenceNode::Slice { element_type }
                            | TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
                            _ => return None,
                        };
                        let identity = program.normalized_type_identity(element);
                        program
                            .type_reference_table
                            .slice_view_references()
                            .find_map(|(reference, candidate_access, candidate_element)| {
                                (candidate_access == access
                                    && program.normalized_type_identity(candidate_element)
                                        == identity)
                                    .then_some(reference)
                            })
                    })()
                }
                ExpressionNode::Member(member) => {
                    result_type(program, owner, member.receiver, active).and_then(|receiver| {
                        if crate::collection_operations::collection_measure_member(
                            program,
                            member,
                            crate::collection_operations::MeasureReceiver::Declared(receiver),
                        )
                        .is_some()
                            || crate::collection_operations::core_vector_measure_member(
                                program, member, receiver,
                            )
                        {
                            return builtin_type_reference(program, BuiltinTypeAtom::U64);
                        }
                        let field = crate::value_custody::places::applied_data_member_field(
                            program, receiver, member,
                        )?;
                        crate::project_type_reference_from_segments(
                            program,
                            receiver,
                            &[facts::PlaceSegment::Field {
                                symbol: field.symbol,
                            }],
                        )
                    })
                }
                ExpressionNode::Indexed(indexed) => (|| {
                    let operands = [indexed.collection, indexed.index]
                        .map(|operand| result_type(program, owner, operand, active));
                    let candidates = typed_trees::operator::resolve_indexed_spelling_for_operands(
                        program,
                        OperatorSpelling::Index,
                        &operands,
                        expression_table(program).source_span(expression),
                    );
                    if candidates.is_empty()
                        && typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            owner.symbol(),
                            expression,
                            OperatorSpelling::Index,
                            &operands,
                        )
                    {
                        operands[0].and_then(|collection| {
                            crate::value_custody::places::collection_element_type_reference(
                                program, collection,
                            )
                        })
                    } else {
                        let [selected] = candidates.as_slice() else {
                            return None;
                        };
                        if !typed_trees::operator::selected_trait_operator_meanings(
                            program,
                            owner.symbol(),
                            OperatorSpelling::Index,
                            &operands,
                        )
                        .is_empty()
                        {
                            return None;
                        }
                        bound_result_type_parameter(program, selected.operator, &operands).or_else(
                            || {
                                (!result_mentions_type_parameters(
                                    program,
                                    selected.operator,
                                    selected.operator.return_type,
                                ))
                                .then_some(selected.operator.return_type)
                            },
                        )
                    }
                })(),
                ExpressionNode::ZeroValue(reference) => Some(*reference),
                ExpressionNode::Borrow(_) => {
                    let reference =
                        expression_table(program).constructed_value_type_reference(expression);
                    reference.is_valid().then_some(reference)
                }
                ExpressionNode::Integer(literal) => {
                    let reference =
                        crate::declarations::operators::landed_integer_literal_type_reference(
                            program, expression,
                        );
                    reference.and_then(|reference| {
                        let domain = literal.landing()?.domain;
                        if domain == ArithmeticDomain::Exact {
                            Some(reference)
                        } else {
                            program
                                .type_reference_table
                                .find_arithmetic_result_type_reference(
                                    program.type_reference_table.type_symbol(reference),
                                    domain,
                                )
                        }
                    })
                }
                ExpressionNode::Boolean(_) => {
                    builtin_type_reference(program, BuiltinTypeAtom::Bool)
                }
                ExpressionNode::UndeterminedNaN(format) => builtin_type_reference(
                    program,
                    match format {
                        numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                        numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
                    },
                ),
                ExpressionNode::Float(literal) => literal.landing().and_then(|format| {
                    builtin_type_reference(
                        program,
                        match format {
                            numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                            numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
                        },
                    )
                }),
                ExpressionNode::Binary(binary) => {
                    if program.static_type_predicate_operands(expression).is_some() {
                        builtin_type_reference(program, BuiltinTypeAtom::Bool)
                    } else {
                        let operands = [binary.left, binary.right]
                            .map(|operand| result_type(program, owner, operand, active));
                        binary_result(program, owner, expression, binary, operands, active)
                    }
                }
                ExpressionNode::Unary(unary) => {
                    let operand = result_type(program, owner, unary.operand, active);
                    match unary.operator {
                        UnaryOperator::BitwiseNot => operand
                            .and_then(|reference| {
                                arithmetic_result_type_reference(program, reference)
                            })
                            .filter(|reference| integer(program, *reference)),
                        UnaryOperator::LogicalNot => operand
                            .filter(|reference| {
                                primitive(program, *reference) == Some(PrimitiveType::Bool)
                            })
                            .and_then(|_| builtin_type_reference(program, BuiltinTypeAtom::Bool)),
                    }
                }
                _ => {
                    match owner {
                        ExpressionOwner::Declaration { place_type, .. } => place_type(expression),
                        ExpressionOwner::Telescope {
                            owner_symbol,
                            parameters,
                            ..
                        } => telescope_place_type(program, owner_symbol, parameters, expression),
                        ExpressionOwner::State { machine, state } => {
                            crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                expression,
                            )
                        }
                        ExpressionOwner::Domain(domain) => {
                            match expression_table(program).expression(expression) {
                            ExpressionNode::Name(_)
                                if typed_trees::domain::domain_subject_reference(
                                    program, domain, expression,
                                ) =>
                            {
                                Some(domain.target_type)
                            }
                            _ => crate::proof_contracts::proof_embeddings::expression_type_reference(
                                program, expression,
                            ),
                        }
                        }
                    }
                }
            };
            let result = result
                .map(|reference| program.statically_refined_type_reference(expression, reference));
            active.pop();
            let result = result.filter(|reference| {
                program
                    .type_reference_table
                    .contains_type_reference(*reference)
            });
            if let Some((memos, key)) = memo
                && (result.is_some() || active.is_empty())
            {
                match key {
                    ResultTypeMemoKey::State(key) => {
                        if let Ok(mut types) = memos.expression_result_types.write() {
                            types.insert(key, result);
                        }
                    }
                    ResultTypeMemoKey::Telescope(key) => {
                        if let Ok(mut types) = memos.parameter_result_types.write() {
                            types.insert(key, result);
                        }
                    }
                }
            }
            result
        }

        pub fn join_result_type_references(
            program: &dyn crate::CheckingInput,
            references: &[Option<TypeReferenceHandle>],
            anonymous: &[bool],
        ) -> Option<TypeReferenceHandle> {
            if references.len() != anonymous.len() {
                return None;
            }
            references
                .iter()
                .copied()
                .flatten()
                .next()
                .and_then(|first| {
                    if references.iter().all(|reference| {
                        reference.is_some_and(|reference| {
                            same_result_type(program, first, reference, &mut Vec::new())
                        })
                    }) {
                        return Some(first);
                    }
                    if has_domain_result_shell(program, first) {
                        let identity = program.normalized_type_identity(first);
                        if references.iter().all(|reference| {
                            reference.is_some_and(|reference| {
                                has_domain_result_shell(program, reference)
                                    && program.normalized_type_identity(reference) == identity
                            })
                        }) {
                            return Some(first);
                        }
                    }
                    let carrier = arithmetic_carrier(program, first)?;
                    anonymous
                        .iter()
                        .zip(references)
                        .all(|(anonymous, reference)| {
                            reference.map_or_else(
                                || *anonymous,
                                |reference| arithmetic_carrier(program, reference) == Some(carrier),
                            )
                        })
                        .then(|| arithmetic_result_type_reference(program, first))
                        .flatten()
                })
        }

        fn same_result_type(
            program: &dyn crate::CheckingInput,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
            active: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        ) -> bool {
            if left == right {
                return true;
            }
            if active.contains(&(left, right)) {
                return false;
            }
            let left_type = program.type_reference_table.type_reference(left);
            let right_type = program.type_reference_table.type_reference(right);
            if left_type == right_type {
                return true;
            }
            active.push((left, right));
            let same = match (left_type, right_type) {
                (
                    TypeReferenceNode::FixedArray {
                        element_type: left_element,
                        length: left_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: right_element,
                        length: right_length,
                    },
                ) => {
                    left_length == right_length
                        && same_result_type(program, *left_element, *right_element, active)
                }
                (
                    TypeReferenceNode::Slice {
                        element_type: left_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: right_element,
                    },
                ) => same_result_type(program, *left_element, *right_element, active),
                (
                    TypeReferenceNode::Reference {
                        referee: left_referee,
                        access: left_access,
                        lifetime: left_lifetime,
                    },
                    TypeReferenceNode::Reference {
                        referee: right_referee,
                        access: right_access,
                        lifetime: right_lifetime,
                    },
                ) => {
                    left_access == right_access
                        && left_lifetime == right_lifetime
                        && same_result_type(program, *left_referee, *right_referee, active)
                }
                (
                    TypeReferenceNode::Constrained {
                        base_type: left_base,
                        constraints: left_constraints,
                    },
                    TypeReferenceNode::Constrained {
                        base_type: right_base,
                        constraints: right_constraints,
                    },
                ) => {
                    left_constraints == right_constraints
                        && same_result_type(program, *left_base, *right_base, active)
                }
                (
                    TypeReferenceNode::Generic {
                        base_symbol: left_symbol,
                        lifetime_arguments: left_lifetimes,
                        arguments: left_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: right_symbol,
                        lifetime_arguments: right_lifetimes,
                        arguments: right_arguments,
                        ..
                    },
                ) if left_symbol == right_symbol && left_lifetimes == right_lifetimes => {
                    same_result_arguments(program, *left_arguments, *right_arguments, active)
                }
                (
                    TypeReferenceNode::DynamicTrait {
                        symbol: left_symbol,
                        arguments: left_arguments,
                        ..
                    },
                    TypeReferenceNode::DynamicTrait {
                        symbol: right_symbol,
                        arguments: right_arguments,
                        ..
                    },
                ) if left_symbol == right_symbol => {
                    same_result_arguments(program, *left_arguments, *right_arguments, active)
                }
                _ => false,
            };
            active.pop();
            same
        }

        fn same_result_arguments(
            program: &dyn crate::CheckingInput,
            left: arena::HandleSpan<TypeReferenceHandle>,
            right: arena::HandleSpan<TypeReferenceHandle>,
            active: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        ) -> bool {
            let left_arguments = program.type_reference_table.type_reference_handles(left);
            let right_arguments = program.type_reference_table.type_reference_handles(right);
            left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| same_result_type(program, *left, *right, active))
        }

        fn has_domain_result_shell(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> bool {
            let mut seen = Vec::new();
            let mut domain = false;
            while program
                .type_reference_table
                .contains_type_reference(reference)
                && !seen.contains(&reference)
            {
                seen.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        let Some(constraints) =
                            program.type_reference_table.constraint_span(*constraints)
                        else {
                            return false;
                        };
                        for constraint in constraints {
                            match constraint {
                                TypeConstraintNode::Domain(qualification)
                                    if qualification.symbol.is_valid() =>
                                {
                                    domain = true
                                }
                                TypeConstraintNode::ArithmeticDomain(_) => {}
                                _ => return false,
                            }
                        }
                        reference = *base_type;
                    }
                    TypeReferenceNode::Named { .. } => return domain,
                    _ => return false,
                }
            }
            false
        }

        fn binary_result(
            program: &dyn crate::CheckingInput,
            owner: ExpressionOwner<'_>,
            expression: ExpressionHandle,
            binary: &TableBinaryExpression,
            operands: [Option<TypeReferenceHandle>; 2],
            active: &mut Vec<ExpressionHandle>,
        ) -> Option<TypeReferenceHandle> {
            use BinaryOperator::*;
            let scoped_membership = matches!(owner, ExpressionOwner::State { machine, state }
                if crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                    binary,
                )
            );
            if scoped_membership
        || crate::proof_contracts::bound_expression_meaning::has_exact_typed_case_membership_meaning(
            program,
            expression,
            binary,
            operands[0],
        )
            {
        return builtin_type_reference(program, BuiltinTypeAtom::Bool);
            }
            let spelling = match binary.operator {
                Add => Some(OperatorSpelling::Add),
                Subtract => Some(OperatorSpelling::Subtract),
                Multiply => Some(OperatorSpelling::Multiply),
                Divide => Some(OperatorSpelling::Divide),
                Modulo => Some(OperatorSpelling::Modulo),
                Equal => Some(OperatorSpelling::Equal),
                NotEqual => Some(OperatorSpelling::NotEqual),
                Less => Some(OperatorSpelling::Less),
                LessOrEqual => Some(OperatorSpelling::LessEqual),
                Greater => Some(OperatorSpelling::Greater),
                GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
                And | Or | BitwiseAnd | BitwiseOr | BitwiseXor | ShiftLeft | ShiftRight
                | CaseMembership => None,
            };
            if let Some(spelling) = spelling
                && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner.symbol(),
                    expression,
                    spelling,
                    &operands,
                )
            {
                if let [meaning] = typed_trees::operator::bound_trait_operator_meanings(
                    program,
                    owner.symbol(),
                    spelling,
                    &operands,
                )
                .as_slice()
                {
                    return applied_requirement_result(
                        program,
                        meaning.requirement.return_type,
                        &meaning.substitutions,
                    );
                }
                if let [meaning] = typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    owner.symbol(),
                    spelling,
                    &operands,
                )
                .as_slice()
                {
                    let bindings = typed_trees::operator::trait_operator_operand_bindings(
                        program,
                        meaning.trait_definition,
                        meaning.requirement,
                        &operands,
                        None,
                    )?;
                    return applied_requirement_result(
                        program,
                        meaning.requirement.return_type,
                        &bindings.types,
                    );
                }
                let candidates = typed_trees::operator::resolve_spelling_for_operands(
                    program,
                    spelling,
                    &operands,
                    expression_table(program).source_span(expression),
                );
                let [selected] = candidates.as_slice() else {
                    return None;
                };
                if !typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    owner.symbol(),
                    spelling,
                    &operands,
                )
                .is_empty()
                {
                    return None;
                }
                if selected.domain.is_none_or(|domain| {
                    operands.iter().flatten().any(|reference| {
                        typed_trees::domain::type_selects_semantic_domain(
                            program,
                            *reference,
                            domain.symbol,
                        )
                    })
                }) {
                    let result = selected.operator.return_type;
                    if let Some(bound) =
                        bound_result_type_parameter(program, selected.operator, &operands)
                    {
                        return Some(bound);
                    }
                    qualified_builtin_carrier(program, result)?;
                    return selected_operator_result_type_reference(
                        program,
                        selected.operator.symbol,
                        result,
                        &operands,
                    );
                }
            }
            builtin_binary_result_type_reference(program, binary, operands, &mut |operand| {
                result_type(program, owner, operand, active)
            })
        }

        pub(crate) enum BuiltinBinaryResult {
            Boolean,
            ProofInteger,
            AddressDistance,
            Arithmetic {
                symbol: symbols::SymbolHandle,
                policy: ArithmeticDomain,
            },
            Schematic(TypeReferenceHandle),
        }

        pub(crate) fn builtin_binary_result_type_reference(
            program: &dyn crate::CheckingInput,
            binary: &TableBinaryExpression,
            operands: [Option<TypeReferenceHandle>; 2],
            expression_type: &mut dyn FnMut(ExpressionHandle) -> Option<TypeReferenceHandle>,
        ) -> Option<TypeReferenceHandle> {
            match builtin_binary_result(program, binary, operands, expression_type)? {
                BuiltinBinaryResult::Boolean => {
                    builtin_type_reference(program, BuiltinTypeAtom::Bool)
                }
                BuiltinBinaryResult::ProofInteger => {
                    crate::proof_contracts::proof_embeddings::proof_int_type_reference(program)
                }
                BuiltinBinaryResult::AddressDistance => {
                    builtin_type_reference(program, BuiltinTypeAtom::U64)
                }
                BuiltinBinaryResult::Arithmetic { symbol, policy } => program
                    .type_reference_table
                    .find_arithmetic_result_type_reference(symbol, policy),
                BuiltinBinaryResult::Schematic(reference) => Some(reference),
            }
        }

        fn selected_arithmetic_result(
            program: &dyn crate::CheckingInput,
            operands: [Option<TypeReferenceHandle>; 2],
            carrier: TypeReferenceHandle,
        ) -> Option<BuiltinBinaryResult> {
            let (symbol, _) = arithmetic_carrier(program, carrier)?;
            let policy = operand_arithmetic_domain(program, &operands)?;
            Some(BuiltinBinaryResult::Arithmetic { symbol, policy })
        }

        pub(crate) fn builtin_binary_result(
            program: &dyn crate::CheckingInput,
            binary: &TableBinaryExpression,
            operands: [Option<TypeReferenceHandle>; 2],
            expression_type: &mut dyn FnMut(ExpressionHandle) -> Option<TypeReferenceHandle>,
        ) -> Option<BuiltinBinaryResult> {
            use BinaryOperator::*;
            let declared_operands = operands;
            let operands = operands.map(|reference| {
                reference.and_then(|reference| {
                    super::scalar_observations::readable_observation_type_reference(
                        program, reference,
                    )
                })
            });
            let mathematical = std::array::from_fn::<_, 2, _>(|position| {
                let expression = [binary.left, binary.right][position];
                operands[position].is_some_and(|reference| {
                    matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.builtin_type_symbol(symbols::BuiltinType::Int) == Some(*symbol))
                }) || crate::proof_contracts::contract_entailment::proof_integer_expression(
                    program, expression,
                )
            });
            if mathematical.into_iter().any(|value| value)
                && [binary.left, binary.right]
                    .into_iter()
                    .zip(mathematical)
                    .all(|(expression, mathematical)| {
                        mathematical
                            || crate::proof_contracts::contract_entailment::anonymous_integer_value(
                                program, expression,
                            )
                            .is_some()
                    })
            {
                return match binary.operator {
                    Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
                        Some(BuiltinBinaryResult::Boolean)
                    }
                    Add | Subtract | Multiply | Divide | Modulo => {
                        Some(BuiltinBinaryResult::ProofInteger)
                    }
                    _ => None,
                };
            }
            match binary.operator {
                CaseMembership => None,
                And | Or => operands
                    .into_iter()
                    .all(|reference| {
                        reference.and_then(|reference| primitive(program, reference))
                            == Some(PrimitiveType::Bool)
                    })
                    .then_some(BuiltinBinaryResult::Boolean),
                Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
                    if schematic_type_parameter_operand(program, binary, operands).is_some() {
                        return Some(BuiltinBinaryResult::Boolean);
                    }
                    let equality = matches!(binary.operator, Equal | NotEqual)
                        && matches!(operands, [Some(left), Some(right)]
                    if crate::declared_carriers_match(program, left, right));
                    let mathematical =
                        crate::proof_contracts::contract_entailment::mathematical_integer_operands(
                            program,
                            [binary.left, binary.right],
                        );
                    let integer_comparison = [binary.left, binary.right]
                .into_iter()
                .zip(operands)
                .all(|(expression, reference)| {
                    super::scalar_observations::completed_observations_satisfy(
                        program, expression, reference, expression_type, &|expression, reference| {
                            let Some(reference) = reference else {
                                return crate::proof_contracts::contract_entailment::anonymous_integer_value(program, expression).map(|_| ());
                            };
                            let observed = super::scalar_observations::readable_observation_type_reference(program, reference)?;
                            let bare = program.type_reference_table.unconstrained_type_reference(observed);
                            integer(program, bare).then_some(())
                        },
                    )
                });
                    let byte_equality = matches!(binary.operator, Equal | NotEqual)
                        && [binary.left, binary.right]
                            .into_iter()
                            .zip(declared_operands)
                            .all(|(expression, reference)| {
                                super::scalar_observations::completed_observations_satisfy(
                                    program,
                                    expression,
                                    reference,
                                    expression_type,
                                    &|expression, reference| {
                                        if let Some(reference) = reference {
                                            crate::comparable_byte_carrier_type(program, reference)
                                                .map(|_| ())
                                        } else {
                                            typed_trees::byte_predicates::literal_bytes(
                                                program, expression,
                                            )
                                            .map(|_| ())
                                        }
                                    },
                                )
                            });
                    let compatible =
                        if equality || mathematical || integer_comparison || byte_equality {
                            true
                        } else if let Some(carrier) = operands.into_iter().flatten().next() {
                            let meaning = super::scalar_observations::scalar_observation_meaning(
                                program, carrier,
                            )?;
                            let primitive = meaning.0;
                            let supported = primitive.is_integer()
                                || matches!(
                                    primitive,
                                    PrimitiveType::Addr | PrimitiveType::F32 | PrimitiveType::F64
                                )
                                || (matches!(binary.operator, Equal | NotEqual)
                                    && primitive == PrimitiveType::Bool);
                            supported
                                && [binary.left, binary.right].into_iter().zip(operands).all(
                                    |(expression, reference)| {
                                        reference.map_or_else(
                                || {
                                    crate::value_custody::literals::has_anonymous_numeric_results(
                                        program, expression,
                                    )
                                },
                                |reference| {
                                    super::scalar_observations::scalar_observation_meaning(
                                        program, reference,
                                    )
                                    .is_some_and(|other| {
                                        (primitive.is_fixed_integer() && other.0.is_fixed_integer())
                                            || other == meaning
                                    })
                                },
                            )
                                    },
                                )
                        } else {
                            [binary.left, binary.right].into_iter().all(|operand| {
                                crate::value_custody::literals::has_anonymous_numeric_results(
                                    program, operand,
                                )
                            })
                        };
                    compatible.then_some(BuiltinBinaryResult::Boolean)
                }
                ShiftLeft | ShiftRight => {
                    let left = operands[0].filter(|reference| integer(program, *reference))?;
                    let count = operands[1].map_or_else(
                        || {
                            crate::value_custody::literals::has_anonymous_numeric_results(
                                program,
                                binary.right,
                            )
                        },
                        |reference| integer(program, reference),
                    );
                    count
                        .then(|| arithmetic_carrier(program, left))
                        .flatten()
                        .map(|(symbol, policy)| BuiltinBinaryResult::Arithmetic { symbol, policy })
                }
                Add | Subtract | Multiply | Divide => {
                    if let Some(carrier) =
                        schematic_type_parameter_operand(program, binary, operands)
                    {
                        return Some(BuiltinBinaryResult::Schematic(carrier));
                    }
                    if operands.iter().any(|reference| {
                        reference.is_some_and(|reference| {
                            primitive(program, reference) == Some(PrimitiveType::Addr)
                        })
                    }) {
                        let mut carriers = operands.map(|reference| {
                            reference.and_then(|reference| primitive(program, reference))
                        });
                        for position in 0..2 {
                            if carriers[position].is_none() {
                                let expression = if position == 0 {
                                    binary.left
                                } else {
                                    binary.right
                                };
                                if operands[position].is_some()
                                    || !crate::value_custody::literals::has_anonymous_numeric_results(
                                        program, expression,
                                    )
                                {
                                    return None;
                                }
                                carriers[position] =
                                    Some(typed_trees::operator::integer_peer_operand_carrier(
                                        binary.operator,
                                        carriers[1 - position]?,
                                        position == 1,
                                    ));
                            }
                        }
                        let result = typed_trees::operator::builtin_integer_result_carrier(
                            binary.operator,
                            [carriers[0]?, carriers[1]?],
                        )?;
                        if result == PrimitiveType::Addr {
                            let carrier = operands.into_iter().flatten().find(|reference| {
                                primitive(program, *reference) == Some(PrimitiveType::Addr)
                            })?;
                            return selected_arithmetic_result(program, operands, carrier);
                        }
                        return Some(BuiltinBinaryResult::AddressDistance);
                    }
                    let carrier = operands.into_iter().flatten().next()?;
                    let numeric = integer(program, carrier)
                        || matches!(
                            primitive(program, carrier),
                            Some(PrimitiveType::F32 | PrimitiveType::F64)
                        );
                    (numeric && compatible_operands(program, binary, operands, carrier))
                        .then(|| selected_arithmetic_result(program, operands, carrier))
                        .flatten()
                }
                Modulo | BitwiseAnd | BitwiseOr | BitwiseXor => {
                    if matches!(binary.operator, Modulo)
                        && let Some(carrier) =
                            schematic_type_parameter_operand(program, binary, operands)
                    {
                        return Some(BuiltinBinaryResult::Schematic(carrier));
                    }
                    let carrier = operands.into_iter().flatten().next()?;
                    (integer(program, carrier)
                        && compatible_operands(program, binary, operands, carrier))
                    .then(|| selected_arithmetic_result(program, operands, carrier))
                    .flatten()
                }
            }
        }

        fn schematic_type_parameter_operand(
            program: &dyn crate::CheckingInput,
            binary: &TableBinaryExpression,
            operands: [Option<TypeReferenceHandle>; 2],
        ) -> Option<TypeReferenceHandle> {
            let mut carrier = None;
            for (expression, reference) in [binary.left, binary.right].into_iter().zip(operands) {
                let Some(reference) = reference else {
                    if crate::value_custody::literals::has_anonymous_numeric_results(
                        program, expression,
                    ) {
                        continue;
                    }
                    return None;
                };
                let bare = program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(bare)
                else {
                    return None;
                };
                let parameter = program.first_data_type_parameter_by_symbol(*symbol)?;
                if !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type) {
                    return None;
                }
                if let Some(existing) = carrier {
                    let TypeReferenceNode::Named {
                        symbol: existing_symbol,
                        ..
                    } = program.type_reference_table.type_reference(existing)
                    else {
                        return None;
                    };
                    if *existing_symbol != *symbol {
                        return None;
                    }
                } else {
                    carrier = Some(bare);
                }
            }
            carrier
        }

        fn applied_requirement_result(
            program: &dyn crate::CheckingInput,
            result: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeReferenceHandle> {
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(result)
                && let Some((_, actual)) = substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
            {
                return Some(*actual);
            }
            program
                .primitive_type_reference(result)
                .is_some()
                .then_some(result)
        }

        fn bound_result_type_parameter(
            program: &dyn crate::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            operands: &[Option<TypeReferenceHandle>],
        ) -> Option<TypeReferenceHandle> {
            let TypeReferenceNode::Named {
                symbol: result_symbol,
                name: result_name,
            } = program
                .type_reference_table
                .type_reference(operator.return_type)
            else {
                return None;
            };
            let result_parameter =
                program
                    .operator_type_parameters(operator)
                    .iter()
                    .find(|parameter| {
                        matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                            && ((result_symbol.is_valid() && parameter.symbol == *result_symbol)
                                || parameter.name.as_str() == result_name.as_str())
                    })?;
            let parameters = program.operator_parameters(operator);
            parameters
                .iter()
                .zip(operands.iter())
                .find_map(|(parameter, operand)| {
                    let operand = (*operand)?;
                    let TypeReferenceNode::Named { symbol, name } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    ((symbol.is_valid() && *symbol == result_parameter.symbol)
                        || name.as_str() == result_parameter.name.as_str())
                    .then_some(operand)
                })
        }

        fn result_mentions_type_parameters(
            program: &dyn crate::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            reference: TypeReferenceHandle,
        ) -> bool {
            let parameters = program.operator_type_parameters(operator);
            let mut pending = vec![reference];
            let mut seen = Vec::new();
            while let Some(reference) = pending.pop() {
                if seen.contains(&reference) {
                    continue;
                }
                seen.push(reference);
                if let TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                } = program.type_reference_table.type_reference(reference)
                    && parameters
                        .iter()
                        .any(|parameter| parameter.symbol == *symbol)
                {
                    return true;
                }
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        pending.push(*base_type);
                        if let Some(constraints) =
                            program.type_reference_table.constraint_span(*constraints)
                        {
                            for constraint in constraints {
                                if let TypeConstraintNode::Domain(qualification) = constraint {
                                    pending.extend(qualification.arguments.iter().copied());
                                }
                            }
                        }
                    }
                    TypeReferenceNode::Generic { arguments, .. } => {
                        pending.extend_from_slice(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments),
                        );
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        pending.push(*element_type);
                        if let typed_trees::types::FixedArrayLength::ConstParameter {
                            symbol, ..
                        } = length
                            && parameters
                                .iter()
                                .any(|parameter| parameter.symbol == *symbol)
                        {
                            return true;
                        }
                    }
                    TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                    _ => {}
                }
            }
            false
        }

        fn compatible_operands(
            program: &dyn crate::CheckingInput,
            binary: &TableBinaryExpression,
            operands: [Option<TypeReferenceHandle>; 2],
            carrier: TypeReferenceHandle,
        ) -> bool {
            [binary.left, binary.right]
                .into_iter()
                .zip(operands)
                .all(|(expression, reference)| {
                    reference.map_or_else(
                        || {
                            crate::value_custody::literals::has_anonymous_numeric_results(
                                program, expression,
                            )
                        },
                        |reference| {
                            arithmetic_carrier(program, reference).map(|(symbol, _)| symbol)
                                == arithmetic_carrier(program, carrier).map(|(symbol, _)| symbol)
                        },
                    )
                })
        }

        fn primitive(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            arithmetic_carrier(program, reference)?;
            program.primitive_type_reference(reference)
        }

        fn integer(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> bool {
            primitive(program, reference).is_some_and(|primitive| primitive.is_integer())
                || arithmetic_carrier(program, reference).is_some_and(|(symbol, _)| {
                    matches!(
                        program.symbols.builtin_type_atom(symbol),
                        Some(BuiltinTypeAtom::UInt | BuiltinTypeAtom::Int)
                    )
                })
        }

        pub(crate) fn arithmetic_carrier(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
            qualified_builtin_carrier(program, reference)
        }

        fn qualified_builtin_carrier(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
            let mut policy = None;
            let mut visited = Vec::new();
            while program
                .type_reference_table
                .contains_type_reference(reference)
                && !visited.contains(&reference)
            {
                visited.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } => {
                        program.symbols.builtin_type_atom(*symbol)?;
                        return Some((*symbol, policy.unwrap_or(ArithmeticDomain::Exact)));
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        for constraint in
                            program.type_reference_table.constraint_span(*constraints)?
                        {
                            match constraint {
                                TypeConstraintNode::ArithmeticDomain(domain) => {
                                    if policy.is_some() {
                                        return None;
                                    }
                                    policy = Some(*domain);
                                }
                                TypeConstraintNode::Named(_) | TypeConstraintNode::Domain(_) => {
                                    return None;
                                }
                            }
                        }
                        reference = *base_type;
                    }
                    _ => return None,
                }
            }
            None
        }

        pub fn arithmetic_result_type_reference(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            let (carrier, policy) = arithmetic_carrier(program, reference)?;
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(carrier, policy)
        }

        pub(crate) fn cast_result_type_reference(
            program: &dyn crate::CheckingInput,
            cast: &typed_trees::expression::TableCastExpression,
        ) -> Option<TypeReferenceHandle> {
            if program
                .type_reference_table
                .contains_type_reference(cast.result_type)
            {
                Some(cast.result_type)
            } else if !cast.semantic_domain.is_empty() {
                None
            } else if cast.domain == ArithmeticDomain::Exact {
                Some(cast.target_type)
            } else if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(cast.target_type)
            {
                program.symbols.builtin_type_atom(*symbol).and_then(|_| {
                    program
                        .type_reference_table
                        .find_arithmetic_result_type_reference(*symbol, cast.domain)
                })
            } else {
                program
                    .type_reference_table
                    .find_policy_qualified_type_reference(cast.target_type, cast.domain)
            }
        }

        pub(crate) fn operand_arithmetic_domain(
            program: &dyn crate::CheckingInput,
            operands: &[Option<TypeReferenceHandle>],
        ) -> Option<ArithmeticDomain> {
            let mut selected = ArithmeticDomain::Exact;
            for reference in operands.iter().flatten() {
                let domain = program.type_reference_table.arithmetic_domain(*reference);
                if domain == ArithmeticDomain::Exact {
                    continue;
                }
                if selected != ArithmeticDomain::Exact && selected != domain {
                    return None;
                }
                selected = domain;
            }
            Some(selected)
        }

        pub(crate) fn selected_operator_result_type_reference(
            program: &dyn crate::CheckingInput,
            operator: symbols::SymbolHandle,
            declared_result: TypeReferenceHandle,
            operands: &[Option<TypeReferenceHandle>],
        ) -> Option<TypeReferenceHandle> {
            if !crate::machine_calls::calls::selected_float_operation_retains_argument_policy(
                program, operator,
            ) {
                return Some(declared_result);
            }
            let domain = operand_arithmetic_domain(program, operands)?;
            let (carrier, _) = arithmetic_carrier(program, declared_result)?;
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(carrier, domain)
        }
    }
    pub(crate) use result_type::{
        BuiltinBinaryResult, builtin_binary_result, builtin_binary_result_type_reference,
    };
    pub(crate) mod scalar_observations {
        use crate::value_custody::type_references::substituted_identity;
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn primitive_delivery_matches(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            required: TypeReferenceHandle,
            required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            if program.primitive_type_reference(required).is_none() {
                return false;
            }
            let mut substitutions = Vec::new();
            let Some(mut reference) = super::reference_values::declared_value_type(
                program,
                expression,
                &mut substitutions,
            ) else {
                return false;
            };
            let mut read = false;
            for _ in 0..=program.type_reference_table.type_reference_count() {
                let Some(declared) = super::reference_values::substituted_reference(
                    program,
                    reference,
                    &substitutions,
                ) else {
                    return false;
                };
                let Some(observed) = readable_observation_type_reference(program, declared) else {
                    return false;
                };
                read |= observed != declared;
                let Some(resolved) = super::reference_values::substituted_reference(
                    program,
                    observed,
                    &substitutions,
                ) else {
                    return false;
                };
                if resolved != observed {
                    reference = resolved;
                    continue;
                }
                if !read
                    || scalar_observation_meaning(program, observed)
                        != scalar_observation_meaning(program, required)
                {
                    return false;
                }
                use typed_trees::type_identity::TypeIdentityRequest;
                return substituted_identity(program, observed, &substitutions)
                    == program.type_identity_with_simultaneous_substitutions(
                        TypeIdentityRequest {
                            substitutions: required_substitutions,
                            ..TypeIdentityRequest::ordinary(required)
                        },
                    );
            }
            false
        }

        pub(crate) fn builtin_scalar_operand_references(
            program: &dyn crate::CheckingInput,
            binary: &typed_trees::expression::TableBinaryExpression,
            references: [Option<TypeReferenceHandle>; 2],
            expected: Option<TypeReferenceHandle>,
        ) -> Option<[TypeReferenceHandle; 2]> {
            let reference = |index: usize| {
                if let Some(reference) = references[index] {
                    return Some(reference);
                }
                let destination = references[1 - index].or(expected)?;
                let (primitive, _) = scalar_observation_meaning(program, destination)?;
                let primitive = if references[1 - index].is_some() {
                    typed_trees::operator::integer_peer_operand_carrier(
                        binary.operator,
                        primitive,
                        index == 1,
                    )
                } else {
                    primitive
                };
                if primitive == PrimitiveType::Addr {
                    if !binary.operator.is_comparison()
                        || references[1 - index].is_none()
                        || !crate::value_custody::literals::has_anonymous_numeric_results(
                            program,
                            [binary.left, binary.right][index],
                        )
                    {
                        return None;
                    }
                    let mut diagnostics = Vec::new();
                    crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_primitive_range(
                program,
                primitive,
                [binary.left, binary.right][index],
                "completed address comparison",
                &mut diagnostics,
            );
                    if !diagnostics.is_empty() {
                        return None;
                    }
                } else {
                    if !primitive.is_integer() {
                        return None;
                    }
                    crate::value_custody::literals::land_anonymous_integer_expression(
                        program,
                        [binary.left, binary.right][index],
                        primitive,
                        &[],
                        |expression| {
                            crate::value_custody::literals::has_anonymous_operator_meaning(
                                program, expression,
                            )
                        },
                    )?;
                }
                crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    typed_trees::types::builtin_type_atom(primitive.landed_landing()?),
                )
            };
            Some([reference(0)?, reference(1)?])
        }

        pub fn scalar_observation_meaning(
            program: &dyn crate::CheckingInput,
            declared: TypeReferenceHandle,
        ) -> Option<(PrimitiveType, ArithmeticDomain)> {
            let observed = readable_observation_type_reference(program, declared)?;
            Some((
                program.primitive_type_reference(observed)?,
                program.arithmetic_domain_for_type_reference(observed),
            ))
        }

        pub(crate) fn readable_observation_type_reference(
            program: &dyn crate::CheckingInput,
            declared: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            let mut reference = declared;
            let mut observed = declared;
            for _ in 0..=program.type_reference_table.type_reference_count() {
                if !program
                    .type_reference_table
                    .contains_type_reference(reference)
                {
                    return None;
                }
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } if access.is_readable() => {
                        reference = *referee;
                        observed = *referee;
                    }
                    TypeReferenceNode::Reference { .. } => return None,
                    _ => return Some(observed),
                }
            }
            None
        }

        pub(crate) fn completed_observations_satisfy(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            reference: Option<TypeReferenceHandle>,
            expression_type: &mut dyn FnMut(ExpressionHandle) -> Option<TypeReferenceHandle>,
            accepts: &dyn Fn(ExpressionHandle, Option<TypeReferenceHandle>) -> Option<()>,
        ) -> bool {
            if reference.is_some() {
                return accepts(expression, reference).is_some();
            }
            match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Atomic(atomic) => {
                    let reference = expression_type(atomic.value);
                    completed_observations_satisfy(
                        program,
                        atomic.value,
                        reference,
                        expression_type,
                        accepts,
                    )
                }
                typed_trees::expression::ExpressionNode::Match(dispatch) => {
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    !arms.is_empty()
                        && arms.iter().all(|arm| {
                            let reference = expression_type(arm.value);
                            completed_observations_satisfy(
                                program,
                                arm.value,
                                reference,
                                expression_type,
                                accepts,
                            )
                        })
                }
                _ => accepts(expression, None).is_some(),
            }
        }
    }

    pub use match_dispatch::match_subject_primitive_type;
    pub use match_dispatch::{
        structural_match_cases_are_covered, structural_match_pattern_case,
        structural_match_pattern_is_total, structural_match_subject_type,
    };
    pub use match_dispatch::{validate_match_coverage, validate_match_dispatch};
    pub use result_type::{
        arithmetic_result_type_reference, expression_result_type_reference,
        join_result_type_references, parameter_expression_result_type_reference,
        parameter_expression_result_type_reference_in_scope,
    };

    pub(crate) use crate::value_custody::places::declared_reference_access;
    pub(crate) use crate::value_custody::type_references::substituted_identity;
    pub(crate) use delivered_types::report_delivered_type_mismatch;
    pub(crate) use delivered_types::report_delivered_type_mismatch_with_component;
    pub use reference_values::borrow_selects_reference_storage;
    pub(crate) use reference_values::{
        StorageWriteAccess, owned_rvalue_exclusive_reference_access,
        place_forwarded_exclusive_reference_access, place_permits_reference_borrow,
        place_storage_borrow_access, place_storage_write_access, substituted_required_reference,
    };
    pub(crate) use result_type::declaration_expression_result_type_reference;
    pub(crate) use result_type::domain_expression_result_type_reference;
    pub(crate) use result_type::{
        cast_result_type_reference, operand_arithmetic_domain,
        selected_operator_result_type_reference,
    };
    pub use scalar_observations::scalar_observation_meaning;
    pub(crate) use scalar_observations::{
        builtin_scalar_operand_references, readable_observation_type_reference,
    };

    pub(crate) use cast_validation::validate_cast_types;
    pub(crate) use float_coercions::{
        float_coercion_preserves_denotation, validate_exact_float_coercion,
    };

    pub(crate) use operator_validation::{
        report_non_bool_logical_not, report_non_integer_bitwise_not, report_non_integer_index,
        validate_binary_operand_types, validate_builtin_comparison_operand_landing,
    };

    pub(crate) use shape_validation::{
        report_array_scalar_shape_mismatch, report_scalar_data_shape_mismatch,
        report_view_element_argument_mismatch,
    };

    pub(crate) use value_classification::ValueClass;
    pub(crate) use value_classification::{report_cross_class_store, report_data_type_conflict};

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum ExpressionTypeOwner<'program> {
        StateTerminalExpression {
            machine: &'program str,
            state: &'program str,
        },
    }

    impl fmt::Display for ExpressionTypeOwner<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::StateTerminalExpression { machine, state } => {
                    write!(
                        formatter,
                        "machine `{machine}` state `{state}` terminal expression"
                    )
                }
            }
        }
    }

    pub fn argument_matches_type_reference_handle(
        program: &dyn crate::CheckingInput,
        argument: ExpressionHandle,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        argument_matches_type_reference_with_substitutions(program, argument, type_reference, &[])
    }

    pub(crate) fn completed_value_type_matches(
        program: &dyn crate::CheckingInput,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
    ) -> bool {
        crate::value_custody::type_references::type_references_match(program, actual, required)
            || (actual.is_valid()
                && required.is_valid()
                && reference_values::reference_type_matches(program, actual, required, &[], &[]))
    }

    pub(crate) fn completed_call_operand_matches(
        program: &dyn crate::CheckingInput,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
        required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        actual.is_valid()
            && required.is_valid()
            && (program.type_identity(TypeIdentityRequest::ordinary(actual))
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required)
                })
                || reference_values::reference_type_matches(
                    program,
                    actual,
                    required,
                    &[],
                    required_substitutions,
                ))
    }

    pub(crate) fn argument_matches_type_reference_with_substitutions(
        program: &dyn crate::CheckingInput,
        argument: ExpressionHandle,
        type_reference: TypeReferenceHandle,
        substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        argument_matches_type_reference_using(
            program,
            SymbolHandle::invalid(),
            argument,
            type_reference,
            reference_values::declared_value_matches_reference,
            substitutions,
        )
    }

    pub(crate) fn argument_matches_carrier(
        program: &dyn crate::CheckingInput,
        scope: SymbolHandle,
        argument: ExpressionHandle,
        type_reference: TypeReferenceHandle,
        required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        let (type_reference, required_substitutions) =
            reference_values::substituted_required_reference(
                program,
                type_reference,
                required_substitutions,
            );
        if let Some(borrow) = expression_table(program).borrow_expression(argument)
            && borrow.access == language_semantics::ReferenceAccess::Mutable
            && let TypeReferenceNode::Reference {
                referee,
                access: language_semantics::ReferenceAccess::Shared,
                ..
            } = program.type_reference_table.type_reference(type_reference)
        {
            return argument_matches_type_reference_using(
                program,
                scope,
                borrow.target,
                *referee,
                reference_values::value_carrier_matches_reference,
                required_substitutions,
            );
        }
        argument_matches_type_reference_using(
            program,
            scope,
            argument,
            type_reference,
            reference_values::value_carrier_matches_reference,
            required_substitutions,
        )
    }

    fn argument_matches_type_reference_using(
        program: &dyn crate::CheckingInput,
        scope: SymbolHandle,
        argument: ExpressionHandle,
        type_reference: TypeReferenceHandle,
        correspondence: reference_values::ReferenceCorrespondence,
        required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        if argument.is_valid()
            && program.pattern_component(argument) != Some(language_core::PatternComponent::Fields)
        {
            return false;
        }
        // A static equality applies to the complete delivered type, including
        // reference shells. Comparing only an outer named carrier loses `T == U`
        // at `&T` calls; the equality never supplies a loan or changes access.
        if let Some(actual) = delivered_types::delivered_value_type(program, argument)
            && program.type_identities_match_in_static_context(
                argument,
                TypeIdentityRequest {
                    substitutions: &actual.substitutions,
                    ..TypeIdentityRequest::ordinary(actual.reference)
                },
                TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(type_reference)
                },
            )
        {
            return true;
        }
        let (type_reference, required_substitutions) =
            reference_values::substituted_required_reference(
                program,
                type_reference,
                required_substitutions,
            );
        macro_rules! match_argument {
            ($argument:expr, $type_reference:expr $(,)?) => {
                argument_matches_type_reference_using(
                    program,
                    scope,
                    $argument,
                    $type_reference,
                    correspondence,
                    required_substitutions,
                )
            };
        }
        if scalar_observations::primitive_delivery_matches(
            program,
            argument,
            type_reference,
            required_substitutions,
        ) {
            return true;
        }
        if let Some(dispatch) = expression_table(program).match_node(argument) {
            return !expression_table(program)
                .match_arms(dispatch.arms)
                .is_empty()
                && expression_table(program).match_arms_all(dispatch.arms, |_, value| {
                    match_argument!(value, type_reference)
                });
        }
        if let Some(inner_expression) = expression_table(program).borrow_expression(argument) {
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(type_reference)
            else {
                return false;
            };
            if inner_expression.access != *access {
                return false;
            }
            let mut substitutions = Vec::new();
            if let Some(actual) = reference_values::declared_value_type(
                program,
                inner_expression.target,
                &mut substitutions,
            )
            .and_then(|actual| {
                reference_values::substituted_reference(program, actual, &substitutions)
            }) && matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            ) {
                if reference_values::borrow_selects_reference_storage(
                    program,
                    inner_expression.target,
                    type_reference,
                    required_substitutions,
                ) {
                    return true;
                }
                if *access == language_semantics::ReferenceAccess::WriteOnly
                    && let TypeReferenceNode::Reference {
                        referee: actual_referee,
                        access: language_semantics::ReferenceAccess::Mutable,
                        ..
                    } = program.type_reference_table.type_reference(actual)
                {
                    return substituted_identity(program, *actual_referee, &substitutions)
                        == program.type_identity_with_simultaneous_substitutions(
                            TypeIdentityRequest {
                                substitutions: required_substitutions,
                                ..TypeIdentityRequest::ordinary(*referee)
                            },
                        );
                }
                return correspondence(
                    program,
                    actual,
                    type_reference,
                    &substitutions,
                    required_substitutions,
                );
            }
            if let Some(actual) =
                delivered_types::delivered_value_type(program, inner_expression.target)
            {
                return if *access == language_semantics::ReferenceAccess::Shared {
                    correspondence(
                        program,
                        actual.reference,
                        type_reference,
                        &actual.substitutions,
                        required_substitutions,
                    )
                } else {
                    reference_values::owned_value_matches_referee(
                        program,
                        actual.reference,
                        *referee,
                        &actual.substitutions,
                        required_substitutions,
                    )
                };
            }
            return match_argument!(inner_expression.target, *referee);
        }

        if let Some(cast) = expression_table(program).cast_node(argument)
            && cast.form.is_recast()
        {
            let required_carrier = if let TypeReferenceNode::Reference {
                referee, access, ..
            } =
                program.type_reference_table.type_reference(type_reference)
            {
                let Some(formation) = crate::reference_formation_source(program, argument) else {
                    return false;
                };
                if formation.access != *access
                    && !(formation.access == language_semantics::ReferenceAccess::Mutable
                        && *access == language_semantics::ReferenceAccess::Shared)
                {
                    return false;
                }
                *referee
            } else {
                type_reference
            };
            return program.type_identity(TypeIdentityRequest::ordinary(cast.target_type))
                == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                    substitutions: required_substitutions,
                    ..TypeIdentityRequest::ordinary(required_carrier)
                });
        }

        if let Some(actual) = crate::machine_calls::calls::call_result_type(program, argument)
            && matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            )
        {
            return correspondence(program, actual, type_reference, &[], required_substitutions);
        }

        let selects_stored_value = match expression_table(program).expression(argument) {
            ExpressionNode::Member(_) => true,
            ExpressionNode::Indexed(indexed) => !matches!(
                expression_table(program).expression(indexed.index),
                ExpressionNode::Range(_)
            ),
            _ => false,
        };
        if selects_stored_value
            && matches!(
                program.type_reference_table.type_reference(type_reference),
                TypeReferenceNode::Reference { .. }
            )
        {
            return reference_values::projected_matches_reference(
                program,
                scope,
                argument,
                type_reference,
                correspondence,
                required_substitutions,
            );
        }

        if matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Reference { .. }
        ) && let ExpressionNode::Name(path) = expression_table(program).expression(argument)
            && let Some(actual) = named_value_type_reference(program, path)
            && matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            )
        {
            return correspondence(program, actual, type_reference, &[], required_substitutions);
        }

        if let Some(actual) = delivered_types::delivered_value_type(program, argument) {
            if delivered_types::delivered_carrier_conflict(
                program,
                scope,
                argument,
                actual.reference,
                &actual.substitutions,
                type_reference,
                required_substitutions,
            ) {
                return false;
            }
            let delivered_identity =
                substituted_identity(program, actual.reference, &actual.substitutions);
            if delivered_identity.is_valid()
                && delivered_identity
                    == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(type_reference)
                    })
            {
                return true;
            }
        }

        let argument_node = expression_table(program).expression(argument);

        if let ExpressionNode::Indexed(indexed) = argument_node
            && matches!(
                expression_table(program).expression(indexed.index),
                ExpressionNode::Range(_)
            )
            && reference_values::subslice_view_matches_reference(
                program,
                indexed,
                type_reference,
                correspondence,
                required_substitutions,
            )
        {
            return true;
        }

        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                if let Some(actual) = delivered_types::delivered_value_type(program, argument) {
                    return correspondence(
                        program,
                        actual.reference,
                        type_reference,
                        &actual.substitutions,
                        required_substitutions,
                    );
                }
                *access == language_semantics::ReferenceAccess::Shared
                    && match_argument!(argument, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                match_argument!(argument, *base_type)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if let ExpressionNode::String(bytes) = argument_node {
                    return bytes.is_empty()
                        && *length == typed_trees::types::FixedArrayLength::Literal(0)
                        && program.primitive_type_reference(*element_type)
                            == Some(PrimitiveType::U8);
                }
                if let ExpressionNode::ArrayLiteral(elements) = argument_node {
                    let elements = expression_table(program).expression_handles(*elements);
                    return !matches!(length, typed_trees::types::FixedArrayLength::Literal(length)
                    if *length != elements.len())
                        && elements
                            .iter()
                            .all(|element| match_argument!(*element, *element_type));
                }
                let mut substitutions = Vec::new();
                let Some(actual) =
                    reference_values::declared_value_type(program, argument, &mut substitutions)
                else {
                    return false;
                };
                let actual = program
                    .type_reference_table
                    .unconstrained_type_reference(actual);
                substituted_identity(program, actual, &substitutions)
                    == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(type_reference)
                    })
                    || reference_values::fixed_array_binds_open_length(
                        program,
                        actual,
                        type_reference,
                        &substitutions,
                        required_substitutions,
                    )
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_is_u8 = matches!(
                    program.type_reference_table.primitive_type(*element_type),
                    Some(PrimitiveType::U8)
                );
                matches!(
                    argument_node,
                    ExpressionNode::Call(_)
                        | ExpressionNode::Indexed(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Name(_)
                ) || (element_is_u8 && matches!(argument_node, ExpressionNode::String(_)))
            }
            TypeReferenceNode::Generic { .. } => matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Unary(_)
            ),
            TypeReferenceNode::DynamicTrait { .. } => matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            ),
            TypeReferenceNode::Named {
                symbol,
                name: type_name,
            } => {
                if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                    if matches!(argument_node, ExpressionNode::Atomic(_)) {
                        let Some(actual) = delivered_types::delivered_value_type(program, argument)
                        else {
                            return false;
                        };
                        return substituted_identity(
                            program,
                            actual.reference,
                            &actual.substitutions,
                        ) == program.type_identity_with_simultaneous_substitutions(
                            TypeIdentityRequest {
                                substitutions: required_substitutions,
                                ..TypeIdentityRequest::ordinary(type_reference)
                            },
                        );
                    }
                    if crate::value_custody::literals::land_anonymous_integer_expression(
                        program,
                        argument,
                        primitive_type,
                        &[],
                        |expression| {
                            crate::value_custody::literals::has_anonymous_operator_meaning(
                                program, expression,
                            )
                        },
                    )
                    .is_some()
                    {
                        return true;
                    }
                    if let ExpressionNode::UndeterminedNaN(format) = argument_node {
                        return primitive_type
                            == match format {
                                numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                                numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                            };
                    }
                    return matches!(argument_node, ExpressionNode::Boolean(_))
                        && primitive_type == PrimitiveType::Bool
                        || matches!(argument_node, ExpressionNode::Float(_))
                            && primitive_type.accepts_float_literal()
                        || matches!(argument_node, ExpressionNode::Integer(_))
                            && primitive_type.accepts_integer_literal()
                        || matches!(argument_node, ExpressionNode::Unary(unary)
                        if match unary.operator {
                            typed_trees::expression::UnaryOperator::BitwiseNot => {
                                primitive_type.accepts_integer_literal()
                            }
                            typed_trees::expression::UnaryOperator::LogicalNot => {
                                primitive_type == PrimitiveType::Bool
                            }
                        })
                        || matches!(
                            argument_node,
                            ExpressionNode::Binary(_)
                                | ExpressionNode::Call(_)
                                | ExpressionNode::Cast(_)
                                | ExpressionNode::Indexed(_)
                                | ExpressionNode::Member(_)
                                | ExpressionNode::Name(_)
                                | ExpressionNode::StructLiteral(_)
                        );
                }

                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data
                {
                    match argument_node {
                        ExpressionNode::StructLiteral(literal) => {
                            return program
                                .closed_data_constructor_definition(
                                    type_reference,
                                    literal.type_symbol,
                                )
                                .is_some();
                        }
                        ExpressionNode::Name(path)
                            if program.symbols.get(path.symbol).kind
                                == symbols::SymbolKind::Variant =>
                        {
                            return program.symbols.get(path.symbol).parent == *symbol;
                        }
                        ExpressionNode::Cast(cast)
                            if program.primitive_type_reference(cast.target_type).is_some() =>
                        {
                            return false;
                        }
                        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => return false,
                        _ => {}
                    }
                }

                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                    && matches!(argument_node, ExpressionNode::Integer(_))
                {
                    return true;
                }

                matches!(
                    argument_node,
                    ExpressionNode::Binary(_)
                        | ExpressionNode::Call(_)
                        | ExpressionNode::Cast(_)
                        | ExpressionNode::Indexed(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Name(_)
                        | ExpressionNode::StructLiteral(_)
                        | ExpressionNode::Unary(_)
                )
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
        }
    }

    fn builtin_member_type_reference(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        match program.symbols.builtin_type_member(symbol)? {
            symbols::BuiltinTypeMember::AddressBound => {
                crate::proof_contracts::proof_embeddings::proof_int_type_reference(program)
            }
        }
    }

    pub(crate) fn named_value_type_reference(
        program: &dyn crate::CheckingInput,
        path: &typed_trees::expression::TableNamePath,
    ) -> Option<TypeReferenceHandle> {
        if let Some(reference) = builtin_member_type_reference(program, path.symbol) {
            return Some(reference);
        }
        let [_] = expression_table(program).name_path_members(path.members) else {
            return None;
        };
        if let Some(reference) =
            crate::proof_contracts::mathematical_applications::parameter_type_reference(
                program,
                path.symbol,
            )
        {
            return Some(reference);
        }
        if path.symbol.is_valid()
            && path.head_symbol == path.symbol
            && let Some(parameter) = program.first_data_type_parameter_by_symbol(path.symbol)
            && let typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
        {
            return Some(type_reference);
        }
        let matches_symbol = |candidate: symbols::SymbolHandle| {
            candidate.is_valid()
                && ((path.symbol.is_valid() && candidate == path.symbol)
                    || (path.head_symbol.is_valid() && candidate == path.head_symbol))
        };

        if let Some(type_reference) =
            hinted_named_value_type_reference(program, path, matches_symbol)
        {
            return Some(type_reference);
        }

        program.first_named_value_declared_type(path.symbol, path.head_symbol)
    }

    fn hinted_named_value_type_reference(
        program: &dyn crate::CheckingInput,
        path: &typed_trees::expression::TableNamePath,
        matches_symbol: impl Fn(symbols::SymbolHandle) -> bool,
    ) -> Option<TypeReferenceHandle> {
        let binder = if path.symbol.is_valid() {
            path.symbol
        } else {
            path.head_symbol
        };
        if !binder.is_valid() {
            return None;
        }
        let parent = program.symbols.get(binder).parent;
        if !parent.is_valid() {
            return None;
        }
        match program.symbols.get(parent).kind {
            symbols::SymbolKind::Domain => program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == parent)
                .filter(|domain| matches_symbol(domain.subject_symbol))
                .map(|domain| domain.target_type),
            symbols::SymbolKind::Data => {
                let definition = program.data_definition_by_symbol(parent)?;
                if definition.predicate_subject_symbol.is_valid()
                    && matches_symbol(definition.predicate_subject_symbol)
                    && program
                        .symbols
                        .get(definition.predicate_subject_symbol)
                        .parent
                        == definition.symbol
                    && program
                        .symbols
                        .get(definition.predicate_subject_symbol)
                        .kind
                        == symbols::SymbolKind::Parameter
                {
                    return definition
                        .predicate_subject_type
                        .is_valid()
                        .then_some(definition.predicate_subject_type);
                }
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if matches_symbol(field.symbol) => {
                            Some(field.type_reference)
                        }
                        _ => None,
                    })
            }
            symbols::SymbolKind::Variant => {
                let owner = program.symbols.get(parent).parent;
                let definition = program.data_definition_by_symbol(owner)?;
                let variant =
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == parent => {
                                Some(variant)
                            }
                            _ => None,
                        })?;
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| matches_symbol(field.symbol))
                    .map(|field| field.type_reference)
            }
            symbols::SymbolKind::Machine => {
                if let Some(machine) = program.machine(parent) {
                    return program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned| matches_symbol(owned.symbol))
                        .map(|owned| owned.type_reference);
                }
                None
            }
            symbols::SymbolKind::Proposition => program
                .proposition_definition_by_symbol(parent)
                .and_then(|proposition| {
                    program
                        .proposition_parameters(proposition)
                        .iter()
                        .find(|parameter| matches_symbol(parameter.symbol))
                        .map(|parameter| parameter.type_reference)
                }),
            symbols::SymbolKind::Measure => program
                .measures()
                .iter()
                .find(|measure| measure.symbol == parent)
                .and_then(|measure| measure.parameter.as_ref())
                .filter(|parameter| {
                    path.symbol.is_valid()
                        && path.head_symbol == path.symbol
                        && parameter.symbol == path.symbol
                })
                .map(|parameter| parameter.type_reference),
            symbols::SymbolKind::State => {
                let grandparent = program.symbols.get(parent).parent;
                if program.symbols.get(grandparent).kind != symbols::SymbolKind::Machine {
                    return None;
                }
                let (_, state) = program.machine_state(grandparent, parent)?;
                state_symbol_declared_type(program, state, matches_symbol)
            }
            _ => None,
        }
    }

    fn state_symbol_declared_type(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        matches_symbol: impl Fn(SymbolHandle) -> bool,
    ) -> Option<TypeReferenceHandle> {
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| matches_symbol(parameter.symbol))
        {
            return Some(parameter.type_reference);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data()
                && matches_symbol(local.symbol)
            {
                return Some(local.type_reference);
            }
        }
        None
    }

    pub fn bounded_byte_buffer_capacity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<usize> {
        match crate::byte_sequence_carrier(program, type_reference, &[])? {
            CheckedByteSequenceCarrier::BoundedOwned { capacity } => usize::try_from(capacity).ok(),
            _ => None,
        }
    }

    pub(crate) fn validate_expression_type_handle(
        program: &dyn crate::CheckingInput,
        scope: SymbolHandle,
        expression: ExpressionHandle,
        type_reference: TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
        owner: ExpressionTypeOwner<'_>,
    ) {
        if let ExpressionNode::String(literal) = expression_table(program).expression(expression)
            && let Some(capacity) = bounded_byte_buffer_capacity(program, type_reference)
            && literal.len() > capacity
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} constructs {} byte(s), exceeding the {capacity}-byte capacity of `{}`",
                literal.len(),
                program.display_type_reference_with_constraints(type_reference),
            )));
            return;
        }
        if !argument_matches_type_reference_using(
            program,
            scope,
            expression,
            type_reference,
            reference_values::declared_value_matches_reference,
            &[],
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} expects `{}`, got `{}`",
                program.display_type_reference_with_constraints(type_reference),
                expression_type_name_handle(program, expression)
            )));
        }
    }

    pub(crate) fn expression_type_name_handle(
        program: &dyn crate::CheckingInput,
        argument: ExpressionHandle,
    ) -> &'static str {
        match expression_table(program).expression(argument) {
            ExpressionNode::Match(_) => "match expression",
            ExpressionNode::SavedSubject(_) => "saved match subject",
            ExpressionNode::Atomic(atomic) => expression_type_name_handle(program, atomic.value),
            ExpressionNode::ArrayLiteral(_) => "array literal",
            ExpressionNode::Binary(_) => "binary expression",
            ExpressionNode::Boolean(_) => "bool",
            ExpressionNode::Call(_) => "call expression",
            ExpressionNode::Cast(_) => "cast expression",
            ExpressionNode::Float(_) => "float literal",
            ExpressionNode::UndeterminedNaN(format) => format.name(),
            ExpressionNode::Indexed(_) => "indexed value",
            ExpressionNode::Integer(_) => "integer literal",
            ExpressionNode::Member(_) => "member access",
            ExpressionNode::Borrow(inner_expression) => {
                expression_type_name_handle(program, inner_expression.target)
            }
            ExpressionNode::Name(_) => "named value",
            ExpressionNode::Range(_) => "range expression",
            ExpressionNode::StructLiteral(_) => "struct literal",
            ExpressionNode::String(_) => "String",
            ExpressionNode::Unary(unary) => match unary.operator {
                typed_trees::expression::UnaryOperator::BitwiseNot => "integer",
                typed_trees::expression::UnaryOperator::LogicalNot => "bool",
            },
            ExpressionNode::TypeExpression(_) => "static type operand",
            ExpressionNode::ZeroValue(_) => "zero-value representation observation",
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn report_place_value_mismatches(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        slot_context: &str,
        slot_noun: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        report_data_type_conflict(
            program,
            machine,
            state,
            value,
            target_type,
            slot_context,
            slot_noun,
            diagnostics,
        );
        report_array_scalar_shape_mismatch(
            program,
            machine,
            state,
            value,
            target_type,
            slot_context,
            slot_noun,
            diagnostics,
        );
        report_scalar_data_shape_mismatch(
            program,
            machine,
            state,
            value,
            target_type,
            slot_context,
            slot_noun,
            diagnostics,
        );
    }

    pub(crate) use match_dispatch::validate_transition_patterns;
}

pub mod field_domains {
    use symbols::SymbolHandle;
    pub use typed_trees::byte_predicates::{ByteSequencePredicate, domain_byte_predicate};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::Machine;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod declared_fields {
        use super::{
            readable_fixed_array_elements, readable_nominal_definition, readable_type_reference,
        };
        use facts::PlaceSegment;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::types::TypeReferenceHandle;

        fn collect_data_field_domain_rows(
            program: &dyn crate::CheckingInput,
            transfer: bool,
            data: &typed_trees::data::DataDefinition,
            path: &mut Vec<PlaceSegment>,
            visited: &mut Vec<SymbolHandle>,
            rows: &mut Vec<FieldDomainRow>,
        ) {
            for field in program.data_member_fields(program.data_members(data)) {
                let prefix_length = path.len();
                crate::push_field_place_segments(program, path, field.symbol);
                collect_type_domain_rows(
                    program,
                    transfer,
                    field.type_reference,
                    path,
                    visited,
                    rows,
                );
                path.truncate(prefix_length);
            }
        }

        pub(crate) struct DeclaredTypeDomainQueries {
            rows: Vec<(
                TypeReferenceHandle,
                std::sync::OnceLock<Vec<FieldDomainRow>>,
            )>,
        }

        impl DeclaredTypeDomainQueries {
            fn of(program: &dyn crate::CheckingInput) -> Self {
                let mut rows: Vec<_> = program
                    .type_reference_table
                    .type_reference_rows()
                    .map(|(reference, _)| (reference, std::sync::OnceLock::new()))
                    .collect();
                rows.sort_by_key(|(reference, _)| {
                    (reference.arena_index(), reference.generation())
                });
                Self { rows }
            }
        }

        pub fn declared_type_domain_rows(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Vec<FieldDomainRow> {
            if !reference.is_valid() {
                return Vec::new();
            }
            let compute = || {
                let mut rows = Vec::new();
                collect_type_domain_rows(
                    program,
                    false,
                    reference,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut rows,
                );
                rows
            };
            let Some(memos) = program.checking_memos() else {
                return compute();
            };
            let queries = memos
                .declared_type_domains
                .get_or_init(|| DeclaredTypeDomainQueries::of(program));
            let Ok(position) = queries.rows.binary_search_by_key(
                &(reference.arena_index(), reference.generation()),
                |(reference, _)| (reference.arena_index(), reference.generation()),
            ) else {
                return compute();
            };
            queries.rows[position].1.get_or_init(compute).clone()
        }

        fn collect_type_domain_rows(
            program: &dyn crate::CheckingInput,
            transfer: bool,
            reference: TypeReferenceHandle,
            path: &mut Vec<PlaceSegment>,
            visited: &mut Vec<SymbolHandle>,
            rows: &mut Vec<FieldDomainRow>,
        ) {
            if readable_type_reference(program, reference).is_none() {
                return;
            }
            for domain in super::declared_domain_constraint_records(program, reference) {
                rows.push(FieldDomainRow {
                    path: path.clone(),
                    domain_symbol: domain.symbol,
                    semantic_domain: domain.semantic_id,
                    arguments: domain.arguments.clone(),
                    transfer,
                });
            }
            if let Some(data) = readable_nominal_definition(program, reference)
                && !visited.contains(&data.symbol)
            {
                visited.push(data.symbol);
                collect_data_field_domain_rows(program, transfer, data, path, visited, rows);
                visited.pop();
            }
            if let Some(element) = readable_slice_element(program, reference) {
                path.push(WHOLE_ELEMENT_EXTENT);
                collect_type_domain_rows(program, transfer, element, path, visited, rows);
                path.pop();
            }
            if let Some((element, length)) = readable_fixed_array_elements(program, reference) {
                let mut element_rows = Vec::new();
                collect_type_domain_rows(
                    program,
                    transfer,
                    element,
                    &mut Vec::new(),
                    visited,
                    &mut element_rows,
                );
                if !element_rows.is_empty() {
                    for index in 0..length {
                        place_element_rows(
                            path,
                            PlaceSegment::FixedIndex { index },
                            &element_rows,
                            None,
                            rows,
                        );
                    }
                    place_element_rows(path, WHOLE_ELEMENT_EXTENT, &element_rows, Some(true), rows);
                }
            }
        }

        fn place_element_rows(
            prefix: &[PlaceSegment],
            segment: PlaceSegment,
            element_rows: &[FieldDomainRow],
            transfer: Option<bool>,
            rows: &mut Vec<FieldDomainRow>,
        ) {
            for row in element_rows {
                let mut path = Vec::with_capacity(prefix.len() + 1 + row.path.len());
                path.extend_from_slice(prefix);
                path.push(segment);
                path.extend_from_slice(&row.path);
                rows.push(FieldDomainRow {
                    path,
                    domain_symbol: row.domain_symbol,
                    semantic_domain: row.semantic_domain,
                    arguments: row.arguments.clone(),
                    transfer: transfer.unwrap_or(row.transfer),
                });
            }
        }

        #[derive(Clone)]
        pub struct FieldDomainRow {
            pub path: Vec<PlaceSegment>,
            pub domain_symbol: SymbolHandle,
            pub semantic_domain: language_semantics::SemanticDomainId,
            pub arguments: Vec<TypeReferenceHandle>,
            pub transfer: bool,
        }

        impl FieldDomainRow {
            pub fn describes_projection(
                &self,
                path: &[PlaceSegment],
                is_point_index: impl Fn(typed_trees::expression::ExpressionHandle) -> bool,
            ) -> bool {
                self.path.len() == path.len()
                    && self.path.iter().zip(path).all(|(declared, selected)| {
                        declared == selected
                            || *declared == WHOLE_ELEMENT_EXTENT
                                && match selected {
                                    PlaceSegment::FixedIndex { .. } => true,
                                    PlaceSegment::Index { expression } => {
                                        is_point_index(*expression)
                                    }
                                    _ => false,
                                }
                    })
            }
        }

        pub const WHOLE_ELEMENT_EXTENT: PlaceSegment = PlaceSegment::FixedRange {
            start: 0,
            end: usize::MAX,
        };

        pub fn exact_declared_field(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&typed_trees::data::DataField> {
            if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
                return None;
            }
            program
                .data_definitions()
                .iter()
                .flat_map(|data| program.data_member_fields(program.data_members(data)))
                .find(|field| field.symbol == symbol)
        }

        pub fn readable_slice_element(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            match program
                .type_reference_table
                .type_reference(readable_type_reference(program, type_reference)?)
            {
                typed_trees::types::TypeReferenceNode::Slice { element_type } => {
                    Some(*element_type)
                }
                _ => None,
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn declared_element_projection_preserves_complete_path() {
                let field = PlaceSegment::Field {
                    symbol: SymbolHandle::invalid(),
                };
                let case = PlaceSegment::Case {
                    variant: SymbolHandle::invalid(),
                };
                let row = FieldDomainRow {
                    path: vec![field, WHOLE_ELEMENT_EXTENT, case, field],
                    domain_symbol: SymbolHandle::invalid(),
                    semantic_domain: language_semantics::SemanticDomainId::NULL,
                    arguments: Vec::new(),
                    transfer: true,
                };
                let runtime_index = PlaceSegment::Index {
                    expression: typed_trees::expression::ExpressionHandle::invalid(),
                };
                let describes = |path: &[PlaceSegment]| row.describes_projection(path, |_| true);
                assert!(describes(&row.path));
                assert!(describes(&[field, runtime_index, case, field]));
                assert!(!row.describes_projection(&[field, runtime_index, case, field], |_| false));
                assert!(describes(&[
                    field,
                    PlaceSegment::FixedIndex { index: 1 },
                    case,
                    field
                ]));
                assert!(!describes(&[field, runtime_index, field, field]));
                assert!(!describes(&[field, runtime_index, case]));
                assert!(!describes(&[field, PlaceSegment::OpaqueRange, case, field]));
                assert!(!describes(&[
                    field,
                    PlaceSegment::FixedRange { start: 0, end: 2 },
                    case,
                    field
                ]));
                let exact = FieldDomainRow {
                    path: vec![PlaceSegment::FixedIndex { index: 0 }],
                    ..row
                };
                assert!(!exact.describes_projection(&[runtime_index], |_| true));
                assert!(
                    !exact.describes_projection(&[PlaceSegment::FixedIndex { index: 1 }], |_| true)
                );
            }
        }
    }
    pub(crate) use declared_fields::DeclaredTypeDomainQueries;
    pub use declared_fields::{
        FieldDomainRow, WHOLE_ELEMENT_EXTENT, declared_type_domain_rows, exact_declared_field,
        readable_slice_element,
    };

    pub fn assignment_target_type_reference(
        program: &dyn crate::CheckingInput,
        _machine: &Machine,
        state: &typed_trees::state::State,
        target: typed_trees::expression::ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        crate::expression_type_reference_in_state(
            program,
            state.symbol,
            program
                .statement_table
                .statements(state.statement_nodes)
                .len(),
            target,
        )
    }

    pub fn assignment_target_domain_identities(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        target: typed_trees::expression::ExpressionHandle,
    ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
        assignment_target_domain_constraints(program, machine, state, target)
            .into_iter()
            .map(|domain| (domain.symbol, domain.semantic_id))
            .collect()
    }

    pub fn assignment_target_domain_constraints<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        target: typed_trees::expression::ExpressionHandle,
    ) -> Vec<&'program typed_trees::types::DomainConstraint> {
        let Some(type_reference) =
            assignment_target_type_reference(program, machine, state, target)
        else {
            return Vec::new();
        };
        declared_domain_constraints(program, type_reference)
            .into_iter()
            .filter(|domain| {
                program
                    .domain_definition(domain.symbol)
                    .is_some_and(|definition| definition.predicate_presence.is_present())
            })
            .collect()
    }

    pub fn attached_data_field_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let path = self_field_path(program, expression)?;
        let (last, parents) = path.split_last()?;

        let attached = machine.attached_data.as_ref()?;
        let mut data = program.data_definition_by_name_str(attached.as_str())?;
        for segment in parents {
            let field_type = data_field_type_by_name(program, data, segment)?;
            let next = type_reference_data_name(program, field_type)?;
            data = program.data_definition_by_name_str(next.as_str())?;
        }
        data_field_type_by_name(program, data, last)
    }

    fn self_field_path(
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<Vec<String>> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                let mut path = self_field_path(program, member.receiver)?;
                path.push(member.member.as_str().to_owned());
                Some(path)
            }
            ExpressionNode::Name(name) => {
                match program.expression_table.name_path_members(name.members) {
                    [first, rest @ ..] if first.is_self_receiver() => Some(
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

    pub fn construction_field_type(
        program: &dyn crate::CheckingInput,
        data_definition: &typed_trees::data::DataDefinition,
        case_name: Option<&str>,
        field_name: &str,
    ) -> Option<TypeReferenceHandle> {
        if let Some(case_name) = case_name
            && let Some(variant) = program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case_name =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
        {
            for payload_field in program.data_payload_fields(variant) {
                if payload_field.name.as_str() == field_name {
                    return payload_field
                        .type_reference
                        .is_valid()
                        .then_some(payload_field.type_reference);
                }
            }
        }
        data_field_type_by_name(program, data_definition, field_name)
    }

    pub fn data_field_type_by_name(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        field_name: &str,
    ) -> Option<TypeReferenceHandle> {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    field
                        .type_reference
                        .is_valid()
                        .then_some(field.type_reference)
                }
                _ => None,
            })
    }

    pub fn data_field_type_by_symbol_or_name(
        program: &dyn crate::CheckingInput,
        data_definition: &typed_trees::data::DataDefinition,
        member_symbol: SymbolHandle,
        member_name: &typed_trees::name::Identifier,
    ) -> Option<TypeReferenceHandle> {
        program
            .data_members(data_definition)
            .iter()
            .find_map(|member| {
                let field = member.as_field()?;

                ((member_symbol.is_valid() && field.symbol == member_symbol)
                    || field.name == *member_name)
                    .then_some(field.type_reference)
            })
    }

    pub fn data_definition_by_symbol_or_name<'program>(
        program: &'program dyn crate::CheckingInput,
        symbol: SymbolHandle,
        name: &typed_trees::name::Identifier,
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        if symbol.is_valid() {
            return crate::value_custody::places::unique_data_definition_by_symbol(program, symbol);
        }
        let mut definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.name == *name);
        let definition = definitions.next()?;
        definitions.next().is_none().then_some(definition)
    }

    pub fn data_field_type_reference_by_symbol_or_name(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        member_symbol: SymbolHandle,
        member_name: &typed_trees::name::Identifier,
        data_definition: impl for<'program> Fn(
            &'program dyn crate::CheckingInput,
            SymbolHandle,
            &typed_trees::name::Identifier,
        )
            -> Option<&'program typed_trees::data::DataDefinition>,
    ) -> Option<TypeReferenceHandle> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => data_field_type_reference_by_symbol_or_name(
                program,
                *referee,
                member_symbol,
                member_name,
                data_definition,
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => program
                .data_application_definition(type_reference)
                .or_else(|| program.closed_data_definition(type_reference))
                .or_else(|| data_definition(program, *base_symbol, base_name))
                .and_then(|data_definition| {
                    data_field_type_by_symbol_or_name(
                        program,
                        data_definition,
                        member_symbol,
                        member_name,
                    )
                }),
            TypeReferenceNode::Named { symbol, name } => data_definition(program, *symbol, name)
                .and_then(|data_definition| {
                    data_field_type_by_symbol_or_name(
                        program,
                        data_definition,
                        member_symbol,
                        member_name,
                    )
                }),
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit => None,
        }
    }

    pub fn readable_type_reference(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        while reference.is_valid() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if access.is_readable() => reference = *referee,
                TypeReferenceNode::Reference { .. } => return None,
                _ => return Some(reference),
            }
        }
        None
    }

    pub fn readable_fixed_array_elements(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, usize)> {
        let reference = readable_type_reference(program, reference)?;
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => Some((*element_type, *length)),
            _ => None,
        }
    }

    pub fn readable_nominal_definition(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let reference = readable_type_reference(program, reference)?;
        program
            .data_application_definition(reference)
            .or_else(|| program.closed_data_definition(reference))
            .or_else(|| owned_nominal_data_definition(program, reference))
    }

    pub fn owned_nominal_data_definition(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let symbol = match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                return owned_nominal_data_definition(program, *base_type);
            }
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.is_empty() => *base_symbol,
            _ => return None,
        };
        let first = program.data_definition_by_symbol(symbol)?;
        if first.type_parameters.is_empty() {
            return Some(first);
        }
        program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol && data.type_parameters.is_empty())
    }

    pub fn declared_result_field_domain_paths(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<FieldDomainIdentity> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        ) {
            return Vec::new();
        }
        declared_type_domain_rows(program, type_reference)
            .into_iter()
            .filter(|row| !row.transfer && !row.path.is_empty())
            .map(|row| (row.path, row.domain_symbol, row.semantic_domain))
            .collect()
    }

    pub type FieldDomainIdentity = (
        Vec<facts::PlaceSegment>,
        SymbolHandle,
        language_semantics::SemanticDomainId,
    );

    pub fn declared_result_field_domain_identities(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Vec<FieldDomainIdentity> {
        declared_result_field_domain_contracts(program, reference)
            .into_iter()
            .map(|row| row.identity)
            .collect()
    }

    #[derive(Clone)]
    pub struct FieldDomainContract {
        pub identity: FieldDomainIdentity,
        pub arguments: Vec<TypeReferenceHandle>,
    }

    pub fn declared_result_field_domain_contracts(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Vec<FieldDomainContract> {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        let mut rows = if matches!(
            program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Reference { .. }
        ) {
            Vec::new()
        } else {
            declared_type_domain_rows(program, reference)
                .into_iter()
                .filter(|row| !row.transfer && !row.path.is_empty())
                .map(|row| FieldDomainContract {
                    identity: (row.path, row.domain_symbol, row.semantic_domain),
                    arguments: row.arguments,
                })
                .collect::<Vec<_>>()
        };
        let owned = declared_owned_field_obligations(program, reference).domains;
        let keep_owned = {
            let mut seen: std::collections::HashSet<_> =
                rows.iter().map(|row| &row.identity).collect();
            owned
                .iter()
                .map(|row| seen.insert(&row.identity))
                .collect::<Vec<_>>()
        };
        rows.extend(
            owned
                .into_iter()
                .zip(keep_owned)
                .filter_map(|(row, keep)| keep.then_some(row)),
        );
        rows
    }

    #[derive(Clone, Default)]
    pub struct OwnedFieldObligations {
        pub domains: Vec<FieldDomainContract>,
        pub custody: Vec<Vec<facts::PlaceSegment>>,
        pub unresolved: Vec<Vec<facts::PlaceSegment>>,
    }

    pub(crate) struct OwnedFieldDomainQueries {
        rows: Vec<(
            TypeReferenceHandle,
            std::sync::OnceLock<OwnedFieldObligations>,
        )>,
    }

    impl OwnedFieldDomainQueries {
        fn of(program: &dyn crate::CheckingInput) -> Self {
            let mut rows: Vec<_> = program
                .type_reference_table
                .type_reference_rows()
                .map(|(reference, _)| (reference, std::sync::OnceLock::new()))
                .collect();
            rows.sort_by_key(|(reference, _)| (reference.arena_index(), reference.generation()));
            Self { rows }
        }
    }

    pub fn declared_owned_field_domain_identities(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Vec<FieldDomainIdentity> {
        declared_owned_field_obligations(program, reference)
            .domains
            .into_iter()
            .map(|row| row.identity)
            .collect()
    }

    pub fn declared_owned_field_obligations(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> OwnedFieldObligations {
        let compute = || declared_owned_field_obligations_uncached(program, reference);
        let Some(memos) = program.checking_memos() else {
            return compute();
        };
        let queries = memos
            .owned_field_domains
            .get_or_init(|| OwnedFieldDomainQueries::of(program));
        let Ok(position) = queries.rows.binary_search_by_key(
            &(reference.arena_index(), reference.generation()),
            |(reference, _)| (reference.arena_index(), reference.generation()),
        ) else {
            return compute();
        };
        queries.rows[position].1.get_or_init(compute).clone()
    }

    fn declared_owned_field_obligations_uncached(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> OwnedFieldObligations {
        fn visit(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            prefix: &mut Vec<facts::PlaceSegment>,
            ancestors: &mut Vec<SymbolHandle>,
            output: &mut OwnedFieldObligations,
        ) {
            let carrier = program
                .type_reference_table
                .unconstrained_type_reference(reference);
            if matches!(
                program.type_reference_table.type_reference(carrier),
                TypeReferenceNode::Reference { .. }
            ) {
                return;
            }
            if !prefix.is_empty() {
                if crate::linear_claim_frontier(program, carrier)
                    .iter()
                    .any(|claim| claim.path.is_empty())
                    || crate::nominal_cleanup_machine_for_type(program, reference, &[]).is_some()
                {
                    output.custody.push(prefix.clone());
                }
                output.domains.extend(
                    declared_domain_constraint_records(program, reference)
                        .into_iter()
                        .map(|domain| FieldDomainContract {
                            identity: (prefix.clone(), domain.symbol, domain.semantic_id),
                            arguments: domain.arguments.clone(),
                        }),
                );
            }
            // An applied schema keeps its fields even when an argument is generic.
            // Unknown field carriers still retain their unresolved obligations below.
            if let Some(data) = program
                .data_application_definition(carrier)
                .or_else(|| program.closed_data_definition(carrier))
                .or_else(|| owned_nominal_data_definition(program, carrier))
            {
                if ancestors.contains(&data.symbol) {
                    output.unresolved.push(prefix.clone());
                    return;
                }
                ancestors.push(data.symbol);
                for field in program.data_member_fields(program.data_members(data)) {
                    let length = prefix.len();
                    crate::push_field_place_segments(program, prefix, field.symbol);
                    visit(program, field.type_reference, prefix, ancestors, output);
                    prefix.truncate(length);
                }
                ancestors.pop();
            } else if let TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(carrier)
            {
                if *length == 0 {
                    return;
                }
                let mut element = OwnedFieldObligations::default();
                let mut element_prefix = vec![facts::PlaceSegment::FixedIndex { index: 0 }];
                visit(
                    program,
                    *element_type,
                    &mut element_prefix,
                    ancestors,
                    &mut element,
                );
                for index in 0..*length {
                    for row in &element.domains {
                        let (path, symbol, identity) = &row.identity;
                        let mut indexed = Vec::with_capacity(prefix.len() + path.len());
                        indexed.extend_from_slice(prefix);
                        indexed.push(facts::PlaceSegment::FixedIndex { index });
                        indexed.extend_from_slice(&path[1..]);
                        output.domains.push(FieldDomainContract {
                            identity: (indexed, *symbol, *identity),
                            arguments: row.arguments.clone(),
                        });
                    }
                    for path in &element.unresolved {
                        let mut indexed = prefix.clone();
                        indexed.push(facts::PlaceSegment::FixedIndex { index });
                        indexed.extend_from_slice(&path[1..]);
                        output.unresolved.push(indexed);
                    }
                    for path in &element.custody {
                        let mut indexed = prefix.clone();
                        indexed.push(facts::PlaceSegment::FixedIndex { index });
                        indexed.extend_from_slice(&path[1..]);
                        output.custody.push(indexed);
                    }
                }
            } else if program.primitive_type_reference(carrier).is_none()
                && !matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::Unit
                )
            {
                output.unresolved.push(prefix.clone());
            }
        }
        let mut output = OwnedFieldObligations::default();
        visit(
            program,
            reference,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut output,
        );
        output
    }

    fn domain_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::domain::DomainDefinition> {
        typed_trees::domain::domain_by_symbol(program, symbol)
    }

    pub fn domain_requires_provenance(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> bool {
        fn visit(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
            ancestors: &mut Vec<SymbolHandle>,
        ) -> bool {
            if ancestors.contains(&symbol) {
                return true;
            }
            let Some(domain) = domain_by_symbol(program, symbol) else {
                return true;
            };
            if !domain.establishment_routes.is_empty() {
                return true;
            }
            ancestors.push(symbol);
            let routed = domain.alias.as_ref().is_some_and(|alias| {
                alias
                    .constituents
                    .iter()
                    .any(|part| visit(program, part.domain_symbol, ancestors))
            }) || typed_trees::domain::subject_membership_facts(program, domain)
                .any(|membership| visit(program, membership.domain_symbol, ancestors));
            ancestors.pop();
            routed
        }
        visit(program, symbol, &mut Vec::new())
    }

    pub fn domain_predicate_theory_is_complete(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::domain::DomainDefinition,
    ) -> bool {
        definition.classification.is_none()
            && definition.semantic_roles.is_empty()
            && definition.cleanup_selection.is_none()
            && definition.alias.is_none()
            && definition.operators.is_empty()
            && !matches!(
                program
                    .type_reference_table
                    .type_reference(definition.target_type),
                TypeReferenceNode::Constrained { .. }
            )
            && domain_constraint_symbols(program, definition.target_type).is_empty()
            && typed_trees::domain::required_subject_memberships(
                program.typed_program(),
                definition,
            )
            .is_some()
    }

    pub fn domain_is_vacuous(
        program: &dyn crate::CheckingInput,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
            return false;
        }
        let Some(domain) = program.domain_definition(domain_symbol) else {
            return false;
        };
        if let Some(alias) = domain.alias.as_ref() {
            if alias.constituents.is_empty() {
                return false;
            }
            stack.push(domain_symbol);
            let vacuous = alias
                .constituents
                .iter()
                .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
            stack.pop();
            return vacuous;
        }
        !domain.predicate_presence.is_present() && domain.establishment_routes.is_empty()
    }

    fn type_reference_data_name(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<String> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
            TypeReferenceNode::Reference { referee, .. } => {
                type_reference_data_name(program, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference_data_name(program, *base_type)
            }
            _ => None,
        }
    }

    pub fn data_definition_for_field_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let type_reference = program
            .type_reference_table
            .peeled_type_reference(type_reference);
        program.closed_data_definition(type_reference)
    }

    pub fn domain_constraint_symbols(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<SymbolHandle> {
        domain_constraint_identities(program, type_reference)
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect()
    }

    pub fn domain_constraint_identities(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
        declared_domain_constraints(program, type_reference)
            .into_iter()
            .map(|domain| (domain.symbol, domain.semantic_id))
            .collect()
    }

    pub fn cast_erases_membership(
        program: &dyn crate::CheckingInput,
        cast: &typed_trees::expression::TableCastExpression,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        fn names(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
            wanted: SymbolHandle,
            stack: &mut Vec<SymbolHandle>,
        ) -> bool {
            if symbol == wanted {
                return true;
            }
            if stack.contains(&symbol) {
                return false;
            }
            let Some(alias) = program
                .domain_definition(symbol)
                .and_then(|domain| domain.alias.as_ref())
            else {
                return false;
            };
            stack.push(symbol);
            let found = alias
                .constituents
                .iter()
                .any(|part| names(program, part.domain_symbol, wanted, stack));
            stack.pop();
            found
        }
        if cast.form.is_recast() || !cast.result_type.is_valid() {
            return false;
        }
        let meaning_only = domain_requires_provenance(program, domain_symbol)
            || program
                .domain_definition(domain_symbol)
                .is_some_and(|domain| !domain.predicate_presence.is_present());
        let atomic = program
            .domain_definition(domain_symbol)
            .is_some_and(|domain| domain.semantic_id == semantic_domain);
        meaning_only
            && !domain_constraint_identities(program, cast.result_type)
                .iter()
                .any(|(symbol, identity)| {
                    (*symbol == domain_symbol && *identity == semantic_domain)
                        || (*symbol != domain_symbol
                            && atomic
                            && names(program, *symbol, domain_symbol, &mut Vec::new()))
                })
    }

    pub fn expression_erases_membership(
        program: &dyn crate::CheckingInput,
        mut expression: typed_trees::expression::ExpressionHandle,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        while let Some(borrow) = program.expression_table.borrow_expression(expression) {
            expression = borrow.target;
        }
        while let Some(cast) = program.expression_table.cast_node(expression) {
            if cast.form.is_recast() || !cast.result_type.is_valid() {
                return false;
            }
            if cast_erases_membership(program, cast, domain_symbol, semantic_domain) {
                return true;
            }
            expression = cast.value;
        }
        false
    }

    pub fn declared_domain_constraint_records(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<&typed_trees::types::DomainConstraint> {
        declared_domain_constraints(program, type_reference)
    }

    fn declared_domain_constraints(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<&typed_trees::types::DomainConstraint> {
        typed_trees::operator::declared_domain_constraints(program.typed_program(), type_reference)
            .into_iter()
            .filter(|domain| domain.symbol.is_valid())
            .collect()
    }

    pub fn predicate_domain_constraint_symbols(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<SymbolHandle> {
        predicate_domain_constraint_identities(program, type_reference)
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect()
    }

    pub fn predicate_domain_constraint_identities(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
        domain_constraint_identities(program, type_reference)
            .into_iter()
            .filter(|(symbol, _)| {
                program
                    .domain_definition(*symbol)
                    .is_some_and(|domain| domain.predicate_presence.is_present())
            })
            .collect()
    }

    pub fn declared_domain_implies(
        program: &dyn crate::CheckingInput,
        source_domain: SymbolHandle,
        target_domain: SymbolHandle,
    ) -> bool {
        typed_trees::domain::declared_domain_implies(program, source_domain, target_domain)
    }

    pub fn domain_membership_implies(
        program: &dyn crate::CheckingInput,
        source_domain: SymbolHandle,
        target_domain: SymbolHandle,
    ) -> bool {
        if !typed_trees::domain::has_symbol_only_proof(program, source_domain)
            || !typed_trees::domain::has_symbol_only_proof(program, target_domain)
        {
            return false;
        }
        if declared_domain_implies(program, source_domain, target_domain) {
            return true;
        }

        let Some(source) = program.domain_definition(source_domain) else {
            return false;
        };
        let Some(target) = program.domain_definition(target_domain) else {
            return false;
        };
        let bytes = |reference| is_byte_sequence_carrier(program, reference);
        if !bytes(source.target_type)
            || !bytes(target.target_type)
            || source.semantic_roles != Default::default()
            || target.classification.is_some()
            || target.semantic_roles != Default::default()
        {
            return false;
        }
        match (
            domain_byte_predicate(program, source_domain),
            domain_byte_predicate(program, target_domain),
        ) {
            (Some(source), Some(target)) => source.implies(target),
            _ => false,
        }
    }

    fn is_byte_sequence_carrier(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let Some(carrier) = crate::value_custody::reference_shells::carrier_beneath_borrows(
            program,
            type_reference,
        ) else {
            return false;
        };
        let element_type = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => *element_type,
            _ => return false,
        };
        program.type_reference_table.primitive_type(element_type)
            == Some(typed_trees::types::PrimitiveType::U8)
    }

    pub fn string_literal_expression_grants_domain(
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        let Some(literal) = typed_trees::byte_predicates::literal_bytes(program, expression) else {
            return false;
        };
        let Some(predicate) = domain_byte_predicate(program, domain_symbol) else {
            return false;
        };
        predicate.holds_for(&literal)
    }

    pub fn string_literal_grants_view_domains(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::String(_)
        ) && domain_constraint_symbols(program, type_reference)
            .into_iter()
            .all(|symbol| {
                domain_byte_predicate(program, symbol).is_some()
                    && program.domain_definition(symbol).is_some_and(|domain| {
                        domain.establishment_routes.is_empty()
                            && domain.semantic_roles == Default::default()
                    })
                    && string_literal_expression_grants_domain(program, expression, symbol)
            })
    }

    pub fn domain_admits_zero_byte_carrier(
        program: &dyn crate::CheckingInput,
        domain_symbol: SymbolHandle,
        carrier: TypeReferenceHandle,
    ) -> bool {
        if !program
            .domain_definition(domain_symbol)
            .is_some_and(|domain| domain.semantic_roles == Default::default())
        {
            return false;
        }
        let Some(predicate) = domain_byte_predicate(program, domain_symbol) else {
            return false;
        };
        let carrier = program.type_reference_table.peeled_type_reference(carrier);
        let (element, length) = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => (*element_type, *length),
            TypeReferenceNode::Slice { element_type } => (*element_type, 0),
            _ => return false,
        };
        if program.primitive_type_reference(element) != Some(typed_trees::types::PrimitiveType::U8)
        {
            return false;
        }
        match predicate {
            ByteSequencePredicate::ValidUtf8 | ByteSequencePredicate::AsciiOnly => true,
            ByteSequencePredicate::NoNul => length == 0,
            ByteSequencePredicate::NonEmpty => length != 0,
        }
    }

    pub fn domain_is_concat_preserving(
        program: &dyn crate::CheckingInput,
        domain_symbol: SymbolHandle,
    ) -> bool {
        domain_byte_predicate(program, domain_symbol)
            .is_some_and(ByteSequencePredicate::is_concat_preserving)
    }

    pub fn domain_is_subslice_preserving(
        program: &dyn crate::CheckingInput,
        domain_symbol: SymbolHandle,
    ) -> bool {
        domain_byte_predicate(program, domain_symbol)
            .is_some_and(ByteSequencePredicate::is_subslice_preserving)
    }

    pub fn type_reference_fixed_array_capacity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<usize> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference_fixed_array_capacity(program, *base_type)
            }
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(capacity),
                ..
            } => Some(*capacity),
            _ => None,
        }
    }
}
