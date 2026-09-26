use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_with_generic_data_result;
use checked_trees::CheckedTrees;

fn accepts(source: &str) -> CheckedTrees {
    let typed =
        typed_program_with_generic_data_result(source).expect("const value source should type");
    lower_typed_trees(typed, &CheckingRequest::settled()).unwrap_or_else(|diagnostics| {
        panic!("const value source should check: {diagnostics:#?}\n{source}")
    })
}

fn rejects(source: &str, fragment: &str) {
    let typed = typed_program_with_generic_data_result(source)
        .expect("const value refusal source should type");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("false const value obligation must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}\n{source}"
    );
}

#[test]
fn boolean_const_values_check_in_original_and_cloned_bodies() {
    accepts(
        "const Values::ENABLED: bool = true;
         const Values::DISABLED: bool = false;
         data Witness<const N: bool> { case Only; }
         data Main { first: Witness<Values::ENABLED>; second: Witness<Values::DISABLED>; }
         machine value<const N: bool>(witness: &Witness<N>) -> bool ensures result == N { N }
         machine Main::main(&self) -> bool {
             let enabled: bool = value(&self.first);
             value(&self.second)
         }",
    );
}

#[test]
fn const_return_promises_follow_static_binder_identity_not_spelling() {
    accepts(
        "machine endpoint<const Selected: u64>() -> u64 ensures result == Selected { Selected }
         machine forward<const Expected: u64>() -> u64
             ensures result == Expected { endpoint<Expected>() }
         machine main() -> u64 { forward<2>() }",
    );
    rejects(
        "machine endpoint<const N: u64, const M: u64>() -> u64 ensures result == N { N }
         machine forward<const N: u64, const M: u64>() -> u64
             ensures result == N { endpoint<M, N>() }
         machine main() -> u64 { forward<2, 3>() }",
        "ensures",
    );
    rejects(
        "machine endpoint<const N: u64>() -> u64 ensures result == N { N }
         machine forward<const N: u64>() -> u64 ensures result == N { endpoint<0>() }
         machine main() -> u64 { forward<0>() }",
        "ensures",
    );
}

#[test]
fn const_return_call_requires_a_declared_guarantee() {
    rejects(
        "machine endpoint<const N: u64>() -> u64 { N }
         machine forward<const N: u64>() -> u64 ensures result == N { endpoint<N>() }
         machine main() -> u64 { forward<2>() }",
        "ensures",
    );
    rejects(
        "machine endpoint<const result: u64>() -> u64 ensures result == result { 0 }
         machine forward<const N: u64>() -> u64 ensures result == N { endpoint<N>() }
         machine main() -> u64 { 0 }",
        "ensures",
    );
}

#[test]
fn symbolic_const_return_ranges_use_the_declared_binder_not_selected_values() {
    accepts(
        "machine endpoint<const N: u64>() -> u64[0..=N] { N }
        machine main() -> u64 { endpoint<2>() }",
    );
    accepts(
        "machine endpoint<const N: u64>() -> u64[0..=N] { 0 }
        machine main() -> u64 { 0 }",
    );
    accepts(
        "const Values::NEGATIVE: i32 = -2;
        machine endpoint<const N: i32>() -> i32[N..=N] { N }
        machine main() -> i32 { endpoint<Values::NEGATIVE>() }",
    );
    accepts(
        "machine endpoint<const N: i32[2..=3]>() -> i32[2..=N] { N }
        machine main() -> i32 { endpoint<2>() }",
    );
    rejects(
        "machine endpoint<const N: u64>() -> u64[N..=N] { 0 }
        machine main() -> u64 { endpoint<0>() }",
        "declared symbolic const range",
    );
    rejects(
        "machine endpoint<const N: u64, const M: u64>() -> u64[0..=N] { M }
        machine main() -> u64 { endpoint<2, 2>() }",
        "declared symbolic const range",
    );
    rejects(
        "machine endpoint<const N: i32>() -> i32[0..=N] { N }
        machine main() -> i32 { endpoint<2>() }",
        "declared symbolic const range",
    );
    rejects(
        "machine endpoint<const N: u64>() -> u64[N..=N] {
            let N: u64 = 0;
            N
        }
        machine main() -> u64 { endpoint<0>() }",
        "declared symbolic const range",
    );
}

#[test]
fn cloned_callers_keep_inferred_result_types_and_authored_annotations_distinct() {
    accepts(
        "machine value<const N: u64>(witness: &[u8; N]) -> u64[0..=N] { N }
        machine forward<const N: u64>(witness: &[u8; N]) -> u64 { value(witness) }
        machine main() -> u64 {
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let first: u64 = forward(&pair);
            forward(&triple)
        }",
    );
    rejects(
        "machine value<const N: u64>(witness: &[u8; N]) -> u64[0..=N] { N }
        machine forward<const N: u64>(witness: &[u8; N]) -> u64 {
            let __hoist_9000: u64[0..=2] = value(witness);
            __hoist_9000
        }
        machine main() -> u64 {
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let first: u64 = forward(&pair);
            forward(&triple)
        }",
        "not provably within its declared range",
    );
}

#[test]
fn array_and_case_const_values_retain_their_declared_shape() {
    accepts(
        "data Choice [copy] { case Empty; case Item(value: u8); }
         const Values::PAIR: [u8; 2] = [2, 3];
         const Values::EMPTY: Choice = Choice::Empty;
         const Values::ITEM: Choice = Choice::Item { value: 7 };
         data ArrayWitness<const N: [u8; 2]> { case Only; }
         data ChoiceWitness<const N: Choice> { case Only; }
         data Main {
             pair: ArrayWitness<Values::PAIR>;
             empty: ChoiceWitness<Values::EMPTY>;
             item: ChoiceWitness<Values::ITEM>;
         }
         machine array<const N: [u8; 2]>(witness: &ArrayWitness<N>) -> [u8; 2] { N }
         machine choice<const N: Choice>(witness: &ChoiceWitness<N>) -> Choice { N }
         machine Main::main(&self) -> Choice {
             let pair_value: [u8; 2] = array(&self.pair);
             let empty_value: Choice = choice(&self.empty);
             choice(&self.item)
         }",
    );
}
