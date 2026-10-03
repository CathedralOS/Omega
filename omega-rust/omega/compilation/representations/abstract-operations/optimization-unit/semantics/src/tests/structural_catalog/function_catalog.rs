//! Function structural-signature and attachment catalog tests.
use crate::tests::fixtures::dominance::byte_literal_boundary_unit;
use crate::tests::fixtures::scalar_units::structural_call_unit;
use crate::tests::support::{id, refresh_identity};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};

#[test]
fn attached_and_named_inputs_retain_established_nonempty_fields() {
    for attached in [false, true] {
        let mut candidate = structural_call_unit();
        let structural_type = candidate.structural_types[0].id;
        let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
        predicates.insert(terminal_psi::ByteSequencePredicate::NonEmpty);
        candidate.structural_types.make_mut()[0].shape =
            terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: id(4_712, semantic_vocabulary::StructuralFieldId::new),
                    identity: "established-text".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::PredicateByteSequence {
                        carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                        predicates,
                    },
                }],
            };
        if attached {
            candidate.functions[0].attachment = Some(structural_type);
            candidate.functions[0].structural_parameters[0].is_self = true;
            let StructuralPlaceKind::Parameter { is_self, .. } =
                &mut candidate.functions[0].structural_places[0].kind
            else {
                panic!("parameter root");
            };
            *is_self = true;
        }
        refresh_identity(&mut candidate);
        assert_eq!(validate_psi_optimization_unit(&candidate), Ok(()));
    }
}

#[test]
fn structural_signatures_replay_attachment_and_unique_self_legality() {
    let mut attached = structural_call_unit();
    let structural_type = attached.structural_types[0].id;
    attached.functions[0].attachment = Some(structural_type);
    attached.functions[0].structural_parameters[0].is_self = true;
    let StructuralPlaceKind::Parameter { is_self, .. } =
        &mut attached.functions[0].structural_places[0].kind
    else {
        panic!("fixture retains its parameter root")
    };
    *is_self = true;
    refresh_identity(&mut attached);
    validate_psi_optimization_unit(&attached)
        .expect("one attachment-typed self parameter is canonical");

    let mut self_without_attachment = attached.clone();
    self_without_attachment.functions[0].attachment = None;
    refresh_identity(&mut self_without_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&self_without_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut mismatched_self = attached.clone();
    let alternate = id(4_710, StructuralTypeId::new);
    mismatched_self
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-attachment".into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        });
    mismatched_self.functions[0].attachment = Some(alternate);
    refresh_identity(&mut mismatched_self);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut duplicate_self = attached.clone();
    let mut second = duplicate_self.functions[0].structural_parameters[0].clone();
    second.place = id(4_711, PlaceId::new);
    second.position = 1;
    duplicate_self.functions[0]
        .structural_parameters
        .push(second.clone());
    duplicate_self.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: second.place,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: true,
            },
        });
    refresh_identity(&mut duplicate_self);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut unknown_function_attachment = structural_call_unit();
    unknown_function_attachment.functions[0].attachment = Some(id(4_799, StructuralTypeId::new));
    refresh_identity(&mut unknown_function_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&unknown_function_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut boundary_self = byte_literal_boundary_unit();
    let boundary_type = boundary_self.boundary_machines[0].structural_parameters[0].structural_type;
    // Only machine declarations may carry a nominal attachment.
    boundary_self.boundary_machines[0].kind = terminal_psi::BoundaryRequirementKind::Machine;
    boundary_self.boundary_machines[0].attachment = Some(boundary_type);
    boundary_self.boundary_machines[0].structural_parameters[0].is_self = true;
    refresh_identity(&mut boundary_self);
    validate_psi_optimization_unit(&boundary_self)
        .expect("boundary self uses the exact known attachment type");

    let mut trait_with_attachment = boundary_self.clone();
    trait_with_attachment.boundary_machines[0].kind = terminal_psi::BoundaryRequirementKind::Trait;
    refresh_identity(&mut trait_with_attachment);
    assert_eq!(
        validate_psi_optimization_unit(&trait_with_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None })
    );

    boundary_self.boundary_machines[0].attachment = Some(id(4_798, StructuralTypeId::new));
    refresh_identity(&mut boundary_self);
    assert_eq!(
        validate_psi_optimization_unit(&boundary_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None })
    );
}
