//! Exercise performed formations, stores and ordinary relays before examining
//! their sealed output graphs. No test supplies a permission-role annotation.
use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractCallTarget, AbstractFunction, AbstractFunctionResult,
    AbstractOperation as O, AbstractOperationPlan, AbstractStructuralCallResult,
};
use semantic_vocabulary::{
    EdgeId, MachineId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralRecordStorage, StructuralStoreDisposition,
    StructuralTypeShape, TerminalDynamicDispatchCatalog, TerminalIdentityIndex,
    TerminalIdentityTable, TerminalIdentityTableBuilder, TerminalPsiIdentity, VocabularyMarker,
};

fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}
fn machine(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}
fn ty(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}
fn op(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

struct Fixture {
    identities: TerminalIdentityTable,
    body: TerminalIdentityIndex,
    types: Vec<StructuralTypeDeclaration>,
}

impl Fixture {
    fn new() -> Self {
        let mut names = TerminalIdentityTableBuilder::default();
        let scalar = names.intern("test::Bit");
        let reference = names.intern("test::MutableBit");
        let slot = names.intern("test::Slot");
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
        Self {
            identities: names.build(),
            body,
            types: vec![
                declaration(
                    1,
                    scalar,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                ),
                declaration(
                    2,
                    reference,
                    StructuralTypeShape::Reference {
                        referent: ty(1),
                        access: StructuralAccess::MutableBorrow,
                    },
                ),
                declaration(
                    3,
                    slot,
                    StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::Packed,
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: body,
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(ty(2)),
                            storage: None,
                            inline_carrier: None,
                        }],
                    },
                ),
            ],
        }
    }

    fn parameter(&self, raw: u64, position: u32) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: place(raw),
            position,
            structural_type: ty(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    fn formation(&self, raw: u64, result: u64, source: u64) -> O {
        O::EstablishReference {
            psi_operation: op(raw),
            storage: None,
            result: StructuralOperationResult {
                place: place(result),
                structural_type: ty(2),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            source: StructuralArgument {
                place: place(source),
                path: vec![
                    StructuralPathSegment::Field(self.body),
                    StructuralPathSegment::Referent,
                ],
                access: StructuralAccess::MutableBorrow,
            },
        }
    }

    fn store(&self, raw: u64, destination: u64, position: u32, value: u64) -> O {
        O::StoreStructuralField {
            psi_operation: op(raw),
            destination: self.parameter(destination, position).into(),
            path: Vec::new(),
            field: Some(StructuralFieldId::new(1).unwrap()),
            disposition: StructuralStoreDisposition::Vacant,
            value: StructuralArgument {
                place: place(value),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        }
    }

    fn pointer(&self, raw: u64) -> StructuralOperationResult {
        StructuralOperationResult {
            place: place(raw),
            structural_type: ty(2),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }
    }

    fn extract(&self, raw: u64, result: u64, source: u64, position: u32) -> O {
        O::MoveStructuralField {
            psi_operation: op(raw),
            result: self.pointer(result),
            source: self.parameter(source, position).into(),
            path: Vec::new(),
            field: Some(StructuralFieldId::new(1).unwrap()),
        }
    }

    fn replace(&self, raw: u64, destination: u64, position: u32, value: u64) -> Vec<O> {
        // Empty the old word through an actual move, then end that exact loan.
        // The final store restores the borrowed container's real vacancy.
        let retired = 1000 + raw;
        vec![
            self.extract(raw, retired, destination, position),
            O::ReleaseReference {
                psi_operation: op(raw + 1),
                source: place(retired),
                path: Vec::new(),
            },
            self.store(raw + 2, destination, position, value),
        ]
    }

    fn function(&self, raw: u64, roots: &[u64], mut operations: Vec<O>) -> AbstractFunction {
        operations.push(O::ReturnUnit {
            psi_edge: EdgeId::new(raw).unwrap(),
            cleanup_actions: Vec::new(),
        });
        AbstractFunction {
            machine: machine(raw),
            attachment: None,
            nominal_cleanup_subject: None,
            entry: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            structural_parameters: roots
                .iter()
                .enumerate()
                .map(|(position, raw)| self.parameter(*raw, position as u32))
                .collect(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(raw).unwrap(),
                operation_offset: 0,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
            }],
            operations,
        }
    }

    fn relay(&self, raw: u64, callee: u64) -> AbstractFunction {
        self.function(
            raw,
            &[101, 102],
            vec![O::CallStructural {
                psi_operation: op(100),
                callee: AbstractCallTarget::Machine(machine(callee)),
                arguments: Vec::new(),
                structural_arguments: vec![
                    self.parameter(101, 0).into(),
                    self.parameter(102, 1).into(),
                ],
                claim_transfers: Vec::new(),
                result: AbstractStructuralCallResult::Unit,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }],
        )
    }

    fn catalog(&self, functions: Vec<AbstractFunction>) -> ReferenceOutputCatalog {
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
            },
            entry: functions[0].machine,
            structural_types: self.types.clone().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
            identities: self.identities.clone(),
            functions,
        };
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &plan,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        reconstruct_reference_output_catalog(&unit, &[], |_, node| {
            Some(std::borrow::Cow::Borrowed(&node.operation))
        })
        .unwrap()
    }
}

fn output<'a>(
    body: &'a ReferenceBodyOutputs,
    parameter: usize,
) -> (usize, &'a ReferenceOutputNode) {
    assert_eq!(body.exits().len(), 1);
    body.exits()[0].graph().iter().enumerate().find(|(_, node)|
        matches!(node.residence(), ReferenceOutputResidence::Parameter { slot } if body.slots()[*slot].parameter() == parameter)
    ).unwrap()
}

#[test]
fn performed_reborrow_and_ordinary_relay_retain_the_actual_parent() {
    let fixture = Fixture::new();
    let callee = fixture.function(
        1,
        &[1, 2],
        std::iter::once(fixture.formation(1, 10, 1))
            .chain(fixture.replace(2, 2, 1, 10))
            .collect(),
    );
    let catalog = fixture.catalog(vec![callee, fixture.relay(2, 1)]);
    for raw in [1, 2] {
        let body = catalog
            .body(machine(raw))
            .unwrap_or_else(|| panic!("{:?}", catalog.unavailable_reason(machine(raw))));
        let (parent, unchanged) = output(&body, 0);
        assert!(matches!(
            unchanged.supply(),
            ReferenceOutputSupply::Entry(_)
        ));
        let (_, child) = output(&body, 1);
        assert!(
            matches!(child.supply(), ReferenceOutputSupply::Constructed {
            parent: ReferenceOutputParent::Node(actual), ..
        } if *actual == parent)
        );
    }
}

#[test]
fn two_exported_reborrows_keep_the_immediate_parent_edge() {
    let fixture = Fixture::new();
    let callee = fixture.function(
        1,
        &[1, 2, 3],
        std::iter::once(fixture.formation(1, 10, 1))
            .chain(fixture.replace(2, 2, 1, 10))
            .chain(std::iter::once(fixture.formation(5, 11, 2)))
            .chain(fixture.replace(6, 3, 2, 11))
            .collect(),
    );
    let catalog = fixture.catalog(vec![callee]);
    let body = catalog
        .body(machine(1))
        .unwrap_or_else(|| panic!("{:?}", catalog.unavailable_reason(machine(1))));
    let (parent, _) = output(&body, 1);
    let (_, child) = output(&body, 2);
    assert!(
        matches!(child.supply(), ReferenceOutputSupply::Constructed {
        parent: ReferenceOutputParent::Node(actual), ..
    } if *actual == parent)
    );
}

#[test]
fn equal_backing_cannot_make_two_exclusive_children_siblings() {
    let fixture = Fixture::new();
    let invalid = fixture.function(
        1,
        &[1, 2, 3],
        std::iter::once(fixture.formation(1, 10, 1))
            .chain(fixture.replace(2, 2, 1, 10))
            .chain(std::iter::once(fixture.formation(5, 11, 1)))
            .chain(fixture.replace(6, 3, 2, 11))
            .collect(),
    );
    let catalog = fixture.catalog(vec![invalid]);
    assert!(catalog.body(machine(1)).is_none());
    assert!(
        catalog
            .unavailable_reason(machine(1))
            .is_some_and(|reason| reason.contains("suspended by a live child"))
    );
}

#[test]
fn missing_and_active_recursive_bodies_do_not_supply_empty_outputs() {
    let fixture = Fixture::new();
    for callee in [2, 3] {
        let catalog = fixture.catalog(vec![fixture.relay(2, callee)]);
        assert!(catalog.body(machine(2)).is_none());
        assert!(catalog.unavailable_reason(machine(2)).is_some());
    }
}

#[test]
fn known_unchanged_stored_slots_keep_a_complete_entry_graph() {
    let fixture = Fixture::new();
    let catalog = fixture.catalog(vec![fixture.function(1, &[1], Vec::new())]);
    let body = catalog
        .body(machine(1))
        .unwrap_or_else(|| panic!("{:?}", catalog.unavailable_reason(machine(1))));
    assert_eq!(body.slots().len(), 1);
    assert_eq!(body.exits()[0].cleared_slots(), Some(&[][..]));
    let (_, unchanged) = output(&body, 0);
    assert!(matches!(
        unchanged.supply(),
        ReferenceOutputSupply::Entry(_)
    ));
}

#[test]
fn performed_move_and_refill_keep_the_transferred_occurrence() {
    let fixture = Fixture::new();
    let mut body = fixture.function(
        1,
        &[1, 2],
        std::iter::once(fixture.extract(1, 10, 1, 0))
            .chain(std::iter::once(fixture.store(2, 1, 0, 3)))
            .chain(fixture.replace(3, 2, 1, 10))
            .collect(),
    );
    body.structural_parameters
        .push(StructuralParameterDeclaration {
            place: place(3),
            position: 2,
            structural_type: ty(2),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let catalog = fixture.catalog(vec![body]);
    let body = catalog
        .body(machine(1))
        .unwrap_or_else(|| panic!("{:?}", catalog.unavailable_reason(machine(1))));
    let input_entry = body
        .entry()
        .iter()
        .position(|entry| entry.carrier == place(1))
        .unwrap();
    let spare_entry = body
        .entry()
        .iter()
        .position(|entry| entry.carrier == place(3))
        .unwrap();
    assert!(
        matches!(output(&body, 1).1.supply(), ReferenceOutputSupply::Entry(entry) if *entry == input_entry)
    );
    assert!(
        matches!(output(&body, 0).1.supply(), ReferenceOutputSupply::Entry(entry) if *entry == spare_entry)
    );
}

#[test]
fn parent_reseating_does_not_redirect_an_exported_child() {
    let fixture = Fixture::new();
    let body = fixture.function(
        1,
        &[1, 2],
        std::iter::once(fixture.formation(1, 10, 1))
            .chain(fixture.replace(2, 2, 1, 10))
            .chain(std::iter::once(fixture.extract(5, 11, 1, 0)))
            .collect(),
    );
    let catalog = fixture.catalog(vec![body]);
    assert!(catalog.body(machine(1)).is_none());
    assert!(
        catalog
            .unavailable_reason(machine(1))
            .is_some_and(|reason| reason.contains("suspended")),
        "{:?}",
        catalog.unavailable_reason(machine(1))
    );
}

#[test]
fn owned_ingress_formation_keeps_the_parent_until_the_child_ends() {
    let fixture = Fixture::new();
    let mut body = fixture.function(
        1,
        &[1],
        vec![
            fixture.formation(1, 10, 1),
            O::ReleaseReference {
                psi_operation: op(2),
                source: place(10),
                path: Vec::new(),
            },
        ],
    );
    body.structural_parameters[0].access = StructuralAccess::Owned;
    body.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let Some(O::ReturnUnit {
        cleanup_actions, ..
    }) = body.operations.last_mut()
    else {
        panic!()
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        place(1),
    ));
    let catalog = fixture.catalog(vec![body]);
    let body = catalog
        .body(machine(1))
        .unwrap_or_else(|| panic!("{:?}", catalog.unavailable_reason(machine(1))));
    assert!(body.exits()[0].stored().is_empty());
    assert!(body.exits()[0].returned().is_empty());
}
