use super::*;
use semantic_vocabulary::{
    ByteSequencePredicate, ContractId, DomainSemanticId, IntegerSign, OperationId,
    StructuralPlaceKind,
};
use terminal_psi::{
    ArrayExtent, MachineContract, OperationResult, RegisterTargetApplicability,
    StructuralAccess, StructuralDomainPredicate, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPlaceDeclaration,
    TerminalIdentityIndex,
};

fn ty(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: TerminalIdentityIndex::ABSENT,
        quotient_carrier: None,
        id: StructuralTypeId::new(id).unwrap(),
        identity: TerminalIdentityIndex::ABSENT,
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

fn types(length: u64) -> Vec<StructuralTypeDeclaration> {
    vec![
        ty(
            1,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        ),
        ty(
            2,
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: ArrayExtent::Literal(length),
            },
        ),
    ]
}

fn machine() -> TerminalMachine {
    let root = PlaceId::new(2).unwrap();
    let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
    contract
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: vec![StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        }],
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: Vec::new(),
        contract,
    }
}

fn domain(carrier: StructuralTypeId) -> StructuralDomainDeclaration {
    let subject = PlaceId::new(9).unwrap();
    StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject,
            proposition: Proposition::Equal(
                ScalarTerm::byte_sequence_predicate(
                    ByteSequencePredicate::ValidUtf8,
                    subject,
                    Vec::new(),
                ),
                ScalarTerm::Boolean(true),
            ),
        }),
        id: StructuralDomainId::new(1).unwrap(),
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        carrier,
        subject_identity: TerminalIdentityIndex::ABSENT,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: None,
        access_policy: None,
    }
}

fn store(path: Vec<StructuralPathSegment>) -> Operation {
    Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::WriteOnlyPrimitiveStore {
            destination: PlaceId::new(2).unwrap(),
            path,
            value: ValueId::new(1).unwrap(),
            range_obligation: None,
        },
    )
}

#[test]
fn named_defaults_are_rerooted_goals_not_membership_truth() {
    let mut types = types(2);
    types[1].requires.push(Proposition::Truth);
    let mut machine = machine();
    machine.structural_parameters[0]
        .qualifications
        .push(StructuralDomainId::new(1).unwrap());
    let domains = vec![domain(types[1].id)];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let rows = defaults
        .declared_operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(defaults.clause(&rows[0]).unwrap(), Proposition::Truth);
    assert_eq!(
        defaults.clause(&rows[1]).unwrap(),
        Proposition::Equal(
            ScalarTerm::byte_sequence_predicate(
                ByteSequencePredicate::ValidUtf8,
                PlaceId::new(1).unwrap(),
                Vec::new()
            ),
            ScalarTerm::Boolean(true),
        )
    );
    assert_eq!(defaults.declared().unwrap(), rows);
}

#[test]
fn exclusive_calls_owe_the_same_owner_defaults_as_a_store() {
    let mut types = types(2);
    types[1].requires.push(Proposition::Truth);
    let mut machine = machine();
    machine.structural_parameters[0]
        .qualifications
        .push(StructuralDomainId::new(1).unwrap());
    let domains = vec![domain(types[1].id)];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let path = vec![StructuralPathSegment::FixedIndex(0)];
    let expected = defaults.declared_operation(&store(path.clone())).unwrap();
    assert_eq!(expected.len(), 2);
    for (access, path, owes) in [
        (StructuralAccess::MutableBorrow, path.clone(), true),
        (StructuralAccess::WriteOnlyBorrow, path.clone(), true),
        (StructuralAccess::SharedBorrow, path, false),
        (StructuralAccess::MutableBorrow, Vec::new(), false),
    ] {
        let call = Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: MachineId::new(2).unwrap(),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: PlaceId::new(2).unwrap(),
                    path,
                    access,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let actual = defaults.declared_operation(&call).unwrap();
        if owes {
            assert_eq!(actual, expected);
        } else {
            assert!(actual.is_empty());
        }
    }
}

#[test]
fn additional_input_membership_is_not_a_declared_default() {
    let types = types(2);
    let mut machine = machine();
    machine.structural_parameters[0]
        .additional_input_qualifications
        .push(StructuralPathQualification {
            path: Vec::new(),
            domain: StructuralDomainId::new(1).unwrap(),
        });
    let result_place = PlaceId::new(3).unwrap();
    machine.result = terminal_psi::TerminalMachineResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            place: result_place,
            structural_type: types[1].id,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![StructuralDomainId::new(1).unwrap()],
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        },
    );
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: result_place,
        kind: StructuralPlaceKind::Result,
    });
    let domains = vec![domain(types[1].id)];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    assert!(defaults.declared().unwrap().is_empty());
    assert!(
        defaults
            .declared_operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn plain_arrays_do_not_enumerate_elements_for_default_inventory() {
    let types = types(u64::MAX);
    let machine = machine();
    let defaults = DefaultRequirements::new(&types, &[], &machine).unwrap();
    assert!(defaults.declared().unwrap().is_empty());
    assert!(
        defaults
            .declared_operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reference_display_does_not_become_a_lender_default() {
    let mut types = types(2);
    types.push(ty(
        3,
        StructuralTypeShape::Reference {
            referent: types[1].id,
            access: StructuralAccess::MutableBorrow,
        },
    ));
    let mut machine = machine();
    machine.structural_parameters[0].structural_type = types[2].id;
    machine.structural_parameters[0]
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::Referent],
            domain: StructuralDomainId::new(1).unwrap(),
        });
    let domains = vec![domain(types[1].id)];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let refused = defaults
        .declared_operation(&store(vec![
            StructuralPathSegment::Referent,
            StructuralPathSegment::FixedIndex(0),
        ]))
        .unwrap_err();
    assert_eq!(
        refused.detail,
        "a reference display is not a lender's declared default"
    );
}
