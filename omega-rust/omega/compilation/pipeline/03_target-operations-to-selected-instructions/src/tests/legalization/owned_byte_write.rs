//! An indexed write retains its owned backing, live extent and declared byte goal.
use abstract_operations::{AbstractOperation as Operation, AbstractResult};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ObligationId,
    OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use target::NativeTarget;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}
fn operation(ordinal: u64) -> OperationId {
    OperationId::new(ordinal).unwrap()
}
fn place(ordinal: u64) -> PlaceId {
    PlaceId::new(ordinal).unwrap()
}
fn scalar(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn fixture(
    native: NativeTarget,
    qualified: bool,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    fixture_with_presentation(native, qualified, false)
}

fn fixture_with_presentation(
    native: NativeTarget,
    qualified: bool,
    borrowed_view: bool,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    fixture_with_call(native, qualified, borrowed_view, false, false)
}

fn fixture_with_call(
    native: NativeTarget,
    qualified: bool,
    borrowed_view: bool,
    installed_boundary: bool,
    owned_consumer: bool,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let literal = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "literal".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
    };
    let owned = StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: "owned".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    };
    source.structural_types = vec![literal.clone(), owned.clone()].into();
    let mut operations = Vec::new();
    for (ordinal, bytes) in [(1, b"ab".to_vec()), (2, Vec::new())] {
        operations.push(Operation::EstablishByteSequenceLiteral {
            psi_operation: operation(ordinal * 2 - 1),
            place: StructuralPlaceDeclaration {
                id: place(ordinal),
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: (ordinal - 1) as u32,
                    structural_type: literal.id,
                },
            },
            structural_type: literal.clone(),
            bytes,
            qualifications: Vec::new(),
        });
        operations.push(Operation::ByteSequenceLength {
            psi_operation: operation(ordinal * 2),
            result: AbstractResult {
                value: value(ordinal),
                scalar_type: scalar(64),
            },
            source: place(ordinal),
        });
    }
    let operand = |ordinal| terminal_psi::control_flow::ByteSequenceOperand::Whole {
        source: place(ordinal),
        length: value(ordinal),
    };
    operations.extend([
        Operation::EstablishByteSequenceConcatenation {
            psi_operation: operation(5),
            result: StructuralOperationResult {
                place: place(3),
                structural_type: owned.id,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: if qualified {
                    vec![semantic_vocabulary::StructuralDomainId::new(1).unwrap()]
                } else {
                    Vec::new()
                },
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            left: operand(1),
            right: operand(2),
            capacity_obligation: ObligationId::new(1).unwrap(),
        },
        Operation::ByteSequenceLength {
            psi_operation: operation(6),
            source: place(3),
            result: AbstractResult {
                value: value(3),
                scalar_type: scalar(64),
            },
        },
        Operation::IntegerConstant {
            psi_operation: operation(7),
            result: value(4),
            scalar_type: scalar(64),
            value: IntegerValue::Unsigned(0),
        },
        Operation::IntegerConstant {
            psi_operation: operation(8),
            result: value(5),
            scalar_type: scalar(8),
            value: IntegerValue::Unsigned(0),
        },
        Operation::ByteSequenceWrite {
            psi_operation: operation(9),
            destination: place(3),
            index: value(4),
            value: value(5),
            length: value(3),
            obligation: ObligationId::new(2).unwrap(),
        },
    ]);
    operations.extend(source.functions[0].operations.clone());
    source.functions[0].operations = operations;
    if qualified && !owned_consumer {
        let mut callee = source.functions[0].clone();
        callee.machine = semantic_vocabulary::MachineId::new(2).unwrap();
        callee.entry = semantic_vocabulary::BlockId::new(2).unwrap();
        callee.block_entries[0].block = callee.entry;
        callee.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
            place: place(4),
            position: 0,
            is_self: false,
            structural_type: if borrowed_view { literal.id } else { owned.id },
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: if borrowed_view {
                Vec::new()
            } else {
                vec![semantic_vocabulary::StructuralDomainId::new(1).unwrap()]
            },
            projected_qualifications: Vec::new(),
        }];
        callee.operations = vec![Operation::ReturnUnit {
            psi_edge: semantic_vocabulary::EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        }];
        let caller = &mut source.functions[0];
        caller.operations.insert(
            caller.operations.len() - 1,
            Operation::CallStructural {
                psi_operation: operation(10),
                callee: callee.machine,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: place(3),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                dynamic_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: abstract_operations::AbstractStructuralCallResult::Unit,
            },
        );
        source.functions.push(callee);
    }
    if owned_consumer {
        assert!(!borrowed_view && !installed_boundary);
        let mut callee = source.functions[0].clone();
        callee.machine = semantic_vocabulary::MachineId::new(2).unwrap();
        callee.entry = semantic_vocabulary::BlockId::new(2).unwrap();
        callee.block_entries[0].block = callee.entry;
        callee.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
            place: place(4),
            position: 0,
            is_self: false,
            structural_type: owned.id,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        callee.operations = vec![Operation::ReturnUnit {
            psi_edge: semantic_vocabulary::EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        }];
        let caller = &mut source.functions[0];
        caller.operations.insert(
            caller.operations.len() - 1,
            Operation::CallStructural {
                psi_operation: operation(10),
                callee: callee.machine,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: place(3),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                dynamic_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: abstract_operations::AbstractStructuralCallResult::Unit,
            },
        );
        source.functions.push(callee);
    }
    let installation = installed_boundary.then(|| install_boundary_call(&mut source));
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            installation: installation.as_ref().map(|installation| {
                installation as &dyn installation_evidence::ProviderInstallationEvidence
            }),
            ..abstract_operations_to_target_operations::TargetLoweringRequest::new(native)
        },
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if qualified {
        unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
            id: semantic_vocabulary::StructuralDomainId::new(1).unwrap(),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "owned-utf8".into(),
            carrier: owned.id,
            byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
            content_projection: None,
            establishment_routes: Vec::new(),
        }]
        .into();
    }
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let measured = |ordinal| IntegerMathTerm::MathValue {
        source_type: integer,
        value: value(ordinal),
    };
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    if qualified {
        predicates.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    }
    let write_goal = terminal_semantics::indexed_byte_store_goal(
        Proposition::LessThan(
            ScalarTerm::value(value(4), scalar(64)),
            ScalarTerm::value(value(3), scalar(64)),
        ),
        value(5),
        predicates,
    )
    .unwrap();
    let goals = [
        (
            5,
            1,
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(measured(1)), Box::new(measured(2))),
                IntegerMathTerm::literal(IntegerValue::Unsigned(8)),
            ),
        ),
        (9, 2, write_goal),
    ];
    let facts = goals
        .into_iter()
        .map(|(support, obligation, goal)| {
            optimization_unit::AcceptedObligationFact::new(
                unit.psi,
                [4; 32],
                source.functions[0].machine,
                operation(support),
                ObligationId::new(obligation).unwrap(),
                terminal_codec::canonical_proposition_order_key(&goal).unwrap(),
            )
        })
        .collect();
    let unit = optimization_unit::attach_accepted_obligation_facts(unit, facts).unwrap();
    (source, target, unit)
}

#[derive(Debug)]
struct InstalledBoundaryCall {
    psi: terminal_psi::TerminalPsiIdentity,
    call: installation_evidence::InstalledProviderCallEvidence,
}

impl installation_evidence::ProviderInstallationEvidence for InstalledBoundaryCall {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_calls(
        &self,
    ) -> Vec<installation_evidence::InstalledProviderCallEvidence> {
        vec![self.call.clone()]
    }
}

fn install_boundary_call(
    source: &mut abstract_operations::AbstractOperationPlan,
) -> InstalledBoundaryCall {
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    let parameter = source.functions[1].structural_parameters[0].clone();
    let provider = terminal_psi::ProviderCandidateConformance {
        boundary,
        requirement_identity: "Bytes::observe".into(),
        provider_identity: "ByteObserver".into(),
        candidate_identity: "ByteObserver::observe".into(),
        candidate: source.functions[1].machine,
        signature: terminal_psi::ProviderSignature {
            parameters: vec![terminal_psi::ProviderSignatureParameter {
                position: parameter.position,
                is_self: parameter.is_self,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                qualifications: parameter.qualifications.clone(),
                projected_qualifications: parameter.projected_qualifications.clone(),
            }],
        },
        refinement: terminal_psi::ProviderRefinement {
            positional_parameters: vec![terminal_psi::ProviderParameterRefinement {
                boundary_index: 0,
                candidate_index: 0,
            }],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: provider.requirement_identity.clone(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        });
    source.provider_candidates.push(provider.clone());
    let caller = &mut source.functions[0];
    let row = caller
        .operations
        .iter_mut()
        .find(|row| matches!(row, Operation::CallStructural { .. }))
        .unwrap();
    let Operation::CallStructural {
        psi_operation,
        structural_arguments,
        ..
    } = row
    else {
        unreachable!();
    };
    let call = installation_evidence::InstalledProviderCallEvidence {
        caller: caller.machine,
        psi_operation: *psi_operation,
        boundary,
        provider,
        result: terminal_psi::OperationResult::Unit,
        scalar_arguments: Vec::new(),
        structural_arguments: structural_arguments.clone(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    *row = Operation::BoundaryCall {
        psi_operation: call.psi_operation,
        boundary,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        arguments: Vec::new(),
        structural_arguments: call.structural_arguments.clone(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    InstalledBoundaryCall {
        psi: source.psi,
        call,
    }
}

#[test]
fn owned_byte_write_lends_live_bytes_through_installed_boundary() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, target, unit) = fixture_with_call(native, true, true, true, false);
        let legalized = crate::legalize_target_operations(&target, &source, &unit)
            .expect("installed provider retains the authored boundary call and owned home");
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        for mutation in 0..2 {
            let mut changed = target.clone();
            let row = changed.functions[0]
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|row| matches!(row, target_operations::TargetUnitOperation::Call { .. }))
                .unwrap();
            let target_operations::TargetUnitOperation::Call {
                psi_operation,
                arguments,
                ..
            } = row
            else {
                unreachable!();
            };
            if mutation == 0 {
                *psi_operation = operation(9);
            } else {
                arguments[0].place = place(1);
            }
            assert!(crate::legalize_target_operations(&changed, &source, &unit).is_err());
        }
    }
}

#[test]
fn concatenated_owned_bytes_feed_an_owned_call_from_their_exact_prior_home() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, target, unit) = fixture_with_call(native, true, false, false, true);
        let legalized = crate::legalize_target_operations(&target, &source, &unit)
            .expect("the completed concatenation owns the call operand home");
        crate::validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
            .expect("independent replay retains the concatenation producer");

        let mut missing_required = unit.clone();
        let domain = semantic_vocabulary::StructuralDomainId::new(2).unwrap();
        let carrier = missing_required
            .structural_types
            .iter()
            .find(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                )
            })
            .unwrap()
            .id;
        let mut domains = missing_required.structural_domains.to_vec();
        domains.push(terminal_psi::StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
            identity: "owned-nonempty".into(),
            carrier,
            byte_predicate: Some(terminal_psi::ByteSequencePredicate::NonEmpty),
            content_projection: None,
            establishment_routes: Vec::new(),
        });
        missing_required.structural_domains = domains.into();
        missing_required.functions[1].structural_parameters[0]
            .qualifications
            .push(domain);
        missing_required.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&missing_required);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&missing_required).is_err(),
            "{native:?}: an owned formal cannot manufacture its required predicate"
        );

        let call_argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|row| match row {
                target_operations::TargetUnitOperation::Call {
                    psi_operation,
                    arguments,
                    ..
                } if *psi_operation == operation(10) => arguments.first(),
                _ => None,
            })
            .expect("owned call argument");
        assert_eq!(
            call_argument.source,
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: operation(5),
            }
        );

        let mut forged = target.clone();
        let argument = forged.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|row| match row {
                target_operations::TargetUnitOperation::Call {
                    psi_operation,
                    arguments,
                    ..
                } if *psi_operation == operation(10) => arguments.first_mut(),
                _ => None,
            })
            .expect("owned call argument");
        argument.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: operation(4),
        };
        assert!(
            crate::legalize_target_operations(&forged, &source, &unit).is_err(),
            "{native:?}: a different producer cannot own the operand home"
        );

        let mut reordered = target.clone();
        let operations = &mut reordered.functions[0].graph.blocks[0].operations;
        let concatenation = operations
            .iter()
            .position(|row| {
                matches!(row,
                    target_operations::TargetUnitOperation::EstablishByteSequenceConcatenation {
                        psi_operation, ..
                    } if *psi_operation == operation(5))
            })
            .expect("concatenation");
        let call = operations
            .iter()
            .position(|row| {
                matches!(row,
                    target_operations::TargetUnitOperation::Call { psi_operation, .. }
                        if *psi_operation == operation(10))
            })
            .expect("owned call");
        operations.swap(concatenation, call);
        assert!(
            crate::legalize_target_operations(&reordered, &source, &unit).is_err(),
            "{native:?}: the consumer cannot execute before its producer"
        );
    }
}

#[test]
fn owned_byte_write_lends_live_owned_bytes_as_shared_view() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, target, unit) = fixture_with_presentation(native, true, true);
        let legalized = crate::legalize_target_operations(&target, &source, &unit)
            .expect("owned bytes lend their actual home through a shared view");
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("shared descriptor uses the owned live length rather than capacity");
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
    }
}

#[test]
fn owned_byte_write_retains_place_and_bound_through_native_selection() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, target, unit) = fixture(native, false);
        let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
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
        for mutation in 0..3 {
            let mut changed = target.clone();
            let write = changed.functions[0]
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|row| {
                    matches!(
                        row,
                        target_operations::TargetUnitOperation::ByteSequenceWrite { .. }
                    )
                })
                .unwrap();
            let target_operations::TargetUnitOperation::ByteSequenceWrite {
                destination,
                length,
                obligation,
                ..
            } = write
            else {
                unreachable!()
            };
            match mutation {
                0 => *destination = place(1),
                1 => *length = value(1),
                _ => *obligation = ObligationId::new(1).unwrap(),
            }
            assert!(
                crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                "mutation {mutation}"
            );
        }
        let mut missing = unit.clone();
        missing.accepted_obligation_facts.clear();
        missing.identity = optimization_unit::recompute_psi_optimization_unit_identity(&missing);
        assert!(crate::legalize_target_operations(&target, &source, &missing).is_err());
    }
}

#[test]
fn owned_byte_write_lends_qualified_owned_home() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, target, unit) = fixture(native, true);
        let legalized = crate::legalize_target_operations(&target, &source, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("qualified bounded-owner formal borrows its original qualified home");
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
    }
}

#[test]
fn owned_byte_write_replays_declared_predicate_in_addition_to_bound() {
    let (source, target, mut unit) = fixture(NativeTarget::linux_x64(), true);
    crate::legalize_target_operations(&target, &source, &unit).unwrap();
    let mut facts = unit.accepted_obligation_facts.to_vec();
    let bound_only = Proposition::LessThan(
        ScalarTerm::value(value(4), scalar(64)),
        ScalarTerm::value(value(3), scalar(64)),
    );
    facts.retain(|fact| fact.operation != operation(9));
    facts.push(optimization_unit::AcceptedObligationFact::new(
        unit.psi,
        [4; 32],
        source.functions[0].machine,
        operation(9),
        ObligationId::new(2).unwrap(),
        terminal_codec::canonical_proposition_order_key(&bound_only).unwrap(),
    ));
    unit.accepted_obligation_facts.clear();
    let changed = optimization_unit::attach_accepted_obligation_facts(unit, facts).unwrap();
    assert!(crate::legalize_target_operations(&target, &source, &changed).is_err());
}
