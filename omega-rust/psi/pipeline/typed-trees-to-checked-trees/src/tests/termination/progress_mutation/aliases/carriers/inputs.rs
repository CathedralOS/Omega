use super::*;

mod exposure;
mod queries;
mod transfer;

fn fixture_source(access: &str, body: &str, extra: &str) -> String {
    format!(
        "{CONTEXT_FIXTURE}
         data Carrier {{ context: &{access}Context; }}
         machine replace(mut carrier: Carrier, replacement: &Context) -> u64
         requires carrier.context.scheduler in WeakFair
         requires replacement.scheduler in WeakFair
         {{ {body} }}
         {extra}"
    )
}

fn assert_input_subject(program: &checked_trees::CheckedTrees) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let carrier = program.state_parameters(state)[0].symbol;
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("a frozen input reference requires its exact checked subject");
    };
    let [premise] = premises.as_slice() else {
        panic!("one exact input subject: {premises:#?}")
    };
    assert_eq!(premise.subject.root, carrier);
    assert_eq!(
        premise
            .subject
            .projections
            .iter()
            .map(|field| program.symbols.display_path(*field, "::"))
            .collect::<Vec<_>>(),
        ["Carrier::context", "Context::scheduler"]
    );
}

#[test]
fn a_reference_loaded_from_an_owned_input_carrier_keeps_its_subject() {
    for access in ["", "mut "] {
        let program = check_source(&fixture_source(
            access,
            &format!(
                "let borrowed: &{access}Context = carrier.context;
             transition {{ _ -> wait_context(borrowed) }}"
            ),
            "",
        ));
        assert_input_subject(&program);
    }
}

#[test]
fn a_local_copy_of_an_input_carrier_retains_its_shared_reference() {
    let program = check_source(&fixture_source(
        "",
        "let saved: Carrier = carrier;
         let borrowed: &Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        "",
    ));
    assert_input_subject(&program);
}

#[test]
fn a_disjoint_store_through_an_input_reference_preserves_its_subject() {
    let program = check_source(&fixture_source(
        "mut ",
        "let borrowed: &mut Context = carrier.context;
         borrowed.counter = 1;
         transition { _ -> wait_context(borrowed) }",
        "",
    ));
    assert_input_subject(&program);
}
