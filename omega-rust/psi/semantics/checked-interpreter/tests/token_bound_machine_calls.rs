//! A token-bearing machine (`machine + Wrapped::add`) executes through its own
//! checked body: the checked stage binds the selected `+` use to an ordinary
//! call, so the interpreter runs the declaration like any named call.

use checked_interpreter::InterpretOptions;

fn interpret_main(source: &str) -> checked_interpreter::InterpretOutcome {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("types");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    checked_interpreter::interpret_entry(&checked, "main", &[], InterpretOptions::default())
}

#[test]
fn token_call_runs_the_declaration_body_and_returns_the_wrapped_sum() {
    let outcome = interpret_main(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
             (left.value as u64) + (right.value as u64)
         }
         machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }
         machine main() -> i32 {
             let left: Wrapped = Wrapped { value: 250u8 };
             let right: Wrapped = Wrapped { value: 10u8 };
             transition by_token(left, right) == 260u64 { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn token_and_named_calls_share_one_body() {
    let outcome = interpret_main(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
             (left.value as u64) + (right.value as u64)
         }
         machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }
         machine by_name(left: Wrapped, right: Wrapped) -> u64 { Wrapped::add(left, right) }
         machine main() -> i32 {
             let a: Wrapped = Wrapped { value: 3u8 };
             let b: Wrapped = Wrapped { value: 4u8 };
             let c: Wrapped = Wrapped { value: 3u8 };
             let d: Wrapped = Wrapped { value: 4u8 };
             transition by_token(a, b) == by_name(c, d) { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn borrowed_operand_token_call_runs_the_declaration_body() {
    let outcome = interpret_main(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: &Wrapped, right: &Wrapped) -> u64 {
             (left.value as u64) + (right.value as u64)
         }
         machine by_token(left: &Wrapped, right: &Wrapped) -> u64 { left + right }
         machine main() -> i32 {
             let left: Wrapped = Wrapped { value: 250u8 };
             let right: Wrapped = Wrapped { value: 10u8 };
             transition by_token(&left, &right) == 260u64 { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn domain_homed_token_call_runs_the_declaration_body() {
    // `machine + Quantity::Additive::add` is a domain-family meaning: the
    // caller's `requires` selects the domain, so `left + right` runs the
    // declaration's own body (5 + 7 = 12) through the ordinary call edge.
    let outcome = interpret_main(
        "data Quantity { value: i32; }
         domain Quantity::Additive requires self.value >= 0;
         machine + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity {
             Quantity { value: ((left.value as i32 in Wrapping) + (right.value as i32 in Wrapping)) as i32 }
         }
         machine combine(left: Quantity, right: Quantity) -> Quantity
         requires left in Quantity::Additive
         { left + right }
         machine main() -> i32 {
             let left: Quantity = Quantity { value: 5 };
             let right: Quantity = Quantity { value: 7 };
             let sum: Quantity = combine(left, right);
             transition sum.value == 12 { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
