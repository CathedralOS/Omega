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
    module.interfaces.boundary_machines.push(boundary.clone());
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
    update(&mut module.interfaces.boundary_machines[0]);
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

struct OriginalStorageHandoff {
    owner: crate::TerminalStructuralValue,
    backing: Rc<dyn TerminalScalarBacking>,
    value: TerminalScalarValue,
    path: Vec<Path>,
    allocation_contents: usize,
    calls: usize,
}
impl crate::TerminalEffectHandler for OriginalStorageHandoff {
    fn storage_requirements(
        &self,
        _: semantic_vocabulary::BoundaryMachineId,
        _: &[TerminalScalarValue],
    ) -> crate::TerminalCallStorageRequirements<'_> {
        crate::TerminalCallStorageRequirements {
            arguments: &[0],
            original: true,
        }
    }
    fn prepare_original_storage(
        &mut self,
        _: semantic_vocabulary::BoundaryMachineId,
        requests: &[crate::TerminalOriginalStorageRequest],
    ) -> Result<crate::TerminalOriginalScalarBackings, TerminalEffectRejection> {
        self.calls += 1;
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.structural_type(), self.owner.structural_type);
        assert_eq!(request.view_type(), self.owner.structural_type);
        assert_eq!(request.root().allocation(), self.owner.opaque_identity);
        assert_eq!(request.access(), StructuralAccess::SharedBorrow);
        assert_eq!(request.referent(), self.path.as_slice());
        assert_eq!(
            request.allocation_contents().len(),
            self.allocation_contents
        );
        assert_eq!(request.values().len(), 1);
        let value = &request.values()[0];
        assert_eq!(value.path(), self.path.as_slice());
        assert_eq!(value.value(), self.value);
        assert!(Rc::ptr_eq(value.backing().unwrap(), &self.backing));
        Err(TerminalEffectRejection::new(
            "original-storage handoff has no native pointer authority",
        ))
    }
    fn handle_effect(&mut self, _: &crate::TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("a borrowed sink cannot run before original backing is admitted")
    }
}

fn original_storage_sink(
    execution: &mut TerminalExecution<'_>,
    source: PlaceId,
    structural_type: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
) -> Operation {
    let mut boundary = idle_boundary();
    boundary.id = semantic_vocabulary::BoundaryMachineId::new(2).unwrap();
    boundary.parameter_order = vec![BoundaryParameterKind::Structural];
    boundary.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(10).unwrap(),
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
        additional_input_qualifications: vec![],
    }];
    let operation = Operation::unsuspended(
        semantic_vocabulary::OperationId::new(2).unwrap(),
        OperationResult::Unit,
        OperationKind::BoundaryCall {
            boundary: boundary.id,
            arguments: vec![],
            structural_arguments: vec![StructuralArgument {
                place: source,
                path,
                access: StructuralAccess::SharedBorrow,
            }],
            completion_receipts: vec![],
            requirement_obligations: vec![],
        },
    );
    let module = Arc::get_mut(&mut execution.program)
        .unwrap()
        .fixture_module_mut();
    module.interfaces.boundary_machines.push(boundary);
    module.machines[0].blocks[0].operations[1] = operation.clone();
    module.machines[0]
        .structural_places
        .retain(|place| place.id != PlaceId::new(2).unwrap());
    operation
}

#[test]
fn resident_reference_reaches_original_storage_handoff_without_owned_authority() {
    let (mut execution, _, result) = fixture();
    let home = backing(word(scalar(), 37));
    let mut producer = ReferenceHandler {
        returned: resident(
            1,
            vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
        ),
        calls: 0,
    };
    assert!(matches!(
        dispatch(&mut execution, &mut producer).unwrap(),
        crate::engine::execution::OperationFlow::Advance
    ));
    let carrier = execution.structural_values.get(&result.place).unwrap();
    let owner = execution
        .reference_referents
        .get(&carrier.into())
        .unwrap()
        .view
        .clone();
    assert!(matches!(
        execution.original_owned_allocation(&owner),
        Err(crate::TerminalInterpretError::UnsupportedSemanticVariant(
            "storage source has no live supplying allocation"
        ))
    ));
    let crate::storage::scalar_storage::ScalarCell::Resident { backing, .. } = execution
        .structural_primitive_storage
        .get(&(&owner).into())
        .unwrap()
    else {
        panic!("the provider result must remain resident")
    };
    let mut handler = OriginalStorageHandoff {
        owner,
        backing: backing.clone(),
        value: word(scalar(), 91),
        path: vec![],
        allocation_contents: 0,
        calls: 0,
    };
    *home.value.borrow_mut() = word(scalar(), 91);
    let operation = original_storage_sink(
        &mut execution,
        result.place,
        StructuralTypeId::new(1).unwrap(),
        vec![StructuralPathSegment::Referent],
    );
    let outcome = execution.execute_boundary_call(
        &operation,
        &mut handler,
        &mut terminal_work::WorkMeter::with_budget(1000),
        &mut crate::embedding::instance::ComponentBindings::new(),
    );
    assert_eq!(
        handler.calls,
        1,
        "the typed original-storage handoff was not reached: {:?}",
        outcome.as_ref().err()
    );
    assert!(matches!(
        outcome,
        Err(crate::TerminalInterpretError::StructuralArgumentPreparation { .. })
            | Err(crate::TerminalInterpretError::ResidentStorage(_))
    ));
    assert!(matches!(
        execution.original_owned_allocation(&handler.owner),
        Err(crate::TerminalInterpretError::UnsupportedSemanticVariant(
            "storage source has no live supplying allocation"
        ))
    ));
    assert_eq!(
        read(&execution, &result, &[], scalar()).unwrap(),
        word(scalar(), 91)
    );
}

fn published_scalar_reference() -> (
    TerminalExecution<'static>,
    StructuralOperationResult,
    Rc<Backing>,
    ReferenceHandler,
) {
    let (mut execution, _, result) = fixture();
    let home = backing(word(scalar(), 37));
    let mut producer = ReferenceHandler {
        returned: resident(
            1,
            vec![(vec![], word(scalar(), 37), scalar(), home.clone())],
        ),
        calls: 0,
    };
    dispatch(&mut execution, &mut producer).unwrap();
    (execution, result, home, producer)
}
fn reference_owner(
    execution: &TerminalExecution<'_>,
    place: PlaceId,
) -> crate::TerminalStructuralValue {
    let carrier = execution.structural_values.get(&place).unwrap();
    execution
        .reference_referents
        .get(&carrier.into())
        .unwrap()
        .view
        .clone()
}

#[test]
fn live_reference_owner_retains_record_and_packed_array_geometry() {
    for array in [false, true] {
        let field = StructuralFieldId::new(1).unwrap();
        let shape = if array {
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: 2.into(),
            }
        } else {
            StructuralTypeShape::Record {
                fields: (1..=2)
                    .map(|index| StructuralFieldDeclaration {
                        declaration_identity: Default::default(),
                        inline_carrier: Some(StructuralTypeId::new(1).unwrap()),
                        id: StructuralFieldId::new(index).unwrap(),
                        identity: TerminalIdentityIndex(index as u32),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar()),
                        storage: None,
                    })
                    .collect(),
                storage: StructuralRecordStorage::Packed,
            }
        };
        let (mut execution, _, result) = setup(
            vec![
                ty(1, StructuralTypeShape::PrimitiveScalar(scalar())),
                ty(2, shape),
            ],
            2,
        );
        let paths = if array {
            vec![vec![Path::FixedIndex(0)], vec![Path::FixedIndex(1)]]
        } else {
            vec![
                vec![Path::Field(field)],
                vec![Path::Field(StructuralFieldId::new(2).unwrap())],
            ]
        };
        let homes = [backing(word(scalar(), 37)), backing(word(scalar(), 63))];
        let mut producer = ReferenceHandler {
            returned: resident(
                2,
                vec![
                    (
                        paths[0].clone(),
                        word(scalar(), 37),
                        scalar(),
                        homes[0].clone(),
                    ),
                    (
                        paths[1].clone(),
                        word(scalar(), 63),
                        scalar(),
                        homes[1].clone(),
                    ),
                ],
            ),
            calls: 0,
        };
        dispatch(&mut execution, &mut producer).unwrap();
        let owner = reference_owner(&execution, result.place);
        let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
        assert_eq!(
            execution
                .live_reference_storage_owner(&root)
                .unwrap()
                .unwrap(),
            &owner
        );
        let endpoint_backing = if array {
            execution
                .structural_byte_storage
                .get(&root)
                .unwrap()
                .capture_scalar(0)
                .unwrap()
                .1
                .unwrap()
        } else {
            let cell =
                crate::boundary::values::StructuralScalarRuntimeField::member(root.clone(), field);
            let crate::storage::scalar_storage::ScalarCell::Resident { backing, .. } =
                execution.structural_scalar_fields.get(&cell).unwrap()
            else {
                panic!("resident field")
            };
            backing.clone()
        };
        let mut handler = OriginalStorageHandoff {
            owner,
            backing: endpoint_backing,
            value: word(scalar(), 37),
            path: paths[0].clone(),
            allocation_contents: 1,
            calls: 0,
        };
        let mut path = vec![StructuralPathSegment::Referent];
        path.push(if array {
            StructuralPathSegment::FixedIndex(0)
        } else {
            StructuralPathSegment::Field(TerminalIdentityIndex(1))
        });
        let operation = original_storage_sink(
            &mut execution,
            result.place,
            StructuralTypeId::new(1).unwrap(),
            path,
        );
        let outcome = execution.execute_boundary_call(
            &operation,
            &mut handler,
            &mut terminal_work::WorkMeter::with_budget(1000),
            &mut crate::embedding::instance::ComponentBindings::new(),
        );
        assert_eq!(
            handler.calls,
            1,
            "array={array}, outcome={:?}",
            outcome.as_ref().err()
        );
        assert!(
            format!("{:?}", outcome.err())
                .contains("original-storage handoff has no native pointer authority")
        );
    }
}

#[test]
fn live_reference_owner_ignores_unreachable_cache_and_checks_exact_identity_and_path() {
    let (mut execution, result, _home, _producer) = published_scalar_reference();
    let owner = reference_owner(&execution, result.place);
    let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
    let carrier = execution.structural_values.get(&result.place).unwrap();
    let mut cached = execution
        .reference_referents
        .get(&carrier.into())
        .unwrap()
        .clone();
    cached.view.opaque_identity = u64::MAX;
    let stale = crate::boundary::values::StructuralRuntimePlace {
        opaque_identity: u64::MAX - 1,
        path: vec![],
    };
    execution.reference_referents.insert(stale, cached).unwrap();
    assert_eq!(
        execution
            .live_reference_storage_owner(&root)
            .unwrap()
            .unwrap(),
        &owner
    );
    let unknown = crate::boundary::values::StructuralRuntimePlace {
        opaque_identity: u64::MAX,
        path: vec![],
    };
    assert!(
        execution
            .live_reference_storage_owner(&unknown)
            .unwrap()
            .is_none()
    );
    assert!(execution.actual_storage_type_from_owner(&unknown).is_err());
    let mut bad_path = root;
    bad_path.path.push(StructuralPathSegment::FixedIndex(1));
    assert!(execution.live_reference_storage_owner(&bad_path).is_err());
}

#[test]
fn live_reference_owner_rejects_wrong_types_and_partial_or_reinterpreted_geometry() {
    for invalid in 0..4 {
        let (mut execution, result, _home, _producer) = published_scalar_reference();
        let owner = reference_owner(&execution, result.place);
        let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
        let carrier = execution
            .structural_values
            .get(&result.place)
            .unwrap()
            .clone();
        if invalid == 0 {
            let module = Arc::get_mut(&mut execution.program)
                .unwrap()
                .fixture_module_mut();
            module.types.structural_types.push(ty(
                3,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ));
            execution
                .reference_referents
                .get_mut(&(&carrier).into())
                .unwrap()
                .view
                .structural_type = StructuralTypeId::new(3).unwrap();
        } else if invalid == 1 {
            let reference = execution
                .reference_referents
                .get_mut(&(&carrier).into())
                .unwrap();
            reference
                .view
                .path
                .push(StructuralPathSegment::FixedIndex(0));
        } else if invalid == 2 {
            let mut cursor = execution.local_structural_identities.cursor();
            let mut prepared = execution
                .capture_reference(
                    &StructuralArgument {
                        place: result.place,
                        path: vec![StructuralPathSegment::Referent],
                        access: StructuralAccess::SharedBorrow,
                    },
                    result.structural_type,
                    &[],
                    None,
                    &mut cursor,
                )
                .unwrap();
            execution.local_structural_identities.commit_cursor(cursor);
            prepared
                .referent
                .primitive_loan
                .as_mut()
                .unwrap()
                .storage
                .encoding = Some(
                crate::storage::primitive_storage::PrimitiveStorageEncoding {
                    source: scalar(),
                    view: ScalarType::Boolean,
                },
            );
            execution
                .reference_referents
                .insert((&carrier).into(), prepared.referent)
                .unwrap();
        } else {
            let module = Arc::get_mut(&mut execution.program)
                .unwrap()
                .fixture_module_mut();
            module.types.structural_types.push(ty(
                3,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ));
            module.types.structural_types.push(ty(
                4,
                StructuralTypeShape::Reference {
                    referent: StructuralTypeId::new(3).unwrap(),
                    access: StructuralAccess::SharedBorrow,
                },
            ));
            let mut alias = carrier.clone();
            alias.opaque_identity = execution.local_structural_identities.allocate().unwrap();
            alias.structural_type = StructuralTypeId::new(4).unwrap();
            let mut binding = execution
                .reference_referents
                .get(&(&carrier).into())
                .unwrap()
                .clone();
            binding.view.structural_type = StructuralTypeId::new(3).unwrap();
            execution
                .reference_referents
                .insert((&alias).into(), binding)
                .unwrap();
            execution
                .structural_values
                .insert(PlaceId::new(3).unwrap(), alias)
                .unwrap();
        }
        if invalid == 1 {
            assert!(
                execution
                    .live_reference_storage_owner(&root)
                    .unwrap()
                    .is_none()
            );
        } else {
            assert!(execution.live_reference_storage_owner(&root).is_err());
        }
        assert!(execution.original_owned_allocation(&owner).is_err());
    }
}

#[test]
fn live_reference_owner_survives_reborrow_and_retires_after_the_last_alias() {
    let (mut execution, result, home, producer) = published_scalar_reference();
    let owner = reference_owner(&execution, result.place);
    let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
    let weak = Rc::downgrade(&home);
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
                path: vec![StructuralPathSegment::Referent],
                access: StructuralAccess::SharedBorrow,
            },
            result.structural_type,
            &[],
            None,
            &mut cursor,
        )
        .unwrap();
    execution.local_structural_identities.commit_cursor(cursor);
    let mut alias = first.clone();
    alias.opaque_identity = execution.local_structural_identities.allocate().unwrap();
    let place = PlaceId::new(2).unwrap();
    execution
        .structural_values
        .insert(place, alias.clone())
        .unwrap();
    execution
        .reference_referents
        .insert((&alias).into(), prepared.referent)
        .unwrap();
    assert_eq!(
        execution
            .live_reference_storage_owner(&root)
            .unwrap()
            .unwrap(),
        &owner
    );
    execution.structural_values.remove(&result.place);
    execution.reference_referents.remove(&(&first).into());
    execution.collect_retired_descriptor_storage(None);
    assert_eq!(
        execution
            .live_reference_storage_owner(&root)
            .unwrap()
            .unwrap(),
        &owner
    );
    assert_eq!(
        execution.actual_storage_type_from_owner(&root).unwrap(),
        owner.structural_type
    );
    assert!(execution.original_owned_allocation(&owner).is_err());
    drop(home);
    drop(producer);
    assert!(weak.upgrade().is_some());
    execution.structural_values.remove(&place);
    // Leave an unreachable reference cache row: it must not keep typed authority.
    assert!(
        execution
            .live_reference_storage_owner(&root)
            .unwrap()
            .is_none()
    );
    assert!(execution.actual_storage_type_from_owner(&root).is_err());
    execution.collect_retired_descriptor_storage(None);
    assert!(weak.upgrade().is_none());
}

#[test]
fn live_reference_owner_preserves_guest_allocation_authority_and_competing_errors() {
    let (mut execution, result, _home, _producer) = published_scalar_reference();
    let carrier = execution
        .structural_values
        .get(&result.place)
        .unwrap()
        .clone();
    let owner = reference_owner(&execution, result.place);
    let allocation = execution.original_owned_allocation(&carrier).unwrap();
    assert_eq!(allocation.place, result.place);
    assert_eq!(allocation.value, carrier);
    assert!(execution.original_owned_allocation(&owner).is_err());
    // Existing guest-owned ambiguity must win even though a live reference backing exists.
    let module = Arc::get_mut(&mut execution.program)
        .unwrap()
        .fixture_module_mut();
    for id in 3..=4 {
        let place = PlaceId::new(id).unwrap();
        let operation = semantic_vocabulary::OperationId::new(id).unwrap();
        let mut declared = result.clone();
        declared.place = place;
        declared.structural_type = owner.structural_type;
        module.machines[0]
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: owner.structural_type,
                },
            });
        module.machines[0].blocks[0]
            .operations
            .push(Operation::unsuspended(
                operation,
                OperationResult::Structural(declared),
                OperationKind::EstablishZeroedStructural {},
            ));
    }
    execution
        .structural_values
        .insert(PlaceId::new(3).unwrap(), owner.clone())
        .unwrap();
    execution
        .structural_values
        .insert(PlaceId::new(4).unwrap(), owner.clone())
        .unwrap();
    assert!(matches!(
        execution.original_owned_allocation(&owner),
        Err(crate::TerminalInterpretError::VerifiedOperationMalformed { .. })
    ));
    let mut handler = OriginalStorageHandoff {
        owner: owner.clone(),
        backing: {
            let crate::storage::scalar_storage::ScalarCell::Resident { backing, .. } = execution
                .structural_primitive_storage
                .get(&(&owner).into())
                .unwrap()
            else {
                unreachable!()
            };
            backing.clone()
        },
        value: word(scalar(), 37),
        path: vec![],
        allocation_contents: 0,
        calls: 0,
    };
    let operation = original_storage_sink(
        &mut execution,
        result.place,
        owner.structural_type,
        vec![StructuralPathSegment::Referent],
    );
    assert!(
        execution
            .execute_boundary_call(
                &operation,
                &mut handler,
                &mut terminal_work::WorkMeter::with_budget(1000),
                &mut crate::embedding::instance::ComponentBindings::new()
            )
            .is_err()
    );
    assert_eq!(handler.calls, 0);
}

#[test]
fn live_reference_owner_keeps_stale_resident_reads_from_reaching_the_storage_handler() {
    let (mut execution, result, home, _producer) = published_scalar_reference();
    let owner = reference_owner(&execution, result.place);
    let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
    let crate::storage::scalar_storage::ScalarCell::Resident { backing, .. } =
        execution.structural_primitive_storage.get(&root).unwrap()
    else {
        unreachable!()
    };
    let mut handler = OriginalStorageHandoff {
        owner,
        backing: backing.clone(),
        value: word(scalar(), 37),
        path: vec![],
        allocation_contents: 0,
        calls: 0,
    };
    home.stale.set(true);
    let operation = original_storage_sink(
        &mut execution,
        result.place,
        StructuralTypeId::new(1).unwrap(),
        vec![StructuralPathSegment::Referent],
    );
    assert!(
        execution
            .execute_boundary_call(
                &operation,
                &mut handler,
                &mut terminal_work::WorkMeter::with_budget(1000),
                &mut crate::embedding::instance::ComponentBindings::new()
            )
            .is_err()
    );
    assert_eq!(handler.calls, 0);
}

#[test]
fn live_reference_owner_ignores_inactive_cached_descendants_of_a_live_sum() {
    let (mut execution, result, _home, _producer) = published_scalar_reference();
    let owner = reference_owner(&execution, result.place);
    let root = crate::boundary::values::StructuralRuntimePlace::from(&owner);
    let active = semantic_vocabulary::StructuralCaseId::new(1).unwrap();
    let inactive = semantic_vocabulary::StructuralCaseId::new(2).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .fixture_module_mut()
        .structural_types
        .push(ty(
            3,
            StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: active,
                        identity: TerminalIdentityIndex(1),
                        fields: vec![],
                    },
                    StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: inactive,
                        identity: TerminalIdentityIndex(2),
                        fields: vec![StructuralFieldDeclaration {
                            declaration_identity: Default::default(),
                            inline_carrier: None,
                            id: field,
                            identity: TerminalIdentityIndex(1),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(result.structural_type),
                            storage: None,
                        }],
                    },
                ],
            },
        ));
    let sum = crate::TerminalStructuralValue::minted(
        execution.local_structural_identities.allocate().unwrap(),
        StructuralTypeId::new(3).unwrap(),
        vec![],
    );
    let sum_root = crate::boundary::values::StructuralRuntimePlace::from(&sum);
    execution
        .structural_cases
        .insert(
            sum_root.clone(),
            crate::boundary::structural_inputs::case_membership::StructuralCaseContents {
                structural_type: sum.structural_type,
                case: active,
            },
        )
        .unwrap();
    execution
        .structural_values
        .insert(PlaceId::new(3).unwrap(), sum)
        .unwrap();
    let carrier = execution.structural_values.get(&result.place).unwrap();
    let mut cache = execution
        .reference_referents
        .get(&carrier.into())
        .unwrap()
        .clone();
    cache.view.opaque_identity = u64::MAX;
    let mut cached_home = sum_root;
    cached_home.path = vec![
        StructuralPathSegment::Case(inactive),
        StructuralPathSegment::Field(TerminalIdentityIndex(1)),
    ];
    execution
        .reference_referents
        .insert(cached_home, cache)
        .unwrap();
    assert_eq!(
        execution
            .live_reference_storage_owner(&root)
            .unwrap()
            .unwrap(),
        &owner
    );
    let phantom = crate::boundary::values::StructuralRuntimePlace {
        opaque_identity: u64::MAX,
        path: vec![],
    };
    assert!(
        execution
            .live_reference_storage_owner(&phantom)
            .unwrap()
            .is_none()
    );
    assert!(execution.actual_storage_type_from_owner(&phantom).is_err());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AggregateReferenceControl {
    Aligned,
    Short,
    Misaligned,
    MissingTarget,
    ReferencePermission,
    PlainOverflow,
    DependentOverflow,
}

fn check_aggregate_reference_formation(control: AggregateReferenceControl) {
    use crate::boundary::structural_inputs::contents::TerminalStructuralContents;
    use crate::boundary::values::StructuralRuntimePlace;
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, OperationId, TargetCarrierLayout, ValueId,
    };

    let short = control == AggregateReferenceControl::Short;
    let misaligned = control == AggregateReferenceControl::Misaligned;
    let plain = control == AggregateReferenceControl::PlainOverflow;
    let selected = control != AggregateReferenceControl::MissingTarget;
    let overflow = matches!(
        control,
        AggregateReferenceControl::PlainOverflow | AggregateReferenceControl::DependentOverflow
    );
    let accepted = matches!(
        control,
        AggregateReferenceControl::Aligned | AggregateReferenceControl::PlainOverflow
    );
    let byte_length = if short { 31 } else { 32 };
    let byte_field_id = if misaligned { 4 } else { 3 };
    let type_id = |id| StructuralTypeId::new(id).unwrap();
    let unsigned =
        |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    let field = |id, identity, field_type| StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(id).unwrap(),
        identity: TerminalIdentityIndex(identity),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    };
    let word_field = |id, identity, offset| StructuralFieldDeclaration {
        inline_carrier: Some(type_id(5)),
        storage: Some(AuthoredFieldStorage::Direct {
            byte_offset: offset,
        }),
        ..field(id, identity, StructuralFieldType::Scalar(unsigned(64)))
    };
    // The captured source recipe contains a sibling sum before the byte field.
    // Its field walk needs admitted carrier facts even with no layout applications.
    let mut types = vec![
        ty(
            1,
            StructuralTypeShape::Sum {
                cases: (1..=2)
                    .map(|id| StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: semantic_vocabulary::StructuralCaseId::new(u64::from(id)).unwrap(),
                        identity: TerminalIdentityIndex(id),
                        fields: Vec::new(),
                    })
                    .collect(),
            },
        ),
        ty(2, StructuralTypeShape::PrimitiveScalar(unsigned(8))),
        ty(
            3,
            StructuralTypeShape::FixedArray {
                element: type_id(2),
                length: ArrayExtent::Literal(byte_length),
            },
        ),
        ty(
            4,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, 6, StructuralFieldType::Structural(type_id(1))),
                    StructuralFieldDeclaration {
                        inline_carrier: Some(type_id(5)),
                        ..field(2, 7, StructuralFieldType::Scalar(unsigned(64)))
                    },
                    field(3, 8, StructuralFieldType::Structural(type_id(3))),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
        ty(5, StructuralTypeShape::PrimitiveScalar(unsigned(64))),
        ty(
            6,
            StructuralTypeShape::Record {
                fields: vec![word_field(6, 10, 0), word_field(7, 11, 8)],
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 16,
                    byte_alignment: 8,
                }),
            },
        ),
        ty(
            7,
            StructuralTypeShape::FixedArray {
                element: type_id(2),
                length: ArrayExtent::Literal(16),
            },
        ),
        ty(
            8,
            StructuralTypeShape::Record {
                fields: vec![
                    field(8, 15, StructuralFieldType::Structural(type_id(6))),
                    field(9, 16, StructuralFieldType::Structural(type_id(7))),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
        StructuralTypeDeclaration {
            multiplicity: StructuralMultiplicity::Affine,
            ..ty(
                9,
                StructuralTypeShape::Reference {
                    referent: type_id(8),
                    access: StructuralAccess::SharedBorrow,
                },
            )
        },
    ];
    let StructuralTypeShape::Record { fields, .. } = &mut types[3].shape else {
        unreachable!()
    };
    if plain {
        fields.remove(0);
    }
    if misaligned {
        fields[2].id = StructuralFieldId::new(byte_field_id).unwrap();
        fields.insert(2, field(3, 20, StructuralFieldType::Scalar(unsigned(8))));
    }
    if control == AggregateReferenceControl::ReferencePermission {
        // A real, nonrecursive reference leaf has the same selected pointer
        // footprint as the first word. Equal bytes cannot supply its permission.
        types.push(StructuralTypeDeclaration {
            multiplicity: StructuralMultiplicity::Affine,
            ..ty(
                10,
                StructuralTypeShape::Reference {
                    referent: type_id(5),
                    access: StructuralAccess::SharedBorrow,
                },
            )
        });
        let StructuralTypeShape::Record { fields, .. } = &mut types[5].shape else {
            unreachable!()
        };
        fields[0].field_type = StructuralFieldType::Structural(type_id(10));
        fields[0].inline_carrier = None;
        types[5].multiplicity = StructuralMultiplicity::Affine;
        types[7].multiplicity = StructuralMultiplicity::Affine;
    }
    let source = StructuralArgument {
        place: PlaceId::new(1).unwrap(),
        path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(8))],
        access: StructuralAccess::SharedBorrow,
    };
    let mut module: TerminalModule = (**empty_program().catalogs).clone();
    let mut identities = TerminalIdentityTableBuilder::default();
    for index in 1..=if misaligned { 20 } else { 16 } {
        assert_eq!(
            identities.intern(&format!("aggregate_view_{index}")),
            TerminalIdentityIndex(index)
        );
    }
    module.identities = identities.build();
    module.types.structural_types = types;
    let machine = semantic_vocabulary::MachineId::new(2).unwrap();
    module.entry = Some(TargetMachineCallable::Machine(machine));
    module.machines[0].id = machine;
    module.machines[0].parameter_order = vec![BoundaryParameterKind::Structural];
    module.machines[0].structural_parameters = vec![StructuralParameterDeclaration {
        place: source.place,
        position: 0,
        structural_type: type_id(4),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: source.access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: source.place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
        });
    assert!(module.layouts.target_layout_applications.is_empty());
    assert!(module.access.access_lifecycle_plans.is_empty());
    // Seal the final catalog, then use ordinary admission. No fixture identity
    // or admitted layout object is manufactured after the catalog changes.
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap();
    let program = crate::TerminalProgram::borrow(artifact.view());
    let target = psi_target_layouts::terminal_target_context(artifact.view(), "linux_x86_64");
    let facts = psi_target_layouts::LayoutTargetFacts {
        profile: "linux_x86_64".into(),
        pointer_size: if overflow { 1 << 63 } else { 8 },
        pointer_alignment: 8,
        case_tag_size: 4,
        case_tag_alignment: 4,
        little_endian: true,
    };
    let layouts =
        psi_target_layouts::admit_target_layouts(artifact.view(), &target, &facts).unwrap();
    assert!(layouts.closures().is_empty());
    assert!(layouts.resolved_lifecycles().is_empty());
    let supplies = program
        .admit_runtime_supplies(crate::boundary::effects::TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: None,
            entry: artifact.semantic_module().entry,
            installation: None,
            host_imports: &[],
            external_completions: &[],
            target_machines: selected.then_some(&target),
            target_layouts: selected.then_some(&layouts),
        })
        .unwrap();
    let owner = crate::TerminalStructuralValue::minted(1, type_id(4), Vec::new());
    let mut bytes = vec![0; usize::try_from(byte_length).unwrap()];
    bytes[0] = 42;
    let mut values = vec![
        TerminalBoundaryStorageValue {
            path: vec![Path::Field(StructuralFieldId::new(1).unwrap())],
            contents: TerminalStorageContents::Case(
                semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
            ),
        },
        TerminalBoundaryStorageValue {
            path: vec![Path::Field(StructuralFieldId::new(2).unwrap())],
            contents: TerminalStorageContents::Scalar {
                value: word(unsigned(64), 8),
                stored_scalar: unsigned(64),
            },
        },
    ];
    if plain {
        values.remove(0);
    }
    if misaligned {
        values.push(TerminalBoundaryStorageValue {
            path: vec![Path::Field(StructuralFieldId::new(3).unwrap())],
            contents: TerminalStorageContents::Scalar {
                value: word(unsigned(8), 0),
                stored_scalar: unsigned(8),
            },
        });
    }
    values.extend(
        bytes
            .iter()
            .enumerate()
            .map(|(index, byte)| TerminalBoundaryStorageValue {
                path: vec![
                    Path::Field(StructuralFieldId::new(byte_field_id).unwrap()),
                    Path::FixedIndex(index as u64),
                ],
                contents: TerminalStorageContents::Scalar {
                    value: word(unsigned(8), u128::from(*byte)),
                    stored_scalar: unsigned(8),
                },
            }),
    );
    values.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let contents = [TerminalStructuralContents {
        argument_index: 0,
        values,
    }];
    let mut execution = program
        .start_execution(
            &[],
            crate::TerminalStructuralInputs {
                arguments: std::slice::from_ref(&owner),
                contents: &contents,
                ..Default::default()
            },
            Some(&supplies),
        )
        .unwrap();
    assert_eq!(
        execution.program.target_layouts.as_deref(),
        selected.then_some(&layouts)
    );
    if selected {
        assert_eq!(layouts.target_facts(), &facts);
    }
    if plain {
        let explicit = TargetLayoutInput {
            target: "linux_x86_64".into(),
            pointer_size: facts.pointer_size,
            pointer_alignment: facts.pointer_alignment,
            case_tag_size: facts.case_tag_size,
            case_tag_alignment: facts.case_tag_alignment,
            little_endian: facts.little_endian,
        };
        assert!(
            matches!(
                crate::storage::provider_storage::value_image::geometry::ImageGeometry::new(
                    &execution,
                    Some(&explicit)
                ),
                Err(crate::TerminalInterpretError::VerifiedOperationMalformed { .. })
            ),
            "explicit lifecycle geometry keeps its immediate overflow refusal"
        );
    }
    let mut geometry =
        terminal_semantics::ReferenceStorageGeometry::new(
            artifact.semantic_module().types.structural_types.iter(),
            |application| {
                Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
            },
        )
        .with_target_carriers(facts.pointer_size.checked_mul(2).map(
            |descriptor_byte_size| TargetCarrierLayout {
                pointer_byte_size: facts.pointer_size,
                pointer_byte_alignment: facts.pointer_alignment,
                descriptor_byte_size,
                descriptor_byte_alignment: facts.pointer_alignment,
                case_tag_byte_size: facts.case_tag_size,
                case_tag_byte_alignment: facts.case_tag_alignment,
            },
        ));
    if control == AggregateReferenceControl::DependentOverflow {
        assert_eq!(
            geometry.layout(type_id(4)),
            Err(terminal_semantics::StorageGeometryError::MissingCarrierLayout)
        );
        assert!(geometry.region(type_id(4), &source.path).is_none());
    } else {
        let (root_size, field_offset) = if plain {
            (40, 8)
        } else if misaligned {
            (56, 17)
        } else {
            (48, 16)
        };
        assert_eq!(geometry.layout(type_id(4)).unwrap(), (root_size, 8));
        let region = geometry.region(type_id(4), &source.path).unwrap();
        assert_eq!(
            (region.byte_offset, region.byte_length),
            (field_offset, byte_length)
        );
    }
    // The authority control really retains the same complete footprint; its
    // refusal must come from the reference leaf, not extent or alignment.
    assert_eq!(geometry.layout(type_id(8)).unwrap(), (32, 8));
    let mut backing = StructuralRuntimePlace::from(&owner);
    backing.path = source.path.clone();
    assert_eq!(execution.structural_values.get(&source.place), Some(&owner));
    assert_eq!(
        execution
            .structural_byte_storage
            .get(&backing)
            .unwrap()
            .local_view()
            .unwrap()
            .bytes(),
        bytes
    );
    let projected = execution
        .resolve_live_storage_projection(source.place, &source.path, source.access)
        .unwrap();
    assert_eq!(StructuralRuntimePlace::from(&projected.value), backing);
    assert_eq!(projected.value.structural_type, type_id(3));
    let offset = ValueId::new(2).unwrap();
    execution
        .values
        .insert(offset, word(unsigned(64), 0))
        .unwrap();
    let storage = Some(ReferenceStorageView {
        byte_offset: offset,
        stored_scalar: None,
    });
    let result = StructuralOperationResult {
        place: PlaceId::new(2).unwrap(),
        structural_type: type_id(9),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let operation = Operation::unsuspended(
        OperationId::new(2).unwrap(),
        OperationResult::Structural(result.clone()),
        OperationKind::EstablishReference {
            source: source.clone(),
            storage,
        },
    );
    let identities_before = execution.local_structural_identities.cursor();
    let frontier_before = execution.live_affine_frontier.clone();
    let formed = execution.establish_reference(&operation, &source, storage);
    if let Err(error) = &formed {
        let crate::TerminalInterpretError::ReferenceFormation { stage, detail } = error else {
            panic!("unexpected formation failure: {error:?}");
        };
        assert_eq!(*stage, "establish reference");
        assert!(
            detail.contains("stage: \"validate aggregate storage view\"")
                && detail.contains("compatible=false"),
            "unexpected formation failure: {error:?}"
        );
        assert!(!execution.structural_values.contains_key(&result.place));
        assert!(execution.reference_referents.is_empty());
        assert!(execution.element_view_values.is_empty());
        assert_eq!(
            execution.local_structural_identities.cursor(),
            identities_before
        );
        assert_eq!(execution.live_affine_frontier, frontier_before);
    }
    if !accepted {
        assert!(
            formed.is_err(),
            "invalid aggregate reference unexpectedly formed: {control:?}"
        );
        return;
    }
    formed.expect("aligned aggregate reference formation must consume admitted target geometry");
    let carrier = execution.structural_values.get(&result.place).unwrap();
    let referent = execution
        .reference_referents
        .get(&StructuralRuntimePlace::from(carrier))
        .unwrap();
    assert_eq!(StructuralRuntimePlace::from(&referent.view), backing);
    let region = referent.aggregate_view.expect("aggregate byte window");
    assert_eq!(
        (
            region.structural_type,
            region.byte_offset,
            region.byte_length
        ),
        (type_id(8), 0, 32)
    );
    assert_eq!(referent.collection_loan.as_ref().unwrap().len(), 32);
    assert_eq!(
        execution
            .structural_byte_storage
            .get(&backing)
            .unwrap()
            .local_view()
            .unwrap()
            .bytes(),
        bytes
    );
}

#[test]
fn aggregate_reference_formation_keeps_admitted_target_geometry() {
    check_aggregate_reference_formation(AggregateReferenceControl::Aligned);
}

#[test]
fn aggregate_reference_formation_refuses_invalid_views_without_publication() {
    for control in [
        AggregateReferenceControl::Short,
        AggregateReferenceControl::Misaligned,
        AggregateReferenceControl::MissingTarget,
        AggregateReferenceControl::ReferencePermission,
        AggregateReferenceControl::DependentOverflow,
    ] {
        check_aggregate_reference_formation(control);
    }
}

#[test]
fn aggregate_reference_formation_keeps_plain_views_when_descriptor_overflows() {
    check_aggregate_reference_formation(AggregateReferenceControl::PlainOverflow);
}
