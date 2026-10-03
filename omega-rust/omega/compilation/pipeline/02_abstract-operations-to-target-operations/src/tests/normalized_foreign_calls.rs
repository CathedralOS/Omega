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
use abstract_operations::{
    AbstractBoundaryResult, AbstractFunctionResult, AbstractResult, AbstractStructuralCallResult,
};
use calling_conventions::ValueShape;
use target_operations::{
    BoundarySettlementRealization, NormalizedForeignCallBinding, TargetOperationPlan,
    TargetUnitOperation,
};
use terminal_psi::StructuralPathSegment;

const REQUIREMENT: &str = "Foreign::leaf";

/// Kernel entries use the same transport and result sequencing as imports.
/// An extra call consumes the first call's result; the source has no Clock
/// spelling, and neither a fixed arity nor an immediate-only path can pass it.
#[test]
fn system_calls_share_scalar_result_transport_and_independent_replay() {
    use target_operations::{NormalizedForeignCallMechanism, NormalizedSystemCallBinding};
    let (source, execution) = scalar_fixture();
    for profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ] {
        let native = profile.native_target();
        let bindings = [false, true]
            .into_iter()
            .enumerate()
            .map(|(index, returns)| {
                let signature = CallSignature {
                    parameters: vec![ValueShape::integer(4, 4)],
                    result: returns.then_some(ValueShape::integer(4, 4)),
                };
                (
                    index as u32 + 1,
                    NormalizedForeignCallBinding {
                        boundary_entry_plan:
                            calling_conventions::evaluate_ordinary_boundary_entry_plan(
                                CallingPolicy::native_syscall_for_target(native).unwrap(),
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
                    },
                )
            })
            .collect::<Vec<_>>();
        let lowered = lower(&source, native, &execution, &bindings);
        crate::validate_abstract_to_target_translation(&source, native, &lowered)
            .expect("kernel calls replay through shared transport");
        let TargetUnitOperation::NormalizedForeignCall {
            scalar_arguments, ..
        } = normalized_foreign_ref(&lowered, 9)
        else {
            panic!("expected consuming call");
        };
        assert!(matches!(
            scalar_arguments[0].source,
            target_operations::TargetUnitScalarArgumentSource::Home(_)
        ));
        for corruption in 0..4 {
            let mut changed = lowered.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                binding,
                scalar_arguments,
                ..
            } = normalized_foreign_mut(&mut changed, 8)
            else {
                panic!("expected producing call");
            };
            match corruption {
                0 => {
                    binding.boundary_entry_plan.call.ordinary_clobbers =
                        calling_conventions::RegisterSet::new([])
                }
                1 => {
                    binding.boundary_entry_plan.call.entry_control =
                        calling_conventions::EntryControl::CallReturn
                }
                2 => scalar_arguments[0].parameter_index = 1,
                _ => {
                    let NormalizedForeignCallMechanism::SystemCall(kernel) = &mut binding.mechanism
                    else {
                        panic!("kernel");
                    };
                    kernel.requirement_identity = "Other::leaf".into();
                }
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "corruption {corruption}"
            );
        }
    }
}

#[derive(Debug)]
struct Execution {
    plan_report: u64,
    requirement: String,
}

impl installation_evidence::ProviderExecutionEvidence for Execution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
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
    binding_for_requirement(REQUIREMENT, native, signature)
}

fn binding_for_requirement(
    requirement: &str,
    native: NativeTarget,
    signature: CallSignature,
) -> NormalizedForeignCallBinding {
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
            requirement_identity: requirement.to_owned(),
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0xA2,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        provider_plan_report_identity,
        provider_plan_commitment,
        requirement,
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
    structural_parameters: Vec<StructuralParameterDeclaration>,
    result: terminal_psi::BoundaryMachineResult,
) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        parameter_order: std::iter::repeat_n(
            terminal_psi::BoundaryParameterKind::Scalar,
            scalar_parameters.len(),
        )
        .chain(std::iter::repeat_n(
            terminal_psi::BoundaryParameterKind::Structural,
            structural_parameters.len(),
        ))
        .collect(),
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
/// that a second leaf call consumes as a `Home` argument. The caller is free:
/// a foreign result is an ordinary SSA home, not attachment storage.
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
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            id: BoundaryMachineId::new(2).unwrap(),
            ..declaration(
                vec![i32_type],
                Vec::new(),
                terminal_psi::BoundaryMachineResult::Scalar(i32_type),
            )
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
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
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
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
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
            native_callbacks: &[],
        },
    )
    .unwrap()
}

fn normalized_foreign_ref(plan: &TargetOperationPlan, psi_operation: u32) -> &TargetUnitOperation {
    plan.functions[0]
        .graph
        .blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .find(|operation| {
            matches!(operation, TargetUnitOperation::NormalizedForeignCall { psi_operation: id, .. }
                if *id == OperationId::new(u64::from(psi_operation)).unwrap())
        })
        .expect("normalized foreign row")
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
fn authored_mixed_foreign_call_survives_canonical_artifact_and_target_replay() {
    let source = super::structural_borrows::source_plan(
        r#"
        data Point { x: i32; y: i32; }
        boundary trait Foreign {
            machine leaf(left: i64, point: &Point, right: i32) -> i32;
        }
        data Main { point: Point; }
        machine Main::run(main: &Main) reaches Foreign {
            let observed: i32 = Foreign::leaf(11i64, &main.point, 12i32);
        }
    "#,
    );
    assert_eq!(
        source.boundary_machines[0].parameter_order,
        [
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Structural,
            terminal_psi::BoundaryParameterKind::Scalar,
        ]
    );
    let execution = Execution {
        plan_report: 0xA1,
        requirement: source.boundary_machines[0].identity.clone(),
    };
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let target = lower(
            &source,
            native,
            &execution,
            &[(
                1,
                binding_for_requirement(
                    &execution.requirement,
                    native,
                    CallSignature {
                        parameters: vec![
                            ValueShape::integer(8, 8),
                            ValueShape::integer(8, 8),
                            ValueShape::integer(4, 4),
                        ],
                        result: Some(ValueShape::integer(4, 4)),
                    },
                ),
            )],
        );
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("authored mixed call replays");
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
            result: result_home,
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
                *result_home = target_operations::TargetBoundaryResult::Scalar(
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
            result: target_operations::TargetBoundaryResult::Scalar(result_home),
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
            7 => result_home.source_value = ValueId::new(9).unwrap(),
            8 => result_home.shape = ValueShape::integer(8, 8),
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

/// One registrar leaf: `Foreign::registrar(i32, <callback>) -> Unit` — the
/// callback occupies a private native-only slot at authored ordinal 1.
fn callback_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let value = ValueId::new(5).unwrap();
    let mut source = empty_plan(MachineId::new(1).unwrap());
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
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
}

/// The registrar binding whose entry plan carries the private
/// materialization row beside the declared scalar parameter, joined to the
/// binder/demand context the admission retains.
fn materialized_binding(
    native: NativeTarget,
    scalar: ValueShape,
    pointer: ValueShape,
) -> (
    NormalizedForeignCallBinding,
    calling_conventions::CallbackMaterializationContext,
    calling_conventions::NativeParameterId,
) {
    let mut binding = binding(
        native,
        CallSignature {
            parameters: vec![scalar, pointer],
            result: None,
        },
    );
    let binder = calling_conventions::StaticMachineBinderId::new(81).unwrap();
    let parameter = calling_conventions::NativeParameterId::new(82).unwrap();
    let requirement = calling_conventions::CallbackRequirementId::new(83).unwrap();
    let destination = calling_conventions::NativePlace::Parameter(parameter);
    binding.boundary_entry_plan.call.callback_materializations =
        vec![calling_conventions::CallbackMaterialization {
            binder,
            destination: destination.clone(),
        }];
    let context = calling_conventions::CallbackMaterializationContext {
        binders: vec![calling_conventions::CallbackBinderRequirement {
            binder,
            requirement,
        }],
        demands: vec![calling_conventions::NativeCallbackDemand {
            destination,
            requirement,
        }],
    };
    (binding, context, parameter)
}

#[test]
fn registrar_callback_slot_replays_from_the_retained_roster() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, execution) = callback_fixture();
        let operation = OperationId::new(7).unwrap();
        let scalar = ValueShape::integer(4, 4);
        let pointer = ValueShape::integer(
            u32::try_from(native.pointer_size).unwrap(),
            u16::try_from(native.pointer_alignment).unwrap(),
        );
        let (binding, context, parameter) = materialized_binding(native, scalar, pointer);
        let continuation = function_identity::StateKey {
            machine: symbols::SymbolHandle::from_parts(1, 1),
            state: symbols::SymbolHandle::from_parts(2, 1),
            segment_index: 0,
        };
        let admission = crate::AdmittedNativeCallbackArgument {
            terminal_operation: operation,
            placement_index: 0,
            callback_function: function_identity::MachineFunctionIdentity::callback_thunk(
                continuation,
                0,
            )
            .unwrap(),
            destination: calling_conventions::NativePlace::Parameter(parameter),
            field_byte_offset: None,
            application: calling_conventions::NativeParameterApplication {
                parameter,
                native_ordinal: 1,
                shape: pointer,
                placement: binding.boundary_entry_plan.call.parameters[1].clone(),
            },
            registrar_boundary_entry_plan: binding.boundary_entry_plan.clone(),
            registrar_context: context,
            registrar_application_commitment: [0x66; 32],
        };
        let settlements = [crate::AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: crate::AdmittedBoundaryExecution::Provider(&execution),
            realization: BoundarySettlementRealization::NormalizedForeignCall(binding),
        }];
        let target = crate::lower_to_target_operations(
            &source,
            crate::TargetLoweringRequest {
                target: native,
                settlements: &settlements,
                installation: None,
                native_callbacks: &[admission],
            },
        )
        .expect("admitted callback lowers");
        // The plan retains the roster entry joined to the consuming row, and
        // the row's scalar argument shifts past the callback's native-only
        // ordinal into its own plan position.
        assert_eq!(target.native_callback_arguments.len(), 1);
        let TargetUnitOperation::NormalizedForeignCall {
            scalar_arguments, ..
        } = normalized_foreign_ref(&target, 7)
        else {
            panic!("normalized foreign row")
        };
        assert_eq!(scalar_arguments[0].parameter_index, 0);
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("registrar callback row replays");

        // A cleared or dangling roster leaves the materialized plan without
        // its custody carrier and fails closed; a duplicated operation key
        // and a substituted registrar plan reject identically.
        let mut cleared = target.clone();
        cleared.native_callback_arguments.clear();
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &cleared).is_err(),
            "accepted missing roster row"
        );
        let mut dangling = target.clone();
        dangling.native_callback_arguments[0].terminal_operation = OperationId::new(42).unwrap();
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &dangling).is_err(),
            "accepted dangling roster row"
        );
        let mut duplicated = target.clone();
        duplicated
            .native_callback_arguments
            .push(duplicated.native_callback_arguments[0].clone());
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &duplicated).is_err(),
            "accepted duplicated roster row"
        );
        let mut substituted = target.clone();
        substituted.native_callback_arguments[0]
            .registrar_boundary_entry_plan
            .call
            .parameters
            .pop();
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &substituted).is_err(),
            "accepted substituted registrar plan"
        );
        let mut reordered = target.clone();
        let TargetUnitOperation::NormalizedForeignCall {
            scalar_arguments, ..
        } = normalized_foreign_mut(&mut reordered, 7)
        else {
            panic!("normalized foreign row")
        };
        scalar_arguments[0].parameter_index = 1;
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &reordered).is_err(),
            "accepted scalar argument at the callback's private ordinal"
        );
    }
}

#[test]
fn mixed_registrar_callback_preserves_authored_formals_around_its_private_slot() {
    use terminal_psi::BoundaryParameterKind::{Scalar, Structural};

    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for callback_ordinal in [0usize, 1] {
            let (mut source, execution) = flat_record_fixture();
            let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
            let value = ValueId::new(1).unwrap();
            source.boundary_machines[0].scalar_parameters = vec![ScalarType::Integer(integer)];
            source.boundary_machines[0].parameter_order = vec![Scalar, Structural];
            let AbstractOperation::BoundaryCall { arguments, .. } =
                &mut source.functions[0].operations[0]
            else {
                panic!("boundary call")
            };
            *arguments = vec![value];
            source.functions[0].operations.insert(
                0,
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: value,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Signed(11),
                },
            );

            // Scalar, borrowed referent, and callback deliberately share a
            // native shape. Only the retained identities distinguish them.
            let pointer = ValueShape::integer(8, 8);
            let mut foreign = binding(
                native,
                CallSignature {
                    parameters: vec![pointer; 3],
                    result: Some(ValueShape::integer(4, 4)),
                },
            );
            let binder = calling_conventions::StaticMachineBinderId::new(81).unwrap();
            let parameter = calling_conventions::NativeParameterId::new(82).unwrap();
            let requirement = calling_conventions::CallbackRequirementId::new(83).unwrap();
            let destination = calling_conventions::NativePlace::Parameter(parameter);
            foreign.boundary_entry_plan.call.callback_materializations =
                vec![calling_conventions::CallbackMaterialization {
                    binder,
                    destination: destination.clone(),
                }];
            let context = calling_conventions::CallbackMaterializationContext {
                binders: vec![calling_conventions::CallbackBinderRequirement {
                    binder,
                    requirement,
                }],
                demands: vec![calling_conventions::NativeCallbackDemand {
                    destination: destination.clone(),
                    requirement,
                }],
            };
            let continuation = function_identity::StateKey {
                machine: symbols::SymbolHandle::from_parts(1, 1),
                state: symbols::SymbolHandle::from_parts(2, 1),
                segment_index: 0,
            };
            let admission = crate::AdmittedNativeCallbackArgument {
                terminal_operation: OperationId::new(7).unwrap(),
                placement_index: 0,
                callback_function: function_identity::MachineFunctionIdentity::callback_thunk(
                    continuation,
                    0,
                )
                .unwrap(),
                destination: destination.clone(),
                field_byte_offset: None,
                application: calling_conventions::NativeParameterApplication {
                    parameter,
                    native_ordinal: u32::try_from(callback_ordinal).unwrap(),
                    shape: pointer,
                    placement: foreign.boundary_entry_plan.call.parameters[callback_ordinal]
                        .clone(),
                },
                registrar_boundary_entry_plan: foreign.boundary_entry_plan.clone(),
                registrar_context: context,
                registrar_application_commitment: [0x66; 32],
            };
            let settlements = [AdmittedBoundarySettlement {
                boundary: BoundaryMachineId::new(1).unwrap(),
                execution: AdmittedBoundaryExecution::Provider(&execution),
                realization: BoundarySettlementRealization::NormalizedForeignCall(foreign.clone()),
            }];
            let target = crate::lower_to_target_operations(
                &source,
                crate::TargetLoweringRequest {
                    target: native,
                    settlements: &settlements,
                    installation: None,
                    native_callbacks: &[admission],
                },
            )
            .expect("mixed registrar lowers");
            crate::validate_abstract_to_target_translation(&source, native, &target)
                .expect("mixed registrar independently replays");
            let scalar_position = usize::from(callback_ordinal == 0);
            let structural_position = 2;
            let TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                structural_arguments,
                ..
            } = normalized_foreign_ref(&target, 7)
            else {
                panic!("normalized foreign row")
            };
            assert_eq!(
                scalar_arguments[0].parameter_index as usize,
                scalar_position
            );
            assert_eq!(
                scalar_arguments[0].placement,
                foreign.boundary_entry_plan.call.parameters[scalar_position]
            );
            assert_eq!(
                structural_arguments[0].destination,
                foreign.boundary_entry_plan.call.parameters[structural_position]
            );

            let mut callback_collision = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                structural_arguments,
                ..
            } = normalized_foreign_mut(&mut callback_collision, 7)
            else {
                panic!("normalized foreign row")
            };
            structural_arguments[0].destination =
                foreign.boundary_entry_plan.call.parameters[callback_ordinal].clone();
            assert!(
                crate::validate_abstract_to_target_translation(
                    &source,
                    native,
                    &callback_collision
                )
                .is_err(),
                "accepted structural argument at the callback's private slot"
            );

            let mut swapped = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                structural_arguments,
                ..
            } = normalized_foreign_mut(&mut swapped, 7)
            else {
                panic!("normalized foreign row")
            };
            scalar_arguments[0].parameter_index = structural_position as u32;
            std::mem::swap(
                &mut scalar_arguments[0].placement,
                &mut structural_arguments[0].destination,
            );
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &swapped).is_err(),
                "accepted equally shaped scalar/structural placement swap"
            );
        }
    }
}

/// `Foreign::leaf(f64, bool, f32) -> f64`: the widened scalar lane admits
/// boolean and IEEE arguments end to end — a caller parameter, a block
/// parameter, an IEEE immediate through the relay lane, and a retained f32
/// home from an earlier foreign result — while replay still re-derives every
/// declared scalar shape and source coordinate.
#[test]
fn normalized_foreign_boolean_and_floating_arguments_replay_with_exact_sources() {
    use abstract_operations::{AbstractParameter, ValueBinding};
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
    use target_operations::TargetUnitScalarArgumentSource;

    let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let f64_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let bool_type = ScalarType::Boolean;
    let incoming = ValueId::new(20).unwrap();
    let flag_source = ValueId::new(21).unwrap();
    let flag = ValueId::new(22).unwrap();
    let relay_in = ValueId::new(23).unwrap();
    let homed = ValueId::new(24).unwrap();
    let produced = ValueId::new(25).unwrap();
    let join = BlockId::new(2).unwrap();
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut source = empty_plan(MachineId::new(1).unwrap());
        source.boundary_machines.push(declaration(
            vec![f64_type, bool_type, f32_type],
            Vec::new(),
            terminal_psi::BoundaryMachineResult::Scalar(f64_type),
        ));
        source
            .boundary_machines
            .push(terminal_psi::BoundaryMachineDeclaration {
                unconditional_termination: false,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                id: BoundaryMachineId::new(2).unwrap(),
                ..declaration(
                    vec![f32_type],
                    Vec::new(),
                    terminal_psi::BoundaryMachineResult::Scalar(f32_type),
                )
            });
        let attached = StructuralTypeId::new(9).unwrap();
        source.functions[0].attachment = Some(attached);
        source
            .structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: attached,
                identity: "probe::Attached".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            });
        let function = &mut source.functions[0];
        function.parameters = vec![AbstractParameter {
            value: incoming,
            scalar_type: f64_type,
        }];
        function.block_entries.push(AbstractBlockEntry {
            block: join,
            parameters: vec![AbstractParameter {
                value: flag,
                scalar_type: bool_type,
            }],
            structural_parameters: Vec::new(),
            operation_offset: 3,
        });
        function.operations = vec![
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(6).unwrap(),
                result: flag_source,
                value: true,
            },
            AbstractOperation::IeeeFloatConstant {
                psi_operation: OperationId::new(7).unwrap(),
                result: relay_in,
                value: IeeeFloatValue::Binary32(0x3FC0_0000),
            },
            AbstractOperation::Jump {
                psi_edge: EdgeId::new(2).unwrap(),
                target: join,
                bindings: vec![ValueBinding {
                    parameter: flag,
                    argument: flag_source,
                    scalar_type: bool_type,
                }],
                structural_bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
            AbstractOperation::BoundaryCall {
                psi_operation: OperationId::new(8).unwrap(),
                result: AbstractBoundaryResult::Scalar(AbstractResult {
                    value: homed,
                    scalar_type: f32_type,
                }),
                boundary: BoundaryMachineId::new(2).unwrap(),
                arguments: vec![relay_in],
                structural_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            },
            AbstractOperation::BoundaryCall {
                psi_operation: OperationId::new(9).unwrap(),
                result: AbstractBoundaryResult::Scalar(AbstractResult {
                    value: produced,
                    scalar_type: f64_type,
                }),
                boundary: BoundaryMachineId::new(1).unwrap(),
                arguments: vec![incoming, flag, homed],
                structural_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            },
            AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ];
        let execution = Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        };
        let leaf_signature = CallSignature {
            parameters: vec![
                ValueShape::float(8),
                ValueShape::integer(1, 1),
                ValueShape::float(4),
            ],
            result: Some(ValueShape::float(8)),
        };
        let relay_signature = CallSignature {
            parameters: vec![ValueShape::float(4)],
            result: Some(ValueShape::float(4)),
        };
        let leaf_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(native),
            &leaf_signature,
        )
        .expect("leaf entry plan")
        .plan()
        .clone();
        let target = lower(
            &source,
            native,
            &execution,
            &[
                (1, binding(native, leaf_signature)),
                (2, binding(native, relay_signature)),
            ],
        );
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("boolean and floating foreign arguments replay");

        let TargetUnitOperation::NormalizedForeignCall {
            scalar_arguments,
            result: target_operations::TargetBoundaryResult::Scalar(result_home),
            binding: row_binding,
            ..
        } = normalized_foreign_ref(&target, 9)
        else {
            panic!("normalized foreign row")
        };
        assert_eq!(row_binding.boundary_entry_plan, leaf_plan);
        assert_eq!(scalar_arguments.len(), 3);
        assert_eq!(
            scalar_arguments[0].source,
            TargetUnitScalarArgumentSource::Parameter {
                parameter_index: 0,
                source_value: incoming,
                scalar_type: f64_type,
            }
        );
        assert_eq!(
            scalar_arguments[1].source,
            TargetUnitScalarArgumentSource::BlockParameter(
                target_operations::TargetScalarBlockValue {
                    block: join,
                    value: flag,
                    scalar_type: bool_type,
                }
            )
        );
        let TargetUnitScalarArgumentSource::Home(home) = &scalar_arguments[2].source else {
            panic!("f32 home argument")
        };
        assert_eq!(home.source_value, homed);
        assert_eq!(home.scalar_type, f32_type);
        assert_eq!(home.shape, ValueShape::float(4));
        assert_eq!(home.defining_operation, OperationId::new(8).unwrap());
        for (argument, destination) in scalar_arguments
            .iter()
            .zip(leaf_plan.call.parameters.iter())
        {
            assert_eq!(argument.placement, *destination);
        }
        let home = result_home;
        assert_eq!(home.scalar_type, f64_type);
        assert_eq!(home.shape, ValueShape::float(8));
        assert_eq!(home.source_value, produced);

        for mutation in 0..4 {
            let mut changed = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                result: target_operations::TargetBoundaryResult::Scalar(result_home),
                ..
            } = normalized_foreign_mut(&mut changed, 9)
            else {
                panic!("normalized foreign row")
            };
            match mutation {
                0 => {
                    scalar_arguments[0].source = TargetUnitScalarArgumentSource::BooleanImmediate {
                        defining_operation: OperationId::new(6).unwrap(),
                        source_value: incoming,
                        value: true,
                    }
                }
                1 => {
                    let TargetUnitScalarArgumentSource::BlockParameter(parameter) =
                        &mut scalar_arguments[1].source
                    else {
                        panic!("block parameter source")
                    };
                    parameter.block = source.functions[0].entry;
                }
                2 => {
                    let TargetUnitScalarArgumentSource::Home(home) =
                        &mut scalar_arguments[2].source
                    else {
                        panic!("home source")
                    };
                    home.defining_operation = OperationId::new(6).unwrap();
                }
                _ => result_home.scalar_type = f32_type,
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "forged scalar source or result home {mutation} on {native:?}"
            );
        }
    }
}

/// `Foreign::leaf(quad: Quad, mixed: Mixed) -> Unit` transports both records
/// by value: `quad` is a homogeneous f32 quadruple and `mixed` a split
/// float/integer pair, so the rebuilt signature is the calling policy's
/// classified shape — `HomogeneousFloatAggregate(4)` and a SysV
/// `Sse`/`Integer` eightbyte pair on SysV, HFA and integer on AAPCS, integer
/// views under Microsoft x64.
fn owned_fixture() -> (AbstractOperationPlan, Execution) {
    use semantic_vocabulary::IeeeFloatFormat;
    use terminal_psi::StructuralTypeShape::Record;

    let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let f64_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let quad_type = StructuralTypeId::new(10).unwrap();
    let mixed_type = StructuralTypeId::new(11).unwrap();
    let quad_place = PlaceId::new(1).unwrap();
    let mixed_place = PlaceId::new(2).unwrap();
    let fields = |entries: Vec<(&str, StructuralFieldType)>| Record {
        fields: entries
            .into_iter()
            .enumerate()
            .map(
                |(index, (identity, field_type))| StructuralFieldDeclaration {
                    id: StructuralFieldId::new(index as u64 + 1).unwrap(),
                    identity: identity.into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type,
                },
            )
            .collect(),
    };
    let mut source = empty_plan(MachineId::new(1).unwrap());
    source.functions[0].structural_parameters = vec![
        StructuralParameterDeclaration {
            place: quad_place,
            position: 0,
            is_self: false,
            structural_type: quad_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            place: mixed_place,
            position: 1,
            is_self: false,
            structural_type: mixed_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    source.structural_types.make_mut().extend([
        StructuralTypeDeclaration {
            id: quad_type,
            identity: "probe::Quad".into(),
            shape: fields(vec![
                ("x", StructuralFieldType::Scalar(f32_type)),
                ("y", StructuralFieldType::Scalar(f32_type)),
                ("z", StructuralFieldType::Scalar(f32_type)),
                ("w", StructuralFieldType::Scalar(f32_type)),
            ]),
        },
        StructuralTypeDeclaration {
            id: mixed_type,
            identity: "probe::Mixed".into(),
            shape: fields(vec![
                ("f", StructuralFieldType::Scalar(f64_type)),
                ("tag", StructuralFieldType::Scalar(u64_type)),
            ]),
        },
    ]);
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![
            StructuralParameterDeclaration {
                place: PlaceId::new(31).unwrap(),
                position: 0,
                is_self: false,
                structural_type: quad_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: PlaceId::new(32).unwrap(),
                position: 1,
                is_self: false,
                structural_type: mixed_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        ],
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![
                terminal_psi::StructuralArgument {
                    place: quad_place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                terminal_psi::StructuralArgument {
                    place: mixed_place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            ],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
}

fn owned_signature(native: NativeTarget) -> CallSignature {
    use calling_conventions::SystemVEightbyteClass;
    let quad = match CallingPolicy::native_for_target(native) {
        CallingPolicy::MicrosoftX64 => ValueShape::integer(16, 4),
        _ => ValueShape::homogeneous_float_aggregate(4, 4),
    };
    let mixed = match CallingPolicy::native_for_target(native) {
        CallingPolicy::SystemVAMD64 => ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Sse,
            SystemVEightbyteClass::Integer,
        ),
        _ => ValueShape::integer(16, 8),
    };
    CallSignature {
        parameters: vec![quad, mixed],
        result: None,
    }
}

/// `Foreign::leaf(view: &[u8], holder_view: &[u8]) -> Unit` borrows a whole
/// stored descriptor and a descriptor field stored inside a record: both
/// formals transport the two-word `BorrowedView` by value under the
/// declaration's own `Integer(16, 8)` signature shape.
fn view_fixture() -> (AbstractOperationPlan, Execution) {
    use terminal_psi::StructuralTypeShape::Record;
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let view_type = StructuralTypeId::new(20).unwrap();
    let holder_type = StructuralTypeId::new(21).unwrap();
    let buffer_place = PlaceId::new(3).unwrap();
    let holder_place = PlaceId::new(4).unwrap();
    let mut source = empty_plan(MachineId::new(1).unwrap());
    source.functions[0].structural_parameters = vec![
        StructuralParameterDeclaration {
            place: buffer_place,
            position: 0,
            is_self: false,
            structural_type: view_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            place: holder_place,
            position: 1,
            is_self: false,
            structural_type: holder_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    source.structural_types.make_mut().extend([
        StructuralTypeDeclaration {
            id: view_type,
            identity: "probe::View".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: holder_type,
            identity: "probe::Holder".into(),
            shape: Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "tag".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(i32_type),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "data".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            ByteSequenceCarrier::BorrowedView {
                                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                            },
                        ),
                    },
                ],
            },
        },
    ]);
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![
            StructuralParameterDeclaration {
                place: PlaceId::new(33).unwrap(),
                position: 0,
                is_self: false,
                structural_type: view_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: PlaceId::new(34).unwrap(),
                position: 1,
                is_self: false,
                structural_type: view_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        ],
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![
                terminal_psi::StructuralArgument {
                    place: buffer_place,
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                terminal_psi::StructuralArgument {
                    place: holder_place,
                    path: vec![StructuralPathSegment::Field("data".to_owned())],
                    access: StructuralAccess::MutableBorrow,
                },
            ],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
}

#[test]
fn normalized_foreign_owned_and_descriptor_arguments_reject_substituted_rows() {
    use target_operations::TargetStructuralArgumentSource;
    for native in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        for mutation in 0..8 {
            let (source, execution) = owned_fixture();
            let target = lower(
                &source,
                native,
                &execution,
                &[(1, binding(native, owned_signature(native)))],
            );
            let mut changed = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                structural_arguments,
                ..
            } = normalized_foreign_mut(&mut changed, 7)
            else {
                panic!("normalized foreign row")
            };
            match mutation {
                // The retained shape claims the access-borrowed pointer form
                // rather than the owned referent shape.
                0 => structural_arguments[0].shape = ValueShape::borrowed_reference(16, 4),
                // An owned argument names its whole referent: a projected
                // path is a substituted projection.
                1 => {
                    structural_arguments[0].path =
                        vec![StructuralPathSegment::Field("x".to_owned())]
                }
                // A caller parameter's declared placement is its custody:
                // claiming a producer home is a substituted source.
                2 => {
                    structural_arguments[0].source =
                        TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: OperationId::new(7).unwrap(),
                        }
                }
                // A claimed destination that is not the plan's exact
                // placement refuses.
                3 => {
                    structural_arguments[0].destination = calling_conventions::ValuePlacement {
                        shape: ValueShape::integer(8, 8),
                        locations: vec![calling_conventions::ValueLocation::Register {
                            register: calling_conventions::MachineRegister::X86Rdi,
                            value_byte_offset: 0,
                            byte_size: 8,
                        }],
                    }
                }
                // A mismatched projected type identity refuses.
                4 => structural_arguments[0].structural_type = StructuralTypeId::new(11).unwrap(),
                // A mismatched byte offset refuses.
                5 => structural_arguments[0].source_byte_offset = 8,
                // A declared access substitution refuses: the semantic
                // argument transports the place by value.
                6 => structural_arguments[0].access = StructuralAccess::SharedBorrow,
                // An indexed transport marker is never produced for a
                // record field path.
                _ => structural_arguments[1].element_stride = Some(1),
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "forged owned-aggregate row {mutation} on {native:?}"
            );
        }
        for mutation in 0..5 {
            let (source, execution) = view_fixture();
            let descriptor = ValueShape::integer(16, 8);
            let target = lower(
                &source,
                native,
                &execution,
                &[(
                    1,
                    binding(
                        native,
                        CallSignature {
                            parameters: vec![descriptor, descriptor],
                            result: None,
                        },
                    ),
                )],
            );
            let mut changed = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                structural_arguments,
                ..
            } = normalized_foreign_mut(&mut changed, 7)
            else {
                panic!("normalized foreign row")
            };
            match mutation {
                // A non-descriptor root accepts no empty-path borrow.
                0 => {
                    structural_arguments[0].place = PlaceId::new(4).unwrap();
                }
                // A stored descriptor field's projected shape is the view's
                // own two words; claiming the bare referent pointer form is
                // a substituted shape.
                1 => structural_arguments[1].shape = ValueShape::borrowed_reference(8, 8),
                // The retained field offset must be the reconstructed one.
                2 => structural_arguments[1].source_byte_offset = 0,
                // A non-byte-sequence leaf path is not a descriptor
                // projection: `tag` carries an i32 field.
                3 => {
                    structural_arguments[1].path =
                        vec![StructuralPathSegment::Field("tag".to_owned())]
                }
                // Substituted destination placement refuses.
                _ => {
                    structural_arguments[0].destination = calling_conventions::ValuePlacement {
                        shape: ValueShape::integer(8, 8),
                        locations: vec![calling_conventions::ValueLocation::Register {
                            register: calling_conventions::MachineRegister::X86Rdi,
                            value_byte_offset: 0,
                            byte_size: 8,
                        }],
                    }
                }
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "forged descriptor row {mutation} on {native:?}"
            );
        }
    }
}

/// `Foreign::leaf(pair: Pair) -> Unit` over an owned actual produced by an
/// earlier call: the retained `StructuralHome` source replays through the
/// producing call's exact affine result home — a substituted producer or a
/// claimed caller placement refuses.
fn home_fixture() -> (AbstractOperationPlan, Execution) {
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let pair_type = StructuralTypeId::new(30).unwrap();
    let produced = PlaceId::new(50).unwrap();
    let mut source = empty_plan(MachineId::new(1).unwrap());
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: pair_type,
            identity: "probe::Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: [
                    ("x", StructuralFieldId::new(1).unwrap()),
                    ("y", StructuralFieldId::new(2).unwrap()),
                ]
                .map(|(identity, id)| StructuralFieldDeclaration {
                    id,
                    identity: identity.into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(i32_type),
                })
                .to_vec(),
            },
        });
    source.boundary_machines.push(declaration(
        Vec::new(),
        vec![StructuralParameterDeclaration {
            place: PlaceId::new(35).unwrap(),
            position: 0,
            is_self: false,
            structural_type: pair_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        terminal_psi::BoundaryMachineResult::Unit,
    ));
    source.functions[0].operations.insert(
        0,
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(6).unwrap(),
            result: AbstractStructuralCallResult::Structural {
                result: terminal_psi::StructuralOperationResult {
                    place: produced,
                    structural_type: pair_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
                returned_claim_transfers: Vec::new(),
                selected_evidence: Vec::new(),
            },
            callee: MachineId::new(902).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    source.functions[0].operations.insert(
        1,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: produced,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    let mut callee = source.functions[0].clone();
    callee.machine = MachineId::new(902).unwrap();
    callee.structural_parameters = Vec::new();
    callee.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: ValueId::new(40).unwrap(),
            scalar_type: i32_type,
            value: IntegerValue::Signed(7),
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(2).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: produced,
                structural_type: pair_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            fields: [
                StructuralFieldId::new(1).unwrap(),
                StructuralFieldId::new(2).unwrap(),
            ]
            .map(|field| terminal_psi::RecordFieldInitializer {
                field,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: ValueId::new(40).unwrap(),
                    range_obligation: None,
                },
            })
            .to_vec(),
        },
        AbstractOperation::ReturnStructural {
            psi_edge: EdgeId::new(1).unwrap(),
            source: produced,
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    callee.result = AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
        place: produced,
        structural_type: pair_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    source.functions.push(callee);
    (
        source,
        Execution {
            plan_report: 0xA1,
            requirement: REQUIREMENT.into(),
        },
    )
}

fn pair_signature() -> CallSignature {
    CallSignature {
        parameters: vec![ValueShape::integer(8, 4)],
        result: None,
    }
}

#[test]
fn normalized_foreign_owned_aggregate_from_call_result_replays_affine_home() {
    for native in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
        let (source, execution) = home_fixture();
        let target = lower(
            &source,
            native,
            &execution,
            &[(1, binding(native, pair_signature()))],
        );
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap_or_else(
            |operation| {
                panic!("home-sourced owned row must replay on {native:?}: refused {operation:?}")
            },
        );
        let TargetUnitOperation::NormalizedForeignCall {
            structural_arguments,
            ..
        } = normalized_foreign_ref(&target, 7)
        else {
            panic!("normalized foreign row")
        };
        assert_eq!(
            structural_arguments[0].source,
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(6).unwrap(),
            }
        );
        for mutation in 0..3 {
            let mut changed = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                structural_arguments,
                ..
            } = normalized_foreign_mut(&mut changed, 7)
            else {
                panic!("normalized foreign row")
            };
            match mutation {
                // A substituted producer identity refuses.
                0 => {
                    structural_arguments[0].source =
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: OperationId::new(7).unwrap(),
                        }
                }
                // A claimed caller placement is not the producing call's
                // home.
                1 => {
                    structural_arguments[0].source =
                        target_operations::TargetStructuralArgumentSource::Placement(
                            calling_conventions::ValuePlacement {
                                shape: ValueShape::integer(8, 4),
                                locations: vec![calling_conventions::ValueLocation::Register {
                                    register: calling_conventions::MachineRegister::X86Rax,
                                    value_byte_offset: 0,
                                    byte_size: 8,
                                }],
                            },
                        )
                }
                // An unrestricted formal cannot take an affine result.
                _ => structural_arguments[0].shape = ValueShape::borrowed_reference(8, 4),
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "forged affine-home row {mutation} on {native:?}"
            );
        }
    }
}
