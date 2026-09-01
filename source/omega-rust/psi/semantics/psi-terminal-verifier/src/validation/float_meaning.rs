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
                transitional_source: transitional_source_id(projection.source),
            })?;
        if projection.result.id.0 != index {
            return Err(ModuleError::NonDenseFloatMeaningProjection {
                expected: index,
                result: projection.result.id.0,
                transitional_source: transitional_source_id(projection.source),
            });
        }
        if let psi_terminal::FloatMeaningSource::TransitionalInput(source) = projection.source {
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
        let key = (projection.source, projection.operation);
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
        match projection.source {
            psi_terminal::FloatMeaningSource::DirectMachineParameter(parameter) => {
                crate::verification::verify_direct_float_parameter(module, parameter)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            psi_terminal::FloatMeaningSource::DirectMachineResult(result) => {
                crate::verification::verify_direct_float_result(module, result)
                    .map_err(|error| ModuleError::InvalidFloatMeaningProjection { index, error })?;
            }
            _ => {}
        }
    }
    Ok(())
}

const fn transitional_source_id(source: psi_terminal::FloatMeaningSource) -> Option<u32> {
    match source {
        psi_terminal::FloatMeaningSource::TransitionalInput(input) => Some(input.id.0),
        psi_terminal::FloatMeaningSource::DirectMachineParameter(_)
        | psi_terminal::FloatMeaningSource::DirectMachineResult(_)
        | psi_terminal::FloatMeaningSource::ExactBinary32Literal(_)
        | psi_terminal::FloatMeaningSource::ExactBinary64Literal(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use psi_core::{BlockId, ContractId, IeeeFloatFormat, MachineId, ScalarType, ValueId};
    use psi_terminal::{
        DirectMachineFloatParameter, DirectMachineFloatResult, FloatMeaningEqualityProposition,
        FloatMeaningProjection, FloatMeaningProjectionOperation, FloatMeaningSource,
        FloatProjectionInput, FloatProjectionInputId, ProofOnlyValueType, ProofPropositionId,
        ProofValueDeclaration, ProofValueId, TerminalMachine, TerminalMachineResult,
        ValueDeclaration, VocabularyMarker,
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
            validate_rows(&[projection(0), duplicate]),
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
            validate_rows(&[positive_zero, duplicate_with_fresh_ids]),
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
        let mut second = module.float_meaning_projections[0];
        second.result.id = ProofValueId(1);
        second.source = FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
            owner: second_owner,
            parameter: shared_parameter,
            format: IeeeFloatFormat::Binary32,
        });
        module.float_meaning_projections.push(second);
        assert_eq!(validate_rows(&module.float_meaning_projections), Ok(()));
        assert_eq!(validate_direct_sources(&module), Ok(()));

        let mut duplicate = second;
        duplicate.source = module.float_meaning_projections[0].source;
        assert!(matches!(
            validate_rows(&[module.float_meaning_projections[0], duplicate]),
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
