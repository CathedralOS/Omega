use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_result;

fn rejects(source: &str, fragment: &str) {
    let typed = typed_program_result(source).expect("nested generic call types");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("invalid generic call must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

#[test]
fn nested_calls_infer_from_arguments_without_a_direct_result_annotation() {
    for expression in [
        "identity(&witness) + 1",
        "items[identity(&witness)]",
        "items[..identity(&witness)].len",
    ] {
        let source = format!(
            "machine identity<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] {{ 2 }}
             machine main() -> u64 {{
                 let witness: [u8; 2] = [0, 0];
                 let items: [u64; 4] = [10, 20, 30, 40];
                 {expression}
             }}"
        );
        lower_typed_trees(
            typed_program_result(&source).expect("nested generic expression types"),
            &CheckingRequest::settled(),
        )
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn another_complete_call_cannot_supply_a_nested_calls_missing_binding() {
    rejects(
        "machine make<Element [copy]>() -> Element { zero_value<Element>() }
         machine main() -> u64 {
             let known: u64 = make();
             make() + 1
         }",
        "specialization tuple",
    );
}

#[test]
fn conflicting_nested_generic_arguments_cannot_choose_one_binding() {
    rejects(
        "machine endpoint<const N: u64>(first: &[u8; N], second: &[u8; N]) -> u64 [0..=3] { 2 }
         machine window(items: &[i32; 4]) -> u64 {
             let pair: [u8; 2] = [0, 0];
             let triple: [u8; 3] = [0, 0, 0];
             let view: &[i32] = items[..endpoint(&pair, &triple)];
             view.len
         }",
        "specialization tuple",
    );
}

