use super::{prove, reject};
use crate::tests::front_end::typed_program;

const ACYCLIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/computed_rank_copies/main.omg"
));

const CYCLIC: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> prepare(remaining, remaining) }
    state prepare(left: u32 [0..=5], right: u32 [0..=5]) {
        transition left > 0 {
            true -> finish(left + (right - right) - 1)
            false -> left
        }
    }
    state finish(result: u32 [0..=5]) {
        transition result > 0 {
            true -> prepare(result - 1, result - 1)
            false -> result
        }
    }
}
"#;

#[test]
fn computed_rank_copies_do_not_depend_on_operand_order() {
    for expression in ["left + (right - right)", "right + (left - left)"] {
        prove(&ACYCLIC.replace("left + (right - right)", expression));
    }
    prove(
        &ACYCLIC
            .replace(
                "prepare(remaining, remaining)",
                "prepare(remaining, remaining, remaining)",
            )
            .replace(
                "right: u32 [0..=5]",
                "right: u32 [0..=5], extra: u32 [0..=5]",
            )
            .replace(
                "left + (right - right)",
                "left + (right - right) + (extra - extra)",
            ),
    );
}

#[test]
fn computed_rank_copies_reject_unequal_initial_arrivals_even_with_valid_ranges() {
    // Both actuals fit 0..=5. Only their required equality is missing.
    let source = ACYCLIC.replace("remaining: u32 [0..=5]", "remaining: u32 [1..=4]");
    // A `carrier +/- positive` arrival names the moved copy as the ranked
    // carrier; the stale sibling demotes. `finish(left + (right - right))`
    // still reads the named copy's atom through cancellation, so the arrival
    // order decides which slot's claim survives.
    for actuals in ["remaining + 1, remaining", "remaining - 1, remaining"] {
        prove(&source.replace(
            "prepare(remaining, remaining)",
            &format!("prepare({actuals})"),
        ));
    }
    for actuals in ["remaining, remaining + 1", "remaining, remaining - 1"] {
        // Here `finish` reads the demoted stale copy's atom: `left + (right -
        // right)` substitutes a free `left` for the role `right` carries, so
        // the produced rank has no bound.
        reject(&source.replace(
            "prepare(remaining, remaining)",
            &format!("prepare({actuals})"),
        ));
    }
}

#[test]
fn computed_rank_copies_reestablish_equality_and_descent_on_every_cycle_edge() {
    prove(CYCLIC);
    prove(
        &CYCLIC
            .replace("left + (right - right)", "right + (left - left)")
            .replace("transition left > 0", "transition right > 0"),
    );
    // `result - 1` names `left` as the moved copy of `remaining`: the stale
    // `right` demotes and `left` keeps descending through the loop.
    prove(&CYCLIC.replace(
        "prepare(result - 1, result - 1)",
        "prepare(result - 1, result)",
    ));
    // Stepping the other copy names `right` instead; `finish` then reads the
    // demoted `left`, whose atom no hypothesis bounds.
    reject(&CYCLIC.replace(
        "prepare(result - 1, result - 1)",
        "prepare(result, result - 1)",
    ));
    // Equality and range membership cannot replace strict cyclic decrease.
    reject(&CYCLIC.replace(
        "finish(left + (right - right) - 1)",
        "finish(left + (right - right))",
    ));
    reject(&CYCLIC.replace("prepare(result - 1, result - 1)", "prepare(result, result)"));
}

#[test]
fn computed_rank_copies_admit_undisturbed_mutable_copies_and_checked_operator_meaning() {
    // An undisturbed mutable copy still denotes its arrival value at the
    // edge; the preserved prefix keeps every parameter path untouched.
    prove(&ACYCLIC.replace("right: u32", "mut right: u32"));
    // A prefix store into the copy's path invalidates the arrival premise.
    reject(&ACYCLIC.replace("right: u32", "mut right: u32").replace(
        "transition { _ -> finish",
        "right = 0;\n        transition { _ -> finish",
    ));
    reject(&format!(
        "operator + u32::custom(left: u32, right: u32) -> u32; {ACYCLIC}"
    ));
}

#[test]
fn computed_rank_arrival_uses_established_copy_equality() {
    let source = ACYCLIC.replace(
        "transition { _ -> finish(left + (right - right)) }",
        "transition right >= left { true -> finish(left + (right - left)) false -> left }",
    );
    // This proof requires equality, unlike the cancellation controls above.
    // The independent store-bound consumer does not yet transport this private
    // rank invariant; complete source-check success is covered separately.
    crate::checks::termination::check_machine_termination(&typed_program(&source))
        .expect("established copy equality proves the computed rank arrival");
}

#[test]
fn a_valid_parallel_arrival_cannot_hide_changed_rank_copies() {
    let source = ACYCLIC.replace(
        "remaining: u32 [0..=5]",
        "remaining: u32 [1..=4], enabled: bool",
    );
    for (first, second) in [
        ("remaining, remaining", "remaining, remaining + 1"),
        ("remaining, remaining + 1", "remaining, remaining"),
    ] {
        reject(&source.replace(
            "transition { _ -> prepare(remaining, remaining) }",
            &format!(
                "transition enabled {{ true -> prepare({first}) false -> prepare({second}) }}"
            ),
        ));
    }
}

#[test]
fn contested_entry_roles_resolve_to_their_bare_forward() {
    // Both computed arrivals leave `echo` without an entry role: once its
    // contested `other` claim demotes to the bare `total` forward, the two
    // proposals agree instead of conflicting over which copy is the carrier.
    let source = r#"
machine walk(remaining: u32 [0..=5], other: u32 [0..=5], base: u32 [0..=5], step: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition remaining > 0 {
        true -> tally(remaining, other, other + 0)
        false -> tally(remaining, other, base + step)
    }
    state tally(count: u32 [0..=5], total: u32 [0..=5], echo: u32 [0..=10]) {
        transition count > 0 {
            true -> tally(count - 1, total, echo)
            false -> total
        }
    }
}
"#;
    prove(source);
    // Demoting the contested claim does not equate different carriers: a
    // sibling arrival that genuinely maps `total` to `base` still conflicts.
    reject(&source.replace(
        "false -> tally(remaining, other, base + step)",
        "false -> tally(remaining, base, base + step)",
    ));
}

#[test]
fn role_less_arrivals_abstain_instead_of_contesting_a_claimed_role() {
    // The entry arrival fills `first`'s spare slots with duplicated copies of
    // `spare`; the cyclic sibling arrival writes literals that name no
    // dependency. A role-less proposal abstains on those slots, so the two
    // proposals join instead of removing the state. The registered pass
    // canary `termination/rank_range_state_cycle` is this exact shape.
    let source = r#"
data Payload { value: u64; }
machine walk(remaining: u32 [0..=5], payload: Payload, spare: u32)
terminates by remaining in 0..=5;
-> u32 {
    transition remaining > 2 {
        true -> first(payload, remaining - 1, spare, spare)
        false -> second(remaining + 0, payload)
    }
    state first(carried: Payload, pending: u32 [0..=5], first_spare: u32, second_spare: u32) {
        transition pending > 0 {
            true -> second(pending - 1, carried)
            false -> pending
        }
    }
    state second(left: u32 [0..=5], saved: Payload) {
        transition left > 0 {
            true -> first(saved, left - 1, 0, 1)
            false -> left
        }
    }
}
"#;
    prove(source);
    // A kept role is still judged per arrival: a literal into the ranked slot
    // substitutes the actual value, so `0` is a lawful (terminating) rank
    // arrival while `9` falls outside the declared range.
    prove(&source.replace("first(saved, left - 1, 0, 1)", "first(saved, 0, 0, 1)"));
    reject(&source.replace("first(saved, left - 1, 0, 1)", "first(saved, 9, 0, 1)"));
    // Abstention never merges two different claimed entries: once `second`
    // carries `other` and forwards it into the spare slots, the proposals
    // genuinely conflict over which copy those slots continue.
    let contested = source
        .replace("spare: u32)", "spare: u32, other: u32)")
        .replace(
            "second(remaining + 0, payload)",
            "second(remaining + 0, payload, other)",
        )
        .replace(
            "second(pending - 1, carried)",
            "second(pending - 1, carried, first_spare)",
        )
        .replace(
            "state second(left: u32 [0..=5], saved: Payload)",
            "state second(left: u32 [0..=5], saved: Payload, tag: u32)",
        )
        .replace(
            "first(saved, left - 1, 0, 1)",
            "first(saved, left - 1, tag, tag)",
        );
    reject(&contested);
}

#[test]
fn a_diverging_copy_of_the_rank_subject_keeps_its_equality_obligation() {
    // A computed claimant on a duplicated *required* entry is never demoted:
    // the edge judgment must prove both copies equal at every arrival, which
    // `remaining + 1` cannot satisfy.
    let source = r#"
machine walk(remaining: u32 [0..=5], other: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> tally(remaining, other, remaining + 1) }
    state tally(count: u32 [0..=5], total: u32 [0..=5], echo: u32 [0..=6]) {
        transition count > 0 {
            true -> tally(count - 1, total, echo)
            false -> total
        }
    }
}
"#;
    reject(source);
    // An exactly equal computed copy does satisfy the obligation, but every
    // later arrival must re-establish it: forwarding the stale copy while the
    // rank moved rejects just the same as the diverging arrival.
    let equal = source
        .replace("remaining + 1", "remaining + 0")
        .replace("echo: u32 [0..=6]", "echo: u32 [0..=5]");
    reject(&equal);
    prove(&equal.replace(
        "tally(count - 1, total, echo)",
        "tally(count - 1, total, count - 1)",
    ));
}

#[test]
fn a_strict_step_names_the_ranked_copy_among_duplicated_arrivals() {
    // `live` receives the strict step while `saved` forwards only a stale
    // snapshot: the telescope names `live` the carrier of `remaining`, and
    // the loop still descends. Before strict-step naming, the equal-copies
    // obligation `live - 1 == saved` rejected this outright.
    let source = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> pair(remaining, remaining) }
    state pair(live: u32 [0..=5], saved: u32 [0..=5]) {
        transition live > 0 {
            true -> pair(live - 1, saved)
            false -> live
        }
    }
}
"#;
    prove(source);
    // Stepping both copies differently stays ambiguous: neither claimant is
    // named over the other, so the edge judgment still owes equality and
    // `live - 1 != saved - 2` fails it.
    reject(&source.replace("pair(live - 1, saved)", "pair(live - 1, saved - 2)"));
    // A stale read cannot hide behind the stepped copy: once `saved` is
    // demoted, `finish(saved)` arrives with no `remaining` role and cannot
    // prove the endpoint bound on `result`.
    reject(
        r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> pair(remaining, remaining) }
    state pair(live: u32 [0..=5], saved: u32 [0..=5]) {
        transition live > 0 {
            true -> pair(live - 1, saved)
            false -> finish(saved)
        }
    }
    state finish(result: u32 [0..=5]) {
        transition { _ -> result }
    }
}
"#,
    );
}

#[test]
fn explicit_self_edges_cannot_publish_termination_guarantees() {
    let source = "machine spin(remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 { transition { _ -> self } }";
    assert!(
        crate::checks::termination::check_machine_termination(&typed_program(source)).is_err(),
        "an unchanged self edge has no strict descent"
    );
    let unannotated = "machine spin() { transition { _ -> self } }";
    let program = typed_program(unannotated);
    assert_eq!(
        crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
        Some(language_semantics::TerminationGuarantee::NoGuarantee),
        "compile-time consumers must not infer acyclic termination for self loops"
    );
    super::super::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect("a productive loop without a termination promise remains valid");
    let caller = format!("{unannotated} machine caller() terminates; {{ spin(); }}");
    assert!(
        super::super::lower_typed_trees(typed_program(&caller), &crate::CheckingRequest::settled())
            .is_err(),
        "a terminating caller cannot inherit a false guarantee from a self loop"
    );
}

#[test]
fn explicit_self_occurrences_cannot_hide_beside_descending_edges() {
    for source in [
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining > 0 {
             true -> walk(remaining - 1) false -> self
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining == 0 {
             true -> self false -> walk(remaining - 1)
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 {
             transition { _ -> finish(remaining) }
             state finish(result: u32 [0..=5]) { transition { _ -> self } }
         }",
    ] {
        let diagnostics =
            crate::checks::termination::check_machine_termination(&typed_program(source))
                .expect_err("every self occurrence requires its own strict descent");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove rank range")
                    || diagnostic
                        .message
                        .contains("cannot prove the `terminates by` ranking")
            }),
            "{source}\n{diagnostics:#?}"
        );
        assert!(
            super::super::lower_typed_trees(
                typed_program(source),
                &crate::CheckingRequest::settled()
            )
            .is_err()
        );
    }
}

#[test]
fn a_strict_step_by_a_declared_amount_names_the_moved_copy() {
    // `live - step` is the moved copy of `remaining` once `step`'s declared
    // floor proves the amount nonzero: the stale `saved` snapshot demotes and
    // the cycle descends through `live`. Before this, only a positive literal
    // could name the moved copy of a diverged pair.
    let source = r#"
machine walk(remaining: u32 [0..=9], stride: u32 [1..=5])
terminates by remaining in 0..=9;
-> u32 {
    transition { _ -> pair(remaining, remaining, stride) }
    state pair(live: u32 [0..=9], saved: u32 [0..=9], step: u32 [1..=5]) {
        transition live >= step {
            true -> pair(live - step, saved, step)
            false -> live
        }
    }
}
"#;
    prove(source);
    // The same naming at the entry arrival: `remaining - stride` steps `left`
    // at the root edge and `live - step` steps it again on the cycle, so one
    // consistent carrier is named.
    prove(&source.replace(
        "transition { _ -> pair(remaining, remaining, stride) }",
        "transition remaining >= stride {
            true -> pair(remaining - stride, remaining, stride)
            false -> remaining
        }",
    ));
    // An unbounded or zero-able amount is not divergence evidence: the copies
    // may still hold equal values, so `live - step == saved` keeps its
    // equality obligation and cannot be proved.
    reject(&source.replace("step: u32 [1..=5]", "step: u32"));
    reject(&source.replace("[1..=5]", "[0..=5]"));
    // `live + step` names `live` just the same -- the copy still moved -- but
    // adding cannot satisfy a descending rank's obligations.
    reject(&source.replace("live - step", "live + step"));
    // Stepping both copies by the same declared amount keeps them provably
    // equal -- neither diverges from the other, so both claims hold with no
    // demotion at all.
    prove(
        &source
            .replace(
                "transition live >= step {",
                "transition live >= step && saved >= step {",
            )
            .replace(
                "pair(live - step, saved, step)",
                "pair(live - step, saved - step, step)",
            ),
    );
    // Two copies moved by *different* declared amounts leave two moved
    // claimants: no unique continuation is named, and the kept equality
    // obligation `live - step == saved - other` cannot be proved.
    reject(
        &source
            .replace(
                "stride: u32 [1..=5])",
                "stride: u32 [1..=5], other_stride: u32 [1..=5])",
            )
            .replace(
                "pair(remaining, remaining, stride)",
                "pair(remaining, remaining, stride, other_stride)",
            )
            .replace(
                "step: u32 [1..=5])",
                "step: u32 [1..=5], other: u32 [1..=5])",
            )
            .replace(
                "transition live >= step {",
                "transition live >= step && saved >= other {",
            )
            .replace(
                "pair(live - step, saved, step)",
                "pair(live - step, saved - other, step, other)",
            ),
    );
    // A mutable declared-positive amount still proves nonzero -- its type
    // constrains every stored value -- but a prefix write into any premise
    // carrier (including `step`, which the range constrains) invalidates.
    prove(&source.replace("step: u32 [1..=5]", "mut step: u32 [1..=5]"));
    reject(
        &source
            .replace("step: u32 [1..=5]", "mut step: u32 [1..=5]")
            .replace(
                "transition live >= step {",
                "step = 1;\n        transition live >= step {",
            ),
    );
}
