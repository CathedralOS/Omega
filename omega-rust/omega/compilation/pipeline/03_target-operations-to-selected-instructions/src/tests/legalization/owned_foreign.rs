//! Foreign by-value transport rejoins source ownership and evaluated ABI
//! classification independently, including integer, floating and private-copy
//! placements. The native fixture exercises the source-to-libc leg.
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use target::{NativeTarget, TargetProfile};
use target_operations::{BoundarySettlementRealization, NormalizedForeignCallBinding};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

struct Execution;
impl installation_evidence::ProviderExecutionEvidence for Execution {
    fn requirement_identity(&self) -> &str {
        "Foreign::consume"
    }
    fn provider_plan_report_identity(&self) -> u64 {
        0xA1
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

fn binding(native: NativeTarget, signature: CallSignature) -> NormalizedForeignCallBinding {
    let (candidate, profile) = match (native.architecture, native.object_format) {
        (target::Architecture::X86_64, target::ObjectFormat::Coff) => (
            target::ForeignLocatorCandidate::PeByName {
                library: b"test.dll".to_vec(),
                export: b"consume".to_vec(),
            },
            TargetProfile::WindowsX64,
        ),
        (target::Architecture::Aarch64, target::ObjectFormat::MachO) => (
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"libtest.dylib".to_vec(),
                symbol: b"_consume".to_vec(),
            },
            TargetProfile::MacosArm64,
        ),
        (architecture, _) => (
            target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libtest.so".to_vec(),
                symbol: b"consume".to_vec(),
                version: b"TEST_1".to_vec(),
            },
            if architecture == target::Architecture::Aarch64 {
                TargetProfile::LinuxArm64
            } else {
                TargetProfile::LinuxX64
            },
        ),
    };
    let commitment = task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
    let contribution = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0xA1,
            provider_plan_commitment: commitment,
            requirement_identity: "Foreign::consume".into(),
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0xA2,
            )
            .unwrap(),
            bytes: 128,
            alignment: 16,
        },
        0xA1,
        commitment,
        "Foreign::consume",
    )
    .unwrap();
    NormalizedForeignCallBinding {
        boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(native),
            &signature,
        )
        .unwrap()
        .plan()
        .clone(),
        mechanism: target_operations::NormalizedForeignCallMechanism::Import {
            locator: target::normalize_foreign_locator(candidate, profile).unwrap(),
            same_stack_contribution: contribution,
        },
    }
}

fn fixture(fields: &[ScalarType], scalar_prefix: usize) -> AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    source
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "Payload".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: fields
                    .iter()
                    .enumerate()
                    .map(
                        |(position, scalar)| terminal_psi::StructuralFieldDeclaration {
                            id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                            identity: format!("field_{position}"),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Scalar(*scalar),
                        },
                    )
                    .collect(),
            },
        });
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    source.functions[0]
        .structural_parameters
        .push(parameter.clone());
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: vec![],
            scalar_requires: vec![],
            fixed_service_reach: vec![],
            id: boundary,
            identity: "Foreign::consume".into(),
            attachment: None,
            parameter_order: std::iter::repeat_n(
                terminal_psi::BoundaryParameterKind::Scalar,
                scalar_prefix,
            )
            .chain([
                terminal_psi::BoundaryParameterKind::Structural,
                terminal_psi::BoundaryParameterKind::Scalar,
            ])
            .collect(),
            scalar_parameters: vec![scalar_type; scalar_prefix + 1],
            structural_parameters: vec![parameter],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![],
            program_local_root_introductions: vec![],
            content_guarantees: vec![],
            published_service_ceiling: vec![],
            crash_routes: vec![],
        });
    let mut arguments = Vec::new();
    for position in 0..=scalar_prefix {
        let value = ValueId::new(position as u64 + 10).unwrap();
        arguments.push(value);
        source.functions[0].operations.insert(
            position,
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(position as u64 + 10).unwrap(),
                result: value,
                scalar_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(position as u128 + 1),
            },
        );
    }
    source.functions[0].operations.insert(
        scalar_prefix + 1,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(100).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary,
            arguments,
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place,
                path: vec![],
                access: StructuralAccess::Owned,
            }],
            completion_claim_sources: vec![],
            completion_receipts: vec![],
        },
    );
    source
}

#[test]
fn owned_foreign_arguments_replay_payload_fragments_and_private_copies() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let float = ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64);
    // Each row exercises a distinct ABI obligation, not a source arrangement:
    // one word, multiple integer words, private indirect/stack copy, HFA,
    // mixed SysV banks, and register exhaustion with a following scalar.
    for (fields, scalar_prefix) in [
        (vec![integer], 0),
        (vec![integer; 2], 0),
        (vec![integer; 3], 0),
        (vec![float; 4], 0),
        (vec![integer, float], 0),
        (vec![integer; 2], 8),
    ] {
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let source = fixture(&fields, scalar_prefix);
            let shape = abstract_operations_to_target_operations::classified_boundary_shape(
                StructuralTypeId::new(1).unwrap(),
                &source.structural_types,
                CallingPolicy::native_for_target(native),
            )
            .unwrap();
            let mut parameters = vec![ValueShape::integer(4, 4); scalar_prefix];
            parameters.extend([shape, ValueShape::integer(4, 4)]);
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest {
                    target: native,
                    settlements: &[AdmittedBoundarySettlement {
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        execution: AdmittedBoundaryExecution::Provider(&Execution),
                        realization: BoundarySettlementRealization::NormalizedForeignCall(binding(
                            native,
                            CallSignature {
                                parameters,
                                result: None,
                            },
                        )),
                    }],
                    installation: None,
                    native_callbacks: &[],
                },
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap_or_else(
                |error| panic!("{native:?} {fields:?} prefix={scalar_prefix}: {error:?}"),
            );
            crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
                .unwrap();
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
                panic!("{native:?} {fields:?} prefix={scalar_prefix}: {error:?}")
            });
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            for mutation in 0..4 {
                let mut changed = legal.plan().clone();
                let call = changed.functions[0].blocks.iter_mut().flat_map(|block| &mut block.instructions).find_map(|instruction| {
                    if let legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &mut instruction.kind { Some(call) } else { None }
                }).unwrap();
                let argument = &mut call.structural_arguments[0];
                match mutation {
                    0 => argument.access = StructuralAccess::SharedBorrow,
                    1 => argument.source_byte_offset = 1,
                    2 => {
                        argument.destination.shape.class =
                            calling_conventions::ValueClass::BorrowedReference
                    }
                    _ => argument.place = PlaceId::new(99).unwrap(),
                }
                assert!(
                    crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                    "accepted owned argument corruption {mutation}"
                );
            }
            let mut changed = selected.plan().clone();
            let function = &mut changed.functions[0];
            let call_id = function.normalized_foreign_calls[0].instruction;
            let instruction = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| instruction.id == call_id)
                .unwrap();
            if let Some(operand) = instruction.operands.first_mut() {
                operand.fixed_view = None;
            } else {
                instruction.clobbers.clear();
            }
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
