use checked_trees::{CheckFacts, FlowStateFact};

fn statement_diagnostics(
    typed: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    super::check_statement_borrows(
        typed,
        facts,
        state,
        &[],
        &mut diagnostics,
        &mut Vec::new(),
        &[],
        &mut [],
        &mut Vec::new(),
        &[],
        &mut [],
        &crate::flow::StateMutationSummaryCache::default(),
        &crate::flow::StateReceiverReadCache::default(),
        None,
        &mut None,
    );
    diagnostics
}

#[test]
fn held_successor_writes_require_the_local_carriers_exact_closure() {
    let checked = crate::tests::front_end::checked_program_result(
        "data Main { count: u64; other: u64; }
         machine Main::run(main: &mut Main) -> u64 {
             let view: &u64 = &main.count;
             transition { _ -> next(main, view) }
             state next(main: &mut Main, child: &u64) -> u64 {
                 main.other = child;
                 transition { _ -> done(main) }
             }
             state done(main: &mut Main) -> u64 {
                 main.count = 7;
                 (main.count)
             }
         }",
    )
    .expect("a forwarded local child ends before the parent writes its former backing");
    let (loan_handle, loan) = checked
        .facts
        .borrow
        .loans
        .iter()
        .find(|(_, loan)| checked.typed.symbols.name(loan.owner_symbol) == "view")
        .expect("source local child loan");
    let source_state = checked.typed.symbols.get(loan.owner_symbol).parent;
    let (flow_handle, flow) = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| state.state_symbol == source_state)
        .expect("the child's source state");
    let (weakening_handle, weakening) = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .iter()
        .find(|(_, weakening)| weakening.loan == loan_handle)
        .expect("recorded source-carrier closure");
    assert_ne!(
        weakening.source,
        checked_trees::FlowInvalidationSource::Statement { statement_index: 0 },
    );
    let diagnostics = statement_diagnostics(&checked.typed, &checked.facts, flow);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");

    // Resource replay also rejects missing or changed closure evidence, but
    // that earlier rejection would pass even if mutation admission silently
    // dropped every post-release write. Exercise the owning statement check
    // directly so this control pins its fallback to the complete write run.
    for missing in [true, false] {
        let mut facts = checked.facts.clone();
        if missing {
            facts
                .flow
                .control
                .states
                .get_mut(flow_handle)
                .borrow_weakenings = Default::default();
        } else {
            facts
                .flow
                .borrow_lifetimes
                .weakenings
                .get_mut(weakening_handle)
                .source = checked_trees::FlowInvalidationSource::Statement { statement_index: 0 };
        }
        let diagnostics = statement_diagnostics(
            &checked.typed,
            &facts,
            facts.flow.control.states.get(flow_handle),
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("mutates `main.count`")
                    && diagnostic
                        .message
                        .contains("while local borrow `view` is still active")
            }),
            "missing closure: {missing}; {diagnostics:#?}",
        );
    }
}
