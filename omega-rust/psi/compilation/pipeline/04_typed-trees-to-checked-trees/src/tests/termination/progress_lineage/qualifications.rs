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
    .replace(
        "pub data SchedulerHandle {}",
        "pub data SchedulerHandle [copy] {}",
    )
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
    // A forwarded reference holds its loan through the transition, so a later
    // operand can no longer write the referent: `done(context,
    // change(context))` is a borrow conflict, not a lost fact. The claim is
    // still checked after every operand rather than at the argument's own
    // evaluation. An opaque call may replace the carrier's reference slot;
    // complete empty and disjoint write frames instead preserve its origin.
    for (operand, preserved) in [
        ("0", true),
        ("bump(&mut carrier.count)", true),
        ("carrier.touch()", true),
        ("host.expose(&mut carrier)", false),
    ] {
        let fixture = source(&format!(
            r#"
            pub data Carrier {{ context: &Context; count: u64; }}
            pub boundary trait Host {{ machine expose(carrier: &mut Carrier) -> u64; }}
            machine Carrier::touch(&mut self) -> u64 {{ 0 }}
            machine bump(count: &mut u64) -> u64 {{ count = 1; 0 }}
            machine route(host: Binding<Host>, mut carrier: Carrier) -> u64 reaches Host
            requires carrier.context.scheduler in WeakFair
            {{
                transition {{ _ -> done(carrier.context, {operand}) }}
                state done(selected: &Context, ignored: u64) -> u64 {{ borrowed(selected) }}
            }}
        "#
        ));
        let mut typed = crate::tests::front_end::typed_program_with_core_service(&fixture);
        crate::tests::bind_fixture_fused_service_erasures(&mut typed);
        match lower_typed_trees(typed, &CheckingRequest::settled()) {
            Ok(_) => assert!(preserved, "opaque write must retire the reference origin"),
            Err(diagnostics) => {
                assert!(!preserved, "{operand}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("cannot prove requires contract for call borrowed")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}
