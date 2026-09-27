//! Invocation qualification is preserved on the original parameter, never
//! inferred for a later block arrival from the entry's precondition.
use super::{CheckedComposedUnitControlMachinePlan, CheckedTrees, admission};
use crate::{LoweringError, TerminalMachineSelection};

fn fixture() -> (CheckedTrees, CheckedComposedUnitControlMachinePlan) {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8]::Path requires no_nul(self);
        data Worker {}
        machine Worker::run(path: &[u8] in Path) {
            transition { _ -> done() }
            state done() {}
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 2)
        .expect("qualified invocation retains the ordinary two-state graph")
        .clone();
    assert_eq!(plan.states[0].structural_parameters.len(), 1);
    assert_eq!(
        plan.states[0].structural_parameters[0].qualifications.len(),
        1
    );
    (checked, plan)
}

#[test]
fn qualified_invocation_retains_exact_domain_on_original_parameter() {
    let (checked, plan) = fixture();
    assert!(admission::has_shared_graph_custody(&checked, &plan));
    admission::admit(&checked, &plan).expect("source-exact invocation qualification");
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Worker::run"))
        .expect("qualified entry lowers through the ordinary signature catalog");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the invocation qualification independently verifies");
    let module = &lowered.semantic_module;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("the selected invocation");
    let parameter = &machine.structural_parameters[0];
    assert_eq!(parameter.qualifications.len(), 1);
    let domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.id == parameter.qualifications[0])
        .expect("the qualification has a published declaration");
    assert_eq!(
        domain.semantic_domain,
        semantic_vocabulary::DomainSemanticId::new(u64::from(
            plan.states[0].structural_parameters[0].qualifications[0].0
        ),)
        .expect("the source domain identity is nonzero")
    );
    assert_eq!(domain.carrier, parameter.structural_type);
    assert!(
        machine
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty()),
        "invocation custody is not redeclared as a block parameter"
    );
}

#[test]
fn qualified_invocation_rejects_missing_extra_and_duplicate_domains() {
    let (checked, original) = fixture();
    admission::admit(&checked, &original).expect("unmodified control");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let qualifications = &mut changed.states[0].structural_parameters[0].qualifications;
        match mutation {
            0 => qualifications.clear(),
            1 => {
                let extra = language_semantics::SemanticDomainTable::WRAPPING;
                assert!(!qualifications.contains(&extra));
                qualifications.push(extra);
            }
            2 => qualifications.push(qualifications[0]),
            _ => unreachable!(),
        }
        let error = admission::admit(&checked, &changed)
            .err()
            .expect("retained domains must equal the original declaration exactly");
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(
                    "Unit graph borrowed parameter qualifications differ from invocation custody"
                )
            ),
            "mutation {mutation}: {error:?}"
        );
    }
}

#[test]
fn qualified_invocation_rejects_entry_reentry_as_new_domain_authority() {
    let (checked, original) = fixture();
    admission::admit(&checked, &original).expect("unmodified control");
    let mut changed = original.clone();
    let entry = changed.states[0].state;
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[0].terminator
    else {
        panic!("fixture starts with an ordinary successor");
    };
    successor.target_state = entry;
    // Even before argument/edge replay, the qualified invocation is no longer
    // a one-shot input: its incoming backedge would need its own proof.
    assert!(!admission::has_shared_graph_custody(&checked, &changed));
    let error = admission::admit(&checked, &changed)
        .err()
        .expect("entry qualifications cannot authorize a new block arrival");
    assert!(
        matches!(
            error,
            LoweringError::Unsupported(
                "Unit graph borrowed parameter qualifications differ from invocation custody"
            )
        ),
        "{error:?}"
    );
}
