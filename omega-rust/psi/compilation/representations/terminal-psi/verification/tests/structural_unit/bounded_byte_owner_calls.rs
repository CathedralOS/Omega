//! Qualified owner loans keep their canonical field identity through call verification.
use super::write_only_attenuation::{call_arguments, record_receiver_module};
use super::{
    AdmissionProfile, ModuleError, ProofBundle, StructuralAccess, StructuralDomainDeclaration,
    StructuralFieldType, StructuralPlaceKind, StructuralTypeShape, TerminalModule, domain_id,
    operation_id, structural_type_id, validate_module, verify_module,
};
use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet};

fn qualified_owner_call() -> TerminalModule {
    let mut module = record_receiver_module(&["line".into()]);
    module.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        unreachable!()
    };
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    fields[0].field_type = StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 16 },
        predicates,
    };
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "test::Utf8".into(),
        carrier: structural_type_id(1),
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    let callee = &mut module.machines[1];
    callee.attachment = None;
    callee.structural_parameters[0].is_self = false;
    callee.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    callee.structural_parameters[0]
        .qualifications
        .push(domain_id(1));
    callee.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    call_arguments(&mut module)[0].access = StructuralAccess::MutableBorrow;
    module
}

#[test]
fn qualified_owner_call_verifies_canonical_field_contract_transport() {
    // Full verification traverses contract substitutions even for an empty body;
    // declaration validation alone misses an unsupported canonical leaf path.
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = qualified_owner_call();
        module.machines[1].structural_parameters[0].access = access;
        call_arguments(&mut module)[0].access = access;
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("qualified inline owner retains its exact field through call composition");
    }
}

#[test]
fn qualified_owner_call_rejects_changed_capacity_access_or_preservation() {
    let module = qualified_owner_call();
    let invalid = Some(ModuleError::InvalidStructuralArgumentPath {
        operation: operation_id(1),
        argument_index: 0,
    });

    let mut changed_capacity = module.clone();
    changed_capacity.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 15 });
    assert_eq!(validate_module(&changed_capacity).err(), invalid);

    let mut changed_access = module.clone();
    changed_access.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert_eq!(validate_module(&changed_access).err(), invalid);

    let mut weakened_preservation = module;
    weakened_preservation.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(validate_module(&weakened_preservation).err(), invalid);
}
