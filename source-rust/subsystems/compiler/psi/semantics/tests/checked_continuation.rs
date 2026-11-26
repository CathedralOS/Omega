use semantic_vocabulary::{BlockId, MachineId, OperationId};
use semantic_vocabulary::{
    ContractId, EdgeId, ScalarQualificationSetId, ScalarType, SuspensionCrossingId, ValueId,
};
use terminal_psi::{Block, Operation, OperationKind, TerminalMachine, TerminalModule, Terminator};
use terminal_psi::{
    CheckedMachineDeclaration, MachineContract, NominalDeclarationIdentity, OperationResult,
    TerminalIdentityIndex, ValueDeclaration,
};
use terminal_semantics::continuation::CheckedPrefix;

fn machine(id: MachineId, identity: TerminalIdentityIndex) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
    contract.declaration = Some(CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(identity),
        state: NominalDeclarationIdentity::new(identity),
        semantic_declaration: identity,
        parameter_types: Vec::new(),
        callable: identity,
        result_type: identity,
        contract: [0; 32],
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    });
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id,
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract,
    }
}
fn constant(value: bool, id: u64) -> Operation {
    Operation::unsuspended(
        OperationId::new(id).unwrap(),
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(id).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        }),
        OperationKind::BooleanConstant { value },
    )
}

fn program() -> (TerminalModule, (MachineId, OperationId)) {
    let entry = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let cut = OperationId::new(1).unwrap();
    let mut previous = TerminalModule::for_entry(entry);
    let mut names = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
    let mut root = machine(entry, names.intern("entry"));
    let mut helper = machine(callee, names.intern("helper"));
    previous.identities = names.build();
    let mut call = Operation::unsuspended(
        cut,
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    call.suspension_crossing = Some(SuspensionCrossingId::new(1).unwrap());
    root.blocks[0].operations.push(call);
    helper.blocks[0].operations.push(constant(false, 1));
    previous
        .execution
        .suspension_call_plans
        .push(terminal_psi::TerminalSuspensionCallPlan {
            operation: cut,
            crossing: SuspensionCrossingId::new(1).unwrap(),
            target: terminal_psi::TerminalSuspensionCallTarget::Machine(callee),
            live_value_count: 0,
            live_values: Vec::new(),
            loans: Default::default(),
        });
    previous.machines = vec![root, helper];
    (previous, (entry, cut))
}

#[test]
fn a_successor_can_change_its_suffix_but_must_preserve_completed_calls() {
    let (previous, site) = program();
    let mut next = previous.clone();
    next.machines[0].blocks[0]
        .operations
        .push(constant(true, 2));
    let certificate = CheckedPrefix::check(&previous, site, &next, site).unwrap();
    assert!(certificate.accepts(&previous, &next));
    assert!(!certificate.accepts(&previous.clone(), &next));
    next.machines[1].blocks[0].operations[0] = constant(true, 1);
    assert!(CheckedPrefix::check(&previous, site, &next, site).is_err());
}

#[test]
fn an_equal_call_operand_cannot_name_a_different_nominal_declaration() {
    let (mut previous, site) = program();
    let mut next = previous.clone();
    let mut old_names = std::mem::take(&mut previous.identities).into_builder();
    let a = old_names.intern("first::state");
    previous.identities = old_names.build();
    let mut new_names = std::mem::take(&mut next.identities).into_builder();
    let b = new_names.intern("second::state");
    next.identities = new_names.build();
    assert_eq!(a, b);
    let checked = CheckedPrefix::check(&previous, site, &next, site).unwrap();
    assert_eq!(checked.field_identity(a), None);
}

#[test]
fn equal_storage_numbers_do_not_transfer_another_scalar_domain() {
    use semantic_vocabulary::{
        DomainSemanticId, QualifiedScalarType, ScalarDomainId, StructuralTypeId,
    };
    use terminal_psi::{
        ScalarDomainDeclaration, ScalarPrimitiveQualification, ScalarQualificationSet,
        StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let storage = StructuralTypeId::new(1).unwrap();
    let domain = ScalarDomainId::new(1).unwrap();
    let set = ScalarQualificationSetId::new(1);
    let make = |issuer: &str, storage| {
        let (mut module, _) = program();
        let mut names = std::mem::take(&mut module.identities).into_builder();
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                quotient_carrier: None,
                id: storage,
                identity: names.intern("bool"),
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                scalar_restriction: None,
                requires: Vec::new(),
            });
        let scalar = &mut module.types.scalar_qualifications;
        scalar.primitives.push(ScalarPrimitiveQualification {
            structural_type: storage,
            value_type: QualifiedScalarType {
                scalar_type: ScalarType::Boolean,
                qualifications: set,
            },
        });
        scalar.sets.push(ScalarQualificationSet {
            id: set,
            domains: vec![domain],
        });
        scalar.domains.push(ScalarDomainDeclaration {
            required_memberships: None,
            predicate: None,
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: names.intern(issuer),
            carrier: ScalarType::Boolean,
            establishment_routes: None,
        });
        module.identities = names.build();
        module
    };
    let (_, site) = program();
    let old = make("First", storage);
    let new_tag = StructuralTypeId::new(2).unwrap();
    for (new, expected) in [
        (make("First", new_tag), Some(new_tag)),
        (make("Different", storage), None),
    ] {
        let proof = CheckedPrefix::check(&old, site, &new, site).unwrap();
        assert_eq!(proof.structural_type(storage), expected);
    }
}

#[test]
fn a_renamed_root_record_keeps_its_exact_fields_and_changed_records_do_not_rejoin() {
    use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
    };
    fn gauge(
        tag: u64,
        prefix: &str,
        occurrence: Option<u8>,
    ) -> (TerminalModule, TerminalIdentityIndex) {
        let (mut module, _) = program();
        let mut names = std::mem::take(&mut module.identities).into_builder();
        let id = StructuralTypeId::new(tag).unwrap();
        if tag != 1 {
            names.intern("newly mounted declaration");
        }
        let amount = names.intern("amount");
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: amount,
            declaration_identity: names.intern(&format!("{prefix}Gauge::amount")),
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            storage: None,
            inline_carrier: None,
            relevance: Default::default(),
        };
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                id,
                identity: names.intern(&format!("{prefix}Gauge storage")),
                declaration_identity: names.intern(&format!("{prefix}Gauge")),
                shape: StructuralTypeShape::Record {
                    fields: vec![field],
                    storage: StructuralRecordStorage::Packed,
                },
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                quotient_carrier: None,
                nominal_cleanup: None,
                scalar_restriction: None,
                requires: Vec::new(),
            });
        names
            .retain_invocation_record(id, None, occurrence.map(|x| [x; 32]))
            .unwrap();
        module.identities = names.build();
        (module, amount)
    }
    let (_, site) = program();
    let (old, old_field) = gauge(1, "", Some(23));
    let (new, new_field) = gauge(2, "mounted::", Some(23));
    assert_ne!(old_field, new_field);
    let predecessor = StructuralTypeId::new(1).unwrap();
    let successor = StructuralTypeId::new(2).unwrap();
    let proof = CheckedPrefix::check(&old, site, &new, site).unwrap();
    assert_eq!(proof.structural_type(predecessor), Some(successor));
    assert_eq!(proof.field_identity(old_field), Some(new_field));
    let (foreign, _) = gauge(2, "", Some(42));
    let proof = CheckedPrefix::check(&old, site, &foreign, site).unwrap();
    assert_eq!(proof.structural_type(predecessor), None);
    let (unidentified, _) = gauge(1, "", None);
    let proof = CheckedPrefix::check(&unidentified, site, &foreign, site).unwrap();
    assert_eq!(proof.structural_type(predecessor), None);
}

#[test]
fn a_prepare_in_a_helper_attests_the_callers_return_site_and_the_helpers_cut() {
    use terminal_semantics::continuation::PrefixCall;
    let (mut old, entry_site) = program();
    let helper = MachineId::new(2).unwrap();
    let prepare = MachineId::new(3).unwrap();
    let second = OperationId::new(2).unwrap();
    let block = BlockId::new(1).unwrap();
    let crossing = SuspensionCrossingId::new(2).unwrap();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    old.machines.push(machine(prepare, names.intern("prepare")));
    old.identities = names.build();
    let mut nested = old.machines[0].blocks[0].operations[0].clone();
    let OperationKind::CallUnit { callee, .. } = &mut nested.kind else {
        unreachable!()
    };
    *callee = prepare;
    nested.id = second;
    nested.suspension_crossing = Some(crossing);
    old.machines[1].blocks[0].operations.push(nested);
    let mut plan = old.execution.suspension_call_plans[0].clone();
    plan.target = terminal_psi::TerminalSuspensionCallTarget::Machine(prepare);
    plan.operation = second;
    plan.crossing = crossing;
    old.execution.suspension_call_plans.push(plan);
    let mut new = old.clone();
    new.machines[1].blocks[0].operations.push(constant(true, 3));
    let outer = PrefixCall {
        machine: entry_site.0,
        operation: entry_site.1,
        block,
        index: 0,
    };
    let inner = PrefixCall {
        machine: helper,
        operation: second,
        block,
        index: 1,
    };
    let leaf = (helper, second);
    let proof = CheckedPrefix::check_stack(&old, &[outer, inner], &new, leaf).unwrap();
    assert_eq!(proof.stack(), &[(outer, outer), (inner, inner)]);
    assert_eq!(proof.old_call(), inner);
    assert!(CheckedPrefix::check_stack(&old, &[inner], &new, leaf).is_err());
    assert!(
        CheckedPrefix::check_stack(&old, &[outer, PrefixCall { index: 0, ..inner }], &new, leaf)
            .is_err()
    );
    new.machines[1].blocks[0].operations[0] = constant(true, 1);
    assert!(CheckedPrefix::check_stack(&old, &[outer, inner], &new, leaf).is_err());
}

#[test]
fn a_saved_return_cannot_gain_another_parameter_as_its_borrow_source() {
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        BoundaryParameterKind, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralReferenceResultSource,
        StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalMachineResult,
    };
    let (mut old, cut) = program();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let access = StructuralAccess::SharedBorrow;
    let multiplicity = StructuralMultiplicity::Unrestricted;
    for (id, name, shape) in [
        (
            boolean,
            "bool",
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        (
            reference,
            "@bool",
            StructuralTypeShape::Reference {
                referent: boolean,
                access,
            },
        ),
    ] {
        old.types.structural_types.push(StructuralTypeDeclaration {
            id,
            identity: names.intern(name),
            declaration_identity: Default::default(),
            shape,
            multiplicity,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            quotient_carrier: None,
            nominal_cleanup: None,
            scalar_restriction: None,
            requires: Vec::new(),
        });
    }
    old.identities = names.build();
    let entry = &mut old.machines[0];
    let first = PlaceId::new(1).unwrap();
    let second = PlaceId::new(2).unwrap();
    for (position, place) in [(0, first), (1, second)] {
        entry
            .contract
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                structural_type: reference,
                multiplicity,
                access: StructuralAccess::Owned,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: vec![],
            });
        entry
            .contract
            .parameter_order
            .push(BoundaryParameterKind::Structural);
    }
    let origin = |place| StructuralArgument::whole(place, access).into();
    entry.contract.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: PlaceId::new(3).unwrap(),
        structural_type: reference,
        multiplicity,
        qualifications: vec![],
        projected_qualifications: vec![],
        reference_sources: vec![StructuralReferenceResultSource {
            path: vec![],
            sources: vec![origin(first)],
        }],
    });
    entry.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: first,
        returned_claims: vec![],
        cleanup_actions: vec![],
    };
    let mut new = old.clone();
    assert!(CheckedPrefix::check(&old, cut, &new, cut).is_ok());
    let TerminalMachineResult::Structural(result) = &mut new.machines[0].contract.result else {
        unreachable!()
    };
    result.reference_sources[0].sources.push(origin(second));
    let error = CheckedPrefix::check(&old, cut, &new, cut).err().unwrap();
    assert_eq!(
        error,
        "successor changed a reached machine's return contract"
    );
}

#[test]
fn a_repeated_return_site_preserves_its_previous_iterations() {
    let (mut previous, cut) = program();
    let block = &mut previous.machines[0].blocks[0];
    block.operations.push(constant(false, 2));
    block.terminator = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: block.id,
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        cleanup_actions: vec![],
    };
    let mut new = previous.clone();
    assert!(CheckedPrefix::check(&previous, cut, &new, cut).is_ok());
    new.machines[0].blocks[0].operations[1] = constant(true, 2);
    assert!(CheckedPrefix::check(&previous, cut, &new, cut).is_err());
}

fn program_with_live_byte_literal() -> (
    TerminalModule,
    (MachineId, OperationId),
    semantic_vocabulary::PlaceId,
) {
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalSuspensionLiveValue, TerminalSuspensionPlace, TerminalSuspensionStorage,
        TerminalSuspensionValueLoans, TerminalSuspensionValueType,
    };
    let (mut old, cut) = program();
    let ty = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    old.types.structural_types.push(StructuralTypeDeclaration {
        id: ty,
        identity: names.intern("shared byte view"),
        declaration_identity: Default::default(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
        multiplicity: StructuralMultiplicity::Unrestricted,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        quotient_carrier: None,
        nominal_cleanup: None,
        scalar_restriction: None,
        requires: Vec::new(),
    });
    old.identities = names.build();
    old.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: ty,
            },
        });
    let literal = Operation::unsuspended(
        OperationId::new(2).unwrap(),
        OperationResult::Unit,
        OperationKind::EstablishByteSequenceLiteral {
            destination: place,
            bytes: vec![0xff, 0],
            qualifications: vec![],
        },
    );
    old.machines[0].blocks[0].operations.insert(0, literal);
    let frontier = &mut old.execution.suspension_call_plans[0];
    frontier.live_value_count = 1;
    frontier.live_values.push(TerminalSuspensionLiveValue {
        place: TerminalSuspensionPlace::Structural(StructuralArgument {
            place,
            path: vec![],
            access: StructuralAccess::Owned,
        }),
        value_type: TerminalSuspensionValueType::Structural(ty),
        storage: TerminalSuspensionStorage::Local,
        claim_count: 0,
        claims: vec![],
        loans: TerminalSuspensionValueLoans::Unresolved,
    });
    (old, cut, place)
}

#[test]
fn two_arguments_lending_the_same_place_have_two_suspension_rows() {
    use terminal_psi::{
        StructuralAccess, StructuralArgument, TerminalSuspensionPlace, TerminalSuspensionStorage,
    };
    use terminal_semantics::suspension_frontier::MachineFrontiers;

    let (mut module, (_, call), place) = program_with_live_byte_literal();
    let argument = StructuralArgument::whole(place, StructuralAccess::SharedBorrow);
    let operation = module.machines[0].blocks[0]
        .operations
        .iter_mut()
        .find(|operation| operation.id == call)
        .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut operation.kind
    else {
        panic!("the fixture must park at a call");
    };
    structural_arguments.extend([argument.clone(), argument.clone()]);
    let crossing = operation.suspension_crossing.unwrap();
    let plan = MachineFrontiers::new(&module, &module.machines[0])
        .call_plan(call, crossing)
        .unwrap();
    let rows: Vec<_> = plan
        .live_values
        .iter()
        .filter(|row| row.storage == TerminalSuspensionStorage::CallArgument)
        .map(|row| &row.place)
        .collect();
    let lent = TerminalSuspensionPlace::Structural(argument);
    assert_eq!(rows, [&lent, &lent]);
}

#[test]
fn a_live_byte_literal_requires_its_unique_dominating_image_backing() {
    use terminal_psi::{StructuralAccess, TerminalSuspensionPlace};
    let (mut old, cut, place) = program_with_live_byte_literal();
    assert!(CheckedPrefix::check(&old, cut, &old, cut).is_ok());
    let mut parked = old.clone();
    let block = &mut parked.machines[0].blocks[0];
    let stop = (block.id, u32::try_from(block.operations.len()).unwrap());
    block.terminator = terminal_psi::Terminator::PendingNormalCompletion {
        call: cut.1,
        scalars: Vec::new(),
        storage: vec![place],
    };
    let graph = terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from(
        &parked,
        [cut.0],
        &[],
    )
    .unwrap();
    let entry = graph.iter().find(|machine| machine.id == cut.0).unwrap();
    assert!(
        entry.flow[&stop]
            .observed_inputs()
            .any(|input| input.root == place && input.path.is_empty())
    );
    assert!(!entry.normal_exits.contains(&stop));
    let mut write_only = old.clone();
    let TerminalSuspensionPlace::Structural(arg) =
        &mut write_only.execution.suspension_call_plans[0].live_values[0].place
    else {
        unreachable!()
    };
    arg.access = StructuralAccess::WriteOnlyBorrow;
    assert!(CheckedPrefix::check(&write_only, cut, &write_only, cut).is_err());
    let mut duplicate = old.clone();
    let mut duplicate_literal = duplicate.machines[0].blocks[0].operations[0].clone();
    duplicate_literal.id = OperationId::new(3).unwrap();
    duplicate.machines[0].blocks[0]
        .operations
        .push(duplicate_literal);
    assert!(CheckedPrefix::check(&old, cut, &duplicate, cut).is_err());
    old.machines[0].blocks[0].operations.swap(0, 1);
    assert!(CheckedPrefix::check(&old, cut, &old, cut).is_err());
}

#[test]
fn a_completed_byte_read_keeps_its_source_index_length_and_bounds_obligation() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ObligationId, PlaceId};
    use terminal_psi::{StructuralAccess, StructuralArgument};

    let (mut old, cut, place) = program_with_live_byte_literal();
    let scalar = |id, width, kind| {
        Operation::unsuspended(
            OperationId::new(id).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(id).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
                ),
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            kind,
        )
    };
    let index = |id, value| {
        scalar(
            id,
            64,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value),
            },
        )
    };
    let first = ValueId::new(3).unwrap();
    let second = ValueId::new(4).unwrap();
    let count = ValueId::new(5).unwrap();
    let bounds = ObligationId::new(1).unwrap();
    let read = |source, index, length, obligation| OperationKind::ByteSequenceRead {
        source,
        index,
        length,
        obligation,
    };
    old.machines[0].blocks[0].operations.splice(
        1..1,
        [
            index(3, 0),
            index(4, 1),
            scalar(
                5,
                64,
                OperationKind::ByteSequenceLength {
                    source: StructuralArgument {
                        place,
                        path: vec![],
                        access: StructuralAccess::SharedBorrow,
                    },
                },
            ),
            scalar(6, 8, read(place, first, count, bounds)),
        ],
    );

    CheckedPrefix::check(&old, cut, &old, cut).unwrap();
    for changed in [
        read(place, second, count, bounds),
        read(PlaceId::new(2).unwrap(), first, count, bounds),
        read(place, first, second, bounds),
        read(place, first, count, ObligationId::new(2).unwrap()),
    ] {
        let mut next = old.clone();
        next.machines[0].blocks[0].operations[4].kind = changed;
        assert_eq!(
            CheckedPrefix::check(&old, cut, &next, cut).err(),
            Some("successor changed an operation or its result in the executed prefix")
        );
    }
}

#[test]
fn only_the_active_root_may_add_previously_unstated_service_reach() {
    use semantic_vocabulary::ServiceId;
    use terminal_psi::ServiceDeclaration;
    let (mut old, cut) = program();
    let root = ServiceId::new(1).unwrap();
    let output = ServiceId::new(2).unwrap();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    for (id, name) in [(root, "Build"), (output, "output")] {
        old.interfaces.services.push(ServiceDeclaration {
            id,
            identity: names.intern(name),
            parents: vec![],
        });
    }
    for machine in &mut old.machines {
        let decl = machine.contract.declaration.as_ref().unwrap();
        names
            .retain_invocation_machine(decl, None, [1; 32], [7; 32])
            .unwrap();
        machine.contract.published_service_ceiling = vec![root];
    }
    old.identities = names.build();
    let mut new = old.clone();
    // Prefix admission now needs the source commitment. Each machine's portable
    // contract digest changed when its service envelope was republished.
    for machine in &mut new.machines {
        machine.contract.declaration.as_mut().unwrap().contract = [2; 32];
    }
    new.machines[0]
        .contract
        .published_service_ceiling
        .push(output);
    assert!(CheckedPrefix::check(&old, cut, &new, cut).is_ok());
    let mut wrong_parent = new.clone();
    wrong_parent.interfaces.services[0].parents.push(output);
    assert!(CheckedPrefix::check(&old, cut, &wrong_parent, cut).is_err());
    let mut new_callee = new.clone();
    new_callee.machines[1]
        .contract
        .published_service_ceiling
        .push(output);
    assert!(CheckedPrefix::check(&old, cut, &new_callee, cut).is_err());
    old.machines[0].contract.declared_service_reach = Some(vec![root]);
    new.machines[0].contract.declared_service_reach = Some(vec![root]);
    assert!(CheckedPrefix::check(&old, cut, &new, cut).is_err());
}

#[test]
fn only_paths_that_can_arrive_at_the_parked_call_have_run_in_this_frame() {
    use terminal_psi::SuccessorEdge;
    let (mut previous, site) = program();
    let [choose, wait, other] = [1, 2, 3].map(|n| BlockId::new(n).unwrap());
    let successor = |n, target| SuccessorEdge {
        edge: EdgeId::new(n).unwrap(),
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let machine = &mut previous.machines[0];
    machine.blocks[0].id = wait;
    let mut entry = Block::plain(
        choose,
        Vec::new(),
        Terminator::Conditional {
            condition: ValueId::new(7).unwrap(),
            when_true: successor(2, wait),
            when_false: successor(3, other),
        },
    );
    entry.operations.push(constant(true, 7));
    machine.blocks.insert(0, entry);
    let mut alternate = Block::plain(
        other,
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(4).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    alternate.operations.push(constant(false, 8));
    machine.blocks.push(alternate);
    let mut next = previous.clone();
    next.machines[0].blocks[2].operations[0] = constant(true, 8);
    CheckedPrefix::check(&previous, site, &next, site).unwrap();
    // If the other arm can rejoin before the wait, its differing operation
    // must be checked even though this particular condition happens to be true.
    let rejoin = Terminator::Jump {
        edge: EdgeId::new(4).unwrap(),
        target: wait,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    previous.machines[0].blocks[2].terminator = rejoin.clone();
    next.machines[0].blocks[2].terminator = rejoin;
    assert!(CheckedPrefix::check(&previous, site, &next, site).is_err());
}

#[test]
fn an_added_call_requirement_needs_evidence_from_its_own_invocation() {
    let (old, cut) = program();
    let mut new = old.clone();
    new.machines[1]
        .contract
        .requires
        .push(semantic_vocabulary::Proposition::Falsehood);
    let call = new.machines[0].blocks[0]
        .operations
        .iter_mut()
        .find(|operation| operation.id == cut.1)
        .unwrap();
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &mut call.kind
    else {
        panic!("the fixture parks at the unit helper call");
    };
    requirement_obligations.push(semantic_vocabulary::ObligationId::new(1).unwrap());
    assert!(CheckedPrefix::check(&old, cut, &new, cut).is_err());
}

#[test]
fn overloads_retain_and_rejoin_their_own_source_occurrences() {
    let (module, _) = program();
    let mut names = module.identities.into_builder();
    let mut byte = module.machines[1].declaration.as_ref().unwrap().clone();
    let mut boolean = byte.clone();
    byte.callable = names.intern("choose(u8)");
    boolean.callable = names.intern("choose(bool)");
    names
        .retain_invocation_machine(&byte, None, [1; 32], [3; 32])
        .unwrap();
    names
        .retain_invocation_machine(&boolean, None, [2; 32], [3; 32])
        .unwrap();
    assert!(
        names
            .retain_invocation_machine(&byte, None, [2; 32], [3; 32])
            .is_err()
    );

    let names = names.build();
    assert!(names.rejoins_machine(&byte, &names, &byte));
    assert!(names.rejoins_machine(&boolean, &names, &boolean));
    assert!(!names.rejoins_machine(&byte, &names, &boolean));
}

#[test]
fn the_same_source_owner_and_receipt_number_do_not_hide_a_changed_authored_contract() {
    use semantic_vocabulary::{ObligationId, Proposition};
    let (mut old, cut) = program();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    let helper = &mut old.machines[1];
    let declaration = helper.declaration.as_ref().unwrap();
    names
        .retain_invocation_machine(declaration, None, [8; 32], [9; 32])
        .unwrap();
    helper.contract.requires.push(Proposition::Truth);
    helper.contract.ensures.push(terminal_psi::ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: Proposition::Truth,
    });
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &mut old.machines[0].blocks[0].operations[0].kind
    else {
        panic!("declared call")
    };
    requirement_obligations.push(ObligationId::new(2).unwrap());
    old.identities = names.build();
    let mut candidate = old.clone();
    CheckedPrefix::check(&old, cut, &candidate, cut).unwrap();
    candidate.machines[1].contract.requires[0] = Proposition::Falsehood;
    assert!(CheckedPrefix::check(&old, cut, &candidate, cut).is_err());
    candidate.machines[1].contract.requires[0] = Proposition::Truth;
    candidate.machines[1].contract.ensures[0].proposition = Proposition::Falsehood;
    assert!(matches!(
        CheckedPrefix::check(&old, cut, &candidate, cut),
        Err("successor changed a reached machine's normal guarantees")
    ));
}

#[test]
fn a_helper_owing_a_value_requires_an_admitted_suffix_with_the_same_result() {
    let (mut old, site) = program();
    let answer = ValueDeclaration {
        id: ValueId::new(5).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::ZERO,
    };
    old.machines[0].contract.result = terminal_psi::TerminalMachineResult::Scalar(answer);
    old.machines[0].blocks[0].terminator = Terminator::PendingNormalCompletion {
        call: site.1,
        scalars: Vec::new(),
        storage: Vec::new(),
    };
    assert!(matches!(
        CheckedPrefix::check(&old, site, &old, site),
        Err("the checked successor still owes the selected completion")
    ));
    let mut next = old.clone();
    next.machines[0].blocks[0]
        .operations
        .push(constant(true, 5));
    next.machines[0].blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value: answer.id,
        cleanup_actions: Vec::new(),
    };
    CheckedPrefix::check(&old, site, &next, site).unwrap();
    let mut promised = old.clone();
    promised.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Truth,
        });
    assert!(matches!(
        CheckedPrefix::check(&promised, site, &next, site),
        Err("successor changed a reached machine's normal guarantees")
    ));
    next.machines[0].contract.result = terminal_psi::TerminalMachineResult::Unit;
    next.machines[0].blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(1).unwrap(),
        cleanup_actions: Vec::new(),
    };
    assert!(matches!(
        CheckedPrefix::check(&old, site, &next, site),
        Err("successor changed a reached machine's return contract")
    ));
}

#[test]
fn a_completed_helper_cannot_acquire_an_unproved_guarantee() {
    let (original, cut) = program();
    let mut successor = original.clone();
    successor.machines[1]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Falsehood,
        });
    assert!(CheckedPrefix::check(&original, cut, &successor, cut).is_err());
}

mod case_projections {
    use super::*;
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
    };

    fn choices(declarations: &[(&str, u64, u64)]) -> TerminalModule {
        let (mut module, _) = program();
        let mut names = std::mem::take(&mut module.identities).into_builder();
        for (index, &(owner, case, field)) in declarations.iter().enumerate() {
            module
                .types
                .structural_types
                .push(StructuralTypeDeclaration {
                    id: StructuralTypeId::new(index as u64 + 1).unwrap(),
                    identity: names.intern(owner),
                    declaration_identity: names.intern(owner),
                    shape: StructuralTypeShape::Sum {
                        cases: vec![StructuralCaseDeclaration {
                            id: StructuralCaseId::new(case).unwrap(),
                            identity: names.intern("present"),
                            declaration_identity: names.intern(&format!("{owner}::present")),
                            fields: vec![StructuralFieldDeclaration {
                                id: StructuralFieldId::new(field).unwrap(),
                                identity: names.intern("ready"),
                                declaration_identity: names
                                    .intern(&format!("{owner}::present::ready")),
                                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                                relevance: Default::default(),
                                storage: None,
                                inline_carrier: None,
                            }],
                        }],
                    },
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: false,
                    nominal_cleanup: None,
                    quotient_carrier: None,
                    scalar_restriction: None,
                    requires: Vec::new(),
                });
        }
        module.identities = names.build();
        module
    }

    #[test]
    fn a_selector_without_a_parent_requires_every_candidate_to_agree() {
        let (_, site) = program();
        let old = choices(&[("First", 9, 21), ("Second", 9, 21)]);
        let original_case = StructuralCaseId::new(9).unwrap();
        let original_field = StructuralFieldId::new(21).unwrap();
        let new_case = StructuralCaseId::new(11).unwrap();
        let new_field = StructuralFieldId::new(24).unwrap();
        let owner = StructuralTypeId::new(1).unwrap();
        for (new, agreed) in [
            (choices(&[("First", 11, 24), ("Second", 11, 24)]), true),
            (choices(&[("First", 11, 24), ("Second", 12, 25)]), false),
            (choices(&[("First", 11, 24)]), false),
        ] {
            let checked = CheckedPrefix::check(&old, site, &new, site).unwrap();
            assert_eq!(checked.case_id(owner, original_case), Some(new_case));
            assert_eq!(checked.field_id(owner, original_field), Some(new_field));
            assert_eq!(
                checked.unambiguous_case_id(original_case),
                agreed.then_some(new_case)
            );
            assert_eq!(
                checked.unambiguous_field_id(original_field),
                agreed.then_some(new_field)
            );
        }
    }

    #[test]
    fn a_call_selects_a_case_from_the_type_behind_its_reference() {
        use semantic_vocabulary::PlaceId;
        use terminal_psi::{
            BoundaryParameterKind, StructuralAccess, StructuralArgument,
            StructuralParameterDeclaration, StructuralPathSegment as Step,
        };

        fn passing_case(first: u64, alternate: u64, second: u64, variant: u64) -> TerminalModule {
            let mut module = choices(&[("First", first, 21), ("Second", second, 21)]);
            let owner = module.types.structural_types[0].id;
            let reference = StructuralTypeId::new(3).unwrap();
            let flag = StructuralTypeId::new(4).unwrap();
            let access = StructuralAccess::SharedBorrow;
            let mut names = std::mem::take(&mut module.identities).into_builder();
            for (id, name, shape) in [
                (
                    reference,
                    "@First",
                    StructuralTypeShape::Reference {
                        referent: owner,
                        access,
                    },
                ),
                (
                    flag,
                    "bool",
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                ),
            ] {
                module
                    .types
                    .structural_types
                    .push(StructuralTypeDeclaration {
                        id,
                        identity: names.intern(name),
                        declaration_identity: names.intern(name),
                        shape,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: true,
                        descriptor_cleanup: false,
                        nominal_cleanup: None,
                        quotient_carrier: None,
                        scalar_restriction: None,
                        requires: Vec::new(),
                    });
            }
            let StructuralTypeShape::Sum { cases } = &mut module.types.structural_types[0].shape
            else {
                unreachable!()
            };
            cases[0].fields[0].inline_carrier = Some(flag);
            let mut sibling = cases[0].clone();
            sibling.id = StructuralCaseId::new(alternate).unwrap();
            sibling.identity = names.intern("alternate");
            sibling.declaration_identity = names.intern("First::alternate");
            sibling.fields[0].id = StructuralFieldId::new(22).unwrap();
            sibling.fields[0].declaration_identity = names.intern("First::alternate::ready");
            cases.push(sibling);
            let ready = names.intern("ready");
            module.identities = names.build();
            let place = PlaceId::new(1).unwrap();
            for (machine, ty) in module.machines.iter_mut().zip([reference, flag]) {
                machine
                    .contract
                    .structural_parameters
                    .push(StructuralParameterDeclaration {
                        place,
                        position: 0,
                        structural_type: ty,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                machine
                    .contract
                    .parameter_order
                    .push(BoundaryParameterKind::Structural);
            }
            let call = &mut module.machines[0].blocks[0].operations[0];
            call.kind
                .structural_arguments_mut()
                .unwrap()
                .push(StructuralArgument {
                    place,
                    path: vec![
                        Step::Referent,
                        Step::Case(StructuralCaseId::new(variant).unwrap()),
                        Step::Field(ready),
                    ],
                    access,
                });
            module
        }

        let (_, site) = program();
        let old = passing_case(9, 10, 9, 9);
        let renamed = passing_case(11, 9, 12, 11);
        CheckedPrefix::check(&old, site, &renamed, site).unwrap();
        for changed in [9, 12] {
            let wrong_case = passing_case(11, 9, 12, changed);
            assert_eq!(
                CheckedPrefix::check(&old, site, &wrong_case, site).err(),
                Some("successor changed a projected argument")
            );
        }
    }
}

#[test]
fn contract_fields_rejoin_their_declarations_when_sibling_numbers_are_swapped() {
    use semantic_vocabulary::{
        ObligationId, PlaceId, Proposition, ScalarTerm, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BoundaryParameterKind, ContractClause, StructuralAccess, StructuralFieldDeclaration,
        StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
    };

    let (mut old, cut) = program();
    let root = PlaceId::new(1).unwrap();
    let ty = StructuralTypeId::new(1).unwrap();
    let first = StructuralFieldId::new(1).unwrap();
    let second = StructuralFieldId::new(2).unwrap();
    let mut names = std::mem::take(&mut old.identities).into_builder();
    let mut field = |id, name| StructuralFieldDeclaration {
        id,
        identity: names.intern(name),
        declaration_identity: names.intern(&format!("Flags::{name}")),
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        storage: None,
        inline_carrier: None,
        relevance: Default::default(),
    };
    let fields = vec![field(first, "enabled"), field(second, "disabled")];
    old.types.structural_types.push(StructuralTypeDeclaration {
        id: ty,
        declaration_identity: names.intern("Flags"),
        identity: names.intern("Flags storage"),
        shape: StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        },
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        quotient_carrier: None,
        nominal_cleanup: None,
        scalar_restriction: None,
        requires: Vec::new(),
    });
    old.identities = names.build();
    let predicate = |id| {
        Proposition::Equal(
            ScalarTerm::boolean_field(root, id),
            ScalarTerm::Boolean(true),
        )
    };
    let contract = &mut old.machines[0].contract;
    contract
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            structural_type: ty,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    contract
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    contract.requires.push(predicate(first));
    contract.ensures.push(ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: predicate(first),
    });
    old.types.structural_types[0]
        .requires
        .push(predicate(first));
    let mut new = old.clone();
    let StructuralTypeShape::Record { fields, .. } = &mut new.types.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].id = second;
    fields[1].id = first;
    new.machines[0].contract.requires[0] = predicate(second);
    new.machines[0].contract.ensures[0].proposition = predicate(second);
    new.types.structural_types[0].requires[0] = predicate(second);
    CheckedPrefix::check(&old, cut, &new, cut).unwrap();
    let mut wrong = new.clone();
    wrong.machines[0].contract.requires[0] = predicate(first);
    assert!(CheckedPrefix::check(&old, cut, &wrong, cut).is_err());
    let mut wrong = new.clone();
    wrong.machines[0].contract.ensures[0].proposition = predicate(first);
    assert!(CheckedPrefix::check(&old, cut, &wrong, cut).is_err());
    new.types.structural_types[0].requires[0] = predicate(first);
    assert!(CheckedPrefix::check(&old, cut, &new, cut).is_err());
}

#[path = "checked_continuation/record_fields.rs"]
mod record_fields;
