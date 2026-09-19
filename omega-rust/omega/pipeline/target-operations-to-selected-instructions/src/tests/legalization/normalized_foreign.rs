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
        locator,
        boundary_entry_plan,
        same_stack_contribution,
    }
}

fn declaration(
    scalar_parameters: Vec<ScalarType>,
    structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    result: terminal_psi::BoundaryMachineResult,
) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
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
            ieee_float_fma: &[],
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
fn scalar_lane_projects_exact_custody_and_replays() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, execution) = scalar_fixture();
        let shape = calling_conventions::ValueShape::integer(4, 4);
        let binding = binding(
            native,
            calling_conventions::CallSignature {
                parameters: vec![shape],
                result: None,
            },
        );
        let target = lower(&source, native, &execution, binding.clone());
        let unit = seed(&source);
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("scalar normalized foreign call legalizes");
        let call = normalized_foreign_instruction(legal.plan());
        assert_eq!(call.boundary, BoundaryMachineId::new(1).unwrap());
        assert_eq!(call.binding, binding);
        let [argument] = call.scalar_arguments.as_slice() else {
            panic!("one scalar argument")
        };
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(argument.source_value(), ValueId::new(5).unwrap());
        assert_eq!(
            argument.placement,
            call.binding.boundary_entry_plan.call.parameters[0]
        );
        assert!(call.structural_arguments.is_empty());
        assert_eq!(call.result_home, None);
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
            .expect("independent replay");
    }
}

#[test]
fn flat_record_lane_projects_source_rooted_borrow_and_replays() {
    let native = NativeTarget::macos_arm64();
    let (source, execution) = flat_record_fixture();
    let pointer = u16::try_from(native.pointer_size).unwrap();
    let binding = binding(
        native,
        calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(
                pointer,
                u16::try_from(native.pointer_alignment).unwrap(),
            )],
            result: Some(calling_conventions::ValueShape::integer(4, 4)),
        },
    );
    let target = lower(&source, native, &execution, binding.clone());
    let unit = seed(&source);
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("flat-record normalized foreign call legalizes");
    let call = normalized_foreign_instruction(legal.plan());
    assert_eq!(call.binding, binding);
    assert!(call.scalar_arguments.is_empty());
    let [argument] = call.structural_arguments.as_slice() else {
        panic!("one structural argument")
    };
    assert_eq!(argument.place, PlaceId::new(1).unwrap());
    assert_eq!(
        argument.path.as_slice(),
        [terminal_psi::StructuralPathSegment::Field("p".to_owned())]
    );
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    assert_eq!(
        argument.root_structural_type,
        StructuralTypeId::new(1).unwrap()
    );
    assert_eq!(argument.structural_type, StructuralTypeId::new(2).unwrap());
    assert_eq!(argument.source_byte_offset, 0);
    let result_home = call.result_home.expect("scalar result home");
    assert_eq!(result_home.source_value, ValueId::new(5).unwrap());
    assert_eq!(result_home.defining_operation, OperationId::new(7).unwrap());
    assert_eq!(
        result_home.shape,
        calling_conventions::ValueShape::integer(4, 4)
    );
    validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
        .expect("independent replay");
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
                normalized_foreign_call_mut(&mut changed).result_home =
                    Some(target_operations::TargetUnitScalarHomeRequirement {
                        defining_operation: OperationId::new(7).unwrap(),
                        source_value: ValueId::new(9).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        ),
                        shape,
                    })
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

#[test]
fn flat_record_replay_rejects_projection_and_result_home_drift() {
    let native = NativeTarget::macos_arm64();
    let (source, execution) = flat_record_fixture();
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
    for mutation in 0..6 {
        let mut changed = legal.plan().clone();
        match mutation {
            0 => normalized_foreign_call_mut(&mut changed).structural_arguments[0]
                .path
                .clear(),
            1 => {
                normalized_foreign_call_mut(&mut changed).structural_arguments[0]
                    .source_byte_offset = 4
            }
            2 => {
                normalized_foreign_call_mut(&mut changed).structural_arguments[0].structural_type =
                    StructuralTypeId::new(1).unwrap()
            }
            3 => {
                normalized_foreign_call_mut(&mut changed).structural_arguments[0].access =
                    terminal_psi::StructuralAccess::Owned
            }
            4 => {
                normalized_foreign_call_mut(&mut changed)
                    .result_home
                    .as_mut()
                    .unwrap()
                    .source_value = ValueId::new(9).unwrap()
            }
            _ => {
                normalized_foreign_call_mut(&mut changed)
                    .result_home
                    .as_mut()
                    .unwrap()
                    .shape = calling_conventions::ValueShape::integer(8, 8)
            }
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "accepted flat-record mutation {mutation}"
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
