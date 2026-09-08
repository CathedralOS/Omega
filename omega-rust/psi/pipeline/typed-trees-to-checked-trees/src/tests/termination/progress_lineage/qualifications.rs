//! Arrival membership is a live fact, independently of progress subject lineage.

use super::*;

fn source(body: &str) -> String {
    format!(
        r#"{PROGRESS_PROFILE}
        pub data Context {{ scheduler: SchedulerHandle; counter: u64; }}
        machine owned(context: Context) -> u64
        requires context.scheduler in WeakFair
        {{ 0 }}
        machine borrowed(context: &Context) -> u64
        requires context.scheduler in WeakFair
        {{ 0 }}
        {body}
    "#
    )
}

fn reject(source: &str, target: &str) {
    let Err(diagnostics) = lower_typed_trees(typed(source)) else {
        panic!("missing live qualification must reject call {target}");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains(&format!("cannot prove requires contract for call {target}"))),
        "{diagnostics:#?}"
    );
}

#[test]
fn every_arrival_must_supply_the_same_qualification() {
    for (first, second) in [("qualified", "other"), ("other", "qualified")] {
        let fixture = source(&format!(
            r#"
            machine route(qualified: Context, other: Context, choose: bool) -> u64
            requires qualified.scheduler in WeakFair
            requires other.scheduler in WeakFair
            {{
                transition choose {{ true -> done({first}) false -> done({second}) }}
                state done(selected: Context) -> u64 {{ owned(selected) }}
            }}
        "#
        ));
        checked(&fixture);
        reject(
            &fixture.replace("requires other.scheduler in WeakFair", ""),
            "owned",
        );
    }
}

#[test]
fn a_same_named_formal_does_not_take_another_actuals_membership() {
    let fixture = source(
        r#"
        machine route(qualified: Context, other: Context) -> u64
        requires qualified.scheduler in WeakFair
        {
            transition { _ -> done(other, qualified) }
            state done(qualified: Context, other: Context) -> u64 { owned(qualified) }
        }
    "#,
    );
    reject(&fixture, "owned");
    checked(&fixture.replace("done(other, qualified)", "done(qualified, other)"));
}

#[test]
fn an_owned_argument_keeps_its_qualification_snapshot_before_later_writes() {
    let fixture = source(
        r#"
        machine change(context: &mut Context) -> u64 {
            context.scheduler = SchedulerHandle {};
            0
        }
        machine route(mut context: Context) -> u64
        requires context.scheduler in WeakFair
        {
            transition { _ -> done(context, change(&mut context)) }
            state done(selected: Context, ignored: u64) -> u64 { owned(selected) }
        }
    "#,
    );
    checked(&fixture);
    reject(
        &fixture.replace(
            "requires context.scheduler in WeakFair\n        {\n            transition",
            "{\n            transition",
        ),
        "owned",
    );
}

#[test]
fn a_reference_argument_requires_its_fact_to_survive_later_operands() {
    for (write, preserved) in [
        ("context.counter = 1;", true),
        ("context.scheduler = SchedulerHandle {};", false),
    ] {
        let fixture = source(&format!(
            r#"
            machine change(context: &mut Context) -> u64 {{ {write} 0 }}
            machine route(context: &mut Context) -> u64
            requires context.scheduler in WeakFair
            {{
                transition {{ _ -> done(context, change(context)) }}
                state done(selected: &Context, ignored: u64) -> u64 {{ borrowed(selected) }}
            }}
        "#
        ));
        if preserved {
            checked(&fixture);
        } else {
            reject(&fixture, "borrowed");
        }
    }
}

#[test]
fn prior_alias_writes_preserve_only_disjoint_qualifications() {
    for (write, preserved) in [
        ("alias.counter = 1;", true),
        ("alias.scheduler = SchedulerHandle {};", false),
    ] {
        let fixture = source(&format!(
            r#"
            machine route(context: &mut Context) -> u64
            requires context.scheduler in WeakFair
            {{
                let alias: &mut Context = context;
                {write}
                transition {{ _ -> done(context) }}
                state done(selected: &Context) -> u64 {{ borrowed(selected) }}
            }}
        "#
        ));
        if preserved {
            checked(&fixture);
        } else {
            reject(&fixture, "borrowed");
        }
    }
}

#[test]
fn a_late_unqualified_loop_arrival_retires_the_initial_membership() {
    let fixture = source(
        r#"
        machine route(qualified: Context, other: Context, remaining: u64) -> u64
        requires qualified.scheduler in WeakFair
        {
            transition { _ -> cycling(qualified, other, remaining) }
            state cycling(selected: Context, replacement: Context, remaining: u64) -> u64 {
                transition remaining > 0 {
                    true -> cycling(replacement, replacement, remaining - 1)
                    false -> owned(selected)
                }
            }
        }
    "#,
    );
    reject(&fixture, "owned");
    checked(&fixture.replace(
        "cycling(replacement, replacement, remaining - 1)",
        "cycling(selected, replacement, remaining - 1)",
    ));
}

#[test]
fn self_arrivals_preserve_only_memberships_live_at_the_edge() {
    let fixture = source(
        r#"
        machine route(qualified: Context, again: bool) -> u64
        requires qualified.scheduler in WeakFair
        {
            transition { _ -> waiting(qualified, again) }
            state waiting(selected: Context, again: bool) -> u64 {
                transition again { true -> self false -> owned(selected) }
            }
        }
    "#,
    );
    checked(&fixture);
    reject(
        &fixture.replace("requires qualified.scheduler in WeakFair", ""),
        "owned",
    );
}

#[test]
fn reference_binding_exposure_is_not_an_empty_frame_preservation_proof() {
    // Match the qualified binding exposure used by aliases/helpers/effects:
    // the helper accepts &mut Context, not an authored nested reference type.
    let fixture = source(
        r#"
        machine expose(binding: &mut Context) -> u64 { 0 }
        machine route(mut context: &Context) -> u64
        requires context.scheduler in WeakFair
        {
            transition { _ -> done(context, expose(&mut context)) }
            state done(selected: &Context, ignored: u64) -> u64 { borrowed(selected) }
        }
    "#,
    );
    reject(&fixture, "borrowed");
    checked(&fixture.replace("expose(&mut context)", "0"));
}
