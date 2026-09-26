use super::super::super::TerminationGuarantee;
use crate::tests::termination::progress_mutation::CONTEXT_FIXTURE;

mod exposure;
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
