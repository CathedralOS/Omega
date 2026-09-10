use checked_trees::{BorrowFacts, CheckedOperatorFacts};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

use crate::flow::StateMutationSummaryCache;

struct RangeCheckFixture {
    program: TypedTrees,
    borrows: BorrowFacts,
    operators: CheckedOperatorFacts,
    flow: checked_trees::FlowFacts,
}

impl RangeCheckFixture {
    fn new(written_element: usize) -> Self {
        let mut source = format!(
            "machine touch(selectors: &mut [u64; 2]) -> u64 {{
                selectors[{written_element}] = 9;
                0
            }}\n"
        );
        for machine_name in ["first_reader", "second_reader"] {
            source.push_str(&format!(
                r#"
                machine {machine_name}(items: &[u8; 4], selectors: &mut [u64; 2], flag: bool) -> u8
                requires selectors[0] < 4;
                {{
                    transition flag {{
                        true -> read(touch(selectors), items, selectors, flag)
                        false -> repeat(touch(selectors), items, selectors, flag)
                    }}
                    state read(ignored: u64, items: &[u8; 4], selectors: &mut [u64; 2], flag: bool) -> u8
                    requires selectors[0] < 4;
                    {{
                        touch(selectors);
                        let value: u8 = items[selectors[0]];
                        transition flag {{
                            true -> repeat(0, items, selectors, flag)
                            false -> value
                        }}
                    }}
                    state repeat(ignored: u64, items: &[u8; 4], selectors: &mut [u64; 2], flag: bool) -> u8
                    requires selectors[0] < 4;
                    {{
                        touch(selectors);
                        let value: u8 = items[selectors[0]];
                        transition flag {{
                            true -> read(touch(selectors), items, selectors, flag)
                            false -> value
                        }}
                    }}
                }}
                "#
            ));
        }
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize range cache fixture");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse range cache fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve range cache fixture");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type range cache fixture");
        let borrows = crate::build_borrow_facts(&program);
        let proof_plan = proof::obligations::build_proof_plan(&program);
        let values = crate::values::build_value_facts(&program, &proof_plan);
        let operators = crate::operators::build_operator_facts(&program, &values);
        let flow = range_flow_fixture(&program, &borrows);
        Self {
            program,
            borrows,
            operators,
            flow,
        }
    }

    fn check(&self) -> (Result<(), Vec<Diagnostic>>, usize) {
        let frames = validation::CallFrameResolver::new(&self.program)
            .expect("fixture has complete symbol resolution");
        let incoming =
            super::incoming_guards::IncomingGuardIndex::build(&self.program, Some(&frames));
        // Count only the actual range check, including its incoming-state
        // fixed points and branch snapshots. Setup must not enter the delta.
        let before = StateMutationSummaryCache::build_count();
        let result = super::check_indexed_accesses(
            &self.program,
            &self.operators,
            &self.borrows,
            &self.flow,
            Some(&frames),
            &incoming,
        );
        let initialized = StateMutationSummaryCache::build_count() - before;
        (result, initialized)
    }
}

pub(super) fn range_flow_fixture(
    program: &TypedTrees,
    borrows: &BorrowFacts,
) -> checked_trees::FlowFacts {
    let plan = proof::obligations::build_proof_plan(program);
    let proof = crate::build_proof_facts(program, &plan, borrows);
    let mut semantic = crate::build_semantic_facts(program, &proof);
    let domains = crate::build_domain_facts(program, &semantic);
    let operational = validation::infer_operational_may(program);
    let mut flow = crate::build_flow_facts(
        program,
        borrows,
        &proof,
        &mut semantic,
        &domains,
        &operational,
    );
    crate::review_sources::bind_checked_body_call_source_spans(program, &mut flow)
        .expect("bind exact range fixture call identities as production does");
    flow
}

#[test]
fn range_check_shares_summaries_across_machines_state_passes_and_branches() {
    let fixture = RangeCheckFixture::new(1);
    // Reached-state discovery needs multiple collection passes through the
    // cycle. Bounds are checked at their declared state, after a real call;
    // this does not depend on transporting projected bounds between states.
    // The first mutating calls occur inside the entry's cloned branches.
    for invocation in 0..2 {
        let (result, initialized) = fixture.check();
        assert!(
            result.is_ok(),
            "invocation {invocation}, {initialized} summary builds: {result:#?}"
        );
        assert_eq!(
            initialized, 1,
            "invocation {invocation} must build one table across both machines, \
             repeated reached-state visits, branch snapshots, and final checking"
        );
    }
}

#[test]
fn fresh_range_check_rebuilds_summaries_after_callee_write_changes() {
    let disjoint = RangeCheckFixture::new(1);
    let overlapping = RangeCheckFixture::new(0);
    // Both programs have the same declaration layout. Changing only the
    // callee's element write must not inherit the first program's summary.
    assert_eq!(
        disjoint.program.machines()[0].symbol,
        overlapping.program.machines()[0].symbol
    );
    let (accepted, first_initialized) = disjoint.check();
    assert!(accepted.is_ok(), "disjoint write: {accepted:#?}");

    let (rejected, second_initialized) = overlapping.check();
    let diagnostics = rejected.expect_err("writing selector zero invalidates its live index bound");
    assert!(
        !diagnostics.is_empty()
            && diagnostics.iter().all(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove index `selectors[0]`")
                    && diagnostic.message.contains("length 4")
            }),
        "expected only the stale live selector bound to reject: {diagnostics:#?}"
    );
    assert_eq!(first_initialized, 1, "first invocation summary table");
    assert_eq!(second_initialized, 1, "changed program needs a fresh table");
}
