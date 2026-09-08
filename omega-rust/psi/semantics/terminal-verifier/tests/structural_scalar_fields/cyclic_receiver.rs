use super::*;
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use terminal_psi::{ContractClause, SuccessorEdge};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, reconstruct_terminal_obligations,
    validate_module_for_interpretation, verify_module_for_interpretation,
};

#[path = "cyclic_receiver/mutation_facts.rs"]
mod mutation_facts;

fn boolean_read(operation: u64, source: u64, field: u64) -> Operation {
    Operation {
        id: id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(operation),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            source: id(source),
            field: id(field),
        },
    }
}

fn boolean_constant(operation: u64, value: bool) -> Operation {
    Operation {
        id: id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(operation),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value },
    }
}

fn store(operation: u64, destination: u64, value: u64) -> Operation {
    Operation {
        id: id(operation),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: id(destination),
            path: Vec::new(),
            field: id(1),
            value: id(value),
        },
    }
}

fn call() -> Operation {
    Operation {
        id: id(13),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: id(2),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(1),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn successor(edge: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(edge),
        target: id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn receiver_module(cyclic: bool) -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.structural_types.truncate(1);
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: (1..=4)
            .map(|field| StructuralFieldDeclaration {
                id: id(field),
                identity: format!("field_{field}"),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(if field <= 2 {
                    ScalarType::Boolean
                } else {
                    integer_type()
                }),
            })
            .collect(),
    };
    let caller = &mut module.machines[0];
    caller.blocks[0].operations = vec![
        boolean_read(10, 1, 1),
        boolean_constant(11, false),
        store(12, 1, 11),
        call(),
        boolean_read(14, 1, 1),
        boolean_read(17, 1, 2),
    ];
    for (operation, field) in [(15, 3), (16, 4)] {
        caller.blocks[0].operations.push(Operation {
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(operation),
                scalar_type: integer_type(),
            }),
            kind: OperationKind::IntegerStructuralField {
                source: id(1),
                field: id(field),
            },
        });
    }
    if cyclic {
        caller.blocks[0].terminator = Terminator::Conditional {
            condition: id(14),
            when_true: successor(1, 1),
            when_false: successor(3, 3),
        };
        caller.blocks.push(Block {
            id: id(3),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(4),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    let callee = &mut module.machines[1];
    callee.attachment = Some(id(1));
    callee.parameters.clear();
    callee.structural_parameters[0].structural_type = id(1);
    callee.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    callee.result = TerminalMachineResult::Unit;
    callee.blocks[0].operations = vec![boolean_constant(20, true), store(21, 2, 20)];
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(2),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn equation(value: u64, root: u64, field: u64) -> Proposition {
    Proposition::Equal(
        ScalarTerm::value(id(value), ScalarType::Boolean),
        ScalarTerm::boolean_field(id(root), id(field)),
    )
}

fn exit_axioms(module: &mut TerminalModule) -> Vec<Proposition> {
    let field = ScalarTerm::boolean_field(id(1), id(1));
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation: id(90),
        proposition: Proposition::Equal(field.clone(), field),
    }];
    reconstruct_terminal_obligations(module)
        .unwrap()
        .obligations()[0]
        .semantic_axioms
        .clone()
}

#[test]
fn mutable_receiver_cycles_keep_exact_frontiers_and_independent_field_identities() {
    let mut module = receiver_module(true);
    for reversed in [false, true] {
        if reversed {
            module.machines[0].blocks.reverse();
        }
        validate_module_for_interpretation(&module).unwrap();
        let verified = verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        let _ = verified;
        let frontiers =
            terminal_verifier::reconstruct_structural_ownership_frontiers(&module).unwrap();
        let caller = frontiers.machine(id(1)).unwrap();
        for block in &module.machines[0].blocks {
            let entry = caller.block_entry(block.id).unwrap();
            assert!(entry.claims().is_empty());
            assert!(entry.owned_places().is_empty());
            assert!(entry.partial_custody().is_empty());
            for operation in &block.operations {
                assert_eq!(caller.operation_entry(operation.id), Some(entry));
                assert_eq!(caller.operation_exit(operation.id), Some(entry));
            }
            for edge in block.terminator.edges() {
                assert_eq!(caller.edge_entry(edge), Some(entry));
                if matches!(block.terminator, Terminator::Conditional { .. }) {
                    assert_eq!(caller.edge_exit(edge), Some(entry));
                }
            }
        }
    }
}

#[test]
fn loop_cut_does_not_import_a_prefix_field_observation() {
    let mut module = receiver_module(true);
    let caller = &mut module.machines[0];
    caller.blocks[0]
        .operations
        .retain(|operation| !matches!(operation.id.get(), 10 | 12 | 13));
    caller.entry = id(4);
    caller.blocks.push(Block {
        id: id(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![boolean_read(10, 1, 1)],
        terminator: Terminator::Jump {
            edge: id(5),
            target: id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    });
    let axioms = exit_axioms(&mut module);
    assert!(!axioms.contains(&equation(10, 1, 1)));
    assert!(axioms.contains(&equation(14, 1, 1)));
}

#[test]
fn mutable_entry_requirements_are_not_loop_invariants() {
    let mut module = receiver_module(true);
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::boolean_field(id(1), id(1)),
            ScalarTerm::Boolean(true),
        ));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ControlCycle(_))
    ));
}

#[test]
fn receiver_cycles_reject_incompatible_access_and_malformed_claim_transfers() {
    for access in [StructuralAccess::Owned, StructuralAccess::WriteOnlyBorrow] {
        let mut module = receiver_module(true);
        module.machines[0].structural_parameters[0].access = access;
        assert!(validate_module(&module).is_err(), "{access:?}");
    }
    let mut module = receiver_module(true);
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    assert!(validate_module(&module).is_err());
    let mut module = receiver_module(true);
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    claim_transfers.push(terminal_psi::ClaimTransfer {
        claim: id(80),
        argument_index: 0,
    });
    assert!(matches!(validate_module(&module),
        Err(ModuleError::UnitCallClaimTransferCountMismatch { operation, expected: 0, actual: 1 })
            if operation == id(13)
    ));
}

#[test]
fn valid_callee_requirement_is_admitted_acyclically_but_fenced_in_cycles() {
    for cyclic in [false, true] {
        let mut module = receiver_module(cyclic);
        // A scalar entry requirement remains immutable across receiver writes.
        module.machines[0].parameters.push(ValueDeclaration {
            id: id(70),
            scalar_type: ScalarType::Boolean,
        });
        module.machines[1].parameters.push(ValueDeclaration {
            id: id(71),
            scalar_type: ScalarType::Boolean,
        });
        let caller_requirement = Proposition::Equal(
            ScalarTerm::value(id(70), ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        );
        module.machines[0].contract.requires = vec![caller_requirement.clone()];
        module.machines[1].contract.requires = vec![Proposition::Equal(
            ScalarTerm::value(id(71), ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        )];
        let OperationKind::CallUnit {
            arguments,
            requirement_obligations,
            ..
        } = &mut module.machines[0].blocks[0].operations[3].kind
        else {
            unreachable!()
        };
        arguments.push(id(70));
        requirement_obligations.push(id(80));
        if cyclic {
            assert!(matches!(
                validate_module(&module),
                Err(ModuleError::ControlCycle(_))
            ));
        } else {
            let bundle = ProofBundle {
                evidence: vec![certificate(
                    80,
                    caller_requirement,
                    ProofRule::Assumption { index: 0 },
                )],
                ..ProofBundle::default()
            };
            verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default())
                .expect("the exact call and requirement certificate verify without the cycle");
        }
    }
}

#[test]
fn receiver_cycles_reject_stale_store_operands_identity_drift_and_fake_cleanup() {
    let mut module = receiver_module(true);
    module.machines[0].blocks[0].operations.swap(1, 2);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ValueUsedBeforeDefinition(_))
    ));
    let mut module = receiver_module(true);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = id(2);
    assert!(validate_module(&module).is_err());
    let mut module = receiver_module(true);
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.trivial_affine_discards.push(id(1));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::EdgeAffineDiscardsInvalid { .. })
    ));
}

#[test]
fn projected_mutable_unit_call_can_return_a_caller_scalar_observation() {
    let mut module = structural_scalar_field_module();
    let mut writer = module.machines[1].clone();
    writer.id = id(3);
    writer.contract = contract(3);
    writer.result = TerminalMachineResult::Unit;
    writer.parameters.clear();
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    writer.structural_parameters[0].place = id(3);
    writer.structural_places = vec![structural_place(id(3))];
    writer.blocks[0].id = id(3);
    writer.entry = id(3);
    writer.blocks[0].operations.clear();
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(3),
        trivial_affine_discards: Vec::new(),
    };
    let mut invoke = call();
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        ..
    } = &mut invoke.kind
    else {
        unreachable!()
    };
    *callee = id(3);
    structural_arguments[0].path = vec![StructuralPathSegment::Field("item".into())];
    module.machines[0].blocks[0].operations.insert(2, invoke);
    module.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: id(30),
        scalar_type: integer_type(),
    });
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: id(1),
        value: id(2),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(writer);
    validate_module(&module).expect("exclusive projection carries no caller result custody");
}

fn certificate(obligation: u64, conclusion: Proposition, rule: ProofRule) -> ObligationEvidence {
    ObligationEvidence {
        obligation: id(obligation),
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(obligation),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode { conclusion, rule },
        }),
    }
}
