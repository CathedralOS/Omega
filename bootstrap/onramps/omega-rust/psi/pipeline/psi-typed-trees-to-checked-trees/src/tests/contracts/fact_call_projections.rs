use super::*;

fn checked(
    source: &str,
) -> Result<psi_checked_trees::CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lower_typed_trees(parse_typed_trees(source))
}

fn messages(source: &str) -> String {
    checked(source)
        .expect_err("program should reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

const RECORD: &str = r#"
    data Pair [copy] { left: u64; right: u64; }

    machine make_pair(left: u64, right: u64) -> Pair terminates; {
        transition { _ -> (Pair { left: left, right: right }) }
    }
"#;

#[test]
fn direct_pure_total_record_projection_is_retained_without_a_runtime_call() {
    let source = format!(
        r#"
        {RECORD}
        machine law(left: u64, right: u64) -> u64
        ensures make_pair(left, right).left == make_pair(left, right).left
        {{
            transition {{ _ -> left }}
        }}
    "#
    );
    let checked = checked(&source).expect("bounded fact-call projection should check");
    assert_eq!(checked.facts.fact_call_projections.len(), 2);
    assert!(
        checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .all(|(_, call)| { checked.symbols.name(call.target_symbol) != "make_pair" })
    );
}

#[test]
fn bodyless_projection_rejects() {
    let source = format!(
        r#"
        {RECORD}
        boundary machine foreign_pair(left: u64, right: u64) -> Pair;
        machine law(left: u64, right: u64) -> u64
        requires foreign_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    assert!(messages(&source).contains("bodyless, boundary, accepted, and external"));
}

#[test]
fn reaching_projection_rejects() {
    let source = format!(
        r#"
        {RECORD}
        machine reaching_pair(left: u64, right: u64) -> Pair
        reaches PortIo
        terminates;
        {{ transition {{ _ -> (Pair {{ left: left, right: right }}) }} }}
        machine law(left: u64, right: u64) -> u64
        requires reaching_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    let diagnostics = messages(&source);
    assert!(
        diagnostics.contains("observes or mutates hidden state"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn preconditioned_projection_rejects() {
    let source = format!(
        r#"
        {RECORD}
        machine guarded_pair(left: u64, right: u64) -> Pair
        requires left == right
        terminates;
        {{ transition {{ _ -> (Pair {{ left: left, right: right }}) }} }}
        machine law(left: u64, right: u64) -> u64
        requires guarded_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    assert!(messages(&source).contains("has preconditions"));
}

#[test]
fn sum_projection_rejects() {
    let source = r#"
        data Choice { case One(value: u64); case Two(value: u64); }
        machine choose(value: u64) -> Choice terminates; {
            transition { _ -> (Choice::One { value: value }) }
        }
        machine law(value: u64) -> u64
        requires choose(value).value == value
        { transition { _ -> value } }
    "#;
    assert!(messages(source).contains("sum, mixed, empty, and opaque"));
}

#[test]
fn qualified_content_projection_rejects() {
    let source = format!(
        r#"
        {RECORD}
        domain Pair::Ready;
        machine qualified_pair(left: u64, right: u64) -> Pair in Ready terminates; {{
            transition {{ _ -> (Pair {{ left: left, right: right }}) }}
        }}
        machine law(left: u64, right: u64) -> u64
        requires qualified_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    assert!(messages(&source).contains("content-bearing or otherwise qualified"));
}

#[test]
fn nested_projection_rejects() {
    let source = r#"
        data Inner [copy] { value: u64; }
        data Outer [copy] { inner: Inner; }
        machine make_outer(value: u64) -> Outer terminates; {
            transition { _ -> (Outer { inner: Inner { value: value } }) }
        }
        machine law(value: u64) -> u64
        requires make_outer(value).inner.value == value
        { transition { _ -> value } }
    "#;
    assert!(messages(source).contains("nested or adapted projection"));
}

#[test]
fn crashing_projection_rejects() {
    let source = format!(
        r#"
        {RECORD}
        machine crashing_pair(left: u64, right: u64) -> Pair
        crashes Abort
        terminates;
        {{
            crash Abort;
        }}
        machine law(left: u64, right: u64) -> u64
        requires crashing_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    assert!(messages(&source).contains("has a crash route"));
}

#[test]
fn cyclic_projection_never_acquires_a_totality_certificate() {
    let source = format!(
        r#"
        {RECORD}
        machine looping_pair(left: u64, right: u64) -> Pair {{
            transition {{ _ -> looping_pair(left, right) }}
        }}
        machine law(left: u64, right: u64) -> u64
        requires looping_pair(left, right).left == left
        {{ transition {{ _ -> left }} }}
    "#
    );
    let diagnostics = messages(&source);
    assert!(
        diagnostics.contains("not unconditionally terminating")
            || diagnostics.contains("call cycle")
            || diagnostics.contains("recursive")
    );
}

#[test]
fn domain_projection_dependencies_are_the_call_argument_occurrences() {
    let source = format!(
        r#"
        {RECORD}
        data Input [copy] {{ left: u64; right: u64; }}
        domain Input::Stable
        requires make_pair(self.left, self.right).left == self.left;
    "#
    );
    let checked = checked(&source).expect("domain fact-call projection should check");
    let record = checked
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .map(|(_, record)| record)
        .next()
        .expect("stable domain definition fact");
    let dependencies = record
        .dependencies
        .iter()
        .map(|dependency| checked.expression_table.display_name(dependency.expression))
        .collect::<Vec<_>>();
    assert!(dependencies.iter().any(|name| name == "self.left"));
    assert!(dependencies.iter().any(|name| name == "self.right"));
    assert!(
        dependencies
            .iter()
            .all(|name| !name.starts_with("make_pair(")),
        "the denotational result must not become a synthetic place: {dependencies:?}"
    );
}

#[test]
fn transparent_named_fact_can_be_proved_from_an_exact_call_projection() {
    let source = format!(
        r#"
        {RECORD}
        proposition projected_left(left: u64, right: u64) =
            make_pair(left, right).left == left;
        machine theorem(left: u64, right: u64) -> u64
        ensures projected_left(left, right)
        {{ transition {{ _ -> left }} }}
    "#
    );
    checked(&source).expect("transparent named fact should use structural call evidence");
}

#[test]
fn transparent_named_fact_does_not_certify_a_false_call_projection() {
    let source = format!(
        r#"
        {RECORD}
        proposition projected_left_is_right(left: u64, right: u64) =
            make_pair(left, right).left == right;
        machine bogus(left: u64, right: u64) -> u64
        ensures projected_left_is_right(left, right)
        {{ transition {{ _ -> left }} }}
    "#
    );
    assert!(messages(&source).contains("cannot establish proposition ensure"));
}

#[test]
fn transparent_relation_retains_exact_projected_call_arguments() {
    let source = format!(
        r#"
        {RECORD}
        proposition same_left(left: Pair, right: Pair) = left.left == right.left;
        machine theorem(value: u64, b: u64, c: u64) -> u64
        ensures same_left(make_pair(value, b), make_pair(value, c))
        {{ transition {{ _ -> value }} }}
    "#
    );
    let checked = checked(&source).expect("relation over exact call results should check");
    assert_eq!(checked.facts.fact_call_projections.len(), 2);
}

#[test]
fn transparent_relation_call_arguments_retain_contract_revision_dependencies() {
    let source = format!(
        r#"
        {RECORD}
        data Input [copy] {{ left: u64; right: u64; }}
        proposition same_left(left: Pair, right: Pair) = left.left == right.left;
        machine theorem(input: &Input) -> u64
        requires same_left(
            make_pair(input.left, input.right),
            make_pair(input.left, input.right)
        )
        {{ transition {{ _ -> input.left }} }}
    "#
    );
    let checked = checked(&source).expect("named fact call arguments should retain dependencies");
    let theorem = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "theorem")
        .expect("theorem machine");
    let state = checked
        .machine_states(theorem)
        .first()
        .expect("theorem entry");
    let input = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "input")
        .expect("input parameter");
    let context = checked
        .facts
        .semantic
        .contexts_at_point(psi_facts::ProgramPoint::Machine {
            machine_symbol: theorem.symbol,
        })
        .next()
        .expect("machine requires context");
    let fields = context
        .facts()
        .filter_map(|fact| {
            let psi_facts::FactPlace::Place(place) = fact.place else {
                return None;
            };
            let place = checked.facts.semantic.places.get(place);
            (place.root == psi_facts::PlaceRoot::Symbol(input.symbol)).then_some(
                checked
                    .facts
                    .semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .iter()
                    .filter_map(|segment| match segment {
                        psi_facts::PlaceSegment::Field { symbol } => {
                            Some(checked.symbols.name(*symbol).to_owned())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect::<Vec<_>>();
    assert!(fields.iter().any(|field| field == "left"));
    assert!(fields.iter().any(|field| field == "right"));
}

#[test]
fn transparent_relation_cannot_hide_an_ineligible_call_projection() {
    let source = format!(
        r#"
        {RECORD}
        boundary machine foreign_pair(left: u64, right: u64) -> Pair;
        proposition same_left(left: Pair, right: Pair) = left.left == right.left;
        machine theorem(value: u64, other: u64) -> u64
        ensures same_left(foreign_pair(value, other), make_pair(value, other))
        {{ transition {{ _ -> value }} }}
    "#
    );
    assert!(messages(&source).contains("bodyless, boundary, accepted, and external"));
}
