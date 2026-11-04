//! Canonical dependency-graph execution, not source-production acceptance.
//! The unchanged source witnesses remain the outer migration acceptance.
use super::*;
use arena::Arena;
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{PredicateNode, PredicateOperator, ScalarValue};
use semantic_vocabulary::proof_applications::{
    ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, StructuralTypeId, ValueId,
};
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_installation::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, ScalarConditionSelection,
    admit_provider_installation, admit_target_machine_selections,
};
use terminal_interpreter::{TerminalRuntimeSupplyRequest, TerminalScalarValue};
use terminal_psi::{
    ArrayExtent, Block, LayoutMeasurement, MachineContract, Operation, OperationKind,
    OperationResult, ScalarApplication, ScalarConditionSubject, ScalarPredicateNode,
    ScalarRangeObservation, ScalarRangeRequirement, StructuralTypeDeclaration, StructuralTypeShape,
    TargetMachineCallable, TerminalIdentityTableBuilder, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration,
};

#[path = "closure_tests.rs"]
mod closure;

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

/// One entry in its primary value scope. Both provisional findings borrow the
/// same inputs, so the caller keeps discharging and asserting on them.
fn evaluated<'a>(
    artifact: &'a CanonicalTerminalArtifact,
    targets: &'a AdmittedTargetMachineSelections,
    providers: &'a AdmittedProviderSelections,
    entry: u64,
) -> (
    ScalarApplicationObservations<'a>,
    ScalarConditionSelection<'a>,
) {
    let mut observations = ScalarApplicationObservations::new(
        artifact.view(),
        targets,
        providers,
        &terminal_installation::LayoutTargetFacts {
            profile: targets.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    let conditions = evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(entry)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
    (observations, conditions)
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
        result: TerminalMachineResult::Scalar(result),
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
            semantic_vocabulary::ScalarConditionRole::Result,
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
            semantic_vocabulary::ScalarConditionRole::Result,
            "total layout size".into(),
            nodes,
            root,
        )
        .unwrap();
    canonical(module)
}

fn canonical(mut module: TerminalModule) -> CanonicalTerminalArtifact {
    retain_graph_proof_scope(&mut module);
    let optimization = build_identity_optimization_execution_record(&module).unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap();
    CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap()
}

/// Give each hand-built graph the declaration, call and checked-header rows
/// that lowering emits for its executable bodies. Completing an evaluated
/// entry admits its root's checked crash summary before execution, so every
/// entry needs an exact declaration with compiler-evaluation facts even when
/// it has no prerequisite. Existing scope fixtures keep their authored
/// identities and edges.
fn retain_graph_proof_scope(module: &mut TerminalModule) {
    use semantic_vocabulary::compiler_evaluation::{
        CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
    };
    use terminal_psi::{ProofCallableAssociation, ProofCallableHeader, ProofScopeDeclaration};

    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let scope = &mut module.scalar_applications.proof_scope;
    for machine in &module.machines {
        let callable = TargetMachineCallable::Machine(machine.id);
        if scope.callable_declaration(callable).is_some() {
            continue;
        }
        let declaration = next_declaration(scope);
        let identity = identities.intern(&graph_machine_name(machine.id));
        scope
            .declarations
            .insert(ProofScopeDeclaration::Nominal(identity));
        scope.callables.insert(ProofCallableAssociation::Callable {
            declaration,
            callable,
        });
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: declaration,
            product_selected: true,
            ..Default::default()
        });
    }
    // Codec validation wants headers in increasing declaration order. Every
    // fixture authors a header for each declaration it authors, so only the
    // declarations appended above in machine order receive one here.
    for machine in &module.machines {
        let declaration = scope
            .callable_declaration(TargetMachineCallable::Machine(machine.id))
            .unwrap();
        if !scope
            .callable_headers
            .iter()
            .any(|(_, header)| header.declaration == declaration)
        {
            scope.callable_headers.insert(ProofCallableHeader {
                declaration,
                compiler_evaluation: Ok(CompilerEvaluationDeclaration {
                    name: graph_machine_name(machine.id),
                    custody: DeclarationCustody::Toolchain,
                    effects: CompilerEvaluationEffects::default(),
                    local_terminates: true,
                    entry_requires: false,
                    linear_carrier: None,
                    static_application: None,
                    selections: Ok(Vec::new()),
                }),
                ..ProofCallableHeader::default()
            });
        }
        if scope
            .calls
            .machines
            .iter()
            .any(|(_, row)| row.machine == declaration)
        {
            continue;
        }
        let calls = machine
            .operations()
            .filter_map(|operation| match &operation.kind {
                OperationKind::Call { callee, .. } => {
                    let callee = scope
                        .callable_declaration(TargetMachineCallable::Machine(*callee))
                        .expect("test call target must have its declaration association");
                    Some(ProofApplicationCall {
                        callee,
                        state: callee,
                        ..Default::default()
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let calls = scope.calls.calls.insert_many(calls);
        scope.calls.machines.insert(ProofCallMachine {
            machine: declaration,
            calls,
            ..Default::default()
        });
    }
    module.identities = identities.build();
}

fn graph_machine_name(machine: MachineId) -> String {
    format!("scalar-graph::machine{}", machine.get())
}

/// The next declaration ID; scope declarations are numbered from 1 in
/// insertion order.
fn next_declaration(
    scope: &terminal_psi::ProofApplicationScope,
) -> terminal_psi::ProofDeclarationId {
    terminal_psi::ProofDeclarationId(u32::try_from(scope.declarations.len() + 1).unwrap())
}

#[test]
fn canonical_scalar_jobs_do_not_capture_an_unselected_declarations_result() {
    let mut module = graph(24, false, false).view().module().clone();
    let scope = &mut module.scalar_applications.proof_scope;
    let dormant = next_declaration(scope);
    scope
        .declarations
        .insert(terminal_psi::ProofScopeDeclaration::Unavailable(
            "dormant declaration".into(),
        ));
    scope.plan.machines.insert(ProofApplicationMachine {
        machine: dormant,
        authored_target: Some("windows_x86_64".into()),
        source_target: Some("windows_x86_64".into()),
        product_selected: true,
        ..Default::default()
    });
    let result = ValueDeclaration {
        id: value_id(99),
        scalar_type: scalar(),
        qualifications: Default::default(),
    };
    module.machines[1].blocks.push(Block {
        id: BlockId::new(99).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![Operation::unsuspended(
            OperationId::new(99).unwrap(),
            OperationResult::Scalar(result),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        )],
        terminator: Terminator::Return {
            edge: EdgeId::new(99).unwrap(),
            value: value_id(99),
            cleanup_actions: Vec::new(),
        },
    });
    module
        .scalar_applications
        .conditions
        .retain_range(
            ScalarConditionSubject::Declaration(dormant),
            semantic_vocabulary::ScalarConditionRole::Result,
            ScalarRangeRequirement::Fixed {
                minimum: BigInt::from_u64(0),
                maximum: BigInt::from_u64(0),
            },
            vec![IntegerBoundStep::Observation(
                ScalarRangeObservation::EntryResult {
                    application: application(5),
                    machine: machine_id(2),
                    result: value_id(99),
                    scalar_type: scalar(),
                },
            )],
        )
        .unwrap();
    let artifact = canonical(module);
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let mut observations = ScalarApplicationObservations::new(
        artifact.view(),
        &targets,
        &providers,
        &terminal_installation::LayoutTargetFacts {
            profile: targets.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(5)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
    assert_eq!(
        observations.value(application(5)).unwrap(),
        Some(integer(24))
    );
    let windows = admit_target_machine_selections(artifact.view(), "windows_x86_64");
    let mut selected = ScalarApplicationObservations::new(
        artifact.view(),
        &windows,
        &providers,
        &terminal_installation::LayoutTargetFacts {
            profile: windows.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    let error = evaluate_scalar_applications(
        &mut selected,
        "macos_arm64",
        &[application(5)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .err()
    .unwrap();
    assert!(
        error.contains("entry returned without completing every requested result occurrence"),
        "{error}"
    );
}

fn sibling_extent_graph() -> CanonicalTerminalArtifact {
    let mut module = graph(24, false, false).view().module().clone();
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    module.machines.push(machine(
        3,
        vec![
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(3),
            },
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(4),
            },
        ],
    ));
    module.machines.push(machine(
        4,
        vec![OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(5),
        }],
    ));
    for identity in [3, 4] {
        module
            .scalar_applications
            .insert(ScalarApplication::Evaluate {
                entry: TargetMachineCallable::Machine(machine_id(identity)),
                scalar_type: scalar(),
            })
            .unwrap();
    }
    for (identity, extent) in [(5, 6), (6, 7)] {
        let mut declaration = module.structural_types[3].clone();
        declaration.id = type_id(identity);
        declaration.identity = identities.intern(&format!("graph::type{identity}"));
        declaration.shape = StructuralTypeShape::FixedArray {
            element: type_id(4),
            length: ArrayExtent::Application(application(extent)),
        };
        module.structural_types.push(declaration);
    }
    let mut record = module.structural_types[3].clone();
    record.id = type_id(7);
    record.identity = identities.intern("graph::type7");
    let fields: Vec<_> = [5, 6]
        .into_iter()
        .map(|identity| terminal_psi::StructuralFieldDeclaration {
            inline_carrier: None,
            id: semantic_vocabulary::StructuralFieldId::new(identity).unwrap(),
            identity: identities.intern(&format!("graph::type7::field{identity}")),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Structural(type_id(identity)),
            storage: None,
        })
        .collect();
    module.native_layouts.insert(
        record.id,
        record.identity,
        terminal_psi::TerminalIdentityIndex::ABSENT,
        terminal_psi::TerminalIdentityIndex::ABSENT,
        fields
            .iter()
            .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                field: field.id,
                declaration_identity: field.identity,
            }),
        [],
    );
    record.shape = StructuralTypeShape::Record {
        storage: terminal_psi::StructuralRecordStorage::Packed,
        fields,
    };
    module.structural_types.push(record);
    module.identities = identities.build();
    module
        .scalar_applications
        .insert(ScalarApplication::LayoutObservation {
            subject: type_id(7),
            measurement: LayoutMeasurement::Size,
            semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
            scalar_type: scalar(),
        })
        .unwrap();
    module
        .scalar_applications
        .conditions
        .retain_range(
            ScalarConditionSubject::Application(application(7)),
            semantic_vocabulary::ScalarConditionRole::Result,
            ScalarRangeRequirement::Fixed {
                minimum: BigInt::from_u64(3),
                maximum: BigInt::from_u64(3),
            },
            vec![IntegerBoundStep::Observation(
                ScalarRangeObservation::EntryResult {
                    application: application(6),
                    machine: machine_id(3),
                    result: value_id(1),
                    scalar_type: scalar(),
                },
            )],
        )
        .unwrap();
    canonical(module)
}

#[test]
fn canonical_scalar_jobs_discover_sibling_conditions_before_execution() {
    let artifact = sibling_extent_graph();
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let (observations, conditions) = evaluated(&artifact, &targets, &providers, 8);
    assert_eq!(conditions.ranges(), &[1]);
    assert!(conditions.predicates().is_empty());
    assert_eq!(
        observations.value(application(8)).unwrap(),
        Some(integer(9))
    );
    assert_eq!(
        observations.value(application(6)).unwrap(),
        Some(integer(4))
    );
    assert_eq!(
        observations
            .entry_result(application(6), machine_id(3), value_id(1), scalar())
            .unwrap(),
        integer(3)
    );
    observations.discharge_range(1, None).unwrap();
    assert_eq!(
        observations.value(application(5)).unwrap(),
        None,
        "unrelated application stays dormant"
    );
}

#[test]
fn canonical_scalar_jobs_refuse_replay_to_fill_a_late_capture() {
    let artifact = sibling_extent_graph();
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let mut observations = ScalarApplicationObservations::new(
        artifact.view(),
        &targets,
        &providers,
        &terminal_installation::LayoutTargetFacts {
            profile: targets.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::unbounded();
    evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(6)],
        &mut meter,
    )
    .unwrap();
    let usage = meter.usage().clone();
    let error = evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(8)],
        &mut meter,
    )
    .err()
    .unwrap();
    assert!(error.contains("unobserved entry result"), "{error}");
    assert_eq!(
        meter.usage(),
        &usage,
        "missing capture refuses before any new execution"
    );
    assert_eq!(observations.value(application(7)).unwrap(), None);
    assert_eq!(observations.value(application(8)).unwrap(), None);
}

#[test]
fn canonical_scalar_jobs_compose_geometry_entries_and_exact_observations() {
    let artifact = graph(24, false, false);
    let original = artifact.to_bytes();
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    for target in ["windows_x86_64", "macos_arm64"] {
        let selections = admit_target_machine_selections(artifact.view(), target);
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &selections,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: selections.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let mut meter = TerminalFuelMeter::unbounded();
        evaluate_scalar_applications(&mut observations, target, &[application(5)], &mut meter)
            .unwrap();
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
        evaluate_scalar_applications(&mut observations, target, &[application(5)], &mut meter)
            .unwrap();
        assert_eq!(meter.usage(), &usage, "completed jobs are not re-executed");
        assert_eq!(
            artifact.to_bytes(),
            original,
            "target observations never rewrite canonical input"
        );
    }
}

#[test]
fn canonical_scalar_jobs_check_local_prerequisites_before_completion() {
    for expected_operand in [16, 17] {
        let mut module = graph(24, false, false).view().module().clone();
        module
            .scalar_applications
            .conditions
            .retain_range(
                ScalarConditionSubject::Application(application(5)),
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
                ScalarRangeRequirement::Fixed {
                    minimum: BigInt::from_u64(expected_operand),
                    maximum: BigInt::from_u64(expected_operand),
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
        let artifact = canonical(module);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: targets.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let result = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
        );
        if expected_operand == 16 {
            result.unwrap();
            assert_eq!(
                observations.value(application(5)).unwrap(),
                Some(integer(24))
            );
            assert_eq!(
                observations
                    .entry_result(application(5), machine_id(2), value_id(1), scalar())
                    .unwrap(),
                integer(16)
            );
            observations.discharge_range(1, None).unwrap();
            observations.discharge_predicate(0).unwrap();
        } else {
            let error = result
                .err()
                .expect("a false local prerequisite must stop the entry before completion");
            assert!(
                error.contains("constant invocation prerequisite"),
                "{error}"
            );
            assert!(
                error.contains("bounds [16, 16] lie outside [17, 17]"),
                "{error}"
            );
            assert_eq!(observations.value(application(5)).unwrap(), None);
            assert!(
                observations
                    .entry_result(application(5), machine_id(2), value_id(1), scalar())
                    .is_err(),
                "a private capture from a failed entry must not become a completed observation"
            );
        }
    }
}

#[test]
fn canonical_scalar_jobs_refuse_cycles_and_false_conditions() {
    for cyclic in [false, true] {
        let artifact = graph(23, cyclic, false);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &selections,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: selections.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let mut meter = TerminalFuelMeter::unbounded();
        let usage = meter.usage().clone();
        let evaluated = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            // A ready independent job precedes the cycle in request order.
            &[application(4), application(5)],
            &mut meter,
        );
        if cyclic {
            assert!(evaluated.err().unwrap().contains("dependency cycle"));
            assert_eq!(meter.usage(), &usage);
            for identity in 1..=5 {
                assert_eq!(observations.value(application(identity)).unwrap(), None);
            }
        } else {
            let error = evaluated
                .err()
                .expect("a false result predicate must reject the completed evaluation");
            assert!(error.contains("predicate is false"), "{error}");
            assert_eq!(
                observations.value(application(5)).unwrap(),
                Some(integer(24))
            );
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
                "a completed value with a false result promise cannot admit ordinary execution"
            );
        }
    }
}

#[test]
fn canonical_scalar_jobs_preserve_nonnegative_signed_extent_carriers() {
    let artifact = graph(24, false, true);
    let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
    let mut observations = ScalarApplicationObservations::new(
        artifact.view(),
        &selections,
        &providers,
        &terminal_installation::LayoutTargetFacts {
            profile: selections.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
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
    let mut observations = ScalarApplicationObservations::new(
        artifact.view(),
        &windows,
        &providers,
        &terminal_installation::LayoutTargetFacts {
            profile: windows.target().to_owned(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        },
    )
    .unwrap();
    observations.observe_layout(application(1)).unwrap();
    let program = TerminalProgram::borrow(artifact.view());
    for (selected, entry) in [
        (&macos, machine_id(1)),
        (&windows, machine_id(2)),
        (&windows, machine_id(1)),
    ] {
        let layouts = terminal_installation::admit_target_layouts(
            artifact.view(),
            selected,
            &layout_plans::LayoutTargetFacts {
                profile: selected.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let installation = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                entry: Some(TargetMachineCallable::Machine(entry)),
                installation: Some(&providers),
                target_machines: Some(selected),
                target_layouts: Some(&layouts),
                host_imports: &[],
                x86_flags: None,
                memory_ordering: None,
            })
            .unwrap();
        let prerequisites =
            terminal_installation::ScalarEntryPrerequisites::prepare(&observations, application(2))
                .unwrap();
        let admission = prerequisites.admit(&observations, None).unwrap();
        let result = program.instantiate(TerminalInstantiation {
            installation: Some(&installation),
            scalar_application: Some(TerminalScalarApplicationRequest {
                admission: &admission,
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
