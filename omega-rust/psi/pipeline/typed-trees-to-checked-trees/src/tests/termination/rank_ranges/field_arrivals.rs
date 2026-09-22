use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_named_arrival/main.omg"
));

const COMPUTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_computed_arrival/main.omg"
));

const FRESH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_fresh_arrival/main.omg"
));

fn prove(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

#[test]
fn computed_record_arrival_needs_no_identity_forwarding_state() {
    prove(COMPUTED);
    prove(&COMPUTED.replace(
        "remaining: countdown.remaining - 1,\n            limit: countdown.limit",
        "limit: countdown.limit, remaining: countdown.remaining - 1",
    ));
    prove(
        &COMPUTED
            .replace("limit: u64 [0..=5];", "limit: u64 [0..=5]; enabled: bool;")
            .replace(
                "limit: countdown.limit",
                "limit: countdown.limit, enabled: true",
            )
            .replace(
                "limit: pending.limit",
                "limit: pending.limit, enabled: false",
            ),
    );
}

#[test]
fn computed_record_role_is_independent_of_auxiliary_step_inputs() {
    let source = COMPUTED
        .replace(
            "walk(countdown: Countdown)",
            "walk(countdown: Countdown, step: u64 [1..=1])",
        )
        .replace("countdown.remaining > 0", "countdown.remaining >= step")
        .replace("countdown.remaining - 1", "countdown.remaining - step");
    prove(&source);
    reject(&source.replace("countdown.remaining - step", "countdown.remaining + step"));
}

#[test]
fn computed_record_arrivals_still_prove_every_field_and_endpoint() {
    for source in [
        COMPUTED.replace("countdown.remaining - 1", "countdown.remaining + 1"),
        COMPUTED.replace("countdown.remaining - 1", "countdown.remaining - 2"),
        COMPUTED.replace("limit: countdown.limit", "limit: 5"),
        COMPUTED.replace("limit: countdown.limit", "limit: countdown.remaining"),
        COMPUTED.replace("pending.remaining - 1", "pending.remaining"),
        COMPUTED.replace("pending.remaining - 1", "pending.remaining + 1"),
        COMPUTED.replace("limit: pending.limit", "limit: 5"),
        COMPUTED.replace("requires countdown.remaining <= countdown.limit;", ""),
    ] {
        reject(&source);
    }
}

#[test]
fn computed_record_arrivals_reject_conflicting_roles_in_either_order() {
    let (declarations, _) = COMPUTED.split_once("machine walk").expect("declarations");
    for (first, second) in [("left", "right"), ("right", "left")] {
        let source = format!("{declarations}
            machine walk(left: Countdown, right: Countdown, choose: bool)
            requires left.remaining <= left.limit && right.remaining <= right.limit && left.limit == right.limit;
            terminates by left -> Countdown::Remaining in 0..=left.limit;
            -> u64 {{
                transition choose {{
                    true -> iterate(Countdown {{ remaining: {first}.remaining, limit: {first}.limit }})
                    false -> iterate(Countdown {{ remaining: {second}.remaining, limit: {second}.limit }})
                }}
                state iterate(pending: Countdown) {{
                    transition pending.remaining > 0 {{
                        true -> iterate(Countdown {{ remaining: pending.remaining - 1, limit: pending.limit }})
                        false -> pending.remaining
                    }}
                }}
            }}");
        reject(&source);
        prove(
            &source
                .replace("remaining: right.remaining", "remaining: left.remaining")
                .replace("limit: right.limit", "limit: left.limit"),
        );
    }
}

#[test]
fn computed_record_dependencies_do_not_authorize_effects_or_operator_meanings() {
    for declaration in [
        "operator - u64::custom(left: u64, right: u64) -> u64;",
        "operator > u64::custom(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {COMPUTED}"));
    }
    reject(&COMPUTED.replace(
        "transition countdown.remaining",
        "countdown.remaining = 5; transition countdown.remaining",
    ));
    let effectful = COMPUTED.replace(
        "limit: countdown.limit",
        "limit: reset(&mut countdown.limit)",
    );
    reject(&format!(
        "machine reset(value: &mut u64) -> u64 {{ value = 5; 5 }} {effectful}"
    ));
}

fn reject(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
}

#[test]
fn fresh_record_arrival_carries_the_ranked_role() {
    prove(FRESH);
    // The role follows the unique owner-typed formal, not its spelling.
    prove(&FRESH.replace("pending", "delivered"));
    prove(&FRESH.replace("remaining: 3", "remaining: 0"));
    // The fresh literal still descends when its edge closes a cycle: the
    // guard proves `3 < countdown.remaining`, and the backedge rebuilds the
    // record through the ordinary computed-arrival path.
    let cyclic = FRESH.replace(
        "true -> iterate(Countdown {\n                remaining: pending.remaining - 1,\n                limit: pending.limit\n            })",
        "true -> walk(Countdown { remaining: pending.remaining - 1, limit: pending.limit })",
    );
    assert_ne!(cyclic, FRESH);
    prove(&cyclic);
}

#[test]
fn fresh_record_arrival_still_owes_membership_pinning_and_descent() {
    for source in [
        // A fresh field value must still land inside the declared rank range.
        FRESH.replace("remaining: 3", "remaining: 6"),
        // A dynamic endpoint cannot be pinned by a literal that never
        // forwards it.
        FRESH
            .replace(
                "terminates by",
                "requires countdown.remaining <= countdown.limit;\nterminates by",
            )
            .replace("in 0..=5", "in 0..=countdown.limit"),
        // Inside a cycle the constant reset is not a decrease.
        FRESH.replace(
            "remaining: pending.remaining - 1",
            "remaining: pending.limit",
        ),
        // Forwarding a different record does not establish the ranked field.
        FRESH
            .replace(
                "machine walk(countdown: Countdown)",
                "machine walk(countdown: Countdown, spare: Countdown)",
            )
            .replace(
                "iterate(Countdown { remaining: 3, limit: 5 })",
                "iterate(spare)",
            ),
    ] {
        reject(&source);
    }
}

#[test]
fn fresh_record_arrival_needs_a_unique_owner_typed_slot() {
    let (declarations, _) = FRESH.split_once("machine walk").expect("declarations");
    reject(&format!(
        "{declarations}
        machine walk(countdown: Countdown)
        terminates by countdown -> Countdown::Remaining in 0..=5;
        -> u64 {{
            transition countdown.remaining > 3 {{
                true -> iterate(Countdown {{ remaining: 3, limit: 5 }}, Countdown {{ remaining: 2, limit: 5 }})
                false -> countdown.remaining
            }}
            state iterate(first: Countdown, second: Countdown) {{
                transition first.remaining > 0 {{
                    true -> iterate(Countdown {{ remaining: first.remaining - 1, limit: first.limit }}, second)
                    false -> first.remaining
                }}
            }}
        }}"
    ));
    // A borrowed record is not an owned record slot the view can rank.
    reject(&FRESH.replace("pending: Countdown", "pending: &Countdown"));
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

/// Duplicated record carriers: an affine record reaches two slots only under
/// borrows, and the strict step is a rebuilt literal under a borrow.
const BORROWED: &str = r#"
data Card { power: u64; }
measure Card::Remaining(card: Card) -> u64 { card.power }

machine walk(card: Card, ceiling: u64)
requires card.power <= ceiling;
terminates by card -> Card::Remaining in 0..=ceiling;
-> u64 {
    transition card.power > 0 {
        true -> pair(&card, &card, ceiling)
        false -> 0
    }
    state pair(live: &Card, saved: &Card, bound: u64) {
        transition live.power > 0 {
            true -> pair(&Card { power: live.power - 1 }, saved, bound)
            false -> 0
        }
    }
}
"#;

#[test]
fn record_arrivals_need_unique_roles_and_exact_nominal_owners() {
    for source in [
        COUNTDOWN.replace("pending: Countdown)", "pending: Other)"),
        COUNTDOWN.replace("iterate(limit, Countdown", "iterate(limit, Other"),
    ] {
        reject(&format!(
            "data Other {{ remaining: u64 [0..=5]; }} {source}"
        ));
    }
    reject(&COUNTDOWN.replace("pending: Countdown", "pending: &Countdown"));
}

#[test]
fn duplicated_record_copies_name_the_rebuilt_step_as_continuation() {
    // Two borrowed forwards of the affine record share one role until a
    // strict step names the moved copy: the rebuilt literal landing in
    // `live` is the continuation the rank reads, so the stale `saved`
    // snapshot demotes.
    prove(BORROWED);
    // Forwarding the moved copy itself keeps the continuation honest:
    // `saved` receives `live`'s pre-step record and still demotes.
    prove(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: live.power - 1 }, live, bound)",
    ));
    // A literal that does not step is not divergence evidence: `live`
    // demotes to the bare `saved` forward, which still denotes the entry's
    // stale value -- the rank never decreases.
    reject(&BORROWED.replace("power: live.power - 1", "power: live.power"));
    // Two moved copies leave the continuation ambiguous: neither literal is
    // the unique step, so no slot names the record the view reads.
    reject(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: live.power - 1 }, &Card { power: saved.power - 1 }, bound)",
    ));
    // Stepping the demoted copy is not divergence evidence either: `saved`'s
    // atom is free once the role names `live`, so a literal built from it
    // cannot prove descent or membership.
    reject(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: saved.power - 1 }, saved, bound)",
    ));
    // Reading the demoted stale copy as the rank's carrier is still
    // rejected: `rest` receives `saved`'s pre-step snapshot while the role
    // named `live`, so `rest`'s record has no rank coordinate at all.
    reject(&BORROWED
        .replace(
            "false -> 0\n        }\n    }\n}",
            "false -> finish(saved)\n        }\n    }\n    state finish(rest: &Card) {\n        transition rest.power > 0 {\n            true -> finish(&Card { power: rest.power - 1 })\n            false -> 0\n        }\n    }\n}",
        ));
}

#[test]
fn a_computed_claimant_leaves_the_record_role_to_its_bare_forward() {
    // `countdown.remaining` computes a scalar from the ranked record's entry:
    // its claim contests the role the bare `countdown` forward already
    // carries, so the computed claimant demotes and `pending` stays the one
    // record the field view can read.
    let source = COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, countdown, countdown.remaining)",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, echo: u64 [0..=5])",
        )
        .replace("pending.remaining - 1 })", "pending.remaining - 1 }, echo)");
    prove(&source);
    // With no bare forward at all, every computed claimant demotes at that
    // arrival -- but the literal still lands in the destination's one
    // `Countdown` formal, so `fresh_record_carrier` keeps `pending` the
    // record slot the view reads and the stepped rebuild proves out.
    prove(&COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, Countdown { remaining: countdown.remaining }, countdown.remaining)",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, echo: u64 [0..=5])",
        )
        .replace(
            "pending.remaining - 1 })",
            "pending.remaining - 1 }, echo)",
        ));
    // Two record-typed slots make the fresh-carrier claim ambiguous, so the
    // duplicated literal arrivals leave no slot naming the ranked record.
    reject(&COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, Countdown { remaining: countdown.remaining }, Countdown { remaining: countdown.remaining })",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, spare: Countdown)",
        )
        .replace(
            "pending.remaining - 1 })",
            "pending.remaining - 1 }, spare)",
        ));
}

#[test]
fn mutable_record_parameters_prove_only_while_the_prefix_preserves_them() {
    // The same preserved-prefix evidence that admits mutable integer inputs
    // carries a mutable record's arrival fields: a write into its path before
    // the transition invalidates the premise, a disjoint local write does not.
    let mutable_subject =
        COUNTDOWN.replace("walk(countdown: Countdown", "walk(mut countdown: Countdown");
    prove(&mutable_subject);
    prove(&mutable_subject.replace(
        "    transition {",
        "    let mut scratch: u64 = 0; scratch = 5;\n    transition {",
    ));
    for statement in [
        "countdown.remaining = 5;",
        "countdown = Countdown { remaining: 5 };",
    ] {
        reject(&mutable_subject.replace(
            "    transition {",
            &format!("    {statement}\n    transition {{"),
        ));
    }
    // A mutable arrival parameter carries the field coordinate through
    // `at_arrival` the same way. The recursive arm rebuilds the record from a
    // literal so its constructor obligation needs no fact about the mutable
    // place: constructor guard facts still require immutable inputs until
    // that adapter consumes write frames.
    let mutable_arrival = COUNTDOWN
        .replace("pending: Countdown", "mut pending: Countdown")
        .replace(
            "Countdown { remaining: pending.remaining - 1 }",
            "Countdown { remaining: 0 }",
        );
    prove(&mutable_arrival);
    prove(&mutable_arrival.replace(
        "transition pending.remaining",
        "let mut scratch: u64 = 0; scratch = 5; transition pending.remaining",
    ));
    for statement in [
        "pending.remaining = 5;",
        "pending = Countdown { remaining: 5 };",
    ] {
        reject(&mutable_arrival.replace(
            "transition pending.remaining",
            &format!("{statement} transition pending.remaining"),
        ));
    }
    // A store into a different mutable input still cannot reach the edge:
    // the preserved prefix must spare every parameter path, not only the
    // ranked one.
    let mutable_bound = mutable_arrival.replace("limit: u64 [5..=10]", "mut limit: u64 [5..=10]");
    reject(&mutable_bound.replace(
        "transition pending.remaining",
        "limit = 10; transition pending.remaining",
    ));
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

    let program = typed_program(&format!(
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
