//! A separate inline field does not separate a domain's borrowed byte backing.
//! These component controls supply an already-held duty, then ask the existing
//! preservation owner to frame it across a sibling write. They grant no loan,
//! access, membership establishment, or whole-module admission.
use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, IntegerSign, IntegerType, IntegerValue, Proposition,
    StructuralFieldId,
};
use terminal_psi::{
    ArrayExtent, BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate,
    StructuralDomainPredicate, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathQualification, StructuralRecordStorage, TerminalIdentityTableBuilder,
};

enum ByteStorage {
    InlineArray,
    BorrowedView,
    BoundedOwned,
}

enum DomainObservation {
    Bytes,
    InlineFlag,
}

struct Fixture {
    module: TerminalModule,
    held: PayloadMembershipContract,
    sibling: StructuralFieldId,
    flag_name: TerminalIdentityIndex,
    bytes_name: TerminalIdentityIndex,
}

fn field(
    raw: u64,
    identity: TerminalIdentityIndex,
    field_type: StructuralFieldType,
    inline_carrier: Option<StructuralTypeId>,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(raw).unwrap(),
        identity,
        inline_carrier,
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

fn fixture(storage: ByteStorage, observation: DomainObservation) -> Fixture {
    let boolean = StructuralTypeId::new(1).unwrap();
    let octet = StructuralTypeId::new(2).unwrap();
    let bytes = StructuralTypeId::new(3).unwrap();
    let aggregate = StructuralTypeId::new(4).unwrap();
    let outer = StructuralTypeId::new(5).unwrap();
    let octet_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let sibling = StructuralFieldId::new(1).unwrap();
    let flag = StructuralFieldId::new(3).unwrap();
    let byte_field = StructuralFieldId::new(4).unwrap();
    let mut identities = TerminalIdentityTableBuilder::default();
    let type_names =
        ["Bool", "u8", "Bytes", "Aggregate", "Outer"].map(|name| identities.intern(name));
    let sibling_name = identities.intern("Outer::a");
    let aggregate_name = identities.intern("Outer::b");
    let flag_name = identities.intern("Aggregate::flag");
    let bytes_name = identities.intern("Aggregate::bytes");
    let domain_name = identities.intern("Aggregate::ObservedRule");
    let borrowed = matches!(storage, ByteStorage::BorrowedView);
    let byte_shape = match storage {
        ByteStorage::InlineArray => StructuralTypeShape::FixedArray {
            element: octet,
            length: ArrayExtent::Literal(1),
        },
        ByteStorage::BorrowedView => {
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            })
        }
        ByteStorage::BoundedOwned => {
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 1 })
        }
    };
    let shapes = [
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(octet_type)),
        byte_shape,
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                field(
                    flag.get(),
                    flag_name,
                    StructuralFieldType::Scalar(ScalarType::Boolean),
                    Some(boolean),
                ),
                field(
                    byte_field.get(),
                    bytes_name,
                    StructuralFieldType::Structural(bytes),
                    None,
                ),
            ],
        },
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                field(
                    sibling.get(),
                    sibling_name,
                    StructuralFieldType::Scalar(ScalarType::Integer(octet_type)),
                    Some(octet),
                ),
                field(
                    2,
                    aggregate_name,
                    StructuralFieldType::Structural(aggregate),
                    None,
                ),
            ],
        },
    ];
    let types = [boolean, octet, bytes, aggregate, outer]
        .into_iter()
        .zip(type_names)
        .zip(shapes)
        .map(|((id, identity), shape)| StructuralTypeDeclaration {
            id,
            identity,
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: borrowed && id.get() >= bytes.get(),
            scalar_restriction: None,
            shape,
        })
        .collect();
    let subject = PlaceId::new(77).unwrap();
    let proposition = Proposition::Equal(
        match observation {
            DomainObservation::Bytes => ScalarTerm::ByteSequencePredicate {
                predicate: ByteSequencePredicate::NoNul,
                root: subject,
                path: vec![Segment::Field(byte_field)],
            },
            DomainObservation::InlineFlag => {
                ScalarTerm::boolean_field_path(subject, vec![Segment::Field(flag)])
            }
        },
        ScalarTerm::Boolean(true),
    );
    let domain = StructuralDomainId::new(1).unwrap();
    let module = TerminalModule {
        identities: identities.build(),
        structural_types: types,
        structural_domains: vec![StructuralDomainDeclaration {
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject,
                proposition,
            }),
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: domain_name,
            carrier: aggregate,
            content_projection: None,
            // This is a predicate on an aggregate, not a primitive-byte label.
            byte_predicate: None,
            establishment_routes: Vec::new(),
        }],
        ..TerminalModule::declarations()
    };
    let projected = vec![StructuralPathSegment::Field(aggregate_name)];
    let declaration = &module.structural_domains[0];
    let predicate = declaration.predicate.as_ref().unwrap();
    predicate.proposition.validate().unwrap();
    assert!(declaration.required_memberships_are_well_formed(&module.structural_domains));
    assert!(
        declaration
            .required_domain_closure(&module.structural_domains)
            .is_some_and(|closure| closure.len() == 1
                && closure[0].complete_predicate_theory
                && closure[0].predicate.is_some())
    );
    assert!(crate::byte_predicate_carrier(
        module.structural_types.iter(),
        bytes
    ));
    assert_eq!(
        crate::qualification_path_carrier(module.structural_types.iter(), outer, &projected),
        Some(declaration.carrier),
    );
    assert!(crate::domain_predicate_is_closed(
        module.structural_types.iter(),
        declaration.carrier,
        predicate,
    ));
    Fixture {
        module,
        held: PayloadMembershipContract {
            carrier: outer,
            qualifications: Vec::new(),
            projected_qualifications: vec![StructuralPathQualification {
                path: projected,
                domain,
            }],
            scalar_requirements: Vec::new(),
            scalar_observations: Vec::new(),
        },
        sibling,
        flag_name,
        bytes_name,
    }
}

fn preserve_sibling_write(fixture: &Fixture) -> Result<(), &'static str> {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    super::super::primitive_writes::preserves_roster(
        &fixture.module,
        &fixture.held,
        &[DescriptorPathSegment::Field(fixture.sibling)],
        &ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap(),
    )
}

#[test]
fn projected_inline_byte_rule_survives_an_ordinary_sibling_write() {
    let fixture = fixture(ByteStorage::InlineArray, DomainObservation::Bytes);
    assert!(crate::domain_write_is_disjoint(
        fixture.module.structural_types.iter(),
        &fixture.module.identities,
        &fixture.module.structural_domains[0],
        &[StructuralPathSegment::Field(fixture.flag_name)],
    ));
    assert_eq!(preserve_sibling_write(&fixture), Ok(()));
}

#[test]
fn projected_aggregate_byte_view_rule_needs_backing_separation() {
    let fixture = fixture(ByteStorage::BorrowedView, DomainObservation::Bytes);
    // The existing domain owner accepts this closed rule but cannot frame its
    // indirect observation even against another member of the same aggregate.
    assert!(!crate::domain_write_is_disjoint(
        fixture.module.structural_types.iter(),
        &fixture.module.identities,
        &fixture.module.structural_domains[0],
        &[StructuralPathSegment::Field(fixture.flag_name)],
    ));
    assert_eq!(
        preserve_sibling_write(&fixture),
        Err("scalar preservation needs captured domain dependency evidence"),
        "separating the aggregate's word from a sibling does not separate its byte backing",
    );
}

#[test]
fn unused_borrowed_view_does_not_taint_an_inline_only_aggregate_rule() {
    let fixture = fixture(ByteStorage::BorrowedView, DomainObservation::InlineFlag);
    // The complete rule observes only `flag`, not the view's descriptor or
    // backing. Recursive shape rejection alone would lose this valid framing.
    assert!(crate::domain_write_is_disjoint(
        fixture.module.structural_types.iter(),
        &fixture.module.identities,
        &fixture.module.structural_domains[0],
        &[StructuralPathSegment::Field(fixture.bytes_name)],
    ));
    assert_eq!(preserve_sibling_write(&fixture), Ok(()));
}

#[test]
fn opaque_inline_subject_keeps_its_held_duty_across_a_sibling_write() {
    let mut fixture = fixture(ByteStorage::InlineArray, DomainObservation::Bytes);
    let domain = &mut fixture.module.structural_domains[0];
    domain.complete_predicate_theory = false;
    domain.predicate = None;
    domain.required_memberships = None;
    assert!(
        fixture.module.structural_domains[0]
            .required_memberships_are_well_formed(&fixture.module.structural_domains)
    );
    assert!(
        fixture.module.structural_domains[0]
            .required_domain_closure(&fixture.module.structural_domains)
            .is_none()
    );
    assert!(plain_owned_shape(
        fixture.module.structural_types.iter(),
        fixture.module.structural_domains[0].carrier,
    ));
    // No rule or prerequisite is invented. The exact held subject is entirely
    // inline and the typed ordinary sibling write leaves that value unchanged.
    assert_eq!(preserve_sibling_write(&fixture), Ok(()));
}

#[test]
fn bounded_owned_byte_label_keeps_its_held_duty_across_a_sibling_write() {
    let mut fixture = fixture(ByteStorage::BoundedOwned, DomainObservation::Bytes);
    let bytes = StructuralTypeId::new(3).unwrap();
    let domain = &mut fixture.module.structural_domains[0];
    domain.carrier = bytes;
    domain.complete_predicate_theory = false;
    domain.predicate = None;
    domain.byte_predicate = Some(ByteSequencePredicate::NoNul);
    fixture.held.projected_qualifications[0]
        .path
        .push(StructuralPathSegment::Field(fixture.bytes_name));
    let domain = &fixture.module.structural_domains[0];
    // This is the admitted byte-label declaration lane: a byte carrier, no
    // prerequisites, issuer routes or Content projection. It does not claim a
    // predicate theory that the declaration does not publish.
    assert!(domain.has_no_required_memberships());
    assert!(domain.required_memberships_are_well_formed(&fixture.module.structural_domains));
    assert!(domain.establishment_routes.is_empty());
    assert!(domain.content_projection.is_none());
    assert!(crate::byte_predicate_carrier(
        fixture.module.structural_types.iter(),
        bytes
    ));
    assert!(matches!(
        fixture.module.structural_types[2].shape,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 1 })
    ));
    assert_eq!(
        crate::qualification_path_carrier(
            fixture.module.structural_types.iter(),
            fixture.held.carrier,
            &fixture.held.projected_qualifications[0].path,
        ),
        Some(domain.carrier),
    );
    assert!(plain_owned_shape(
        fixture.module.structural_types.iter(),
        bytes
    ));
    assert_eq!(preserve_sibling_write(&fixture), Ok(()));
}
