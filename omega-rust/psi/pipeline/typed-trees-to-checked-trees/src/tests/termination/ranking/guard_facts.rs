use super::*;

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn countdown_evidence_preserves_both_boolean_arm_orders_and_base_case_polarities() {
    for (predicate, recursive_truth) in [
        ("remaining > 0", true),
        ("remaining == 0", false),
        ("remaining < 1", false),
        ("remaining <= 0", false),
        ("0 < remaining", true),
        ("!(remaining > 0)", false),
    ] {
        for first_truth in [false, true] {
            let target = |truth| {
                if truth == recursive_truth {
                    "value(remaining - 1)"
                } else {
                    "0u32"
                }
            };
            let source = format!(
                "machine value(remaining: u32) -> u32
                 terminates by remaining -> Nat::Descending;
                 {{ transition {predicate} {{ {first_truth} -> {} {} -> {} }} }}",
                target(first_truth),
                !first_truth,
                target(!first_truth),
            );
            let typed = typed_source(&source);
            let machine = &typed.machines()[0];
            let components = crate::checks::termination::proven_nat_countdown_sccs(&typed, machine)
                .unwrap_or_else(|| panic!("{source}: retained countdown evidence"));
            assert_eq!(components.len(), 1);
            assert_eq!(components[0].covered_cyclic_edges.len(), 1);
            assert_eq!(
                components[0].covered_cyclic_edges[0].statement_ordinal,
                u32::from(first_truth != recursive_truth)
            );
            lower_typed_trees(typed)
                .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn distance_ranking_preserves_both_boolean_arm_orders() {
    for (predicate, recursive_truth) in [("index < limit", true), ("index >= limit", false)] {
        for first_truth in [false, true] {
            let target = |truth| {
                if truth == recursive_truth {
                    "value(limit, index + 1)"
                } else {
                    "index"
                }
            };
            let source = format!(
                "machine value(limit: u64, index: u64) -> u64
                 terminates by (index, limit) -> Nat::BoundedDistance;
                 {{ transition {predicate} {{ {first_truth} -> {} {} -> {} }} }}",
                target(first_truth),
                !first_truth,
                target(!first_truth),
            );
            lower_typed_trees(typed_source(&source))
                .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn mutual_ranking_uses_each_edges_own_failed_guard() {
    let source = r#"
        machine value(remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                false -> 0u64
                true -> other(remaining - 1)
            }
            state other(remaining: u64) -> u64 {
                transition remaining == 0 {
                    true -> 0u64
                    false -> value(remaining - 1)
                }
            }
        }
    "#;
    lower_typed_trees(typed_source(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let stalled = source.replace("false -> value(remaining - 1)", "false -> value(remaining)");
    let diagnostics =
        lower_typed_trees(typed_source(&stalled)).expect_err("every mutual edge must decrease");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ranking")),
        "{diagnostics:#?}"
    );
}

#[test]
fn one_decreasing_self_edge_cannot_cover_an_unchanged_fallback_edge() {
    let source = r#"
        machine value(remaining: u32) -> u32
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> value(remaining - 1)
                false -> value(remaining)
            }
        }
    "#;
    let typed = typed_source(source);
    assert!(
        crate::checks::termination::proven_nat_countdown_sccs(&typed, &typed.machines()[0])
            .is_none()
    );
    let diagnostics = lower_typed_trees(typed).expect_err("unchanged fallback must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ranking")),
        "{diagnostics:#?}"
    );
}

#[test]
fn signed_nonzero_does_not_prove_a_positive_countdown_rank() {
    let source = r#"
        machine value(remaining: i32 in Wrapping) -> i32 in Wrapping
        terminates by remaining -> Nat::Descending;
        {
            transition remaining == 0 {
                true -> 0i32
                false -> value(remaining - 1)
            }
        }
    "#;
    let diagnostics = lower_typed_trees(typed_source(source))
        .expect_err("nonzero signed input does not establish positive Nat descent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ranking")),
        "{diagnostics:#?}"
    );
}
