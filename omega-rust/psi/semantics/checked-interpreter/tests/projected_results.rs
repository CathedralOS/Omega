use checked_interpreter::evaluate_const_machine;

fn typed_program(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("projection tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("projection syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("projection symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("projection types")
}

fn evaluate(source: &str) -> Result<i64, String> {
    // Exercise the shared evaluator independently of Terminal admission. Full
    // checking must keep its result-realization fence until stored references
    // and their source loans have executable Terminal producers and consumers.
    evaluate_const_machine(&typed_program(source), "main")
}

#[test]
fn projected_record_result_preserves_referent_and_evaluates_calls_once() {
    reference_result("forward_outer(Outer { inner: input }, &mut forwarded).inner");
}

#[test]
fn projected_array_result_preserves_referent_and_evaluates_calls_once() {
    reference_result("forward_array([input], &mut forwarded)[0]");
}

#[test]
fn nested_array_result_preserves_referent_and_evaluates_calls_once() {
    reference_result("forward_grid([[input]], &mut forwarded)[0][0]");
}

fn reference_result(projection: &str) {
    let source = format!(
        "data View {{ body: &mut i32; }}
         data Outer {{ inner: View; }}
         machine forward_outer(value: Outer, calls: &mut i32 in Wrapping) -> Outer {{
             calls = calls + 1;
             value
         }}
         machine forward_array(value: [View; 1], calls: &mut i32 in Wrapping) -> [View; 1] {{
             calls = calls + 1;
             value
         }}
         machine forward_grid(value: [[View; 1]; 1], calls: &mut i32 in Wrapping) -> [[View; 1]; 1] {{
             calls = calls + 1;
             value
         }}
         machine select(value: View, calls: &mut i32 in Wrapping) -> &mut i32 {{
             calls = calls + 1;
             value.body
         }}
         machine write(value: &mut i32) {{ value = 7; }}
         machine main() -> i64 {{
             let mut source: i32 = 3;
             let mut forwarded: i32 in Wrapping = 0;
             let mut selected: i32 in Wrapping = 0;
             let input: View = View {{ body: &mut source }};
             let held: &mut i32 = select({projection}, &mut selected);
             write(held);
             transition source == 7 && forwarded == 1 && selected == 1 {{
                 true -> 7
                 false -> 0
             }}
         }}"
    );
    assert_eq!(evaluate(&source), Ok(7), "{projection}");
}

#[test]
fn projected_reference_result_survives_return_and_repeated_forwarding() {
    for projection in ["forward(input).body", "references([input])[0].body"] {
        let source = format!(
            "data View {{ body: &mut i32; }}
             machine forward(value: View) -> View {{ value }}
             machine references(value: [View; 1]) -> [View; 1] {{ value }}
             machine relay(value: &mut i32) -> &mut i32 {{ value }}
             machine write(value: &mut i32) {{ value = 7; }}
             machine main() -> i64 {{
                 let mut source: i32 = 3;
                 let input: View = View {{ body: &mut source }};
                 let held: &mut i32 = {projection};
                 write(relay(relay(held)));
                 transition source == 7 {{ true -> 7 false -> 0 }}
             }}"
        );
        assert_eq!(evaluate(&source), Ok(7), "{projection}");
    }
}

#[test]
fn projected_reference_field_in_scalar_position_observes_the_referent() {
    for result in ["forward(input).body", "forward_array([input])[0].body"] {
        let source = format!(
            "data View {{ body: &mut i32; }}
             machine forward(value: View) -> View {{ value }}
             machine forward_array(value: [View; 1]) -> [View; 1] {{ value }}
             machine main() -> i32 {{
                 let mut source: i32 = 7;
                 let input: View = View {{ body: &mut source }};
                 {result}
             }}"
        );
        assert_eq!(evaluate(&source), Ok(7), "{result}");
    }
}

#[test]
fn projected_recast_field_argument_observes_its_declared_layout() {
    let source = "data Pair { low: u16; high: u16; }
                  machine select(value: u16) -> u16 { value }
                  machine main() -> u16 {
                      let mut bytes: [u8; 4] = [7, 0, 9, 0];
                      let view: &mut Pair = &mut bytes[0] as &mut Pair;
                      select(view.low)
                  }";
    assert_eq!(evaluate(source), Ok(7));
}

#[test]
fn whole_recast_reference_argument_retains_its_view_and_original_backing() {
    let source = "data Pair { low: u16; high: u16; }
                  machine write(value: &mut Pair) { value.low = 9; }
                  machine read(value: &Pair) -> u16 { value.low }
                  machine main() -> u16 {
                      let mut bytes: [u8; 4] = [7, 0, 11, 0];
                      let view: &mut Pair = &mut bytes[0] as &mut Pair;
                      write(view);
                      let observed: u16 = read(view);
                      transition bytes[0] == 9 && bytes[2] == 11 && observed == 9 {
                          true -> 9 false -> 0
                      }
                  }";
    assert_eq!(evaluate(source), Ok(9));
}

#[test]
fn projected_arguments_evaluate_collection_selector_and_consumer_once_in_order() {
    for (collection, expected_trace) in [("values(&mut trace)", 123), ("stored", 23)] {
        let source = format!(
            "machine values(trace: &mut i32 in Wrapping) -> [i32; 2] {{
                 trace = trace * 10 + 1; [7, 9]
             }}
             machine index(trace: &mut i32 in Wrapping) -> u64 {{
                 trace = trace * 10 + 2; 0
             }}
             machine select(value: i32, trace: &mut i32 in Wrapping) -> i32 {{
                 trace = trace * 10 + 3; value
             }}
             machine main() -> i64 {{
                 let mut trace: i32 in Wrapping = 0;
                 let stored: [i32; 2] = [7, 9];
                 let result: i32 = select({collection}[index(&mut trace)], &mut trace);
                 transition result == 7 && trace == {expected_trace} {{ true -> 7 false -> 0 }}
             }}"
        );
        assert_eq!(evaluate(&source), Ok(7), "{collection}");
    }
}

#[test]
fn checked_projected_argument_evaluates_its_selector_once() {
    let source = "machine index(calls: &mut i32 in Wrapping) -> u64 [0..=1] {
                      calls = calls + 1; 0
                  }
                  machine select(value: i32) -> i32 { value }
                  machine main() -> i32 {
                      let mut calls: i32 in Wrapping = 0;
                      let values: [i32; 2] = [7, 9];
                      let result: i32 = select(values[index(&mut calls)]);
                      transition calls == 1 && result == 7 { true -> 7 false -> 0 }
                  }";
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let outcome = checked_interpreter::interpret_entry(&checked, "main", &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn projected_owned_copy_does_not_alias_its_source() {
    let source = "data Item { number: i32; }
                  machine forward(value: [Item; 1]) -> [Item; 1] { value }
                  machine main() -> i64 {
                      let values: [Item; 1] = [Item { number: 7 }];
                      let mut selected: Item = forward(values)[0];
                      selected.number = 9;
                      transition values[0].number == 7 && selected.number == 9 {
                          true -> 7 false -> 0
                      }
                  }";
    assert_eq!(evaluate(source), Ok(7));
}

#[test]
fn projected_result_cannot_inherit_authored_indexing_as_builtin() {
    for (declaration, participates) in [
        ("", false),
        (
            "operator [] Indexing::index(items: &[u8; 2], index: u64) -> u8;",
            false,
        ),
        ("operator [] index(items: &[u8], index: u64) -> u8;", true),
    ] {
        let source = format!(
            "data Indexing {{}} {declaration}
             machine values() -> [u8; 2] {{ [7, 9] }}
             machine main() -> u8 {{ values()[0] }}"
        );
        let result = evaluate(&source);
        if participates {
            let error = result.expect_err("authored indexing needs its own execution custody");
            assert!(error.contains("builtin indexing meaning"), "{error}");
        } else {
            assert_eq!(result, Ok(7), "{declaration}");
        }
    }
}

#[test]
fn projected_result_checks_selector_bounds() {
    for (selector, expected) in [("2", "out of bounds"), ("-1", "out of range")] {
        let source = format!(
            "machine values() -> [i32; 2] {{ [7, 9] }}
             machine main() -> i32 {{ values()[{selector}] }}"
        );
        let error = evaluate(&source).expect_err("invalid selector must fail");
        assert!(error.contains(expected), "{selector}: {error}");
    }
}

#[test]
fn projected_collection_failure_precedes_selector_and_consumer() {
    let source = "machine values() -> [i32; 1] { [7 / 0] }
                  machine index() -> u64 { [0u64][2] }
                  machine consume(value: i32) -> i32 { [value][3] }
                  machine main() -> i32 { consume(values()[index()]) }";
    let error = evaluate(source).expect_err("collection must fail first");
    assert!(error.contains("division by zero"), "{error}");
}

#[test]
fn projected_selector_failure_precedes_consumer() {
    let source = "machine values() -> [i32; 1] { [7] }
                  machine index() -> u64 { 7 / 0 }
                  machine consume(value: i32) -> i32 { [value][3] }
                  machine main() -> i32 { consume(values()[index()]) }";
    let error = evaluate(source).expect_err("selector must fail before consumer");
    assert!(error.contains("division by zero"), "{error}");
}
