use super::{LiveDefinitions, StructuralTypeLookup, plain_home_cleanup};
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, StructuralTypeCatalog,
};
use semantic_vocabulary::{
    BlockId, MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

fn fixture() -> (AbstractFunction, LiveDefinitions, StructuralTypeCatalog) {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let source = PlaceId::new(1).unwrap();
    let place = PlaceId::new(2).unwrap();
    let operation = OperationId::new(1).unwrap();
    let function = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: Vec::new(),
        operations: vec![AbstractOperation::ElementViewSubslice {
            psi_operation: operation,
            result: StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                claims: Vec::new(),
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
            },
            source,
            start: ValueId::new(1).unwrap(),
            end: ValueId::new(2).unwrap(),
            length: ValueId::new(3).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }],
    };
    // Model the live definition left by successful subslice materialization.
    // Source/bounds/access validation belongs to that materialization owner;
    // these tests isolate the subsequent no-code cleanup contract.
    let live = LiveDefinitions {
        structural_homes: BTreeMap::new(),
        nonreturning: false,
        stored_descriptors: BTreeSet::new(),
        integers: BTreeMap::new(),
        booleans: BTreeMap::new(),
        scalar_homes: BTreeMap::new(),
        ieee_float_constants: BTreeMap::new(),
        scalar_block_parameters: BTreeMap::new(),
        views: BTreeMap::from([(place, (operation, structural_type))]),
        block_views: BTreeSet::new(),
        address_joins: BTreeSet::from([source]),
        owned_arrivals: BTreeSet::new(),
        lengths: BTreeMap::new(),
        trivial_affine_locals: BTreeMap::new(),
        references: BTreeMap::new(),
        drained_loan_roots: BTreeSet::new(),
    };
    let types = vec![
        StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::view".into(),
            shape: StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(2).unwrap(),
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "bool".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
    ]
    .into_iter()
    .collect();
    (function, live, types)
}

#[test]
fn affine_descriptor_cleanup_keeps_borrowed_backing_without_an_owned_home() {
    let (function, mut live, catalog) = fixture();
    let types = StructuralTypeLookup::new(&catalog);
    let actions = [TerminalAffineCleanupAction::DiscardRoot(
        PlaceId::new(2).unwrap(),
    )];
    assert!(plain_home_cleanup(&function, &mut live, &types, &actions).unwrap());
    assert!(live.structural_homes.is_empty());
    assert_eq!(
        live.address_joins,
        BTreeSet::from([PlaceId::new(1).unwrap()])
    );
    assert!(live.drained_loan_roots.is_empty());
    // Mutually exclusive sibling edges reconstruct the same semantic carrier.
    assert!(plain_home_cleanup(&function, &mut live, &types, &actions).unwrap());
}

#[test]
fn cleanup_releases_a_carrier_before_discarding_its_backing() {
    for reverse_order in [false, true] {
        let (mut function, mut live, _) = fixture();
        let backing = PlaceId::new(1).unwrap();
        let carrier = PlaceId::new(2).unwrap();
        let backing_type = StructuralTypeId::new(1).unwrap();
        let reference_type = StructuralTypeId::new(2).unwrap();
        function.structural_parameters[0].access = StructuralAccess::Owned;
        function.operations.clear();
        live.views.clear();
        live.address_joins.clear();
        live.references.insert(
            (carrier, Vec::new()),
            super::super::references::ReferenceCustody {
                identity: (carrier, Vec::new()),
                root: super::super::references::ReferenceRoot::Place(backing),
                parent: None,
                operation: Some(OperationId::new(1).unwrap()),
                result: StructuralOperationResult {
                    place: carrier,
                    structural_type: reference_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    claims: Vec::new(),
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                },
                referent: backing_type,
            },
        );
        let catalog: StructuralTypeCatalog = [
            StructuralTypeDeclaration {
                id: backing_type,
                identity: "Backing".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: reference_type,
                identity: "&Backing".into(),
                shape: StructuralTypeShape::Reference {
                    referent: backing_type,
                    access: StructuralAccess::SharedBorrow,
                },
            },
        ]
        .into_iter()
        .collect();
        let mut actions = [
            TerminalAffineCleanupAction::DiscardRoot(carrier),
            TerminalAffineCleanupAction::DiscardRoot(backing),
        ];
        if reverse_order {
            actions.reverse();
        }
        assert_eq!(
            plain_home_cleanup(
                &function,
                &mut live,
                &StructuralTypeLookup::new(&catalog),
                &actions,
            )
            .unwrap(),
            !reverse_order
        );
        assert_eq!(live.references.is_empty(), !reverse_order);
    }
}

#[test]
fn affine_descriptor_cleanup_rejects_metadata_drift_missing_liveness_and_duplicates() {
    for mutation in 0..7 {
        let (mut function, mut live, catalog) = fixture();
        let place = PlaceId::new(2).unwrap();
        let mut actions = vec![TerminalAffineCleanupAction::DiscardRoot(place)];
        match mutation {
            0 | 1 => {
                let AbstractOperation::ElementViewSubslice { result, .. } =
                    &mut function.operations[0]
                else {
                    unreachable!()
                };
                result.multiplicity = if mutation == 0 {
                    StructuralMultiplicity::Unrestricted
                } else {
                    StructuralMultiplicity::Linear
                };
            }
            2 => live.views.get_mut(&place).unwrap().0 = OperationId::new(9).unwrap(),
            3 => live.views.get_mut(&place).unwrap().1 = StructuralTypeId::new(2).unwrap(),
            4 => function.operations.push(function.operations[0].clone()),
            5 => {
                live.views.remove(&place);
            }
            6 => actions.push(actions[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            !plain_home_cleanup(
                &function,
                &mut live,
                &StructuralTypeLookup::new(&catalog),
                &actions
            )
            .unwrap(),
            "mutation {mutation}"
        );
        assert!(live.structural_homes.is_empty());
        assert_eq!(
            live.address_joins,
            BTreeSet::from([PlaceId::new(1).unwrap()])
        );
    }
}
