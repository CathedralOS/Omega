use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_named_arrival/main.omg"
));

fn prove(source: &str) {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
}

#[test]
fn field_rank_follows_renamed_and_reordered_state_arrivals() {
    prove(COUNTDOWN);
    prove(&COUNTDOWN.replace("pending", "renamed"));
    prove(&COUNTDOWN
        .replace("iterate(ceiling, countdown)", "relay(countdown, ceiling)")
        .replace("    state iterate", "    state relay(value: Countdown, bound: u64 [5..=10]) { transition { _ -> iterate(bound, value) } }\n    state iterate"));
    prove(&COUNTDOWN.replace("in 0..=ceiling", "in 0..=5"));
    prove(
        &COUNTDOWN
            .replace("[5..=10]", "[6..=10]")
            .replace("in 0..=ceiling", "in 0..ceiling"),
    );
}

#[test]
fn field_arrival_still_owes_descent_range_and_fixed_endpoints() {
    for source in [
        COUNTDOWN.replace("pending.remaining - 1", "pending.remaining"),
        COUNTDOWN.replace("pending.remaining - 1", "pending.remaining + 1"),
        COUNTDOWN.replace("pending.remaining - 1", "pending.remaining - 2"),
        COUNTDOWN.replace("in 0..=ceiling", "in 1..=ceiling"),
        COUNTDOWN.replace("in 0..=ceiling", "in 0..=4"),
        COUNTDOWN.replace("iterate(limit, Countdown", "iterate(5, Countdown"),
        COUNTDOWN.replace("iterate(limit, Countdown", "iterate(limit + 1, Countdown"),
        COUNTDOWN.replace("iterate(ceiling, countdown)", "iterate(5, countdown)"),
    ] {
        reject(&source);
    }
}

#[test]
fn field_arrival_preserves_live_storage_and_selected_meaning() {
    for statement in ["pending.remaining = 5;", "limit = 10;"] {
        reject(&COUNTDOWN.replace(
            "transition pending.remaining",
            &format!("{statement} transition pending.remaining"),
        ));
    }
    prove(&COUNTDOWN.replace(
        "transition pending.remaining",
        "let mut scratch: u64 = 0; scratch = 5; transition pending.remaining",
    ));
    for declaration in [
        "operator - u64::custom(left: u64, right: u64) -> u64;",
        "operator > u64::custom(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {COUNTDOWN}"));
    }
}

#[test]
fn named_field_endpoints_use_simultaneous_exact_record_substitution() {
    let source = COUNTDOWN
        .replace(
            "remaining: u64 [0..=5];",
            "remaining: u64 [0..=5]; bound: u64 [0..=5];",
        )
        .replace(
            "terminates by",
            "requires countdown.remaining <= countdown.bound;\nterminates by",
        )
        .replace("in 0..=ceiling", "in 0..=countdown.bound")
        .replace(
            "remaining: pending.remaining - 1",
            "remaining: pending.remaining - 1, bound: pending.bound",
        );
    prove(&source);
    prove(&source.replace("bound: pending.bound", "bound: pending.bound + 0"));
    for endpoint in ["5", "pending.remaining", "pending.bound - 1"] {
        reject(&source.replace("bound: pending.bound", &format!("bound: {endpoint}")));
    }
    reject(&source.replace("requires countdown.remaining <= countdown.bound;", ""));
}

#[test]
fn record_arrivals_need_unique_roles_and_exact_nominal_owners() {
    let duplicated = COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, countdown, countdown)",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, spare: Countdown)",
        )
        .replace(
            "pending.remaining - 1 })",
            "pending.remaining - 1 }, spare)",
        );
    reject(&duplicated);
    for source in [
        COUNTDOWN.replace("pending: Countdown)", "pending: Other)"),
        COUNTDOWN.replace("iterate(limit, Countdown", "iterate(limit, Other"),
    ] {
        reject(&format!(
            "data Other {{ remaining: u64 [0..=5]; }} {source}"
        ));
    }
    for parameter in ["pending: &Countdown", "mut pending: Countdown"] {
        reject(&COUNTDOWN.replace("pending: Countdown", parameter));
    }
}

#[test]
fn separate_endpoint_records_keep_their_own_arrival_roles() {
    let source = format!(
        "data Bounds {{ value: u64 [5..=10]; }} {}",
        COUNTDOWN
            .replace("ceiling: u64 [5..=10]", "bounds: Bounds")
            .replace("in 0..=ceiling", "in 0..=bounds.value")
            .replace("iterate(ceiling, countdown)", "iterate(bounds, countdown)")
            .replace("limit: u64 [5..=10]", "limits: Bounds")
            .replace("iterate(limit, Countdown", "iterate(limits, Countdown")
    );
    prove(&source);
    reject(&source.replace(
        "iterate(limits, Countdown",
        "iterate(Bounds { value: 5 }, Countdown",
    ));
    reject(&source.replace("iterate(limits, Countdown", "iterate(pending, Countdown"));
    reject(
        &source
            .replace("iterate(bounds, countdown)", "iterate(countdown)")
            .replace("limits: Bounds, ", "")
            .replace("iterate(limits, Countdown", "iterate(Countdown"),
    );
}

#[test]
fn field_arrival_proof_does_not_accept_foreign_same_spelled_handles() {
    use typed_trees::data::DataMember;
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let program = typed(&format!(
        "{COUNTDOWN} data Other {{ remaining: u64 [0..=5]; }}"
    ));
    crate::checks::termination::check_machine_termination(&program).expect("valid arrival");
    let subtraction_subject = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Subtract => {
                Some(binary.left)
            }
            _ => None,
        })
        .expect("decrement subject");
    let foreign_field = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .next_back()
        .expect("other field");
    let root = &program.machine_states(&program.machines()[0])[0];
    let foreign_parameter = program.state_parameters(root)[0].symbol;
    for replace_receiver in [false, true] {
        let mut changed = program.clone();
        let ExpressionNode::Member(member) =
            changed.expression_table.expression_mut(subtraction_subject)
        else {
            panic!("field read")
        };
        if replace_receiver {
            let receiver = member.receiver;
            let ExpressionNode::Name(path) = changed.expression_table.expression_mut(receiver)
            else {
                panic!("parameter receiver")
            };
            path.symbol = foreign_parameter;
            path.head_symbol = foreign_parameter;
        } else {
            assert_ne!(member.member_symbol, foreign_field);
            member.member_symbol = foreign_field;
        }
        crate::checks::termination::check_machine_termination(&changed)
            .expect_err("spelling does not establish field or receiver identity");
    }
}
