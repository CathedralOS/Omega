use super::*;
use abstract_operations::AbstractParameter;
use semantic_vocabulary::IeeeFloatFormat;
use target_operations::TargetUnitWriteOnlyPrimitiveStoreSource;

fn plan(format: IeeeFloatFormat, field_store: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let value = ValueId::new(1).unwrap();
    let scalar_type = ScalarType::IeeeFloat(format);
    let destination = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 1,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let shape = if field_store {
        StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "prefix".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                },
                StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::IeeeFloat(format),
                },
            ],
        }
    } else {
        StructuralTypeShape::PrimitiveScalar(scalar_type)
    };
    let operation = OperationId::new(1).unwrap();
    let result = AbstractResult { value, scalar_type };
    let store = if field_store {
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: operation,
            destination: destination.clone(),
            path: Vec::new(),
            field,
            value: result,
        }
    } else {
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation: operation,
            destination: destination.clone(),
            value: result,
        }
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "FloatDestination".into(),
            shape,
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter { value, scalar_type }],
            structural_parameters: vec![destination],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                store,
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

#[test]
fn ieee_store_parameters_keep_float_abi_and_exact_source_identity() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let byte_size = match format {
            IeeeFloatFormat::Binary32 => 4,
            IeeeFloatFormat::Binary64 => 8,
        };
        for field_store in [false, true] {
            let source = plan(format, field_store);
            for target in [
                NativeTarget::linux_x64(),
                NativeTarget::windows_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
            ] {
                let lowered = lower_to_target_operations(&source, target).unwrap();
                let function = &lowered.functions[0];
                let TargetOperation::UnitBody(body) = &function.operation else {
                    panic!("IEEE store retains the ordinary Unit body")
                };
                assert_eq!(
                    body.call_plan.parameters[0].shape,
                    ValueShape::float(byte_size)
                );
                assert_eq!(
                    body.scalar_parameters[0].scalar_type,
                    ScalarType::IeeeFloat(format)
                );
                match &body.operations[0] {
                    TargetUnitOperation::WriteOnlyPrimitiveStore { source, .. } => {
                        assert_eq!(
                            source,
                            &TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                                parameter_index: 0,
                                source_value: ValueId::new(1).unwrap(),
                                scalar_type: ScalarType::IeeeFloat(format),
                            }
                        );
                    }
                    TargetUnitOperation::StructuralScalarFieldStore {
                        source,
                        field_byte_offset,
                        ..
                    } => {
                        assert_eq!(*field_byte_offset, u32::from(byte_size));
                        assert_eq!(
                            source,
                            &TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: 0,
                                source_value: ValueId::new(1).unwrap(),
                                scalar_type: ScalarType::IeeeFloat(format),
                            }
                        );
                    }
                    _ => panic!("exact primitive or field store expected"),
                }
            }
            let mut mismatched = source;
            mismatched.functions[0].parameters[0].scalar_type =
                ScalarType::IeeeFloat(match format {
                    IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                    IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
                });
            assert!(lower_to_target_operations(&mismatched, NativeTarget::linux_x64()).is_err());
        }
    }
}

#[test]
fn forwarded_ieee_store_call_keeps_the_parameter_and_borrowed_pointer() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let mut source = plan(format, false);
        let mut caller = source.functions[0].clone();
        caller.machine = MachineId::new(2).unwrap();
        let destination = caller.structural_parameters[0].clone();
        caller.operations[0] = AbstractOperation::CallUnit {
            psi_operation: OperationId::new(2).unwrap(),
            callee: source.entry,
            arguments: vec![caller.parameters[0].value],
            structural_arguments: vec![StructuralArgument {
                place: destination.place,
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        source.entry = caller.machine;
        source.functions.push(caller);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let lowered = lower_to_target_operations(&source, target).unwrap();
            let TargetOperation::UnitBody(body) = &lowered.functions[1].operation else {
                panic!("forwarding remains an ordinary Unit body")
            };
            let TargetUnitOperation::Call {
                scalar_arguments,
                arguments,
                ..
            } = &body.operations[0]
            else {
                panic!("ordinary Unit call")
            };
            assert_eq!(
                scalar_arguments[0].source,
                TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::IeeeFloat(format),
                }
            );
            assert_eq!(arguments[0].access, StructuralAccess::WriteOnlyBorrow);
            assert_eq!(
                arguments[0].source,
                target_operations::TargetStructuralArgumentSource::Placement(
                    body.parameters[0].placement.clone()
                )
            );
        }
    }
}
