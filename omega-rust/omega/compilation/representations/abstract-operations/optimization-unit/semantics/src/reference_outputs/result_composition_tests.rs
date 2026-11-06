//! A real access-reference factory returns through an ordinary relay before
//! another body stores the reference in caller storage. The public result is
//! only a WithinRegion ceiling; the actual conversion must supply its capture.
//!
//! This raw-unit test identity asserts no canonical admission or runtime rights.

use super::{
    ReferenceBodyOutputs, ReferenceOutputCatalog, ReferenceOutputInterpretation,
    ReferenceOutputResidence, reconstruct_reference_output_catalog,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractCallTarget, AbstractFunction, AbstractFunctionResult,
    AbstractOperation as O, AbstractOperationPlan, AbstractStructuralCallResult,
    AccessReferenceBacking, StructuralTypeCatalog,
};
use access_plans::{AccessFields, AccessPlan, BoundaryReach, validate_access_plan};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use semantic_vocabulary::{
    AuthoredRecordStorage, BlockId, EdgeId, MachineId, OperationId, PlaceId, ReferenceOriginExtent,
    ScalarType, StructuralFieldId, StructuralTypeId,
};
use std::borrow::Cow;
use terminal_psi::{
    BindingRelevance, ReferenceOriginSource, SemanticFingerprint, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralRecordStorage, StructuralReferenceOrigin, StructuralReferenceResultSource,
    StructuralResultDeclaration, StructuralStoreDisposition, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAccessViewPlan, TerminalDynamicDispatchCatalog,
    TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder,
    TerminalPsiIdentity, VocabularyMarker,
};

fn ty(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}
fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}
fn machine(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}
fn op(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

struct Fixture {
    identities: TerminalIdentityTable,
    body: TerminalIdentityIndex,
    types: StructuralTypeCatalog,
    backing: AccessReferenceBacking,
}

impl Fixture {
    fn new() -> Self {
        let mut names = TerminalIdentityTableBuilder::default();
        let schema_name = names.intern("test::Schema");
        let view_name = names.intern("test::AccessView");
        let reference_name = names.intern("test::SharedAccessReference");
        let slot_name = names.intern("test::Slot");
        let payload = names.intern("payload");
        let body = names.intern("body");
        let policy = names.intern("test::PlacementPolicy");
        let policy_plan = names.intern("test::PlacementPolicy::plan");
        let declaration = |raw, identity, shape| StructuralTypeDeclaration {
            id: ty(raw),
            identity,
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: raw == 1,
            no_code_affine_cleanup: raw == 3 || raw == 4,
            descriptor_cleanup: raw == 3 || raw == 4,
            scalar_restriction: None,
        };
        let declarations = vec![
            declaration(
                1,
                schema_name,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: payload,
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        storage: None,
                        inline_carrier: None,
                    }],
                },
            ),
            declaration(
                2,
                view_name,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::AccessView(AuthoredRecordStorage {
                        byte_size: 1,
                        byte_alignment: 1,
                    }),
                    // No accessor is exposed. The whole interpretation still
                    // has issuing geometry and must survive as a descriptor.
                    fields: Vec::new(),
                },
            ),
            declaration(
                3,
                reference_name,
                StructuralTypeShape::Reference {
                    referent: ty(2),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            declaration(
                4,
                slot_name,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: body,
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(ty(3)),
                        storage: None,
                        inline_carrier: None,
                    }],
                },
            ),
        ];
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 19,
            offsets: None,
            size: Some(1),
            align: 1,
            entries: vec![LayoutFieldEntryReport {
                field: "payload".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
        };
        let placement = validate_access_plan(AccessPlan {
            access: AccessFields::inaccessible(&layout).unwrap(),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("validate the actual whole-view placement");
        let plan = TerminalAccessViewPlan {
            carrier: ty(2),
            schema: ty(1),
            view_identity: view_name,
            policy_identity: policy,
            policy_plan_machine_identity: policy_plan,
            schema_identity: schema_name,
            placement,
            fields: Vec::new(),
        };
        let correspondence = terminal_semantics::access_ram_plan_correspondence(
            &declarations,
            std::slice::from_ref(&plan),
            &plan,
            ty(1),
        )
        .expect("derive correspondence from the real schema and placement owners");
        let backing = AccessReferenceBacking::OrdinaryRam(correspondence);
        crate::access_references::validate_backing(
            &declarations,
            std::slice::from_ref(&plan),
            &plan,
            ty(1),
            &backing,
        )
        .expect("the performed backing must meet the same prerequisite as the body");
        assert!(terminal_semantics::discardable_store_value(
            declarations.iter(),
            ty(3),
            StructuralMultiplicity::Affine,
            false,
        ));
        Self {
            identities: names.build(),
            body,
            types: StructuralTypeCatalog::from(declarations).with_access_views(vec![plan]),
            backing,
        }
    }

    fn source(&self, raw: u64) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: place(raw),
            position: 0,
            structural_type: ty(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    fn result(&self, raw: u64) -> StructuralOperationResult {
        StructuralOperationResult {
            place: place(raw),
            structural_type: ty(3),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }
    }

    fn result_contract(&self, raw: u64, source: u64) -> AbstractFunctionResult {
        AbstractFunctionResult::Structural(StructuralResultDeclaration {
            place: place(raw),
            structural_type: ty(3),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: Vec::new(),
                sources: vec![StructuralReferenceOrigin::Parameter(
                    ReferenceOriginSource {
                        place: place(source),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                        extent: ReferenceOriginExtent::WithinRegion,
                    },
                )],
            }],
        })
    }

    fn function(
        &self,
        raw: u64,
        parameters: Vec<StructuralParameterDeclaration>,
        result: AbstractFunctionResult,
        operations: Vec<O>,
    ) -> AbstractFunction {
        let block = BlockId::new(raw * 10 + 1).unwrap();
        AbstractFunction {
            machine: machine(raw),
            attachment: None,
            nominal_cleanup_subject: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: parameters,
            result,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                operation_offset: 0,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
            }],
            operations,
        }
    }

    fn call(&self, operation: u64, callee: u64, source: u64, result: u64) -> O {
        O::CallStructural {
            psi_operation: op(operation),
            callee: AbstractCallTarget::Machine(machine(callee)),
            arguments: Vec::new(),
            structural_arguments: vec![self.source(source).into()],
            claim_transfers: Vec::new(),
            result: AbstractStructuralCallResult::Structural {
                result: self.result(result),
                selected_evidence: Vec::new(),
            },
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn unit(&self, corrupt_issuer: bool) -> optimization_unit::PsiOptimizationUnit {
        let mut issued = self.backing.clone();
        if corrupt_issuer {
            let AccessReferenceBacking::OrdinaryRam(backing) = &mut issued else {
                unreachable!();
            };
            backing.source_byte_length += 1;
        }
        let factory = self.function(
            1,
            vec![self.source(101)],
            self.result_contract(109, 101),
            vec![
                O::EstablishAccessReference {
                    psi_operation: op(1001),
                    result: self.result(109),
                    source: self.source(101).into(),
                    correspondence: issued,
                },
                O::ReturnStructural {
                    psi_edge: EdgeId::new(101).unwrap(),
                    source: place(109),
                    returned_claims: Vec::new(),
                    trivial_affine_locals: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            ],
        );
        let relay = self.function(
            2,
            vec![self.source(201)],
            self.result_contract(209, 201),
            vec![
                self.call(2001, 1, 201, 209),
                O::ReturnStructural {
                    psi_edge: EdgeId::new(201).unwrap(),
                    source: place(209),
                    returned_claims: Vec::new(),
                    trivial_affine_locals: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            ],
        );
        let output = StructuralParameterDeclaration {
            place: place(302),
            position: 1,
            structural_type: ty(4),
            // A borrowed carrier does not own its contained pointer words.
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let store = self.function(
            3,
            vec![self.source(301), output.clone()],
            AbstractFunctionResult::Unit,
            vec![
                self.call(3001, 2, 301, 309),
                O::StoreStructuralField {
                    psi_operation: op(3002),
                    destination: output.into(),
                    path: Vec::new(),
                    field: Some(StructuralFieldId::new(2).unwrap()),
                    disposition: StructuralStoreDisposition::ReplaceDiscardable,
                    value: StructuralArgument::whole(place(309), StructuralAccess::Owned),
                },
                O::ReturnUnit {
                    psi_edge: EdgeId::new(301).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        );
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
            },
            entry: machine(3),
            structural_types: self.types.clone(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
            identities: self.identities.clone(),
            functions: vec![factory, relay, store],
        };
        optimization_unit::reconstruct_psi_optimization_unit_seed(
            &plan,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .expect("construct actual function CFGs and derived definition/fact indexes")
    }

    fn catalog(&self, unit: &optimization_unit::PsiOptimizationUnit) -> ReferenceOutputCatalog {
        reconstruct_reference_output_catalog(unit, &[], |_, node| {
            Some(Cow::Borrowed(&node.operation))
        })
        .expect("prepare the same immutable operation view for all three bodies")
    }

    fn assert_stored_capture(&self, body: &ReferenceBodyOutputs) {
        assert_eq!(body.machine(), machine(3));
        assert!(body.compatibility_requirements().is_empty());
        assert_eq!(body.slots().len(), 1);
        assert_eq!(body.slots()[0].parameter(), 1);
        assert_eq!(
            body.slots()[0].path(),
            &[StructuralPathSegment::Field(self.body)]
        );
        assert_eq!(body.exits().len(), 1);
        let outputs = body.exits()[0]
            .graph()
            .iter()
            .filter(|node| {
                matches!(
                    node.residence(),
                    ReferenceOutputResidence::Parameter { slot: 0 }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].reference_type(), ty(3));
        let ReferenceOutputInterpretation::Captured(actual) = outputs[0].interpretation() else {
            panic!("a WithinRegion result ceiling did not retain the actual factory capture");
        };
        assert_eq!(actual, &self.backing);
    }
}

#[test]
fn access_factory_region_result_keeps_capture_through_relay_and_stored_output() {
    let fixture = Fixture::new();
    let unit = fixture.unit(false);
    crate::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[]).expect(
        "the real factory, ordinary relay and descriptor store satisfy ownership prerequisites",
    );
    let catalog = fixture.catalog(&unit);
    let body = catalog.body(machine(3)).unwrap_or_else(|| {
        panic!(
            "stored body unavailable: {:?}",
            catalog.unavailable_reason(machine(3))
        )
    });
    fixture.assert_stored_capture(&body);
}

#[test]
fn inconsistent_performed_factory_backing_cannot_donate_a_stored_capture() {
    let fixture = Fixture::new();
    let valid = fixture.unit(false);
    let valid_catalog = fixture.catalog(&valid);
    fixture.assert_stored_capture(&valid_catalog.body(machine(3)).expect("positive control"));

    // Change only the actual factory opcode's issuing correspondence. No
    // result annotation, requested output type or relay can repair that proof.
    let invalid = fixture.unit(true);
    assert!(
        crate::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&invalid, &[])
            .is_err()
    );
    let catalog = fixture.catalog(&invalid);
    assert!(catalog.body(machine(1)).is_none());
    assert!(catalog.body(machine(2)).is_none());
    assert!(catalog.body(machine(3)).is_none());
}
