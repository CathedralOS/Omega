use super::*;
use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeShape, TerminalIdentityTableBuilder,
};

struct Fixture {
    identities: TerminalIdentityTable,
    declarations: Vec<StructuralTypeDeclaration>,
    offset: u64,
}

impl Fixture {
    fn new(offset: u64) -> Self {
        let mut identities = TerminalIdentityTableBuilder::default();
        if offset != 0 {
            identities.intern("unrelated table prefix");
        }
        let declaration = |identities: &mut TerminalIdentityTableBuilder,
                           ordinal,
                           identity,
                           shape| StructuralTypeDeclaration {
            quotient_carrier: None,
            id: StructuralTypeId::new(offset + ordinal).unwrap(),
            identity: identities.intern(identity),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        };
        let field = |identities: &mut TerminalIdentityTableBuilder,
                     ordinal,
                     identity,
                     field_type| StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(offset + ordinal).unwrap(),
            identity: identities.intern(identity),
            relevance: BindingRelevance::Relevant,
            field_type,
            storage: None,
        };
        let common = field(
            &mut identities,
            11,
            "flag",
            StructuralFieldType::Scalar(ScalarType::Boolean),
        );
        let payload = field(
            &mut identities,
            12,
            "bits",
            StructuralFieldType::Structural(StructuralTypeId::new(offset + 2).unwrap()),
        );
        let case = StructuralCaseDeclaration {
            id: StructuralCaseId::new(offset + 70).unwrap(),
            identity: identities.intern("Active"),
            fields: vec![payload],
        };
        let declarations = vec![
            declaration(
                &mut identities,
                1,
                "Envelope",
                StructuralTypeShape::Mixed {
                    fields: vec![common],
                    cases: vec![case],
                },
            ),
            declaration(
                &mut identities,
                2,
                "[bool;2]",
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(offset + 3).unwrap(),
                    length: 2.into(),
                },
            ),
            declaration(
                &mut identities,
                3,
                "bool",
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
        ];
        Self {
            identities: identities.build(),
            declarations,
            offset,
        }
    }
    fn root(&self) -> StructuralTypeId {
        self.declarations[0].id
    }
    fn payload_path(&self, element: u64) -> Vec<Path> {
        vec![
            Path::Case(StructuralCaseId::new(self.offset + 70).unwrap()),
            Path::Field(StructuralFieldId::new(self.offset + 12).unwrap()),
            Path::FixedIndex(element),
        ]
    }
}

fn mapping<'source, 'destination>(
    source: &'source Fixture,
    destination: &'destination Fixture,
) -> ComponentContentsMapping<'source, 'destination> {
    ComponentContentsMapping {
        source_types: ProgramDeclarations::new(&source.declarations),
        source_identities: &source.identities,
        destination_types: ProgramDeclarations::new(&destination.declarations),
        destination_identities: &destination.identities,
    }
}

fn scalar(path: Vec<Path>) -> TerminalBoundaryStorageValue {
    TerminalBoundaryStorageValue {
        path,
        contents: TerminalStorageContents::Scalar {
            value: crate::TerminalScalarValue::Boolean(true),
            stored_scalar: ScalarType::Boolean,
        },
    }
}

#[test]
fn component_contents_rehome_nominal_cases_payloads_and_array_cells() {
    let source = Fixture::new(0);
    let destination = Fixture::new(100);
    let mapping = mapping(&source, &destination);
    let selected = mapping
        .rehome(
            source.root(),
            destination.root(),
            TerminalBoundaryStorageValue {
                path: vec![],
                contents: TerminalStorageContents::Case(StructuralCaseId::new(70).unwrap()),
            },
        )
        .unwrap();
    assert_eq!(
        selected.contents,
        TerminalStorageContents::Case(StructuralCaseId::new(170).unwrap())
    );
    let payload = mapping
        .rehome(
            source.root(),
            destination.root(),
            scalar(source.payload_path(1)),
        )
        .unwrap();
    assert_eq!(payload.path, destination.payload_path(1));
    let common = mapping
        .rehome(
            source.root(),
            destination.root(),
            scalar(vec![Path::Field(StructuralFieldId::new(11).unwrap())]),
        )
        .unwrap();
    assert_eq!(
        common.path,
        vec![Path::Field(StructuralFieldId::new(111).unwrap())]
    );
}

#[test]
fn component_contents_refuse_reference_crossings_bad_indexes_and_nominal_substitutions() {
    let source = Fixture::new(0);
    let mut destination = Fixture::new(100);
    assert!(
        mapping(&source, &destination)
            .rehome(
                source.root(),
                destination.root(),
                scalar(vec![Path::Referent])
            )
            .is_err()
    );
    assert!(
        mapping(&source, &destination)
            .rehome(
                source.root(),
                destination.root(),
                scalar(source.payload_path(2))
            )
            .is_err()
    );
    let mut identities = destination.identities.clone().into_builder();
    destination.declarations[2].identity = identities.intern("OtherBool");
    destination.identities = identities.build();
    assert!(
        mapping(&source, &destination)
            .rehome(
                source.root(),
                destination.root(),
                scalar(source.payload_path(1))
            )
            .is_err()
    );
    // Equal dense coordinates cannot stand in for a different declared field.
    let mut destination = Fixture::new(100);
    let mut identities = destination.identities.clone().into_builder();
    let wrong = identities.intern("unrelated_bits");
    let StructuralTypeShape::Mixed { cases, .. } = &mut destination.declarations[0].shape else {
        panic!("fixture is mixed")
    };
    cases[0].fields[0].identity = wrong;
    destination.identities = identities.build();
    assert!(
        mapping(&source, &destination)
            .rehome(
                source.root(),
                destination.root(),
                scalar(source.payload_path(1))
            )
            .is_err()
    );
}
