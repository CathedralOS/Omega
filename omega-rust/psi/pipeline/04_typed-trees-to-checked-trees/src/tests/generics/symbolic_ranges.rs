use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_result;

#[test]
fn discarded_calls_in_open_templates_validate_inferred_const_bounds() {
    for (length, accepted) in [(0, true), (2, true), (3, true), (4, false), (5, false)] {
        let source = format!("machine endpoint<const N: u64[0..=3]>(witness: &[u8; N]) -> u64 {{ N }}
            machine forward<Value>(unused: Value, witness: &[u8; {length}]) {{ _ = endpoint(witness); }}");
        let typed = typed_program_result(&source).expect("discarded generic call types");
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        if let Ok(checked) = result {
            assert!(
                checked.machine_specializations.is_empty(),
                "the unused open caller remains generic"
            );
        }
    }
}

#[test]
fn open_templates_forward_array_length_binders_only_with_matching_bounds() {
    for (declared, accepted) in [("u64[0..=3]", true), ("u64[0..=2]", false), ("u64", false)] {
        let source = format!(
            "machine endpoint<const N: u64[0..=3]>(witness: &[u8; N]) -> u64 {{ N }}
            machine forward<const K: {declared}>(witness: &[u8; K]) {{ _ = endpoint(witness); }}"
        );
        let typed = typed_program_result(&source).expect("forwarded generic call types");
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        if let Ok(checked) = result {
            assert!(
                checked.machine_specializations.is_empty(),
                "the open caller keeps its symbolic call"
            );
        }
    }
}

#[test]
fn open_mutable_symbolic_parameter_stores_preserve_the_declared_range() {
    let source = "machine store<const N: u64>(mut value: u64[0..=N]) { value = N; value = 0; }";
    let typed = typed_program_result(source).expect("symbolic mutable parameter types");
    assert_eq!(typed.machine_type_parameters(&typed.machines()[0]).len(), 1);
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("both assignments satisfy the declared symbolic range");
    assert!(checked.machine_specializations.is_empty());
}
