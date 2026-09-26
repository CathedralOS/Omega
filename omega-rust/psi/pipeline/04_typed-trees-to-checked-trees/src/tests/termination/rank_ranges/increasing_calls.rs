use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const PAIR: &str = r#"
data Main { observed: u64; }
machine Main::first(&mut self, limit: u64, cursor: u64, capacity: u64)
requires cursor <= limit && limit <= capacity;
terminates by cursor -> Nat::IncreasingTo(limit) in 0..=capacity;
-> u64 {
    transition cursor < limit {
        true -> self.second(capacity, cursor, limit)
        false -> cursor
    }
}
machine Main::second(&mut self, ceiling: u64, position: u64, bound: u64)
requires position <= bound && bound <= ceiling;
terminates by position -> Nat::IncreasingTo(bound) in 0..=ceiling;
-> u64 {
    transition position < bound {
        true -> self.first(bound, position + 1, ceiling)
        false -> position
    }
}
"#;

fn prove(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics =
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).expect_err(source);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("machine call cycle")
                || diagnostic.message.contains("cannot prove rank range")
        ),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn mixed_increasing_call_ranges_keep_range_endpoints_and_view_limits_pinned() {
    for omitted in [" in 0..=capacity", " in 0..=ceiling"] {
        let source = PAIR.replace(omitted, "");
        prove(&source);
        reject(&source.replace(
            "self.second(capacity, cursor, limit)",
            "self.second(capacity + 1, cursor, limit)",
        ));
        reject(&source.replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound, position + 1, ceiling + 1)",
        ));
        reject(&source.replace(
            "self.first(bound, position + 1, ceiling)",
            "self.first(bound - 1, position, ceiling)",
        ));
        reject(&source.replace("position + 1", "position"));
    }
}
