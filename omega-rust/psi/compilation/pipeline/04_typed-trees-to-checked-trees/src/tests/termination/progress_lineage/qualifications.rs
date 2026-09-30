//! Arrival membership is a live fact, independently of progress subject lineage.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::termination::progress_lineage::PROGRESS_PROFILE;
use crate::tests::termination::progress_lineage::checked;
use crate::tests::termination::progress_lineage::typed;

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
    let Err(diagnostics) = lower_typed_trees(typed(source), &CheckingRequest::settled()) else {
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
fn an_owned_argument_keeps_its_qualification_snapshot_before_later_writes() {
    // The owned argument is a copy taken at its own evaluation, so a later
    // operand may write the source. An affine `Context` would instead move
    // into `selected` and leave nothing for `&mut context` to lend.
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
    )
    .replace("pub data SchedulerHandle {}", "pub data SchedulerHandle [copy] {}")
    .replace("pub data Context {", "pub data Context [copy] {");
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
