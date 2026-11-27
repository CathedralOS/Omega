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
fn inventory_keeps_a_content_domains_predicate_without_proving_a_changed_value() {
    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentProjectionExpression,
        ContentProjectionIdentity, ContentProjectionScalar,
    };
    let types = types(2);
    let mut machine = machine();
    let mut owner = domain(types[1].id);
    owner.predicate.as_mut().unwrap().proposition = Proposition::Truth;
    let input = &mut machine.structural_parameters[0];
    input.qualifications.push(owner.id);
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Bytes".into(),
    };
    let value =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("2".into()));
    owner.content_projection = Some(terminal_psi::StructuralContentProjection {
        identity: ContentProjectionIdentity {
            domain: ContentDomainId::new(owner.semantic_domain.get()).unwrap(),
            projection_report_fingerprint: value.report_fingerprint(&algebra),
        },
        algebra,
        expression: value,
    });
    let domains = [owner];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let held = defaults.declared(|_, _, _| None).unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].origin, DefaultClause::Domain(domains[0].id));
    assert_eq!(defaults.clause(&held[0]).unwrap(), Proposition::Truth);
    let changed = store(vec![StructuralPathSegment::FixedIndex(0)]);
    let error = defaults.declared_operation(&changed).unwrap_err();
    assert_eq!(
        error.detail,
        "a named default has no complete storage predicate theory"
    );
}

/// A whole stored value carries the content domain its destination displays,
/// so the lender owes no predicate for it (allocation's `bump.omg` stores a
/// split's `rest` into its `@mutable` strategy's `tail`). Under a destination
/// that does not display the domain, the lender's content still owes a proof.
#[test]
fn a_whole_stored_value_delivers_the_content_domain_its_destination_displays() {
    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentProjectionExpression,
        ContentProjectionIdentity, ContentProjectionScalar,
    };
    let types = types(2);
    let mut machine = machine();
    let mut owner = domain(types[1].id);
    owner.predicate.as_mut().unwrap().proposition = Proposition::Truth;
    machine.structural_parameters[0]
        .qualifications
        .push(owner.id);
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Bytes".into(),
    };
    let value =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("2".into()));
    owner.content_projection = Some(terminal_psi::StructuralContentProjection {
        identity: ContentProjectionIdentity {
            domain: ContentDomainId::new(owner.semantic_domain.get()).unwrap(),
            projection_report_fingerprint: value.report_fingerprint(&algebra),
        },
        algebra,
        expression: value,
    });
    let domains = [owner];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let root = PlaceId::new(2).unwrap();
    let mut lost = Vec::new();
    defaults
        .append_lost(&mut lost, root, &[], root, &[])
        .unwrap();
    assert!(lost.is_empty(), "{lost:?}");
    let element = [StructuralPathSegment::FixedIndex(0)];
    let error = defaults
        .append_lost(&mut lost, root, &element, root, &[])
        .unwrap_err();
    assert_eq!(
        error.detail,
        "a named default has no complete storage predicate theory"
    );
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
    assert_eq!(defaults.declared(|_, _, _| None).unwrap(), rows);
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
    assert!(defaults.declared(|_, _, _| None).unwrap().is_empty());
    assert!(
        defaults
            .declared_operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_window_requires_coordinates_only_for_its_potential_owners() {
    use crate::borrowed_storage::Projection;

    let root = PlaceId::new(2).unwrap();
    let window = Projection::window(
        ValueId::new(1).unwrap(),
        ValueId::new(2).unwrap(),
        (BlockId::new(1).unwrap(), 0),
    );
    let project = |types: &[_], domains: &[_], machine: &TerminalMachine| {
        DefaultRequirements::new(types, domains, machine)
            .unwrap()
            .has_no_window_defaults(root, &window, &[], WindowWrite::Store(&[StructuralPathSegment::FixedIndex(1)]))
            .unwrap()
    };
    let mut types = types(2);
    let record = StructuralTypeShape::Record {
        fields: Vec::new(),
        storage: terminal_psi::StructuralRecordStorage::Packed,
    };
    types[0].shape = record.clone();
    types[1].shape = StructuralTypeShape::ElementView {
        element: types[0].id,
        access: Some(StructuralAccess::MutableBorrow),
    };
    let mut unrelated = ty(3, record);
    unrelated.requires.push(Proposition::Truth);
    types.push(unrelated);
    let mut machine = machine();
    assert!(project(&types, &[], &machine));

    types[0].requires.push(Proposition::Truth);
    assert!(!project(&types, &[], &machine));
    let handback = |types: &[_], machine: &TerminalMachine, projection: &Projection, displayed| {
        DefaultRequirements::new(types, &[], machine)
            .unwrap()
            .has_no_window_defaults(root, projection, &[], WindowWrite::ExclusiveCall(displayed))
            .unwrap()
    };
    let selected = window.then(&Projection::new(&[StructuralPathSegment::FixedIndex(1)]));
    let element = types[0].id;
    let view = types[1].id;
    assert!(handback(&types, &machine, &selected, element));
    assert!(!handback(&types, &machine, &selected, view));
    let recast = selected.then(&Projection::new(&[StructuralPathSegment::RepresentationView {
        structural_type: element,
        stored_scalar: None,
    }]));
    assert!(!handback(&types, &machine, &recast, element));
    assert!(handback(&types, &machine, &window, view));
    assert!(!handback(&types, &machine, &window, element));
    // This clause belongs to the containing collection. Handing back one
    // valid element, or every element of a subview, cannot establish it.
    types[1].requires.push(Proposition::Truth);
    assert!(!handback(&types, &machine, &selected, element));
    assert!(!handback(&types, &machine, &window, view));
    types[1].requires.clear();
    types[0].requires.clear();

    let owner = domain(types[1].id);
    machine.structural_parameters[0]
        .qualifications
        .push(owner.id);
    assert!(!handback(&types, &machine, &selected, element));
    assert!(!project(&types, &[owner], &machine));
}

#[test]
fn plain_arrays_do_not_enumerate_elements_for_default_inventory() {
    let types = types(u64::MAX);
    let machine = machine();
    let defaults = DefaultRequirements::new(&types, &[], &machine).unwrap();
    assert!(defaults.declared(|_, _, _| None).unwrap().is_empty());
    assert!(
        defaults
            .declared_operation(&store(vec![StructuralPathSegment::FixedIndex(0)]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_universal_marker_leaves_only_the_sparse_element_predicate() {
    use StructuralPathSegment::{AllElements, FixedIndex};

    let types = types(u64::MAX);
    let mut marker = domain(types[0].id);
    marker.predicate = None;
    let mut predicated = domain(types[0].id);
    predicated.id = StructuralDomainId::new(2).unwrap();
    predicated.semantic_domain = DomainSemanticId::new(2).unwrap();
    predicated.identity = TerminalIdentityIndex(2);
    let mut machine = machine();
    let index = u64::MAX - 1;
    machine.structural_parameters[0].projected_qualifications = vec![
        StructuralPathQualification {
            path: vec![AllElements],
            domain: marker.id,
        },
        StructuralPathQualification {
            path: vec![FixedIndex(index)],
            domain: predicated.id,
        },
    ];
    let domains = [marker, predicated];
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let rows = defaults.declared(|_, _, _| None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, [FixedIndex(index)]);
    assert_eq!(rows[0].origin, DefaultClause::Domain(domains[1].id));
}

#[test]
fn a_nested_element_predicate_reaches_each_concrete_storage_path_once() {
    use StructuralPathSegment::{AllElements, FixedIndex};

    let mut types = types(3);
    types.push(ty(
        3,
        StructuralTypeShape::FixedArray {
            element: types[1].id,
            length: ArrayExtent::Literal(2),
        },
    ));
    let domains = [domain(types[0].id)];
    let mut machine = machine();
    let parameter = &mut machine.structural_parameters[0];
    parameter.structural_type = types[2].id;
    for path in [
        vec![AllElements, AllElements],
        vec![FixedIndex(1), FixedIndex(2)],
    ] {
        parameter
            .projected_qualifications
            .push(StructuralPathQualification {
                path,
                domain: domains[0].id,
            });
    }
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let paths = defaults
        .declared(|_, _, _| None)
        .unwrap()
        .into_iter()
        .map(|row| row.path)
        .collect::<Vec<_>>();
    let expected = (0..2)
        .flat_map(|outer| (0..3).map(move |inner| vec![FixedIndex(outer), FixedIndex(inner)]))
        .collect::<Vec<_>>();
    assert_eq!(paths, expected);
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

#[test]
fn sparse_snapshot_names_the_read_index_and_each_explicit_qualification() {
    use StructuralPathSegment::FixedIndex;

    let mut types = types(4096);
    types[0].shape = StructuralTypeShape::Record {
        fields: Vec::new(),
        storage: terminal_psi::StructuralRecordStorage::Packed,
    };
    types[0].requires.push(Proposition::Truth);
    let domains = [domain(types[0].id)];
    let mut machine = machine();
    machine.structural_parameters[0]
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![FixedIndex(11)],
            domain: domains[0].id,
        });
    let defaults = DefaultRequirements::new(&types, &domains, &machine).unwrap();
    let sparse = defaults.declared(|_, _, _| Some(vec![4095])).unwrap();
    let paths: Vec<_> = sparse.into_iter().map(|row| row.path).collect();
    assert_eq!(
        paths,
        [vec![FixedIndex(11)], vec![FixedIndex(11)], vec![FixedIndex(4095)]]
    );
    assert_eq!(defaults.declared(|_, _, _| None).unwrap().len(), 4097);
}
