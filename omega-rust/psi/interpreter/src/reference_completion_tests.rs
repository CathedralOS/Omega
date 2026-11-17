//! Host references retain the supplied scalar cells, not independent snapshots.
use crate::boundary::effects::boundary_predicates::tests::idle_boundary;
use crate::engine::calls::frames::tests::empty_program;
use crate::engine::execution::TerminalExecution;
use crate::{
    TerminalAggregateValue, TerminalBoundaryStorageValue, TerminalEffectRejection,
    TerminalEffectResult, TerminalReferencePayload, TerminalReferenceResult,
    TerminalReferenceScalarBacking, TerminalScalarBacking, TerminalScalarValue,
    TerminalStorageContents,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerValue, StructuralFieldId,
};
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, StructuralTypeId};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use terminal_psi::*;

fn ty(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: StructuralTypeId::new(id).unwrap(),
        identity: TerminalIdentityIndex(0),
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    }
}

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
}

fn fixture() -> (
    TerminalExecution<'static>,
    BoundaryMachineDeclaration,
    StructuralOperationResult,
) {
    setup(
        vec![ty(1, StructuralTypeShape::PrimitiveScalar(scalar()))],
        1,
    )
}

fn setup(
    mut types: Vec<StructuralTypeDeclaration>,
    referent: u64,
) -> (
    TerminalExecution<'static>,
    BoundaryMachineDeclaration,
    StructuralOperationResult,
) {
    let mut program = empty_program();
    let reference = types.iter().map(|ty| ty.id.get()).max().unwrap() + 1;
    types.push(ty(
        reference,
        StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(referent).unwrap(),
            access: StructuralAccess::SharedBorrow,
        },
    ));
    Arc::get_mut(&mut program)
        .unwrap()
        .fixture_module_mut()
        .structural_types = types;
    let result = StructuralOperationResult {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(reference).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let mut boundary = idle_boundary();
    boundary.result = BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    boundary.result_lifetimes = Some(vec![DeclaredResultLifetime {
        inputs: Vec::new(),
        results: vec![Vec::new()],
    }]);
    let module = Arc::get_mut(&mut program).unwrap().fixture_module_mut();
    module.boundary_machines.push(boundary.clone());
    for index in 1..=2 {
        let operation = semantic_vocabulary::OperationId::new(index).unwrap();
        let mut declared = result.clone();
        declared.place = PlaceId::new(index).unwrap();
        let kind = if index == 1 {
            OperationKind::BoundaryCall {
                boundary: boundary.id,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            }
        } else {
            declared.multiplicity = StructuralMultiplicity::Affine;
            OperationKind::EstablishReference {
                source: StructuralArgument {
                    place: result.place,
                    path: vec![StructuralPathSegment::Referent],
                    access: StructuralAccess::SharedBorrow,
                },
                storage: None,
            }
        };
        module.machines[0]
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: declared.place,
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: declared.structural_type,
                },
            });
        module.machines[0].blocks[0]
            .operations
            .push(Operation::unsuspended(
                operation,
                OperationResult::Structural(declared),
                kind,
            ));
    }
    let execution = TerminalExecution::start_with_program(
        program,
        &[],
        crate::TerminalStructuralInputs::default(),
        None,
        None,
    )
    .unwrap();
    (execution, boundary, result)
}

#[test]
fn canonical_provider_reference_preflights_the_actual_result() {
    let (execution, boundary, result) = fixture();
    assert!(
        terminal_semantics::boundary_result_slots(
            &execution.program.catalogs.structural_types,
            &boundary,
            terminal_semantics::descriptor_results::DescriptorSlotKind::BorrowedStorage,
            &result,
        )
        .is_ok()
    );
    execution
        .preflight_boundary_result(&boundary, &OperationResult::Structural(result))
        .unwrap();
}

#[derive(Debug)]
struct Backing {
    value: RefCell<TerminalScalarValue>,
    stale: Cell<bool>,
}
impl TerminalScalarBacking for Backing {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        if self.stale.get() {
            return Err(TerminalEffectRejection::new("backing retired"));
        }
        Ok(*self.value.borrow())
    }
    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        *self.value.borrow_mut() = value;
        Ok(())
    }
}
fn word(scalar: ScalarType, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: scalar.as_integer().unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}
fn backing(value: TerminalScalarValue) -> Rc<Backing> {
    Rc::new(Backing {
        value: RefCell::new(value),
        stale: Cell::new(false),
    })
}
fn resident(
    root: u64,
    fields: Vec<(Vec<Path>, TerminalScalarValue, ScalarType, Rc<Backing>)>,
) -> TerminalReferenceResult {
    TerminalReferenceResult {
        referent: TerminalReferencePayload::Resident {
            aggregate: TerminalAggregateValue {
                structural_type: StructuralTypeId::new(root).unwrap(),
                result_case: None,
                fields: fields
                    .iter()
                    .map(
                        |(path, value, stored_scalar, _)| TerminalBoundaryStorageValue {
                            path: path.clone(),
                            contents: TerminalStorageContents::Scalar {
                                value: *value,
                                stored_scalar: *stored_scalar,
                            },
                        },
                    )
                    .collect(),
            },
            scalars: fields
                .into_iter()
                .map(|(path, _, _, backing)| TerminalReferenceScalarBacking { path, backing })
                .collect(),
        },
        references: Vec::new(),
    }
}
fn commit(
    execution: &mut TerminalExecution<'_>,
    boundary: &BoundaryMachineDeclaration,
    result: &StructuralOperationResult,
    supply: TerminalReferenceResult,
) -> Result<(), crate::TerminalInterpretError> {
    execution.preflight_boundary_result(boundary, &OperationResult::Structural(result.clone()))?;
    execution.commit_host_boundary_result(
        boundary,
        &OperationResult::Structural(result.clone()),
        TerminalEffectResult::Reference(supply),
    )
}
fn read(
    execution: &TerminalExecution<'_>,
    result: &StructuralOperationResult,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
) -> Result<TerminalScalarValue, crate::TerminalInterpretError> {
    let mut full = vec![StructuralPathSegment::Referent];
    full.extend_from_slice(path);
    execution.read_primitive_place(result.place, &full, scalar)
}

#[test]
fn resident_payload_clone_and_equality_keep_identity_without_observing_backing() {
    let home = backing(word(scalar(), 37));
    let supply = resident(
        1,
        vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
    );
    home.stale.set(true);
    assert_eq!(supply, supply.clone());
    assert_ne!(
        supply,
        resident(
            1,
            vec![(
                vec![],
                word(scalar(), 37),
                scalar(),
                backing(word(scalar(), 37))
            )]
        )
    );
}

#[test]
fn resident_reference_reads_live_contents_and_refuses_shared_writes_and_stale_reads() {
    let (mut execution, boundary, result) = fixture();
    let home = backing(word(scalar(), 37));
    commit(
        &mut execution,
        &boundary,
        &result,
        resident(
            1,
            vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
        ),
    )
    .unwrap();
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 37)
    );
    *home.value.borrow_mut() = word(scalar(), 91);
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 91)
    );
    let carrier = execution.structural_values.get(&result.place).unwrap();
    let referent = &execution
        .reference_referents
        .get(&carrier.into())
        .unwrap()
        .view;
    let cell = execution
        .structural_primitive_storage
        .get(&referent.into())
        .unwrap();
    let mut copied_cell = cell.clone();
    assert_eq!(&copied_cell, cell);
    assert!(copied_cell.write(word(scalar(), 99)).is_err());
    assert_eq!(*home.value.borrow(), word(scalar(), 91));
    *home.value.borrow_mut() = TerminalScalarValue::Boolean(true);
    assert!(read(&execution, &result, &[], scalar()).is_err());
    *home.value.borrow_mut() = word(scalar(), 256);
    assert!(read(&execution, &result, &[], scalar()).is_err());
    home.stale.set(true);
    assert!(read(&execution, &result, &[], scalar()).is_err());
}

fn bounded() -> StructuralFieldType {
    StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            scalar().as_integer().unwrap(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(127),
        )
        .unwrap(),
    )
}
#[test]
fn resident_primitive_and_packed_array_recheck_restrictions_on_every_read() {
    for array in [false, true] {
        let mut element = ty(1, StructuralTypeShape::PrimitiveScalar(scalar()));
        element.scalar_restriction = Some(bounded());
        let mut types = vec![element];
        let root = if array {
            types.push(ty(
                2,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(1).unwrap(),
                    length: 2.into(),
                },
            ));
            2
        } else {
            1
        };
        let (mut execution, boundary, result) = setup(types, root);
        let homes = [backing(word(scalar(), 37)), backing(word(scalar(), 63))];
        let fields = if array {
            vec![
                (
                    vec![Path::FixedIndex(0)],
                    word(scalar(), 37),
                    scalar(),
                    homes[0].clone(),
                ),
                (
                    vec![Path::FixedIndex(1)],
                    word(scalar(), 63),
                    scalar(),
                    homes[1].clone(),
                ),
            ]
        } else {
            vec![(vec![], word(scalar(), 37), scalar(), homes[0].clone())]
        };
        commit(&mut execution, &boundary, &result, resident(root, fields)).unwrap();
        let path = if array {
            vec![StructuralPathSegment::FixedIndex(0)]
        } else {
            vec![]
        };
        *homes[0].value.borrow_mut() = word(scalar(), 127);
        assert_eq!(
            read(&execution, &result, &path, scalar()).unwrap(),
            word(scalar(), 127)
        );
        *homes[0].value.borrow_mut() = word(scalar(), 128);
        assert!(read(&execution, &result, &path, scalar()).is_err());
        if array {
            assert_eq!(
                read(
                    &execution,
                    &result,
                    &[StructuralPathSegment::FixedIndex(1)],
                    scalar()
                )
                .unwrap(),
                word(scalar(), 63)
            );
        }
    }
}

#[test]
fn resident_record_rechecks_stored_representation_and_field_restrictions() {
    let wide = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let field_id = StructuralFieldId::new(1).unwrap();
    let field = StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        identity: TerminalIdentityIndex(1),
        id: field_id,
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(wide),
        storage: Some(semantic_vocabulary::AuthoredFieldStorage::Scalar {
            byte_offset: 0,
            stored_scalar: Some(scalar()),
        }),
    };
    let (mut execution, boundary, result) = setup(
        vec![ty(
            1,
            StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        declaration_identity: Default::default(),
                        inline_carrier: None,
                        identity: TerminalIdentityIndex(2),
                        id: StructuralFieldId::new(2).unwrap(),
                        relevance: BindingRelevance::Relevant,
                        field_type: bounded(),
                        storage: None,
                    },
                    field,
                ],
                storage: StructuralRecordStorage::Packed,
            },
        )],
        1,
    );
    let home = backing(word(wide, 37));
    let sibling = backing(word(scalar(), 63));
    let mut supply = resident(
        1,
        vec![
            (
                vec![Path::Field(field_id)],
                word(wide, 37),
                scalar(),
                home.clone(),
            ),
            (
                vec![Path::Field(StructuralFieldId::new(2).unwrap())],
                word(scalar(), 63),
                scalar(),
                sibling.clone(),
            ),
        ],
    );
    let TerminalReferencePayload::Resident { aggregate, .. } = &mut supply.referent else {
        unreachable!()
    };
    aggregate.fields.reverse(); // declaration order differs from canonical field-ID order
    commit(&mut execution, &boundary, &result, supply).unwrap();
    let path = [StructuralPathSegment::Field(TerminalIdentityIndex(1))];
    *home.value.borrow_mut() = word(wide, 255);
    assert_eq!(
        read(&execution, &result, &path, wide).unwrap(),
        word(wide, 255)
    );
    *home.value.borrow_mut() = word(wide, 256);
    assert!(read(&execution, &result, &path, wide).is_err());
    *home.value.borrow_mut() = word(wide, 37);
    *sibling.value.borrow_mut() = word(scalar(), 128);
    assert!(
        read(
            &execution,
            &result,
            &[StructuralPathSegment::Field(TerminalIdentityIndex(2))],
            scalar()
        )
        .is_err()
    );
}

#[test]
fn resident_roster_and_initial_values_refuse_before_storage_publication() {
    for invalid in 0..9 {
        let (mut execution, boundary, result) = setup(
            vec![
                ty(1, StructuralTypeShape::PrimitiveScalar(scalar())),
                ty(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: StructuralTypeId::new(1).unwrap(),
                        length: 2.into(),
                    },
                ),
            ],
            2,
        );
        let homes = [backing(word(scalar(), 37)), backing(word(scalar(), 63))];
        let mut supply = resident(
            2,
            vec![
                (
                    vec![Path::FixedIndex(0)],
                    word(scalar(), 37),
                    scalar(),
                    homes[0].clone(),
                ),
                (
                    vec![Path::FixedIndex(1)],
                    word(scalar(), 63),
                    scalar(),
                    homes[1].clone(),
                ),
            ],
        );
        let TerminalReferencePayload::Resident { aggregate, scalars } = &mut supply.referent else {
            unreachable!()
        };
        match invalid {
            0 => {
                scalars.pop();
            }
            1 => scalars.swap(0, 1),
            2 => scalars[1].path = scalars[0].path.clone(),
            3 => scalars[1].path = vec![Path::FixedIndex(2)],
            4 => {
                *homes[0].value.borrow_mut() = word(scalar(), 38);
            }
            5 => {
                aggregate.fields[0].contents = TerminalStorageContents::Scalar {
                    value: TerminalScalarValue::Boolean(true),
                    stored_scalar: ScalarType::Boolean,
                }
            }
            6 => aggregate.structural_type = StructuralTypeId::new(1).unwrap(),
            7 => {
                aggregate.fields[0].contents = TerminalStorageContents::Scalar {
                    value: word(scalar(), 256),
                    stored_scalar: scalar(),
                }
            }
            8 => {
                aggregate.fields[0].contents = TerminalStorageContents::Scalar {
                    value: word(scalar(), 37),
                    stored_scalar: ScalarType::Boolean,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            commit(&mut execution, &boundary, &result, supply).is_err(),
            "invalid {invalid}"
        );
        assert!(!execution.structural_values.contains_key(&result.place));
        assert_eq!(execution.structural_primitive_storage.len(), 0);
        assert_eq!(execution.structural_scalar_fields.len(), 0);
        assert!(execution.structural_byte_storage.is_empty());
        assert!(execution.reference_referents.is_empty());
    }
}

#[test]
fn unknown_or_malformed_provider_frontiers_still_refuse() {
    for invalid in 0..6 {
        let (execution, mut boundary, result) = fixture();
        match invalid {
            0 => boundary.result_lifetimes = None,
            1 => boundary.result_lifetimes = Some(vec![]),
            2 => boundary.result_lifetimes.as_mut().unwrap()[0]
                .results
                .push(vec![]),
            3 => {
                boundary.result_lifetimes.as_mut().unwrap()[0].results[0] =
                    vec![StructuralPathSegment::FixedIndex(0)]
            }
            4 => boundary.result_lifetimes.as_mut().unwrap()[0].inputs.push(
                BoundaryLifetimeDependency::Referent {
                    argument_index: 0,
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            5 => {
                let BoundaryMachineResult::Structural(declared) = &mut boundary.result else {
                    unreachable!()
                };
                declared.structural_type = StructuralTypeId::new(1).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            execution
                .preflight_boundary_result(&boundary, &OperationResult::Structural(result))
                .is_err(),
            "invalid {invalid}"
        );
    }
}

#[test]
fn resident_rows_release_backing_after_the_last_reborrow_retires() {
    for array in [false, true] {
        let types = if array {
            vec![
                ty(1, StructuralTypeShape::PrimitiveScalar(scalar())),
                ty(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: StructuralTypeId::new(1).unwrap(),
                        length: 1.into(),
                    },
                ),
            ]
        } else {
            vec![ty(1, StructuralTypeShape::PrimitiveScalar(scalar()))]
        };
        let root = if array { 2 } else { 1 };
        let (mut execution, boundary, result) = setup(types, root);
        let home = backing(word(scalar(), 37));
        let weak = Rc::downgrade(&home);
        let path = if array {
            vec![Path::FixedIndex(0)]
        } else {
            vec![]
        };
        commit(
            &mut execution,
            &boundary,
            &result,
            resident(
                root,
                vec![(path, word(scalar(), 37), scalar(), home.clone())],
            ),
        )
        .unwrap();
        drop(home);
        let first = execution
            .structural_values
            .get(&result.place)
            .unwrap()
            .clone();
        let mut cursor = execution.local_structural_identities.cursor();
        let prepared = execution
            .capture_reference(
                &StructuralArgument {
                    place: result.place,
                    access: StructuralAccess::SharedBorrow,
                    path: vec![StructuralPathSegment::Referent],
                },
                result.structural_type,
                &[],
                None,
                &mut cursor,
            )
            .unwrap();
        execution.local_structural_identities.commit_cursor(cursor);
        // Use the ordinary reference capture, including any primitive or array loan.
        let mut second = first.clone();
        second.opaque_identity = execution.local_structural_identities.allocate().unwrap();
        let second_place = PlaceId::new(2).unwrap();
        execution
            .structural_values
            .insert(second_place, second.clone())
            .unwrap();
        execution
            .reference_referents
            .insert((&second).into(), prepared.referent)
            .unwrap();
        execution.structural_values.remove(&result.place);
        execution.reference_referents.remove(&(&first).into());
        execution.collect_retired_descriptor_storage(None);
        assert!(
            weak.upgrade().is_some(),
            "reborrow must retain the live backing"
        );
        let path = if array {
            vec![StructuralPathSegment::FixedIndex(0)]
        } else {
            vec![]
        };
        let mut reborrow_result = result.clone();
        reborrow_result.place = second_place;
        assert_eq!(
            read(&execution, &reborrow_result, &path, scalar()).unwrap(),
            word(scalar(), 37)
        );
        execution.structural_values.remove(&second_place);
        execution.reference_referents.remove(&(&second).into());
        execution.collect_retired_descriptor_storage(None);
        assert!(
            weak.upgrade().is_none(),
            "the last dead referent must release its resident cells"
        );
    }
}

fn nested_reference_fixture() -> (
    TerminalExecution<'static>,
    BoundaryMachineDeclaration,
    StructuralOperationResult,
    TerminalReferenceResult,
) {
    let field = StructuralFieldId::new(1).unwrap();
    let (execution, boundary, result) = setup(
        vec![
            ty(1, StructuralTypeShape::PrimitiveScalar(scalar())),
            ty(
                2,
                StructuralTypeShape::Reference {
                    referent: StructuralTypeId::new(1).unwrap(),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            ty(
                3,
                StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        declaration_identity: Default::default(),
                        inline_carrier: None,
                        id: field,
                        identity: TerminalIdentityIndex(1),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(
                            StructuralTypeId::new(2).unwrap(),
                        ),
                        storage: None,
                    }],
                    storage: StructuralRecordStorage::Packed,
                },
            ),
        ],
        3,
    );
    let supplied = TerminalReferenceResult {
        referent: TerminalReferencePayload::Zeroed,
        references: vec![(
            vec![Path::Field(field)],
            resident(
                1,
                vec![(
                    vec![],
                    word(scalar(), 37),
                    scalar(),
                    backing(word(scalar(), 37)),
                )],
            ),
        )],
    };
    (execution, boundary, result, supplied)
}

#[test]
fn nested_resident_backing_cannot_bypass_the_outer_completion_contract() {
    let (mut execution, boundary, result, supplied) = nested_reference_fixture();
    let mut materialized = supplied.clone();
    let TerminalReferencePayload::Resident { aggregate, .. } =
        &materialized.references[0].1.referent
    else {
        unreachable!()
    };
    materialized.references[0].1.referent = TerminalReferencePayload::Aggregate(aggregate.clone());
    // The root reference's existing materialized route itself is supported.
    execution
        .preflight_boundary_result(&boundary, &OperationResult::Structural(result.clone()))
        .unwrap();
    let accepted = commit(&mut execution, &boundary, &result, supplied);
    if accepted.is_ok() {
        assert!(execution.structural_values.contains_key(&result.place));
        assert_eq!(execution.structural_primitive_storage.len(), 1);
        assert!(!execution.reference_referents.is_empty());
    }
    assert!(
        accepted.is_err(),
        "nested Resident was accepted and published under the outer materialized contract"
    );
    assert!(!execution.structural_values.contains_key(&result.place));
    assert_eq!(execution.structural_primitive_storage.len(), 0);
    assert!(execution.reference_referents.is_empty());
    commit(&mut execution, &boundary, &result, materialized).unwrap();
}

struct ReferenceHandler {
    returned: TerminalReferenceResult,
    calls: usize,
}
impl crate::TerminalEffectHandler for ReferenceHandler {
    fn handle_effect(&mut self, _: &crate::TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("unexpected untyped effect"))
    }
    fn handle_effect_result(
        &mut self,
        effect: &crate::TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        assert!(matches!(effect, crate::TerminalEffect::BoundaryCall { .. }));
        self.calls += 1;
        Ok(TerminalEffectResult::Reference(self.returned.clone()))
    }
}
fn dispatch(
    execution: &mut TerminalExecution<'_>,
    handler: &mut impl crate::TerminalEffectHandler,
) -> Result<crate::engine::execution::OperationFlow, crate::TerminalInterpretError> {
    let operation =
        execution.program.machine(execution.current_machine)?.blocks[0].operations[0].clone();
    execution.execute_boundary_call(
        &operation,
        handler,
        &mut terminal_work::WorkMeter::with_budget(1000),
        &mut crate::embedding::instance::ComponentBindings::new(),
    )
}

#[test]
fn ordinary_dispatch_keeps_canonical_provider_reference_live() {
    let (mut execution, _, result) = fixture();
    let home = backing(word(scalar(), 37));
    let mut handler = ReferenceHandler {
        returned: resident(
            1,
            vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
        ),
        calls: 0,
    };
    let outcome = dispatch(&mut execution, &mut handler);
    assert_eq!(
        handler.calls, 1,
        "the ordinary host handler must actually supply its result"
    );
    assert!(matches!(
        outcome.unwrap(),
        crate::engine::execution::OperationFlow::Advance
    ));
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 37)
    );
    *home.value.borrow_mut() = word(scalar(), 91);
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 91)
    );
    assert_eq!(execution.effects().len(), 1);
}

fn edit_boundary(
    execution: &mut TerminalExecution<'_>,
    update: impl FnOnce(&mut BoundaryMachineDeclaration),
) {
    let module = Arc::get_mut(&mut execution.program)
        .unwrap()
        .fixture_module_mut();
    update(&mut module.boundary_machines[0]);
}
fn legacy_invocation(execution: &mut TerminalExecution<'_>, referent: u64) {
    edit_boundary(execution, |boundary| {
        boundary.result_lifetimes = None;
        boundary.result_reference_regions = vec![TerminalDynamicReferenceRegion {
            path: vec![],
            source_path: vec![],
            access: StructuralAccess::SharedBorrow,
            origin: TerminalDynamicReferenceOrigin::Invocation {
                carrier_type: StructuralTypeId::new(referent).unwrap(),
            },
        }];
    });
}

#[test]
fn ordinary_dispatch_refuses_missing_and_forged_provider_frontiers_before_the_handler() {
    for invalid in 0..5 {
        let (mut execution, _, result) = fixture();
        edit_boundary(&mut execution, |boundary| match invalid {
            0 => boundary.result_lifetimes = None,
            1 => boundary.result_lifetimes = Some(vec![]),
            2 => {
                boundary.result_lifetimes.as_mut().unwrap()[0].results[0] =
                    vec![StructuralPathSegment::FixedIndex(0)]
            }
            3 => boundary.result_lifetimes.as_mut().unwrap()[0]
                .results
                .push(vec![]),
            4 => {
                let BoundaryMachineResult::Structural(declared) = &mut boundary.result else {
                    unreachable!()
                };
                declared.structural_type = StructuralTypeId::new(1).unwrap();
            }
            _ => unreachable!(),
        });
        let mut handler = ReferenceHandler {
            returned: resident(
                1,
                vec![(
                    vec![],
                    word(scalar(), 37),
                    scalar(),
                    backing(word(scalar(), 37)),
                )],
            ),
            calls: 0,
        };
        assert!(
            dispatch(&mut execution, &mut handler).is_err(),
            "invalid {invalid}"
        );
        assert_eq!(
            handler.calls, 0,
            "invalid {invalid} must refuse before invoking the provider"
        );
        assert!(!execution.structural_values.contains_key(&result.place));
        assert!(execution.reference_referents.is_empty());
    }
}

#[test]
fn ordinary_dispatch_preserves_materialized_reference_contracts() {
    for zeroed in [false, true] {
        for legacy in [false, true] {
            let (mut execution, _, result) = fixture();
            if legacy {
                legacy_invocation(&mut execution, 1);
            }
            let TerminalReferencePayload::Resident { aggregate, .. } = resident(
                1,
                vec![(
                    vec![],
                    word(scalar(), 37),
                    scalar(),
                    backing(word(scalar(), 37)),
                )],
            )
            .referent
            else {
                unreachable!()
            };
            let mut handler = ReferenceHandler {
                returned: TerminalReferenceResult {
                    referent: if zeroed {
                        TerminalReferencePayload::Zeroed
                    } else {
                        TerminalReferencePayload::Aggregate(aggregate)
                    },
                    references: vec![],
                },
                calls: 0,
            };
            let outcome = dispatch(&mut execution, &mut handler);
            assert_eq!(handler.calls, 1);
            if legacy {
                assert!(matches!(
                    outcome.unwrap(),
                    crate::engine::execution::OperationFlow::Advance
                ));
                assert_eq!(
                    read(&execution, &result, &[], scalar()).unwrap(),
                    word(scalar(), if zeroed { 0 } else { 37 })
                );
            } else {
                assert!(
                    outcome.is_err(),
                    "a canonical lifetime alone must not authorize materialized reference contents"
                );
                assert!(!execution.structural_values.contains_key(&result.place));
                assert!(execution.reference_referents.is_empty());
            }
        }
    }
}

#[test]
fn ordinary_dispatch_refuses_nested_resident_under_a_legacy_materialized_reference() {
    let (mut execution, _, result, supplied) = nested_reference_fixture();
    legacy_invocation(&mut execution, 3);
    let mut handler = ReferenceHandler {
        returned: supplied,
        calls: 0,
    };
    assert!(dispatch(&mut execution, &mut handler).is_err());
    assert_eq!(handler.calls, 1);
    assert!(!execution.structural_values.contains_key(&result.place));
    assert_eq!(execution.structural_primitive_storage.len(), 0);
    assert!(execution.reference_referents.is_empty());
}

struct WaitingReferenceHandler {
    inner: ReferenceHandler,
    waiting: bool,
}
impl crate::TerminalEffectHandler for WaitingReferenceHandler {
    fn handle_effect(
        &mut self,
        effect: &crate::TerminalEffect,
    ) -> Result<(), TerminalEffectRejection> {
        self.inner.handle_effect(effect)
    }
    fn handle_effect_result(
        &mut self,
        effect: &crate::TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.inner.handle_effect_result(effect)
    }
    fn poll_host_computation(
        &mut self,
        _: &crate::TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(!std::mem::replace(&mut self.waiting, false))
    }
}
#[test]
fn ordinary_dispatch_resumes_the_same_live_provider_reference_completion() {
    let (mut execution, _, result) = fixture();
    let home = backing(word(scalar(), 37));
    let mut handler = WaitingReferenceHandler {
        inner: ReferenceHandler {
            returned: resident(
                1,
                vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
            ),
            calls: 0,
        },
        waiting: true,
    };
    assert!(matches!(
        dispatch(&mut execution, &mut handler).unwrap(),
        crate::engine::execution::OperationFlow::Yield(
            crate::TerminalExecutionStatus::HostComputationPending(_)
        )
    ));
    assert_eq!(handler.inner.calls, 0);
    assert!(!execution.structural_values.contains_key(&result.place));
    assert!(matches!(
        dispatch(&mut execution, &mut handler).unwrap(),
        crate::engine::execution::OperationFlow::Advance
    ));
    assert_eq!(handler.inner.calls, 1);
    *home.value.borrow_mut() = word(scalar(), 91);
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 91)
    );
    assert_eq!(execution.effects().len(), 1);
}
