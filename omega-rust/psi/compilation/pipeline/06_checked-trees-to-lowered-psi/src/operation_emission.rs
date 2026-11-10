/*
Core operation buffer (`OperationBuffer`) and instruction emitters for direct/boundary calls,
scalar and boolean expressions, integer arithmetic, view subslices, and place declarations.
*/

pub(crate) mod boolean {

    use super::buffer::OperationBuffer;
    use super::expressions::{
        LoweredDirectExpression, emit_direct_expression, emit_element_length, emit_scalar_leaf,
    };
    use super::integer::LoweredIntegerComparisonKind;
    use crate::emission::operation_emission::buffer::scalar_result_declaration;
    use crate::shared::terminal_identities::mint;
    use crate::shared::terminal_identities::obligation_id;
    use crate::shared::terminal_identities::value_id;
    use semantic_vocabulary::{PlaceId, ScalarType, StructuralFieldId, ValueId};
    use terminal_psi::{OperationKind, ValueDeclaration};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredByteSequenceOperand {
        pub source: PlaceId,
        pub path: Vec<super::expressions::LoweredElementPathSegment>,
        pub field: Option<StructuralFieldId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredBooleanReturnExpression {
        ScalarObservation {
            operand: Box<LoweredDirectExpression>,
        },
        StructuralCaseMembership {
            source: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            case: semantic_vocabulary::StructuralCaseId,
        },
        IndexedCaseMembership {
            source: PlaceId,
            observes_element_view_length: bool,
            path: Vec<terminal_psi::StructuralPathSegment>,
            index: Box<LoweredDirectExpression>,
            element_path: Vec<terminal_psi::StructuralPathSegment>,
            case: semantic_vocabulary::StructuralCaseId,
        },
        PrimitiveRead {
            qualifications: semantic_vocabulary::ScalarQualificationSetId,
            source: super::expressions::PrimitiveReadSource,
            path: Vec<terminal_psi::StructuralPathSegment>,
        },
        Constant {
            value: bool,
        },
        Parameter {
            position: usize,
        },
        Local {
            position: usize,
        },
        UnresolvedStructuralParameterField {
            parameter_position: u32,
            path: Vec<String>,
        },
        StructuralField {
            qualifications: semantic_vocabulary::ScalarQualificationSetId,
            source: super::expressions::PrimitiveReadSource,
            path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            field: StructuralFieldId,
        },
        ByteSequencePredicate {
            predicate: terminal_psi::ByteSequencePredicate,
            source: PlaceId,
            path: Vec<super::expressions::LoweredElementPathSegment>,
            field: Option<StructuralFieldId>,
        },
        ByteSequenceEqual {
            left: LoweredByteSequenceOperand,
            right: LoweredByteSequenceOperand,
        },
        Not {
            operand: Box<LoweredBooleanReturnExpression>,
        },
        Equal {
            left: Box<LoweredBooleanReturnExpression>,
            right: Box<LoweredBooleanReturnExpression>,
        },
        IntegerComparison {
            kind: LoweredIntegerComparisonKind,
            left: Box<LoweredDirectExpression>,
            right: Box<LoweredDirectExpression>,
        },
        And {
            left: Box<LoweredBooleanReturnExpression>,
            right: Box<LoweredBooleanReturnExpression>,
        },
        Or {
            left: Box<LoweredBooleanReturnExpression>,
            right: Box<LoweredBooleanReturnExpression>,
        },
    }

    pub(crate) fn emit_boolean_expression(
        expression: &LoweredBooleanReturnExpression,
        parameters: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> ValueId {
        match expression {
            LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                emit_direct_expression(operand, parameters, next_value_identity, operations)
            }
            LoweredBooleanReturnExpression::Constant { value } => {
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, ScalarType::Boolean),
                    OperationKind::BooleanConstant { value: *value },
                );
                id
            }
            LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
                let left =
                    emit_direct_expression(left, parameters, next_value_identity, operations);
                let right =
                    emit_direct_expression(right, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, ScalarType::Boolean),
                    kind.operation(left, right),
                );
                id
            }
            LoweredBooleanReturnExpression::Parameter { position }
            | LoweredBooleanReturnExpression::Local { position } => parameters[*position].id,
            LoweredBooleanReturnExpression::PrimitiveRead {
                source,
                path,
                qualifications,
            } => emit_direct_expression(
                &LoweredDirectExpression::PrimitiveRead {
                    qualifications: *qualifications,
                    source: source.clone(),
                    path: path.clone(),
                    scalar_type: ScalarType::Boolean,
                },
                parameters,
                next_value_identity,
                operations,
            ),
            LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case } => {
                emit_scalar_leaf(
                    OperationKind::StructuralCaseMembership {
                        source: *source,
                        path: path.clone(),
                        case: *case,
                    },
                    ScalarType::Boolean,
                    next_value_identity,
                    operations,
                )
            }
            LoweredBooleanReturnExpression::IndexedCaseMembership {
                source,
                observes_element_view_length,
                path,
                index,
                element_path,
                case,
            } => {
                let index =
                    emit_direct_expression(index, parameters, next_value_identity, operations);
                if *observes_element_view_length {
                    emit_element_length(
                        terminal_psi::StructuralArgument::whole(
                            *source,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                        next_value_identity,
                        operations,
                    );
                }
                let obligation = obligation_id(
                    operations
                        .next_identity
                        .checked_add(1)
                        .expect("membership obligation follows its operation identity"),
                );
                let mut path = path.clone();
                path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
                path.extend(element_path.iter().cloned());
                emit_scalar_leaf(
                    OperationKind::StructuralCaseMembership {
                        source: *source,
                        path,
                        case: *case,
                    },
                    ScalarType::Boolean,
                    next_value_identity,
                    operations,
                )
            }
            LoweredBooleanReturnExpression::StructuralField {
                qualifications,
                source,
                path,
                field,
            } => {
                let source = super::expressions::emit_read_source(source, operations);
                emit_scalar_leaf(
                    OperationKind::BooleanStructuralField {
                        source,
                        path: path.clone(),
                        field: *field,
                    },
                    semantic_vocabulary::QualifiedScalarType {
                        scalar_type: ScalarType::Boolean,
                        qualifications: *qualifications,
                    },
                    next_value_identity,
                    operations,
                )
            }
            LoweredBooleanReturnExpression::ByteSequencePredicate {
                predicate,
                source,
                path,
                field,
            } => {
                let operand = byte_sequence_operand(
                    *source,
                    path,
                    *field,
                    parameters,
                    next_value_identity,
                    operations,
                );
                emit_scalar_leaf(
                    OperationKind::ByteSequencePredicate {
                        operand,
                        predicate: *predicate,
                    },
                    ScalarType::Boolean,
                    next_value_identity,
                    operations,
                )
            }
            LoweredBooleanReturnExpression::ByteSequenceEqual { left, right } => {
                let left = byte_sequence_operand(
                    left.source,
                    &left.path,
                    left.field,
                    parameters,
                    next_value_identity,
                    operations,
                );
                let right = byte_sequence_operand(
                    right.source,
                    &right.path,
                    right.field,
                    parameters,
                    next_value_identity,
                    operations,
                );
                emit_scalar_leaf(
                    OperationKind::ByteSequenceEqual { left, right },
                    ScalarType::Boolean,
                    next_value_identity,
                    operations,
                )
            }
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
                unreachable!("shared Boolean members resolve before terminal operation emission")
            }
            LoweredBooleanReturnExpression::Not { operand } => {
                let operand =
                    emit_boolean_expression(operand, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, ScalarType::Boolean),
                    OperationKind::BooleanNot { operand },
                );
                id
            }
            LoweredBooleanReturnExpression::Equal { left, right } => {
                let left =
                    emit_boolean_expression(left, parameters, next_value_identity, operations);
                let right =
                    emit_boolean_expression(right, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, ScalarType::Boolean),
                    OperationKind::BooleanEqual { left, right },
                );
                id
            }
            LoweredBooleanReturnExpression::And { .. }
            | LoweredBooleanReturnExpression::Or { .. } => {
                unreachable!("short-circuit Boolean expressions lower through terminal control")
            }
        }
    }

    fn byte_sequence_operand(
        source: PlaceId,
        path: &[super::expressions::LoweredElementPathSegment],
        field: Option<StructuralFieldId>,
        parameters: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> terminal_psi::control_flow::ByteSequenceOperand {
        match field {
            Some(field) => {
                let path = super::expressions::emit_prepared_path(
                    path,
                    0,
                    parameters,
                    next_value_identity,
                    operations,
                );
                let path =
                    super::expressions::path_with_current_bounds(&path, operations.next_identity);
                let length = emit_scalar_leaf(
                    OperationKind::StructuralByteSequenceFieldLength {
                        source,
                        path: path.clone(),
                        field,
                    },
                    ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64 is valid"),
                    ),
                    next_value_identity,
                    operations,
                );
                operations.storage_observations.field_bytes.push((
                    source,
                    path.clone(),
                    field,
                    length,
                ));
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source: terminal_psi::StructuralArgument {
                        place: source,
                        path,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                    field,
                    length,
                }
            }
            None => {
                let path = super::expressions::emit_prepared_path(
                    path,
                    0,
                    parameters,
                    next_value_identity,
                    operations,
                );
                let path =
                    super::expressions::path_with_current_bounds(&path, operations.next_identity);
                let source = terminal_psi::StructuralArgument {
                    place: source,
                    path,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
                let length = super::expressions::emit_byte_length(
                    source.clone(),
                    next_value_identity,
                    operations,
                );
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length }
            }
        }
    }
}
pub(crate) mod boundary_calls {

    use crate::unit::attached_unit::argument_evaluation::validated_values;
    pub(crate) mod overflow_eligibility {

        use super::super::buffer::OperationBuffer;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use crate::shared::terminal_identities::{allocate_dense, value_id};
        use semantic_vocabulary::{
            IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId,
        };
        use terminal_psi::{OperationKind, OperationResult, ValueDeclaration};

        pub(crate) fn emit(
            arguments: &[ValueId],
            types: &[ScalarType],
            divisor: Option<usize>,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<ValueId, LoweringError> {
            if arguments.len() != types.len()
                || divisor.is_some_and(|position| {
                    !matches!(types.get(position), Some(ScalarType::IeeeFloat(_)))
                })
            {
                return unsupported("overflow eligibility lost its exact numeric operands");
            }
            let mut emitter = Emitter {
                next_value,
                operations,
            };
            let byte = IntegerType::new(IntegerSign::Unsigned, 8)
                .map_err(|_| LoweringError::Unsupported("invalid Boolean byte carrier"))?;
            let one = emitter.constant(byte, 1)?;
            let mut conjunction = one;
            for (position, (argument, kind)) in arguments.iter().zip(types).enumerate() {
                match kind {
                    ScalarType::Integer(_) => {}
                    ScalarType::IeeeFloat(format) => {
                        let (bits, exponent_mask, magnitude_mask) = match format {
                            IeeeFloatFormat::Binary32 => (32, 0x7f80_0000, 0x7fff_ffff),
                            IeeeFloatFormat::Binary64 => {
                                (64, 0x7ff0_0000_0000_0000, 0x7fff_ffff_ffff_ffff)
                            }
                        };
                        let integer =
                            IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| {
                                LoweringError::Unsupported("invalid float image carrier")
                            })?;
                        let raw = emitter.value(
                            ScalarType::Integer(integer),
                            OperationKind::ScalarRepresentationView {
                                operand: *argument,
                                landing_obligation: None,
                            },
                        )?;
                        let mask = emitter.constant(integer, exponent_mask)?;
                        let exponent = emitter.value(
                            ScalarType::Integer(integer),
                            OperationKind::IntegerBitwiseAnd {
                                left: raw,
                                right: mask,
                            },
                        )?;
                        let nonfinite = emitter.value(
                            ScalarType::Boolean,
                            OperationKind::IntegerEqual {
                                left: exponent,
                                right: mask,
                            },
                        )?;
                        let finite = emitter.value(
                            ScalarType::Boolean,
                            OperationKind::BooleanNot { operand: nonfinite },
                        )?;
                        conjunction = emitter.conjoin(byte, conjunction, finite)?;
                        if divisor == Some(position) {
                            let mask = emitter.constant(integer, magnitude_mask)?;
                            let magnitude = emitter.value(
                                ScalarType::Integer(integer),
                                OperationKind::IntegerBitwiseAnd {
                                    left: raw,
                                    right: mask,
                                },
                            )?;
                            let zero = emitter.constant(integer, 0)?;
                            let is_zero = emitter.value(
                                ScalarType::Boolean,
                                OperationKind::IntegerEqual {
                                    left: magnitude,
                                    right: zero,
                                },
                            )?;
                            let nonzero = emitter.value(
                                ScalarType::Boolean,
                                OperationKind::BooleanNot { operand: is_zero },
                            )?;
                            conjunction = emitter.conjoin(byte, conjunction, nonzero)?;
                        }
                    }
                    _ => return unsupported("overflow eligibility input is not numeric"),
                }
            }
            emitter.value(
                ScalarType::Boolean,
                OperationKind::IntegerEqual {
                    left: conjunction,
                    right: one,
                },
            )
        }
        struct Emitter<'a> {
            next_value: &'a mut u64,
            operations: &'a mut OperationBuffer,
        }
        impl Emitter<'_> {
            fn value(
                &mut self,
                scalar_type: ScalarType,
                kind: OperationKind,
            ) -> Result<ValueId, LoweringError> {
                let id = value_id(allocate_dense(self.next_value)?);
                self.operations.push_leaf(
                    OperationResult::Scalar(ValueDeclaration {
                        id,
                        scalar_type,
                        qualifications: Default::default(),
                    }),
                    kind,
                );
                Ok(id)
            }
            fn constant(
                &mut self,
                integer: IntegerType,
                value: u64,
            ) -> Result<ValueId, LoweringError> {
                self.value(
                    ScalarType::Integer(integer),
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(value.into()),
                    },
                )
            }
            fn conjoin(
                &mut self,
                byte: IntegerType,
                conjunction: ValueId,
                predicate: ValueId,
            ) -> Result<ValueId, LoweringError> {
                let predicate = self.value(
                    ScalarType::Integer(byte),
                    OperationKind::ScalarRepresentationView {
                        operand: predicate,
                        landing_obligation: None,
                    },
                )?;
                self.value(
                    ScalarType::Integer(byte),
                    OperationKind::IntegerBitwiseAnd {
                        left: conjunction,
                        right: predicate,
                    },
                )
            }
        }
    }
    pub(crate) mod result_policy {

        use super::super::buffer::{OperationBuffer, SourceCallCoordinate};
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use crate::shared::terminal_identities::{allocate_dense, value_id};
        use checked_trees::{
            CheckedArithmeticPolicyAdapter, CheckedTrees, CheckedValueOrigin, NominalMachineUseSite,
        };
        use numerics::float_semantics::FloatFormat;
        use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};
        use symbols::SymbolHandle;
        use terminal_psi::{FloatResultPolicy, OperationKind, OperationResult, ValueDeclaration};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum BoundaryResultPolicy {
            None,
            Trapping {
                format: IeeeFloatFormat,
            },
            Saturating {
                format: IeeeFloatFormat,
                divisor_position: Option<usize>,
            },
        }

        impl BoundaryResultPolicy {
            pub(crate) fn prepare(
                checked: &CheckedTrees,
                machine: SymbolHandle,
                coordinate: SourceCallCoordinate,
                site: Option<NominalMachineUseSite>,
                target: SymbolHandle,
            ) -> Result<Self, LoweringError> {
                let Some(NominalMachineUseSite::Expression(expression)) = site else {
                    return Ok(Self::None);
                };
                let mut first_selection = None;
                let mut multiple_selections = false;
                checked
                    .facts
                    .operators
                    .for_each_resolved_expression_selection(
                        expression,
                        |origin, selected, adapter, _| {
                            if origin_matches(checked, origin, machine, coordinate) {
                                if first_selection.is_none() {
                                    first_selection = Some((selected, adapter));
                                } else {
                                    multiple_selections = true;
                                }
                            }
                        },
                    );
                let Some((selected, adapter)) = first_selection else {
                    if typed_trees::operator::declaration_by_symbol(&checked.typed, target)
                        .is_some()
                    {
                        return unsupported("boundary result policy lost its checked selection");
                    }
                    return Ok(Self::None);
                };
                if multiple_selections || selected != target {
                    return unsupported(
                        "boundary result policy has no unique exact checked selection",
                    );
                }
                let Some(format) = adapter.float_format() else {
                    return Ok(Self::None);
                };
                let format = match format {
                    FloatFormat::BINARY32 => IeeeFloatFormat::Binary32,
                    FloatFormat::BINARY64 => IeeeFloatFormat::Binary64,
                    _ => {
                        return unsupported(
                            "boundary result policy has an unsupported float format",
                        );
                    }
                };
                match adapter {
                    CheckedArithmeticPolicyAdapter::None => Ok(Self::None),
                    CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } => {
                        Ok(Self::Trapping { format })
                    }
                    CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. } => {
                        let declaration =
                            typed_trees::operator::declaration_by_symbol(&checked.typed, target)
                                .ok_or(LoweringError::Unsupported(
                                    "saturating boundary lost its selected declaration",
                                ))?;
                        let divisor = validation::float_saturation_divisor_parameter(
                            &checked.typed,
                            declaration,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "saturating boundary lost its raw semantic equation",
                        ))?;
                        Ok(Self::Saturating {
                            format,
                            divisor_position: divisor,
                        })
                    }
                }
            }

            pub(crate) fn bind(
                self,
                raw: ValueDeclaration,
                arguments: &[ValueId],
                argument_types: &[ScalarType],
                next_value: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<Option<FloatResultPolicy>, LoweringError> {
                let format = match self {
                    Self::None => return Ok(None),
                    Self::Trapping { format } | Self::Saturating { format, .. } => format,
                };
                if raw.scalar_type != ScalarType::IeeeFloat(format)
                    || !raw.qualifications.is_empty()
                {
                    return unsupported(
                        "boundary result policy disagrees with its raw result format",
                    );
                }
                Ok(Some(match self {
                    Self::Trapping { .. } => FloatResultPolicy::Trapping { raw_result: raw.id },
                    Self::Saturating {
                        divisor_position, ..
                    } => {
                        let eligible = super::overflow_eligibility::emit(
                            arguments,
                            argument_types,
                            divisor_position,
                            next_value,
                            operations,
                        )?;
                        FloatResultPolicy::Saturating {
                            raw_result: raw.id,
                            eligible,
                        }
                    }
                    Self::None => return Ok(None),
                }))
            }
        }

        fn origin_matches(
            checked: &CheckedTrees,
            origin: CheckedValueOrigin,
            machine: SymbolHandle,
            coordinate: SourceCallCoordinate,
        ) -> bool {
            let Some(origin) = checked.consistent_terminal_origin(origin) else {
                return false;
            };
            matches!(origin, CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
                if machine_symbol == machine && state_symbol == coordinate.state && statement_index == coordinate.statement_index)
        }

        pub(crate) fn emit(
            policy: Option<FloatResultPolicy>,
            raw: ValueDeclaration,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<ValueDeclaration, LoweringError> {
            let Some(policy) = policy else {
                return Ok(raw);
            };
            let result = ValueDeclaration {
                id: value_id(allocate_dense(next_value)?),
                ..raw
            };
            operations.push_leaf(
                OperationResult::Scalar(result),
                OperationKind::FloatResultPolicy { policy },
            );
            Ok(result)
        }
    }

    use super::buffer::{OperationBuffer, SourceCallCoordinate};
    use super::calls::{CallEmissionContext, PreparedStructuralArgument};
    use super::expressions::{LoweredDirectExpression, emit_direct_expression};
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{mint_with, value_id};
    use result_policy::BoundaryResultPolicy;
    use semantic_vocabulary::{QualifiedScalarType, ValueId};
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

    pub(crate) struct BoundaryOperands {
        pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
        pub(crate) arguments: Vec<ValueId>,
        pub(crate) structural_arguments: Vec<terminal_psi::StructuralArgument>,
        pub(crate) completion_receipts: Vec<terminal_psi::CompletionReceipt>,
        pub(crate) requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
    }

    pub(crate) fn complete_operands(
        target: &crate::unit::attached_unit::BoundaryParameters,
        arguments: &[ValueDeclaration],
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        receipts: &[crate::emission::claim_bindings::PreparedClaimTransfer],
        calls: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<BoundaryOperands, LoweringError> {
        if structural_arguments.len() != target.structural.len() {
            return unsupported(
                "boundary arguments disagree with their declared structural roster",
            );
        }
        let arguments = validated_values(Some(arguments), &target.scalar)?
            .iter()
            .map(|value| value.id)
            .collect();
        let completion_receipts = crate::emission::claim_bindings::emit_completion_receipts(
            receipts,
            calls.claims.bindings(),
            &structural_arguments,
            current_claims,
        )?;
        Ok(BoundaryOperands {
            boundary: target.id,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations: (0..target.requirement_count)
                .map(|_| calls.allocate_requirement())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    impl BoundaryOperands {
        pub(crate) fn into_kind(self) -> OperationKind {
            OperationKind::BoundaryCall {
                boundary: self.boundary,
                arguments: self.arguments,
                structural_arguments: self.structural_arguments,
                completion_receipts: self.completion_receipts,
                requirement_obligations: self.requirement_obligations,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredBoundaryCallBinding {
        pub(crate) source_coordinate: SourceCallCoordinate,
        pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
        pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
        pub(crate) target_machine: symbols::SymbolHandle,
        pub(crate) result_type: QualifiedScalarType,
        pub(crate) result_policy: BoundaryResultPolicy,
        pub(crate) arguments: Vec<LoweredDirectExpression>,
        pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
        pub(crate) completion_receipts: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
    }

    pub(crate) fn emit_boundary_call_binding(
        call: &LoweredBoundaryCallBinding,
        parameters: &[ValueDeclaration],
        frontier: &lowered_psi::LoweredSourceFrontierBindings,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
        call_emission: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<ValueId, LoweringError> {
        let mut targets = call_emission
            .boundaries
            .iter()
            .filter(|boundary| boundary.source == call.target_machine);
        let (Some(target), None) = (targets.next(), targets.next()) else {
            return unsupported("nested boundary application has no unique lowered declaration");
        };
        if call.structural_arguments.len() != target.structural.len() {
            return unsupported("computed boundary arguments disagree with their call shape");
        }
        if target.result != terminal_psi::BoundaryMachineResult::Scalar(call.result_type) {
            return unsupported("nested boundary application result disagrees with its signature");
        }
        if call.arguments.len() != target.scalar.len() {
            return unsupported("nested boundary application operand count disagrees");
        }
        let parameter_types = parameters
            .iter()
            .map(|parameter| parameter.value_type())
            .collect::<Vec<_>>();
        let mut arguments = Vec::with_capacity(call.arguments.len());
        for (argument, expected) in call.arguments.iter().zip(&target.scalar) {
            let value_type = argument.value_type(&parameter_types)?;
            if value_type != *expected {
                return unsupported(
                    "nested boundary application operand disagrees with its formal",
                );
            }
            arguments.push(ValueDeclaration {
                id: emit_direct_expression(argument, parameters, next_value_identity, operations),
                scalar_type: value_type.scalar_type,
                qualifications: value_type.qualifications,
            });
        }
        let structural_arguments = call
            .structural_arguments
            .iter()
            .map(|argument| {
                argument.emit(parameters, call_emission, next_value_identity, operations)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let operands = complete_operands(
            target,
            &arguments,
            structural_arguments,
            &call.completion_receipts,
            call_emission,
            current_claims,
        )?;
        let result = mint_with(
            next_value_identity,
            value_id,
            "nested boundary application value identity overflow",
        )?;
        let raw = ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: call.result_type.qualifications,
        };
        let policy = call.result_policy.bind(
            raw,
            &operands.arguments,
            &target
                .scalar
                .iter()
                .map(|parameter| parameter.scalar_type)
                .collect::<Vec<_>>(),
            next_value_identity,
            operations,
        )?;
        let operation = operations.allocate();
        operations.record_source_call(
            call.source_coordinate,
            Some(checked_trees::NominalMachineUseSite::Expression(
                call.source_expression,
            )),
            operation,
            call.target_machine,
            frontier,
        )?;
        operations.push(Operation::unsuspended(
            operation,
            OperationResult::Scalar(raw),
            operands.into_kind(),
        ));
        Ok(result_policy::emit(policy, raw, next_value_identity, operations)?.id)
    }
}
pub(crate) mod buffer {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{mint, operation_id};
    use lowered_psi::{
        LoweredSourceCallOccurrence, LoweredSourceCallSite, LoweredSourceFrontierBindings,
    };
    use semantic_vocabulary::{
        ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId, ValueId,
    };
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
    use crate::shared::terminal_identities::edge_id;
    pub(crate) mod restored_memberships {
        use super::SourceCallCoordinate;
        use semantic_vocabulary::{OperationId, PlaceId, StructuralDomainId};
        use terminal_psi::StructuralPathQualification;

        #[derive(Default)]
        pub(crate) struct RestoredCallMemberships {
            homes: Vec<RestoredHomeMemberships>,
        }

        struct RestoredHomeMemberships {
            place: PlaceId,
            calls: Vec<RestoredCallMembership>,
        }

        pub(crate) struct RestoredCallMembership {
            pub(crate) coordinate: SourceCallCoordinate,
            operation: OperationId,
            pub(crate) qualifications: Vec<StructuralDomainId>,
            pub(crate) projected: Vec<StructuralPathQualification>,
        }

        impl RestoredCallMemberships {
            pub(crate) fn candidates(&self, place: PlaceId) -> &[RestoredCallMembership] {
                self.homes
                    .binary_search_by_key(&place, |home| home.place)
                    .map_or(&[], |index| self.homes[index].calls.as_slice())
            }

            pub(crate) fn record(
                &mut self,
                place: PlaceId,
                coordinate: SourceCallCoordinate,
                operation: OperationId,
                qualifications: &[StructuralDomainId],
                projected: &[StructuralPathQualification],
            ) -> Result<(), &'static str> {
                let index = match self.homes.binary_search_by_key(&place, |home| home.place) {
                    Ok(index) => index,
                    Err(index) => {
                        self.homes.insert(
                            index,
                            RestoredHomeMemberships {
                                place,
                                calls: Vec::new(),
                            },
                        );
                        index
                    }
                };
                let rows = &mut self.homes[index].calls;
                let row =
                    if let Some(index) = rows.iter().position(|row| row.operation == operation) {
                        if rows[index].coordinate != coordinate {
                            return Err(
                                "restored membership operation has different source coordinates",
                            );
                        }
                        &mut rows[index]
                    } else {
                        let index = rows.len();
                        rows.push(RestoredCallMembership {
                            coordinate,
                            operation,
                            qualifications: Vec::new(),
                            projected: Vec::new(),
                        });
                        &mut rows[index]
                    };
                row.qualifications.extend_from_slice(qualifications);
                row.qualifications.sort();
                row.qualifications.dedup();
                row.projected.extend_from_slice(projected);
                row.projected.sort();
                row.projected.dedup();
                Ok(())
            }
        }
    }
    pub(crate) use lowered_psi::LoweredSourceCallOrigin as SourceInvocation;
    pub(crate) use lowered_psi::LoweredStatementCallCoordinate as SourceCallCoordinate;
    use restored_memberships::RestoredCallMemberships;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct SourceQuotientCall {
        pub(crate) value: checked_trees::CheckedStructuralValueHandle,
        pub(crate) call: OperationId,
    }

    pub(crate) struct PendingInvocationCompletion {
        pub(crate) coordinate: SourceInvocation,
        pub(crate) source_site: Option<checked_trees::NominalMachineUseSite>,
        pub(crate) target: symbols::SymbolHandle,
        pub(crate) frontier: LoweredSourceFrontierBindings,
        pub(crate) outcome: PendingInvocationOutcome,
    }

    pub(crate) enum PendingInvocationOutcome {
        ModuleNoNormal {
            callee: semantic_vocabulary::MachineId,
            inputs: crate::shared::direct_calls::ModuleInvocationArguments,
        },
        External {
            boundary: semantic_vocabulary::BoundaryMachineId,
            kind: semantic_vocabulary::ExternalCompletionKind,
            arguments: Vec<ValueId>,
            structural_arguments: Vec<terminal_psi::StructuralArgument>,
            requirement_obligations: Vec<ObligationId>,
        },
    }

    #[derive(Debug, Clone)]
    pub(crate) enum StructuralBinding {
        OperationResult(terminal_psi::StructuralOperationResult),
        ByteSequenceLiteral {
            place: PlaceId,
            structural_type: semantic_vocabulary::StructuralTypeId,
            declaration_ordinal: u32,
        },
        ProjectedAlias {
            place: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            structural_type: semantic_vocabulary::StructuralTypeId,
            multiplicity: terminal_psi::StructuralMultiplicity,
            claims: Vec<terminal_psi::StructuralResultClaimBinding>,
        },
    }

    impl StructuralBinding {
        pub(crate) fn place(&self) -> PlaceId {
            match self {
                Self::OperationResult(result) => result.place,
                Self::ByteSequenceLiteral { place, .. } | Self::ProjectedAlias { place, .. } => {
                    *place
                }
            }
        }

        pub(crate) fn path(&self) -> &[terminal_psi::StructuralPathSegment] {
            match self {
                Self::OperationResult(_) | Self::ByteSequenceLiteral { .. } => &[],
                Self::ProjectedAlias { path, .. } => path,
            }
        }

        pub(crate) fn structural_type(&self) -> semantic_vocabulary::StructuralTypeId {
            match self {
                Self::OperationResult(result) => result.structural_type,
                Self::ByteSequenceLiteral {
                    structural_type, ..
                }
                | Self::ProjectedAlias {
                    structural_type, ..
                } => *structural_type,
            }
        }

        pub(crate) fn multiplicity(&self) -> terminal_psi::StructuralMultiplicity {
            match self {
                Self::OperationResult(result) => result.multiplicity,
                Self::ByteSequenceLiteral { .. } => {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                }
                Self::ProjectedAlias { multiplicity, .. } => *multiplicity,
            }
        }

        pub(crate) fn claims(&self) -> &[terminal_psi::StructuralResultClaimBinding] {
            match self {
                Self::OperationResult(result) => &result.claims,
                Self::ByteSequenceLiteral { .. } => &[],
                Self::ProjectedAlias { claims, .. } => claims,
            }
        }

        pub(crate) fn operation_result(
            &self,
        ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
            match self {
                Self::OperationResult(result) => Ok(result),
                Self::ByteSequenceLiteral { .. } => Err(LoweringError::Unsupported(
                    "literal binding has no structural operation result",
                )),
                Self::ProjectedAlias { .. } => Err(LoweringError::Unsupported(
                    "projected alias binding has no structural operation result",
                )),
            }
        }
    }

    impl From<terminal_psi::StructuralOperationResult> for StructuralBinding {
        fn from(result: terminal_psi::StructuralOperationResult) -> Self {
            Self::OperationResult(result)
        }
    }

    pub(crate) fn scalar_result_declaration(
        id: ValueId,
        scalar_type: ScalarType,
    ) -> terminal_psi::OperationResult {
        terminal_psi::OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        })
    }

    pub(crate) fn structural_result_declaration(
        place: PlaceId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
    ) -> terminal_psi::OperationResult {
        terminal_psi::OperationResult::Structural(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            claims: Vec::new(),
        })
    }

    #[derive(Clone)]
    pub(crate) struct CapturedStructuralCaseObservation {
        pub(crate) source: PlaceId,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
        pub(crate) structural_type: semantic_vocabulary::StructuralTypeId,
        pub(crate) case: semantic_vocabulary::StructuralCaseId,
        pub(crate) value: ValueDeclaration,
        pub(crate) producer: OperationId,
    }

    #[derive(Clone, Default)]
    pub(crate) struct StorageObservations {
        pub(crate) cases: Vec<CapturedStructuralCaseObservation>,
        pub(crate) bytes: Vec<(PlaceId, ValueId)>,
        pub(crate) elements: Vec<(terminal_psi::StructuralArgument, ValueId)>,
        pub(crate) field_bytes: Vec<(
            PlaceId,
            Vec<terminal_psi::StructuralPathSegment>,
            StructuralFieldId,
            ValueId,
        )>,
    }

    impl StorageObservations {
        pub(crate) fn clear(&mut self) {
            self.cases.clear();
            self.bytes.clear();
            self.elements.clear();
            self.field_bytes.clear();
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct SourceQuotientAdaptation {
        pub(crate) value: checked_trees::CheckedStructuralValueHandle,
        pub(crate) copy: OperationId,
        pub(crate) source: PlaceId,
        pub(crate) result: PlaceId,
        pub(crate) source_type: semantic_vocabulary::StructuralTypeId,
        pub(crate) result_type: semantic_vocabulary::StructuralTypeId,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct ComputedValueBinding {
        pub(crate) fresh_storage: bool,
        pub(crate) argument: terminal_psi::StructuralArgument,
        pub(crate) structural_type: semantic_vocabulary::StructuralTypeId,
        pub(crate) observation: Result<ComputedValueObservation, LoweringError>,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct ComputedValueObservation {
        pub(crate) root_type: semantic_vocabulary::StructuralTypeId,
        pub(crate) path: Vec<PreparedStructuralPathSegment>,
        pub(crate) access: terminal_psi::StructuralAccess,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct StructuralMaterialization {
        place: PlaceId,
        state: symbols::SymbolHandle,
        statement: u32,
    }

    pub(crate) struct OperationBuffer {
        pub(crate) reference_backings: crate::emission::reference_backings::ReferenceBackings,
        pub(crate) source_returns:
            Vec<crate::expression_preparation::qualifications::SourceScalarReturn>,
        pub(crate) computed_values: Vec<(
            checked_trees::CheckedStructuralValueHandle,
            ComputedValueBinding,
        )>,
        pub(crate) claim_edge_bindings: Vec<crate::emission::claim_bindings::ClaimEdgeBinding>,
        pub(crate) arrival_claims: Vec<(PlaceId, Vec<terminal_psi::StructuralResultClaimBinding>)>,
        pub(crate) block_entry_claims: Vec<terminal_psi::EntryClaim>,
        pub(crate) claim_introductions: Vec<crate::emission::claim_bindings::ClaimIntroduction>,
        pub(crate) completed_cleanup_frontiers:
            Vec<crate::emission::selected_continuations::CompletedCleanupFrontier>,
        pub(crate) selected_cleanup_groups:
            Vec<crate::emission::selected_continuations::SelectedCleanupGroup>,
        pub(crate) invocation_completion: Option<PendingInvocationCompletion>,
        pub(crate) structural_values: Vec<(u32, StructuralBinding)>,
        pub(crate) structural_establishments: Vec<PlaceId>,
        structural_materializations: Vec<StructuralMaterialization>,
        pub(crate) literal_types: Vec<(PlaceId, semantic_vocabulary::StructuralTypeId)>,
        pub(crate) captured_place_types: Vec<(PlaceId, semantic_vocabulary::StructuralTypeId)>,
        pub(crate) scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
        pub(crate) scalar_index_coordinates: Vec<(ObligationId, ValueId, u64)>,
        pub(crate) selected_comparisons: Vec<lowered_psi::LoweredSelectedComparisonOccurrence>,
        pub(crate) source_checked_obligations: Vec<ObligationId>,
        pub(crate) next_identity: u64,
        pub(crate) operations: Vec<Operation>,
        pub(crate) storage_observations: StorageObservations,
        pub(crate) source_calls: Vec<LoweredSourceCallOccurrence>,
        next_observation_scope: u64,
        pub(crate) quotient_adaptations: Vec<SourceQuotientAdaptation>,
        pub(crate) quotient_calls: Vec<SourceQuotientCall>,
        pub(crate) loan_captures: Vec<lowered_psi::LoweredSourceLoanCapture>,
        pub(crate) reference_formations:
            std::sync::Arc<crate::emission::reference_formations::ReferenceFormations>,
        pub(crate) loan_completions: Vec<terminal_psi::LoanCompletion>,
        restored_memberships: RestoredCallMemberships,
        pub(crate) dynamic_dispatch:
            std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
    }

    impl OperationBuffer {
        pub(crate) fn reserve_observation_scope(&mut self) -> Result<u64, LoweringError> {
            let scope = self.next_observation_scope;
            self.next_observation_scope =
                scope.checked_add(1).ok_or(LoweringError::Unsupported(
                    "declaration observation identity exceeds the producer's u64 scope space",
                ))?;
            Ok(scope)
        }

        pub(crate) fn declared_source_claims(
            &self,
            argument: &terminal_psi::StructuralArgument,
        ) -> Result<Option<Vec<terminal_psi::StructuralResultClaimBinding>>, LoweringError>
        {
            let mut selected: Option<(usize, Vec<terminal_psi::StructuralResultClaimBinding>)> =
                None;
            for (_, binding) in &self.structural_values {
                if binding.place() != argument.place {
                    continue;
                }
                let Some(suffix) = argument.path.strip_prefix(binding.path()) else {
                    continue;
                };
                let claims = binding
                    .claims()
                    .iter()
                    .filter_map(|claim| {
                        claim.path.strip_prefix(suffix).map(|path| {
                            terminal_psi::StructuralResultClaimBinding {
                                claim: claim.claim,
                                path: path.to_vec(),
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                let depth = binding.path().len();
                if let Some((previous_depth, previous)) = &selected {
                    if *previous_depth > depth {
                        continue;
                    }
                    if *previous_depth == depth && previous != &claims {
                        return unsupported("completed source has conflicting custody bindings");
                    }
                }
                selected = Some((depth, claims));
            }
            if let Some((_, claims)) = selected {
                return Ok(Some(claims));
            }
            let candidates = self
                .operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .filter(|result| result.place == argument.place)
                .map(|result| result.claims.as_slice())
                .chain(self.arrival_claims.iter().filter_map(|(place, claims)| {
                    (*place == argument.place).then_some(claims.as_slice())
                }));
            let mut claims: Option<&[terminal_psi::StructuralResultClaimBinding]> = None;
            for candidate in candidates {
                if claims.is_some_and(|previous| previous != candidate) {
                    return unsupported("completed place has conflicting custody declarations");
                }
                claims = Some(candidate);
            }
            Ok(claims.map(|claims| {
                claims
                    .iter()
                    .filter_map(|claim| {
                        claim
                            .path
                            .strip_prefix(argument.path.as_slice())
                            .map(|path| terminal_psi::StructuralResultClaimBinding {
                                claim: claim.claim,
                                path: path.to_vec(),
                            })
                    })
                    .collect()
            }))
        }

        pub(crate) fn push(&mut self, operation: Operation) {
            use crate::emission::claim_bindings::ClaimIntroduction;
            let transferred = match &operation.kind {
                OperationKind::CallDynamic {
                    claim_transfers, ..
                }
                | OperationKind::CallUnit {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralScalar {
                    claim_transfers, ..
                }
                | OperationKind::CallStructural {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    claim_transfers, ..
                } => Some(claim_transfers.as_slice()),
                OperationKind::Call { .. } | OperationKind::BoundaryCall { .. } => Some(&[][..]),
                _ => None,
            };
            if let (Some(transferred), Some(result)) =
                (transferred, operation.result.as_structural())
            {
                self.claim_introductions.extend(
                    result
                        .claims
                        .iter()
                        .filter(|binding| {
                            !transferred.iter().any(|input| input.claim == binding.claim)
                        })
                        .map(|binding| ClaimIntroduction {
                            operation: operation.id,
                            claim: binding.claim,
                        }),
                );
            }
            if let Some(result) = operation.result.structural() {
                match &operation.kind {
                    OperationKind::MoveStructuralValue { source } => {
                        self.reference_backings.forward(source, result.place);
                    }
                    OperationKind::StructuralLeafCopy { source, path } if path.is_empty() => {
                        self.reference_backings.forward(
                            &terminal_psi::StructuralArgument {
                                place: *source,
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::Owned,
                            },
                            result.place,
                        );
                    }
                    _ => {}
                }
                self.structural_establishments.push(result.place);
            } else if let OperationKind::EstablishTrivialAffineLocal { destination }
            | OperationKind::EstablishByteSequenceLiteral { destination, .. } =
                &operation.kind
            {
                self.retain_structural_establishment(*destination);
            }
            self.operations.push(operation);
        }

        pub(crate) fn bind_structural_value(&mut self, ordinal: u32, value: StructuralBinding) {
            if value.path().is_empty() {
                self.retain_structural_establishment(value.place());
            }
            self.structural_values.push((ordinal, value));
        }

        fn retain_structural_establishment(&mut self, place: PlaceId) {
            if !self.structural_establishments.contains(&place) {
                self.structural_establishments.push(place);
            }
        }

        pub(crate) fn register_structural_materialization(
            &mut self,
            place: PlaceId,
            state: symbols::SymbolHandle,
            statement: u32,
        ) -> Result<(), LoweringError> {
            let point = StructuralMaterialization {
                place,
                state,
                statement,
            };
            if let Some(existing) = self
                .structural_materializations
                .iter()
                .find(|row| row.place == place)
            {
                if *existing != point {
                    return Err(LoweringError::Unsupported(
                        "one structural home has conflicting checked materializations",
                    ));
                }
                return Ok(());
            }
            self.structural_materializations.push(point);
            self.retain_structural_establishment(place);
            Ok(())
        }

        pub(crate) fn structural_materialization_statement(
            &self,
            place: PlaceId,
            state: symbols::SymbolHandle,
        ) -> Result<Option<u32>, LoweringError> {
            let Some(point) = self
                .structural_materializations
                .iter()
                .find(|row| row.place == place)
            else {
                return Ok(None);
            };
            if point.state != state {
                return Err(LoweringError::Unsupported(
                    "cleanup home materialization belongs to another checked state",
                ));
            }
            Ok(Some(point.statement))
        }

        pub(crate) fn structural_result_contract(
            &self,
            place: PlaceId,
        ) -> Option<&terminal_psi::StructuralOperationResult> {
            self.operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .chain(
                    self.structural_values
                        .iter()
                        .filter_map(|(_, binding)| match binding {
                            StructuralBinding::OperationResult(result) => Some(result),
                            _ => None,
                        }),
                )
                .find(|result| result.place == place)
        }

        pub(crate) fn set_structural_result_claims(
            &mut self,
            place: PlaceId,
            claims: Vec<terminal_psi::StructuralResultClaimBinding>,
        ) -> Result<(), LoweringError> {
            let result = self
                .operations
                .iter_mut()
                .rev()
                .find_map(|operation| match &mut operation.result {
                    OperationResult::Structural(result) if result.place == place => Some(result),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "constructed value lost its result claim home",
                ))?;
            result.claims = claims;
            Ok(())
        }

        pub(crate) fn record_call_memberships(
            &mut self,
            coordinate: SourceCallCoordinate,
            place: PlaceId,
            qualifications: &[semantic_vocabulary::StructuralDomainId],
            projected: &[terminal_psi::StructuralPathQualification],
        ) -> Result<(), &'static str> {
            let mut calls = self.source_calls.iter().filter(|call| {
                call.matches_statement(
                    coordinate.state,
                    coordinate.statement_index,
                    coordinate.call_ordinal,
                )
            });
            let call = calls
                .next()
                .ok_or("restored memberships have no emitted source call")?;
            if calls.next().is_some() {
                return Err("restored memberships have multiple emitted source calls");
            }
            let LoweredSourceCallSite::Operation(operation) = call.terminal_site else {
                return Err("restored memberships require a normal call completion");
            };
            self.restored_memberships.record(
                place,
                coordinate,
                operation,
                qualifications,
                projected,
            )
        }

        pub(crate) fn call_membership_candidates(
            &self,
            place: PlaceId,
            state: symbols::SymbolHandle,
            statement: u32,
        ) -> impl Iterator<Item = &restored_memberships::RestoredCallMembership> {
            self.restored_memberships
                .candidates(place)
                .iter()
                .filter(move |row| {
                    row.coordinate.state == state
                        && row.coordinate.statement_index <= statement as usize
                })
        }

        pub(crate) fn push_read_obligation(
            &mut self,
            bound_decided: bool,
        ) -> semantic_vocabulary::ObligationId {
            let obligation = crate::shared::terminal_identities::obligation_id(
                self.next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            if bound_decided {
                self.source_checked_obligations.push(obligation);
            }
            obligation
        }

        pub(crate) fn new(
            identity_base: u64,
            first_place: u64,
            reference_formations: std::sync::Arc<
                crate::emission::reference_formations::ReferenceFormations,
            >,
            dynamic_dispatch: std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
        ) -> Self {
            Self {
                reference_backings: crate::emission::reference_backings::ReferenceBackings::new(
                    first_place,
                ),
                source_returns: Vec::new(),
                arrival_claims: Vec::new(),
                block_entry_claims: Vec::new(),
                invocation_completion: None,
                dynamic_dispatch,
                structural_values: Vec::new(),
                computed_values: Vec::new(),
                structural_establishments: Vec::new(),
                structural_materializations: Vec::new(),
                literal_types: Vec::new(),
                captured_place_types: Vec::new(),
                scalar_block_invariants: Vec::new(),
                scalar_index_coordinates: Vec::new(),
                next_identity: identity_base
                    .checked_add(1)
                    .expect("operation identity base admits one-based identities"),
                operations: Vec::new(),
                storage_observations: StorageObservations::default(),
                source_calls: Vec::new(),
                next_observation_scope: 0,
                quotient_adaptations: Vec::new(),
                quotient_calls: Vec::new(),
                loan_captures: Vec::new(),
                reference_formations,
                loan_completions: Vec::new(),
                restored_memberships: RestoredCallMemberships::default(),
                selected_comparisons: Vec::new(),
                source_checked_obligations: Vec::new(),
                claim_introductions: Vec::new(),
                claim_edge_bindings: Vec::new(),
                selected_cleanup_groups: Vec::new(),
                completed_cleanup_frontiers: Vec::new(),
            }
        }

        pub(crate) fn allocate(&mut self) -> OperationId {
            mint(&mut self.next_identity, operation_id)
        }

        pub(crate) fn push_leaf(
            &mut self,
            result: OperationResult,
            kind: OperationKind,
        ) -> OperationId {
            let id = self.allocate();
            self.push(Operation::unsuspended(id, result, kind));
            id
        }

        pub(crate) fn invalidate_field_lengths_under(
            &mut self,
            place: PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
        ) {
            self.storage_observations.elements.retain(|(source, _)| {
                source.place != place
                    || !terminal_semantics::structural_paths_may_overlap(&source.path, path)
            });
            self.storage_observations
                .field_bytes
                .retain(|(root, carrier, _, _)| {
                    *root != place
                        || !terminal_semantics::structural_paths_may_overlap(carrier, path)
                });
        }

        pub(crate) fn record_source_call(
            &mut self,
            coordinate: impl Into<SourceInvocation>,
            source_site: Option<checked_trees::NominalMachineUseSite>,
            operation: OperationId,
            target: symbols::SymbolHandle,
            frontier_before_call: &LoweredSourceFrontierBindings,
        ) -> Result<(), LoweringError> {
            self.record_source_call_site(
                coordinate.into(),
                source_site,
                LoweredSourceCallSite::Operation(operation),
                target,
                frontier_before_call,
            )
        }

        fn record_source_call_site(
            &mut self,
            coordinate: SourceInvocation,
            source_site: Option<checked_trees::NominalMachineUseSite>,
            terminal_site: LoweredSourceCallSite,
            target: symbols::SymbolHandle,
            frontier_before_call: &LoweredSourceFrontierBindings,
        ) -> Result<(), LoweringError> {
            if let Some(existing) = self
                .source_calls
                .iter()
                .find(|existing| existing.origin == coordinate)
            {
                let Some(coordinate) = coordinate.statement() else {
                    return unsupported(
                        "declaration call emitted twice in the same observation scope",
                    );
                };
                return Err(LoweringError::DuplicateContentPartitionProducerCoordinate {
                    state: coordinate.state,
                    statement_index: coordinate.statement_index,
                    call_ordinal: coordinate.call_ordinal,
                    previous_target: existing.source_target,
                    requested_target: target,
                    previous_source_site: existing.source_site,
                    requested_source_site: source_site,
                    previous_terminal_site: existing.terminal_site,
                    requested_terminal_site: terminal_site,
                    source_names: None,
                });
            }
            self.source_calls.push(LoweredSourceCallOccurrence {
                source_site,
                origin: coordinate,
                terminal_site,
                source_target: target,
                frontier_before_call: frontier_before_call.clone(),
            });
            Ok(())
        }

        pub(crate) fn take_invocation_completion(
            &mut self,
            next_edge: &mut u64,
        ) -> Result<Option<terminal_psi::Terminator>, LoweringError> {
            let Some(completion) = self.invocation_completion.take() else {
                return Ok(None);
            };
            let edge = edge_id(crate::shared::terminal_identities::allocate_dense(
                next_edge,
            )?);
            let (site, terminator) = match completion.outcome {
                PendingInvocationOutcome::ModuleNoNormal { callee, inputs } => {
                    let crate::shared::direct_calls::ModuleInvocationArguments {
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    } = inputs;
                    (
                        LoweredSourceCallSite::InvokeNoNormal(edge),
                        terminal_psi::Terminator::InvokeNoNormal {
                            static_reach_binding: None,
                            edge,
                            callee,
                            arguments,
                            erased_arguments,
                            erased_proof_arguments,
                            structural_arguments,
                            claim_transfers,
                            requirement_obligations,
                            crash_continuations,
                        },
                    )
                }
                PendingInvocationOutcome::External {
                    boundary,
                    kind,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                } => (
                    LoweredSourceCallSite::ExternalCompletion(edge),
                    terminal_psi::Terminator::ExternalCompletion {
                        edge,
                        boundary,
                        kind,
                        arguments,
                        structural_arguments,
                        requirement_obligations,
                        frontier_lower_bound: Vec::new(),
                    },
                ),
            };
            self.record_source_call_site(
                completion.coordinate,
                completion.source_site,
                site,
                completion.target,
                &completion.frontier,
            )?;
            Ok(Some(terminator))
        }
    }

    impl std::ops::Deref for OperationBuffer {
        type Target = Vec<Operation>;

        fn deref(&self) -> &Self::Target {
            &self.operations
        }
    }

    impl std::ops::DerefMut for OperationBuffer {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.operations
        }
    }
}
pub(crate) mod calls {

    use super::buffer::{OperationBuffer, SourceInvocation};
    use super::expressions::{LoweredDirectExpression, emit_direct_expression};
    use crate::expression_preparation::bindings::structural_paths::structural_argument;
    use crate::proofs::crash_routes::lowered_erased_scalar_term;
    use crate::shared::direct_calls::DirectCallResult;
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use crate::shared::terminal_identities::mint;
    use crate::shared::terminal_identities::obligation_id;
    use crate::shared::terminal_identities::value_id;
    use semantic_vocabulary::{MachineId, ObligationId, QualifiedScalarType, ValueId};
    use terminal_psi::{StructuralArgument, ValueDeclaration};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum StructuralValueSource {
        Place(semantic_vocabulary::PlaceId),
        Computed(checked_trees::CheckedStructuralValueHandle),
    }

    impl StructuralValueSource {
        pub(crate) fn known_place(self) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
            match self {
                Self::Place(place) => Ok(place),
                Self::Computed(_) => {
                    unsupported("structural value has not completed physical binding")
                }
            }
        }

        pub(crate) fn resolve(
            self,
            access: terminal_psi::StructuralAccess,
            operations: &OperationBuffer,
        ) -> Result<StructuralArgument, LoweringError> {
            match self {
                Self::Place(place) => Ok(StructuralArgument::whole(place, access)),
                Self::Computed(value) => {
                    let binding = operations
                        .computed_values
                        .iter()
                        .rev()
                        .find(|(candidate, _)| *candidate == value)
                        .map(|(_, binding)| binding)
                        .ok_or(LoweringError::Unsupported(
                            "computed operand has no completed binding",
                        ))?;
                    let compatible = match access {
                        terminal_psi::StructuralAccess::Owned => {
                            binding.argument.access == terminal_psi::StructuralAccess::Owned
                        }
                        access => access.admits(binding.argument.access),
                    };
                    if !compatible {
                        return unsupported("computed operand cannot strengthen completed access");
                    }
                    Ok(StructuralArgument {
                        place: binding.argument.place,
                        path: binding.argument.path.clone(),
                        access,
                    })
                }
            }
        }
    }

    impl From<semantic_vocabulary::PlaceId> for StructuralValueSource {
        fn from(place: semantic_vocabulary::PlaceId) -> Self {
            Self::Place(place)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PreparedStructuralArgument {
        pub(crate) source: StructuralValueSource,
        pub(crate) root_type: Option<semantic_vocabulary::StructuralTypeId>,
        pub(crate) access: terminal_psi::StructuralAccess,
        pub(crate) path: Vec<
            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment,
        >,
        pub(crate) selector_positions: Vec<usize>,
    }

    impl PreparedStructuralArgument {
        pub(crate) fn from_static(
            argument: StructuralArgument,
            root_type: Option<semantic_vocabulary::StructuralTypeId>,
        ) -> Result<Self, LoweringError> {
            let path = argument.path.into_iter().map(|segment| {
                if matches!(segment, terminal_psi::StructuralPathSegment::RuntimeIndex { .. }) {
                    return unsupported("prepared call cannot capture another operation's runtime path");
                }
                Ok(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static(segment))
            }).collect::<Result<Vec<_>, _>>()?;
            Ok(Self {
                root_type,
                source: argument.place.into(),
                access: argument.access,
                path,
                selector_positions: Vec::new(),
            })
        }

        pub(crate) fn emit(
            &self,
            values: &[ValueDeclaration],
            calls: &mut CallEmissionContext<'_>,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<StructuralArgument, LoweringError> {
            let source = self.source.resolve(self.access, operations)?;
            let selectors = self
                .selector_positions
                .iter()
                .map(|position| {
                    let value = values.get(*position).ok_or(LoweringError::Unsupported(
                        "captured selector has no value in the call input frame",
                    ))?;
                    if !matches!(
                        value.scalar_type,
                        semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address()
                    ) {
                        return unsupported("captured selector is not an integer");
                    }
                    Ok(value.id)
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut selectors = selectors.into_iter();
            let mut prepared_path = source.path.iter().cloned()
                .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                .collect::<Vec<_>>();
            prepared_path.extend_from_slice(&self.path);
            let path =
                crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                    &prepared_path,
                    source.place,
                    Some(values),
                    &mut selectors,
                    Some(calls),
                    Some((next_value, operations)),
                )?;
            if selectors.next().is_some() {
                return unsupported("captured selectors exceed the structural argument path");
            }
            Ok(structural_argument(source.place, path, self.access))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredDirectCallBinding {
        pub(crate) source_coordinate: SourceInvocation,
        pub(crate) caller_types: crate::expression_preparation::bindings::PublishedStructuralTypes,
        pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
        pub(crate) target_machine: symbols::SymbolHandle,
        pub(crate) result_type: QualifiedScalarType,
        pub(crate) arguments: Vec<LoweredDirectExpression>,
        pub(crate) erased_arguments: Vec<LoweredDirectExpression>,
        pub(crate) erased_proof_arguments: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
        pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
        pub(crate) claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
        pub(crate) crash_continuations: Vec<checked_trees::CrashRouteBucket>,
        pub(crate) parameter_relative_crash_routes: Vec<checked_trees::CrashRouteBucket>,
    }

    pub(crate) struct CallEmissionContext<'a> {
        pub(crate) terminal_machine: MachineId,
        pub(crate) service_ids: &'a [(
            language_semantics::ServiceReachId,
            semantic_vocabulary::ServiceId,
        )],
        pub(crate) domain_ids: &'a [crate::shared::terminal_identities::StructuralDomainBinding],
        pub(crate) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
        pub(crate) scalar_qualifications: &'a terminal_psi::ScalarQualificationCatalog,
        pub(crate) claims: crate::emission::claim_bindings::ClaimBindings<'a>,
        pub(crate) cleanup_receivers: &'a [(symbols::SymbolHandle, semantic_vocabulary::PlaceId)],
        pub(crate) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
        pub(crate) signatures: &'a [crate::unit::attached_unit::signatures::MachineSignature],
        pub(crate) structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
        pub(crate) requirement_counts: &'a [(symbols::SymbolHandle, usize)],
        pub(crate) boundaries: &'a [crate::unit::attached_unit::BoundaryParameters],
        pub(crate) next_obligation_identity: u64,
        pub(crate) obligation_limit: u64,
    }

    impl CallEmissionContext<'_> {
        pub(crate) fn allocate_requirement(&mut self) -> Result<ObligationId, LoweringError> {
            if self.next_obligation_identity >= self.obligation_limit {
                return unsupported(
                    "terminal call obligations exceed their machine identity range",
                );
            }
            let obligation = obligation_id(self.next_obligation_identity);
            self.next_obligation_identity = self
                .next_obligation_identity
                .checked_add(1)
                .expect("terminal call obligation identities advance");
            Ok(obligation)
        }
    }

    pub(crate) fn emit_scalar_call_binding(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        call: &LoweredDirectCallBinding,
        parameters: &[ValueDeclaration],
        frontier: &lowered_psi::LoweredSourceFrontierBindings,
        caller_erased_formals: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
        call_emission: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<ValueId, LoweringError> {
        let parameter_types = parameters
            .iter()
            .map(|parameter| parameter.value_type())
            .collect::<Vec<_>>();
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                let value_type = argument.value_type(&parameter_types)?;
                Ok(ValueDeclaration {
                    qualifications: value_type.qualifications,
                    id: emit_direct_expression(
                        argument,
                        parameters,
                        next_value_identity,
                        operations,
                    ),
                    scalar_type: value_type.scalar_type,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        emit_direct_call_operation(
            identities,
            checked,
            call,
            &call.parameter_relative_crash_routes,
            parameters,
            frontier,
            caller_erased_formals,
            &arguments,
            next_value_identity,
            operations,
            call_emission,
            current_claims,
        )
    }

    fn emit_direct_call_operation(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        call: &LoweredDirectCallBinding,
        crash_routes: &[checked_trees::CrashRouteBucket],
        source_values_before_call: &[ValueDeclaration],
        frontier: &lowered_psi::LoweredSourceFrontierBindings,
        caller_erased_formals: &[ValueDeclaration],
        arguments: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
        call_emission: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<ValueId, LoweringError> {
        let callee = crate::shared::terminal_identities::lookup_machine_id_with(
            call_emission.machine_ids,
            call.target_machine,
            "direct scalar call target is absent from the terminal closure",
        )?;
        let result = mint(next_value_identity, value_id);
        let erased_arguments = call
            .erased_arguments
            .iter()
            .map(|argument| {
                lowered_erased_scalar_term(
                    argument,
                    source_values_before_call,
                    caller_erased_formals,
                )
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        crate::unit::attached_unit::ordinary_calls::prepare_computed_module_call(
            identities,
            checked,
            call.source_coordinate,
            &call.caller_types,
            call.target_machine,
            arguments,
            erased_arguments,
            &call.erased_proof_arguments,
            &call.structural_arguments,
            &call.claim_transfers,
            crash_routes,
            source_values_before_call,
            caller_erased_formals,
            call_emission,
            next_value_identity,
            operations,
            current_claims,
        )?
        .emit(
            checked,
            operations,
            call.source_coordinate,
            crate::shared::direct_calls::CallRecord::Frontier(frontier),
            terminal_psi::OperationResult::Scalar(ValueDeclaration {
                qualifications: call.result_type.qualifications,
                id: result,
                scalar_type: call.result_type.scalar_type,
            }),
            callee,
            DirectCallResult::Scalar,
        )?;
        Ok(result)
    }
}
pub(crate) mod descriptor_tables {

    use super::buffer::{OperationBuffer, SourceCallCoordinate};
    use super::calls::{CallEmissionContext, PreparedStructuralArgument};
    use super::expressions::{LoweredDirectExpression, emit_direct_expression};
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::terminal_identities::{lookup_service_id, mint_with, value_id};
    use language_core::descriptor_tables::DescriptorTableOperation;
    use semantic_vocabulary::{QualifiedScalarType, ServiceId, ValueId};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralArgument, ValueDeclaration,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredDescriptorTableBinding {
        pub(crate) source_coordinate: SourceCallCoordinate,
        pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
        pub(crate) source_site: checked_trees::NominalMachineUseSite,
        pub(crate) target_machine: symbols::SymbolHandle,
        pub(crate) result_type: QualifiedScalarType,
        pub(crate) operation: DescriptorTableOperation,
        pub(crate) service: language_semantics::ServiceReachId,
        pub(crate) arguments: Vec<LoweredDirectExpression>,
        pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
    }

    pub(crate) fn operation_kind(
        action: DescriptorTableOperation,
        service: ServiceId,
        arguments: &[ValueDeclaration],
        storage: Vec<StructuralArgument>,
    ) -> Result<OperationKind, LoweringError> {
        let missing = |operand: &str| {
            LoweringError::InvalidDeclarationIdentity(format!(
                "checked descriptor instruction lost its {operand} operand"
            ))
        };
        Ok(match action {
            DescriptorTableOperation::ActivateTask { entries } => {
                let selector = arguments.first().ok_or_else(|| missing("selector"))?.id;
                let mut storage = storage.into_iter();
                OperationKind::ActivateTaskRegister {
                    service,
                    entries,
                    selector,
                    gdt: storage.next().ok_or_else(|| missing("GDT"))?,
                    task_state: storage.next().ok_or_else(|| missing("task-state"))?,
                    scratch: storage.next().ok_or_else(|| missing("scratch"))?,
                }
            }
            DescriptorTableOperation::ReadTask => OperationKind::ReadTaskRegister { service },
            DescriptorTableOperation::ReloadSegments { entries, kind } => {
                let selector = arguments.first().ok_or_else(|| missing("selector"))?.id;
                let mut storage = storage.into_iter();
                OperationKind::ReloadSegmentDescriptors {
                    service,
                    entries,
                    kind,
                    selector,
                    gdt: storage.next().ok_or_else(|| missing("GDT"))?,
                    scratch: storage.next().ok_or_else(|| missing("scratch"))?,
                }
            }
            DescriptorTableOperation::ReadSegment { register } => {
                OperationKind::ReadSegmentSelector { service, register }
            }
        })
    }

    pub(crate) fn emit(
        call: &LoweredDescriptorTableBinding,
        parameters: &[ValueDeclaration],
        frontier: &lowered_psi::LoweredSourceFrontierBindings,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueId, LoweringError> {
        let service = lookup_service_id(calls.service_ids, call.service)?;
        let parameter_types = parameters
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                let value_type = argument.value_type(&parameter_types)?;
                Ok(ValueDeclaration {
                    id: emit_direct_expression(
                        argument,
                        parameters,
                        next_value_identity,
                        operations,
                    ),
                    scalar_type: value_type.scalar_type,
                    qualifications: value_type.qualifications,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let storage = call
            .structural_arguments
            .iter()
            .map(|argument| argument.emit(parameters, calls, next_value_identity, operations))
            .collect::<Result<Vec<_>, _>>()?;
        let kind = operation_kind(call.operation, service, &arguments, storage)?;
        let result = mint_with(
            next_value_identity,
            value_id,
            "descriptor result identity overflow",
        )?;
        let operation = operations.allocate();
        operations.record_source_call(
            call.source_coordinate,
            Some(call.source_site),
            operation,
            call.target_machine,
            frontier,
        )?;
        operations.push(Operation::unsuspended(
            operation,
            OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: call.result_type.scalar_type,
                qualifications: call.result_type.qualifications,
            }),
            kind,
        ));
        Ok(result)
    }
}
pub(crate) mod dynamic_calls {

    use super::buffer::{OperationBuffer, SourceCallCoordinate};
    use super::calls::{CallEmissionContext, PreparedStructuralArgument};
    use super::expressions::{LoweredDirectExpression, emit_direct_expression};
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{mint_with, value_id};
    use semantic_vocabulary::OperationId;
    use semantic_vocabulary::{QualifiedScalarType, ValueId};
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredDynamicCallBinding {
        pub(crate) source_coordinate: SourceCallCoordinate,
        pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
        pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
        pub(crate) slot: symbols::SymbolHandle,
        pub(crate) interface_identity: typed_trees::type_identity::TypeIdentityId,
        pub(crate) requirement_identity: typed_trees::type_identity::CallableIdentityId,
        pub(crate) family_tuple: Vec<String>,
        pub(crate) result_type: QualifiedScalarType,
        pub(crate) selector: LoweredDirectExpression,
        pub(crate) arguments: Vec<LoweredDirectExpression>,
        pub(crate) family_actuals: Vec<LoweredDirectExpression>,
        pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
        pub(crate) claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
    }

    pub(crate) fn emit_dynamic_call_binding(
        call: &LoweredDynamicCallBinding,
        parameters: &[ValueDeclaration],
        frontier: &lowered_psi::LoweredSourceFrontierBindings,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
        call_emission: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<ValueId, LoweringError> {
        if !call.result_type.qualifications.is_empty() {
            return unsupported("a selector call's result carries qualifications");
        }
        if !call.family_actuals.is_empty() && call.family_tuple.is_empty() {
            return unsupported("finite-family selector call outside its dispatch expansion");
        }
        let (interface, slot) = if call.family_tuple.is_empty() {
            operations
                .dynamic_dispatch
                .resolve_slot(&call.interface_identity, &call.requirement_identity)?
        } else {
            operations.dynamic_dispatch.resolve_tuple(
                &call.interface_identity,
                &call.requirement_identity,
                &call.family_tuple,
            )?
        };
        let selector =
            emit_direct_expression(&call.selector, parameters, next_value_identity, operations);
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                emit_direct_expression(argument, parameters, next_value_identity, operations)
            })
            .collect();
        let family_actuals = call
            .family_actuals
            .iter()
            .map(|actual| {
                emit_direct_expression(actual, parameters, next_value_identity, operations)
            })
            .collect();
        let structural_arguments = call
            .structural_arguments
            .iter()
            .map(|argument| {
                argument.emit(parameters, call_emission, next_value_identity, operations)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
            &call.claim_transfers,
            call_emission.claims.bindings(),
            &structural_arguments,
            current_claims,
        )?;
        let result = mint_with(
            next_value_identity,
            value_id,
            "selector call value identity overflow",
        )?;
        let operation = operations.allocate();
        operations.record_source_call(
            call.source_coordinate,
            Some(checked_trees::NominalMachineUseSite::Expression(
                call.source_expression,
            )),
            operation,
            call.slot,
            frontier,
        )?;
        emit_dynamic_call_leaf(
            selector,
            interface,
            slot,
            arguments,
            family_actuals,
            structural_arguments,
            claim_transfers,
            OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: call.result_type.scalar_type,
                qualifications: Default::default(),
            }),
            operation,
            operations,
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_dynamic_call_leaf(
        selector: ValueId,
        interface: u32,
        slot: u32,
        arguments: Vec<ValueId>,
        family_actuals: Vec<ValueId>,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        claim_transfers: Vec<terminal_psi::ClaimTransfer>,
        produced: OperationResult,
        operation: OperationId,
        operations: &mut OperationBuffer,
    ) {
        operations.push(Operation::unsuspended(
            operation,
            produced,
            OperationKind::CallDynamic {
                selector,
                interface,
                slot,
                arguments,
                family_actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments,
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_completed_dynamic_call(
        interface_identity: &typed_trees::type_identity::TypeIdentityId,
        requirement_identity: &typed_trees::type_identity::CallableIdentityId,
        family_tuple: &[String],
        operands: &[ValueDeclaration],
        family_actuals: &[ValueDeclaration],
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        claim_transfers: Vec<terminal_psi::ClaimTransfer>,
        result: &OperationResult,
        operation: OperationId,
        span: &mut crate::unit::attached_unit::argument_evaluation::BlockEmissionSpan,
        blocks: &mut Vec<terminal_psi::Block>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        mut structural_result_place: impl FnMut(
            usize,
        )
            -> Result<semantic_vocabulary::PlaceId, LoweringError>,
    ) -> Result<bool, LoweringError> {
        let (selector, arguments) = operands.split_first().ok_or(LoweringError::Unsupported(
            "selector call has no evaluated selector operand",
        ))?;
        let (interface, slot) = if family_tuple.is_empty() {
            operations
                .dynamic_dispatch
                .resolve_slot(interface_identity, requirement_identity)?
        } else {
            operations.dynamic_dispatch.resolve_tuple(
                interface_identity,
                requirement_identity,
                family_tuple,
            )?
        };
        let arguments = arguments.iter().map(|value| value.id).collect::<Vec<_>>();
        let actuals = family_actuals
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>();
        if family_actuals.is_empty() || !family_tuple.is_empty() {
            emit_dynamic_call_leaf(
                selector.id,
                interface,
                slot,
                arguments,
                actuals,
                structural_arguments,
                claim_transfers,
                result.clone(),
                operation,
                operations,
            );
            return Ok(false);
        }
        let roster =
            operations
                .dynamic_dispatch
                .family_roster(interface, slot, requirement_identity)?;
        let actual_types = family_actuals
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let mut first_leaf = Some(operation);
        crate::unit::attached_unit::argument_evaluation::emit_dynamic_family_dispatch(
            span,
            blocks,
            operations,
            next_value,
            next_block,
            next_edge,
            &roster,
            &actuals,
            &actual_types,
            result,
            |operations, next_value, tuple| {
                let produced = match result {
                    OperationResult::Unit => OperationResult::Unit,
                    OperationResult::Scalar(value) => OperationResult::Scalar(ValueDeclaration {
                        id: value_id(crate::shared::terminal_identities::allocate_dense(
                            next_value,
                        )?),
                        ..*value
                    }),
                    OperationResult::Structural(value) => {
                        let mut value = value.clone();
                        value.place = structural_result_place(tuple)?;
                        OperationResult::Structural(value)
                    }
                };
                let operation = first_leaf.take().unwrap_or_else(|| operations.allocate());
                let slot =
                    slot.checked_add(u32::try_from(tuple).map_err(|_| {
                        LoweringError::Unsupported("dynamic family slot overflows")
                    })?)
                    .ok_or(LoweringError::Unsupported("dynamic family slot overflows"))?;
                emit_dynamic_call_leaf(
                    selector.id,
                    interface,
                    slot,
                    arguments.clone(),
                    actuals.clone(),
                    structural_arguments.clone(),
                    claim_transfers.clone(),
                    produced.clone(),
                    operation,
                    operations,
                );
                Ok(produced)
            },
        )?;
        Ok(true)
    }
}
pub(crate) mod expressions {

    use super::boolean::{LoweredBooleanReturnExpression, emit_boolean_expression};
    use super::buffer::OperationBuffer;
    use super::integer::LoweredIntegerBinaryKind;
    use crate::emission::operation_emission::buffer::{
        scalar_result_declaration, structural_result_declaration,
    };
    use crate::expression_preparation::saturating_shift::expansion;
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use crate::shared::terminal_identities::mint;
    use crate::shared::terminal_identities::obligation_id;
    use crate::shared::terminal_identities::place_id;
    use crate::shared::terminal_identities::value_id;
    use semantic_vocabulary::{
        IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, QualifiedScalarType,
        ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
    };
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

    const TERMINAL_HOISTED_ELEMENT_PLACE_BASE: u64 = 1 << 62;

    const TERMINAL_NESTED_SELECTOR_OBLIGATION_BASE: u64 = 1 << 61;
    pub(crate) const TERMINAL_NESTED_SELECTOR_LIMIT: usize = 7;
    const TERMINAL_NESTED_SELECTOR_STRIDE: u64 = 8;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum PrimitiveReadSource {
        Place(PlaceId),
        CaseLeaf {
            source: PlaceId,
            path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            structural_type: StructuralTypeId,
        },
    }

    impl From<PlaceId> for PrimitiveReadSource {
        fn from(place: PlaceId) -> Self {
            Self::Place(place)
        }
    }

    impl PrimitiveReadSource {
        pub(crate) fn field_origin(
            &self,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        ) -> (
            PlaceId,
            Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        ) {
            match self {
                Self::Place(source) => (*source, path.to_vec()),
                Self::CaseLeaf {
                    source,
                    path: carrier,
                    ..
                } => {
                    let mut canonical = carrier.clone();
                    canonical.extend_from_slice(path);
                    (*source, canonical)
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredElementPathSegment {
        Static(terminal_psi::StructuralPathSegment),
        ViewExtent {
            source: PlaceId,
            bytes: bool,
        },
        RuntimeIndex {
            selector: LoweredDirectExpression,
            extent: Option<LoweredDirectExpression>,
            bound_decided: bool,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredDirectExpression {
        AtomicLoad {
            source: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            ordering: terminal_psi::MemoryOrdering,
            scalar_type: ScalarType,
        },
        PrimitiveRead {
            source: PrimitiveReadSource,
            path: Vec<terminal_psi::StructuralPathSegment>,
            scalar_type: ScalarType,
            qualifications: semantic_vocabulary::ScalarQualificationSetId,
        },
        StructuralField {
            source: PrimitiveReadSource,
            path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            field: StructuralFieldId,
            scalar_type: ScalarType,
            qualifications: semantic_vocabulary::ScalarQualificationSetId,
        },
        ByteSequenceLength {
            source: PlaceId,
            path: Vec<LoweredElementPathSegment>,
            access: terminal_psi::StructuralAccess,
            scalar_type: ScalarType,
        },
        ByteSequenceFieldLength {
            source: PlaceId,
            path: Vec<LoweredElementPathSegment>,
            field: StructuralFieldId,
            scalar_type: ScalarType,
        },
        ByteSequenceFieldRead {
            source: PlaceId,
            path: Vec<LoweredElementPathSegment>,
            field: StructuralFieldId,
            index: Box<LoweredDirectExpression>,
            scalar_type: ScalarType,
            bound_decided: bool,
        },
        IndexedPrimitiveRead {
            qualifications: semantic_vocabulary::ScalarQualificationSetId,
            source: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            index: Box<LoweredDirectExpression>,
            extent: Option<Box<LoweredDirectExpression>>,
            element_path: Vec<LoweredElementPathSegment>,
            scalar_type: ScalarType,
            bound_decided: bool,
        },
        ElementViewLength {
            source: PlaceId,
            path: Vec<LoweredElementPathSegment>,
            access: terminal_psi::StructuralAccess,
            scalar_type: ScalarType,
        },
        ElementViewRead {
            source: PlaceId,
            index: Box<LoweredDirectExpression>,
            path: Vec<LoweredElementPathSegment>,
            scalar_type: ScalarType,
            bound_decided: bool,
        },
        Parameter {
            position: usize,
            scalar_type: ScalarType,
        },
        ErasedParameter {
            position: usize,
            scalar_type: ScalarType,
        },
        Local {
            position: usize,
            scalar_type: ScalarType,
        },
        IntegerLiteral {
            value: IntegerValue,
            scalar_type: ScalarType,
        },
        ScalarApplication {
            application: semantic_vocabulary::ScalarApplicationId,
            scalar_type: ScalarType,
        },
        IeeeFloatLiteral {
            value: IeeeFloatValue,
        },
        ImplementationSelection {
            implementation: symbols::SymbolHandle,
            interface_identity: typed_trees::type_identity::TypeIdentityId,
            scalar_type: ScalarType,
        },
        IntegerBinary {
            kind: LoweredIntegerBinaryKind,
            scalar_type: ScalarType,
            left: Box<LoweredDirectExpression>,
            right: Box<LoweredDirectExpression>,
            formation_decided: bool,
        },
        SaturatingShiftLeft {
            scalar_type: ScalarType,
            left: Box<LoweredDirectExpression>,
            right: Box<LoweredDirectExpression>,
        },
        IntegerBitwiseNot {
            scalar_type: ScalarType,
            operand: Box<LoweredDirectExpression>,
        },
        IntegerWiden {
            scalar_type: ScalarType,
            operand: Box<LoweredDirectExpression>,
        },
        IntegerExactCast {
            formation_decided: bool,
            scalar_type: ScalarType,
            operand: Box<LoweredDirectExpression>,
        },
        RepresentationView {
            scalar_type: ScalarType,
            operand: Box<LoweredDirectExpression>,
        },
        Boolean {
            expression: Box<LoweredBooleanReturnExpression>,
        },
    }

    impl LoweredDirectExpression {
        pub(crate) fn retain_observed_type(
            &mut self,
            value_type: QualifiedScalarType,
            parameters: &[QualifiedScalarType],
        ) -> Result<(), LoweringError> {
            match self {
                Self::PrimitiveRead {
                    scalar_type,
                    qualifications,
                    ..
                }
                | Self::StructuralField {
                    scalar_type,
                    qualifications,
                    ..
                }
                | Self::IndexedPrimitiveRead {
                    scalar_type,
                    qualifications,
                    ..
                } => {
                    *scalar_type = value_type.scalar_type;
                    *qualifications = value_type.qualifications;
                }
                Self::Boolean { expression } => match expression.as_mut() {
                    LoweredBooleanReturnExpression::PrimitiveRead { qualifications, .. }
                    | LoweredBooleanReturnExpression::StructuralField { qualifications, .. } => {
                        *qualifications = value_type.qualifications
                    }
                    LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                        operand.retain_observed_type(value_type, parameters)?
                    }
                    LoweredBooleanReturnExpression::Parameter { position }
                    | LoweredBooleanReturnExpression::Local { position }
                        if parameters.get(*position) == Some(&value_type) => {}
                    _ if value_type.qualifications.is_empty() => {}
                    _ => return unsupported("qualified Boolean observation has no storage read"),
                },
                Self::Parameter { position, .. } | Self::Local { position, .. }
                    if parameters.get(*position) == Some(&value_type) => {}
                _ if value_type.qualifications.is_empty() => {}
                _ => return unsupported("qualified scalar observation has no storage read"),
            }
            Ok(())
        }

        pub(crate) fn value_type(
            &self,
            parameters: &[QualifiedScalarType],
        ) -> Result<QualifiedScalarType, LoweringError> {
            match self {
                Self::PrimitiveRead {
                    scalar_type,
                    qualifications,
                    ..
                }
                | Self::StructuralField {
                    scalar_type,
                    qualifications,
                    ..
                }
                | Self::IndexedPrimitiveRead {
                    scalar_type,
                    qualifications,
                    ..
                } => {
                    return Ok(QualifiedScalarType {
                        scalar_type: *scalar_type,
                        qualifications: *qualifications,
                    });
                }
                Self::Boolean { expression } => match expression.as_ref() {
                    LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                        return operand.value_type(parameters);
                    }
                    LoweredBooleanReturnExpression::PrimitiveRead { qualifications, .. }
                    | LoweredBooleanReturnExpression::StructuralField { qualifications, .. } => {
                        return Ok(QualifiedScalarType {
                            scalar_type: ScalarType::Boolean,
                            qualifications: *qualifications,
                        });
                    }
                    _ => {}
                },
                _ => {}
            }
            let position = match self {
                Self::Parameter { position, .. } | Self::Local { position, .. } => Some(*position),
                Self::Boolean { expression } => match expression.as_ref() {
                    LoweredBooleanReturnExpression::Parameter { position }
                    | LoweredBooleanReturnExpression::Local { position } => Some(*position),
                    _ => None,
                },
                _ => None,
            };
            if let Some(position) = position {
                let value_type =
                    parameters
                        .get(position)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "scalar value has no typed parameter",
                        ))?;
                if value_type.scalar_type != self.scalar_type() {
                    return unsupported("scalar value carrier disagrees with its parameter");
                }
                Ok(value_type)
            } else {
                Ok(self.scalar_type().into())
            }
        }

        pub(crate) const fn scalar_type(&self) -> ScalarType {
            match self {
                Self::Parameter { scalar_type, .. }
                | Self::ErasedParameter { scalar_type, .. }
                | Self::PrimitiveRead { scalar_type, .. }
                | Self::StructuralField { scalar_type, .. }
                | Self::AtomicLoad { scalar_type, .. }
                | Self::ByteSequenceLength { scalar_type, .. }
                | Self::ByteSequenceFieldLength { scalar_type, .. }
                | Self::ByteSequenceFieldRead { scalar_type, .. }
                | Self::IndexedPrimitiveRead { scalar_type, .. }
                | Self::ElementViewLength { scalar_type, .. }
                | Self::ElementViewRead { scalar_type, .. }
                | Self::Local { scalar_type, .. }
                | Self::IntegerLiteral { scalar_type, .. }
                | Self::ScalarApplication { scalar_type, .. }
                | Self::ImplementationSelection { scalar_type, .. }
                | Self::IntegerBinary { scalar_type, .. }
                | Self::SaturatingShiftLeft { scalar_type, .. }
                | Self::IntegerBitwiseNot { scalar_type, .. }
                | Self::IntegerWiden { scalar_type, .. }
                | Self::IntegerExactCast { scalar_type, .. }
                | Self::RepresentationView { scalar_type, .. } => *scalar_type,
                Self::IeeeFloatLiteral { value } => ScalarType::IeeeFloat(value.format()),
                Self::Boolean { .. } => ScalarType::Boolean,
            }
        }
    }

    pub(crate) fn emit_scalar_leaf(
        kind: OperationKind,
        value_type: impl Into<QualifiedScalarType>,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> ValueId {
        let id = mint(next_value_identity, value_id);
        let value_type = value_type.into();
        operations.push_leaf(
            OperationResult::Scalar(ValueDeclaration {
                id,
                scalar_type: value_type.scalar_type,
                qualifications: value_type.qualifications,
            }),
            kind,
        );
        id
    }

    pub(crate) fn emit_byte_length(
        source: terminal_psi::StructuralArgument,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> ValueId {
        if source.path.is_empty()
            && source.access == terminal_psi::StructuralAccess::SharedBorrow
            && let Some(value) = operations
                .storage_observations
                .bytes
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == source.place).then_some(*value))
        {
            return value;
        }
        let scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
        let cache =
            source.path.is_empty() && source.access == terminal_psi::StructuralAccess::SharedBorrow;
        let place = source.place;
        let source = terminal_psi::StructuralArgument {
            path: path_with_current_bounds(&source.path, operations.next_identity),
            ..source
        };
        let value = emit_scalar_leaf(
            OperationKind::ByteSequenceLength { source },
            scalar_type,
            next_value_identity,
            operations,
        );
        if cache {
            operations.storage_observations.bytes.push((place, value));
        }
        value
    }

    pub(crate) fn emit_element_length(
        source: terminal_psi::StructuralArgument,
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> ValueId {
        if source.path.is_empty()
            && let Some(value) = operations
                .storage_observations
                .elements
                .iter()
                .rev()
                .find_map(|(place, value)| (place == &source).then_some(*value))
        {
            return value;
        }
        let scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
        let value = emit_scalar_leaf(
            OperationKind::ElementViewLength {
                source: terminal_psi::StructuralArgument {
                    path: path_with_current_bounds(&source.path, operations.next_identity),
                    ..source.clone()
                },
            },
            scalar_type,
            next_value_identity,
            operations,
        );
        if source.path.is_empty() {
            operations
                .storage_observations
                .elements
                .push((source, value));
        }
        value
    }

    fn selector_obligation(operation: u64, ordinal: usize) -> semantic_vocabulary::ObligationId {
        assert!(
            ordinal < TERMINAL_NESTED_SELECTOR_LIMIT,
            "preparation bounds the nested selector count"
        );
        obligation_id(
            operation
                .checked_mul(TERMINAL_NESTED_SELECTOR_STRIDE)
                .and_then(|offset| offset.checked_add(TERMINAL_NESTED_SELECTOR_OBLIGATION_BASE))
                .and_then(|base| base.checked_add(ordinal as u64 + 1))
                .expect("nested selector obligations fit their reserved namespace"),
        )
    }

    enum EvaluatedProjection {
        Static(terminal_psi::StructuralPathSegment),
        ReadIndex { index: ValueId, bound_decided: bool },
        RuntimeIndex { index: ValueId, bound_decided: bool },
    }

    pub(crate) fn emit_prepared_path(
        path: &[LoweredElementPathSegment],
        first_ordinal: usize,
        parameters: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Vec<terminal_psi::StructuralPathSegment> {
        emit_path_from_capture(
            Vec::with_capacity(path.len()),
            path,
            first_ordinal,
            parameters,
            next_value_identity,
            operations,
        )
    }

    fn emit_path_from_capture(
        mut evaluated: Vec<EvaluatedProjection>,
        path: &[LoweredElementPathSegment],
        first_ordinal: usize,
        parameters: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Vec<terminal_psi::StructuralPathSegment> {
        let complete = |evaluated: &[EvaluatedProjection], operations: &mut OperationBuffer| {
            let mut ordinal = first_ordinal;
            let mut prefix_ordinal = 0;
            evaluated
                .iter()
                .map(|segment| match segment {
                    EvaluatedProjection::ReadIndex {
                        index,
                        bound_decided,
                    } => terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: operations.push_read_obligation(*bound_decided),
                    },
                    EvaluatedProjection::RuntimeIndex {
                        index,
                        bound_decided,
                    } => {
                        let obligation = selector_obligation(operations.next_identity, ordinal);
                        ordinal += 1;
                        if *bound_decided {
                            operations.source_checked_obligations.push(obligation);
                        }
                        terminal_psi::StructuralPathSegment::RuntimeIndex {
                            index: *index,
                            obligation,
                        }
                    }
                    EvaluatedProjection::Static(
                        terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. },
                    ) => {
                        let obligation =
                            selector_obligation(operations.next_identity, prefix_ordinal);
                        prefix_ordinal += 1;
                        terminal_psi::StructuralPathSegment::RuntimeIndex {
                            index: *index,
                            obligation,
                        }
                    }
                    EvaluatedProjection::Static(segment) => segment.clone(),
                })
                .collect::<Vec<_>>()
        };
        for segment in path {
            match segment {
                LoweredElementPathSegment::Static(segment) => {
                    evaluated.push(EvaluatedProjection::Static(segment.clone()));
                }
                LoweredElementPathSegment::ViewExtent { source, bytes } => {
                    let source = terminal_psi::StructuralArgument {
                        place: *source,
                        path: complete(&evaluated, operations),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    };
                    if *bytes {
                        emit_byte_length(source, next_value_identity, operations);
                    } else {
                        emit_element_length(source, next_value_identity, operations);
                    }
                }
                LoweredElementPathSegment::RuntimeIndex {
                    selector,
                    extent,
                    bound_decided,
                } => {
                    let index = emit_direct_expression(
                        selector,
                        parameters,
                        next_value_identity,
                        operations,
                    );
                    if let Some(extent) = extent {
                        emit_direct_expression(extent, parameters, next_value_identity, operations);
                    }
                    evaluated.push(EvaluatedProjection::RuntimeIndex {
                        index,
                        bound_decided: *bound_decided,
                    });
                }
            }
        }
        complete(&evaluated, operations)
    }

    pub(crate) fn path_with_current_bounds(
        path: &[terminal_psi::StructuralPathSegment],
        operation: u64,
    ) -> Vec<terminal_psi::StructuralPathSegment> {
        let mut ordinal = 0;
        path.iter()
            .map(|segment| match segment {
                terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } => {
                    let obligation = selector_obligation(operation, ordinal);
                    ordinal += 1;
                    terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation,
                    }
                }
                segment => segment.clone(),
            })
            .collect()
    }

    pub(crate) fn emit_read_source(
        source: &PrimitiveReadSource,
        operations: &mut OperationBuffer,
    ) -> PlaceId {
        match source {
            PrimitiveReadSource::Place(place) => *place,
            PrimitiveReadSource::CaseLeaf {
                source,
                path,
                structural_type,
            } => {
                let producer = operations.allocate();
                let place = place_id(
                    TERMINAL_HOISTED_ELEMENT_PLACE_BASE
                        .checked_add(producer.get())
                        .expect("hoisted leaf places admit a reserved namespace"),
                );
                operations.push(Operation::unsuspended(
                    producer,
                    structural_result_declaration(
                        place,
                        *structural_type,
                        terminal_psi::StructuralMultiplicity::Unrestricted,
                    ),
                    OperationKind::StructuralCaseLeafCopy {
                        source: *source,
                        path: path.clone(),
                    },
                ));
                place
            }
        }
    }

    pub(crate) fn emit_direct_expression(
        expression: &LoweredDirectExpression,
        parameters: &[ValueDeclaration],
        next_value_identity: &mut u64,
        operations: &mut OperationBuffer,
    ) -> ValueId {
        macro_rules! emit_leaf {
            ($kind:expr, $value_type:expr $(,)?) => {
                emit_scalar_leaf($kind, $value_type, next_value_identity, operations)
            };
        }
        match expression {
            LoweredDirectExpression::AtomicLoad {
                source,
                path,
                ordering,
                scalar_type,
            } => emit_leaf!(
                OperationKind::AtomicAccess {
                    place: *source,
                    path: path.clone(),
                    event: terminal_psi::AtomicAccessEvent::Load {
                        ordering: *ordering,
                    },
                    resident_obligation: None,
                },
                *scalar_type
            ),
            LoweredDirectExpression::PrimitiveRead {
                source,
                path,
                scalar_type,
                qualifications,
            } => {
                let source = emit_read_source(source, operations);
                emit_leaf!(
                    OperationKind::PrimitiveScalarRead {
                        source,
                        path: path.clone(),
                    },
                    QualifiedScalarType {
                        scalar_type: *scalar_type,
                        qualifications: *qualifications,
                    }
                )
            }
            LoweredDirectExpression::StructuralField {
                source,
                path,
                field,
                scalar_type,
                qualifications,
            } => {
                let source = emit_read_source(source, operations);
                emit_leaf!(
                    OperationKind::ScalarStructuralField {
                        source,
                        path: path.clone(),
                        field: *field,
                    },
                    QualifiedScalarType {
                        scalar_type: *scalar_type,
                        qualifications: *qualifications,
                    }
                )
            }
            LoweredDirectExpression::ErasedParameter { .. } => {
                unreachable!("erased formal is proof-only and has no runtime operand")
            }
            LoweredDirectExpression::Parameter { position, .. }
            | LoweredDirectExpression::Local { position, .. } => parameters[*position].id,
            LoweredDirectExpression::IntegerLiteral { value, scalar_type } => emit_leaf!(
                OperationKind::IntegerConstant { value: *value },
                *scalar_type
            ),
            LoweredDirectExpression::ScalarApplication {
                application,
                scalar_type,
            } => emit_scalar_leaf(
                OperationKind::ReadScalarApplication {
                    application: *application,
                },
                *scalar_type,
                next_value_identity,
                operations,
            ),
            LoweredDirectExpression::ImplementationSelection {
                implementation,
                interface_identity,
                scalar_type,
            } => {
                let table = operations
                    .dynamic_dispatch
                    .table(interface_identity, *implementation)
                    .expect("checking records every admitted implementation selection");
                emit_leaf!(OperationKind::SelectImplementation { table }, *scalar_type)
            }
            LoweredDirectExpression::ByteSequenceLength {
                source,
                path,
                access,
                ..
            } => {
                let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
                let path = path_with_current_bounds(&path, operations.next_identity);
                emit_byte_length(
                    terminal_psi::StructuralArgument {
                        place: *source,
                        path,
                        access: *access,
                    },
                    next_value_identity,
                    operations,
                )
            }
            LoweredDirectExpression::ByteSequenceFieldLength {
                source,
                path,
                field,
                scalar_type,
            } => {
                let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
                let path = path_with_current_bounds(&path, operations.next_identity);
                let value = emit_leaf!(
                    OperationKind::StructuralByteSequenceFieldLength {
                        source: *source,
                        path: path.clone(),
                        field: *field,
                    },
                    *scalar_type
                );
                operations.storage_observations.field_bytes.push((
                    *source,
                    path.clone(),
                    *field,
                    value,
                ));
                value
            }
            LoweredDirectExpression::ByteSequenceFieldRead {
                source,
                path,
                field,
                index,
                scalar_type,
                bound_decided,
            } => {
                let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
                let index =
                    emit_direct_expression(index, parameters, next_value_identity, operations);
                let length = operations
                    .storage_observations
                    .field_bytes
                    .iter()
                    .rev()
                    .find_map(|(place, carrier, named, value)| {
                        (*place == *source
                            && terminal_semantics::structural_paths_select_same_location(
                                carrier, &path,
                            )
                            && *named == *field)
                            .then_some(*value)
                    })
                    .unwrap_or_else(|| {
                        emit_leaf!(
                            OperationKind::StructuralByteSequenceFieldLength {
                                source: *source,
                                path: path_with_current_bounds(&path, operations.next_identity),
                                field: *field,
                            },
                            ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
                            )
                        )
                    });
                let obligation = operations.push_read_obligation(*bound_decided);
                emit_leaf!(
                    OperationKind::StructuralByteSequenceFieldRead {
                        source: *source,
                        path: path_with_current_bounds(&path, operations.next_identity),
                        field: *field,
                        index,
                        length,
                        obligation,
                    },
                    *scalar_type
                )
            }
            LoweredDirectExpression::IndexedPrimitiveRead {
                qualifications,
                source,
                path,
                index,
                extent,
                element_path,
                scalar_type,
                bound_decided,
            } => {
                if let Some(extent) = extent {
                    emit_direct_expression(extent, parameters, next_value_identity, operations);
                }
                let index =
                    emit_direct_expression(index, parameters, next_value_identity, operations);
                let prefix_selectors = path
                    .iter()
                    .filter(|step| {
                        matches!(
                            step,
                            terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                        )
                    })
                    .count();
                let mut captured = path_with_current_bounds(path, operations.next_identity)
                    .into_iter()
                    .map(EvaluatedProjection::Static)
                    .collect::<Vec<_>>();
                captured.push(EvaluatedProjection::ReadIndex {
                    index,
                    bound_decided: *bound_decided,
                });
                let path = emit_path_from_capture(
                    captured,
                    element_path,
                    prefix_selectors,
                    parameters,
                    next_value_identity,
                    operations,
                );
                emit_leaf!(
                    OperationKind::PrimitiveScalarRead {
                        source: *source,
                        path,
                    },
                    QualifiedScalarType {
                        scalar_type: *scalar_type,
                        qualifications: *qualifications,
                    }
                )
            }
            LoweredDirectExpression::ElementViewLength {
                source,
                path,
                access,
                ..
            } => {
                let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
                let path = path_with_current_bounds(&path, operations.next_identity);
                emit_element_length(
                    terminal_psi::StructuralArgument {
                        place: *source,
                        path,
                        access: *access,
                    },
                    next_value_identity,
                    operations,
                )
            }
            LoweredDirectExpression::ElementViewRead {
                source,
                index,
                path,
                scalar_type,
                bound_decided,
            } => {
                let index =
                    emit_direct_expression(index, parameters, next_value_identity, operations);
                let length = operations
                    .storage_observations
                    .elements
                    .iter()
                    .rev()
                    .find_map(|(observed, value)| {
                        (observed.place == *source && observed.path.is_empty()).then_some(*value)
                    })
                    .unwrap_or_else(|| {
                        emit_element_length(
                            terminal_psi::StructuralArgument::whole(
                                *source,
                                terminal_psi::StructuralAccess::SharedBorrow,
                            ),
                            next_value_identity,
                            operations,
                        )
                    });
                let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
                let obligation = operations.push_read_obligation(*bound_decided);
                emit_leaf!(
                    OperationKind::ElementViewRead {
                        source: *source,
                        index,
                        length,
                        obligation,
                        path,
                    },
                    *scalar_type
                )
            }
            LoweredDirectExpression::IeeeFloatLiteral { value } => emit_leaf!(
                OperationKind::IeeeFloatConstant { value: *value },
                ScalarType::IeeeFloat(value.format())
            ),
            LoweredDirectExpression::SaturatingShiftLeft {
                scalar_type,
                left,
                right,
            } => {
                let left_value =
                    emit_direct_expression(left, parameters, next_value_identity, operations);
                let right_value =
                    emit_direct_expression(right, parameters, next_value_identity, operations);
                let bindings = [
                    ValueDeclaration {
                        id: left_value,
                        scalar_type: *scalar_type,
                        qualifications: Default::default(),
                    },
                    ValueDeclaration {
                        id: right_value,
                        scalar_type: right.scalar_type(),
                        qualifications: Default::default(),
                    },
                ];
                let left = LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type: *scalar_type,
                };
                let right = LoweredDirectExpression::Parameter {
                    position: 1,
                    scalar_type: right.scalar_type(),
                };
                let expanded = expansion(*scalar_type, left, right);
                emit_direct_expression(&expanded, &bindings, next_value_identity, operations)
            }
            LoweredDirectExpression::IntegerBinary {
                kind,
                scalar_type,
                left,
                right,
                formation_decided,
            } => {
                let left =
                    emit_direct_expression(left, parameters, next_value_identity, operations);
                let right =
                    emit_direct_expression(right, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                let operation = operations.allocate();
                if *formation_decided && let Some(obligation) = kind.formation_obligation(operation)
                {
                    operations.source_checked_obligations.push(obligation);
                }
                operations.push(Operation::unsuspended(
                    operation,
                    scalar_result_declaration(id, *scalar_type),
                    kind.operation(operation, left, right),
                ));
                id
            }
            LoweredDirectExpression::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                let operand =
                    emit_direct_expression(operand, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, *scalar_type),
                    OperationKind::IntegerBitwiseNot { operand },
                );
                id
            }
            LoweredDirectExpression::IntegerWiden {
                scalar_type,
                operand,
            } => {
                let operand =
                    emit_direct_expression(operand, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                operations.push_leaf(
                    scalar_result_declaration(id, *scalar_type),
                    OperationKind::IntegerWiden { operand },
                );
                id
            }
            LoweredDirectExpression::IntegerExactCast {
                scalar_type,
                operand,
                formation_decided,
            } => {
                let operand =
                    emit_direct_expression(operand, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                let operation = operations.allocate();
                let obligation = obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-cast obligation follows its operation identity"),
                );
                if *formation_decided {
                    operations.source_checked_obligations.push(obligation);
                }
                operations.push(Operation::unsuspended(
                    operation,
                    scalar_result_declaration(id, *scalar_type),
                    OperationKind::IntegerExactCast {
                        operand,
                        obligation,
                    },
                ));
                id
            }
            LoweredDirectExpression::RepresentationView {
                scalar_type,
                operand,
            } => {
                let source_type = operand.scalar_type();
                let operand =
                    emit_direct_expression(operand, parameters, next_value_identity, operations);
                let id = mint(next_value_identity, value_id);
                let operation = operations.allocate();
                let landing_obligation =
                    terminal_semantics::ScalarRepresentationViewPair::classify(
                        source_type,
                        *scalar_type,
                    )
                    .filter(|pair| pair.requires_landing())
                    .map(|_| {
                        obligation_id(
                            operation
                                .get()
                                .checked_add(1)
                                .expect("landing obligation follows its operation identity"),
                        )
                    });
                if let Some(obligation) = landing_obligation {
                    operations.source_checked_obligations.push(obligation);
                }
                operations.push(Operation::unsuspended(
                    operation,
                    scalar_result_declaration(id, *scalar_type),
                    OperationKind::ScalarRepresentationView {
                        operand,
                        landing_obligation,
                    },
                ));
                id
            }
            LoweredDirectExpression::Boolean { expression } => {
                emit_boolean_expression(expression, parameters, next_value_identity, operations)
            }
        }
    }
}
pub(crate) mod integer {

    use crate::shared::terminal_identities::obligation_id;
    use numerics::{
        arithmetic::ArithmeticDomain,
        integer_policy::{IntegerPolicyPrimitive, integer_policy_bridge},
    };
    use semantic_vocabulary::{ObligationId, OperationId, ValueId};
    use terminal_psi::{OperationKind, TrappingIntegerOperation};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum LoweredIntegerComparisonKind {
        Equal,
        LessThan,
        LessOrEqual,
    }

    impl LoweredIntegerComparisonKind {
        pub(crate) const fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
            match self {
                Self::Equal => OperationKind::IntegerEqual { left, right },
                Self::LessThan => OperationKind::IntegerLessThan { left, right },
                Self::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
            }
        }
    }

    macro_rules! lowered_integer_binary_kinds {
        ($($variant:ident => $binding:expr),* $(,)?) => {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum LoweredIntegerBinaryKind {
                $($variant,)*
            }

            impl LoweredIntegerBinaryKind {
                pub(crate) const fn integer_policy_binding(
                    self,
                ) -> Option<(IntegerPolicyPrimitive, ArithmeticDomain)> {
                    match self {
                        $(Self::$variant => $binding,)*
                    }
                }
            }
        };
    }

    lowered_integer_binary_kinds! {
        BitwiseAnd => None,
        BitwiseOr => None,
        BitwiseXor => None,
        WrappingShiftLeft => Some((IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Wrapping)),
        WrappingShiftRight => Some((IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Wrapping)),
        ExactShiftLeft => Some((IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Exact)),
        ExactShiftRight => Some((IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Exact)),
        ExactAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Exact)),
        ExactSubtract => Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact)),
        ExactMultiply => Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Exact)),
        ExactDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact)),
        ExactRemainder => Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact)),
        WrappingDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Wrapping)),
        WrappingRemainder => Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Wrapping)),
        SaturatingDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating)),
        SaturatingRemainder => Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Saturating,
        )),
        WrappingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Wrapping)),
        SaturatingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Saturating)),
        WrappingSubtract => Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Wrapping)),
        SaturatingSubtract => Some((
            IntegerPolicyPrimitive::Subtract,
            ArithmeticDomain::Saturating,
        )),
        WrappingMultiply => Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Wrapping)),
        SaturatingMultiply => Some((
            IntegerPolicyPrimitive::Multiply,
            ArithmeticDomain::Saturating,
        )),
        TrappingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Trapping)),
        TrappingSubtract => Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Trapping)),
        TrappingMultiply => Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Trapping)),
        TrappingDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Trapping)),
        TrappingRemainder => Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Trapping)),
        TrappingShiftLeft => Some((IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Trapping)),
        TrappingShiftRight => Some((IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Trapping)),
    }

    impl LoweredIntegerBinaryKind {
        pub(crate) fn formation_obligation(self, operation: OperationId) -> Option<ObligationId> {
            let catalog_requires_formation =
                self.integer_policy_binding()
                    .is_some_and(|(primitive, policy)| {
                        !integer_policy_bridge(primitive, policy)
                            .formation_conditions
                            .is_empty()
                    });
            catalog_requires_formation.then(|| {
                obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("integer formation obligation follows its operation identity"),
                )
            })
        }

        pub(crate) fn operation(
            self,
            operation: OperationId,
            left: ValueId,
            right: ValueId,
        ) -> OperationKind {
            let formation_obligation = self.formation_obligation(operation);
            match self {
                Self::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
                Self::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
                Self::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
                Self::WrappingShiftLeft => OperationKind::WrappingIntegerShiftLeft {
                    value: left,
                    count: right,
                },
                Self::WrappingShiftRight => OperationKind::WrappingIntegerShiftRight {
                    value: left,
                    count: right,
                },
                Self::ExactShiftLeft => OperationKind::ExactIntegerShiftLeft {
                    value: left,
                    count: right,
                    obligation: formation_obligation.expect("exact shift has formation conditions"),
                },
                Self::ExactShiftRight => OperationKind::ExactIntegerShiftRight {
                    value: left,
                    count: right,
                    obligation: formation_obligation.expect("exact shift has formation conditions"),
                },
                Self::ExactAdd => OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation: formation_obligation.expect("exact add has formation conditions"),
                },
                Self::ExactSubtract => OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("exact subtract has formation conditions"),
                },
                Self::ExactMultiply => OperationKind::ExactIntegerMultiply {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("exact multiply has formation conditions"),
                },
                Self::ExactDivide => OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("exact divide has formation conditions"),
                },
                Self::ExactRemainder => OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("exact remainder retains its obligation"),
                },
                Self::WrappingDivide => OperationKind::WrappingIntegerDivide {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("wrapping divide has formation conditions"),
                },
                Self::WrappingRemainder => OperationKind::WrappingIntegerRemainder {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("wrapping remainder retains its obligation"),
                },
                Self::SaturatingDivide => OperationKind::SaturatingIntegerDivide {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("saturating divide has formation conditions"),
                },
                Self::SaturatingRemainder => OperationKind::SaturatingIntegerRemainder {
                    left,
                    right,
                    obligation: formation_obligation
                        .expect("saturating remainder retains its obligation"),
                },
                Self::WrappingAdd => OperationKind::WrappingIntegerAdd { left, right },
                Self::SaturatingAdd => OperationKind::SaturatingIntegerAdd { left, right },
                Self::WrappingSubtract => OperationKind::WrappingIntegerSubtract { left, right },
                Self::SaturatingSubtract => {
                    OperationKind::SaturatingIntegerSubtract { left, right }
                }
                Self::WrappingMultiply => OperationKind::WrappingIntegerMultiply { left, right },
                Self::SaturatingMultiply => {
                    OperationKind::SaturatingIntegerMultiply { left, right }
                }
                Self::TrappingAdd => trapping(TrappingIntegerOperation::Add { left, right }),
                Self::TrappingSubtract => {
                    trapping(TrappingIntegerOperation::Subtract { left, right })
                }
                Self::TrappingMultiply => {
                    trapping(TrappingIntegerOperation::Multiply { left, right })
                }
                Self::TrappingDivide => trapping(TrappingIntegerOperation::Divide { left, right }),
                Self::TrappingRemainder => {
                    trapping(TrappingIntegerOperation::Remainder { left, right })
                }
                Self::TrappingShiftLeft => trapping(TrappingIntegerOperation::ShiftLeft {
                    value: left,
                    count: right,
                }),
                Self::TrappingShiftRight => trapping(TrappingIntegerOperation::ShiftRight {
                    value: left,
                    count: right,
                }),
            }
        }
    }

    const fn trapping(operation: TrappingIntegerOperation) -> OperationKind {
        OperationKind::TrappingInteger { operation }
    }
}
pub(crate) mod place_declarations {

    use std::collections::BTreeSet;

    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::{StructuralPlaceDeclaration, TerminalModule};

    pub(crate) fn reconcile_leaf_copy_place_declarations(module: &mut TerminalModule) {
        for machine in &mut module.machines {
            let mut declared: Option<BTreeSet<_>> = None;
            let mut additions = Vec::new();
            for block in &machine.blocks {
                for operation in &block.operations {
                    if !matches!(
                        operation.kind,
                        terminal_psi::structural_leaf_copy_pair_pattern!()
                    ) {
                        continue;
                    }
                    let Some(result) = operation.result.structural() else {
                        continue;
                    };
                    let declared = declared.get_or_insert_with(|| {
                        machine
                            .structural_places
                            .iter()
                            .map(|place| place.id)
                            .collect()
                    });
                    if !declared.insert(result.place) {
                        continue;
                    }
                    additions.push(StructuralPlaceDeclaration {
                        id: result.place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation.id,
                            structural_type: result.structural_type,
                        },
                    });
                }
            }
            machine.structural_places.extend(additions);
            if !machine.structural_places.is_sorted_by_key(|place| place.id) {
                machine.structural_places.sort_by_key(|place| place.id);
            }
        }
    }

    pub(crate) fn operation_result_place(
        place: semantic_vocabulary::PlaceId,
        producer: semantic_vocabulary::OperationId,
        structural_type: semantic_vocabulary::StructuralTypeId,
    ) -> StructuralPlaceDeclaration {
        StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        }
    }

    pub(crate) fn operation_result_declaration(
        operation: &terminal_psi::Operation,
    ) -> Option<StructuralPlaceDeclaration> {
        let result = operation.result.as_structural()?;
        Some(operation_result_place(
            result.place,
            operation.id,
            result.structural_type,
        ))
    }

    pub(crate) fn operation_results<'a>(
        operations: impl Iterator<Item = &'a terminal_psi::Operation>,
    ) -> impl Iterator<Item = StructuralPlaceDeclaration> {
        operations.filter_map(operation_result_declaration)
    }

    pub(crate) fn retain_operation_results<'a>(
        operations: impl Iterator<Item = &'a terminal_psi::Operation>,
        declarations: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        let mut index_by_id: std::collections::HashMap<semantic_vocabulary::PlaceId, usize> =
            declarations
                .iter()
                .enumerate()
                .map(|(index, existing)| (existing.id, index))
                .collect();
        for declaration in operation_results(operations) {
            if let Some(&index) = index_by_id.get(&declaration.id) {
                if declarations[index] != declaration {
                    return unsupported(
                        "computed result conflicts with an existing place declaration",
                    );
                }
            } else {
                let index = declarations.len();
                index_by_id.insert(declaration.id, index);
                declarations.push(declaration);
            }
        }
        Ok(())
    }

    pub(crate) fn retain_block_parameters<'a>(
        blocks: impl Iterator<Item = &'a terminal_psi::Block>,
        declarations: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        let mut index_by_id: std::collections::HashMap<semantic_vocabulary::PlaceId, usize> =
            declarations
                .iter()
                .enumerate()
                .map(|(index, existing)| (existing.id, index))
                .collect();
        for block in blocks {
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                let position = u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("computed structural parameter position overflows")
                })?;
                if parameter.position != position {
                    return unsupported("computed structural parameter positions are not dense");
                }
                let declaration = StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position,
                    },
                };
                if let Some(&index) = index_by_id.get(&declaration.id) {
                    if declarations[index] != declaration {
                        return unsupported(
                            "computed parameter conflicts with an existing place declaration",
                        );
                    }
                } else {
                    let index = declarations.len();
                    index_by_id.insert(declaration.id, index);
                    declarations.push(declaration);
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod view_subslice {
    use crate::emission::expression_validation::{
        direct_expression_contains_short_circuit, validate_direct_parameter_types,
    };
    use crate::emission::operation_emission::buffer::{
        OperationBuffer, structural_result_declaration,
    };
    use crate::emission::operation_emission::expressions::path_with_current_bounds;
    use crate::emission::operation_emission::expressions::{
        LoweredDirectExpression, emit_byte_length, emit_direct_expression, emit_element_length,
    };
    use crate::emission::operation_emission::place_declarations::operation_result_place;
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::obligation_id;
    use checked_trees::types::PrimitiveType;
    use semantic_vocabulary::{IntegerValue, PlaceId, StructuralTypeId, ValueId};
    use terminal_psi::{
        Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
        ValueDeclaration,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ViewFamily {
        Bytes,
        Elements,
    }

    pub(crate) fn coordinate(
        expression: LoweredDirectExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let source = expression
            .scalar_type()
            .as_integer()
            .filter(|integer| !integer.is_address())
            .ok_or(LoweringError::Unsupported(
                "range endpoint requires an integer carrier",
            ))?;
        let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
        if expression.scalar_type() == scalar_type {
            return Ok(expression);
        }
        if source.can_widen_to(
            scalar_type
                .as_integer()
                .ok_or(LoweringError::InvalidPsiIntegerType)?,
        ) {
            Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type,
                operand: Box::new(expression),
            })
        } else {
            Ok(LoweredDirectExpression::IntegerExactCast {
                scalar_type,
                operand: Box::new(expression),
                formation_decided: true,
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        family: ViewFamily,
        source: terminal_psi::StructuralArgument,
        start: Option<&LoweredDirectExpression>,
        end: Option<&LoweredDirectExpression>,
        destination: PlaceId,
        structural_type: StructuralTypeId,
        values: &[ValueDeclaration],
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<StructuralPlaceDeclaration, LoweringError> {
        let multiplicity = match source.access {
            terminal_psi::StructuralAccess::SharedBorrow => StructuralMultiplicity::Unrestricted,
            terminal_psi::StructuralAccess::MutableBorrow => StructuralMultiplicity::Affine,
            _ => return unsupported("view subslice requires readable borrowed access"),
        };
        let count_type = terminal_scalar_type(PrimitiveType::U64)?;
        let types = values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let mut endpoint =
            |expression: &LoweredDirectExpression| -> Result<ValueId, LoweringError> {
                let expression = coordinate(expression.clone())?;
                if direct_expression_contains_short_circuit(&expression) {
                    return unsupported("view subslice endpoint needs a branch-free u64 value");
                }
                validate_direct_parameter_types(&expression, &types)?;
                Ok(emit_direct_expression(
                    &expression,
                    values,
                    next_value,
                    operations,
                ))
            };
        let start = match start {
            Some(start) => endpoint(start)?,
            None => endpoint(&LoweredDirectExpression::IntegerLiteral {
                value: IntegerValue::Unsigned(0),
                scalar_type: count_type,
            })?,
        };
        let end = end.map(&mut endpoint).transpose()?;
        let length = match family {
            ViewFamily::Bytes => emit_byte_length(source.clone(), next_value, operations),
            ViewFamily::Elements => emit_element_length(source.clone(), next_value, operations),
        };
        let end = end.unwrap_or(length);
        let source = terminal_psi::StructuralArgument {
            path: path_with_current_bounds(&source.path, operations.next_identity),
            ..source
        };
        let producer = operations.allocate();
        let obligation = obligation_id(producer.get().checked_add(1).ok_or(
            LoweringError::Unsupported("view subslice obligation identity overflows"),
        )?);
        let kind = match family {
            ViewFamily::Bytes => OperationKind::ByteSequenceSubslice {
                source,
                start,
                end,
                length,
                obligation,
            },
            ViewFamily::Elements => OperationKind::ElementViewSubslice {
                source,
                start,
                end,
                length,
                obligation,
            },
        };
        operations.push(Operation::unsuspended(
            producer,
            structural_result_declaration(destination, structural_type, multiplicity),
            kind,
        ));
        Ok(operation_result_place(
            destination,
            producer,
            structural_type,
        ))
    }
}

use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::emission::selected_comparison::SelectedComparisonMeaning;
use crate::shared::lowering_error::LoweringError;
use crate::shared::terminal_identities::{mint_with, value_id};
pub(crate) use boolean::emit_boolean_expression;
use buffer::OperationBuffer;
use calls::{CallEmissionContext, LoweredDirectCallBinding};
use expressions::LoweredDirectExpression;
pub(crate) use expressions::emit_direct_expression;
use lowered_psi::LoweredSelectedComparisonOperandOrder;
use semantic_vocabulary::{QualifiedScalarType, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, ValueDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarBinding {
    StorageAddress(calls::PreparedStructuralArgument, ScalarType),
    SelectedComparison {
        occurrence: crate::emission::selected_comparison::SelectedComparison,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
    },
    Expression(LoweredDirectExpression),
    DirectCall(LoweredDirectCallBinding),
    DescriptorTable(descriptor_tables::LoweredDescriptorTableBinding),
    BoundaryCall(boundary_calls::LoweredBoundaryCallBinding),
    DynamicCall(dynamic_calls::LoweredDynamicCallBinding),
}

impl LoweredScalarBinding {
    pub(crate) fn value_type(
        &self,
        parameters: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        match self {
            Self::StorageAddress(_, scalar_type) => Ok((*scalar_type).into()),
            Self::SelectedComparison { .. } => Ok(ScalarType::Boolean.into()),
            Self::Expression(expression) => expression.value_type(parameters),
            Self::DirectCall(call) => Ok(call.result_type),
            Self::DescriptorTable(call) => Ok(call.result_type),
            Self::BoundaryCall(call) => Ok(call.result_type),
            Self::DynamicCall(call) => Ok(call.result_type),
        }
    }
    pub(crate) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::StorageAddress(_, scalar_type) => *scalar_type,
            Self::SelectedComparison { .. } => ScalarType::Boolean,
            Self::Expression(expression) => expression.scalar_type(),
            Self::DirectCall(call) => call.result_type.scalar_type,
            Self::DescriptorTable(call) => call.result_type.scalar_type,
            Self::BoundaryCall(call) => call.result_type.scalar_type,
            Self::DynamicCall(call) => call.result_type.scalar_type,
        }
    }
}

pub(crate) fn emit_scalar_binding(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    binding: &LoweredScalarBinding,
    parameters: &[ValueDeclaration],
    frontier: &lowered_psi::LoweredSourceFrontierBindings,
    caller_erased_formals: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<ValueId, LoweringError> {
    let result = match binding {
        LoweredScalarBinding::StorageAddress(argument, scalar_type) => {
            let source =
                argument.emit(parameters, call_emission, next_value_identity, operations)?;
            let id = mint_with(
                next_value_identity,
                value_id,
                "address result identity overflow",
            )?;
            operations.push_leaf(
                buffer::scalar_result_declaration(id, *scalar_type),
                OperationKind::StorageAddress { source },
            );
            Ok(id)
        }

        LoweredScalarBinding::SelectedComparison {
            occurrence,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = mint_with(
                next_value_identity,
                value_id,
                "comparison value identity overflow",
            )?;
            let operation = operations.allocate();
            let terminal_machine = call_emission.terminal_machine;
            let (kind, negated) = match occurrence.meaning {
                SelectedComparisonMeaning::IeeeFloat { comparison, .. } => {
                    operations.selected_comparisons.push(
                        lowered_psi::LoweredSelectedComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            terminal_machine,
                            terminal_operation: operation,
                            operand_order: LoweredSelectedComparisonOperandOrder::Authored,
                        },
                    );
                    (
                        OperationKind::IeeeFloatCompare {
                            comparison,
                            left,
                            right,
                        },
                        false,
                    )
                }
                SelectedComparisonMeaning::Integer {
                    comparison,
                    operand_order,
                    negated,
                    ..
                } => {
                    operations.selected_comparisons.push(
                        lowered_psi::LoweredSelectedComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            terminal_machine,
                            terminal_operation: operation,
                            operand_order,
                        },
                    );
                    let (first, second) = match operand_order {
                        LoweredSelectedComparisonOperandOrder::Authored => (left, right),
                        LoweredSelectedComparisonOperandOrder::Swapped => (right, left),
                    };
                    (comparison.operation(first, second), negated)
                }
            };
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: scalar_result_declaration(id, ScalarType::Boolean),
                kind,
            });
            if !negated {
                return Ok(id);
            }
            let negated_id = mint_with(
                next_value_identity,
                value_id,
                "comparison negation value identity overflow",
            )?;
            operations.push_leaf(
                scalar_result_declaration(negated_id, ScalarType::Boolean),
                OperationKind::BooleanNot { operand: id },
            );
            Ok(negated_id)
        }
        LoweredScalarBinding::Expression(expression) => Ok(emit_direct_expression(
            expression,
            parameters,
            next_value_identity,
            operations,
        )),
        LoweredScalarBinding::DirectCall(call) => calls::emit_scalar_call_binding(
            identities,
            checked,
            call,
            parameters,
            frontier,
            caller_erased_formals,
            next_value_identity,
            operations,
            call_emission,
            current_claims,
        ),
        LoweredScalarBinding::DescriptorTable(call) => descriptor_tables::emit(
            call,
            parameters,
            frontier,
            next_value_identity,
            operations,
            call_emission,
        ),
        LoweredScalarBinding::BoundaryCall(call) => boundary_calls::emit_boundary_call_binding(
            call,
            parameters,
            frontier,
            next_value_identity,
            operations,
            call_emission,
            current_claims,
        ),
        LoweredScalarBinding::DynamicCall(call) => dynamic_calls::emit_dynamic_call_binding(
            call,
            parameters,
            frontier,
            next_value_identity,
            operations,
            call_emission,
            current_claims,
        ),
    }?;
    let completion = match binding {
        LoweredScalarBinding::DirectCall(call) => {
            Some((call.source_coordinate, &call.loan_completions))
        }
        LoweredScalarBinding::DescriptorTable(call) => {
            Some((call.source_coordinate.into(), &call.loan_completions))
        }
        LoweredScalarBinding::BoundaryCall(call) => {
            Some((call.source_coordinate.into(), &call.loan_completions))
        }
        LoweredScalarBinding::DynamicCall(call) => {
            Some((call.source_coordinate.into(), &call.loan_completions))
        }
        _ => None,
    };
    if let Some((coordinate, completions)) = completion {
        crate::emission::loan_completions::emit(
            completions,
            coordinate,
            operations,
            call_emission,
            current_claims,
        )?;
    }
    Ok(result)
}
