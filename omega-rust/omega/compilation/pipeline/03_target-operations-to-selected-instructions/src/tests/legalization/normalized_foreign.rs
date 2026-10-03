//! Evaluated normalized foreign calls keep admitted provider custody through
//! projection and reject substituted bindings, executions, and arguments.
use crate::{legalize_target_operations, validate_legalized_operations};
use abstract_operations::{
    AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan, AbstractResult,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundarySettlementRealization, NormalizedForeignCallBinding, TargetOperationPlan,
    TargetUnitOperation,
};

const REQUIREMENT: &str = "Foreign::leaf";

#[derive(Debug)]
struct Execution {
    plan_report: u64,
}

impl installation_evidence::ProviderExecutionEvidence for Execution {
    fn requirement_identity(&self) -> &str {
        REQUIREMENT
    }
    fn provider_plan_report_identity(&self) -> u64 {
        self.plan_report
    }
    fn provider_execution_report_identity(&self) -> u64 {
        0xB1
    }
    fn provider_execution_report_fingerprint(&self) -> u64 {
        0xB2
    }
    fn normalized_root_report_identity(&self) -> u64 {
        0xB3
    }
    fn boundary_contract_report_fingerprint(&self) -> u64 {
        0xB4
    }
}

fn locator_for(native: NativeTarget) -> target::NormalizedForeignLocator {
    let (candidate, profile) = match native.architecture {
        target::Architecture::X86_64 if native.object_format == target::ObjectFormat::Coff => (
            target::ForeignLocatorCandidate::PeByName {
                library: b"KERNEL32.dll".to_vec(),
                export: b"Leaf".to_vec(),
            },
            target::TargetProfile::WindowsX64,
        ),
        target::Architecture::X86_64 => (
            target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"leaf".to_vec(),
                version: b"GLIBC_2.0".to_vec(),
            },
            target::TargetProfile::LinuxX64,
        ),
        target::Architecture::Aarch64 if native.object_format == target::ObjectFormat::MachO => (
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"libm.dylib".to_vec(),
                symbol: b"leaf".to_vec(),
            },
            target::TargetProfile::MacosArm64,
        ),
        _ => (
            target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"leaf".to_vec(),
                version: b"GLIBC_2.0".to_vec(),
            },
            target::TargetProfile::LinuxArm64,
        ),
    };
    target::normalize_foreign_locator(candidate, profile).expect("applicable locator")
}

fn binding(
    native: NativeTarget,
    signature: calling_conventions::CallSignature,
) -> NormalizedForeignCallBinding {
    let locator = locator_for(native);
    let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(native),
        &signature,
    )
    .expect("evaluated entry plan")
    .plan()
    .clone();
    let provider_plan_report_identity = 0xA1;
    let provider_plan_commitment =
        task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
    let same_stack_contribution = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity,
            provider_plan_commitment,
            requirement_identity: REQUIREMENT.to_owned(),
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0xA2,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        provider_plan_report_identity,
        provider_plan_commitment,
        REQUIREMENT,
    )
    .expect("same-stack admission");
    NormalizedForeignCallBinding {
        boundary_entry_plan,
        mechanism: target_operations::NormalizedForeignCallMechanism::Import {
            locator,
            same_stack_contribution,
        },
    }
}

fn declaration(
    scalar_parameters: Vec<ScalarType>,
    structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    result: terminal_psi::BoundaryMachineResult,
) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        id: BoundaryMachineId::new(1).unwrap(),
        identity: REQUIREMENT.into(),
        attachment: None,
        parameter_order: std::iter::repeat_n(
            terminal_psi::BoundaryParameterKind::Scalar,
            scalar_parameters.len(),
        )
        .chain(std::iter::repeat_n(
            terminal_psi::BoundaryParameterKind::Structural,
            structural_parameters.len(),
        ))
        .collect(),
        scalar_parameters,
        structural_parameters,
        result,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
    }
}

fn lower(
    source: &AbstractOperationPlan,
    native: NativeTarget,
    execution: &dyn installation_evidence::ProviderExecutionEvidence,
    binding: NormalizedForeignCallBinding,
) -> TargetOperationPlan {
    abstract_operations_to_target_operations::lower_to_target_operations(
        source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[AdmittedBoundarySettlement {
                boundary: BoundaryMachineId::new(1).unwrap(),
                execution: AdmittedBoundaryExecution::Provider(execution),
                realization: BoundarySettlementRealization::NormalizedForeignCall(binding),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
    .unwrap()
}

fn seed(source: &AbstractOperationPlan) -> optimization_unit::PsiOptimizationUnit {
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

/// One scalar-lane leaf: `Foreign::leaf(i32) -> Unit` over an authored
/// constant. Only immediates and durable homes are admissible scalar sources.
fn scalar_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let value = ValueId::new(5).unwrap();
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    source.boundary_machines.push(declaration(
        vec![i32_type],
        Vec::new(),
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source.functions[0].operations.insert(
        0,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(6).unwrap(),
            result: value,
            scalar_type: i32_type,
            value: semantic_vocabulary::IntegerValue::Signed(9),
        },
    );
    source.functions[0].operations.insert(
        1,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: vec![value],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (source, Execution { plan_report: 0xA1 })
}

/// The flat-record lane: `main(&mut self)` calls `shift(&self.p) -> i32`
/// through a shared-borrowed record projection, exactly as the compiler probe
/// authors it.
fn flat_record_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let main_type = StructuralTypeId::new(1).unwrap();
    let point_type = StructuralTypeId::new(2).unwrap();
    let p_field = StructuralFieldId::new(1).unwrap();
    let x_field = StructuralFieldId::new(2).unwrap();
    let y_field = StructuralFieldId::new(3).unwrap();
    let self_place = PlaceId::new(1).unwrap();
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    source.structural_types.make_mut().extend([
        terminal_psi::StructuralTypeDeclaration {
            id: main_type,
            identity: "probe::Main".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: p_field,
                    identity: "p".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(point_type),
                }],
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: point_type,
            identity: "probe::Point".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: [("x", x_field), ("y", y_field)]
                    .map(|(identity, id)| terminal_psi::StructuralFieldDeclaration {
                        id,
                        identity: identity.into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(i32_type),
                    })
                    .to_vec(),
            },
        },
    ]);
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(9).unwrap(),
            position: 0,
            is_self: false,
            structural_type: point_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        terminal_psi::BoundaryMachineResult::Scalar(i32_type),
    ));
    source.functions[0].attachment = Some(main_type);
    source.functions[0]
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: self_place,
            position: 0,
            is_self: true,
            structural_type: main_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Scalar(AbstractResult {
                value: ValueId::new(5).unwrap(),
                scalar_type: i32_type,
            }),
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: self_place,
                path: vec![terminal_psi::StructuralPathSegment::Field("p".to_owned())],
                access: terminal_psi::StructuralAccess::SharedBorrow,
            }],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (source, Execution { plan_report: 0xA1 })
}

#[test]
fn local_record_foreign_loans_replay_exact_establishment_and_projection() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for (projected, copied) in [(false, false), (true, false), (false, true)] {
            let (mut source, execution) = flat_record_fixture();
            let function = &mut source.functions[0];
            let place = function.structural_parameters[0].place;
            let root_type = if projected {
                StructuralTypeId::new(1).unwrap()
            } else {
                StructuralTypeId::new(2).unwrap()
            };
            function.attachment = None;
            function.structural_parameters.clear();
            if !projected {
                let AbstractOperation::BoundaryCall {
                    structural_arguments,
                    ..
                } = &mut function.operations[0]
                else {
                    unreachable!()
                };
                structural_arguments[0].path.clear();
            }
            function.operations.insert(
                0,
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: OperationId::new(6).unwrap(),
                    result: terminal_psi::StructuralOperationResult {
                        place,
                        structural_type: root_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
            );
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = function.operations.last_mut().unwrap()
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                place,
            ));
            if copied {
                let copied_place = PlaceId::new(2).unwrap();
                let AbstractOperation::BoundaryCall {
                    structural_arguments,
                    ..
                } = &mut function.operations[1]
                else {
                    unreachable!()
                };
                structural_arguments[0].place = copied_place;
                function.operations.insert(
                    1,
                    AbstractOperation::StructuralLeafCopy {
                        psi_operation: OperationId::new(8).unwrap(),
                        source: place,
                        path: abstract_operations::StructuralCopyPath::Ordinary(Vec::new()),
                        result: terminal_psi::StructuralOperationResult {
                            place: copied_place,
                            structural_type: root_type,
                            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            qualification_establishments: Vec::new(),
                            claims: Vec::new(),
                        },
                    },
                );
            }
            let binding = binding(
                native,
                calling_conventions::CallSignature {
                    parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
                    result: Some(calling_conventions::ValueShape::integer(4, 4)),
                },
            );
            let target = lower(&source, native, &execution, binding);
            let unit = seed(&source);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();

            for mutation in 0..4 {
                let mut changed = target.clone();
                let TargetUnitOperation::NormalizedForeignCall {
                    structural_arguments,
                    ..
                } = changed.functions[0].graph.blocks[0]
                    .operations
                    .iter_mut()
                    .find(|row| matches!(row, TargetUnitOperation::NormalizedForeignCall { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                let argument = &mut structural_arguments[0];
                match mutation {
                    0 => {
                        argument.source =
                            target_operations::TargetStructuralArgumentSource::StructuralHome {
                                psi_operation: OperationId::new(7).unwrap(),
                            }
                    }
                    1 => argument.source_byte_offset += 1,
                    2 => argument.root_structural_type = StructuralTypeId::new(999).unwrap(),
                    _ => argument.place = PlaceId::new(999).unwrap(),
                }
                assert!(abstract_operations_to_target_operations::validate_abstract_to_target_translation(&source, native, &changed).is_err(), "target mutation {mutation}: {native:?}, projected={projected}");
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "legalization mutation {mutation}: {native:?}, projected={projected}"
                );
            }
            let mut changed = legal.plan().clone();
            normalized_foreign_call_mut(&mut changed).structural_arguments[0].source_byte_offset +=
                1;
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
            let mut changed = selected.plan().clone();
            changed.functions[0].normalized_foreign_calls[0]
                .call
                .structural_arguments[0]
                .source_byte_offset += 1;
            assert!(
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
        }
    }
}

/// The libobjc/libSystem `const char*` lane: `Foreign::leaf(&u8) -> u64`.
/// Unforwarded, `main(&self)` over `Words { bytes: [u8; 6] }` lends
/// `&self.bytes[2]`; forwarded, `main(text: &u8)` lends its own loan whole.
fn borrowed_byte_fixture(forwarded: bool) -> (AbstractOperationPlan, Execution) {
    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let byte_type = StructuralTypeId::new(1).unwrap();
    let bytes_type = StructuralTypeId::new(2).unwrap();
    let words_type = StructuralTypeId::new(3).unwrap();
    let caller_place = PlaceId::new(1).unwrap();
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    source.structural_types.make_mut().extend([
        terminal_psi::StructuralTypeDeclaration {
            id: byte_type,
            identity: "u8".into(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(u8_type),
        },
        terminal_psi::StructuralTypeDeclaration {
            id: bytes_type,
            identity: "[u8; 6]".into(),
            shape: terminal_psi::StructuralTypeShape::FixedArray {
                element: byte_type,
                length: 6,
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: words_type,
            identity: "probe::Words".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "bytes".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(bytes_type),
                }],
            },
        },
    ]);
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(9).unwrap(),
            position: 0,
            is_self: false,
            structural_type: byte_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        terminal_psi::BoundaryMachineResult::Scalar(u64_type),
    ));
    let (root, path) = if forwarded {
        (byte_type, Vec::new())
    } else {
        source.functions[0].attachment = Some(words_type);
        (
            words_type,
            vec![
                terminal_psi::StructuralPathSegment::Field("bytes".to_owned()),
                terminal_psi::StructuralPathSegment::FixedIndex(2),
            ],
        )
    };
    source.functions[0]
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: caller_place,
            position: 0,
            is_self: !forwarded,
            structural_type: root,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Scalar(AbstractResult {
                value: ValueId::new(5).unwrap(),
                scalar_type: u64_type,
            }),
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: caller_place,
                path,
                access: terminal_psi::StructuralAccess::SharedBorrow,
            }],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (source, Execution { plan_report: 0xA1 })
}

fn normalized_foreign_instruction(
    plan: &legalized_operations::LegalizedOperationPlan,
) -> &legalized_operations::LegalizedNormalizedForeignCall {
    plan.scalar_functions[0].blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
                Some(call)
            }
            _ => None,
        })
        .expect("normalized foreign instruction")
}

#[test]
fn foreign_record_results_preserve_storage_classification_and_capture_custody() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut import_template = None;
        for (field_count, field_type) in [
            (
                2,
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            ),
            (
                1,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
            ),
            (
                2,
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
            ),
            (
                2,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
            ),
            (
                3,
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
            ),
        ] {
            let (mut source, execution) = flat_record_fixture();
            let structural_type = StructuralTypeId::new(2).unwrap();
            {
                let declaration = source
                    .structural_types
                    .make_mut()
                    .iter_mut()
                    .find(|declaration| declaration.id == structural_type)
                    .unwrap();
                let terminal_psi::StructuralTypeShape::Record { fields } = &mut declaration.shape
                else {
                    panic!("record")
                };
                let template = fields[0].clone();
                *fields = (0..field_count)
                    .map(|position| {
                        let mut field = template.clone();
                        field.id = StructuralFieldId::new(position + 2).unwrap();
                        field.identity = format!("field_{position}");
                        field.field_type = terminal_psi::StructuralFieldType::Scalar(field_type);
                        field
                    })
                    .collect();
            }
            let result = terminal_psi::StructuralOperationResult {
                place: PlaceId::new(10).unwrap(),
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            };
            source.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Structural(
                terminal_psi::BoundaryStructuralResultDeclaration {
                    structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: Vec::new(),
                },
            );
            let AbstractOperation::BoundaryCall {
                result: produced, ..
            } = &mut source.functions[0].operations[0]
            else {
                panic!("boundary")
            };
            *produced = AbstractBoundaryResult::Structural(result.clone());
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = source.functions[0].operations.last_mut().unwrap()
            else {
                panic!("return")
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                result.place,
            ));
            let shape = abstract_operations_to_target_operations::classified_boundary_shape(
                structural_type,
                &source.structural_types,
                calling_conventions::CallingPolicy::native_for_target(native),
            )
            .unwrap();
            let binding = binding(
                native,
                calling_conventions::CallSignature {
                    parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
                    result: Some(shape),
                },
            );
            let target = lower(&source, native, &execution, binding);
            let unit = seed(&source);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let selected_call = selected.plan().functions[0].blocks[0]
                .instructions
                .iter()
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::NormalizedForeignCall { .. }
                    )
                })
                .unwrap();
            assert_eq!(
                *import_template.get_or_insert(selected_call.constraint),
                selected_call.constraint,
                "one import mechanism template must cover every evaluated result placement"
            );
            assert!(
                selected.plan().functions[0]
                    .local_storage_slots
                    .iter()
                    .any(|slot| slot.id
                        == selected_instructions::LocalStorageSlotId::Structural {
                            operation: OperationId::new(7).unwrap(),
                            place: result.place,
                        }
                        && slot.byte_size == shape.byte_size
                        && slot.alignment == shape.alignment)
            );
            for mutation in 0..3 {
                let mut changed = legal.plan().clone();
                let call = normalized_foreign_call_mut(&mut changed);
                if mutation == 2 {
                    call.binding
                        .boundary_entry_plan
                        .call
                        .result
                        .as_mut()
                        .unwrap()
                        .shape
                        .class = calling_conventions::ValueClass::BorrowedReference;
                } else {
                    let target_operations::TargetBoundaryResult::Structural(home) =
                        &mut call.result
                    else {
                        panic!("structural result")
                    };
                    if mutation == 0 {
                        let target_operations::TargetStructuralHomeOrigin::OperationResult {
                            result,
                            ..
                        } = &mut home.origin
                        else {
                            panic!("operation result")
                        };
                        result.place = PlaceId::new(11).unwrap();
                    } else {
                        home.layout = target_operations::TargetStructuralHomeLayout::Aggregate(
                            calling_conventions::ValueShape::integer(
                                shape.byte_size + 8,
                                shape.alignment,
                            ),
                        );
                    }
                }
                assert!(
                    validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                    "substituted foreign result {mutation}"
                );
            }
            for mutation in 0..3 {
                let mut changed = selected.plan().clone();
                let instruction = changed.functions[0].blocks[0].instructions.iter_mut()
                    .find(|instruction| matches!(instruction.kind,
                        selected_instructions::SelectedInstructionKind::NormalizedForeignCall { .. }))
                    .unwrap();
                match mutation {
                    0 => instruction.operands.last_mut().unwrap().fixed_view = None,
                    1 => {
                        let operand = instruction.operands.last_mut().unwrap();
                        operand.access = match operand.access {
                            register_model::RegisterOperandAccess::Use => {
                                register_model::RegisterOperandAccess::Def
                            }
                            _ => register_model::RegisterOperandAccess::Use,
                        };
                    }
                    _ => instruction.clobbers.clear(),
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err(),
                    "substituted foreign placement/effect {mutation}"
                );
            }
            for mutation in 0..3 {
                let mut changed = selected.plan().clone();
                let roster = &mut changed.functions[0].normalized_foreign_calls;
                match mutation {
                    0 => roster.clear(),
                    1 => roster.push(roster[0].clone()),
                    _ => roster[0]
                        .call
                        .binding
                        .boundary_entry_plan
                        .call
                        .result
                        .as_mut()
                        .unwrap()
                        .locations
                        .reverse(),
                }
                // Reversing a single fragment does not corrupt its placement.
                if mutation == 2
                    && roster[0]
                        .call
                        .binding
                        .boundary_entry_plan
                        .call
                        .result
                        .as_ref()
                        .unwrap()
                        .locations
                        .len()
                        < 2
                {
                    continue;
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err(),
                    "substituted foreign call roster {mutation}"
                );
            }
            let direct_result = selected.plan().functions[0].normalized_foreign_calls[0]
                .call
                .binding
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .unwrap()
                .locations
                .iter()
                .all(|location| {
                    matches!(
                        location,
                        calling_conventions::ValueLocation::Register { .. }
                    )
                });
            for mutation in 0..if direct_result { 3 } else { 0 } {
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                let access = function
                    .memory_accesses
                    .iter_mut()
                    .find(|access| {
                        access.place == result.place
                            && access.role
                                == selected_instructions::SelectedMemoryAccessRole::WritePlace
                    })
                    .unwrap();
                let store_id = access.instruction;
                if mutation == 0 {
                    access.byte_offset += 1;
                } else {
                    let instructions = &mut function.blocks[0].instructions;
                    let position = instructions
                        .iter()
                        .position(|instruction| instruction.id == store_id)
                        .unwrap();
                    if mutation == 1 {
                        instructions[position].operands[1] = instructions[position].operands[0];
                    } else {
                        instructions.remove(position);
                    }
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err(),
                    "substituted foreign capture {mutation}"
                );
            }
        }
    }
}

#[test]
fn mixed_arguments_preserve_authored_order_through_selection_and_replay() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let (mut source, execution) = flat_record_fixture();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let values = [ValueId::new(100).unwrap(), ValueId::new(101).unwrap()];
        let boundary = &mut source.boundary_machines[0];
        boundary.scalar_parameters = vec![scalar_type; 2];
        boundary.parameter_order = vec![
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Structural,
            terminal_psi::BoundaryParameterKind::Scalar,
        ];
        let AbstractOperation::BoundaryCall { arguments, .. } =
            &mut source.functions[0].operations[0]
        else {
            panic!("boundary call");
        };
        *arguments = values.to_vec();
        for (scalar_ordinal, value) in values.into_iter().enumerate() {
            source.functions[0].operations.insert(
                scalar_ordinal,
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(100 + scalar_ordinal as u64).unwrap(),
                    result: value,
                    scalar_type,
                    value: semantic_vocabulary::IntegerValue::Signed(5 + scalar_ordinal as i128),
                },
            );
        }
        let scalar_shape = calling_conventions::ValueShape::integer(4, 4);
        let pointer_shape = calling_conventions::ValueShape::integer(
            native.pointer_size as u32,
            native.pointer_alignment as u16,
        );
        let binding = binding(
            native,
            calling_conventions::CallSignature {
                parameters: vec![scalar_shape, pointer_shape, scalar_shape],
                result: Some(scalar_shape),
            },
        );
        let target = lower(&source, native, &execution, binding);
        let unit = seed(&source);
        let legal =
            legalize_target_operations(&target, &source, &unit).expect("mixed call legalizes");
        let call = normalized_foreign_instruction(legal.plan());
        assert_eq!(
            call.scalar_arguments
                .iter()
                .map(|argument| argument.parameter_index)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        assert_eq!(
            call.structural_arguments[0].destination,
            call.binding.boundary_entry_plan.call.parameters[1]
        );
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        for mutation in 0..4 {
            let mut changed = legal.plan().clone();
            let call = normalized_foreign_call_mut(&mut changed);
            match mutation {
                0 => call.scalar_arguments[1].parameter_index = 0,
                1 => call.scalar_arguments[1].parameter_index = 1,
                2 => call.scalar_arguments.swap(0, 1),
                _ => {
                    call.structural_arguments[0].destination =
                        call.scalar_arguments[0].placement.clone()
                }
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "accepted mixed legalized mutation {mutation} on {native:?}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        let call = &function.normalized_foreign_calls[0];
        let slot = selected_instructions::LocalStorageSlotId::Boundary {
            operation: call.operation,
        };
        let instructions = &function.blocks[0].instructions;
        let call_position = instructions
            .iter()
            .position(|instruction| instruction.id == call.instruction)
            .unwrap();
        let save_position = instructions
            .iter()
            .position(|instruction| {
                instruction.kind
                    == selected_instructions::SelectedInstructionKind::SaveFloatingControl { slot }
            })
            .expect("save before argument staging");
        assert!(save_position < call_position);
        assert_eq!(
            instructions[call_position + 1].kind,
            selected_instructions::SelectedInstructionKind::RestoreFloatingControl { slot }
        );
        assert!(
            instructions[save_position..=call_position + 2]
                .iter()
                .all(|instruction| { instruction.provenance.operations.contains(&call.operation) }),
            "argument staging and result normalization belong to the complete call interval"
        );
        assert!(
            function
                .local_storage_slots
                .iter()
                .any(|home| { home.id == slot && home.byte_size == 8 && home.alignment == 8 })
        );
        for mutation in 0..7 {
            let mut changed = selected.plan().clone();
            let function = &mut changed.functions[0];
            let call = &mut function.normalized_foreign_calls[0];
            match mutation {
                0 => call.call.scalar_arguments[1].parameter_index = 1,
                1 => {
                    call.call.structural_arguments[0].destination =
                        call.call.scalar_arguments[0].placement.clone()
                }
                2 => {
                    let instruction = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.instructions)
                        .find(|instruction| instruction.id == call.instruction)
                        .unwrap();
                    instruction.operands.swap(1, 2);
                }
                3 => function.blocks[0]
                    .instructions
                    .swap(call_position, call_position + 1),
                4 => {
                    function.blocks[0].instructions[call_position + 1].kind =
                        selected_instructions::SelectedInstructionKind::RestoreFloatingControl {
                            slot: selected_instructions::LocalStorageSlotId::Boundary {
                                operation: OperationId::new(999).unwrap(),
                            },
                        };
                }
                5 => function.blocks[0].instructions[save_position + 1]
                    .provenance
                    .operations
                    .clear(),
                _ => {
                    function.blocks[0].instructions.remove(save_position);
                }
            }
            assert!(
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "accepted mixed selected mutation {mutation} on {native:?}"
            );
        }
    }
}

/// A fixed element inside the caller's borrowed receiver lends its address
/// at the element's static offset, and a caller's own `&u8` loan forwards
/// its pointer word whole; both replay through legalization and selection.
#[test]
fn borrowed_byte_elements_and_forwarded_loans_select_pointer_words() {
    let native = NativeTarget::macos_arm64();
    let word = calling_conventions::ValueShape::integer(8, 8);
    for (forwarded, offset) in [(false, 2), (true, 0)] {
        let (source, execution) = borrowed_byte_fixture(forwarded);
        let binding = binding(
            native,
            calling_conventions::CallSignature {
                parameters: vec![word],
                result: Some(word),
            },
        );
        let target = lower(&source, native, &execution, binding);
        let unit = seed(&source);
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("borrowed byte argument legalizes");
        let call = normalized_foreign_instruction(legal.plan());
        assert_eq!(call.structural_arguments[0].source_byte_offset, offset);
        assert_eq!(
            call.structural_arguments[0].destination,
            call.binding.boundary_entry_plan.call.parameters[0]
        );
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let mut shifted = legal.plan().clone();
        normalized_foreign_call_mut(&mut shifted).structural_arguments[0].source_byte_offset += 1;
        assert!(
            validate_legalized_operations(&target, &source, &unit, shifted).is_err(),
            "a shifted referent offset must not replay (forwarded: {forwarded})"
        );
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
    }
}

#[test]
fn mixed_scalar_banks_and_stack_arguments_replay_with_exact_types() {
    use calling_conventions::{ValueLocation, ValueShape};
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
    let types = [
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ScalarType::Boolean,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    ];
    let shapes = [
        ValueShape::float(4),
        ValueShape::integer(1, 1),
        ValueShape::float(8),
        ValueShape::integer(4, 4),
    ];
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for repetitions in [1, 5] {
            for result_index in 0..3 {
                let (mut source, execution) = flat_record_fixture();
                let count = types.len() * repetitions;
                let values = (0..count)
                    .map(|ordinal| ValueId::new(100 + ordinal as u64).unwrap())
                    .collect::<Vec<_>>();
                let declaration = &mut source.boundary_machines[0];
                declaration.scalar_parameters = types.into_iter().cycle().take(count).collect();
                declaration.result =
                    terminal_psi::BoundaryMachineResult::Scalar(types[result_index]);
                declaration.parameter_order =
                    vec![terminal_psi::BoundaryParameterKind::Scalar; count];
                declaration
                    .parameter_order
                    .insert(1, terminal_psi::BoundaryParameterKind::Structural);
                let AbstractOperation::BoundaryCall {
                    arguments, result, ..
                } = &mut source.functions[0].operations[0]
                else {
                    panic!("boundary call")
                };
                *arguments = values.clone();
                let AbstractBoundaryResult::Scalar(result) = result else {
                    panic!("scalar result")
                };
                result.scalar_type = types[result_index];
                for (ordinal, value) in values.iter().copied().enumerate() {
                    let operation = OperationId::new(100 + ordinal as u64).unwrap();
                    let constant = match ordinal % types.len() {
                        0 => AbstractOperation::IeeeFloatConstant {
                            psi_operation: operation,
                            result: value,
                            value: IeeeFloatValue::Binary32(1.5f32.to_bits()),
                        },
                        1 => AbstractOperation::BooleanConstant {
                            psi_operation: operation,
                            result: value,
                            value: true,
                        },
                        2 => AbstractOperation::IeeeFloatConstant {
                            psi_operation: operation,
                            result: value,
                            value: IeeeFloatValue::Binary64(2.25f64.to_bits()),
                        },
                        _ => AbstractOperation::IntegerConstant {
                            psi_operation: operation,
                            result: value,
                            scalar_type: types[3],
                            value: semantic_vocabulary::IntegerValue::Signed(17),
                        },
                    };
                    source.functions[0].operations.insert(ordinal, constant);
                }
                let mut parameters = shapes.into_iter().cycle().take(count).collect::<Vec<_>>();
                parameters.insert(
                    1,
                    ValueShape::integer(
                        native.pointer_size as u32,
                        native.pointer_alignment as u16,
                    ),
                );
                let binding = binding(
                    native,
                    calling_conventions::CallSignature {
                        parameters,
                        result: Some(shapes[result_index]),
                    },
                );
                let target = lower(&source, native, &execution, binding);
                let unit = seed(&source);
                let legal = legalize_target_operations(&target, &source, &unit).unwrap();
                validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
                    .unwrap();
                let call = normalized_foreign_instruction(legal.plan());
                assert_eq!(
                    call.scalar_arguments
                        .iter()
                        .map(|argument| argument.source_value())
                        .collect::<Vec<_>>(),
                    values
                );
                assert_eq!(call.scalar_arguments[0].parameter_index, 0);
                assert_eq!(call.scalar_arguments[1].parameter_index, 2);
                if repetitions == 5 {
                    assert!(call.scalar_arguments.iter().any(|argument| matches!(
                        argument.placement.locations.as_slice(),
                        [ValueLocation::Stack { .. }]
                    )));
                }
                for mutation in 0..3 {
                    let mut changed = legal.plan().clone();
                    let call = normalized_foreign_call_mut(&mut changed);
                    match mutation {
                        0 => call.scalar_arguments[0].parameter_index = 1,
                        1 => {
                            call.scalar_arguments[0].placement =
                                call.scalar_arguments[1].placement.clone()
                        }
                        _ => {
                            let target_operations::TargetBoundaryResult::Scalar(home) =
                                &mut call.result
                            else {
                                panic!("scalar result home");
                            };
                            home.scalar_type = types[(result_index + 1) % 3]
                        }
                    }
                    assert!(
                        validate_legalized_operations(&target, &source, &unit, changed).is_err()
                    );
                }
                let environment =
                    register_environment::baseline_target_register_environment(native).unwrap();
                let constraints = crate::selection_constraints(&legal, &environment);
                let selected = crate::select_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{native:?}, repetitions {repetitions}, result {result_index}: {error:?}"
                    )
                });
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    selected.plan().clone(),
                )
                .unwrap();
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                let call = &function.normalized_foreign_calls[0];
                function.blocks[0]
                    .instructions
                    .iter_mut()
                    .find(|instruction| instruction.id == call.instruction)
                    .unwrap()
                    .operands
                    .swap(0, 2);
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn system_call_transport_selects_supervisor_rows_without_floating_control_envelopes() {
    use target_operations::{NormalizedForeignCallMechanism, NormalizedSystemCallBinding};
    for profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ] {
        for (argument_count, returns_value) in
            [(0, false), (0, true), (1, false), (4, true), (6, true)]
        {
            let (mut source, execution) = scalar_fixture();
            let scalar_type = source.boundary_machines[0].scalar_parameters[0];
            source.boundary_machines[0].scalar_parameters = vec![scalar_type; argument_count];
            source.boundary_machines[0].parameter_order =
                vec![terminal_psi::BoundaryParameterKind::Scalar; argument_count];
            if returns_value {
                source.boundary_machines[0].result =
                    terminal_psi::BoundaryMachineResult::Scalar(scalar_type);
            }
            {
                let AbstractOperation::BoundaryCall {
                    arguments, result, ..
                } = &mut source.functions[0].operations[1]
                else {
                    panic!("boundary call");
                };
                *arguments = vec![ValueId::new(5).unwrap(); argument_count];
                if returns_value {
                    *result = AbstractBoundaryResult::Scalar(AbstractResult {
                        value: ValueId::new(9).unwrap(),
                        scalar_type,
                    });
                }
            }
            if returns_value && argument_count != 0 {
                source.functions[0].operations.insert(
                    2,
                    AbstractOperation::BoundaryCall {
                        psi_operation: OperationId::new(8).unwrap(),
                        result: AbstractBoundaryResult::Scalar(AbstractResult {
                            value: ValueId::new(10).unwrap(),
                            scalar_type,
                        }),
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        arguments: vec![ValueId::new(9).unwrap(); argument_count],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                );
            }
            let native = profile.native_target();
            let signature = calling_conventions::CallSignature {
                parameters: vec![calling_conventions::ValueShape::integer(4, 4); argument_count],
                result: returns_value.then_some(calling_conventions::ValueShape::integer(4, 4)),
            };
            let binding = NormalizedForeignCallBinding {
                boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    calling_conventions::CallingPolicy::native_syscall_for_target(native).unwrap(),
                    &signature,
                )
                .unwrap()
                .plan()
                .clone(),
                mechanism: NormalizedForeignCallMechanism::SystemCall(
                    NormalizedSystemCallBinding {
                        target: profile,
                        number: 211,
                        requirement_identity: REQUIREMENT.into(),
                        provider_plan_report_identity: execution.plan_report,
                        provider_plan_commitment: [0x42; 32],
                    },
                ),
            };
            let target = lower(&source, native, &execution, binding);
            let unit = seed(&source);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let function = &selected.plan().functions[0];
            for call in &function.normalized_foreign_calls {
                let instruction = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .find(|instruction| instruction.id == call.instruction)
                    .unwrap();
                assert_eq!(
                    instruction.constraint.family,
                    register_model::RegisterConstraintFamily::SystemCall
                );
                assert_eq!(
                    instruction.operands.len(),
                    argument_count + usize::from(returns_value)
                );
            }
            assert!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .all(|instruction| {
                        !matches!(instruction.kind,
                    selected_instructions::SelectedInstructionKind::SaveFloatingControl { .. }
                    | selected_instructions::SelectedInstructionKind::RestoreFloatingControl { .. })
                    })
            );
            assert!(function.local_storage_slots.iter().all(|slot| {
                !matches!(
                    slot.id,
                    selected_instructions::LocalStorageSlotId::Boundary { .. }
                )
            }));
            for mutation in 0..3 {
                let mut forged = selected.plan().clone();
                let function = &mut forged.functions[0];
                let call = &mut function.normalized_foreign_calls[0];
                if mutation == 0 {
                    let NormalizedForeignCallMechanism::SystemCall(binding) =
                        &mut call.call.binding.mechanism
                    else {
                        panic!("kernel binding");
                    };
                    binding.number += 1;
                } else {
                    let instruction = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.instructions)
                        .find(|instruction| instruction.id == call.instruction)
                        .unwrap();
                    if mutation == 1 {
                        instruction.clobbers.clear();
                    } else {
                        instruction.constraint.family =
                            register_model::RegisterConstraintFamily::Call;
                    }
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        forged,
                    )
                    .is_err(),
                    "accepted mutation {mutation} on {profile:?}"
                );
            }
        }
    }
}

#[test]
fn replay_rejects_substituted_binding_execution_arguments_and_home() {
    let native = NativeTarget::linux_x64();
    let (source, execution) = scalar_fixture();
    let shape = calling_conventions::ValueShape::integer(4, 4);
    let binding = binding(
        native,
        calling_conventions::CallSignature {
            parameters: vec![shape],
            result: None,
        },
    );
    let target = lower(&source, native, &execution, binding);
    let unit = seed(&source);
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let different_execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(0xA1).unwrap(),
        0xC1,
        0xC2,
        0xC3,
        0xC4,
    )
    .unwrap();
    let different_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(native),
        &calling_conventions::CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone();
    for mutation in 0..8 {
        let mut changed = legal.plan().clone();
        match mutation {
            0 => {
                normalized_foreign_call_mut(&mut changed).boundary =
                    BoundaryMachineId::new(2).unwrap()
            }
            1 => normalized_foreign_call_mut(&mut changed).provider_execution = different_execution,
            2 => {
                normalized_foreign_call_mut(&mut changed)
                    .binding
                    .boundary_entry_plan = different_plan.clone()
            }
            3 => normalized_foreign_call_mut(&mut changed).scalar_arguments[0].parameter_index = 9,
            4 => {
                let call = normalized_foreign_call_mut(&mut changed);
                call.scalar_arguments[0].source =
                    target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: OperationId::new(7).unwrap(),
                        source_value: ValueId::new(5).unwrap(),
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        value: semantic_vocabulary::IntegerValue::Signed(4),
                    }
            }
            5 => {
                normalized_foreign_call_mut(&mut changed).result =
                    target_operations::TargetBoundaryResult::Scalar(
                        target_operations::TargetUnitScalarHomeRequirement {
                            defining_operation: OperationId::new(7).unwrap(),
                            source_value: ValueId::new(9).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                            ),
                            shape,
                        },
                    )
            }
            6 => {
                changed.scalar_functions[0].blocks[0]
                    .instructions
                    .retain(|instruction| {
                        !matches!(
                            instruction.kind,
                            legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(_)
                        )
                    });
                continue;
            }
            _ => {
                *normalized_foreign_kind_mut(&mut changed) =
                    legalized_operations::LegalizedScalarInstructionKind::HostedWriteByteI32 {
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        source: ValueId::new(5).unwrap(),
                    }
            }
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "accepted legalized mutation {mutation}"
        );
    }
    // The same substitutions in the admitted target row fail input custody
    // before projection. The foreign-plan execution keeps the admitted
    // provider plan identity; a different plan identity breaks the same-stack
    // cross-check.
    let foreign_plan_execution =
        target_operations::ProviderExecutionBinding::from_execution_record(
            target_operations::ProviderPlanReportIdentity::new(0xA9).unwrap(),
            0xB1,
            0xB2,
            0xB3,
            0xB4,
        )
        .unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let row = changed.functions[0].graph.blocks[0]
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(operation, TargetUnitOperation::NormalizedForeignCall { .. })
            })
            .expect("normalized foreign row");
        let TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            ..
        } = row
        else {
            panic!("normalized foreign row")
        };
        match mutation {
            0 => *psi_operation = OperationId::new(99).unwrap(),
            1 => *boundary = BoundaryMachineId::new(2).unwrap(),
            2 => *provider_execution = foreign_plan_execution,
            3 => binding.boundary_entry_plan = different_plan.clone(),
            _ => scalar_arguments[0].parameter_index = 9,
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "accepted target mutation {mutation}"
        );
    }
}

fn normalized_foreign_kind_mut(
    plan: &mut legalized_operations::LegalizedOperationPlan,
) -> &mut legalized_operations::LegalizedScalarInstructionKind {
    plan.scalar_functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match &mut instruction.kind {
            kind @ legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(
                _,
            ) => Some(kind),
            _ => None,
        })
        .expect("normalized foreign instruction")
}

fn normalized_foreign_call_mut(
    plan: &mut legalized_operations::LegalizedOperationPlan,
) -> &mut legalized_operations::LegalizedNormalizedForeignCall {
    let legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) =
        normalized_foreign_kind_mut(plan)
    else {
        panic!("normalized foreign instruction")
    };
    call
}

/// Where the free caller in `dominated_result_fixture` places its producing
/// relay call and its consuming leaf call.
#[derive(Clone, Copy)]
enum ResultPlacement {
    /// The entry block produces; the true arm consumes. A static machine's
    /// later state reads an earlier state's foreign result this way.
    EntryToTrueArm,
    /// The entry block produces; the false arm consumes.
    EntryToFalseArm,
    /// The true arm produces and the false arm consumes: the producer does
    /// not dominate its use.
    SiblingBranches,
}

/// A free (receiverless) caller branching on its Boolean parameter.
/// `Foreign::relay(i32) -> i32` (boundary 2) produces value 8, and
/// `Foreign::leaf(i32)` (boundary 1) consumes it in another block.
fn dominated_result_fixture(placement: ResultPlacement) -> AbstractOperationPlan {
    use abstract_operations::{AbstractBlockEntry, AbstractParameter, AbstractSuccessor};
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let block = |identity| semantic_vocabulary::BlockId::new(identity).unwrap();
    let edge = |identity| semantic_vocabulary::EdgeId::new(identity).unwrap();
    let argument = ValueId::new(5).unwrap();
    let relayed = ValueId::new(8).unwrap();
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    source.boundary_machines.push(declaration(
        vec![i32_type],
        Vec::new(),
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            id: BoundaryMachineId::new(2).unwrap(),
            ..declaration(
                vec![i32_type],
                Vec::new(),
                terminal_psi::BoundaryMachineResult::Scalar(i32_type),
            )
        });
    let constant = AbstractOperation::IntegerConstant {
        psi_operation: OperationId::new(6).unwrap(),
        result: argument,
        scalar_type: i32_type,
        value: semantic_vocabulary::IntegerValue::Signed(9),
    };
    let produce = AbstractOperation::BoundaryCall {
        psi_operation: OperationId::new(7).unwrap(),
        result: AbstractBoundaryResult::Scalar(AbstractResult {
            value: relayed,
            scalar_type: i32_type,
        }),
        boundary: BoundaryMachineId::new(2).unwrap(),
        arguments: vec![argument],
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let consume = AbstractOperation::BoundaryCall {
        psi_operation: OperationId::new(9).unwrap(),
        result: AbstractBoundaryResult::Unit,
        boundary: BoundaryMachineId::new(1).unwrap(),
        arguments: vec![relayed],
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let successor = |identity, target| AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(target),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let branch = AbstractOperation::Conditional {
        condition: ValueId::new(100).unwrap(),
        when_true: successor(10, 2),
        when_false: successor(11, 3),
    };
    let finish = |identity| AbstractOperation::ReturnUnit {
        psi_edge: edge(identity),
        cleanup_actions: Vec::new(),
    };
    let (entry, true_arm, false_arm) = match placement {
        ResultPlacement::EntryToTrueArm => (
            vec![constant, produce, branch],
            vec![consume, finish(12)],
            vec![finish(13)],
        ),
        ResultPlacement::EntryToFalseArm => (
            vec![constant, produce, branch],
            vec![finish(12)],
            vec![consume, finish(13)],
        ),
        ResultPlacement::SiblingBranches => (
            vec![constant, branch],
            vec![produce, finish(12)],
            vec![consume, finish(13)],
        ),
    };
    let function = &mut source.functions[0];
    assert_eq!(function.attachment, None, "the caller is receiverless");
    function.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type: ScalarType::Boolean,
    }];
    function.block_entries = [(1, 0), (2, entry.len()), (3, entry.len() + true_arm.len())]
        .map(|(identity, offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: block(identity),
            parameters: Vec::new(),
            operation_offset: offset,
        })
        .to_vec();
    function.operations = [entry, true_arm, false_arm].concat();
    source
}

fn lower_relay(
    source: &AbstractOperationPlan,
    native: NativeTarget,
) -> Result<TargetOperationPlan, abstract_operations_to_target_operations::LoweringError> {
    let shape = calling_conventions::ValueShape::integer(4, 4);
    let execution = Execution { plan_report: 0xA1 };
    let settlements =
        [(1, None), (2, Some(shape))].map(|(boundary, result)| AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(boundary).unwrap(),
            execution: AdmittedBoundaryExecution::Provider(&execution),
            realization: BoundarySettlementRealization::NormalizedForeignCall(binding(
                native,
                calling_conventions::CallSignature {
                    parameters: vec![shape],
                    result,
                },
            )),
        });
    abstract_operations_to_target_operations::lower_to_target_operations(
        source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &settlements,
            installation: None,
            native_callbacks: &[],
        },
    )
}

/// A normalized foreign scalar result is an ordinary SSA value. A free caller
/// keeps it without an attachment, and a later block the producer dominates
/// consumes it as a durable home through legalization, selection and both
/// replays on every hosted ABI.
#[test]
fn free_caller_foreign_result_reaches_a_dominated_block() {
    let source = dominated_result_fixture(ResultPlacement::EntryToTrueArm);
    let unit = seed(&source);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let target = lower_relay(&source, native).expect("free caller lowers");
        abstract_operations_to_target_operations::validate_abstract_to_target_translation(
            &source, native, &target,
        )
        .expect("free caller replays");
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let caller = &legal.plan().scalar_functions[0];
        assert_eq!(caller.attachment, None);
        let calls = caller
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.kind {
                legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(
                    call,
                ) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [relay, leaf] = calls.as_slice() else {
            panic!("two normalized foreign calls")
        };
        let home = *relay.result.scalar_home().expect("relay result home");
        assert_eq!(home.source_value, ValueId::new(8).unwrap());
        assert_eq!(
            leaf.scalar_arguments[0].source,
            target_operations::TargetUnitScalarArgumentSource::Home(home)
        );
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
    }
}

/// Legalization rejoins a later block's foreign argument only to a producer
/// that dominates it. Stage 02 refuses to lower the sibling arrangement at
/// all, so this control transplants the true-arm lowering's own rows. Moving
/// only the consumer to the false arm keeps the entry producer dominating it
/// and legalizes; also moving the producer into the true arm leaves the
/// consumer's source undominated, and legalization's independent replay
/// refuses it.
#[test]
fn foreign_result_does_not_reach_a_sibling_block() {
    let native = NativeTarget::linux_x64();
    let sibling = dominated_result_fixture(ResultPlacement::SiblingBranches);
    assert!(
        lower_relay(&sibling, native).is_err(),
        "stage 02 lowered an undominated foreign argument"
    );
    let lowered = lower_relay(
        &dominated_result_fixture(ResultPlacement::EntryToTrueArm),
        native,
    )
    .unwrap();
    for (source, move_producer) in [
        (
            dominated_result_fixture(ResultPlacement::EntryToFalseArm),
            false,
        ),
        (sibling, true),
    ] {
        let mut target = lowered.clone();
        let blocks = &mut target.functions[0].graph.blocks;
        let leaf = blocks[1].operations.pop().expect("leaf row");
        if move_producer {
            let relay = blocks[0].operations.pop().expect("relay row");
            assert!(matches!(
                relay,
                TargetUnitOperation::NormalizedForeignCall {
                    result: target_operations::TargetBoundaryResult::Scalar(_),
                    ..
                }
            ));
            blocks[1].operations.push(relay);
        }
        blocks[2].operations.push(leaf);
        let legalized = legalize_target_operations(&target, &source, &seed(&source));
        assert_eq!(
            legalized.is_err(),
            move_producer,
            "producer moved: {move_producer}"
        );
    }
}
