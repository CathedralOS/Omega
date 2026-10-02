//! Current-IR controls for alternative reference origins and storage regions.

use super::alternative_result_tests::{call, selected_record};
use super::*;
use crate::tests::support::id;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, OperationId, ScalarType,
    StructuralFieldId, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape,
};

struct SelectedRecordRegions {
    function: PsiOptimizationFunction,
    declarations: Vec<StructuralTypeDeclaration>,
    result: terminal_psi::StructuralOperationResult,
    root: PlaceId,
    holders: StructuralFieldId,
    unrelated: StructuralFieldId,
    value: StructuralFieldId,
    scalar: ScalarType,
}

fn selected_record_regions() -> SelectedRecordRegions {
    let (mut function, mut declarations, result) = selected_record();
    let root = function.structural_parameters[0].place;
    let array = function.structural_parameters[0].structural_type;
    let carrier = result.structural_type;
    let StructuralTypeShape::Reference { referent, .. } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)
        .expect("reference carrier")
        .shape
    else {
        unreachable!()
    };
    let referent = *referent;
    let value = id(9_220, StructuralFieldId::new);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let StructuralTypeShape::Record { fields } = &mut declarations
        .iter_mut()
        .find(|declaration| declaration.id == referent)
        .expect("holder declaration")
        .shape
    else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: value,
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(scalar),
    });
    let container = id(9_221, StructuralTypeId::new);
    let holders = id(9_222, StructuralFieldId::new);
    let unrelated = id(9_223, StructuralFieldId::new);
    declarations.push(StructuralTypeDeclaration {
        id: container,
        identity: "SelectedHolders".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: holders,
                    identity: "holders".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(array),
                },
                StructuralFieldDeclaration {
                    id: unrelated,
                    identity: "unrelated".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(referent),
                },
            ],
        },
    });
    function.structural_parameters[0].structural_type = container;
    let abstract_operations::AbstractFunctionResult::Structural(result_declaration) =
        &mut function.result
    else {
        unreachable!()
    };
    for source in &mut result_declaration.reference_sources[0].sources {
        source
            .path
            .insert(0, StructuralPathSegment::Field("holders".into()));
    }
    function
        .structural_places
        .retain(|declaration| declaration.id != result.place);
    function.structural_places.push(StructuralPlaceDeclaration {
        id: result.place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id(9_205, OperationId::new),
            structural_type: result.structural_type,
        },
    });
    SelectedRecordRegions {
        function,
        declarations,
        result,
        root,
        holders,
        unrelated,
        value,
        scalar,
    }
}

fn live_alternative(regions: &SelectedRecordRegions) -> Vec<LiveReference> {
    let types = regions
        .declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let functions = BTreeMap::from([(regions.function.machine, &regions.function)]);
    let mut live = Vec::new();
    apply_operation(
        &regions.function,
        &functions,
        &types,
        regions.function.entry,
        0,
        &call(&regions.function, &regions.result),
        &mut live,
    )
    .unwrap();
    live
}

fn scalar_read(
    regions: &SelectedRecordRegions,
    operation: u64,
    path: Vec<CanonicalStructuralPathSegment>,
) -> O {
    O::StructuralScalarFieldRead {
        psi_operation: id(operation, OperationId::new),
        result: abstract_operations::AbstractResult {
            value: id(operation, ValueId::new),
            scalar_type: regions.scalar,
        },
        source: regions.root,
        path,
        field: regions.value,
    }
}

fn scalar_store(
    regions: &SelectedRecordRegions,
    operation: u64,
    path: Vec<StructuralPathSegment>,
) -> O {
    O::StructuralScalarFieldStore {
        psi_operation: id(operation, OperationId::new),
        destination: abstract_operations::StructuralScalarStoreDestination::Parameter(
            regions.function.structural_parameters[0].clone(),
        ),
        path,
        field: regions.value,
        value: abstract_operations::AbstractResult {
            value: id(operation, ValueId::new),
            scalar_type: regions.scalar,
        },
        range_obligation: None,
    }
}

#[test]
fn alternative_origins_protect_each_candidate_but_not_disjoint_storage() {
    let regions = selected_record_regions();
    let types = regions
        .declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let live = live_alternative(&regions);
    let check = |path: &[StructuralPathSegment], field| {
        check_record_field_access(
            &regions.function,
            &types,
            regions.function.entry,
            1,
            &live,
            regions.root,
            path,
            field,
        )
    };

    for index in [0, 1] {
        assert!(
            check(
                &[
                    StructuralPathSegment::Field("holders".into()),
                    StructuralPathSegment::FixedIndex(index),
                ],
                Some(regions.value),
            )
            .is_err(),
            "candidate {index} remains suspended"
        );
    }
    assert!(
        check(
            &[
                StructuralPathSegment::Field("holders".into()),
                StructuralPathSegment::FixedIndex(2),
            ],
            Some(regions.value),
        )
        .is_ok(),
        "same-array sibling is disjoint from every candidate"
    );
    assert!(
        check(
            &[StructuralPathSegment::Field("unrelated".into())],
            Some(regions.value),
        )
        .is_ok(),
        "another root field is disjoint from every candidate"
    );
    assert!(
        check(&[], None).is_err(),
        "the whole root overlaps all candidates"
    );

    let functions = BTreeMap::from([(regions.function.machine, &regions.function)]);
    for (operation, canonical_path, spelled_path) in [
        (
            9_230,
            vec![
                CanonicalStructuralPathSegment::Field(regions.holders),
                CanonicalStructuralPathSegment::FixedIndex(2),
            ],
            vec![
                StructuralPathSegment::Field("holders".into()),
                StructuralPathSegment::FixedIndex(2),
            ],
        ),
        (
            9_231,
            vec![CanonicalStructuralPathSegment::Field(regions.unrelated)],
            vec![StructuralPathSegment::Field("unrelated".into())],
        ),
    ] {
        let mut after_read = live.clone();
        apply_operation(
            &regions.function,
            &functions,
            &types,
            regions.function.entry,
            1,
            &scalar_read(&regions, operation, canonical_path),
            &mut after_read,
        )
        .expect("canonical scalar read stays outside every candidate");
        apply_operation(
            &regions.function,
            &functions,
            &types,
            regions.function.entry,
            1,
            &scalar_store(&regions, operation + 10, spelled_path),
            &mut after_read,
        )
        .expect("scalar store stays outside every candidate");
    }

    for (operation, index) in [(9_240, 0), (9_241, 1)] {
        let mut observed = live.clone();
        assert!(
            apply_operation(
                &regions.function,
                &functions,
                &types,
                regions.function.entry,
                1,
                &scalar_read(
                    &regions,
                    operation,
                    vec![
                        CanonicalStructuralPathSegment::Field(regions.holders),
                        CanonicalStructuralPathSegment::FixedIndex(index),
                    ],
                ),
                &mut observed,
            )
            .is_err(),
            "canonical read of candidate {index} remains suspended"
        );
    }
}

#[test]
fn alternative_carrier_observation_requires_a_live_unsuspended_parent() {
    let regions = selected_record_regions();
    let types = regions
        .declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let mut live = live_alternative(&regions);
    let path = [StructuralPathSegment::Referent];
    let check = |live: &[LiveReference]| {
        check_record_field_access(
            &regions.function,
            &types,
            regions.function.entry,
            1,
            live,
            regions.result.place,
            &path,
            Some(regions.value),
        )
    };

    assert!(check(&live).is_ok());
    assert!(check(&[]).is_err(), "released reference has no authority");
    let mut child = live[0].clone();
    child.identity = ReferenceIdentity {
        place: id(9_224, PlaceId::new),
        path: Vec::new(),
    };
    child.carrier = child.identity.place;
    child.parent = ReferenceParent::Reference(live[0].identity.clone());
    live.push(child);
    assert!(
        check(&live).is_err(),
        "a live child suspends observation through its parent"
    );
}

#[test]
fn operation_result_rebind_rejects_a_place_pinned_by_an_alternative_origin() {
    let regions = selected_record_regions();
    let types = regions
        .declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let functions = BTreeMap::from([(regions.function.machine, &regions.function)]);
    let mut live = live_alternative(&regions);
    let operation = O::EstablishRecord {
        psi_operation: id(9_205, OperationId::new),
        result: regions.result.clone(),
        fields: Vec::new(),
    };

    assert!(matches!(
        apply_operation(
            &regions.function,
            &functions,
            &types,
            regions.function.entry,
            1,
            &operation,
            &mut live,
        ),
        Err(
            OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
                reason: "operation result remains pinned by live reference custody",
                ..
            }
        )
    ));
}
