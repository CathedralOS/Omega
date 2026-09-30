use super::{
    DeclarationLifetimeFrontier, carried_lifetimes, declaration_lifetime_frontier,
    substituted_carried_lifetimes, substituted_result_is_view_free, whole_elided_result_accesses,
};
use crate::borrow::BorrowOwnerSegment;
use crate::tests::front_end::typed_program;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

fn array_type(
    program: &mut TypedTrees,
    element_type: TypeReferenceHandle,
    length: usize,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        })
}

#[test]
fn large_view_free_arrays_inspect_structure_without_enumerating_elements() {
    let program = typed_program(
        "data Record { payload: [[u8; 65536]; 65536]; } \
         trait Producer { machine result() -> [Record; 65536]; }",
    );
    let result = program.trait_machine_signatures(&program.traits()[0])[0].return_type;
    assert!(substituted_result_is_view_free(&program, result, &[]));
    assert!(
        carried_lifetimes(&program, result)
            .expect("closed frontier")
            .is_empty()
    );
    assert_eq!(
        declaration_lifetime_frontier(&program, result, &[]),
        DeclarationLifetimeFrontier::Complete
    );
}

#[test]
fn array_frontiers_preserve_prefixes_case_paths_lifetimes_and_access_order() {
    let program = typed_program(
        r#"
        data Leaf<'source> {
            shared: &'source i32;
            mutable: &'source mut i32;
            write_only: &'source write i32;
        }
        data Choice<'item> { case Present(leaves: [Leaf<'item>; 2]); case Empty; }
        data Envelope<'buffer> { choices: [Choice<'buffer>; 2]; }
        trait Producer<'caller> { machine result() -> Envelope<'caller>; }
        "#,
    );
    let result = program.trait_machine_signatures(&program.traits()[0])[0].return_type;
    let frontier = carried_lifetimes(&program, result).expect("closed frontier");
    let definition = |name: &str| {
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .expect("data definition")
    };
    let DataMember::Field(choices) = &program.data_members(definition("Envelope"))[0] else {
        panic!("choices field");
    };
    let DataMember::Variant(present) = &program.data_members(definition("Choice"))[0] else {
        panic!("Present case");
    };
    let leaves = program.data_payload_fields(present)[0].symbol;
    let fields = program.data_members(definition("Leaf"));
    let mut actual_leaves = frontier.iter();
    for choice_index in 0..2 {
        for leaf_index in 0..2 {
            for (field, access) in fields.iter().zip([
                ReferenceAccess::Shared,
                ReferenceAccess::Mutable,
                ReferenceAccess::WriteOnly,
            ]) {
                let DataMember::Field(field) = field else {
                    panic!("reference field");
                };
                let leaf = actual_leaves
                    .next()
                    .expect("one leaf per field and array element");
                assert_eq!(
                    leaf.owner_path,
                    [
                        BorrowOwnerSegment::Field(choices.symbol),
                        BorrowOwnerSegment::FixedIndex(choice_index),
                        BorrowOwnerSegment::Case(present.symbol),
                        BorrowOwnerSegment::Field(leaves),
                        BorrowOwnerSegment::FixedIndex(leaf_index),
                        BorrowOwnerSegment::Field(field.symbol),
                    ]
                );
                assert_eq!(leaf.lifetime.as_deref(), Some("caller"));
                assert_eq!(leaf.access, access);
            }
        }
    }
    assert!(actual_leaves.next().is_none());
}

#[test]
fn array_templates_retain_dependency_until_exact_substitution() {
    let program =
        typed_program("trait Producer<T> { machine result(value: &i32, scalar: i32) -> [T; 2]; }");
    let definition = &program.traits()[0];
    let parameter = program.trait_type_parameters(definition)[0].symbol;
    let signature = &program.trait_machine_signatures(definition)[0];
    let result = signature.return_type;
    let parameters = program.state_signature_parameters(signature);
    assert_eq!(
        declaration_lifetime_frontier(&program, result, &[parameter]),
        DeclarationLifetimeFrontier::TemplateDependent
    );
    assert!(!substituted_result_is_view_free(&program, result, &[]));
    assert!(substituted_result_is_view_free(
        &program,
        result,
        &[(parameter, parameters[1].type_reference)],
    ));
    let substitutions = [(parameter, parameters[0].type_reference)];
    assert!(!substituted_result_is_view_free(
        &program,
        result,
        &substitutions
    ));
    let frontier = substituted_carried_lifetimes(&program, result, &substitutions)
        .expect("reference substitution closes the frontier");
    assert_eq!(frontier.len(), 2);
    for (element_index, leaf) in frontier.iter().enumerate() {
        assert_eq!(
            leaf.owner_path,
            [BorrowOwnerSegment::FixedIndex(element_index)]
        );
        assert_eq!(leaf.access, ReferenceAccess::Shared);
        assert_eq!(leaf.lifetime, None);
    }
}

#[test]
fn nonempty_arrays_reject_incomplete_elements_but_empty_arrays_do_not_visit_them() {
    let mut program = typed_program(
        "data Partial { valid: &i32; unknown: [u8]; } \
         data Recursive { valid: &i32; next: Recursive; } \
         trait Producer { machine partial() -> Partial; machine recursive() -> Recursive; }",
    );
    let signatures = program.trait_machine_signatures(&program.traits()[0]);
    let partial = signatures[0].return_type;
    let recursive = signatures[1].return_type;
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let stale = arena::Handle::from_parts(unit.arena_index(), unit.generation() + 1);
    for element in [partial, recursive, stale] {
        let nonempty = array_type(&mut program, element, 2);
        assert!(carried_lifetimes(&program, nonempty).is_none());
        assert!(!substituted_result_is_view_free(&program, nonempty, &[]));
        assert_eq!(
            declaration_lifetime_frontier(&program, nonempty, &[]),
            DeclarationLifetimeFrontier::Incomplete
        );
        let empty = array_type(&mut program, element, 0);
        assert!(substituted_result_is_view_free(&program, empty, &[]));
    }
    let recursive_array = array_type(&mut program, recursive, 2);
    assert_eq!(
        whole_elided_result_accesses(&program, recursive_array),
        Some(vec![ReferenceAccess::Shared; 2]),
        "the existing whole-input elision fallback still retains access"
    );
}
