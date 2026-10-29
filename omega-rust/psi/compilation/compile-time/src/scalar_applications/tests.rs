//! Canonical dependency-graph execution, not source-production acceptance.
//! The unchanged source witnesses remain the outer migration acceptance.
use super::*;
use arena::Arena;
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{PredicateNode, PredicateOperator, ScalarValue};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, StructuralTypeId, ValueId,
};
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_installation::{admit_provider_installation, admit_target_machine_selections};
use terminal_interpreter::TerminalScalarValue;
use terminal_psi::{
    ArrayExtent, Block, LayoutMeasurement, MachineContract, Operation, OperationKind,
    OperationResult, ScalarConditionSubject, ScalarPredicateNode, ScalarRangeRequirement,
    StructuralTypeDeclaration, StructuralTypeShape, TargetMachineCallable,
    TerminalIdentityTableBuilder, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration,
};

fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn value_id(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}
fn type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn application(value: u64) -> ScalarApplicationId {
    ScalarApplicationId::new(value).unwrap()
}
fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}
fn scalar() -> ScalarType {
    ScalarType::Integer(integer_type())
}
fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: integer_type(),
        value: IntegerValue::Unsigned(value),
    }
}

fn machine(identity: u64, operations: Vec<OperationKind>) -> TerminalMachine {
    let result = ValueDeclaration {
        id: value_id(operations.len() as u64),
        scalar_type: scalar(),
        qualifications: Default::default(),
    };
    TerminalMachine {
        id: machine_id(identity),
        attachment: None,
        declaration: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Scalar(result.clone()),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: operations
                .into_iter()
                .enumerate()
                .map(|(position, kind)| {
                    Operation::unsuspended(
                        OperationId::new(position as u64 + 1).unwrap(),
                        OperationResult::Scalar(ValueDeclaration {
                            id: value_id(position as u64 + 1),
                            scalar_type: scalar(),
                            qualifications: Default::default(),
                        }),
                        kind,
                    )
                })
                .collect(),
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: MachineContract::empty(ContractId::new(identity).unwrap()),
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    }
}

fn graph(expected: u64, cyclic: bool, signed_extent: bool) -> CanonicalTerminalArtifact {
    let mut module = TerminalModule::for_entry(machine_id(2));
    let mut identities = TerminalIdentityTableBuilder::default();
    for (identity, shape) in [
        (1, StructuralTypeShape::PrimitiveScalar(scalar())),
        (
            2,
            StructuralTypeShape::FixedArray {
                element: type_id(1),
                length: ArrayExtent::Literal(2),
            },
        ),
        (
            3,
            StructuralTypeShape::FixedArray {
                element: type_id(4),
                length: ArrayExtent::Application(application(if cyclic { 5 } else { 2 })),
            },
        ),
        (
            4,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        ),
    ] {
        module.structural_types.push(StructuralTypeDeclaration {
            id: type_id(identity),
            identity: identities.intern(&format!("graph::type{identity}")),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        });
    }
    module.identities = identities.build();
    let mut extent = machine(
        1,
        vec![OperationKind::ReadScalarApplication {
            application: application(1),
        }],
    );
    let extent_type = if signed_extent {
        let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
        extent.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: value_id(1),
            scalar_type: signed,
            qualifications: Default::default(),
        });
        extent.blocks[0].operations[0] = Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: value_id(1),
                scalar_type: signed,
                qualifications: Default::default(),
            }),
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(16),
            },
        );
        signed
    } else {
        scalar()
    };
    module.machines = vec![
        extent,
        machine(
            2,
            vec![
                OperationKind::ReadScalarApplication {
                    application: application(3),
                },
                OperationKind::ReadScalarApplication {
                    application: application(4),
                },
                OperationKind::WrappingIntegerAdd {
                    left: value_id(1),
                    right: value_id(2),
                },
            ],
        ),
    ];
    for row in [
        ScalarApplication::LayoutObservation {
            subject: type_id(2),
            measurement: LayoutMeasurement::Size,
            semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
            scalar_type: scalar(),
        },
        ScalarApplication::Evaluate {
            entry: TargetMachineCallable::Machine(machine_id(1)),
            scalar_type: extent_type,
        },
        ScalarApplication::LayoutObservation {
            subject: type_id(3),
            measurement: LayoutMeasurement::Size,
            semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
            scalar_type: scalar(),
        },
        ScalarApplication::LayoutObservation {
            subject: type_id(2),
            measurement: LayoutMeasurement::Alignment,
            semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
            scalar_type: scalar(),
        },
        ScalarApplication::Evaluate {
            entry: TargetMachineCallable::Machine(machine_id(2)),
            scalar_type: scalar(),
        },
    ] {
        module.scalar_applications.insert(row).unwrap();
    }
    module
        .scalar_applications
        .conditions
        .retain_range(
            ScalarConditionSubject::Application(application(5)),
            ScalarRangeRequirement::Fixed {
                minimum: BigInt::from_u64(16),
                maximum: BigInt::from_u64(16),
            },
            vec![IntegerBoundStep::Observation(
                ScalarRangeObservation::EntryResult {
                    application: application(5),
                    machine: machine_id(2),
                    result: value_id(1),
                    scalar_type: scalar(),
                },
            )],
        )
        .unwrap();
    let mut nodes = Arena::new();
    let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
        ScalarRangeObservation::Application(application(5)),
    )));
    let right = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
        ScalarValue::Integer(BigInt::from_u64(expected)),
    )));
    let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
        operator: PredicateOperator::Equal,
        left,
        right,
    }));
    module
        .scalar_applications
        .conditions
        .retain_predicate(
            ScalarConditionSubject::Application(application(5)),
            "total layout size".into(),
            nodes,
            root,
        )
        .unwrap();
    let optimization = build_identity_optimization_execution_record(&module).unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap();
    CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap()
}

#[test]
fn canonical_scalar_jobs_compose_geometry_entries_and_exact_observations() {
    let artifact = graph(24, false, false);
    let original = artifact.to_bytes();
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    for target in ["windows_x86_64", "macos_arm64"] {
        let selections = admit_target_machine_selections(artifact.view(), target);
        let mut observations =
            ScalarApplicationObservations::new(artifact.view(), &selections, &providers).unwrap();
        let mut meter = TerminalFuelMeter::unbounded();
        evaluate_scalar_applications(&mut observations, &[application(5)], &mut meter).unwrap();
        for (identity, expected) in [(1, 16), (2, 16), (3, 16), (4, 8), (5, 24)] {
            assert_eq!(
                observations.value(application(identity)).unwrap(),
                Some(integer(expected))
            );
        }
        // The intermediate 16 and final 24 discharge different obligations.
        observations.discharge_range(0, None).unwrap();
        observations.discharge_predicate(0).unwrap();
        let usage = meter.usage().clone();
        evaluate_scalar_applications(&mut observations, &[application(5)], &mut meter).unwrap();
        assert_eq!(meter.usage(), &usage, "completed jobs are not re-executed");
        assert_eq!(
            artifact.to_bytes(),
            original,
            "target observations never rewrite canonical input"
        );
    }
}

#[test]
fn canonical_scalar_jobs_refuse_cycles_and_false_conditions() {
    for cyclic in [false, true] {
        let artifact = graph(23, cyclic, false);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations =
            ScalarApplicationObservations::new(artifact.view(), &selections, &providers).unwrap();
        let evaluated = evaluate_scalar_applications(
            &mut observations,
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
        );
        if cyclic {
            assert!(evaluated.unwrap_err().contains("dependency cycle"));
        } else {
            evaluated.unwrap();
            assert!(
                observations
                    .discharge_predicate(0)
                    .unwrap_err()
                    .contains("predicate is false")
            );
            assert!(
                TerminalProgram::borrow(artifact.view())
                    .instantiate(TerminalInstantiation::default())
                    .is_err(),
                "provisional success does not admit ordinary execution"
            );
        }
    }
}

#[test]
fn canonical_scalar_jobs_preserve_nonnegative_signed_extent_carriers() {
    let artifact = graph(24, false, true);
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let mut observations =
        ScalarApplicationObservations::new(artifact.view(), &selections, &providers).unwrap();
    evaluate_scalar_applications(
        &mut observations,
        &[application(5)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
    observations.discharge_range(0, None).unwrap();
    observations.discharge_predicate(0).unwrap();
    assert_eq!(observations.extent(application(2)).unwrap(), Some(16));
}

#[test]
fn canonical_scalar_jobs_bind_provisional_inputs_to_the_target_and_entry() {
    use terminal_interpreter::{TerminalArtifactInterpretError, TerminalInterpretError};
    let artifact = graph(24, false, false);
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let windows = admit_target_machine_selections(artifact.view(), "windows_x86_64");
    let macos = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let mut observations =
        ScalarApplicationObservations::new(artifact.view(), &windows, &providers).unwrap();
    observations.observe_layout(application(1)).unwrap();
    let program = TerminalProgram::borrow(artifact.view());
    for (selected, entry) in [
        (&macos, machine_id(1)),
        (&windows, machine_id(2)),
        (&windows, machine_id(1)),
    ] {
        let layouts =
            terminal_installation::admit_target_layouts(artifact.view(), selected).unwrap();
        let installation = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                entry: Some(TargetMachineCallable::Machine(entry)),
                installation: Some(&providers),
                target_machines: Some(selected),
                target_layouts: Some(&layouts),
                host_imports: &[],
                x86_flags: None,
                memory_ordering: None,
            })
            .unwrap();
        let result = program.instantiate(TerminalInstantiation {
            installation: Some(&installation),
            scalar_application: Some(TerminalScalarApplicationRequest {
                application: application(2),
                observations: &observations,
            }),
            ..Default::default()
        });
        if selected == &windows && entry == machine_id(1) {
            assert!(
                matches!(result.unwrap().invoke(&mut TerminalFuelMeter::unbounded()).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) if value == integer(16))
            );
        } else {
            assert!(
                matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::UnsupportedSemanticVariant(reason)))
                if reason == "scalar evaluation inputs differ from the exact artifact, entry, target or provider selections")
            );
        }
    }
}

#[test]
fn canonical_scalar_jobs_do_not_admit_raw_byte_execution() {
    use terminal_interpreter::{TerminalArtifactInterpretError, TerminalInterpretError};
    let artifact = graph(24, false, false);
    let result = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
    );
    assert!(
        matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::ScalarApplicationEvaluation(reason)))
        if reason == "ordinary execution requires admission of retained scalar conditions")
    );
}
