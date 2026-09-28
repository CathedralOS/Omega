use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{checked_program_result, typed_program};

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
            let typed = typed_program(&source);
            let machine = &typed.machines()[0];
            let components = crate::checks::termination::proven_nat_countdown_sccs(&typed, machine)
                .unwrap_or_else(|| panic!("{source}: retained countdown evidence"));
            assert_eq!(components.len(), 1);
            assert_eq!(components[0].covered_cyclic_edges.len(), 1);
            assert_eq!(
                components[0].covered_cyclic_edges[0].statement_ordinal,
                u32::from(first_truth != recursive_truth)
            );
            lower_typed_trees(typed, &CheckingRequest::settled())
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
            checked_program_result(&source)
                .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

/// A cyclic state may rename and reorder the ranked input — `iterate` binds
/// the authored `remaining` to its third formal `pending`. The retained
/// countdown evidence and the shared natural ranks must name the carried
/// formal, not a name twin of the authored subject.
#[test]
fn renamed_state_formal_carries_the_ranked_entry_role() {
    let source = r#"
        machine walk(remaining: u64, cap: u64, divisor: u64)
            requires 0 <= remaining && remaining <= 5
                && 0 <= cap && cap <= 20
                && 1 <= divisor && divisor <= 5
        terminates by remaining in 0..(cap % divisor + 6);
        -> u64 {
            transition { _ -> iterate(divisor, cap, remaining) }
            state iterate(width: u64, limit: u64, pending: u64)
                requires 1 <= width && width <= 5
                    && 0 <= limit && limit <= 20
                    && 0 <= pending && pending <= 5
            {
                transition pending > 0 {
                    true -> iterate(width - 0, limit - 0, pending - 1)
                    false -> pending
                }
            }
        }
    "#;
    let typed = typed_program(source);
    let machine = &typed.machines()[0];
    let iterate = &typed.machine_states(machine)[1];
    let (pending_position, pending) = typed
        .state_parameters(iterate)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.name.as_str() == "pending")
        .map(|(position, parameter)| (position, parameter.symbol))
        .expect("iterate binds pending");

    let components = crate::checks::termination::proven_nat_countdown_sccs(&typed, machine)
        .expect("renamed carrier retains countdown evidence");
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].header_state, iterate.symbol);
    assert_eq!(
        components[0].header_rank_parameter_position as usize, pending_position,
        "the header coordinate names `pending`, the carried formal"
    );
    assert_eq!(components[0].covered_cyclic_edges.len(), 1);
    let edge = &components[0].covered_cyclic_edges[0];
    assert_eq!(edge.source_state, iterate.symbol);
    assert_eq!(
        edge.source_rank_parameter_position as usize,
        pending_position
    );
    assert_eq!(
        edge.target_rank_parameter_position as usize,
        pending_position
    );

    let ranks = crate::checks::termination::proven_state_natural_ranks_with_call_frames(
        &typed, machine, None,
    )
    .expect("the shared natural ranks publish the carried formal");
    let rank = ranks
        .iter()
        .find(|rank| rank.state == iterate.symbol)
        .expect("iterate carries a natural rank");
    assert_eq!(rank.parameter, pending);
    assert_eq!(rank.parameter_position as usize, pending_position);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
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
    let diagnostics = checked_program_result(source)
        .expect_err("nonzero signed input does not establish positive Nat descent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ranking")),
        "{diagnostics:#?}"
    );
}
