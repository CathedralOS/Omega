//! Module-wide validation for proof-only float-meaning projection rows.

use psi_terminal::TerminalModule;

use super::ModuleError;

pub(super) fn validate_float_meaning_projections(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    validate_rows(&module.float_meaning_projections)?;
    validate_direct_sources(module)?;
    validate_equalities(
        &module.float_meaning_projections,
        &module.float_meaning_equalities,
    )
}

fn validate_equalities(
    projections: &[psi_terminal::FloatMeaningProjection],
    equalities: &[psi_terminal::FloatMeaningEqualityProposition],
) -> Result<(), ModuleError> {
    for (index, proposition) in equalities.iter().enumerate() {
        let expected =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningEquality {
                expected: u32::MAX,
                actual: proposition.id.0,
            })?;
        if proposition.id.0 != expected {
            return Err(ModuleError::NonDenseFloatMeaningEquality {
                expected,
                actual: proposition.id.0,
            });
        }
        if proposition.left > proposition.right {
            return Err(ModuleError::NonCanonicalFloatMeaningEqualityOperands {
                proposition: expected,
                left: proposition.left.0,
                right: proposition.right.0,
            });
        }
        let mut reconstructed = Vec::with_capacity(2);
        for operand in [proposition.left, proposition.right] {
            let projection = usize::try_from(operand.0)
                .ok()
                .and_then(|index| projections.get(index))
                .filter(|projection| projection.result.id == operand)
                .ok_or(ModuleError::UnknownFloatMeaningEqualityOperand {
                    proposition: expected,
                    operand: operand.0,
                })?;
            reconstructed.push(
                crate::verification::reconstruct_float_meaning_projection(projection).map_err(
                    |error| ModuleError::InvalidFloatMeaningProjection {
                        index: operand.0,
                        error,
                    },
                )?,
            );
        }
        let [left, right] = reconstructed.as_slice() else {
            unreachable!("float meaning equality has exactly two operands")
        };
        if left.source_format != right.source_format
            || left.operation != right.operation
            || left.contract != right.contract
        {
            return Err(ModuleError::InvalidFloatMeaningProjection {
                index: proposition.right.0,
                error:
                    crate::verification::FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
            });
        }
    }
    Ok(())
}

fn validate_rows(projections: &[psi_terminal::FloatMeaningProjection]) -> Result<(), ModuleError> {
    let mut sources = Vec::new();
    let mut projection_keys = Vec::new();
    for (index, projection) in projections.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ModuleError::NonDenseFloatMeaningProjection {
                expected: u32::MAX,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(&projection.source),
            })?;
        if projection.result.id.0 != index {
            return Err(ModuleError::NonDenseFloatMeaningProjection {
                expected: index,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(&projection.source),
            });
        }
        if let psi_terminal::FloatMeaningSource::TransitionalInput(source) = &projection.source {
            if let Some((_, format)) = sources.iter().find(|(id, _)| *id == source.id) {
                if *format != source.format {
                    return Err(
                        ModuleError::InconsistentFloatMeaningProjectionSourceFormat {
                            source: source.id.0,
                        },
                    );
                }
            } else {
                let expected = u32::try_from(sources.len()).map_err(|_| {
                    ModuleError::NonDenseFloatMeaningProjection {
                        expected: u32::MAX,
                        result: projection.result.id.0,
                        transitional_source: Some(source.id.0),
                    }
                })?;
                if source.id.0 != expected {
                    return Err(ModuleError::NonDenseFloatMeaningProjection {
                        expected,
                        result: projection.result.id.0,
                        transitional_source: Some(source.id.0),
                    });
                }
                sources.push((source.id, source.format));
            }
        }
        let key = (projection.source.clone(), projection.operation);
        if let Some(first) = projection_keys.iter().position(|existing| *existing == key) {
            return Err(ModuleError::DuplicateFloatMeaningProjection {
                first: u32::try_from(first).unwrap_or(u32::MAX),
                duplicate: index,
            });
        }
        projection_keys.push(key);
        crate::verification::reconstruct_float_meaning_projection(projection)
            .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
    }
    Ok(())
}

fn validate_direct_sources(module: &TerminalModule) -> Result<(), ModuleError> {
    for (index, projection) in module.float_meaning_projections.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        match &projection.source {
            psi_terminal::FloatMeaningSource::DirectMachineParameter(parameter) => {
                crate::verification::verify_direct_float_parameter(module, *parameter)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectMachineResult(result) => {
                crate::verification::verify_direct_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectBlockParameter(parameter) => {
                crate::verification::verify_direct_block_float_parameter(module, *parameter)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectOperationResult(result) => {
                crate::verification::verify_direct_operation_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectCallResult(result) => {
                crate::verification::verify_direct_call_float_result(module, *result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectStructuralLeaf(leaf) => {
                crate::verification::verify_direct_structural_float_leaf(module, leaf)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            _ => {}
        }
    }
    Ok(())
}

const fn transitional_source_id(source: &psi_terminal::FloatMeaningSource) -> Option<u32> {
    match source {
        psi_terminal::FloatMeaningSource::TransitionalInput(input) => Some(input.id.0),
        psi_terminal::FloatMeaningSource::DirectMachineParameter(_)
        | psi_terminal::FloatMeaningSource::DirectMachineResult(_)
        | psi_terminal::FloatMeaningSource::DirectBlockParameter(_)
        | psi_terminal::FloatMeaningSource::DirectOperationResult(_)
        | psi_terminal::FloatMeaningSource::DirectCallResult(_)
        | psi_terminal::FloatMeaningSource::DirectStructuralLeaf(_)
        | psi_terminal::FloatMeaningSource::ExactBinary32Literal(_)
        | psi_terminal::FloatMeaningSource::ExactBinary64Literal(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use psi_core::{
        BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, IeeeFloatFormat,
        IeeeFloatStructuralField, IeeeFloatValue, MachineId, OperationId, PlaceId, ScalarType,
        StructuralFieldId, StructuralTypeId, ValueId,
    };
    use psi_terminal::{
        Block, DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
        DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
        FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
        FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId, Operation, OperationKind,
        OperationResult, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration,
        ProofValueId, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
        Terminator, ValueDeclaration, VocabularyMarker,
    };

    fn contract(
        operation: psi_numerics::float_projection::FloatProjectionOperation,
    ) -> psi_terminal::FloatProjectionContractIdentity {
        let contract = operation.contract_identity();
        psi_terminal::FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        }
    }

    use super::*;

    fn projection(index: u32) -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(index),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                id: FloatProjectionInputId(index),
                format: IeeeFloatFormat::Binary32,
            }),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(psi_numerics::float_projection::FloatProjectionOperation::Meaning32),
        }
    }

    fn exact_binary32_projection(index: u32, bits: u32) -> FloatMeaningProjection {
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(index),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::ExactBinary32Literal(bits),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(psi_numerics::float_projection::FloatProjectionOperation::Meaning32),
        }
    }

    fn semantic_id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero semantic identity")
    }

    fn terminal_machine(
        owner: MachineId,
        parameter: ValueId,
        scalar_type: ScalarType,
    ) -> TerminalMachine {
        TerminalMachine {
            id: owner,
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: parameter,
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: semantic_id(owner.get(), BlockId::new),
            blocks: Vec::new(),
            contract: psi_terminal::MachineContract {
                id: semantic_id(owner.get(), ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn direct_module() -> TerminalModule {
        let owner = semantic_id(1, MachineId::new);
        let parameter = semantic_id(1, ValueId::new);
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: owner,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: vec![FloatMeaningProjection {
                result: ProofValueDeclaration {
                    id: ProofValueId(0),
                    value_type: ProofOnlyValueType::FloatMeaning,
                },
                source: FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                    owner,
                    parameter,
                    format: IeeeFloatFormat::Binary32,
                }),
                operation: FloatMeaningProjectionOperation::Meaning32,
                contract: contract(
                    psi_numerics::float_projection::FloatProjectionOperation::Meaning32,
                ),
            }],
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            quotient_correspondences: Vec::new(),
            machines: vec![terminal_machine(
                owner,
                parameter,
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            )],
        }
    }

    fn direct_result_module() -> TerminalModule {
        let mut module = direct_module();
        let owner = module.entry;
        let result = semantic_id(2, ValueId::new);
        module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        });
        module.float_meaning_projections[0].source =
            FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
                owner,
                result,
                format: IeeeFloatFormat::Binary32,
            });
        module
    }

    fn direct_operation_result_module(format: IeeeFloatFormat) -> TerminalModule {
        let mut module = direct_module();
        let owner = module.entry;
        let producer = semantic_id(1, OperationId::new);
        let result = semantic_id(2, ValueId::new);
        let block = module.machines[0].entry;
        module.machines[0].blocks = vec![Block {
            id: block,
            parameters: vec![ValueDeclaration {
                id: semantic_id(3, ValueId::new),
                scalar_type: ScalarType::IeeeFloat(format),
            }],
            operations: vec![Operation {
                id: producer,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: result,
                    scalar_type: ScalarType::IeeeFloat(format),
                }),
                kind: OperationKind::IeeeFloatConstant {
                    value: match format {
                        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0x3f80_0000),
                        IeeeFloatFormat::Binary64 => {
                            IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000)
                        }
                    },
                },
            }],
            terminator: Terminator::ReturnUnit {
                trivial_affine_discards: Vec::new(),
                edge: semantic_id(1, EdgeId::new),
            },
        }];
        module.float_meaning_projections[0].source =
            FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                owner,
                producer,
                result,
                format,
            });
        module.float_meaning_projections[0].operation = match format {
            IeeeFloatFormat::Binary32 => FloatMeaningProjectionOperation::Meaning32,
            IeeeFloatFormat::Binary64 => FloatMeaningProjectionOperation::Meaning64,
        };
        module.float_meaning_projections[0].contract = contract(match format {
            IeeeFloatFormat::Binary32 => {
                psi_numerics::float_projection::FloatProjectionOperation::Meaning32
            }
            IeeeFloatFormat::Binary64 => {
                psi_numerics::float_projection::FloatProjectionOperation::Meaning64
            }
        });
        module
    }

    fn direct_block_parameter_module(format: IeeeFloatFormat) -> TerminalModule {
        let mut module = direct_operation_result_module(format);
        let owner = module.entry;
        let block = module.machines[0].blocks[0].id;
        let parameter = module.machines[0].blocks[0].parameters[0].id;
        module.float_meaning_projections[0].source =
            FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                owner,
                block,
                parameter,
                format,
            });
        module
    }

    fn direct_call_result_module(format: IeeeFloatFormat) -> TerminalModule {
        let mut module = direct_operation_result_module(format);
        let owner = module.entry;
        let producer = module.machines[0].blocks[0].operations[0].id;
        let result = module.machines[0].blocks[0].operations[0]
            .result
            .scalar()
            .expect("fixture call scalar result")
            .id;
        module.machines[0].blocks[0].operations[0].kind = OperationKind::Call {
            callee: owner,
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        module.float_meaning_projections[0].source =
            FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
                owner,
                producer,
                result,
                format,
            });
        module
    }

    fn direct_structural_leaf_module(format: IeeeFloatFormat) -> TerminalModule {
        let mut module = direct_module();
        let owner = module.entry;
        let root = semantic_id(10, PlaceId::new);
        let structural_type = semantic_id(10, StructuralTypeId::new);
        let field = semantic_id(10, StructuralFieldId::new);
        module.structural_types = vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Fixture::FloatRecord".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::IeeeFloat(format),
                }],
            },
        }];
        module.machines[0].structural_parameters = vec![StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        module.float_meaning_projections[0].source =
            FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
                owner,
                field: IeeeFloatStructuralField::new(
                    root,
                    vec![CanonicalStructuralPathSegment::Field(field)],
                )
                .expect("nonempty structural field path"),
                format,
            });
        module.float_meaning_projections[0].operation = match format {
            IeeeFloatFormat::Binary32 => FloatMeaningProjectionOperation::Meaning32,
            IeeeFloatFormat::Binary64 => FloatMeaningProjectionOperation::Meaning64,
        };
        module.float_meaning_projections[0].contract = contract(match format {
            IeeeFloatFormat::Binary32 => {
                psi_numerics::float_projection::FloatProjectionOperation::Meaning32
            }
            IeeeFloatFormat::Binary64 => {
                psi_numerics::float_projection::FloatProjectionOperation::Meaning64
            }
        });
        module
    }

    #[test]
    fn module_rows_reconstruct_in_dense_source_free_order() {
        assert_eq!(validate_rows(&[projection(0), projection(1)]), Ok(()));
    }

    #[test]
    fn module_rows_reject_identity_and_format_tampering() {
        assert!(matches!(
            validate_rows(&[projection(1)]),
            Err(ModuleError::NonDenseFloatMeaningProjection { .. })
        ));

        let mut tampered = projection(0);
        tampered.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary64,
        });
        assert!(matches!(
            validate_rows(&[tampered]),
            Err(ModuleError::InvalidFloatMeaningProjection { .. })
        ));
    }

    #[test]
    fn module_rows_reject_duplicate_values_and_inconsistent_source_formats() {
        let mut duplicate = projection(1);
        duplicate.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary32,
        });
        assert!(matches!(
            validate_rows(&[projection(0), duplicate.clone()]),
            Err(ModuleError::DuplicateFloatMeaningProjection {
                first: 0,
                duplicate: 1,
            })
        ));

        duplicate.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(0),
            format: IeeeFloatFormat::Binary64,
        });
        duplicate.operation = FloatMeaningProjectionOperation::Meaning64;
        assert!(matches!(
            validate_rows(&[projection(0), duplicate]),
            Err(ModuleError::InconsistentFloatMeaningProjectionSourceFormat { source: 0 })
        ));
    }

    #[test]
    fn exact_literal_identity_ignores_producer_numbering_and_retains_raw_bits() {
        let positive_zero = exact_binary32_projection(0, 0x0000_0000);
        let mut duplicate_with_fresh_ids = exact_binary32_projection(1, 0x0000_0000);
        assert!(matches!(
            validate_rows(&[positive_zero.clone(), duplicate_with_fresh_ids.clone()]),
            Err(ModuleError::DuplicateFloatMeaningProjection {
                first: 0,
                duplicate: 1,
            })
        ));

        duplicate_with_fresh_ids.source = FloatMeaningSource::ExactBinary32Literal(0x8000_0000);
        assert_eq!(
            validate_rows(&[positive_zero, duplicate_with_fresh_ids]),
            Ok(())
        );
    }

    #[test]
    fn exact_literal_rejects_cross_format_operation_substitution() {
        let mut projection = exact_binary32_projection(0, 0x3f80_0000);
        projection.operation = FloatMeaningProjectionOperation::Meaning64;
        assert!(matches!(
            validate_rows(&[projection]),
            Err(ModuleError::InvalidFloatMeaningProjection { .. })
        ));
    }

    #[test]
    fn direct_parameter_rejoins_only_its_exact_owner_parameter_and_format() {
        let module = direct_module();
        assert_eq!(validate_direct_sources(&module), Ok(()));

        let mut unknown_owner = module.clone();
        let FloatMeaningSource::DirectMachineParameter(parameter) =
            &mut unknown_owner.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        parameter.owner = semantic_id(2, MachineId::new);
        assert!(matches!(
            validate_direct_sources(&unknown_owner),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectParameterOwner(_),
                ..
            })
        ));

        let mut unknown_parameter = module.clone();
        let FloatMeaningSource::DirectMachineParameter(parameter) =
            &mut unknown_parameter.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        parameter.parameter = semantic_id(2, ValueId::new);
        assert!(matches!(
            validate_direct_sources(&unknown_parameter),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectParameter { .. },
                ..
            })
        ));

        let mut wrong_format = module;
        let FloatMeaningSource::DirectMachineParameter(parameter) =
            &mut wrong_format.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        parameter.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_direct_sources(&wrong_format),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectParameterFormatMismatch,
                ..
            })
        ));
    }

    #[test]
    fn direct_parameter_identity_includes_the_owner_machine() {
        let mut module = direct_module();
        let second_owner = semantic_id(2, MachineId::new);
        let shared_parameter = semantic_id(1, ValueId::new);
        module.machines.push(terminal_machine(
            second_owner,
            shared_parameter,
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ));
        let mut second = module.float_meaning_projections[0].clone();
        second.result.id = ProofValueId(1);
        second.source = FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
            owner: second_owner,
            parameter: shared_parameter,
            format: IeeeFloatFormat::Binary32,
        });
        module.float_meaning_projections.push(second.clone());
        assert_eq!(validate_rows(&module.float_meaning_projections), Ok(()));
        assert_eq!(validate_direct_sources(&module), Ok(()));

        let mut duplicate = second;
        duplicate.source = module.float_meaning_projections[0].source.clone();
        assert!(matches!(
            validate_rows(&[module.float_meaning_projections[0].clone(), duplicate]),
            Err(ModuleError::DuplicateFloatMeaningProjection { .. })
        ));
    }

    #[test]
    fn direct_result_rejoins_only_its_exact_owner_scalar_result_and_format() {
        let module = direct_result_module();
        assert_eq!(validate_direct_sources(&module), Ok(()));

        let mut unknown_owner = module.clone();
        let FloatMeaningSource::DirectMachineResult(result) =
            &mut unknown_owner.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        result.owner = semantic_id(2, MachineId::new);
        assert!(matches!(
            validate_direct_sources(&unknown_owner),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectResultOwner(_),
                ..
            })
        ));

        let mut redirected_to_parameter = module.clone();
        let FloatMeaningSource::DirectMachineResult(result) =
            &mut redirected_to_parameter.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        result.result = redirected_to_parameter.machines[0].parameters[0].id;
        assert!(matches!(
            validate_direct_sources(&redirected_to_parameter),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectResult { .. },
                ..
            })
        ));

        let mut wrong_format = module.clone();
        let FloatMeaningSource::DirectMachineResult(result) =
            &mut wrong_format.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        result.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_direct_sources(&wrong_format),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectResultFormatMismatch,
                ..
            })
        ));

        let mut unit_result = module;
        unit_result.machines[0].result = TerminalMachineResult::Unit;
        assert!(matches!(
            validate_direct_sources(&unit_result),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectResult { .. },
                ..
            })
        ));
    }

    #[test]
    fn direct_block_parameter_rejoins_exact_f32_and_f64_declarations() {
        assert_eq!(
            validate_direct_sources(&direct_block_parameter_module(IeeeFloatFormat::Binary32)),
            Ok(())
        );
        assert_eq!(
            validate_direct_sources(&direct_block_parameter_module(IeeeFloatFormat::Binary64)),
            Ok(())
        );
    }

    #[test]
    fn direct_block_parameter_rejects_coordinates_class_and_format_substitution() {
        let module = direct_block_parameter_module(IeeeFloatFormat::Binary32);

        let mut wrong_owner = module.clone();
        let FloatMeaningSource::DirectBlockParameter(source) =
            &mut wrong_owner.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.owner = semantic_id(2, MachineId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_owner),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterOwner(_),
                ..
            })
        ));

        let mut wrong_block = module.clone();
        let FloatMeaningSource::DirectBlockParameter(source) =
            &mut wrong_block.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.block = semantic_id(2, BlockId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_block),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectBlockParameterBlock { .. },
                ..
            })
        ));

        let mut machine_parameter = module.clone();
        let parameter = machine_parameter.machines[0].parameters[0].id;
        let FloatMeaningSource::DirectBlockParameter(source) =
            &mut machine_parameter.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.parameter = parameter;
        assert!(matches!(
            validate_direct_sources(&machine_parameter),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter { .. },
                ..
            })
        ));

        let mut operation_result = module.clone();
        let result = operation_result.machines[0].blocks[0].operations[0]
            .result
            .scalar()
            .expect("fixture operation scalar result")
            .id;
        let FloatMeaningSource::DirectBlockParameter(source) =
            &mut operation_result.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.parameter = result;
        assert!(matches!(
            validate_direct_sources(&operation_result),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectBlockParameter { .. },
                ..
            })
        ));

        let mut wrong_format = module;
        let FloatMeaningSource::DirectBlockParameter(source) =
            &mut wrong_format.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_direct_sources(&wrong_format),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectBlockParameterFormatMismatch,
                ..
            })
        ));
    }

    #[test]
    fn direct_operation_result_rejoins_exact_f32_and_f64_producers() {
        assert_eq!(
            validate_direct_sources(&direct_operation_result_module(IeeeFloatFormat::Binary32)),
            Ok(())
        );
        assert_eq!(
            validate_direct_sources(&direct_operation_result_module(IeeeFloatFormat::Binary64)),
            Ok(())
        );
    }

    #[test]
    fn direct_operation_result_rejects_coordinate_and_result_class_substitution() {
        let module = direct_operation_result_module(IeeeFloatFormat::Binary32);

        let mut wrong_owner = module.clone();
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut wrong_owner.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.owner = semantic_id(2, MachineId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_owner),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResultOwner(_),
                ..
            })
        ));

        let mut wrong_producer = module.clone();
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut wrong_producer.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.producer = semantic_id(2, OperationId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_producer),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResultProducer { .. },
                ..
            })
        ));

        let mut redirected_to_block_parameter = module.clone();
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut redirected_to_block_parameter.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.result = redirected_to_block_parameter.machines[0].blocks[0].parameters[0].id;
        assert!(matches!(
            validate_direct_sources(&redirected_to_block_parameter),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResult { .. },
                ..
            })
        ));

        let mut redirected_to_parameter = module.clone();
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut redirected_to_parameter.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.result = redirected_to_parameter.machines[0].parameters[0].id;
        assert!(matches!(
            validate_direct_sources(&redirected_to_parameter),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResult { .. },
                ..
            })
        ));

        let mut redirected_to_machine_result = module.clone();
        let machine_result = ValueDeclaration {
            id: semantic_id(4, ValueId::new),
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        };
        redirected_to_machine_result.machines[0].result =
            TerminalMachineResult::Scalar(machine_result);
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut redirected_to_machine_result.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.result = machine_result.id;
        assert!(matches!(
            validate_direct_sources(&redirected_to_machine_result),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResult { .. },
                ..
            })
        ));

        let mut wrong_format = module.clone();
        let FloatMeaningSource::DirectOperationResult(source) =
            &mut wrong_format.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_direct_sources(&wrong_format),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectOperationResultFormatMismatch,
                ..
            })
        ));

        let mut unit = module.clone();
        unit.machines[0].blocks[0].operations[0].result = OperationResult::Unit;
        assert!(matches!(
            validate_direct_sources(&unit),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResult { .. },
                ..
            })
        ));

        let mut call = module.clone();
        call.machines[0].blocks[0].operations[0].kind = OperationKind::Call {
            callee: call.entry,
            arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        assert!(matches!(
            validate_direct_sources(&call),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectOperationResultCallProducer { .. },
                ..
            })
        ));

        let mut structural = module;
        structural.machines[0].blocks[0].operations[0].result =
            OperationResult::Structural(StructuralOperationResult {
                place: semantic_id(1, PlaceId::new),
                structural_type: semantic_id(1, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            });
        assert!(matches!(
            validate_direct_sources(&structural),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectOperationResult { .. },
                ..
            })
        ));
    }

    #[test]
    fn direct_call_result_rejoins_every_scalar_result_call_variant() {
        assert_eq!(
            validate_direct_sources(&direct_call_result_module(IeeeFloatFormat::Binary32)),
            Ok(())
        );
        assert_eq!(
            validate_direct_sources(&direct_call_result_module(IeeeFloatFormat::Binary64)),
            Ok(())
        );

        let variants = [
            OperationKind::CallStructuralScalar {
                callee: semantic_id(1, MachineId::new),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            OperationKind::CallDynamicScalar {
                descriptor_ordinal: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            OperationKind::CallDynamicParameterScalar {
                parameter_ordinal: 0,
                requirement_slot: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            OperationKind::BoundaryCall {
                boundary: semantic_id(1, psi_core::BoundaryMachineId::new),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        ];
        for variant in variants {
            let mut module = direct_call_result_module(IeeeFloatFormat::Binary32);
            module.machines[0].blocks[0].operations[0].kind = variant;
            assert_eq!(validate_direct_sources(&module), Ok(()));
        }
    }

    #[test]
    fn direct_call_result_rejects_coordinates_non_calls_and_wrong_result_classes() {
        let module = direct_call_result_module(IeeeFloatFormat::Binary32);

        let mut wrong_owner = module.clone();
        let FloatMeaningSource::DirectCallResult(source) =
            &mut wrong_owner.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.owner = semantic_id(2, MachineId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_owner),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResultOwner(_),
                ..
            })
        ));

        let mut wrong_producer = module.clone();
        let FloatMeaningSource::DirectCallResult(source) =
            &mut wrong_producer.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.producer = semantic_id(2, OperationId::new);
        assert!(matches!(
            validate_direct_sources(&wrong_producer),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducer { .. },
                ..
            })
        ));

        let mut wrong_value = module.clone();
        let block_parameter = wrong_value.machines[0].blocks[0].parameters[0].id;
        let FloatMeaningSource::DirectCallResult(source) =
            &mut wrong_value.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.result = block_parameter;
        assert!(matches!(
            validate_direct_sources(&wrong_value),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResult { .. },
                ..
            })
        ));

        let mut non_call = module.clone();
        non_call.machines[0].blocks[0].operations[0].kind = OperationKind::IeeeFloatConstant {
            value: IeeeFloatValue::Binary32(0x3f80_0000),
        };
        assert!(matches!(
            validate_direct_sources(&non_call),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducerKind { .. },
                ..
            })
        ));

        let mut unit_call = module.clone();
        unit_call.machines[0].blocks[0].operations[0].kind = OperationKind::CallUnit {
            callee: unit_call.entry,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        assert!(matches!(
            validate_direct_sources(&unit_call),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducerKind { .. },
                ..
            })
        ));

        let mut structural_call = module.clone();
        structural_call.machines[0].blocks[0].operations[0].kind = OperationKind::CallStructural {
            callee: structural_call.entry,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        };
        assert!(matches!(
            validate_direct_sources(&structural_call),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResultProducerKind { .. },
                ..
            })
        ));

        let mut unit_result = module.clone();
        unit_result.machines[0].blocks[0].operations[0].result = OperationResult::Unit;
        assert!(matches!(
            validate_direct_sources(&unit_result),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResult { .. },
                ..
            })
        ));

        let mut structural_result = module.clone();
        structural_result.machines[0].blocks[0].operations[0].result =
            OperationResult::Structural(StructuralOperationResult {
                place: semantic_id(1, PlaceId::new),
                structural_type: semantic_id(1, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            });
        assert!(matches!(
            validate_direct_sources(&structural_result),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectCallResult { .. },
                ..
            })
        ));

        let mut wrong_format = module;
        let FloatMeaningSource::DirectCallResult(source) =
            &mut wrong_format.float_meaning_projections[0].source
        else {
            unreachable!()
        };
        source.format = IeeeFloatFormat::Binary64;
        assert!(matches!(
            validate_direct_sources(&wrong_format),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::DirectCallResultFormatMismatch,
                ..
            })
        ));
    }

    #[test]
    fn direct_call_result_tuple_deduplicates_without_aliasing_non_call_source_class() {
        let module = direct_call_result_module(IeeeFloatFormat::Binary32);
        let first = module.float_meaning_projections[0].clone();
        let mut duplicate = first.clone();
        duplicate.result.id = ProofValueId(1);
        assert!(matches!(
            validate_rows(&[first.clone(), duplicate.clone()]),
            Err(ModuleError::DuplicateFloatMeaningProjection { .. })
        ));

        let FloatMeaningSource::DirectCallResult(source) = first.source else {
            unreachable!()
        };
        let mut non_call_class = duplicate;
        non_call_class.source =
            FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                owner: source.owner,
                producer: source.producer,
                result: source.result,
                format: source.format,
            });
        assert_eq!(validate_rows(&[first, non_call_class]), Ok(()));
    }

    #[test]
    fn direct_operation_result_tuple_deduplicates_and_does_not_alias_machine_result() {
        let module = direct_operation_result_module(IeeeFloatFormat::Binary32);
        let first = module.float_meaning_projections[0].clone();
        let mut duplicate = first.clone();
        duplicate.result.id = ProofValueId(1);
        assert!(matches!(
            validate_rows(&[first.clone(), duplicate.clone()]),
            Err(ModuleError::DuplicateFloatMeaningProjection { .. })
        ));

        let mut machine_result = duplicate;
        let FloatMeaningSource::DirectOperationResult(source) = first.source else {
            unreachable!()
        };
        machine_result.source = FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
            owner: source.owner,
            result: source.result,
            format: source.format,
        });
        assert_eq!(validate_rows(&[first, machine_result]), Ok(()));
    }

    #[test]
    fn direct_structural_leaf_replays_exact_owner_root_path_and_format() {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let module = direct_structural_leaf_module(format);
            assert_eq!(validate_direct_sources(&module), Ok(()));

            for access in [
                StructuralAccess::Owned,
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ] {
                let mut observable = module.clone();
                observable.machines[0].structural_parameters[0].access = access;
                assert_eq!(validate_direct_sources(&observable), Ok(()));
            }

            let mut write_only = module.clone();
            write_only.machines[0].structural_parameters[0].access =
                StructuralAccess::WriteOnlyBorrow;
            assert!(matches!(
                validate_direct_sources(&write_only),
                Err(ModuleError::InvalidFloatMeaningProjection {
                    error: crate::verification::FloatMeaningProjectionVerificationError::DirectStructuralLeafWriteOnlyRoot { .. },
                    ..
                })
            ));

            let mut wrong_owner = module.clone();
            let FloatMeaningSource::DirectStructuralLeaf(source) =
                &mut wrong_owner.float_meaning_projections[0].source
            else {
                unreachable!()
            };
            source.owner = semantic_id(99, MachineId::new);
            assert!(matches!(
                validate_direct_sources(&wrong_owner),
                Err(ModuleError::InvalidFloatMeaningProjection {
                    error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeafOwner(_),
                    ..
                })
            ));

            let mut wrong_root = module.clone();
            let FloatMeaningSource::DirectStructuralLeaf(source) =
                &mut wrong_root.float_meaning_projections[0].source
            else {
                unreachable!()
            };
            source.field = IeeeFloatStructuralField::new(
                semantic_id(99, PlaceId::new),
                source.field.path().to_vec(),
            )
            .expect("nonempty structural field path");
            assert!(matches!(
                validate_direct_sources(&wrong_root),
                Err(ModuleError::InvalidFloatMeaningProjection {
                    error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf { .. },
                    ..
                })
            ));

            let mut wrong_path = module.clone();
            let FloatMeaningSource::DirectStructuralLeaf(source) =
                &mut wrong_path.float_meaning_projections[0].source
            else {
                unreachable!()
            };
            source.field = IeeeFloatStructuralField::new(
                source.field.root(),
                vec![CanonicalStructuralPathSegment::Field(semantic_id(
                    99,
                    StructuralFieldId::new,
                ))],
            )
            .expect("nonempty structural field path");
            assert!(matches!(
                validate_direct_sources(&wrong_path),
                Err(ModuleError::InvalidFloatMeaningProjection {
                    error: crate::verification::FloatMeaningProjectionVerificationError::InvalidDirectStructuralLeaf { .. },
                    ..
                })
            ));

            let mut wrong_format = module;
            let FloatMeaningSource::DirectStructuralLeaf(source) =
                &mut wrong_format.float_meaning_projections[0].source
            else {
                unreachable!()
            };
            source.format = match format {
                IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
            };
            assert!(matches!(
                validate_direct_sources(&wrong_format),
                Err(ModuleError::InvalidFloatMeaningProjection {
                    error: crate::verification::FloatMeaningProjectionVerificationError::DirectStructuralLeafFormatMismatch,
                    ..
                })
            ));
        }
    }

    #[test]
    fn direct_structural_leaf_tuple_deduplicates_but_path_is_semantic() {
        let module = direct_structural_leaf_module(IeeeFloatFormat::Binary32);
        let first = module.float_meaning_projections[0].clone();
        let mut duplicate = first.clone();
        duplicate.result.id = ProofValueId(1);
        assert!(matches!(
            validate_rows(&[first.clone(), duplicate.clone()]),
            Err(ModuleError::DuplicateFloatMeaningProjection { .. })
        ));

        let FloatMeaningSource::DirectStructuralLeaf(source) = &mut duplicate.source else {
            unreachable!()
        };
        source.field = IeeeFloatStructuralField::new(
            source.field.root(),
            vec![CanonicalStructuralPathSegment::Field(semantic_id(
                99,
                StructuralFieldId::new,
            ))],
        )
        .expect("nonempty structural field path");
        assert_eq!(validate_rows(&[first, duplicate]), Ok(()));
    }

    #[test]
    fn equality_rows_consume_exact_projection_results() {
        let projections = vec![projection(0), projection(1)];
        let equalities = vec![FloatMeaningEqualityProposition {
            id: ProofPropositionId(0),
            left: ProofValueId(0),
            right: ProofValueId(1),
        }];
        assert_eq!(validate_equalities(&projections, &equalities), Ok(()));
    }

    #[test]
    fn equality_rows_reject_cross_format_projection_carriers() {
        let left = projection(0);
        let mut right = projection(1);
        right.source = FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(1),
            format: IeeeFloatFormat::Binary64,
        });
        right.operation = FloatMeaningProjectionOperation::Meaning64;
        right.contract =
            contract(psi_numerics::float_projection::FloatProjectionOperation::Meaning64);
        let equalities = vec![FloatMeaningEqualityProposition {
            id: ProofPropositionId(0),
            left: ProofValueId(0),
            right: ProofValueId(1),
        }];
        assert!(matches!(
            validate_equalities(&[left, right], &equalities),
            Err(ModuleError::InvalidFloatMeaningProjection {
                error: crate::verification::FloatMeaningProjectionVerificationError::EqualityCarrierMismatch,
                ..
            })
        ));
    }

    #[test]
    fn equality_rows_reject_operand_and_identity_tampering() {
        let projections = vec![projection(0), projection(1)];
        let mut equalities = vec![FloatMeaningEqualityProposition {
            id: ProofPropositionId(0),
            left: ProofValueId(0),
            right: ProofValueId(2),
        }];
        assert!(matches!(
            validate_equalities(&projections, &equalities),
            Err(ModuleError::UnknownFloatMeaningEqualityOperand { .. })
        ));

        equalities[0] = FloatMeaningEqualityProposition {
            id: ProofPropositionId(1),
            left: ProofValueId(0),
            right: ProofValueId(1),
        };
        assert!(matches!(
            validate_equalities(&projections, &equalities),
            Err(ModuleError::NonDenseFloatMeaningEquality { .. })
        ));
    }
}
