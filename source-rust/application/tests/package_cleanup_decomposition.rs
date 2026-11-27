//! A record declared `dropped by finish` can still be taken apart by any
//! other machine of its own package with a complete consuming pattern.
//! Here `release` takes one `Lease` apart and returns its mark, and the
//! other `Lease` dies at the end of `main`, where `finish` disposes of it.
//!
//! The checker used to refuse every such program with "selected declaration
//! lost its exact canonical package owner": it read the decomposing
//! machine's package from its hermetic semantic identity, which is not in
//! the type identity grammar. It now reads the machine's nominal identity.
//! The refusal of a machine from another package is tested beside that
//! check, in `terminal-semantics`'s `structural/nominal_cleanup_tests.rs`.
//!
//! `wiki/spec/language/ownership.md`, "Paths and control flow": a record
//! with a selected cleanup "cannot be partially moved, and only an
//! authorized complete consuming pattern takes it apart".

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn a_machine_of_the_records_package_takes_it_apart_in_native_code_and_psi() {
    let source = std::fs::read_to_string(support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/\
             package_decomposes_selected_cleanup_record/main.omg",
    ))
    .unwrap();
    program::assert_silent_return(&source);
}

#[test]
fn unmatched_owned_sum_disposes_only_active_case_in_native_code_and_psi() {
    let source = r#"
data Counter { value: u64; }
data Token { id: u64; }
data Lease<'view> dropped by release { counter: @'view mutable Counter; value: u64; }
data Choice<'view> {
    case A(left: Token, right: Lease<'view>);
    case B(left: Token, right: Token);
}
machine release(lease: Lease) {
    let { counter, value } = move lease;
    counter.value = value;
}
machine discard(x: Choice) {}
machine main() crashes Trap {
    let mutable counter_a: Counter = Counter { value: 10u64 };
    let choice_a: Choice = Choice::A {
        left: Token { id: 1u64 },
        right: Lease { counter: @mutable counter_a, value: 7u64 },
    };
    discard(choice_a);

    let mutable counter_b: Counter = Counter { value: 10u64 };
    let choice_b: Choice = Choice::B {
        left: Token { id: 1u64 },
        right: Token { id: 2u64 },
    };
    discard(choice_b);

    transition counter_a.value == 7u64 && counter_b.value == 10u64 {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;
    program::assert_silent_return(source);
}
