use super::*;

fn assert_no_checked_guarantee(program: &checked_trees::CheckedTrees) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn a_forwarded_reference_preserves_the_subject_after_a_disjoint_helper_write() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let writable: &mut Context = &mut context;
                 let returned: &{access}Context = forward(writable);
                 transition {{ _ -> wait_context(returned) }}"
            ),
            true,
            false,
            &format!(
                "machine forward(context: &mut Context) -> &{access}Context {{
                     let writable: &mut Context = &mut context;
                     writable.counter = 1;
                     &{access}writable
                 }}"
            ),
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn a_forwarded_reference_cannot_restore_the_subject_after_an_overlapping_helper_write() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let writable: &mut Context = &mut context;
                 let returned: &{access}Context = forward(writable, replacement);
                 transition {{ _ -> wait_context(returned) }}"
            ),
            true,
            false,
            &format!(
                "machine forward<'context, 'replacement>(
                     context: &'context mut Context,
                     replacement: &'replacement Context
                 ) -> &'context {access}Context
                 requires replacement.scheduler in WeakFair
                 ensures context.scheduler in WeakFair
                 terminates;
                 {{
                     context.scheduler = replacement.scheduler;
                     &{access}context
                 }}"
            ),
        );
        // The helper preserves the reference and the field's qualification,
        // but its write frame does not identify the replacement scheduler.
        assert_no_checked_guarantee(&program);
    }
}

#[test]
fn a_shared_helper_local_copy_keeps_its_subject_when_the_original_binding_changes() {
    for (returned, field) in [("prior", "first"), ("borrowed", "second")] {
        let source = format!(
            r#"{CONTEXT_FIXTURE}
            pub data ContextPair {{ first: Context; second: Context; }}
            machine select(choices: &ContextPair) -> &Context {{
                let mut borrowed: &Context = &choices.first;
                let prior: &Context = borrowed;
                borrowed = &choices.second;
                {returned}
            }}
            machine replace(choices: &ContextPair) -> u64
            requires choices.first.scheduler in WeakFair
            requires choices.second.scheduler in WeakFair
            {{
                let borrowed: &Context = select(choices);
                transition {{ _ -> wait_context(borrowed) }}
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
            .find(|parameter| parameter.name.as_str() == "choices")
            .unwrap();
        let plan = program
            .facts
            .termination
            .for_machine(machine.symbol)
            .unwrap();
        let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
            panic!("returning {returned} must retain its exact scheduler subject");
        };
        let [premise] = premises.as_slice() else {
            panic!("returning {returned} uses one scheduler: {premises:#?}");
        };
        assert_eq!(premise.subject.root, parameter.symbol);
        assert_eq!(
            premise
                .subject
                .projections
                .iter()
                .map(|projection| program.symbols.display_path(*projection, "::"))
                .collect::<Vec<_>>(),
            [
                format!("ContextPair::{field}"),
                "Context::scheduler".to_owned()
            ],
            "returning {returned} must select {field}, even though both fields share a root"
        );
    }
}

#[test]
fn a_shared_parameter_exposed_inside_a_helper_has_no_exact_returned_subject() {
    let program = fixture_with_body(
        "let returned: &Context = forward(context);
         transition { _ -> wait_context(returned) }",
        true,
        false,
        "machine inspect_binding(binding: &mut Context) {}
         machine forward(mut context: &Context) -> &Context {
             inspect_binding(&mut context);
             context
         }",
    );
    // An empty frame cannot exempt exclusive exposure of a shared binding
    // from the helper's non-rebinding requirement.
    assert_no_checked_guarantee(&program);
}

#[test]
fn a_shared_local_exposed_inside_a_helper_has_no_exact_returned_subject() {
    let program = fixture_with_body(
        "let returned: &Context = forward(context);
         transition { _ -> wait_context(returned) }",
        true,
        false,
        "machine inspect_binding(binding: &mut Context) {}
         machine forward(context: &Context) -> &Context {
             let mut borrowed: &Context = context;
             inspect_binding(&mut borrowed);
             borrowed
         }",
    );
    assert_no_checked_guarantee(&program);
}

#[test]
fn an_unused_helper_argument_cannot_hide_shared_binding_exposure() {
    for selected in ["borrowed", "returned"] {
        let program = fixture_with_body(
            &format!(
                "let mut borrowed: &Context = &context;
                 let returned: &Context = forward(replacement, &mut borrowed);
                 transition {{ _ -> wait_context({selected}) }}"
            ),
            true,
            false,
            "machine forward<'selected, 'binding>(
                 selected: &'selected Context,
                 binding: &'binding mut Context
             ) -> &'selected Context {
                 selected
             }",
        );
        // Proving the selected argument's origin does not admit exposure in
        // an unused sibling argument, even when the helper writes nothing.
        assert_no_checked_guarantee(&program);
    }
}
