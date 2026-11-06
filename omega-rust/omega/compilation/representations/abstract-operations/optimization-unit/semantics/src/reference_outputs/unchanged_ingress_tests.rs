//! A no-op may return its one borrowed pointer unchanged even when the array
//! behind it is too large for a complete writable-reference inventory. That
//! unavailable inventory must still withhold a reusable output summary.
//!
//! Proposed child module of `reference_outputs`; not installed or executed.

use crate::reference_outputs::reconstruct_reference_output_catalog;
use crate::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::OwnershipFrontierSite;
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use std::borrow::Cow;
use terminal_psi::{
    ArrayExtent, BindingRelevance, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalDynamicDispatchCatalog, TerminalIdentityTableBuilder,
    TerminalPsiIdentity,
};
use terminal_semantics::descriptor_results::DescriptorSlotKind;

#[test]
fn unchanged_borrowed_holder_returns_when_reachable_pointer_array_exceeds_inventory() {
    let ty = |raw| StructuralTypeId::new(raw).unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let holder = PlaceId::new(1).unwrap();
    let mut names = TerminalIdentityTableBuilder::default();
    let body = names.intern("body");
    let declaration = |raw, identity, shape| StructuralTypeDeclaration {
        id: ty(raw),
        identity,
        shape,
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: raw == 1,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
    };
    // Every edge decreases the type number: Holder -> MutableArray ->
    // ArrayOfReferences -> SharedBit -> Bit. There is no recursive type.
    let types = vec![
        declaration(
            1,
            names.intern("test::Bit"),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        declaration(
            2,
            names.intern("test::SharedBit"),
            StructuralTypeShape::Reference {
                referent: ty(1),
                access: StructuralAccess::SharedBorrow,
            },
        ),
        declaration(
            3,
            names.intern("test::ArrayOfReferences"),
            StructuralTypeShape::FixedArray {
                element: ty(2),
                length: ArrayExtent::Literal(4097),
            },
        ),
        declaration(
            4,
            names.intern("test::MutableArray"),
            StructuralTypeShape::Reference {
                referent: ty(3),
                access: StructuralAccess::MutableBorrow,
            },
        ),
        declaration(
            5,
            names.intern("test::Holder"),
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: body,
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(ty(4)),
                    storage: None,
                    inline_carrier: None,
                }],
            },
        ),
    ];

    // Entry custody follows contained words, not the referents behind them.
    // The real entry-reconstruction owner calls this same public leaf query.
    let entry = terminal_semantics::reference_leaves(types.iter(), ty(5), 4096)
        .expect("the holder has a complete finite contained-reference frontier");
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0].structural_type, ty(4));
    assert_eq!(entry[0].path, vec![StructuralPathSegment::Field(body)]);
    assert!(
        DescriptorSlotKind::References
            .writable_slots(&types, holder, ty(5), StructuralAccess::MutableBorrow)
            .is_none(),
        "the fixture must actually reach unavailable writable-reference inventory",
    );

    let function = AbstractFunction {
        machine,
        attachment: None,
        nominal_cleanup_subject: None,
        entry: block,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: holder,
            position: 0,
            structural_type: ty(5),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block,
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: edge,
            cleanup_actions: Vec::new(),
        }],
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity::from_canonical_bytes(b"unchanged ingress over inventory cap"),
        entry: machine,
        structural_types: types.into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
        identities: names.build(),
        functions: vec![function],
    };
    // Seed construction supplies the actual CFG, definitions, fact indexes,
    // declared places, service reach and canonical content identity.
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("construct the complete acyclic no-op unit");
    let facts = reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .expect("normal return must preserve this exact unchanged borrowed ingress");
    assert!(facts.iter().any(|fact| {
        fact.machine == machine && fact.site == OwnershipFrontierSite::BlockEntry(block)
    }));
    assert!(facts.iter().all(|fact| {
        fact.psi == unit.psi && fact.machine == machine && fact.has_canonical_identity()
    }));

    // Validating a no-op does not turn the unavailable reachable roster into
    // a complete empty summary that another call could consume.
    let catalog = reconstruct_reference_output_catalog(&unit, &[], |_, node| {
        Some(Cow::Borrowed(&node.operation))
    })
    .expect("the full unit must have valid structural and CFG prerequisites");
    assert!(catalog.body(machine).is_none());
    let reason = catalog
        .unavailable_reason(machine)
        .expect("an unavailable output summary retains its reason");
    assert!(
        reason.contains("a reference output has no complete typed slot inventory"),
        "unexpected earlier summary refusal: {reason}",
    );
}
