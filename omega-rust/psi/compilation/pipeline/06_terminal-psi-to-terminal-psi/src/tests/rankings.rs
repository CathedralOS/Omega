//! Loop progress controls tie each comparison to the cycle and value it checks.
use super::*;
use crate::reverify;
use terminal_check_tables::obligations::ObligationRow;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
};

fn ranked(signed: bool, tuple: bool, stalls: bool) -> typed_trees_to_terminal_psi::Elaboration {
    let scalar = if signed {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    } else {
        u32_type()
    };
    let literal = |number: i128| {
        super::literal(
            scalar,
            if signed {
                IntegerValue::Signed(number)
            } else {
                IntegerValue::Unsigned(number as u128)
            },
        )
    };
    let compare = |limit| {
        expression(
            ExpressionKind::Strict {
                operation: Strict::Greater,
                policy: None,
                left: Box::new(slot(0, scalar)),
                right: Box::new(literal(limit)),
            },
            Some(ScalarType::Boolean),
        )
    };
    let mut actuals = vec![strict(Strict::Subtract, slot(0, scalar), literal(1))];
    if tuple {
        actuals.push(literal(0));
    }
    let mut recurse = call(1, actuals);
    recurse.result = Some(ValueType::plain(scalar));
    let mut statements = vec![guarded(
        compare(if signed {
            -5
        } else if stalls {
            1
        } else {
            0
        }),
        recurse,
    )];
    if stalls {
        let mut recurse = call(1, vec![slot(0, scalar)]);
        recurse.result = Some(ValueType::plain(scalar));
        statements.push(guarded(compare(0), recurse));
    }
    statements.push(returns(literal(0)));
    let mut body = machine(vec![scalar; if tuple { 2 } else { 1 }], statements);
    body.result = scalar;
    let mut elaboration = lower(vec![body]).unwrap();
    let machine = &mut elaboration.module.machines[0];
    let members = terminal_semantics::control_graph::control_cycle_members(machine).unwrap();
    assert_eq!(members.len(), 1);
    let members = &members[0];
    let header = machine.block(&members[0]).unwrap();
    let header_id = header.id;
    let ScalarType::Integer(integer_type) = scalar else {
        unreachable!()
    };
    let ranks = header
        .parameters
        .iter()
        .map(|parameter| TerminalIntegerRankValue {
            integer_type,
            value: parameter.id,
        })
        .collect::<Vec<_>>();
    assert_eq!(ranks.len(), if tuple { 2 } else { 1 });
    let mut cycle = TerminalNaturalCycle {
        observations: Default::default(),
        ranks: Vec::new(),
        edges: Vec::new(),
    };
    for block in members {
        let values = cycle.observations.insert_many(ranks.clone());
        cycle.ranks.push(TerminalBlockNaturalRank {
            block: *block,
            values,
        });
    }
    for block in machine
        .blocks
        .iter()
        .filter(|block| members.contains(&block.id))
    {
        for (edge, target) in block
            .terminator
            .successor_edges()
            .into_iter()
            .filter(|(_, target)| members.contains(target))
        {
            let successors = if target == header_id {
                let arguments = match &block.terminator {
                    Terminator::Jump { arguments, .. } => arguments,
                    _ => panic!("tail jump"),
                };
                arguments
                    .iter()
                    .map(|value| TerminalIntegerRankValue {
                        integer_type,
                        value: *value,
                    })
                    .collect::<Vec<_>>()
            } else {
                ranks.clone()
            };
            let comparison = if target == header_id && successors[0].value != ranks[0].value {
                TerminalNaturalRankComparison::Strict
            } else {
                TerminalNaturalRankComparison::Preserving
            };
            let successor_ranks = cycle.observations.insert_many(successors);
            cycle.edges.push(TerminalNaturalRankEdge {
                edge,
                source: block.id,
                target,
                successor_ranks,
                comparison,
            });
        }
    }
    let component = terminal_semantics::control_graph::control_cycle_identity(machine, &cycle);
    let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
    tables.obligations.push(ObligationRow {
        kind: ObligationKind::WellFoundedness { component },
        created_at: ProgramPoint::BlockEntry(members[0]),
    });
    for edge in &cycle.edges {
        tables.obligations.push(ObligationRow {
            kind: ObligationKind::RankComparison {
                component,
                edge: edge.edge,
            },
            created_at: ProgramPoint::Edge(edge.edge),
        });
    }
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![cycle]));
    machine.contract.termination = semantic_vocabulary::Termination::Terminates;
    elaboration
}

#[test]
fn finite_integer_rankings_use_real_kernel_checked_edge_goals() {
    for (signed, tuple) in [(false, false), (true, false), (false, true)] {
        let mut elaboration = ranked(signed, tuple, false);
        let discharged =
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap();
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        let verdicts = &discharged.machines.get(id(1)).unwrap().verdicts;
        for (obligation, row) in tables.obligations.iter() {
            match row.kind {
                ObligationKind::WellFoundedness { component } => assert!(
                    matches!(verdicts.get(obligation), Some(Ok(Discharged::FiniteIntegerRanking(relation))) if relation.component == component && relation.carriers.len() == if tuple { 2 } else { 1 })
                ),
                ObligationKind::RankComparison { .. } => {
                    let Some(Ok(Discharged::Certificate(evidence))) = verdicts.get(obligation)
                    else {
                        panic!("rank edge has no certificate")
                    };
                    assert_kernel_checked(&elaboration, 1, obligation, evidence);
                    let mut consumer = elaboration.clone();
                    let mut activity = crate::ProofSearchActivity::default();
                    let mut replay = crate::prepare_replay(&mut consumer, &mut activity).unwrap();
                    reverify(
                        &mut replay,
                        id(1),
                        obligation,
                        &evidence.certificate,
                        &mut activity,
                    )
                    .unwrap();
                }
                _ => {}
            }
        }
    }
}

#[test]
fn exact_ranking_inventory_rejects_missing_surplus_swapped_and_stale_claims() {
    for fault in 0..8 {
        let mut elaboration = ranked(false, false, false);
        let machine = &mut elaboration.module.machines[0];
        let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
            unreachable!()
        };
        let cycle = &mut cycles[0];
        match fault {
            0 => {
                cycle.ranks.pop();
            }
            1 => {
                cycle.edges.pop();
            }
            2 => cycle.ranks.swap(0, 1),
            3 => cycle.edges.reverse(),
            4 => {
                cycle.edges[0].source = cycle.edges[0].target;
            }
            5 => {
                let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
                let duplicate = *tables
                    .obligations
                    .iter()
                    .find(|(_, row)| matches!(row.kind, ObligationKind::WellFoundedness { .. }))
                    .unwrap()
                    .1;
                tables.obligations.push(duplicate);
            }
            6 => {
                let tables = elaboration.tables.machines.get_mut(machine.id).unwrap();
                let last = tables.obligations.len();
                tables.obligations = IdTable::from_rows(
                    tables
                        .obligations
                        .iter()
                        .take(last - 1)
                        .map(|(id, row)| (id, *row)),
                )
                .expect("the retained obligation prefix remains dense");
            }
            7 => {
                let value = ranks_value(cycle);
                let wrong = cycle
                    .observations
                    .insert_many(vec![TerminalIntegerRankValue {
                        integer_type: IntegerType::supported_address_representation(),
                        value,
                    }]);
                cycle.ranks[0].values = wrong;
            }
            _ => unreachable!(),
        }
        assert!(
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).is_err(),
            "fault {fault} accepted"
        );
    }
}

/// Use the real catalog file and its exact source custody. Only its ranking
/// declaration prefix is needed here; unrelated Nat proofs are not this test's
/// input. Prefix parsing keeps the declaration's original source coordinates.
pub(crate) fn typed_with_ranking_catalog(text: &str) -> typed_trees::TypedTrees {
    use std::sync::Arc;
    let core_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let nat = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/nat.omg"
    ));
    let prefix = nat
        .split_once("// The core algebraic")
        .expect("ranking declaration prefix")
        .0;
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("rankings/source.omg".into(), text.to_owned())
        .source_id;
    let core = sources
        .add_with_metadata(
            core_root.join("nat.omg"),
            nat.to_owned(),
            core_root,
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let tokens = source_files_to_tokens::Lexer::new(prefix)
        .tokenize()
        .unwrap();
    tokens_to_syntax_trees::parse(&mut syntax, core, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: vec![symbols::SourceScopedTopLevelBinding::logical_module_import(
                source,
                core,
                "Nat::Descending",
                0,
            )],
            retained_base: None,
        },
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn an_authored_countdown_measure_must_decrease_on_the_recursive_route() {
    let source = |next: &str| {
        format!(
            r#"
machine countdown(n: u32) -> u32
terminates by n -> Nat::Descending
crashes none
{{
    transition n > 0 {{
        true -> countdown({next})
        false -> 37
    }}
}}
machine entry() -> u32 terminates crashes none {{ countdown(4) }}
"#
        )
    };
    let verdicts = |next| {
        super::boundary_calls::compilation_results_of(&typed_with_ranking_catalog(&source(next)))
    };
    let positive = verdicts("n - 1");
    assert!(
        positive
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "the authored countdown must retain and prove its measure"
    );
    for next in ["n", "n + 1"] {
        let verdicts = verdicts(next);
        assert!(
            verdicts.iter().any(|verdict| matches!(
                verdict, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::RankComparison { .. }))
            ) || matches!(
                verdict, Ok(RouteVerdict::FallBack(refusal))
                    | Err(crate::CompilationRefusal::Elaboration(refusal))
                if refusal.detail.contains("removing strict rank edges")
            )),
            "the {next} recursive route must fail its progress obligation"
        );
    }
}

fn ranks_value(cycle: &TerminalNaturalCycle) -> semantic_vocabulary::ValueId {
    cycle.rank_values(cycle.ranks[0].values).unwrap()[0].value
}

#[test]
fn borrowed_slice_iterations_keep_their_measure_and_backing() {
    let program = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/views/ranked_slice_iteration/main.omg"
    ));
    let source = format!("{program}\nmachine entry() crashes Trap {{ main(); }}\n");
    let results =
        super::boundary_calls::compilation_results_of(&typed_with_ranking_catalog(&source));
    assert!(!results.is_empty(), "the kept program must be checked");
    for result in results {
        match result {
            Ok(RouteVerdict::Accepted(_)) => {}
            Ok(RouteVerdict::FallBack(refusal))
            | Err(crate::CompilationRefusal::Elaboration(refusal)) => {
                panic!("borrowed iteration was refused: {}", refusal.detail);
            }
            other => panic!("borrowed iteration did not pass: {other:?}"),
        }
    }
}

#[test]
fn every_complete_cycle_must_cross_a_strict_edge() {
    let mut elaboration = ranked(false, false, true);
    let refusal =
        discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap_err();
    assert!(
        refusal.detail.contains("removing strict rank edges"),
        "{}",
        refusal.detail
    );
}

#[test]
fn a_forged_successor_cannot_substitute_a_smaller_unrelated_value() {
    let mut elaboration = ranked(false, false, false);
    let machine = &mut elaboration.module.machines[0];
    let zero = machine
        .operations()
        .find_map(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0)
                }
            )
            .then(|| operation.result.scalar_ref().unwrap().id)
        })
        .unwrap();
    let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
        unreachable!()
    };
    let cycle = &mut cycles[0];
    let last = cycle
        .edges
        .iter()
        .position(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .unwrap();
    let old = cycle.edges[last].successor_ranks;
    cycle.observations.get_mut(old.start()).value = zero;
    let refusal =
        discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap_err();
    assert!(
        refusal.detail.contains("successor rank") || refusal.detail.contains("actual scope"),
        "{}",
        refusal.detail
    );
}

fn observed_verdicts(results: &[Result<RouteVerdict, crate::CompilationRefusal>]) -> Vec<String> {
    results.iter().map(|result| match result {
        Ok(RouteVerdict::Accepted(_)) => String::from("accepted"),
        Ok(RouteVerdict::FallBack(refusal)) => format!("unsupported: {}", refusal.detail),
        Err(crate::CompilationRefusal::Elaboration(refusal)) => format!("elaboration refusal: {}", refusal.detail),
        Ok(RouteVerdict::Rejected { formations, violations, unproved, unestablished, conflicts, exceeded }) => format!(
            "rejected: formations={formations:?}, violations={}, unproved={:?}, unestablished={}, conflicts={}, exceeded={}",
            violations.len(), unproved.iter().map(|row| &row.kind).collect::<Vec<_>>(), unestablished.len(), conflicts.len(), exceeded.len(),
        ),
        other => format!("{other:?}"),
    }).collect()
}

#[test]
fn an_authored_ranking_lookalike_grants_no_theory_and_cannot_change_a_default() {
    let source = |definition: &str, view: &str| {
        format!(
            r#"
{definition}
machine countdown(n: u32) -> u32 terminates by n {view} crashes none {{
    transition n > 0 {{ true -> countdown(n - 1) false -> 37 }}
}}
machine entry() -> u32 terminates crashes none {{ countdown(4) }}
"#
        )
    };
    let lookalike = "machine Nat::Descending(value: u64) -> bool { false }";
    let verdicts = super::boundary_calls::compilation_results_of(&typed_with_ranking_catalog(
        &source(lookalike, "-> Nat::Descending"),
    ));
    // A unique authored declaration lacks catalog authority; an ambiguous
    // catalog/lookalike cohort retains no selection. Neither grants a theory.
    assert!(
        verdicts.iter().any(|verdict| matches!(verdict,
            Err(crate::CompilationRefusal::Elaboration(refusal)) | Ok(RouteVerdict::FallBack(refusal))
                if matches!(refusal.detail.as_str(),
                    "contract selected ranking view is not an admitted compiler ranking declaration while reading the body of `countdown`"
                        | "contract ranking view has no admitted resolved selection while reading the body of `countdown`")
        )),
        "an authored bool function must not select the closed integer theory: {:?}",
        observed_verdicts(&verdicts)
    );
    let default = super::boundary_calls::compilation_results_of(&typed_with_ranking_catalog(
        &source(lookalike, ""),
    ));
    assert!(
        default
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "the carrier default must not depend on an authored same-named function: {:?}",
        observed_verdicts(&default)
    );
}

#[test]
fn selected_ranking_views_check_their_own_subject_and_catalog_header() {
    let scalar_length = typed_with_ranking_catalog(
        "machine entry(n: u32) terminates by n -> Slice::Length crashes none {}",
    );
    let verdicts = super::boundary_calls::compilation_results_of(&scalar_length);
    assert!(verdicts.iter().any(|verdict| matches!(verdict,
        Err(crate::CompilationRefusal::Elaboration(refusal)) | Ok(RouteVerdict::FallBack(refusal)) if refusal.detail.contains("non-collection storage")
    )), "an acyclic clause still owes collection formation: {:?}", observed_verdicts(&verdicts));
    for (text, diagnostic) in [
        (
            "machine entry(n: u128) terminates by n -> Nat::Descending crashes none {}",
            "cannot embed exactly",
        ),
        (
            "machine entry(n: i32) terminates by n -> Nat::Descending crashes none {}",
            "unsigned integer subject",
        ),
        (
            "machine entry(n: u32, m: u32) terminates by (n, m) -> Nat::Descending crashes none {}",
            "exactly one subject",
        ),
    ] {
        let program = typed_with_ranking_catalog(text);
        let verdicts = super::boundary_calls::compilation_results_of(&program);
        assert!(verdicts.iter().any(|verdict| matches!(verdict,
            Err(crate::CompilationRefusal::Elaboration(refusal)) | Ok(RouteVerdict::FallBack(refusal)) if refusal.detail.contains(diagnostic)
        )), "a selected view still owes its argument formation: {text}: {:?}", observed_verdicts(&verdicts));
    }
    let mut program = typed_with_ranking_catalog(
        "machine entry(n: u32) terminates by n -> Nat::Descending crashes none {}",
    );
    let declaration = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Nat::Descending")
        .unwrap()
        .clone();
    program.machine_states_mut(&declaration)[0].parameters = Default::default();
    let verdicts = super::boundary_calls::compilation_results_of(&program);
    assert!(verdicts.iter().any(|verdict| matches!(verdict,
        Err(crate::CompilationRefusal::Elaboration(refusal)) | Ok(RouteVerdict::FallBack(refusal)) if refusal.detail.contains("exactly one subject")
    )), "sealed source custody cannot hide a changed selected header: {:?}", observed_verdicts(&verdicts));
}

#[test]
fn an_acyclic_ranking_expression_still_proves_its_subtraction_is_defined() {
    for (contract, actual, accepted) in [("requires n > 0", 1, true), ("", 0, false)] {
        let text = format!(
            r#"
public machine measured(n: u32)
terminates by n - 1 -> Nat::Descending
{contract}
crashes none {{}}
machine entry() crashes none {{ measured({actual}); }}
"#
        );
        let program = typed_with_ranking_catalog(&text);
        let verdicts = super::boundary_calls::compilation_results_of(&program);
        assert!(!verdicts.is_empty());
        if accepted {
            assert!(
                verdicts
                    .iter()
                    .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
                "a positive bound forms the exact n - 1 measure: {:?}",
                observed_verdicts(&verdicts)
            );
        } else {
            assert!(verdicts.iter().any(|verdict| matches!(verdict,
                Ok(RouteVerdict::Rejected { unproved, .. }) if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
            )), "an acyclic measure cannot hide unproved exact subtraction: {:?}", observed_verdicts(&verdicts));
        }
    }
}

#[test]
fn a_slice_loop_must_shrink_the_slice_it_actually_passes() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/views/ranked_slice_iteration/main.omg"
    ));
    let source = original.replace("checksum(bytes[1..],", "checksum(bytes,");
    assert_ne!(source, original);
    let source = format!("{source}\nmachine entry() crashes Trap {{ main(); }}\n");
    let results =
        super::boundary_calls::compilation_results_of(&typed_with_ranking_catalog(&source));
    assert!(results.iter().any(|result| matches!(result,
        Ok(RouteVerdict::Rejected { unproved, .. }) if unproved.iter().any(|row| matches!(row.kind, ObligationKind::RankComparison { .. }))
    ) || matches!(result,
        Ok(RouteVerdict::FallBack(refusal)) | Err(crate::CompilationRefusal::Elaboration(refusal))
        if refusal.detail.contains("removing strict rank edges")
    )), "passing the unchanged slice cannot prove descent: {:?}", observed_verdicts(&results));
}
