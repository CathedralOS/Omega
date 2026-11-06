use super::validate_constructor_claims;
use crate::CodecError;
use semantic_vocabulary::{
    ClaimId, PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment as Path,
    StructuralRecordStorage, StructuralResultClaimBinding, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
};
fn id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn field(identity: u32, child: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        inline_carrier: None,
        id: StructuralFieldId::new(u64::from(identity)).unwrap(),
        identity: TerminalIdentityIndex(identity),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id(child)),
        storage: None,
    }
}
fn declaration(identity: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: id(identity),
        identity: TerminalIdentityIndex(identity as u32),
        owns_linear_claim: identity == 3,
        copyable: false,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        shape,
        scalar_restriction: None,
    }
}
fn module() -> TerminalModule {
    TerminalModule {
        structural_types: vec![
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![field(10, 2)],
                    storage: StructuralRecordStorage::Packed,
                },
            ),
            declaration(
                2,
                StructuralTypeShape::Sum {
                    cases: [1, 2]
                        .into_iter()
                        .map(|case| StructuralCaseDeclaration {
                            id: StructuralCaseId::new(case).unwrap(),
                            identity: TerminalIdentityIndex(case as u32),
                            fields: vec![field(11, 3), field(12, 3)],
                        })
                        .collect(),
                },
            ),
            declaration(
                3,
                StructuralTypeShape::Record {
                    fields: vec![],
                    storage: StructuralRecordStorage::Packed,
                },
            ),
        ],
        ..TerminalModule::declarations()
    }
}
fn payload(case: u64, field: u32) -> Vec<Path> {
    vec![
        Path::Field(TerminalIdentityIndex(10)),
        Path::Case(StructuralCaseId::new(case).unwrap()),
        Path::Field(TerminalIdentityIndex(field)),
    ]
}
fn result(paths: Vec<Vec<Path>>) -> StructuralOperationResult {
    StructuralOperationResult {
        place: PlaceId::new(1).unwrap(),
        structural_type: id(1),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| StructuralResultClaimBinding {
                claim: ClaimId::new(index as u64 + 1).unwrap(),
                path,
            })
            .collect(),
    }
}
#[test]
fn constructor_catalogue_preserves_alternative_and_sibling_claims() {
    assert_eq!(
        validate_constructor_claims(
            &module(),
            &result(vec![payload(1, 11), payload(1, 12), payload(2, 11)])
        ),
        Ok(())
    );
}
#[test]
fn constructor_catalogue_rejects_duplicate_and_same_case_overlap() {
    for paths in [
        vec![payload(1, 11), payload(1, 11)],
        vec![vec![], payload(1, 11)],
        vec![vec![Path::Field(TerminalIdentityIndex(10))], payload(1, 11)],
    ] {
        assert_eq!(
            validate_constructor_claims(&module(), &result(paths)),
            Err(CodecError::MalformedStructuralFoundation(
                "aggregate claim bindings overlap"
            ))
        );
    }
}
#[test]
fn constructor_catalogue_validates_later_case_before_disjointness() {
    let error =
        validate_constructor_claims(&module(), &result(vec![payload(1, 11), payload(3, 11)]))
            .unwrap_err();
    assert_eq!(
        error,
        CodecError::MalformedStructuralFoundation("structural path has an unknown case")
    );
}
