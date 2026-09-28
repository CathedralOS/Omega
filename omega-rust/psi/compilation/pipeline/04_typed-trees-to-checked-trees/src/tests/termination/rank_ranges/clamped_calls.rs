use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const PAIR: &str = r#"
data Main {}
machine Main::first(&mut self, limit: u64, cursor: u64, capacity: u64)
requires limit <= capacity;
terminates by cursor -> Nat::IncreasingTo(limit) in 0..=capacity;
-> u64 {
    transition { _ -> self.second(capacity, cursor, limit) }
}
machine Main::second(&mut self, ceiling: u64, position: u64, bound: u64)
requires bound <= ceiling;
terminates by position -> Nat::IncreasingTo(bound) in 0..=ceiling;
-> u64 {
    transition position < bound {
        true -> self.first(bound, position + 1, ceiling)
        false -> position
    }
}
"#;

#[test]
fn clamped_call_ranges_allow_entry_and_forwarding_beyond_the_bound() {
    for cursor in [4, 6, 9] {
        let source =
            format!("{PAIR} machine Main::main(&mut self) -> u64 {{ self.first(6, {cursor}, 8) }}");
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn clamped_call_ranges_still_check_exclusive_ceilings_and_positive_floors() {
    let exclusive = PAIR
        .replace("limit <= capacity", "limit < capacity")
        .replace("bound <= ceiling", "bound < ceiling")
        .replace("0..=capacity", "0..capacity")
        .replace("0..=ceiling", "0..ceiling");
    lower_typed_trees(typed_program(&exclusive), &CheckingRequest::settled())
        .expect("positive exclusive ceiling");
    for source in [
        PAIR.replace("0..=capacity", "1..=capacity"),
        PAIR.replace("0..=ceiling", "0..ceiling"),
        PAIR.replace("position < bound", "position <= bound"),
        PAIR.replace(
            "false -> position",
            "false -> self.first(bound, position, ceiling)",
        ),
    ] {
        assert!(
            lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).is_err(),
            "{source}"
        );
    }
}
