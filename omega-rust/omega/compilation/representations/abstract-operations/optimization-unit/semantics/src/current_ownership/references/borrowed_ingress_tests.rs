//! Current-custody controls; the runtime corpus covers complete verified units.
use super::*;
use crate::tests::support::id;
use semantic_vocabulary::StructuralFieldId;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};

fn borrowed_record() -> (PsiOptimizationFunction, Vec<StructuralTypeDeclaration>, O) {
    let (mut function, mut types, result) = super::alternative_result_tests::selected_record();
    let record = id(9600, StructuralTypeId::new);
    types.push(StructuralTypeDeclaration {
        id: record,
        identity: "borrowed.reference-record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(9601, StructuralFieldId::new),
                identity: "reference".into(),
                relevance: language_core::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(result.structural_type),
            }],
        },
    });
    let parameter = &mut function.structural_parameters[0];
    parameter.structural_type = record;
    parameter.access = StructuralAccess::MutableBorrow;
    parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    let mut call = super::alternative_result_tests::call(&function, &result);
    let O::CallStructural { result, .. } = &mut call else {
        panic!("call fixture")
    };
    *result = AbstractStructuralCallResult::Unit;
    (function, types, call)
}

#[test]
fn nonescaping_carrier_borrow_retains_exact_live_permissions() {
    let (function, declarations, call) = borrowed_record();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let original = entry_references(&function, &types).unwrap();
    let mut live = original.clone();
    apply_operation(
        &function,
        &BTreeMap::new(),
        &types,
        function.entry,
        0,
        &call,
        &mut live,
    )
    .unwrap();
    assert_eq!(live, original);
    validate_normal_return(&function, &types, function.entry, &live).unwrap();
    let mut conflicting = call.clone();
    let O::CallStructural {
        structural_arguments,
        ..
    } = &mut conflicting
    else {
        panic!("call fixture")
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert!(
        apply_operation(
            &function,
            &BTreeMap::new(),
            &types,
            function.entry,
            0,
            &conflicting,
            &mut live
        )
        .is_err()
    );
}

#[test]
fn borrowed_ingress_return_rejects_missing_relocated_and_changed_permissions() {
    let (function, declarations, _) = borrowed_record();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let original = entry_references(&function, &types).unwrap();
    for mutation in 0..5 {
        let mut live = original.clone();
        match mutation {
            0 => live.clear(),
            1 => live[0].carrier = id(9700, PlaceId::new),
            2 => live[0].carrier_path = vec![StructuralPathSegment::Field("other".into())],
            3 => live[0].root = ReferenceOrigin::Primitive(id(9701, PlaceId::new)),
            _ => live.push(live[0].clone()),
        }
        assert!(
            validate_normal_return(&function, &types, function.entry, &live).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn borrowing_a_shared_carrier_does_not_grant_mutable_access() {
    let (mut function, declarations, call) = borrowed_record();
    function.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let mut live = entry_references(&function, &types).unwrap();
    assert!(
        apply_operation(
            &function,
            &BTreeMap::new(),
            &types,
            function.entry,
            0,
            &call,
            &mut live
        )
        .is_err()
    );
}
