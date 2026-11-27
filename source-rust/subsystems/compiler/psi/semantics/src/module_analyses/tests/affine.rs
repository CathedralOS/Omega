//! A walk made by `ModuleAnalyses::affine` must give every answer the walk
//! alone (`MachineAffine::new`) gives, while it reads its machine's
//! borrowed-storage closure and callee summaries from the bundle. The
//! root borrows a reference, so each walk needs that closure. In the static
//! twin the closure is shared; in the dynamic twin machine 2 calls
//! dynamically, so the closure owns rows adapted against itself.

use super::{ROOT, SELECTING, call, declare_dynamic_slot, dynamic_call, id, machine};
use crate::module_analyses::{DescriptorRows, ModuleAnalyses, kind_index};
use crate::plain_affine::{Error, MachineAffine};
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    BindingRelevance, BoundaryParameterKind, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
};

use crate::descriptors::descriptor_results::DescriptorSlotKind;

const REALIZATION: u64 = 3;
const RECORD: u64 = 1;
const REFERENCE: u64 = 2;
const COPYABLE_PARENT: u64 = 3;

fn declared_type(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        id: StructuralTypeId::new(raw).unwrap(),
        identity: TerminalIdentityIndex(raw as u32),
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Affine,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        quotient_carrier: None,
        scalar_restriction: None,
        requires: Vec::new(),
        shape,
    }
}

fn record(raw: u64, fields: Vec<StructuralFieldDeclaration>) -> StructuralTypeDeclaration {
    declared_type(
        raw,
        StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        },
    )
}

/// Machines are declared in identity order, as affine walks require. The
/// root borrows a reference to an empty record and calls machine 2, which in
/// the dynamic twin calls machine 3 through the dynamic slot.
fn affine_module(dynamic: bool) -> TerminalModule {
    let mut module = TerminalModule::for_entry(id(ROOT));
    declare_dynamic_slot(&mut module, REALIZATION);
    let mut reference = declared_type(
        REFERENCE,
        StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(RECORD).unwrap(),
            access: StructuralAccess::SharedBorrow,
        },
    );
    reference.multiplicity = StructuralMultiplicity::Unrestricted;
    module.types.structural_types = vec![record(RECORD, Vec::new()), reference];
    let mut root = machine(ROOT, vec![call(1, SELECTING)]);
    let place = PlaceId::new(1).unwrap();
    root.contract.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type: StructuralTypeId::new(REFERENCE).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    root.contract.parameter_order = vec![BoundaryParameterKind::Structural];
    root.structural_places = vec![StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    }];
    module.machines = vec![
        root,
        machine(
            SELECTING,
            if dynamic { dynamic_call(2) } else { Vec::new() },
        ),
        machine(REALIZATION, Vec::new()),
    ];
    module
}

/// Adds a copyable record whose field holds the affine record, a permission
/// that would erase the field's ownership obligation.
fn with_invalid_type_permissions(mut module: TerminalModule) -> TerminalModule {
    let mut parent = record(
        COPYABLE_PARENT,
        vec![StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(StructuralTypeId::new(RECORD).unwrap()),
            storage: None,
            inline_carrier: None,
        }],
    );
    parent.multiplicity = StructuralMultiplicity::Unrestricted;
    module.types.structural_types.push(parent);
    module
}

/// Every answer of the walks, printed so plans and refusals compare whole.
fn answers(walks: &MachineAffine<'_>) -> [String; 4] {
    [
        format!("{:?}", walks.plan()),
        format!("{:?}", walks.verify()),
        format!("{:?}", walks.plan_claims()),
        format!("{:?}", walks.verify_claims()),
    ]
}

fn alone(module: &TerminalModule, machine: u64) -> [String; 4] {
    answers(&MachineAffine::new(module, id(machine)).unwrap())
}

/// Whether the bundle has adapted `machine`'s shared borrowed-storage row.
fn shares_row(analyses: &ModuleAnalyses<'_>, machine: u64) -> bool {
    analyses.rows[kind_index(DescriptorSlotKind::BorrowedStorage)]
        .get()
        .zip(analyses.machine_slots.slot(id(machine)))
        .is_some_and(|(family, slot)| family.adapted(slot).is_some())
}

#[test]
fn bundle_walks_without_a_dynamic_call_read_the_shared_rows_and_answer_as_a_walk_alone() {
    let module = affine_module(false);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    let walks = analyses.affine(id(ROOT)).unwrap();
    let expected = alone(&module, ROOT);
    assert_eq!(answers(&walks), expected);
    // The walk needed the closure, and it is the bundle's shared one.
    let plan = walks.plan().unwrap();
    assert_eq!(plan.graphed_machines, [id(SELECTING)]);
    assert!(shares_row(&analyses, ROOT) && shares_row(&analyses, SELECTING));
    assert!(!shares_row(&analyses, REALIZATION));
    assert!(matches!(
        analyses.descriptor_closure(DescriptorSlotKind::BorrowedStorage, [id(ROOT)]),
        Ok(DescriptorRows::Shared { .. })
    ));
    // The walk published its callee's summary, and a second walk over the
    // same bundle takes it and gives the same answers.
    assert!(analyses.holds_summary(id(SELECTING)));
    assert_eq!(answers(&analyses.affine(id(ROOT)).unwrap()), expected);
    for machine in [SELECTING, REALIZATION] {
        assert_eq!(
            answers(&analyses.affine(id(machine)).unwrap()),
            alone(&module, machine)
        );
    }
}

#[test]
fn bundle_walks_reaching_a_dynamic_call_own_their_rows_and_answer_as_a_walk_alone() {
    let module = affine_module(true);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    let walks = analyses.affine(id(ROOT)).unwrap();
    let expected = alone(&module, ROOT);
    assert_eq!(answers(&walks), expected);
    let plan = walks.plan().unwrap();
    // The dynamic call reaches its realization.
    assert_eq!(plan.graphed_machines, [id(SELECTING), id(REALIZATION)]);
    assert!(matches!(
        analyses.descriptor_closure(DescriptorSlotKind::BorrowedStorage, [id(ROOT)]),
        Ok(DescriptorRows::Owned(_))
    ));
    // Rows adapted against this closure alone never share the bundle's
    // summaries.
    assert!(!analyses.holds_summary(id(SELECTING)) && !analyses.holds_summary(id(REALIZATION)));
    for machine in [SELECTING, REALIZATION] {
        assert_eq!(
            answers(&analyses.affine(id(machine)).unwrap()),
            alone(&module, machine)
        );
    }
}

#[test]
fn a_machine_outside_the_catalog_is_refused_as_a_walk_alone_refuses_it() {
    let module = affine_module(false);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    let refused = |walks: Result<MachineAffine<'_>, Error>| format!("{:?}", walks.err());
    assert_eq!(
        refused(analyses.affine(id(9))),
        refused(MachineAffine::new(&module, id(9)))
    );
    assert!(analyses.affine(id(9)).is_err());
}

#[test]
fn type_permissions_are_checked_on_the_first_walk_and_refused_as_a_walk_alone_refuses_them() {
    let module = with_invalid_type_permissions(affine_module(false));
    let analyses = ModuleAnalyses::new(&module).unwrap();
    // Building the bundle checks no type.
    assert!(analyses.type_permissions.get().is_none());
    let expected = alone(&module, ROOT);
    assert!(
        expected
            .iter()
            .all(|answer| answer.contains("parent type permissions erase")),
        "{expected:?}"
    );
    assert_eq!(answers(&analyses.affine(id(ROOT)).unwrap()), expected);
    assert!(matches!(
        analyses.type_permissions.get(),
        Some(Err(Error::Malformed { .. }))
    ));
    // The answer stays with the bundle for every later walk.
    assert_eq!(
        answers(&analyses.affine(id(SELECTING)).unwrap()),
        alone(&module, SELECTING)
    );

    let valid = affine_module(false);
    let analyses = ModuleAnalyses::new(&valid).unwrap();
    analyses.affine(id(ROOT)).unwrap().plan().unwrap();
    assert!(matches!(analyses.type_permissions.get(), Some(Ok(()))));
}

#[test]
fn bundle_walks_can_run_on_several_threads() {
    let module = affine_module(false);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    let expected = alone(&module, ROOT);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let (analyses, expected) = (&analyses, &expected);
            scope.spawn(move || {
                assert_eq!(&answers(&analyses.affine(id(ROOT)).unwrap()), expected);
            });
        }
    });
}

#[test]
fn content_claim_checks_through_a_bundle_answer_as_a_walk_alone() {
    use crate::content_transformations::forwarding::validate_claims;
    for dynamic in [false, true] {
        let module = affine_module(dynamic);
        let analyses = ModuleAnalyses::new(&module).unwrap();
        // A bundle over an equal copy is not this module's bundle, so the
        // check walks alone rather than reading another module's rows.
        let copy = affine_module(dynamic);
        let other = ModuleAnalyses::new(&copy).unwrap();
        for machine in &module.machines {
            let alone = validate_claims(&module, None, machine);
            assert_eq!(validate_claims(&module, Some(&analyses), machine), alone);
            assert_eq!(validate_claims(&module, Some(&other), machine), alone);
        }
        assert!(
            other.rows[kind_index(DescriptorSlotKind::BorrowedStorage)]
                .get()
                .is_none()
        );
        assert!(shares_row(&analyses, ROOT));
    }
}
