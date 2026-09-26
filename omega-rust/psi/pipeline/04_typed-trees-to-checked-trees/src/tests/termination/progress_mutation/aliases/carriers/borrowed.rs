use super::super::super::TerminationGuarantee;

use crate::tests::termination::progress_mutation::CONTEXT_FIXTURE;

mod direct;
mod effects;
mod queries;
mod results;

fn source(outer_access: &str, inner_access: &str, body: &str, extra: &str) -> String {
    format!(
        "{CONTEXT_FIXTURE}
         data Carrier {{ context: &{inner_access}Context; }}
         machine inspect(carrier: &{outer_access}Carrier) -> u64
         requires carrier.context.scheduler in WeakFair
         {{ {body} }}
         {extra}"
    )
}

fn assert_input_premise(program: &checked_trees::CheckedTrees) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("one exact input qualification: {:?}", plan.checked_summary)
    };
    let [premise] = premises.as_slice() else {
        panic!("one premise: {premises:?}")
    };
    assert_eq!(
        premise.subject.root,
        program.state_parameters(state)[0].symbol
    );
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
