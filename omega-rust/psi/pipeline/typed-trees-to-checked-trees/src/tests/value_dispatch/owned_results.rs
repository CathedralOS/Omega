use super::check;

#[test]
fn fresh_match_results_compose_with_owned_destinations() {
    let declarations = "data Category { case Other; case Missing; }
        data Holder { category: Category; }
        machine accept(category: Category) -> Category { category }";
    let expression = "match code { 2 -> Category::Missing, _ -> Category::Other }";
    for body in [
        expression.to_owned(),
        format!("let category: Category = {expression}; category"),
        format!("accept({expression})"),
        format!("let holder: Holder = Holder {{ category: {expression} }}; holder.category"),
    ] {
        let source = format!("{declarations} machine classify(code: u64) -> Category {{ {body} }}");
        check(&source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
    }
    check(
        "data Category { case Other; case Missing; }
        data Holder { category: Category; }
        machine Holder::set(&mut self, code: u64) {
            self.category = match code { 2 -> Category::Missing, _ -> Category::Other };
        }",
    )
    .expect("selected fresh category replaces an affine field through ordinary assignment");
}

#[test]
fn a_fresh_match_result_is_still_moved_at_most_once() {
    let result = check(
        "data Category { case Other; case Missing; }
        machine accept(category: Category) {}
        machine classify(code: u64) {
            let category: Category = match code { 2 -> Category::Missing, _ -> Category::Other };
            accept(category);
            accept(category);
        }",
    );
    let errors = match result {
        Ok(_) => panic!("fresh construction does not grant copy multiplicity"),
        Err(errors) => errors,
    };
    assert!(
        errors.iter().any(|error| error.message.contains("moved")
            || error.message.contains("consumed")
            || error.message.contains("ownership")),
        "{errors:?}"
    );
}

#[test]
fn fresh_match_constructors_preserve_nested_input_custody_limits() {
    for body in [
        "Box { payload: payload }",
        "Box { payload: Payload { value: consume(payload) } }",
    ] {
        let source = format!(
            "data Payload {{ value: u64; }}
            data Box {{ payload: Payload; }}
            machine consume(payload: Payload) -> u64 {{ payload.value }}
            machine choose(flag: bool, payload: Payload) -> Box {{
                match flag {{ true -> {body}, false -> Box {{ payload: Payload {{ value: 0 }} }} }}
            }}"
        );
        let errors = match check(&source) {
            Ok(_) => panic!("fresh outer construction cannot hide an owned input: {source}"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("branch custody join")
                    || error.message.contains("branch-local transfer")),
            "{source}: {errors:?}"
        );
    }
}
