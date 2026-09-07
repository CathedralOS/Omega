use super::{lower_typed_trees, typed_source};

fn rejects(source: &str, fragment: &str) {
    let typed = typed_source(source).expect("nested generic call types");
    let diagnostics = lower_typed_trees(typed).expect_err("invalid generic call must reject");
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
        lower_typed_trees(typed_source(&source).expect("nested generic expression types"))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn generic_selector_infers_a_borrowed_field_type() {
    let source = "data Marker [copy] { value: i32; }
        data Main { marker: Marker; }
        machine Main::endpoint<Element [copy]>(&self, value: &Element) -> u64 [0..=3] { 2 }
        machine Main::window(&self, items: &[i32; 4]) -> u64 {
            let view: &[i32] = items[..self.endpoint(&self.marker)];
            view.len
        }";
    lower_typed_trees(typed_source(source).expect("borrowed field selector types"))
        .expect("field type supplies the nested call's generic argument");
}

#[test]
fn nested_generic_selector_waits_for_its_callers_concrete_witness() {
    let source = "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] { 2 }
        machine window<const N: u64>(items: &[i32; 4], witness: &[u8; N]) -> u64 {
            let view: &[i32] = items[..endpoint(witness)];
            view.len
        }
        machine main() -> u64 {
            let items: [i32; 4] = [0, 0, 0, 0];
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let first: u64 = window(&items, &pair);
            let second: u64 = window(&items, &triple);
            second
        }";
    lower_typed_trees(typed_source(source).expect("forwarded generic selector types"))
        .expect("specializing callers exposes each concrete nested tuple");
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
fn nested_generic_call_without_binding_evidence_still_rejects() {
    rejects(
        "machine endpoint<const N: u64>() -> u64 [0..=3] { 2 }
         machine window(items: &[i32; 4]) -> u64 {
             let view: &[i32] = items[..endpoint()];
             view.len
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

#[test]
fn generic_selector_specialization_does_not_prove_false_return_bounds() {
    rejects(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=3] { 5 }
         machine window(items: &[i32; 4]) -> u64 {
             let witness: [u8; 5] = [0, 0, 0, 0, 0];
             let view: &[i32] = items[..endpoint(&witness)];
             view.len
         }",
        "not provably within its declared range",
    );
}

#[test]
fn valid_generic_return_bounds_must_still_fit_the_selected_collection() {
    rejects(
        "machine endpoint<const N: u64>(witness: &[u8; N]) -> u64 [0..=5] { 5 }
         machine window(items: &[i32; 4]) -> u64 {
             let witness: [u8; 5] = [0, 0, 0, 0, 0];
             let view: &[i32] = items[..endpoint(&witness)];
             view.len
         }",
        "cannot prove subslice range",
    );
}
