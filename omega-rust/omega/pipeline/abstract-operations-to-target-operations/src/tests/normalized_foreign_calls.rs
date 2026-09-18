//! Evaluated normalized foreign calls embed a boundary entry plan and
//! projected borrowed arguments; upstream replay re-derives every retained
//! coordinate from the boundary declaration and the caller's own homes.
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractOperation, AbstractOperationPlan, BlockId,
    BoundaryMachineId, CallSignature, CallingPolicy, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, NativeTarget, OperationId, PlaceId, ScalarType, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, ValueId, identity,
};
use crate::{AdmittedBoundaryExecution, AdmittedBoundarySettlement};
use abstract_operations::{AbstractBoundaryResult, AbstractFunctionResult, AbstractResult};
use calling_conventions::ValueShape;
use target_operations::{
    BoundarySettlementRealization, NormalizedForeignCallBinding, TargetOperationPlan,
    TargetUnitOperation,
};
use terminal_psi::StructuralPathSegment;

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

fn binding(native: NativeTarget, signature: CallSignature) -> NormalizedForeignCallBinding {
    let locator = locator_for(native);
    let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(native),
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
        locator,
        boundary_entry_plan,
        same_stack_contribution,
    }
}

fn declaration(
    scalar_parameters: Vec<ScalarType>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    result: terminal_psi::BoundaryMachineResult,
) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        id: BoundaryMachineId::new(1).unwrap(),
        identity: REQUIREMENT.into(),
        attachment: None,
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

fn empty_plan(machine: MachineId) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                structural_parameters: Vec::new(),
                block: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

/// One scalar-lane leaf: `Foreign::leaf(i32) -> Unit` over an authored
/// constant, then `Foreign::relay(i32) -> i32` producing a scalar-result home
/// that a second leaf call consumes as a `Home` argument.
fn scalar_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let value = ValueId::new(5).unwrap();
    let relayed = ValueId::new(8).unwrap();
    let mut source = empty_plan(MachineId::new(1).unwrap());
    source.boundary_machines.push(declaration(
        vec![i32_type],
        Vec::new(),
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(2).unwrap(),
            ..declaration(
                vec![i32_type],
                Vec::new(),
                terminal_psi::BoundaryMachineResult::Scalar(i32_type),
            )
        });
    source.functions[0].attachment = Some(StructuralTypeId::new(9).unwrap());
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: StructuralTypeId::new(9).unwrap(),
            identity: "probe::Attached".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(6).unwrap(),
            result: value,
            scalar_type: i32_type,
            value: IntegerValue::Signed(9),
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
    source.functions[0].operations.insert(
        2,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(8).unwrap(),
            result: AbstractBoundaryResult::Scalar(AbstractResult {
                value: relayed,
                scalar_type: i32_type,
            }),
            boundary: BoundaryMachineId::new(2).unwrap(),
            arguments: vec![value],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    source.functions[0].operations.insert(
        3,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(9).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: vec![relayed],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (source, Execution { plan_report: 0xA1 })
}

/// The flat-record lane: `main(&mut self)` calls `shift(&self.p) -> i32`
/// through a shared-borrowed record projection.
fn flat_record_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let main_type = StructuralTypeId::new(1).unwrap();
    let point_type = StructuralTypeId::new(2).unwrap();
    let self_place = PlaceId::new(1).unwrap();
    let mut source = empty_plan(MachineId::new(1).unwrap());
    source.functions[0].attachment = Some(main_type);
    source.functions[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: self_place,
            position: 0,
            is_self: true,
            structural_type: main_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source.structural_types.make_mut().extend([
        StructuralTypeDeclaration {
            id: main_type,
            identity: "probe::Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "p".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(point_type),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: point_type,
            identity: "probe::Point".into(),
            shape: StructuralTypeShape::Record {
                fields: [
                    ("x", StructuralFieldId::new(2).unwrap()),
                    ("y", StructuralFieldId::new(3).unwrap()),
                ]
                .map(|(identity, id)| StructuralFieldDeclaration {
                    id,
                    identity: identity.into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(i32_type),
                })
                .to_vec(),
            },
        },
    ]);
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![StructuralParameterDeclaration {
            place: PlaceId::new(9).unwrap(),
            position: 0,
            is_self: false,
            structural_type: point_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        terminal_psi::BoundaryMachineResult::Scalar(i32_type),
    ));
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
                path: vec![StructuralPathSegment::Field("p".to_owned())],
                access: StructuralAccess::SharedBorrow,
            }],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (source, Execution { plan_report: 0xA1 })
}

fn lower(
    source: &AbstractOperationPlan,
    native: NativeTarget,
    execution: &dyn installation_evidence::ProviderExecutionEvidence,
    bindings: &[(u32, NormalizedForeignCallBinding)],
) -> TargetOperationPlan {
    let settlements = bindings
        .iter()
        .map(|(boundary, binding)| AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(u64::from(*boundary)).unwrap(),
            execution: AdmittedBoundaryExecution::Provider(execution),
            realization: BoundarySettlementRealization::NormalizedForeignCall(binding.clone()),
        })
        .collect::<Vec<_>>();
    crate::lower_to_target_operations(
        source,
        crate::TargetLoweringRequest {
            target: native,
            settlements: &settlements,
            installation: None,
            ieee_float_fma: &[],
        },
    )
    .unwrap()
}

fn normalized_foreign_mut(
    plan: &mut TargetOperationPlan,
    psi_operation: u32,
) -> &mut TargetUnitOperation {
    plan.functions[0]
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| block.operations.iter_mut())
        .find(|operation| {
            matches!(operation, TargetUnitOperation::NormalizedForeignCall { psi_operation: id, .. }
                if *id == OperationId::new(u64::from(psi_operation)).unwrap())
        })
        .expect("normalized foreign row")
}

#[test]
fn normalized_foreign_rows_replay_on_both_linux_targets() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, execution) = scalar_fixture();
        let shape = ValueShape::integer(4, 4);
        let target = lower(
            &source,
            native,
            &execution,
            &[
                (
                    1,
                    binding(
                        native,
                        CallSignature {
                            parameters: vec![shape],
                            result: None,
                        },
                    ),
                ),
                (
                    2,
                    binding(
                        native,
                        CallSignature {
                            parameters: vec![shape],
                            result: Some(shape),
                        },
                    ),
                ),
            ],
        );
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("scalar normalized foreign rows replay");

        let (source, execution) = flat_record_fixture();
        let pointer = u16::try_from(native.pointer_size).unwrap();
        let target = lower(
            &source,
            native,
            &execution,
            &[(
                1,
                binding(
                    native,
                    CallSignature {
                        parameters: vec![ValueShape::integer(
                            pointer,
                            u16::try_from(native.pointer_alignment).unwrap(),
                        )],
                        result: Some(ValueShape::integer(4, 4)),
                    },
                ),
            )],
        );
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("projected normalized foreign row replays");
    }
}

#[test]
fn replay_rejects_substituted_scalar_row_coordinates() {
    let native = NativeTarget::linux_x64();
    let (source, execution) = scalar_fixture();
    let shape = ValueShape::integer(4, 4);
    let target = lower(
        &source,
        native,
        &execution,
        &[
            (
                1,
                binding(
                    native,
                    CallSignature {
                        parameters: vec![shape],
                        result: None,
                    },
                ),
            ),
            (
                2,
                binding(
                    native,
                    CallSignature {
                        parameters: vec![shape],
                        result: Some(shape),
                    },
                ),
            ),
        ],
    );
    let different_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone();
    let foreign_plan_execution =
        target_operations::ProviderExecutionBinding::from_execution_record(
            target_operations::ProviderPlanReportIdentity::new(0xA9).unwrap(),
            0xB1,
            0xB2,
            0xB3,
            0xB4,
        )
        .unwrap();
    for mutation in 0..8 {
        let mut changed = target.clone();
        let TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            result_home,
            ..
        } = normalized_foreign_mut(&mut changed, 7)
        else {
            panic!("normalized foreign row")
        };
        match mutation {
            0 => *psi_operation = OperationId::new(99).unwrap(),
            1 => *boundary = BoundaryMachineId::new(2).unwrap(),
            2 => *provider_execution = foreign_plan_execution,
            3 => binding.boundary_entry_plan = different_plan.clone(),
            4 => scalar_arguments[0].parameter_index = 9,
            5 => {
                scalar_arguments[0].source =
                    target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: OperationId::new(7).unwrap(),
                        source_value: ValueId::new(5).unwrap(),
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        value: IntegerValue::Signed(4),
                    }
            }
            6 => scalar_arguments[0].placement.shape = ValueShape::integer(8, 8),
            _ => {
                *result_home = Some(target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: OperationId::new(7).unwrap(),
                    source_value: ValueId::new(9).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    ),
                    shape,
                })
            }
        }
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
            "accepted scalar mutation {mutation}"
        );
    }
    // The second leaf call consumes the relay call's scalar result through a
    // `Home` source; each substituted home coordinate must fail the replay.
    for mutation in 0..3 {
        let mut changed = target.clone();
        let TargetUnitOperation::NormalizedForeignCall {
            scalar_arguments, ..
        } = normalized_foreign_mut(&mut changed, 9)
        else {
            panic!("normalized foreign row")
        };
        let target_operations::TargetUnitScalarArgumentSource::Home(home) =
            &mut scalar_arguments[0].source
        else {
            panic!("home source")
        };
        match mutation {
            0 => home.source_value = ValueId::new(6).unwrap(),
            1 => home.defining_operation = OperationId::new(9).unwrap(),
            _ => home.defining_operation = OperationId::new(99).unwrap(),
        }
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
            "accepted home-source mutation {mutation}"
        );
    }
}

#[test]
fn replay_rejects_projected_argument_identity_substitutions() {
    let native = NativeTarget::linux_x64();
    let (source, execution) = flat_record_fixture();
    let target = lower(
        &source,
        native,
        &execution,
        &[(
            1,
            binding(
                native,
                CallSignature {
                    parameters: vec![ValueShape::integer(8, 8)],
                    result: Some(ValueShape::integer(4, 4)),
                },
            ),
        )],
    );
    for mutation in 0..12 {
        let mut changed = target.clone();
        let TargetUnitOperation::NormalizedForeignCall {
            structural_arguments,
            result_home,
            ..
        } = normalized_foreign_mut(&mut changed, 7)
        else {
            panic!("normalized foreign row")
        };
        match mutation {
            0 => structural_arguments[0].path.clear(),
            1 => structural_arguments[0].source_byte_offset = 4,
            2 => structural_arguments[0].structural_type = StructuralTypeId::new(1).unwrap(),
            3 => structural_arguments[0].root_structural_type = StructuralTypeId::new(2).unwrap(),
            4 => structural_arguments[0].access = StructuralAccess::MutableBorrow,
            5 => {
                structural_arguments[0].destination.locations =
                    vec![calling_conventions::ValueLocation::Register {
                        register: calling_conventions::MachineRegister::X86Rax,
                        value_byte_offset: 0,
                        byte_size: 4,
                    }]
            }
            6 => {
                structural_arguments[0].source =
                    target_operations::TargetStructuralArgumentSource::Placement(
                        calling_conventions::ValuePlacement {
                            shape: ValueShape::integer(8, 8),
                            locations: vec![calling_conventions::ValueLocation::Register {
                                register: calling_conventions::MachineRegister::X86Rax,
                                value_byte_offset: 0,
                                byte_size: 8,
                            }],
                        },
                    )
            }
            7 => result_home.as_mut().unwrap().source_value = ValueId::new(9).unwrap(),
            8 => result_home.as_mut().unwrap().shape = ValueShape::integer(8, 8),
            9 => structural_arguments[0].place = PlaceId::new(9).unwrap(),
            10 => structural_arguments[0].shape = ValueShape::integer(4, 4),
            _ => {
                structural_arguments[0].source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(7).unwrap(),
                    }
            }
        }
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
            "accepted projected mutation {mutation}"
        );
    }
}
