/*
We represent typed operator declarations (`declarations::operator`), operator
spellings, primitive integer/float operators, trait operators, and satisfied
operator applications in `TypedTrees`.
*/

pub mod operator {
    pub(crate) mod applications {
        use language_semantics::const_value::CanonicalConstIdentity;
        use symbols::{SymbolHandle, SymbolKind};

        use crate::TypedTrees;
        use crate::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        use super::{OperatorConstBinding, OperatorDefinition};
        use crate::declarations::operator::operand_signatures::normalized_operand_parameters;
        use crate::declarations::operator::spellings::declared_domain_constraints_match;
        use crate::declarations::operator::type_matching::type_reference_matches_with_policy;
        use crate::static_applications;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum ClosedOperatorApplicationArgument {
            Type {
                binder_symbol: SymbolHandle,
                type_reference: TypeReferenceHandle,
            },
            Const {
                binder_symbol: SymbolHandle,
                declared_carrier: TypeReferenceHandle,
                value: CanonicalConstIdentity,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SymbolicOperatorTypeApplicationArgument {
            pub operator_binder_symbol: SymbolHandle,
            pub machine_binder_symbol: SymbolHandle,
            pub machine_binder_ordinal: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ClosedOperatorRealizationApplication {
            pub requirement_symbol: SymbolHandle,
            pub arguments: Vec<ClosedOperatorApplicationArgument>,
        }

        pub fn closed_operator_application_from_bindings(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            bindings: &[static_applications::StaticMachineBinding],
        ) -> Option<Vec<ClosedOperatorApplicationArgument>> {
            use crate::data::TypeParameterKind;
            use static_applications::StaticMachineBindingValue;
            let parameters = program.operator_type_parameters(operator);
            if !operator.lifetime_parameters.is_empty() || parameters.len() != bindings.len() {
                return None;
            }
            let types = parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .map(|parameter| {
                    let mut selected = bindings
                        .iter()
                        .filter(|binding| binding.parameter == parameter.symbol);
                    let binding = selected.next()?;
                    if selected.next().is_some() {
                        return None;
                    }
                    let StaticMachineBindingValue::Type(actual) = binding.value else {
                        return None;
                    };
                    closed_boundary_application_type(program, actual)
                        .then_some((parameter.symbol, actual))
                })
                .collect::<Option<Vec<_>>>()?;
            parameters
                .iter()
                .map(|parameter| {
                    let mut selected = bindings
                        .iter()
                        .filter(|binding| binding.parameter == parameter.symbol);
                    let binding = selected.next()?;
                    if selected.next().is_some() {
                        return None;
                    }
                    match (&parameter.kind, &binding.value) {
                        (TypeParameterKind::Type, StaticMachineBindingValue::Type(actual))
                            if closed_boundary_application_type(program, *actual) =>
                        {
                            Some(ClosedOperatorApplicationArgument::Type {
                                binder_symbol: parameter.symbol,
                                type_reference: *actual,
                            })
                        }
                        (
                            TypeParameterKind::Const { type_reference },
                            StaticMachineBindingValue::Const(actual),
                        ) => {
                            let carrier = static_applications::project_type_operand(
                                program,
                                *type_reference,
                                &types,
                            );
                            Some(ClosedOperatorApplicationArgument::Const {
                                binder_symbol: parameter.symbol,
                                declared_carrier: carrier,
                                value:
                                    super::type_matching::closed_const_identity_from_type_reference(
                                        program, *actual, carrier,
                                    )?,
                            })
                        }
                        _ => None,
                    }
                })
                .collect()
        }

        pub fn closed_operator_application_for_operands(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Option<Vec<ClosedOperatorApplicationArgument>> {
            closed_application_for_operands(program, operator, operand_types, false)
        }

        pub fn closed_indexed_operator_application_for_operands(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Option<Vec<ClosedOperatorApplicationArgument>> {
            if !matches!(
                operator.spelling,
                Some(super::OperatorSpelling::Index | super::OperatorSpelling::Range)
            ) {
                return None;
            }
            closed_application_for_operands(program, operator, operand_types, true)
        }

        fn closed_application_for_operands(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[Option<TypeReferenceHandle>],
            indexed_collection: bool,
        ) -> Option<Vec<ClosedOperatorApplicationArgument>> {
            if !operator.lifetime_parameters.is_empty() {
                return None;
            }
            let type_parameters = program.operator_type_parameters(operator);
            if type_parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    crate::data::TypeParameterKind::Type
                        | crate::data::TypeParameterKind::Const { .. }
                )
            }) {
                return None;
            }
            if type_parameters.is_empty() {
                return Some(Vec::new());
            }
            let parameters = program.operator_parameters(operator);
            if parameters.len() != operand_types.len() {
                return None;
            }
            let mut bindings = Vec::new();
            let mut const_bindings = Vec::new();
            let matches = operand_types
                .iter()
                .zip(normalized_operand_parameters(parameters))
                .enumerate()
                .all(|(position, (actual, expected))| {
                    actual.is_none_or(|actual| {
                        let (matched_actual, matched_expected) =
                            if indexed_collection && position == 0 {
                                super::indexing::shared_collection_elements(
                                    program,
                                    actual,
                                    expected.type_reference,
                                )
                                .unwrap_or((actual, expected.type_reference))
                            } else {
                                (actual, expected.type_reference)
                            };
                        type_reference_matches_with_policy(
                            program,
                            matched_actual,
                            matched_expected,
                            None,
                            type_parameters,
                            &mut bindings,
                            &mut const_bindings,
                            false,
                        ) && declared_domain_constraints_match(
                            program,
                            actual,
                            expected.type_reference,
                        ) && declared_domain_constraints_match(
                            program,
                            matched_actual,
                            matched_expected,
                        )
                    })
                });
            if !matches {
                return None;
            }
            let application = type_parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    crate::data::TypeParameterKind::Type => bindings
                        .iter()
                        .find_map(|(symbol, argument)| {
                            (*symbol == parameter.symbol).then_some(
                                ClosedOperatorApplicationArgument::Type {
                                    binder_symbol: parameter.symbol,
                                    type_reference: *argument,
                                },
                            )
                        })
                        .filter(|argument| match argument {
                            ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                                closed_boundary_application_type(program, *type_reference)
                            }
                            ClosedOperatorApplicationArgument::Const { .. } => false,
                        }),
                    crate::data::TypeParameterKind::Const { type_reference }
                    | crate::data::TypeParameterKind::Value { type_reference } => const_bindings
                        .iter()
                        .find_map(|OperatorConstBinding { symbol, value }| {
                            (*symbol == parameter.symbol).then_some(
                                ClosedOperatorApplicationArgument::Const {
                                    binder_symbol: parameter.symbol,
                                    declared_carrier: type_reference,
                                    value: value.clone(),
                                },
                            )
                        })
                        .filter(|_| closed_boundary_application_type(program, type_reference)),
                    crate::data::TypeParameterKind::Machine { .. }
                    | crate::data::TypeParameterKind::Implementation
                    | crate::data::TypeParameterKind::Proposition { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(application)
        }

        pub fn symbolic_operator_type_application_for_operands(
            program: &TypedTrees,
            machine: &crate::machine::Machine,
            operator: &OperatorDefinition,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Option<Vec<SymbolicOperatorTypeApplicationArgument>> {
            if !operator.lifetime_parameters.is_empty() {
                return None;
            }
            let operator_parameters = program.operator_type_parameters(operator);
            if operator_parameters.is_empty()
                || operator_parameters.iter().any(|parameter| {
                    !matches!(parameter.kind, crate::data::TypeParameterKind::Type)
                })
            {
                return None;
            }
            let parameters = program.operator_parameters(operator);
            if parameters.len() != operand_types.len() {
                return None;
            }

            let mut type_bindings = Vec::new();
            let mut const_bindings = Vec::new();
            if !operand_types
                .iter()
                .zip(normalized_operand_parameters(parameters))
                .all(|(actual, expected)| {
                    actual.is_some_and(|actual| {
                        type_reference_matches_with_policy(
                            program,
                            actual,
                            expected.type_reference,
                            None,
                            operator_parameters,
                            &mut type_bindings,
                            &mut const_bindings,
                            false,
                        ) && declared_domain_constraints_match(
                            program,
                            actual,
                            expected.type_reference,
                        )
                    })
                })
                || !const_bindings.is_empty()
            {
                return None;
            }

            let machine_parameters = program.machine_type_parameters(machine);
            operator_parameters
                .iter()
                .map(|operator_parameter| {
                    let type_reference = type_bindings.iter().find_map(|(symbol, argument)| {
                        (*symbol == operator_parameter.symbol).then_some(*argument)
                    })?;
                    let symbol = program
                        .type_reference_node(type_reference)
                        .as_named_symbol()?;
                    let (machine_binder_ordinal, machine_parameter) = machine_parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == symbol)?;
                    if !matches!(machine_parameter.kind, crate::data::TypeParameterKind::Type) {
                        return None;
                    }
                    Some(SymbolicOperatorTypeApplicationArgument {
                        operator_binder_symbol: operator_parameter.symbol,
                        machine_binder_symbol: machine_parameter.symbol,
                        machine_binder_ordinal: u32::try_from(machine_binder_ordinal).ok()?,
                    })
                })
                .collect()
        }

        pub fn closed_operator_realization_application(
            program: &TypedTrees,
            machine: &crate::machine::Machine,
            operator: &OperatorDefinition,
        ) -> Option<ClosedOperatorRealizationApplication> {
            let state = program.machine_states(machine).first()?;
            let actual_parameters = program.state_parameters(state);
            let required_parameters = program.operator_parameters(operator);
            if actual_parameters.len() != required_parameters.len()
                || actual_parameters
                    .iter()
                    .zip(required_parameters)
                    .any(|(actual, required)| {
                        actual.is_const != required.is_const
                            || actual.is_mutable != required.is_mutable
                    })
            {
                return None;
            }

            let operand_types = actual_parameters
                .iter()
                .map(|parameter| Some(parameter.type_reference))
                .collect::<Vec<_>>();
            let arguments =
                closed_operator_application_for_operands(program, operator, &operand_types)?;

            if state.return_type.is_valid() != operator.return_type.is_valid() {
                return None;
            }
            if state.return_type.is_valid() {
                let mut type_bindings = arguments
                    .iter()
                    .filter_map(|argument| match argument {
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol,
                            type_reference,
                        } => Some((*binder_symbol, *type_reference)),
                        ClosedOperatorApplicationArgument::Const { .. } => None,
                    })
                    .collect::<Vec<_>>();
                let mut const_bindings = arguments
                    .iter()
                    .filter_map(|argument| match argument {
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol,
                            value,
                            ..
                        } => Some(OperatorConstBinding {
                            symbol: *binder_symbol,
                            value: value.clone(),
                        }),
                        ClosedOperatorApplicationArgument::Type { .. } => None,
                    })
                    .collect::<Vec<_>>();
                if !type_reference_matches_with_policy(
                    program,
                    state.return_type,
                    operator.return_type,
                    None,
                    program.operator_type_parameters(operator),
                    &mut type_bindings,
                    &mut const_bindings,
                    false,
                ) || !declared_domain_constraints_match(
                    program,
                    state.return_type,
                    operator.return_type,
                ) {
                    return None;
                }
            }

            Some(ClosedOperatorRealizationApplication {
                requirement_symbol: operator.symbol,
                arguments,
            })
        }

        fn closed_boundary_application_type(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> bool {
            if !type_reference.is_valid() {
                return false;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference {
                    referee, lifetime, ..
                } => lifetime.is_none() && closed_boundary_application_type(program, *referee),
                TypeReferenceNode::Constrained { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Callable { .. } => false,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(_),
                } => closed_boundary_application_type(program, *element_type),
                TypeReferenceNode::FixedArray { .. } => false,
                TypeReferenceNode::Slice { element_type } => {
                    closed_boundary_application_type(program, *element_type)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                } => {
                    lifetime_arguments.is_empty()
                        && closed_boundary_application_nominal(
                            program,
                            *base_symbol,
                            base_name.as_str(),
                        )
                        && program
                            .type_reference_handles(*arguments)
                            .iter()
                            .all(|argument| closed_boundary_application_type(program, *argument))
                }
                TypeReferenceNode::Named { symbol, name } => {
                    closed_boundary_application_nominal(program, *symbol, name.as_str())
                }
                TypeReferenceNode::Unit => true,
            }
        }

        fn closed_boundary_application_nominal(
            program: &TypedTrees,
            symbol: SymbolHandle,
            name: &str,
        ) -> bool {
            if PrimitiveType::from_name(name).is_some()
                || language_semantics::const_value::CanonicalConstValue::from_atom(name).is_some()
                || name.parse::<i128>().is_ok()
            {
                return true;
            }
            symbol.is_valid()
                && matches!(
                    program.symbols.get(symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data
                )
        }

        pub fn canonical_closed_operator_realization_bytes(
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            retained: &[ClosedOperatorRealizationApplication],
        ) -> Result<Vec<u8>, &'static str> {
            let machine = program
                .machine(machine_symbol)
                .ok_or("operator realization specialization lost its concrete machine")?;
            let expected = program
                .machine_trait_conformances(machine)
                .iter()
                .filter_map(|conformance| {
                    super::declaration_by_symbol(program, conformance.requirement_symbol)
                        .map(|operator| (conformance.requirement_symbol, operator))
                })
                .collect::<Vec<_>>();
            if expected.len() != retained.len() {
                return Err("operator realization specialization has incomplete retained coverage");
            }

            let mut rows = Vec::with_capacity(retained.len());
            for (requirement_symbol, operator) in expected {
                let matching = retained
                    .iter()
                    .filter(|row| row.requirement_symbol == requirement_symbol)
                    .collect::<Vec<_>>();
                let [row] = matching.as_slice() else {
                    return Err(
                        "operator realization specialization has missing or duplicate exact rows",
                    );
                };
                let reconstructed = closed_operator_realization_application(
                    program, machine, operator,
                )
                .ok_or(
                    "operator realization specialization no longer matches its concrete signature",
                )?;
                if **row != reconstructed {
                    return Err(
                        "operator realization specialization mismatches fresh reconstruction",
                    );
                }

                let parameters = program.operator_type_parameters(operator);
                if parameters.len() != row.arguments.len() {
                    return Err(
                        "operator realization specialization mismatches its static telescope",
                    );
                }
                let mut bytes = Vec::new();
                encode_application_text(
                    &operator_symbol_identity(program, requirement_symbol),
                    &mut bytes,
                );
                bytes.extend((row.arguments.len() as u64).to_le_bytes());
                for (ordinal, (parameter, argument)) in
                    parameters.iter().zip(&row.arguments).enumerate()
                {
                    bytes.extend((ordinal as u64).to_le_bytes());
                    match (&parameter.kind, argument) {
                        (
                            crate::data::TypeParameterKind::Type,
                            ClosedOperatorApplicationArgument::Type {
                                binder_symbol,
                                type_reference,
                            },
                        ) if *binder_symbol == parameter.symbol && type_reference.is_valid() => {
                            bytes.push(0);
                            encode_application_text(
                                &program.package_qualified_type_identity_text(*type_reference),
                                &mut bytes,
                            );
                        }
                        (
                            crate::data::TypeParameterKind::Const {
                                type_reference: expected_carrier,
                            }
                            | crate::data::TypeParameterKind::Value {
                                type_reference: expected_carrier,
                            },
                            ClosedOperatorApplicationArgument::Const {
                                binder_symbol,
                                declared_carrier,
                                value,
                            },
                        ) if *binder_symbol == parameter.symbol
                            && *declared_carrier == *expected_carrier =>
                        {
                            bytes.push(1);
                            encode_application_text(
                                &program.package_qualified_type_identity_text(*declared_carrier),
                                &mut bytes,
                            );
                            encode_application_text(&value.type_name, &mut bytes);
                            encode_application_text(&value.encoding, &mut bytes);
                        }
                        _ => {
                            return Err(
                                "operator realization specialization mismatches binder category or identity",
                            );
                        }
                    }
                }
                rows.push(bytes);
            }
            rows.sort();
            if rows.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err("operator realization specialization has duplicate canonical rows");
            }
            let mut bytes = Vec::new();
            bytes.extend((rows.len() as u64).to_le_bytes());
            for row in rows {
                bytes.extend((row.len() as u64).to_le_bytes());
                bytes.extend(row);
            }
            Ok(bytes)
        }

        fn operator_symbol_identity(program: &TypedTrees, symbol: SymbolHandle) -> String {
            let path = program.symbols.display_path(symbol, "::");
            let Some(package) = program.symbols.symbol_package_identity(symbol) else {
                return format!("unmanaged::{path}");
            };
            let mut owner = String::with_capacity(package.digest().len() * 2);
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for byte in package.digest() {
                owner.push(char::from(HEX[usize::from(byte >> 4)]));
                owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
            format!("package:{owner}::{path}")
        }

        fn encode_application_text(value: &str, bytes: &mut Vec<u8>) {
            bytes.extend((value.len() as u64).to_le_bytes());
            bytes.extend(value.as_bytes());
        }
    }
    pub(crate) mod indexing {
        use super::{
            OperatorConstBinding, OperatorSpelling, SpelledOperator, TypeReferenceHandle,
            TypeReferenceNode, TypedTrees, resolve_spelling,
        };
        use crate::data::TypeParameter;
        use crate::declarations::operator;
        use operator::spellings::{OperandType, operator_matches_operands_with_indexed_collection};
        use operator::type_matching::type_reference_matches;
        use symbols::SymbolHandle;

        pub fn builtin_index_read_is_spelled(
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            expression: crate::expression::ExpressionHandle,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> bool {
            resolve_indexed_spelling_for_operands(
                program,
                OperatorSpelling::Index,
                operand_types,
                program.expression_table.source_span(expression),
            )
            .is_empty()
                && super::spellings::has_builtin_spelled_expression_meaning(
                    program,
                    machine_symbol,
                    expression,
                    OperatorSpelling::Index,
                    operand_types,
                )
        }

        pub fn resolve_indexed_spelling_for_operands<'program>(
            program: &'program TypedTrees,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
            use_site: source::SourceSpan,
        ) -> Vec<SpelledOperator<'program>> {
            if !matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
                return Vec::new();
            }
            let operand_types = operand_types
                .iter()
                .copied()
                .map(OperandType::from_reference)
                .collect::<Vec<_>>();
            resolve_spelling(program, spelling, None, use_site)
                .into_iter()
                .filter(|candidate| {
                    operator_matches_operands_with_indexed_collection(
                        program,
                        candidate.operator,
                        &operand_types,
                        true,
                    )
                })
                .collect()
        }

        pub(crate) fn shared_collection_elements(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
        ) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
            let expected = unconstrained_type_reference(program, expected)?;
            let TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } = program.type_reference_table.type_reference(expected)
            else {
                return None;
            };
            let expected = unconstrained_type_reference(program, *referee)?;
            let TypeReferenceNode::Slice {
                element_type: expected_element,
            } = program.type_reference_table.type_reference(expected)
            else {
                return None;
            };

            let mut actual = unconstrained_type_reference(program, actual)?;
            if let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(actual)
            {
                actual = unconstrained_type_reference(program, *referee)?;
            }
            match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    Some((*element_type, *expected_element))
                }
                _ => None,
            }
        }

        pub(crate) fn borrowed_collection_match(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            type_parameters: &[TypeParameter],
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
            const_bindings: &mut Vec<OperatorConstBinding>,
        ) -> bool {
            let Some(expected) = unconstrained_type_reference(program, expected) else {
                return false;
            };
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(expected)
            else {
                return false;
            };
            if !matches!(
                access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) {
                return false;
            }
            let expected = *referee;
            let Some(actual) = unconstrained_type_reference(program, actual) else {
                return false;
            };
            let actual = match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::Reference {
                    referee,
                    access: actual_access,
                    ..
                } => {
                    if *actual_access != *access
                        && *actual_access != language_core::ReferenceAccess::Mutable
                    {
                        return false;
                    }
                    let Some(referee) = unconstrained_type_reference(program, *referee) else {
                        return false;
                    };
                    referee
                }
                _ => actual,
            };
            type_reference_matches(
                program,
                actual,
                expected,
                None,
                type_parameters,
                bindings,
                const_bindings,
            )
        }

        pub fn unconstrained_type_reference(
            program: &TypedTrees,
            mut reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            for _ in 0..program.type_reference_table.type_reference_count() {
                if !program.contains_type_reference(reference) {
                    return None;
                }
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    _ => return Some(reference),
                }
            }
            None
        }
    }
    pub(crate) mod named_calls {
        use crate::TypedTrees;
        use crate::declarations::operator::OperatorDefinition;
        use symbols::SymbolHandle;

        pub fn resolve_named_call<'program>(
            program: &'program TypedTrees,
            target_symbol: SymbolHandle,
            static_receiver_segments: Option<&[&str]>,
            target_name: &str,
            argument_count: usize,
            has_value_receiver: bool,
        ) -> Option<&'program OperatorDefinition> {
            if target_symbol.is_valid() {
                return program.operator_definition_by_symbol(target_symbol);
            }

            let mut candidates = program.operators().iter().filter(|operator| {
                named_operator_path_matches_call(
                    program,
                    operator,
                    static_receiver_segments,
                    target_name,
                ) && named_operator_arity_fits_call(
                    program,
                    operator,
                    argument_count,
                    has_value_receiver,
                )
            });
            let first = candidates.next()?;
            candidates
                .all(|other| {
                    same_authored_operator_declaration(program, first.symbol, other.symbol)
                })
                .then_some(first)
        }

        fn same_authored_operator_declaration(
            program: &TypedTrees,
            left: SymbolHandle,
            right: SymbolHandle,
        ) -> bool {
            let Some(left_span) = program.symbols.symbol_provenance_source_span(left) else {
                return false;
            };
            let Some(right_span) = program.symbols.symbol_provenance_source_span(right) else {
                return false;
            };
            left_span.span == right_span.span
                && left_span.span.start != left_span.span.end
                && program
                    .symbols
                    .source_file(left_span)
                    .zip(program.symbols.source_file(right_span))
                    .is_some_and(|(left_file, right_file)| left_file.path == right_file.path)
        }

        pub fn resolve_named_expression_call<'program>(
            program: &'program TypedTrees,
            call: &crate::expression::TableCallExpression,
        ) -> Option<&'program OperatorDefinition> {
            if call.runtime_call.is_some() {
                return None;
            }
            if call.target_symbol.is_valid() {
                return program.operator_definition_by_symbol(call.target_symbol);
            }
            let candidates = named_expression_call_candidates(program, call);
            let [selected] = candidates.as_slice() else {
                return None;
            };
            Some(*selected)
        }

        pub fn named_expression_call_candidates<'program>(
            program: &'program TypedTrees,
            call: &crate::expression::TableCallExpression,
        ) -> Vec<&'program OperatorDefinition> {
            if call.runtime_call.is_some() {
                return Vec::new();
            }
            if call.target_symbol.is_valid() {
                return program
                    .operators()
                    .iter()
                    .chain(program.machine_token_bindings())
                    .filter(|operator| operator.symbol == call.target_symbol)
                    .collect();
            }
            let static_segments = expression_static_receiver_segments(program, call);
            let has_value_receiver = call.receiver.is_valid() && static_segments.is_none();
            let static_receiver_segments = static_segments.as_deref();
            let argument_handles = program.expression_table.expression_handles(call.arguments);
            let argument_count = argument_handles.len();
            let use_site = call
                .receiver
                .is_valid()
                .then(|| program.expression_table.source_span(call.receiver))
                .filter(|span| span.span.start != span.span.end)
                .or_else(|| {
                    argument_handles
                        .iter()
                        .map(|argument| program.expression_table.source_span(*argument))
                        .find(|span| span.span.start != span.span.end)
                })
                .unwrap_or_default();
            program
                .operators()
                .iter()
                .chain(program.machine_token_bindings())
                .filter(|operator| {
                    named_operator_is_visible_candidate(
                        program,
                        operator,
                        use_site,
                        static_receiver_segments,
                        call.target.as_str(),
                        argument_count,
                        has_value_receiver,
                    )
                })
                .collect()
        }

        fn named_operator_is_visible_candidate(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            use_site: source::SourceSpan,
            static_receiver_segments: Option<&[&str]>,
            target_name: &str,
            argument_count: usize,
            has_value_receiver: bool,
        ) -> bool {
            named_operator_path_matches_call(
                program,
                operator,
                static_receiver_segments,
                target_name,
            ) && named_operator_arity_fits_call(
                program,
                operator,
                argument_count,
                has_value_receiver,
            ) && program
                .symbols
                .source_reference_can_see_symbol(use_site, operator.symbol)
        }

        pub fn selected_named_expression_call_has_no_runtime_receiver(
            program: &TypedTrees,
            call: &crate::expression::TableCallExpression,
        ) -> bool {
            let Some(operator) = resolve_named_expression_call(program, call) else {
                return false;
            };
            let parameters = program.operator_parameters(operator);
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != call.arguments.count() as usize
                || arguments.len() != parameters.len()
            {
                return false;
            }
            if !call.receiver.is_valid() {
                return named_operator_path_matches_call(
                    program,
                    operator,
                    None,
                    call.target.as_str(),
                );
            }
            let Some(namespace) = expression_static_receiver_segments(program, call) else {
                return false;
            };
            named_operator_path_matches_call(
                program,
                operator,
                Some(&namespace),
                call.target.as_str(),
            )
        }

        fn expression_static_receiver_segments<'program>(
            program: &'program TypedTrees,
            call: &crate::expression::TableCallExpression,
        ) -> Option<Vec<&'program str>> {
            if !program.expression_table.expression_is_valid(call.receiver) {
                return None;
            }
            let crate::expression::ExpressionNode::Name(path) =
                program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let receiver_members = program.expression_table.name_path_members(path.members);
            if receiver_members.is_empty() {
                return None;
            }
            let is_known_static_namespace = path.symbol.is_valid()
                && path.symbol.arena_index() as usize <= program.symbols.symbols().len()
                && matches!(
                    program.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Trait
                );
            let is_operator_namespace = !path.symbol.is_valid()
                && program
                    .operators()
                    .iter()
                    .chain(program.machine_token_bindings())
                    .any(|operator| {
                        let operator_path = program.operator_path_members(operator.name);
                        operator_path
                            .split_last()
                            .is_some_and(|(member, namespace)| {
                                member.as_str() == call.target.as_str()
                                    && namespace.len() == receiver_members.len()
                                    && namespace
                                        .iter()
                                        .zip(receiver_members)
                                        .all(|(expected, actual)| expected == actual)
                            })
                    });
            (is_known_static_namespace || is_operator_namespace).then(|| {
                receiver_members
                    .iter()
                    .map(|member| member.as_str())
                    .collect()
            })
        }

        pub fn named_statement_call_candidates<'program>(
            program: &'program TypedTrees,
            call: &crate::statement::TableCall,
        ) -> Vec<&'program OperatorDefinition> {
            if call.runtime_call.is_some() {
                return Vec::new();
            }
            if call.target_symbol.is_valid() {
                return program
                    .operators()
                    .iter()
                    .filter(|operator| operator.symbol == call.target_symbol)
                    .collect();
            }
            let static_segments = statement_static_receiver_segments(program, call);
            let has_value_receiver = !program
                .statement_table
                .name_path_members(call.receiver)
                .is_empty()
                && static_segments.is_none();
            let argument_handles = program.statement_table.expression_handles(call.arguments);
            let argument_count = argument_handles.len();
            let use_site = argument_handles
                .iter()
                .map(|argument| program.expression_table.source_span(*argument))
                .find(|span| span.span.start != span.span.end)
                .unwrap_or_default();
            program
                .operators()
                .iter()
                .filter(|operator| {
                    named_operator_is_visible_candidate(
                        program,
                        operator,
                        use_site,
                        static_segments.as_deref(),
                        call.target.as_str(),
                        argument_count,
                        has_value_receiver,
                    )
                })
                .collect()
        }

        fn named_operator_path_matches_call(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            static_receiver_segments: Option<&[&str]>,
            target_name: &str,
        ) -> bool {
            let path = program.operator_path_members(operator.name);
            let Some((last, prefix)) = path.split_last() else {
                return false;
            };
            if last.as_str() != target_name {
                return false;
            }

            match static_receiver_segments {
                Some(segments) => {
                    prefix.len() == segments.len()
                        && prefix
                            .iter()
                            .zip(segments.iter())
                            .all(|(member, segment)| member.as_str() == *segment)
                }
                None => true,
            }
        }

        fn named_operator_arity_fits_call(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            argument_count: usize,
            has_value_receiver: bool,
        ) -> bool {
            let parameters = program.operator_parameters(operator);
            let positional = parameters.len();
            if has_value_receiver {
                return positional == argument_count + 1;
            }
            positional == argument_count
        }

        pub fn selected_named_statement_call_has_no_runtime_receiver(
            program: &TypedTrees,
            call: &crate::statement::TableCall,
        ) -> bool {
            let candidates = named_statement_call_candidates(program, call);
            let [operator] = candidates.as_slice() else {
                return false;
            };
            let arguments = program.statement_table.expression_handles(call.arguments);
            if arguments.len() != call.arguments.len()
                || arguments.len() != program.operator_parameters(operator).len()
            {
                return false;
            }
            if program
                .statement_table
                .name_path_members(call.receiver)
                .is_empty()
            {
                return named_operator_path_matches_call(
                    program,
                    operator,
                    None,
                    call.target.as_str(),
                );
            }
            let Some(namespace) = statement_static_receiver_segments(program, call) else {
                return false;
            };
            named_operator_path_matches_call(
                program,
                operator,
                Some(&namespace),
                call.target.as_str(),
            )
        }

        fn statement_static_receiver_segments<'program>(
            program: &'program TypedTrees,
            call: &crate::statement::TableCall,
        ) -> Option<Vec<&'program str>> {
            let receiver = program.statement_table.name_path_members(call.receiver);
            let receiver_resolves_to_namespace = call.receiver_symbol.is_valid()
                && matches!(
                    program.symbols.get(call.receiver_symbol).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Trait
                );
            let is_operator_namespace = !call.receiver_symbol.is_valid()
                && !receiver.is_empty()
                && program.operators().iter().any(|operator| {
                    let operator_path = program.operator_path_members(operator.name);
                    operator_path
                        .split_last()
                        .is_some_and(|(member, namespace)| {
                            member.as_str() == call.target.as_str()
                                && namespace.len() == receiver.len()
                                && namespace
                                    .iter()
                                    .zip(receiver.iter())
                                    .all(|(expected, actual)| expected == actual)
                        })
                });
            let is_static_namespace = receiver_resolves_to_namespace || is_operator_namespace;
            is_static_namespace.then(|| {
                receiver
                    .iter()
                    .map(|segment| segment.as_str())
                    .collect::<Vec<_>>()
            })
        }

        /*
            A named call can denote a family of machines that share one path
            and differ in their operands, such as `float::to_bits(f32)` and
            `float::to_bits(f64)`. Symbol resolution leaves such a call without
            a unique target, and checking selects the member from the operand
            and result types. Two readers need the family before that
            selection: overload selection itself, and the declaration walker
            that decides which library declarations the checked program keeps.
            Both ask here, so a member the selector may choose is never one
            the projection already dropped.

            The path is the authored target, qualified by its static receiver
            (a module, type, domain, trait or machine). A value receiver names
            no declaration family. The source span only places the reference
            in its file, which decides which imports and namespaces it sees.
        */
        pub fn named_call_family_path<'name>(
            program: &TypedTrees,
            target: &'name str,
            receiver_missing: bool,
            receiver: SymbolHandle,
        ) -> Option<std::borrow::Cow<'name, str>> {
            if receiver_missing {
                return Some(std::borrow::Cow::Borrowed(target));
            }
            if !receiver.is_valid()
                || !matches!(
                    program.symbols.get(receiver).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Trait
                        | symbols::SymbolKind::Machine
                )
            {
                return None;
            }
            Some(std::borrow::Cow::Owned(format!(
                "{}::{target}",
                program.symbols.display_path(receiver, "::")
            )))
        }

        pub fn named_expression_call_family_path<'call>(
            program: &TypedTrees,
            call: &'call crate::expression::TableCallExpression,
        ) -> Option<std::borrow::Cow<'call, str>> {
            if call.runtime_call.is_some() {
                return None;
            }
            let receiver = if call.receiver.is_valid() {
                match program.expression_table.expression(call.receiver) {
                    crate::expression::ExpressionNode::Name(path) => {
                        path.resolved_symbol().unwrap_or_else(SymbolHandle::invalid)
                    }
                    _ => SymbolHandle::invalid(),
                }
            } else {
                SymbolHandle::invalid()
            };
            named_call_family_path(
                program,
                call.target.as_str(),
                !call.receiver.is_valid(),
                receiver,
            )
        }

        pub fn named_statement_call_family_path<'call>(
            program: &TypedTrees,
            call: &'call crate::statement::TableCall,
        ) -> Option<std::borrow::Cow<'call, str>> {
            if call.runtime_call.is_some() {
                return None;
            }
            named_call_family_path(
                program,
                call.target.as_str(),
                call.receiver.is_empty(),
                call.receiver_symbol,
            )
        }

        /// A named transition target names a family by its single unqualified
        /// member while symbol resolution has left it without a target.
        pub fn named_transition_family_path(
            program: &TypedTrees,
            target: crate::statement::TransitionTargetHandle,
        ) -> Option<(&str, source::SourceSpan)> {
            let crate::statement::TransitionTargetNode::Named {
                path, source_span, ..
            } = program.statement_table.transition_target(target)
            else {
                return None;
            };
            if path.symbol.is_valid() {
                return None;
            }
            let [name] = program.statement_table.name_path_members(path.members) else {
                return None;
            };
            Some((name.as_str(), *source_span))
        }

        /// Every machine the family path names from `reference`, after
        /// namespace and import precedence, before operand selection.
        pub fn named_call_family_members(
            program: &TypedTrees,
            path: &str,
            reference: source::SourceSpan,
        ) -> Vec<SymbolHandle> {
            program.symbols.top_level_candidates_from_source(
                path,
                &[symbols::SymbolKind::Machine],
                reference,
            )
        }
    }
    pub(crate) mod operand_signatures {
        use crate::TypedTrees;
        use crate::declarations::operator::OperatorDefinition;
        use crate::type_identity::TypeIdentityRequest;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::SymbolHandle;

        pub fn operator_operand_signature(
            program: &TypedTrees,
            operator: &OperatorDefinition,
        ) -> Result<String, crate::runtime_callable_identity::RuntimeCallableIdentityError>
        {
            operand_signature(
                program,
                program
                    .operator_type_parameters(operator)
                    .iter()
                    .map(|parameter| parameter.symbol),
                program.operator_parameters(operator),
            )
        }

        pub(crate) fn operand_signature(
            program: &TypedTrees,
            declared: impl Iterator<Item = SymbolHandle>,
            parameters: &[crate::signature::StateParameter],
        ) -> Result<String, crate::runtime_callable_identity::RuntimeCallableIdentityError>
        {
            let mut normalizer = TypeParameterNormalizer::new(declared.collect());
            let callable_operands = normalized_operand_parameters(parameters)
                .map(|parameter| {
                    collect_type_parameter_occurrences(
                        program,
                        parameter.type_reference,
                        &mut normalizer,
                    )
                })
                .collect::<Vec<_>>();
            let binders = normalizer.bindings();
            normalized_operand_parameters(parameters)
                .zip(callable_operands)
                .map(|(parameter, contains_callable)| {
                    if !program.contains_type_reference(parameter.type_reference) {
                        return Err(crate::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedType);
                    }
                    let request = TypeIdentityRequest {
                        binders: &binders,
                        ..TypeIdentityRequest::ordinary(parameter.type_reference)
                    };
                    if contains_callable {
                        program.runtime_callable_operand_shape(request)
                    } else {
                        Ok(program.type_identity_request_text(request))
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|operands| operands.join(", "))
        }

        pub(crate) fn normalized_operand_parameters(
            parameters: &[crate::signature::StateParameter],
        ) -> impl Iterator<Item = &crate::signature::StateParameter> {
            parameters.iter()
        }

        // Follow free occurrences in the callable's operand telescope, not its
        // predicates or effects. Its own declarations shadow exact symbols;
        // surrounding binders keep the order of their first free occurrence.
        pub(crate) fn collect_type_parameter_occurrences(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            normalizer: &mut TypeParameterNormalizer,
        ) -> bool {
            let mut occurrences = OperandOccurrences {
                program,
                normalizer,
                active_types: Vec::new(),
                active_expressions: Vec::new(),
                active_signatures: Vec::new(),
                contains_callable: false,
            };
            occurrences.reference(type_reference);
            occurrences.contains_callable
        }

        struct OperandOccurrences<'program, 'normalizer> {
            program: &'program TypedTrees,
            normalizer: &'normalizer mut TypeParameterNormalizer,
            active_types: Vec<TypeReferenceHandle>,
            active_expressions: Vec<crate::expression::ExpressionHandle>,
            active_signatures: Vec<&'program crate::signature::StateSignature>,
            contains_callable: bool,
        }

        impl<'program> OperandOccurrences<'program, '_> {
            fn reference(&mut self, reference: TypeReferenceHandle) {
                let program = self.program;
                if !program.contains_type_reference(reference)
                    || self.active_types.contains(&reference)
                {
                    return;
                }
                self.active_types.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Callable { contract } => {
                        self.contains_callable = true;
                        if program.runtime_callable_contracts.is_valid(*contract) {
                            self.signature(
                                &program.runtime_callable_contracts.get(*contract).signature,
                            );
                        }
                    }
                    TypeReferenceNode::Reference { referee, .. } => self.reference(*referee),
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        self.reference(*base_type);
                        for constraint in program.type_reference_table.constraints(*constraints) {
                            if let crate::types::TypeConstraintNode::Domain(domain) = constraint {
                                self.normalizer.canonical_index(domain.symbol);
                                for argument in &domain.arguments {
                                    self.reference(*argument);
                                }
                            }
                        }
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        self.reference(*element_type);
                        match length {
                            crate::types::FixedArrayLength::Literal(_) => {}
                            crate::types::FixedArrayLength::ConstParameter { symbol, .. } => {
                                self.normalizer.canonical_index(*symbol);
                            }
                            crate::types::FixedArrayLength::ConstCall { machine, .. } => {
                                self.normalizer.canonical_index(*machine);
                            }
                            crate::types::FixedArrayLength::Expression(value) => {
                                self.reference(*value)
                            }
                        }
                    }
                    TypeReferenceNode::Slice { element_type } => self.reference(*element_type),
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    }
                    | TypeReferenceNode::DynamicTrait {
                        symbol: base_symbol,
                        arguments,
                        ..
                    } => {
                        self.normalizer.canonical_index(*base_symbol);
                        for argument in program.type_reference_handles(*arguments) {
                            self.reference(*argument);
                        }
                    }
                    TypeReferenceNode::Named { symbol, .. } => {
                        self.normalizer.canonical_index(*symbol);
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        crate::references::visit_expression(program, self, *expression);
                    }
                    TypeReferenceNode::Unit => {}
                }
                let extent = program
                    .type_reference_table
                    .array_extent_argument(reference);
                if extent.is_valid() {
                    self.reference(extent);
                }
                for application in program
                    .type_reference_table
                    .type_alias_applications(reference)
                {
                    for argument in program.type_reference_handles(application.arguments) {
                        self.reference(*argument);
                    }
                }
                self.active_types.pop();
            }

            fn signature(&mut self, signature: &'program crate::signature::StateSignature) {
                let program = self.program;
                if self
                    .active_signatures
                    .iter()
                    .any(|active| std::ptr::eq(*active, signature))
                {
                    return;
                }
                self.active_signatures.push(signature);
                let generics = program.state_signature_type_parameters(signature);
                let parameters = program.state_signature_parameters(signature);
                let previous_mask = self.normalizer.masked.len();
                self.normalizer
                    .masked
                    .extend(generics.iter().map(|parameter| parameter.symbol));
                self.normalizer
                    .masked
                    .extend(parameters.iter().map(|parameter| parameter.symbol));
                self.normalizer.masked.extend(
                    signature
                        .conformance_bounds
                        .iter()
                        .map(|bound| bound.binder),
                );
                for generic in generics {
                    use crate::data::{MachineParameterContract, TypeParameterKind};
                    match &generic.kind {
                        TypeParameterKind::Type | TypeParameterKind::Implementation => {}
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => {
                            self.reference(*type_reference)
                        }
                        TypeParameterKind::Machine { contract } => match contract {
                            MachineParameterContract::Structural(nested) => self.signature(nested),
                            MachineParameterContract::Nominal {
                                instantiated: Some(nested),
                                ..
                            } => self.signature(nested),
                            MachineParameterContract::Nominal {
                                receiving_arguments,
                                trait_arguments,
                                ..
                            } => {
                                for argument in receiving_arguments.iter().chain(trait_arguments) {
                                    self.reference(*argument);
                                }
                            }
                            MachineParameterContract::RequirementIdentity => {}
                        },
                        TypeParameterKind::Proposition { contract } => {
                            let formals =
                                program.state_parameters.span_or_empty(contract.parameters);
                            let previous = self.normalizer.masked.len();
                            self.normalizer
                                .masked
                                .extend(formals.iter().map(|formal| formal.symbol));
                            for formal in formals {
                                self.reference(formal.type_reference);
                            }
                            self.normalizer.masked.truncate(previous);
                        }
                    }
                }
                for parameter in parameters {
                    self.reference(parameter.type_reference);
                }
                self.reference(signature.return_type);
                self.normalizer.masked.truncate(previous_mask);
                self.active_signatures.pop();
            }
        }

        impl Extend<SymbolHandle> for OperandOccurrences<'_, '_> {
            fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
                for symbol in symbols {
                    self.normalizer.canonical_index(symbol);
                }
            }
        }

        impl crate::references::TypedReferences for OperandOccurrences<'_, '_> {
            fn derived_declaration_references(&self) -> bool {
                false
            }
            fn callable_contract(
                &mut self,
                _: crate::callable_values::RuntimeCallableContractHandle,
            ) -> bool {
                false
            }
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                self.reference(reference);
                false
            }
            fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
                if self.active_expressions.contains(&expression) {
                    return false;
                }
                self.active_expressions.push(expression);
                true
            }
            fn leave_expression(&mut self, _: crate::expression::ExpressionHandle) {
                self.active_expressions.pop();
            }
        }

        pub(crate) struct TypeParameterNormalizer {
            declared: Vec<SymbolHandle>,
            canonical: Vec<(SymbolHandle, usize)>,
            masked: Vec<SymbolHandle>,
        }

        impl TypeParameterNormalizer {
            pub(crate) fn new(declared: Vec<SymbolHandle>) -> Self {
                Self {
                    declared,
                    canonical: Vec::new(),
                    masked: Vec::new(),
                }
            }

            fn canonical_index(&mut self, symbol: SymbolHandle) -> Option<usize> {
                if self.masked.contains(&symbol) || !self.declared.contains(&symbol) {
                    return None;
                }
                if let Some((_, index)) = self
                    .canonical
                    .iter()
                    .find(|(candidate, _)| *candidate == symbol)
                {
                    return Some(*index);
                }
                let index = self.canonical.len();
                self.canonical.push((symbol, index));
                Some(index)
            }

            pub(crate) fn bindings(&self) -> Vec<(SymbolHandle, String)> {
                self.canonical
                    .iter()
                    .map(|(symbol, index)| (*symbol, format!("${index}")))
                    .collect()
            }
        }
    }
    pub(crate) mod primitive_float {
        use crate::{TypedTrees, expression::BinaryOperator};

        const fn operation_returns_boolean(operation: BinaryOperator) -> bool {
            matches!(
                operation,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        }

        const fn operation_spelling(
            operation: BinaryOperator,
        ) -> Option<language_core::operator_spelling::OperatorSpelling> {
            use language_core::operator_spelling::OperatorSpelling;

            Some(match operation {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                _ => return super::spellings::comparison_operator_spelling(operation),
            })
        }

        pub fn primitive_float_binary_semantics(
            typed: &TypedTrees,
            operator: &super::OperatorDefinition,
        ) -> Option<(BinaryOperator, numerics::literals::FloatFormat)> {
            let [namespace, requirement] = typed.operator_path_members(operator.name) else {
                return None;
            };
            if namespace.as_str() != "Float" {
                return None;
            }
            let operation = match requirement.as_str() {
                "add" => BinaryOperator::Add,
                "subtract" => BinaryOperator::Subtract,
                "multiply" => BinaryOperator::Multiply,
                "divide" => BinaryOperator::Divide,
                "equal" => BinaryOperator::Equal,
                "not_equal" => BinaryOperator::NotEqual,
                "less" => BinaryOperator::Less,
                "less_or_equal" => BinaryOperator::LessOrEqual,
                "greater" => BinaryOperator::Greater,
                "greater_or_equal" => BinaryOperator::GreaterOrEqual,
                _ => return None,
            };
            if operator.spelling != operation_spelling(operation) {
                return None;
            }
            let [left, right] = typed.operator_parameters(operator) else {
                return None;
            };
            let primitive = typed.primitive_type_reference(left.type_reference)?;
            if typed.primitive_type_reference(right.type_reference) != Some(primitive) {
                return None;
            }
            let format = match primitive {
                crate::types::PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
                crate::types::PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
                _ => return None,
            };
            let expected_result = if operation_returns_boolean(operation) {
                crate::types::PrimitiveType::Bool
            } else {
                primitive
            };
            if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
                return None;
            }
            Some((operation, format))
        }

        pub fn is_sealed_toolchain_float_operator(
            typed: &TypedTrees,
            operator: &super::OperatorDefinition,
        ) -> bool {
            let [namespace, name] = typed.operator_path_members(operator.name) else {
                return false;
            };
            typed
                .normalized_hermetic_symbol_identity(operator.symbol)
                .is_ok_and(|identity| {
                    identity == format!("toolchain::{}::{}", namespace.as_str(), name.as_str())
                })
                && typed.symbols.core_source(operator.symbol)
                    == Some(language_core::core_vocabulary::CoreSource::FloatOperations)
        }

        pub(crate) fn is_default_primitive_operator(
            typed: &TypedTrees,
            operator: &super::OperatorDefinition,
        ) -> bool {
            operator.is_boundary
                && operator.is_public
                && !operator.home_domain.is_valid()
                && is_sealed_toolchain_float_operator(typed, operator)
                && primitive_float_binary_semantics(typed, operator).is_some()
        }
    }
    pub(crate) mod primitive_integer {
        use crate::expression::BinaryOperator;
        use crate::types::PrimitiveType;

        pub fn builtin_integer_result_carrier(
            operator: BinaryOperator,
            [left, right]: [PrimitiveType; 2],
        ) -> Option<PrimitiveType> {
            use PrimitiveType::Addr;
            const U64: PrimitiveType = PrimitiveType::U64;
            if !left.accepts_integer_literal() || !right.accepts_integer_literal() {
                return None;
            }
            if left == Addr || right == Addr {
                return match (operator, left, right) {
                    (BinaryOperator::Add, Addr, U64)
                    | (BinaryOperator::Add, U64, Addr)
                    | (BinaryOperator::Subtract, Addr, U64) => Some(Addr),
                    (BinaryOperator::Subtract, Addr, Addr) => Some(U64),
                    _ => None,
                };
            }
            match operator {
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => Some(left),
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                    if left == right =>
                {
                    Some(left)
                }
                _ => None,
            }
        }

        pub fn integer_peer_operand_carrier(
            operator: BinaryOperator,
            peer: PrimitiveType,
            right_operand: bool,
        ) -> PrimitiveType {
            if peer == PrimitiveType::Addr
                && (operator == BinaryOperator::Add
                    || (operator == BinaryOperator::Subtract && right_operand))
            {
                PrimitiveType::U64
            } else {
                peer
            }
        }
    }
    pub(crate) mod satisfied_operators {
        use crate::TypedTrees;
        use crate::declarations::operator::applications::{
            ClosedOperatorRealizationApplication, closed_operator_realization_application,
        };
        use crate::declarations::operator::{OperatorDefinition, declaration_by_symbol};
        use crate::type_identity::TypeIdentityRequest;
        use symbols::SymbolHandle;

        pub fn resolve_satisfied_boundary_operator_for_conformance<'program>(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
            conformance: &'program crate::machine::TraitConformance,
        ) -> Option<&'program OperatorDefinition> {
            resolve_satisfied_operator_for_conformance(program, machine, conformance, true)
        }

        pub fn resolve_satisfied_checked_operator_for_conformance<'program>(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
            conformance: &'program crate::machine::TraitConformance,
        ) -> Option<&'program OperatorDefinition> {
            resolve_satisfied_operator_for_conformance(program, machine, conformance, false)
        }

        fn resolve_satisfied_operator_for_conformance<'program>(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
            conformance: &'program crate::machine::TraitConformance,
            boundary_only: bool,
        ) -> Option<&'program OperatorDefinition> {
            let requirement = conformance.requirement.as_ref()?;
            let candidates = satisfied_operator_candidates(
                program,
                machine,
                conformance.name.as_str(),
                requirement.as_str(),
                boundary_only,
            )
            .collect::<Vec<_>>();
            if conformance.requirement_symbol.is_valid() {
                return candidates
                    .into_iter()
                    .find(|operator| operator.symbol == conformance.requirement_symbol);
            }
            let path = format!("{}::{}", conformance.name, requirement);
            let selected = program
                .symbols
                .lookup_signature_free_top_level_from_source_matching(
                    &path,
                    &[symbols::SymbolKind::Machine, symbols::SymbolKind::Operator],
                    conformance.requirement_source_span.unwrap_or_default(),
                    |symbol| {
                        candidates
                            .iter()
                            .any(|candidate| candidate.symbol == symbol)
                    },
                )
                .unique()?;
            candidates
                .into_iter()
                .find(|candidate| candidate.symbol == selected)
        }

        pub fn resolve_specialized_checked_operator_application<'program>(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
            namespace: &str,
            requirement: &str,
        ) -> Option<(
            &'program OperatorDefinition,
            &'program ClosedOperatorRealizationApplication,
        )> {
            let mut specializations = program
                .machine_specializations
                .iter()
                .filter(|specialization| specialization.instance == machine.symbol);
            let specialization = specializations.next()?;
            if specializations.next().is_some() {
                return None;
            }
            let mut rows = specialization
                .operator_realizations
                .iter()
                .filter_map(|row| {
                    let operator = declaration_by_symbol(program, row.requirement_symbol)?;
                    operator_path_matches(operator, program, namespace, requirement)
                        .then_some((operator, row))
                });
            let (operator, retained) = rows.next()?;
            if rows.next().is_some()
                || closed_operator_realization_application(program, machine, operator).as_ref()
                    != Some(retained)
            {
                return None;
            }
            Some((operator, retained))
        }

        fn satisfied_operator_candidates<'program>(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
            namespace: &str,
            requirement: &str,
            boundary_only: bool,
        ) -> impl Iterator<Item = &'program OperatorDefinition> {
            let state = program.machine_states(machine).first();
            let actual_parameters = state.map(|state| program.state_parameters(state));
            program
                .operators()
                .iter()
                .chain(program.machine_token_bindings())
                .filter(move |operator| {
                    let (Some(state), Some(actual_parameters)) = (state, actual_parameters) else {
                        return false;
                    };
                    if (boundary_only && !operator.is_boundary)
                        || !operator_path_matches(operator, program, namespace, requirement)
                    {
                        return false;
                    }
                    let Some((machine_binders, operator_binders)) =
                        operator_realization_static_binders(program, machine, operator)
                    else {
                        return false;
                    };
                    let required_parameters = program.operator_parameters(operator);
                    actual_parameters.len() == required_parameters.len()
                        && actual_parameters
                            .iter()
                            .zip(required_parameters.iter())
                            .all(|(actual, required)| {
                                actual.is_const == required.is_const
                                    && actual.is_mutable == required.is_mutable
                                    && program.type_identity(TypeIdentityRequest {
                                        binders: &machine_binders,
                                        ..TypeIdentityRequest::ordinary(actual.type_reference)
                                    }) == program.type_identity(TypeIdentityRequest {
                                        binders: &operator_binders,
                                        ..TypeIdentityRequest::ordinary(required.type_reference)
                                    })
                            })
                        && state.return_type.is_valid() == operator.return_type.is_valid()
                        && (!state.return_type.is_valid()
                            || program.type_identity(TypeIdentityRequest {
                                binders: &machine_binders,
                                ..TypeIdentityRequest::ordinary(state.return_type)
                            }) == program.type_identity(TypeIdentityRequest {
                                binders: &operator_binders,
                                ..TypeIdentityRequest::ordinary(operator.return_type)
                            }))
                })
        }

        fn operator_realization_static_binders(
            program: &TypedTrees,
            machine: &crate::machine::Machine,
            operator: &OperatorDefinition,
        ) -> Option<(Vec<(SymbolHandle, String)>, Vec<(SymbolHandle, String)>)> {
            let machine_parameters = program.machine_type_parameters(machine);
            let operator_parameters = program.operator_type_parameters(operator);
            if machine_parameters.len() != operator_parameters.len() {
                return None;
            }
            let machine_binders = machine_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
                .collect::<Vec<_>>();
            let operator_binders = operator_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
                .collect::<Vec<_>>();
            for (machine_parameter, operator_parameter) in
                machine_parameters.iter().zip(operator_parameters)
            {
                match (&machine_parameter.kind, &operator_parameter.kind) {
                    (
                        crate::data::TypeParameterKind::Type,
                        crate::data::TypeParameterKind::Type,
                    ) if !crate::data::type_parameter_demands_stronger_properties(
                        operator_parameter,
                        machine_parameter,
                    ) => {}
                    (
                        crate::data::TypeParameterKind::Const {
                            type_reference: machine_carrier,
                        },
                        crate::data::TypeParameterKind::Const {
                            type_reference: operator_carrier,
                        },
                    ) if machine_parameter.bounds == operator_parameter.bounds
                        && program.type_identity(TypeIdentityRequest {
                            binders: &machine_binders,
                            ..TypeIdentityRequest::ordinary(*machine_carrier)
                        }) == program.type_identity(TypeIdentityRequest {
                            binders: &operator_binders,
                            ..TypeIdentityRequest::ordinary(*operator_carrier)
                        }) => {}
                    _ => return None,
                }
            }
            Some((machine_binders, operator_binders))
        }

        fn operator_path_matches(
            operator: &OperatorDefinition,
            program: &TypedTrees,
            namespace: &str,
            requirement: &str,
        ) -> bool {
            let path = program.operator_path_members(operator.name);
            matches!(path, [owner, member]
        if owner.as_str() == namespace && member.as_str() == requirement)
        }

        pub fn boundary_operator_requirement_identity(
            program: &TypedTrees,
            operator: &OperatorDefinition,
        ) -> String {
            if let Some(requirement) = program.machine(operator.symbol) {
                return program
                    .machine_callable_identity_text(requirement)
                    .unwrap_or_default();
            }
            let path = program.operator_path_string(operator.name);
            let parameters = program
                .operator_parameters(operator)
                .iter()
                .map(|parameter| program.normalized_type_identity_text(parameter.type_reference))
                .collect::<Vec<_>>()
                .join(",");
            let result = operator
                .return_type
                .is_valid()
                .then(|| program.normalized_type_identity_text(operator.return_type));
            format!(
                "operator::{path}({parameters})->{}",
                result.as_deref().unwrap_or("unit")
            )
        }
    }
    pub(crate) mod spellings {
        use crate::TypedTrees;
        use crate::declarations::operator::operand_signatures::normalized_operand_parameters;
        use crate::declarations::operator::type_matching::type_reference_matches;
        use crate::declarations::operator::{
            OperatorConstBinding, OperatorDefinition, SpelledOperator,
            selected_trait_operator_meaning_exists, trait_scope_operator_meaning_exists,
        };
        use crate::expression::BinaryOperator;
        use crate::operator::indexing;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;

        pub fn resolve_spelling<'program>(
            program: &'program TypedTrees,
            spelling: OperatorSpelling,
            receiver_type: Option<TypeReferenceHandle>,
            use_site: source::SourceSpan,
        ) -> Vec<SpelledOperator<'program>> {
            spelled_candidates(program, spelling, receiver_type, use_site).collect()
        }

        fn spelled_candidates<'program>(
            program: &'program TypedTrees,
            spelling: OperatorSpelling,
            receiver_type: Option<TypeReferenceHandle>,
            use_site: source::SourceSpan,
        ) -> impl Iterator<Item = SpelledOperator<'program>> + 'program {
            use crate::declaration_index::OperatorSpellingRow;
            program
                .operator_spelling_rows(spelling)
                .into_iter()
                .flat_map(move |rows| {
                    (0..rows.len()).map(move |position| match rows[position] {
                        OperatorSpellingRow::Root(handle) => {
                            let operator = program.operators.get(handle);
                            SpelledOperator {
                                operator,
                                domain: operator
                                    .home_domain
                                    .is_valid()
                                    .then(|| {
                                        crate::domain::domain_by_symbol(
                                            program,
                                            operator.home_domain,
                                        )
                                    })
                                    .flatten(),
                            }
                        }
                        OperatorSpellingRow::Domain { operator, domain } => SpelledOperator {
                            operator: program.operators.get(operator),
                            domain: Some(program.domain_definitions.get(domain)),
                        },
                    })
                })
                .filter(move |candidate| {
                    program
                        .symbols
                        .source_reference_may_select_symbol(use_site, candidate.operator.symbol)
                        || (program
                            .symbols
                            .source_reference_can_see_symbol(use_site, candidate.operator.symbol)
                            && super::primitive_float::is_default_primitive_operator(
                                program,
                                candidate.operator,
                            ))
                })
                .filter(move |candidate| match receiver_type {
                    Some(receiver_type) => {
                        operator_matches_receiver(program, candidate.operator, receiver_type)
                    }
                    None => true,
                })
        }

        pub fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
            Some(match operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return comparison_operator_spelling(operator),
            })
        }

        pub fn arithmetic_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
            Some(match operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return None,
            })
        }

        pub const fn comparison_operator_spelling(
            operator: BinaryOperator,
        ) -> Option<OperatorSpelling> {
            Some(match operator {
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                _ => return None,
            })
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum OperandType {
            Reference(TypeReferenceHandle),
            Primitive(crate::types::PrimitiveType),
            Unknown,
        }

        impl OperandType {
            pub fn from_reference(type_reference: Option<TypeReferenceHandle>) -> Self {
                type_reference.map_or(Self::Unknown, Self::Reference)
            }

            pub fn is_unknown(self) -> bool {
                matches!(self, Self::Unknown)
            }
        }

        pub fn resolve_spelling_for_operands<'program>(
            program: &'program TypedTrees,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
            use_site: source::SourceSpan,
        ) -> Vec<SpelledOperator<'program>> {
            resolve_spelling_for_operand_types(
                program,
                spelling,
                &operand_types
                    .iter()
                    .copied()
                    .map(OperandType::from_reference)
                    .collect::<Vec<_>>(),
                use_site,
            )
        }

        pub fn resolve_spelling_for_operand_types<'program>(
            program: &'program TypedTrees,
            spelling: OperatorSpelling,
            operand_types: &[OperandType],
            use_site: source::SourceSpan,
        ) -> Vec<SpelledOperator<'program>> {
            spelled_candidates(program, spelling, None, use_site)
                .filter(|candidate| {
                    operator_matches_operands(program, candidate.operator, operand_types)
                })
                .collect()
        }

        pub fn has_builtin_spelled_expression_meaning(
            program: &TypedTrees,
            owner_symbol: SymbolHandle,
            expression: crate::expression::ExpressionHandle,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> bool {
            let machine_symbol = program
                .machine_holding_state(owner_symbol)
                .map_or(owner_symbol, |machine| machine.symbol);
            let abstract_trait_meaning = owner_symbol.is_valid() && {
                let parent = program.symbols.get(owner_symbol).parent;
                parent.is_valid()
                    && program.symbols.symbols().nodes().is_valid(parent)
                    && program.symbols.get(parent).kind == symbols::SymbolKind::Trait
                    && program
                        .trait_definition_by_symbol(parent)
                        .is_some_and(|definition| {
                            program
                                .trait_machine_signature_by_symbol(definition, owner_symbol)
                                .is_some()
                                && trait_scope_operator_meaning_exists(
                                    program,
                                    definition,
                                    spelling,
                                    operand_types,
                                )
                        })
            };
            let anonymous_operands = operand_types.iter().all(Option::is_none)
                && program
                    .expression_table
                    .binary_node(expression)
                    .is_some_and(|binary| {
                        crate::closed_numeric::has_anonymous_numeric_results(program, binary.left)
                            && crate::closed_numeric::has_anonymous_numeric_results(
                                program,
                                binary.right,
                            )
                    });
            (anonymous_operands
                || !spelling_for_operands_exists(
                    program,
                    spelling,
                    &operand_types
                        .iter()
                        .copied()
                        .map(OperandType::from_reference)
                        .collect::<Vec<_>>(),
                    program.expression_table.source_span(expression),
                ))
                && !abstract_trait_meaning
                && !selected_trait_operator_meaning_exists(
                    program,
                    machine_symbol,
                    spelling,
                    operand_types,
                )
                && super::bound_trait_operator_meanings(
                    program,
                    machine_symbol,
                    spelling,
                    operand_types,
                )
                .is_empty()
                && !super::trait_operators::abstract_requirement_may_have_operator_bound(
                    program,
                    owner_symbol,
                    spelling,
                )
                && crate::closed_numeric::has_builtin_anonymous_operands(
                    program, expression, spelling,
                )
        }

        fn spelling_for_operands_exists(
            program: &TypedTrees,
            spelling: OperatorSpelling,
            operand_types: &[OperandType],
            use_site: source::SourceSpan,
        ) -> bool {
            spelled_candidates(program, spelling, None, use_site).any(|candidate| {
                operator_matches_operands(program, candidate.operator, operand_types)
            })
        }

        fn operator_matches_operands(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[OperandType],
        ) -> bool {
            operator_matches_operands_with_indexed_collection(
                program,
                operator,
                operand_types,
                false,
            )
        }

        #[derive(Debug, Clone, Default)]
        pub struct OperatorOperandBindings {
            pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub constants: Vec<OperatorConstBinding>,
        }

        pub fn operator_operand_bindings(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Option<OperatorOperandBindings> {
            let operands = operand_types
                .iter()
                .copied()
                .map(OperandType::from_reference)
                .collect::<Vec<_>>();
            operand_bindings_with_indexed_collection(program, operator, &operands, false)
        }

        pub(crate) fn operator_matches_operands_with_indexed_collection(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[OperandType],
            indexed_collection: bool,
        ) -> bool {
            operand_bindings_with_indexed_collection(
                program,
                operator,
                operand_types,
                indexed_collection,
            )
            .is_some()
        }

        fn operand_bindings_with_indexed_collection(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            operand_types: &[OperandType],
            indexed_collection: bool,
        ) -> Option<OperatorOperandBindings> {
            let parameters = program.operator_parameters(operator);
            if parameters.len() != operand_types.len() {
                return None;
            }
            let type_parameters = program.operator_type_parameters(operator);
            let mut bindings = Vec::new();
            let mut const_bindings = Vec::new();
            let matches = operand_types
                .iter()
                .zip(normalized_operand_parameters(parameters))
                .enumerate()
                .all(|(position, (actual, expected))| {
                    let actual = match *actual {
                        OperandType::Unknown => return true,
                        OperandType::Primitive(primitive) => {
                            return program
                                .type_reference_table
                                .primitive_type(expected.type_reference)
                                .is_none_or(|expected| expected == primitive);
                        }
                        OperandType::Reference(actual) => actual,
                    };
                    let (matched_actual, matched_expected) = if indexed_collection && position == 0
                    {
                        indexing::shared_collection_elements(
                            program,
                            actual,
                            expected.type_reference,
                        )
                        .unwrap_or((actual, expected.type_reference))
                    } else {
                        (
                            scalar_operand_read_type(program, actual, expected.type_reference),
                            expected.type_reference,
                        )
                    };
                    (type_reference_matches(
                        program,
                        matched_actual,
                        matched_expected,
                        None,
                        type_parameters,
                        &mut bindings,
                        &mut const_bindings,
                    ) || indexed_collection_borrow_matches(
                        program,
                        indexed_collection && position == 0,
                        actual,
                        expected.type_reference,
                        type_parameters,
                        &mut bindings,
                        &mut const_bindings,
                    )) && declared_domain_constraints_match(
                        program,
                        actual,
                        expected.type_reference,
                    ) && declared_domain_constraints_match(
                        program,
                        matched_actual,
                        matched_expected,
                    )
                });
            (matches && bindings_satisfy_known_multiplicities(program, type_parameters, &bindings))
                .then_some(OperatorOperandBindings {
                    types: bindings,
                    constants: const_bindings,
                })
        }

        fn bindings_satisfy_known_multiplicities(
            program: &TypedTrees,
            parameters: &[crate::data::TypeParameter],
            bindings: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            parameters.iter().all(|parameter| {
                if !matches!(parameter.kind, crate::data::TypeParameterKind::Type)
                    || !parameter.symbol.is_valid()
                    || parameter.bounds.multiplicity == language_semantics::Multiplicity::Affine
                {
                    return true;
                }
                let Some((_, actual)) = bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                else {
                    return true;
                };
                !closed_multiplicity_carrier(program, *actual, 0)
                    || program.type_multiplicity(*actual) == parameter.bounds.multiplicity
            })
        }

        fn closed_multiplicity_carrier(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
            depth: usize,
        ) -> bool {
            if depth >= 64 || !program.contains_type_reference(reference) {
                return false;
            }
            let closed = |reference| closed_multiplicity_carrier(program, reference, depth + 1);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Callable { .. } => program
                    .runtime_callable_type_identity(
                        crate::type_identity::TypeIdentityRequest::ordinary(reference),
                    )
                    .is_ok(),
                TypeReferenceNode::Unit => true,
                TypeReferenceNode::Constrained { base_type, .. } => closed(*base_type),
                TypeReferenceNode::Reference {
                    referee, lifetime, ..
                } => lifetime.is_none() && closed(*referee),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: crate::types::FixedArrayLength::Literal(_),
                }
                | TypeReferenceNode::Slice { element_type } => closed(*element_type),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::ConstExpression(_) => {
                    false
                }
                TypeReferenceNode::Named { symbol, name } => {
                    if program.data_type_parameter_by_symbol(*symbol).is_some() {
                        return false;
                    }
                    if !symbol.is_valid() {
                        return crate::types::PrimitiveType::from_name(name.as_str()).is_some();
                    }
                    if let Some(atom) = program.symbols.builtin_type_atom(*symbol) {
                        return crate::types::PrimitiveType::from_name(&atom.symbol_name())
                            .is_some()
                            && atom.symbol_name() == name.as_str();
                    }
                    if crate::types::PrimitiveType::from_name(name.as_str()).is_some() {
                        return false;
                    }
                    let Some(definition) = program.unique_data_definition_by_symbol(*symbol) else {
                        return false;
                    };
                    if let Some(origin) = definition.generic_instance {
                        return closed(origin);
                    }
                    program.data_type_parameters(definition).is_empty()
                        && definition.lifetime_parameters.is_empty()
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } => {
                    let Some(definition) = program.unique_data_definition_by_symbol(*base_symbol)
                    else {
                        return false;
                    };
                    lifetime_arguments.is_empty()
                        && definition.lifetime_parameters.is_empty()
                        && closed_multiplicity_arguments(
                            program,
                            program.data_type_parameters(definition),
                            program.type_reference_handles(*arguments),
                            depth + 1,
                        )
                }
                TypeReferenceNode::DynamicTrait {
                    symbol, arguments, ..
                } => {
                    if !symbol.is_valid() {
                        return false;
                    }
                    let mut declarations = program
                        .traits()
                        .iter()
                        .filter(|item| item.symbol == *symbol);
                    let Some(declaration) = declarations.next() else {
                        return false;
                    };
                    declarations.next().is_none()
                        && declaration.lifetime_parameters.is_empty()
                        && closed_multiplicity_arguments(
                            program,
                            program.trait_type_parameters(declaration),
                            program.type_reference_handles(*arguments),
                            depth + 1,
                        )
                }
            }
        }

        fn closed_multiplicity_arguments(
            program: &TypedTrees,
            parameters: &[crate::data::TypeParameter],
            arguments: &[TypeReferenceHandle],
            depth: usize,
        ) -> bool {
            parameters.len() == arguments.len()
                && parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| {
                        if !parameter.symbol.is_valid()
                            || !program.contains_type_reference(*argument)
                        {
                            return false;
                        }
                        match parameter.kind {
                            crate::data::TypeParameterKind::Type => {
                                closed_multiplicity_carrier(program, *argument, depth)
                            }
                            crate::data::TypeParameterKind::Const { .. }
                            | crate::data::TypeParameterKind::Value { .. } => {
                                program
                                    .static_const_literal_from_type_reference(*argument)
                                    .is_some()
                                    || matches!(
                                        program.type_reference_table.type_reference(*argument),
                                        TypeReferenceNode::ConstExpression(value)
                                            if matches!(program.expression_table.expression(*value),
                                                crate::expression::ExpressionNode::Boolean(_))
                                    )
                            }
                            _ => false,
                        }
                    })
        }

        pub fn scalar_operand_read_type(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let Some(expected_primitive) = program.primitive_type_reference(expected) else {
                return actual;
            };
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(actual)
            else {
                return actual;
            };
            if matches!(
                access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            ) && program.primitive_type_reference(*referee) == Some(expected_primitive)
            {
                *referee
            } else {
                actual
            }
        }

        fn indexed_collection_borrow_matches(
            program: &TypedTrees,
            applies: bool,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            type_parameters: &[crate::data::TypeParameter],
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
            const_bindings: &mut Vec<OperatorConstBinding>,
        ) -> bool {
            if !applies {
                return false;
            }
            let mut retry_bindings = bindings.clone();
            let mut retry_const_bindings = const_bindings.clone();
            let matched = indexing::borrowed_collection_match(
                program,
                actual,
                expected,
                type_parameters,
                &mut retry_bindings,
                &mut retry_const_bindings,
            );
            if matched {
                *bindings = retry_bindings;
                *const_bindings = retry_const_bindings;
            }
            matched
        }

        pub(crate) fn declared_domain_constraints_match(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
        ) -> bool {
            let expected_domains = declared_domain_constraints(program, expected);
            if expected_domains.is_empty() {
                return true;
            }
            let actual_domains = declared_domain_constraints(program, actual);
            expected_domains.iter().all(|expected| {
                actual_domains
                    .iter()
                    .any(|actual| declared_domain_constraint_matches(program, actual, expected))
            })
        }

        pub fn declared_domain_constraints(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Vec<&crate::types::DomainConstraint> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    declared_domain_constraints(program, *referee)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let mut domains = declared_domain_constraints(program, *base_type);
                    domains.extend(
                        program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .filter_map(|constraint| match constraint {
                                crate::types::TypeConstraintNode::Domain(domain) => Some(domain),
                                _ => None,
                            }),
                    );
                    domains
                }
                _ => program
                    .layout_storage_arguments(type_reference)
                    .map_or_else(Vec::new, |storage| {
                        declared_domain_constraints(program, storage.semantic_type)
                    }),
            }
        }

        fn declared_domain_constraint_matches(
            program: &TypedTrees,
            actual: &crate::types::DomainConstraint,
            expected: &crate::types::DomainConstraint,
        ) -> bool {
            if actual.semantic_id.is_valid() && expected.semantic_id.is_valid() {
                return actual.semantic_id == expected.semantic_id;
            }
            if !actual.symbol.is_valid() || actual.symbol != expected.symbol {
                return false;
            }
            actual.arguments.len() == expected.arguments.len()
                && actual
                    .arguments
                    .iter()
                    .zip(&expected.arguments)
                    .all(|(actual, expected)| {
                        program.normalized_type_identity(*actual)
                            == program.normalized_type_identity(*expected)
                    })
        }

        fn operator_matches_receiver(
            program: &TypedTrees,
            operator: &OperatorDefinition,
            receiver_type: TypeReferenceHandle,
        ) -> bool {
            let Some(receiver_parameter) = program.operator_parameters(operator).first() else {
                return false;
            };
            type_reference_matches(
                program,
                receiver_type,
                receiver_parameter.type_reference,
                None,
                program.operator_type_parameters(operator),
                &mut Vec::new(),
                &mut Vec::new(),
            )
        }

        #[cfg(test)]
        pub(crate) mod token_roster_tests {
            use super::*;
            use crate::SpelledDeclarationTable;
            use crate::domain::DomainDefinition;
            use crate::name::Identifier;
            use crate::signature::StateParameter;
            use arena::{Arena, Handle, HandleSpan};
            use std::sync::Arc;
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

            fn rows<T: Default>(values: Vec<T>, generation: u32) -> Arena<T> {
                Arena::from_storage(T::default(), values, None, generation, true).unwrap()
            }
            fn span<T>(start: u32, count: u32, generation: u32) -> HandleSpan<T> {
                HandleSpan::from_parts(Handle::from_parts(start, generation), count)
            }
            fn rebase<T>(old: HandleSpan<T>, generation: u32) -> HandleSpan<T> {
                if old.is_empty() {
                    old
                } else {
                    span(old.start().arena_index(), old.count(), generation)
                }
            }

            fn fixture() -> (TypedTrees, TypeReferenceHandle, TypeReferenceHandle) {
                let mut builder = SymbolTableBuilder::default();
                let u64_symbol =
                    builder.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
                let i64_symbol =
                    builder.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("i64"));
                let domains = [
                    builder.insert_root(SymbolKind::Domain, SymbolNameRef::Static("First")),
                    builder.insert_root(SymbolKind::Domain, SymbolNameRef::Static("Second")),
                ];
                let names = [
                    "RootAdd",
                    "RootMultiply",
                    "NoToken",
                    "MachineAdd",
                    "DomainAdd",
                    "DomainSubtract",
                    "DomainI64",
                ];
                let symbols = names.map(|name| {
                    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static(name))
                });
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let u64_type = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: u64_symbol,
                        name: Identifier::from("u64"),
                    });
                let i64_type = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: i64_symbol,
                        name: Identifier::from("i64"),
                    });
                let mut parameters = |reference| {
                    program.state_parameters.insert_many([
                        StateParameter {
                            type_reference: reference,
                            ..Default::default()
                        },
                        StateParameter {
                            type_reference: reference,
                            ..Default::default()
                        },
                    ])
                };
                let unsigned_parameters = parameters(u64_type);
                let signed_parameters = parameters(i64_type);
                let spellings = [
                    Some(OperatorSpelling::Add),
                    Some(OperatorSpelling::Multiply),
                    None,
                    Some(OperatorSpelling::Add),
                    Some(OperatorSpelling::Add),
                    Some(OperatorSpelling::Subtract),
                    Some(OperatorSpelling::Add),
                ];
                let operators = symbols
                    .into_iter()
                    .enumerate()
                    .map(|(position, symbol)| OperatorDefinition {
                        symbol,
                        spelling: spellings[position],
                        parameters: if position == 6 {
                            signed_parameters
                        } else {
                            unsigned_parameters
                        },
                        home_domain: if position == 3 {
                            domains[0]
                        } else {
                            SymbolHandle::invalid()
                        },
                        return_type: u64_type,
                        ..Default::default()
                    })
                    .collect();
                program.tables.operators = rows(operators, 1).into();
                program.roots.operators = span(1, 3, 1);
                program.roots.machine_token_bindings = span(4, 1, 1);
                program.tables.domain_definitions = rows(
                    vec![
                        DomainDefinition {
                            symbol: domains[0],
                            name: Identifier::from("First"),
                            target_type: u64_type,
                            operators: span(5, 2, 1),
                            ..Default::default()
                        },
                        DomainDefinition {
                            symbol: domains[1],
                            name: Identifier::from("Second"),
                            target_type: i64_type,
                            operators: span(7, 1, 1),
                            ..Default::default()
                        },
                    ],
                    1,
                )
                .into();
                program.roots.domain_definitions = span(1, 2, 1);
                program.tables.machines = rows(
                    vec![crate::machine::Machine {
                        symbol: symbols[3],
                        name: Identifier::from("MachineAdd"),
                        spelling: Some(OperatorSpelling::Add),
                        ..Default::default()
                    }],
                    1,
                )
                .into();
                program.roots.machines = span(1, 1, 1);
                (program, u64_type, i64_type)
            }

            fn original<'a>(
                program: &'a TypedTrees,
                spelling: OperatorSpelling,
                receiver: Option<TypeReferenceHandle>,
                use_site: source::SourceSpan,
            ) -> Vec<SpelledOperator<'a>> {
                let root = program
                    .operators()
                    .iter()
                    .chain(program.machine_token_bindings())
                    .filter(|operator| operator.spelling == Some(spelling))
                    .map(|operator| SpelledOperator {
                        operator,
                        domain: operator
                            .home_domain
                            .is_valid()
                            .then(|| crate::domain::domain_by_symbol(program, operator.home_domain))
                            .flatten(),
                    });
                let domains = program.domain_definitions().iter().flat_map(|domain| {
                    program
                        .domain_operators(domain)
                        .iter()
                        .filter(move |operator| operator.spelling == Some(spelling))
                        .map(move |operator| SpelledOperator {
                            operator,
                            domain: Some(domain),
                        })
                });
                root.chain(domains)
                    .filter(|candidate| {
                        program
                            .symbols
                            .source_reference_may_select_symbol(use_site, candidate.operator.symbol)
                            || (program.symbols.source_reference_can_see_symbol(
                                use_site,
                                candidate.operator.symbol,
                            ) && super::super::primitive_float::is_default_primitive_operator(
                                program,
                                candidate.operator,
                            ))
                    })
                    .filter(|candidate| {
                        receiver.is_none_or(|receiver| {
                            operator_matches_receiver(program, candidate.operator, receiver)
                        })
                    })
                    .collect()
            }
            fn identities(rows: &[SpelledOperator<'_>]) -> Vec<(usize, Option<usize>)> {
                rows.iter()
                    .map(|row| {
                        (
                            std::ptr::from_ref(row.operator).addr(),
                            row.domain.map(|domain| std::ptr::from_ref(domain).addr()),
                        )
                    })
                    .collect()
            }
            fn compare(
                program: &TypedTrees,
                unsigned: TypeReferenceHandle,
                signed: TypeReferenceHandle,
            ) {
                let site = source::SourceSpan::default();
                for symbol in program
                    .domain_definitions()
                    .iter()
                    .map(|domain| domain.symbol)
                    .chain(
                        program
                            .machine_token_bindings()
                            .iter()
                            .map(|row| row.home_domain),
                    )
                {
                    let expected = symbol
                        .is_valid()
                        .then(|| {
                            program
                                .domain_definitions()
                                .iter()
                                .position(|row| row.symbol == symbol)
                        })
                        .flatten();
                    assert_eq!(program.domain_position_by_symbol(symbol), expected);
                }
                let names = program.with_domain_candidate_index(
                    |index| {
                        for (position, row) in program.domain_definitions().iter().enumerate() {
                            index
                                .by_qualified
                                .entry(row.name.as_str().into())
                                .or_default()
                                .push(position as u32);
                        }
                    },
                    |index| {
                        let mut names = index
                            .by_qualified
                            .iter()
                            .flat_map(|(name, positions)| {
                                positions.iter().map(|position| (*position, name.clone()))
                            })
                            .collect::<Vec<_>>();
                        names.sort_by_key(|(position, _)| *position);
                        names.into_iter().map(|(_, name)| name).collect::<Vec<_>>()
                    },
                );
                assert_eq!(
                    names,
                    program
                        .domain_definitions()
                        .iter()
                        .map(|row| Box::<str>::from(row.name.as_str()))
                        .collect::<Vec<_>>()
                );
                for spelling in OperatorSpelling::ALL {
                    for receiver in [None, Some(unsigned), Some(signed)] {
                        assert_eq!(
                            identities(&resolve_spelling(program, spelling, receiver, site)),
                            identities(&original(program, spelling, receiver, site))
                        );
                    }
                    for operands in [
                        [OperandType::Unknown, OperandType::Unknown],
                        [
                            OperandType::Reference(unsigned),
                            OperandType::Reference(unsigned),
                        ],
                        [
                            OperandType::Reference(signed),
                            OperandType::Reference(signed),
                        ],
                        [
                            OperandType::Primitive(crate::types::PrimitiveType::U64),
                            OperandType::Unknown,
                        ],
                    ] {
                        let expected = original(program, spelling, None, site)
                            .into_iter()
                            .filter(|candidate| {
                                operator_matches_operands(program, candidate.operator, &operands)
                            })
                            .collect::<Vec<_>>();
                        let actual =
                            resolve_spelling_for_operand_types(program, spelling, &operands, site);
                        assert_eq!(identities(&actual), identities(&expected));
                        assert_eq!(
                            spelling_for_operands_exists(program, spelling, &operands, site),
                            !expected.is_empty()
                        );
                    }
                }
            }

            #[test]
            fn token_rosters_match_original_discovery_across_owned_mutations_and_recovery() {
                let (mut program, unsigned, signed) = fixture();
                compare(&program, unsigned, signed);
                let first = program
                    .operator_spelling_rows(OperatorSpelling::Add)
                    .unwrap();
                assert_eq!(first.len(), 4);
                assert!(
                    Arc::ptr_eq(
                        &first,
                        &program
                            .operator_spelling_rows(OperatorSpelling::Add)
                            .unwrap()
                    ),
                    "use-site queries share only raw rows"
                );
                let roots = program.roots.operators;
                program.roots.operators = HandleSpan::empty();
                compare(&program, unsigned, signed);
                program.roots.operators = roots;
                compare(&program, unsigned, signed);
                let domains = program.roots.domain_definitions;
                program.roots.domain_definitions = span(2, 1, 1);
                compare(&program, unsigned, signed);
                program.roots.domain_definitions = domains;
                compare(&program, unsigned, signed);
                let first = program
                    .operator_spelling_rows(OperatorSpelling::Add)
                    .unwrap();
                let site = source::SourceSpan::default();
                assert_ne!(
                    identities(&resolve_spelling_for_operand_types(
                        &program,
                        OperatorSpelling::Add,
                        &[OperandType::Unknown; 2],
                        site
                    )),
                    identities(&resolve_spelling_for_operand_types(
                        &program,
                        OperatorSpelling::Add,
                        &[OperandType::Reference(unsigned); 2],
                        site
                    ))
                );

                let mut machine = program.machines()[0].clone();
                machine.name = Identifier::from("DifferentBodyOwner");
                machine.spelling = Some(OperatorSpelling::Divide);
                program.tables.machines = rows(vec![machine], 1).into();
                compare(&program, unsigned, signed);
                assert!(Arc::ptr_eq(
                    &first,
                    &program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                ));
                let name = program
                    .operator_path_members
                    .insert_many([Identifier::from("RenamedView")]);
                program
                    .tables
                    .operators
                    .get_mut(Handle::from_parts(4, 1))
                    .name = name;
                compare(&program, unsigned, signed);
                assert!(!Arc::ptr_eq(
                    &first,
                    &program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                ));

                program.roots.machine_token_bindings = program.roots.operators;
                compare(&program, unsigned, signed);
                assert_eq!(
                    program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                        .len(),
                    4
                );
                let domain = program.roots.domain_definitions.start();
                program.tables.domain_definitions.get_mut(domain).operators = span(5, 1, 1);
                compare(&program, unsigned, signed);

                let mut branch = program.clone();
                branch
                    .tables
                    .operators
                    .get_mut(Handle::from_parts(1, 1))
                    .spelling = Some(OperatorSpelling::Divide);
                compare(&branch, unsigned, signed);
                compare(&program, unsigned, signed);
                assert_ne!(
                    branch
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                        .len(),
                    program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                        .len()
                );

                let mut values = program.tables.operators.storage_view().items.to_vec();
                values[0].spelling = Some(OperatorSpelling::Divide);
                program.tables.operators = rows(values, 1).into();
                assert_eq!(program.tables.operators.revision(), 0);
                compare(&program, unsigned, signed);
                let revision = program.tables.operators.revision();
                let prior_rows = program
                    .operator_spelling_rows(OperatorSpelling::Add)
                    .unwrap();
                let mut replacement = program.tables.operators.storage_view().items.to_vec();
                replacement[0].spelling = Some(OperatorSpelling::Add);
                program.tables.operators = rows(replacement, 1).into();
                assert_eq!(program.tables.operators.revision(), revision);
                compare(&program, unsigned, signed);
                assert!(!Arc::ptr_eq(
                    &prior_rows,
                    &program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                ));
                let mut domains = program
                    .tables
                    .domain_definitions
                    .storage_view()
                    .items
                    .to_vec();
                domains[0].symbol = domains[1].symbol;
                domains[0].name = Identifier::from("Replacement");
                program.tables.domain_definitions = rows(domains, 1).into();
                assert_eq!(program.tables.domain_definitions.revision(), 0);
                compare(&program, unsigned, signed);
                let revision = program.tables.domain_definitions.revision();
                let prior_rows = program
                    .operator_spelling_rows(OperatorSpelling::Add)
                    .unwrap();
                let mut replacement = program
                    .tables
                    .domain_definitions
                    .storage_view()
                    .items
                    .to_vec();
                replacement[0].operators = span(6, 1, 1);
                replacement[0].name = Identifier::from("AnotherReplacement");
                program.tables.domain_definitions = rows(replacement, 1).into();
                assert_eq!(program.tables.domain_definitions.revision(), revision);
                compare(&program, unsigned, signed);
                assert!(!Arc::ptr_eq(
                    &prior_rows,
                    &program
                        .operator_spelling_rows(OperatorSpelling::Add)
                        .unwrap()
                ));
                let machine = program.tables.operators.get(Handle::from_parts(4, 1));
                assert!(
                    crate::domain::domain_by_symbol(&program, machine.home_domain).is_none(),
                    "another row at the old position cannot become that owner"
                );

                let values = program.tables.operators.storage_view().items.to_vec();
                program.tables.operators = rows(values, 2).into();
                compare(&program, unsigned, signed);
                assert!(resolve_spelling(&program, OperatorSpelling::Add, None, site).is_empty());
                program.roots.operators = rebase(program.roots.operators, 2);
                program.roots.machine_token_bindings =
                    rebase(program.roots.machine_token_bindings, 2);
                let root = program.roots.domain_definitions;
                for domain in program.tables.domain_definitions.span_mut_or_empty(root) {
                    domain.operators = rebase(domain.operators, 2);
                }
                compare(&program, unsigned, signed);
                assert!(!resolve_spelling(&program, OperatorSpelling::Add, None, site).is_empty());
                let values = program
                    .tables
                    .domain_definitions
                    .storage_view()
                    .items
                    .to_vec();
                program.tables.domain_definitions = rows(values, 2).into();
                compare(&program, unsigned, signed);
                program.roots.domain_definitions = rebase(program.roots.domain_definitions, 2);
                compare(&program, unsigned, signed);

                let arena = Arena::clone(&*program.tables.operators);
                let bytes =
                    compiler_storage::encode(&arena, compiler_storage::RecordLimits::default())
                        .unwrap();
                assert_eq!(
                    bytes,
                    compiler_storage::encode(
                        &program.tables.operators,
                        compiler_storage::RecordLimits::default()
                    )
                    .unwrap()
                );
                let restored: SpelledDeclarationTable<OperatorDefinition> =
                    compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default())
                        .unwrap();
                assert_eq!(restored, program.tables.operators);
                assert!(restored.storage_identity() != program.tables.operators.storage_identity());
                let domain_arena = Arena::clone(&*program.tables.domain_definitions);
                let domain_bytes = compiler_storage::encode(
                    &domain_arena,
                    compiler_storage::RecordLimits::default(),
                )
                .unwrap();
                assert_eq!(
                    domain_bytes,
                    compiler_storage::encode(
                        &program.tables.domain_definitions,
                        compiler_storage::RecordLimits::default()
                    )
                    .unwrap()
                );
                let domain_restored: SpelledDeclarationTable<DomainDefinition> =
                    compiler_storage::decode(
                        &domain_bytes,
                        compiler_storage::RecordLimits::default(),
                    )
                    .unwrap();
                assert_eq!(domain_restored, program.tables.domain_definitions);
                assert!(
                    domain_restored.storage_identity()
                        != program.tables.domain_definitions.storage_identity()
                );
                let mut cold_program = program.clone();
                cold_program.declaration_indexes = Default::default();
                let cold = compiler_storage::encode(
                    &cold_program,
                    compiler_storage::RecordLimits::default(),
                )
                .unwrap();
                compare(&cold_program, unsigned, signed);
                assert_eq!(
                    cold,
                    compiler_storage::encode(
                        &cold_program,
                        compiler_storage::RecordLimits::default()
                    )
                    .unwrap()
                );
                let mut recovered: TypedTrees =
                    compiler_storage::decode(&cold, compiler_storage::RecordLimits::default())
                        .unwrap();
                compare(&recovered, unsigned, signed);
                let replacement = rows(Vec::<OperatorDefinition>::new(), 1);
                recovered.tables.operators = replacement.into();
                compare(&recovered, unsigned, signed);
                assert!(resolve_spelling(&recovered, OperatorSpelling::Add, None, site).is_empty());
            }
        }
    }
    pub(crate) mod trait_operators {
        use crate::TypedTrees;
        use crate::declarations::operator::operand_signatures::normalized_operand_parameters;
        use crate::declarations::operator::type_matching::{
            closed_const_identity_from_type_reference, type_reference_matches_with_policy,
        };
        use crate::type_identity;
        use crate::types::TypeReferenceHandle;
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;
        use type_identity::TypeIdentityRequest;

        pub fn trait_operator_matches_application(
            program: &TypedTrees,
            trait_definition: &crate::trait_definition::TraitDefinition,
            requirement: &crate::signature::StateSignature,
            application: &crate::ClosedConformanceApplication,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> bool {
            let Some(bindings) = trait_operator_operand_bindings(
                program,
                trait_definition,
                requirement,
                operand_types,
                None,
            ) else {
                return false;
            };

            let binding_identity = |symbol| {
                bindings.types.iter().find_map(|(bound, actual)| {
                    (*bound == symbol).then(|| program.normalized_type_identity_text(*actual))
                })
            };
            application.trait_definition != trait_definition.symbol
                || program
                    .trait_type_parameters(trait_definition)
                    .iter()
                    .zip(&application.trait_arguments)
                    .all(|(parameter, expected)| {
                        binding_identity(parameter.symbol).is_none_or(|actual| actual == *expected)
                    })
        }

        pub fn trait_operator_operand_bindings(
            program: &TypedTrees,
            trait_definition: &crate::trait_definition::TraitDefinition,
            requirement: &crate::signature::StateSignature,
            operand_types: &[Option<TypeReferenceHandle>],
            application: Option<&[(SymbolHandle, TypeReferenceHandle)]>,
        ) -> Option<super::OperatorOperandBindings> {
            let parameters = program.state_signature_parameters(requirement);
            if parameters.len() != operand_types.len() || operand_types.iter().all(Option::is_none)
            {
                return None;
            }

            let type_parameters = program
                .trait_type_parameters(trait_definition)
                .iter()
                .chain(program.state_signature_type_parameters(requirement))
                .cloned()
                .collect::<Vec<_>>();
            let mut bindings = Vec::new();
            let mut const_bindings = Vec::new();
            if !operand_types
                .iter()
                .zip(normalized_operand_parameters(parameters))
                .all(|(actual, expected)| {
                    actual.is_none_or(|actual| {
                        if let Some(substitutions) = application
                            && program.normalized_type_identity(actual)
                                == program.type_identity(TypeIdentityRequest {
                                    substitutions,
                                    ..TypeIdentityRequest::ordinary(expected.type_reference)
                                })
                        {
                            return true;
                        }
                        type_reference_matches_with_policy(
                            program,
                            actual,
                            expected.type_reference,
                            None,
                            &type_parameters,
                            &mut bindings,
                            &mut const_bindings,
                            false,
                        )
                    })
                })
            {
                return None;
            }
            Some(super::OperatorOperandBindings {
                types: bindings,
                constants: const_bindings,
            })
        }

        #[derive(Debug, Clone)]
        pub struct BoundTraitOperatorMeaning<'program> {
            pub bound: &'program crate::machine::GenericConformanceBound,
            pub trait_definition: &'program crate::trait_definition::TraitDefinition,
            pub requirement: &'program crate::signature::StateSignature,
            pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub constants: Vec<super::OperatorConstBinding>,
        }

        impl BoundTraitOperatorMeaning<'_> {
            pub fn result_identity(
                &self,
                program: &TypedTrees,
            ) -> type_identity::NormalizedTypeIdentity {
                program.type_identity(TypeIdentityRequest {
                    substitutions: &self.substitutions,
                    ..TypeIdentityRequest::ordinary(self.requirement.return_type)
                })
            }
        }

        pub fn bound_trait_operator_meanings<'program>(
            program: &'program TypedTrees,
            machine_symbol: SymbolHandle,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Vec<BoundTraitOperatorMeaning<'program>> {
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                return Vec::new();
            };
            let mut meanings = Vec::new();
            for bound in &machine.conformance_bounds {
                let Some(trait_definition) = program.trait_definition_by_symbol(bound.trait_symbol)
                else {
                    continue;
                };
                let parameters = program.trait_type_parameters(trait_definition);
                if parameters.len() != bound.arguments.len() {
                    continue;
                }
                let substitutions = parameters
                    .iter()
                    .zip(&bound.arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument))
                    .collect::<Vec<_>>();
                visit_trait_operator_meanings(
                    program,
                    trait_definition,
                    spelling,
                    operand_types,
                    &substitutions,
                    &mut |trait_definition, requirement, substitutions, constants| {
                        let meaning = BoundTraitOperatorMeaning {
                            bound,
                            trait_definition,
                            requirement,
                            substitutions,
                            constants,
                        };
                        let same_application =
                            |known: &BoundTraitOperatorMeaning<'_>| {
                                known.bound.binder == bound.binder
                                    && known.requirement.symbol == requirement.symbol
                                    && known.constants == meaning.constants
                                    && program.trait_type_parameters(trait_definition).iter().all(
                                        |parameter| {
                                            let identity =
                                    |substitutions: &[(SymbolHandle, TypeReferenceHandle)]| {
                                        substitutions
                                            .iter()
                                            .find(|(binder, _)| *binder == parameter.symbol)
                                            .map(|(_, argument)| {
                                                program.type_identity(TypeIdentityRequest {
                                                    substitutions,
                                                    ..TypeIdentityRequest::ordinary(*argument)
                                                })
                                            })
                                    };
                                            identity(&known.substitutions)
                                                == identity(&meaning.substitutions)
                                        },
                                    )
                            };
                        if !meanings.iter().any(same_application) {
                            meanings.push(meaning);
                        }
                    },
                );
            }
            meanings
        }

        #[derive(Debug, Clone)]
        pub struct ScopedTraitOperatorMeaning<'program> {
            pub trait_definition: &'program crate::trait_definition::TraitDefinition,
            pub requirement: &'program crate::signature::StateSignature,
            pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub constants: Vec<super::OperatorConstBinding>,
        }

        pub fn trait_scope_operator_meanings<'program>(
            program: &'program TypedTrees,
            trait_definition: &'program crate::trait_definition::TraitDefinition,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Vec<ScopedTraitOperatorMeaning<'program>> {
            let mut meanings = Vec::new();
            visit_trait_operator_meanings(
                program,
                trait_definition,
                spelling,
                operand_types,
                &[],
                &mut |trait_definition, requirement, substitutions, constants| {
                    meanings.push(ScopedTraitOperatorMeaning {
                        trait_definition,
                        requirement,
                        substitutions,
                        constants,
                    });
                },
            );
            meanings
        }

        pub fn trait_scope_operator_meaning_exists(
            program: &TypedTrees,
            trait_definition: &crate::trait_definition::TraitDefinition,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> bool {
            let mut found = false;
            visit_trait_operator_meanings(
                program,
                trait_definition,
                spelling,
                operand_types,
                &[],
                &mut |_, _, _, _| found = true,
            );
            found
        }

        fn visit_trait_operator_meanings<'program>(
            program: &'program TypedTrees,
            trait_definition: &'program crate::trait_definition::TraitDefinition,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            visit: &mut impl FnMut(
                &'program crate::trait_definition::TraitDefinition,
                &'program crate::signature::StateSignature,
                Vec<(SymbolHandle, TypeReferenceHandle)>,
                Vec<super::OperatorConstBinding>,
            ),
        ) {
            let mut scope =
                crate::trait_definition::AppliedTraitScope::abstract_scope(trait_definition);
            scope.substitutions = substitutions.to_vec();
            scope.argument_owner = SymbolHandle::invalid();
            scope.visit(program, &mut |scope| {
                let trait_definition = scope.declaration;
                let substitutions = scope.substitutions.as_slice();
                for requirement in program.trait_machine_signatures(trait_definition) {
                    if requirement.spelling != Some(spelling) {
                        continue;
                    }
                    let Some(bindings) = trait_operator_operand_bindings(
                        program,
                        trait_definition,
                        requirement,
                        operand_types,
                        Some(substitutions),
                    ) else {
                        continue;
                    };
                    let matches =
                        program
                            .trait_type_parameters(trait_definition)
                            .iter()
                            .all(|parameter| {
                                let actual = bindings
                                    .types
                                    .iter()
                                    .find(|(binder, _)| *binder == parameter.symbol);
                                let constant = bindings
                                    .constants
                                    .iter()
                                    .find(|binding| binding.symbol == parameter.symbol);
                                if actual.is_none() && constant.is_none() {
                                    return true;
                                }
                                let Some(expected) = substitutions
                                    .iter()
                                    .find(|(binder, _)| *binder == parameter.symbol)
                                    .map(|(_, reference)| *reference)
                                    .or_else(|| {
                                        program
                                            .type_reference_table
                                            .find_named_type_reference(parameter.symbol)
                                    })
                                else {
                                    return false;
                                };
                                if let Some(binding) = constant {
                                    let crate::data::TypeParameterKind::Const { type_reference } =
                                        parameter.kind
                                    else {
                                        return false;
                                    };
                                    return closed_const_identity_from_type_reference(
                                        program,
                                        expected,
                                        type_reference,
                                    )
                                    .is_some_and(|expected| expected == binding.value);
                                }
                                let Some((_, actual)) = actual else {
                                    return true;
                                };
                                program.normalized_type_identity(*actual)
                                    == program.type_identity(TypeIdentityRequest {
                                        substitutions,
                                        ..TypeIdentityRequest::ordinary(expected)
                                    })
                            });
                    if !matches {
                        continue;
                    }
                    let mut applied_substitutions = substitutions.to_vec();
                    applied_substitutions.extend(bindings.types.into_iter().filter(
                        |(binder, _)| {
                            !substitutions
                                .iter()
                                .any(|(parameter, _)| parameter == binder)
                        },
                    ));
                    visit(
                        trait_definition,
                        requirement,
                        applied_substitutions,
                        bindings.constants,
                    );
                }
            });
        }

        #[derive(Debug, Clone, Copy)]
        pub struct SelectedTraitOperatorMeaning<'program> {
            pub trait_definition: &'program crate::trait_definition::TraitDefinition,
            pub requirement: &'program crate::signature::StateSignature,
            pub application: &'program crate::ClosedConformanceApplication,
            pub row: &'program crate::ClosedConformanceRowIdentity,
        }

        pub fn selected_trait_operator_meanings<'program>(
            program: &'program TypedTrees,
            machine_symbol: SymbolHandle,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Vec<SelectedTraitOperatorMeaning<'program>> {
            let Some(specialization) = program.machine_specialization(machine_symbol) else {
                return Vec::new();
            };

            specialization
                .conformance_applications
                .iter()
                .flat_map(|application| {
                    application.rows.iter().filter_map(move |row| {
                        let trait_definition =
                            program.trait_definition_by_symbol(row.declaring_trait)?;
                        let requirement = program
                            .trait_machine_signature_by_symbol(trait_definition, row.requirement)?;
                        (requirement.spelling == Some(spelling)
                            && trait_operator_matches_application(
                                program,
                                trait_definition,
                                requirement,
                                application,
                                operand_types,
                            ))
                        .then_some(SelectedTraitOperatorMeaning {
                            trait_definition,
                            requirement,
                            application,
                            row,
                        })
                    })
                })
                .collect()
        }

        pub fn selected_trait_operator_meaning_exists(
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            spelling: OperatorSpelling,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> bool {
            let Some(specialization) = program.machine_specialization(machine_symbol) else {
                return false;
            };

            specialization
                .conformance_applications
                .iter()
                .any(|application| {
                    application.rows.iter().any(|row| {
                        let Some(trait_definition) =
                            program.trait_definition_by_symbol(row.declaring_trait)
                        else {
                            return false;
                        };
                        let Some(requirement) = program
                            .trait_machine_signature_by_symbol(trait_definition, row.requirement)
                        else {
                            return false;
                        };
                        requirement.spelling == Some(spelling)
                            && trait_operator_matches_application(
                                program,
                                trait_definition,
                                requirement,
                                application,
                                operand_types,
                            )
                    })
                })
        }

        pub fn trait_operator_operand_signature(
            program: &TypedTrees,
            trait_definition: &crate::trait_definition::TraitDefinition,
            requirement: &crate::signature::StateSignature,
        ) -> Result<String, crate::runtime_callable_identity::RuntimeCallableIdentityError>
        {
            super::operand_signatures::operand_signature(
                program,
                program
                    .trait_type_parameters(trait_definition)
                    .iter()
                    .chain(program.state_signature_type_parameters(requirement))
                    .map(|parameter| parameter.symbol),
                program.state_signature_parameters(requirement),
            )
        }

        pub(crate) fn abstract_requirement_may_have_operator_bound(
            program: &TypedTrees,
            owner: SymbolHandle,
            spelling: OperatorSpelling,
        ) -> bool {
            if !owner.is_valid() {
                return false;
            }
            let parent = program.symbols.get(owner).parent;
            let Some(declaration) = program.trait_definition_by_symbol(parent) else {
                return parent.is_valid()
                    && program.symbols.get(parent).kind == symbols::SymbolKind::Trait;
            };
            let signatures = program.trait_machine_signatures(declaration);
            if signatures.len() != declaration.machines.len()
                || !signatures.iter().any(|signature| signature.symbol == owner)
            {
                return true;
            }
            let mut pending = vec![(declaration.symbol, false)];
            let mut visited = Vec::new();
            while let Some((symbol, through_bound)) = pending.pop() {
                if visited.contains(&(symbol, through_bound)) {
                    continue;
                }
                visited.push((symbol, through_bound));
                let Some(declaration) = program.trait_definition_by_symbol(symbol) else {
                    return true;
                };
                let requirements = program.trait_requirements(declaration);
                let signatures = program.trait_machine_signatures(declaration);
                if requirements.len() != declaration.requires.len()
                    || signatures.len() != declaration.machines.len()
                {
                    return true;
                }
                if through_bound
                    && signatures
                        .iter()
                        .any(|requirement| requirement.spelling == Some(spelling))
                {
                    return true;
                }
                pending.extend(
                    declaration
                        .conformance_bounds
                        .iter()
                        .map(|bound| (bound.trait_symbol, true)),
                );
                pending.extend(
                    requirements
                        .iter()
                        .chain(declaration.refines.iter())
                        .map(|parent| (parent.symbol, through_bound)),
                );
            }
            false
        }
    }
    pub(crate) mod type_matching {
        use crate::TypedTrees;
        use crate::data::TypeParameter;
        use crate::declarations::operator::OperatorConstBinding;
        use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
        use arena::HandleSpan;
        use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
        use symbols::SymbolHandle;

        pub(crate) fn type_reference_matches(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            bindable_owner: Option<SymbolHandle>,
            type_parameters: &[TypeParameter],
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
            const_bindings: &mut Vec<OperatorConstBinding>,
        ) -> bool {
            type_reference_matches_with_policy(
                program,
                actual,
                expected,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                true,
            )
        }

        pub(crate) fn type_reference_matches_with_policy(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            bindable_owner: Option<SymbolHandle>,
            type_parameters: &[TypeParameter],
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
            const_bindings: &mut Vec<OperatorConstBinding>,
            allow_name_fallback: bool,
        ) -> bool {
            macro_rules! match_reference {
                ($actual:expr, $expected:expr $(,)?) => {
                    type_reference_matches_with_policy(
                        program,
                        $actual,
                        $expected,
                        bindable_owner,
                        type_parameters,
                        bindings,
                        const_bindings,
                        allow_name_fallback,
                    )
                };
            }
            if !actual.is_valid() || !expected.is_valid() {
                return false;
            }
            if let Some(parameter) =
                expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
                && let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind
            {
                if let Some(value) =
                    closed_const_identity_from_type_reference(program, actual, type_reference)
                {
                    return bind_operator_const(const_bindings, parameter.symbol, value);
                }
                if !allow_name_fallback {
                    return false;
                }
            }
            if let Some(bindable_symbol) = expected_bindable_symbol(
                program,
                expected,
                bindable_owner,
                type_parameters,
                allow_name_fallback,
            ) {
                if let Some((_, bound_actual)) = bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == bindable_symbol)
                {
                    return type_reference_matches_with_policy(
                        program,
                        actual,
                        *bound_actual,
                        None,
                        &[],
                        &mut Vec::new(),
                        &mut Vec::new(),
                        allow_name_fallback,
                    );
                }
                bindings.push((bindable_symbol, actual));
                return true;
            }

            match (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(expected),
            ) {
                (
                    TypeReferenceNode::Reference {
                        referee: actual_referee,
                        access: actual_access,
                        lifetime: _,
                    },
                    TypeReferenceNode::Reference {
                        referee: expected_referee,
                        access: expected_access,
                        lifetime: _,
                    },
                ) => {
                    actual_access == expected_access
                        && match_reference!(*actual_referee, *expected_referee)
                }
                (
                    TypeReferenceNode::Constrained {
                        base_type: actual_base,
                        ..
                    },
                    _,
                ) => match_reference!(*actual_base, expected),
                (
                    _,
                    TypeReferenceNode::Constrained {
                        base_type: expected_base,
                        ..
                    },
                ) => match_reference!(actual, *expected_base),
                (
                    TypeReferenceNode::FixedArray {
                        element_type: actual_element,
                        length: actual_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: expected_element,
                        length: expected_length,
                    },
                ) => {
                    fixed_array_lengths_match(
                        program,
                        actual_length,
                        expected_length,
                        type_parameters,
                        const_bindings,
                        allow_name_fallback,
                    ) && match_reference!(*actual_element, *expected_element)
                }
                (
                    TypeReferenceNode::Slice {
                        element_type: actual_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: expected_element,
                    },
                ) => match_reference!(*actual_element, *expected_element),
                (
                    TypeReferenceNode::Named {
                        symbol: actual_symbol,
                        ..
                    },
                    TypeReferenceNode::Generic { .. },
                ) if actual_symbol.is_valid() => program
                    .data_definition_by_symbol(*actual_symbol)
                    .and_then(|definition| definition.generic_instance)
                    .is_some_and(|origin| match_reference!(origin, expected)),
                (
                    TypeReferenceNode::Named {
                        symbol: actual_symbol,
                        name: actual_name,
                    },
                    TypeReferenceNode::Named {
                        symbol: expected_symbol,
                        name: expected_name,
                    },
                ) => nominal_type_identity_matches(
                    *actual_symbol,
                    actual_name.as_str(),
                    *expected_symbol,
                    expected_name.as_str(),
                    allow_name_fallback,
                ),
                (
                    TypeReferenceNode::Generic {
                        base_symbol: actual_symbol,
                        base_name: actual_name,
                        arguments: actual_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: expected_symbol,
                        base_name: expected_name,
                        arguments: expected_arguments,
                        ..
                    },
                ) => {
                    nominal_type_identity_matches(
                        *actual_symbol,
                        actual_name.as_str(),
                        *expected_symbol,
                        expected_name.as_str(),
                        allow_name_fallback,
                    ) && type_reference_spans_match(
                        program,
                        *actual_arguments,
                        *expected_arguments,
                        bindable_owner,
                        type_parameters,
                        bindings,
                        const_bindings,
                        allow_name_fallback,
                    )
                }
                (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
                _ => false,
            }
        }

        fn type_reference_spans_match(
            program: &TypedTrees,
            actual: HandleSpan<TypeReferenceHandle>,
            expected: HandleSpan<TypeReferenceHandle>,
            bindable_owner: Option<SymbolHandle>,
            type_parameters: &[TypeParameter],
            bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
            const_bindings: &mut Vec<OperatorConstBinding>,
            allow_name_fallback: bool,
        ) -> bool {
            let actual = program.type_reference_table.type_reference_handles(actual);
            let expected = program.type_reference_handles(expected);
            actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(actual, expected)| {
                    type_reference_matches_with_policy(
                        program,
                        *actual,
                        *expected,
                        bindable_owner,
                        type_parameters,
                        bindings,
                        const_bindings,
                        allow_name_fallback,
                    )
                })
        }

        fn find_type_parameter<'a>(
            type_parameters: &'a [TypeParameter],
            symbol: &SymbolHandle,
            name: &crate::name::Identifier,
            allow_name_fallback: bool,
        ) -> Option<&'a TypeParameter> {
            type_parameters.iter().find(|parameter| {
                (symbol.is_valid() && parameter.symbol == *symbol)
                    || (allow_name_fallback && parameter.name == *name)
            })
        }

        fn fixed_array_lengths_match(
            program: &TypedTrees,
            actual: &FixedArrayLength,
            expected: &FixedArrayLength,
            type_parameters: &[TypeParameter],
            const_bindings: &mut Vec<OperatorConstBinding>,
            allow_name_fallback: bool,
        ) -> bool {
            let FixedArrayLength::ConstParameter { symbol, name } = expected else {
                return actual == expected;
            };
            let Some(parameter) =
                find_type_parameter(type_parameters, symbol, name, allow_name_fallback)
            else {
                return actual == expected;
            };
            let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind else {
                return false;
            };
            let FixedArrayLength::Literal(value) = actual else {
                return allow_name_fallback && actual == expected;
            };
            let Ok(value) = i128::try_from(*value) else {
                return false;
            };
            let Some(primitive) = program.type_reference_table.primitive_type(type_reference)
            else {
                return false;
            };
            bind_operator_const(
                const_bindings,
                parameter.symbol,
                CanonicalConstIdentity::integer(primitive.name(), value),
            )
        }

        pub fn closed_const_identity_from_type_reference(
            program: &TypedTrees,
            actual: TypeReferenceHandle,
            declared_carrier: TypeReferenceHandle,
        ) -> Option<CanonicalConstIdentity> {
            let TypeReferenceNode::Named { name, .. } =
                program.type_reference_table.type_reference(actual)
            else {
                return None;
            };
            if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                return Some(value.identity());
            }
            let value = name.as_str().parse::<i128>().ok()?;
            let primitive = program
                .type_reference_table
                .primitive_type(declared_carrier)?;
            Some(CanonicalConstIdentity::integer(primitive.name(), value))
        }

        fn bind_operator_const(
            bindings: &mut Vec<OperatorConstBinding>,
            symbol: SymbolHandle,
            value: CanonicalConstIdentity,
        ) -> bool {
            if let Some(existing) = bindings.iter().find(|binding| binding.symbol == symbol) {
                return existing.value == value;
            }
            bindings.push(OperatorConstBinding { symbol, value });
            true
        }

        fn nominal_type_identity_matches(
            actual_symbol: SymbolHandle,
            actual_name: &str,
            expected_symbol: SymbolHandle,
            expected_name: &str,
            allow_name_fallback: bool,
        ) -> bool {
            if allow_name_fallback {
                return (actual_symbol.is_valid() && actual_symbol == expected_symbol)
                    || actual_name == expected_name;
            }
            if actual_symbol.is_valid() || expected_symbol.is_valid() {
                return actual_symbol.is_valid()
                    && expected_symbol.is_valid()
                    && actual_symbol == expected_symbol;
            }
            matches!(
                (
                    crate::types::PrimitiveType::from_name(actual_name),
                    crate::types::PrimitiveType::from_name(expected_name),
                ),
                (Some(actual), Some(expected)) if actual == expected
            )
        }

        fn expected_bindable_symbol(
            program: &TypedTrees,
            expected: TypeReferenceHandle,
            bindable_owner: Option<SymbolHandle>,
            type_parameters: &[TypeParameter],
            allow_name_fallback: bool,
        ) -> Option<SymbolHandle> {
            let (symbol, _) = match program.type_reference_table.type_reference(expected) {
                TypeReferenceNode::Named { symbol, name }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    base_name: name,
                    ..
                } => (*symbol, name),
                _ => return None,
            };
            if symbol.is_valid() && Some(symbol) == bindable_owner {
                return Some(symbol);
            }
            expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
                .map(|parameter| parameter.symbol)
        }

        fn expected_type_parameter<'a>(
            program: &TypedTrees,
            expected: TypeReferenceHandle,
            type_parameters: &'a [TypeParameter],
            allow_name_fallback: bool,
        ) -> Option<&'a TypeParameter> {
            match program.type_reference_table.type_reference(expected) {
                TypeReferenceNode::Named { symbol, name }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    base_name: name,
                    ..
                } => find_type_parameter(type_parameters, symbol, name, allow_name_fallback),
                _ => None,
            }
        }
    }
    pub use type_matching::closed_const_identity_from_type_reference as closed_const_identity_for_type_reference;

    pub use applications::{
        ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
        SymbolicOperatorTypeApplicationArgument, canonical_closed_operator_realization_bytes,
        closed_indexed_operator_application_for_operands, closed_operator_application_for_operands,
        closed_operator_application_from_bindings, closed_operator_realization_application,
        symbolic_operator_type_application_for_operands,
    };
    pub use indexing::{
        builtin_index_read_is_spelled, resolve_indexed_spelling_for_operands,
        unconstrained_type_reference,
    };
    pub use named_calls::{
        named_call_family_members, named_call_family_path, named_expression_call_candidates,
        named_expression_call_family_path, named_statement_call_candidates,
        named_statement_call_family_path, named_transition_family_path, resolve_named_call,
        resolve_named_expression_call, selected_named_expression_call_has_no_runtime_receiver,
        selected_named_statement_call_has_no_runtime_receiver,
    };
    pub use operand_signatures::operator_operand_signature;
    pub use primitive_float::{
        is_sealed_toolchain_float_operator, primitive_float_binary_semantics,
    };
    pub use primitive_integer::{builtin_integer_result_carrier, integer_peer_operand_carrier};
    pub use satisfied_operators::{
        boundary_operator_requirement_identity,
        resolve_satisfied_boundary_operator_for_conformance,
        resolve_satisfied_checked_operator_for_conformance,
        resolve_specialized_checked_operator_application,
    };
    pub use spellings::{
        OperandType, OperatorOperandBindings, arithmetic_operator_spelling,
        binary_operator_spelling, comparison_operator_spelling, declared_domain_constraints,
        has_builtin_spelled_expression_meaning, operator_operand_bindings, resolve_spelling,
        resolve_spelling_for_operand_types, resolve_spelling_for_operands,
        scalar_operand_read_type,
    };
    pub use trait_operators::{
        BoundTraitOperatorMeaning, ScopedTraitOperatorMeaning, SelectedTraitOperatorMeaning,
        bound_trait_operator_meanings, selected_trait_operator_meaning_exists,
        selected_trait_operator_meanings, trait_operator_matches_application,
        trait_operator_operand_bindings, trait_operator_operand_signature,
        trait_scope_operator_meaning_exists, trait_scope_operator_meanings,
    };

    use arena::HandleSpan;
    use language_core::operator_spelling::OperatorSpelling;
    use language_semantics::const_value::CanonicalConstIdentity;
    use symbols::SymbolHandle;

    use crate::TypedTrees;
    use crate::domain::DomainDefinition;
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OperatorConstBinding {
        pub symbol: SymbolHandle,
        pub value: CanonicalConstIdentity,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct OperatorDefinition {
        pub is_public: bool,
        pub is_boundary: bool,
        pub symbol: SymbolHandle,
        pub name: HandleSpan<crate::name::Identifier>,
        pub lifetime_parameters: Vec<crate::name::Identifier>,
        pub type_parameters: HandleSpan<crate::data::TypeParameter>,
        pub parameters: HandleSpan<crate::signature::StateParameter>,
        pub return_type: crate::types::TypeReferenceHandle,
        pub contracts: HandleSpan<crate::signature::SignatureContract>,
        pub spelling: Option<OperatorSpelling>,
        pub token_count: usize,
        pub home_domain: SymbolHandle,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SpelledOperator<'program> {
        pub operator: &'program OperatorDefinition,
        pub domain: Option<&'program DomainDefinition>,
    }

    pub fn declaration_by_symbol(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<&OperatorDefinition> {
        program
            .all_operators()
            .chain(program.machine_token_bindings())
            .find(|operator| operator.symbol == symbol)
    }
}
