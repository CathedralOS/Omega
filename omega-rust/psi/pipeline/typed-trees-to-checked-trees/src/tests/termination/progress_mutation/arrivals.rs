use super::*;

#[test]
fn disjoint_named_state_store_preserves_the_scheduler_origin() {
    let program = fixture_with_body(
        "context.counter = 1;
         transition { _ -> waiting(context) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        true,
        false,
        "",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn published_named_state_replacement_needs_only_the_replacement_premise() {
    let program = fixture_with_body(
        "context.scheduler = replacement.scheduler;
         transition { _ -> waiting(context) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        false,
        true,
        "",
    );
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn named_arrival_keeps_an_owned_copy_before_its_source_was_replaced() {
    let source = fixture_source(
        "let saved: SchedulerHandle = replacement.scheduler;
         replacement.scheduler = context.scheduler;
         context.scheduler = saved;
         transition { _ -> waiting(context) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        true,
        false,
        "",
    )
    .replace(
        "replacement: &Context) -> u64",
        "replacement: &mut Context) -> u64",
    );
    assert_subjects(&check_source(&source), &["replacement"]);
}

#[test]
fn named_state_join_retains_every_incoming_field_origin() {
    let program = fixture_with_body(
        "transition context.counter == 0 {
             true -> change(context, replacement)
             false -> waiting(context)
         }
         state change(selected: &mut Context, other: &Context) -> u64
         requires other.scheduler in WeakFair
         {
             selected.scheduler = other.scheduler;
             transition { _ -> waiting(selected) }
         }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        true,
        true,
        "",
    );
    assert_subjects(&program, &["context", "replacement"]);
}

#[test]
fn nested_owned_fields_keep_exact_origins_across_arrivals() {
    let source = format!(
        r#"{CONTEXT_FIXTURE}
        pub data Envelope {{ context: Context; other: Context; }}
        pub machine replace(context: &mut Envelope, replacement: &Envelope) -> u64
        requires replacement.context.scheduler in WeakFair
        terminates;
        {{
            context.context.scheduler = replacement.context.scheduler;
            context.other.counter = 1;
            transition {{ _ -> waiting(context) }}
            state waiting(selected: &Envelope) -> u64
            requires selected.context.scheduler in WeakFair
            {{ wait_context(selected.context) }}
        }}"#
    );
    let program = check_source(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let parameter = program
        .state_parameters(&program.machine_states(machine)[0])
        .iter()
        .find(|parameter| parameter.name.as_str() == "replacement")
        .unwrap();
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("nested field arrival retains its exact replacement");
    };
    let [premise] = premises.as_slice() else {
        panic!("only one nested scheduler is used: {premises:#?}");
    };
    assert_eq!(premise.subject.root, parameter.symbol);
    assert_eq!(
        premise
            .subject
            .projections
            .iter()
            .map(|projection| program.symbols.display_path(*projection, "::"))
            .collect::<Vec<_>>(),
        ["Envelope::context", "Context::scheduler"]
    );
}

#[test]
fn unknown_helper_replacement_cannot_recover_an_arrival_origin() {
    let program = fixture_with_body(
        "_ = overwrite(context, replacement);
         transition { _ -> waiting(context) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        true,
        false,
        "pub machine overwrite(context: &mut Context, replacement: &Context) -> u64
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         { context.scheduler = replacement.scheduler; 0 }",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn a_ranked_field_swap_cycle_retains_both_finite_entry_origins() {
    let source = format!(
        r#"{CONTEXT_FIXTURE}
        pub data Pair {{ first: SchedulerHandle; second: SchedulerHandle; }}
        pub machine wait_scheduler(scheduler: SchedulerHandle) -> u64
        requires scheduler in WeakFair
        terminates;
        {{ 0 }}
        pub machine rotate(pair: &mut Pair, remaining: u64) -> u64
        requires pair.first in WeakFair
        requires pair.second in WeakFair
        terminates;
        terminates by remaining;
        {{
            transition {{ _ -> cycle(pair, remaining) }}
            state cycle(pair: &mut Pair, remaining: u64) -> u64
            requires pair.first in WeakFair
            requires pair.second in WeakFair
            {{
                let saved: SchedulerHandle = pair.first;
                pair.first = pair.second;
                pair.second = saved;
                transition remaining > 0 {{
                    true -> cycle(pair, remaining - 1)
                    false -> wait_scheduler(pair.first)
                }}
            }}
        }}"#
    );
    let program = check_source(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "rotate")
        .unwrap();
    let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("field permutations have a finite subject closure");
    };
    assert_eq!(premises.len(), 2);
    for field in ["Pair::first", "Pair::second"] {
        assert!(
            premises
                .iter()
                .any(|premise| premise.subject.root == parameter.symbol
                    && premise.subject.projections.len() == 1
                    && program
                        .symbols
                        .display_path(premise.subject.projections[0], "::")
                        == field),
            "{premises:#?}"
        );
    }
}
