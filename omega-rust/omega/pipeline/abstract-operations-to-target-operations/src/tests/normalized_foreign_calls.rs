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
            ieee_float_fma: &[],
        },
    )
    .unwrap()
}

fn normalized_foreign_ref<'a>(
    plan: &'a TargetOperationPlan,
    psi_operation: u32,
) -> &'a TargetUnitOperation {
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
fn mixed_foreign_formals_preserve_each_authored_interleave() {
    use terminal_psi::BoundaryParameterKind::{Scalar, Structural};
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for order in [
            vec![Scalar, Structural, Scalar],
            vec![Structural, Scalar, Scalar],
            vec![Scalar, Scalar, Structural],
        ] {
            let (mut source, execution) = flat_record_fixture();
            let integers = [
                IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            ];
            source.boundary_machines[0].scalar_parameters =
                integers.map(ScalarType::Integer).to_vec();
            source.boundary_machines[0].parameter_order = order.clone();
            let AbstractOperation::BoundaryCall { arguments, .. } =
                &mut source.functions[0].operations[0]
            else {
                panic!("boundary call")
            };
            *arguments = vec![ValueId::new(1).unwrap(), ValueId::new(2).unwrap()];
            for (index, scalar_type) in integers.iter().enumerate() {
                source.functions[0].operations.insert(
                    index,
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(index as u64 + 1).unwrap(),
                        result: ValueId::new(index as u64 + 1).unwrap(),
                        scalar_type: ScalarType::Integer(*scalar_type),
                        value: semantic_vocabulary::IntegerValue::Signed(index as i128 + 11),
                    },
                );
            }
            let mut scalars = [ValueShape::integer(8, 8), ValueShape::integer(4, 4)].into_iter();
            let signature = CallSignature {
                parameters: order
                    .iter()
                    .map(|kind| match kind {
                        Scalar => scalars.next().unwrap(),
                        Structural => ValueShape::integer(8, 8),
                    })
                    .collect(),
                result: Some(ValueShape::integer(4, 4)),
            };
            let foreign = binding(native, signature);
            let target = lower(&source, native, &execution, &[(1, foreign.clone())]);
            crate::validate_abstract_to_target_translation(&source, native, &target)
                .expect("mixed signature independently replays");
            let TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                structural_arguments,
                ..
            } = normalized_foreign_ref(&target, 7)
            else {
                panic!("foreign row")
            };
            let positions = order
                .iter()
                .enumerate()
                .filter_map(|(index, kind)| (*kind == Scalar).then_some(index as u32))
                .collect::<Vec<_>>();
            assert_eq!(
                scalar_arguments
                    .iter()
                    .map(|argument| argument.parameter_index)
                    .collect::<Vec<_>>(),
                positions
            );
            let structural_position = order.iter().position(|kind| *kind == Structural).unwrap();
            assert_eq!(
                structural_arguments[0].destination,
                foreign.boundary_entry_plan.call.parameters[structural_position]
            );

            // Even equally shaped scalar/pointer parameters have distinct
            // authored identities; swapping them cannot replay old custody.
            let mut changed_source = source.clone();
            let scalar_position = positions[0] as usize;
            changed_source.boundary_machines[0]
                .parameter_order
                .swap(scalar_position, structural_position);
            assert!(
                crate::validate_abstract_to_target_translation(&changed_source, native, &target)
                    .is_err()
            );
            let mut changed_target = target.clone();
            let TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments, ..
            } = normalized_foreign_mut(&mut changed_target, 7)
            else {
                panic!("foreign row")
            };
            scalar_arguments[0].parameter_index = structural_position as u32;
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed_target)
                    .is_err()
            );
        }
    }
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
        machine Main::run(&self) reaches Foreign {
            let observed: i32 = Foreign::leaf(11i64, &self.point, 12i32);
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
            u16::try_from(native.pointer_size).unwrap(),
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
        let target = crate::lower_to_target_operations_and_native_callbacks(
            &source,
            crate::TargetLoweringRequest {
                target: native,
                settlements: &settlements,
                installation: None,
                ieee_float_fma: &[],
            },
            &[admission],
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
                    destination,
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
            let target = crate::lower_to_target_operations_and_native_callbacks(
                &source,
                crate::TargetLoweringRequest {
                    target: native,
                    settlements: &settlements,
                    installation: None,
                    ieee_float_fma: &[],
                },
                &[admission],
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
