//! Isolate the receiving loan transition; corpus fixtures test full units.
use super::*;
use crate::tests::support::id;
use semantic_vocabulary::{OperationId, StructuralFieldId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
};

#[test]
fn owned_ingress_leaf_moves_without_minting_an_escaping_child() {
    let (mut function, mut declarations, result) =
        super::alternative_result_tests::selected_record();
    let record = id(9400, StructuralTypeId::new);
    declarations.push(StructuralTypeDeclaration {
        id: record,
        identity: "test.reference-record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(9401, StructuralFieldId::new),
                identity: "body".into(),
                relevance: language_core::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(result.structural_type),
            }],
        },
    });
    let parameter = &mut function.structural_parameters[0];
    parameter.structural_type = record;
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    let source = StructuralArgument {
        place: parameter.place,
        path: vec![
            StructuralPathSegment::Field("body".into()),
            StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let types = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let original = entry_references(&function, &types).unwrap();
    let identity = original[0].identity.clone();
    assert_eq!(
        transferred_ingress_leaf(&function, &types, &original, &source),
        Some(identity.clone())
    );
    for mutation in 0..5 {
        let mut changed = original.clone();
        let mut changed_source = source.clone();
        let mut changed_function = function.clone();
        match mutation {
            0 => changed.clear(),
            1 => changed.push(changed[0].clone()),
            2 => changed[0].parent = ReferenceParent::Reference(identity.clone()),
            3 => changed_source.path[0] = StructuralPathSegment::Field("other".into()),
            4 => changed_function.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            _ => unreachable!(),
        }
        assert!(
            transferred_ingress_leaf(&changed_function, &types, &changed, &changed_source)
                .is_none(),
            "{mutation}"
        );
    }
    let mut live = original;
    apply_operation(
        &function,
        &BTreeMap::new(),
        &types,
        function.entry,
        0,
        &O::EstablishReference {
            psi_operation: id(9402, OperationId::new),
            result: result.clone(),
            source,
        },
        &mut live,
    )
    .unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].identity, identity);
    assert_eq!(live[0].carrier, result.place);
    assert!(live[0].carrier_path.is_empty());
    assert_eq!(live[0].parent, ReferenceParent::Root(live[0].root.clone()));
}
