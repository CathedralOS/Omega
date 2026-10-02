//! Read-only presentation retains a whole owner's storage and qualifications.
use super::*;

fn whole_owner_module() -> TerminalModule {
    let mut module = ordinary_buffer_module();
    module.structural_types[1].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "Owner::Utf8".into(),
        carrier: structural_type_id(2),
        byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    module.machines[0].structural_parameters[0].qualifications = vec![domain_id(1)];
    for parameter in &mut module.machines[1].structural_parameters {
        parameter.access = StructuralAccess::SharedBorrow;
    }
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    for argument in structural_arguments {
        argument.path.clear();
        argument.access = StructuralAccess::SharedBorrow;
    }
    module
}

#[test]
fn qualified_whole_byte_owner_lends_read_only_without_erasing_storage() {
    let module = whole_owner_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a shared presentation observes the qualified owner's current extent");
    assert_eq!(
        module.machines[0].structural_parameters[0].qualifications,
        vec![domain_id(1)]
    );
    assert_eq!(
        module.machines[0].structural_parameters[0].structural_type,
        structural_type_id(2)
    );
}

#[test]
fn whole_byte_presentation_cannot_read_write_only_or_lend_exclusive_view() {
    let mut unreadable = whole_owner_module();
    unreadable.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    assert!(
        validate_module(&unreadable).is_err(),
        "write-only authority cannot observe bytes"
    );

    let mut exclusive = whole_owner_module();
    for parameter in &mut exclusive.machines[1].structural_parameters {
        parameter.access = StructuralAccess::MutableBorrow;
    }
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut exclusive.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    for argument in structural_arguments {
        argument.access = StructuralAccess::MutableBorrow;
    }
    assert!(
        validate_module(&exclusive).is_err(),
        "exclusive view cannot erase owner invariants"
    );
}

#[test]
fn whole_byte_presentation_does_not_invent_formal_predicates() {
    let mut module = whole_owner_module();
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain_id(2),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
        identity: "View::Ascii".into(),
        carrier: structural_type_id(1),
        byte_predicate: Some(terminal_psi::ByteSequencePredicate::AsciiOnly),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    module.machines[1].structural_parameters[0].qualifications = vec![domain_id(2)];
    assert!(
        validate_module(&module).is_err(),
        "UTF-8 ownership does not prove ASCII view membership"
    );
}
