#[path = "owned_scalar_cycles/execution.rs"]
mod execution;
#[path = "owned_scalar_cycles/mutations.rs"]
mod mutations;
#[path = "owned_scalar_cycles/support.rs"]
mod support;

use terminal_psi::{OperationKind, TerminalNaturalRankComparison, TerminalRankedScc};

// Exact rankflow customer, including its ordinary affine data declaration.
const CUSTOMER: &str = r#"machine reset(value: &mut u64) -> u64 { value = 0; 0 }

data Limits {
    limit: u64;
    divisor: u64 [3..=5];
}

machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
}
"#;

const RANK_CLAUSE: &str =
    "terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);\n";

fn field_read_customer() -> String {
    CUSTOMER
        .replace(
            "    let mut scratch",
            "    let observed: u64 = limits.limit;\n    let mut scratch",
        )
        .replace("false -> remaining", "false -> observed")
}

#[test]
fn authored_walk_publishes_a_natural_cycle_with_a_loop_carried_rank() {
    let (module, proof, _, _) = support::publish(CUSTOMER);
    let walk = support::walk(&module);
    let Some(TerminalRankedScc::Natural(components)) = &walk.ranked_scc else {
        panic!("the actual cyclic scalar graph needs Natural evidence, not a countdown shortcut");
    };
    assert_eq!(components.len(), 1);
    let component = &components[0];
    assert_eq!(component.rank_type, support::unsigned_type());
    assert_eq!(proof.control_cycles.len(), 1);
    let verified = terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    assert!(
        support::has_cycle(walk),
        "actual successor topology is cyclic"
    );

    let decrement = support::operation(walk, |kind| {
        matches!(kind, OperationKind::ExactIntegerSubtract { .. })
    });
    let OperationKind::ExactIntegerSubtract { left, right, .. } = decrement.kind else {
        unreachable!();
    };
    support::assert_constant(walk, right, 1);
    let decrement_value = decrement.result.expect_scalar().id;
    let remaining = walk.parameters[0].id;
    let expected_origins = std::collections::BTreeSet::from([remaining, decrement_value]);
    assert_eq!(support::scalar_origins(walk, left), expected_origins);
    let mut carried_parameters = 0;
    for rank in &component.ranks {
        let block = walk
            .blocks
            .iter()
            .find(|block| block.id == rank.block)
            .unwrap();
        let origins = support::scalar_origins(walk, rank.value);
        assert!(
            origins.is_subset(&expected_origins),
            "rank is remaining, not marker or a preheader constant"
        );
        if block
            .parameters
            .iter()
            .any(|parameter| parameter.id == rank.value)
        {
            assert!(
                origins.contains(&decrement_value),
                "rank parameter receives the decremented backedge actual"
            );
            carried_parameters += 1;
        }
    }
    assert!(
        carried_parameters > 0,
        "not just a rank on invocation-entry SSA"
    );
    let strict = component
        .edges
        .iter()
        .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .collect::<Vec<_>>();
    assert!(!strict.is_empty());
    for edge in strict {
        assert!(support::scalar_origins(walk, edge.successor_rank).contains(&decrement_value));
        let (target, arguments, _) = support::successor(walk, edge.edge);
        assert_eq!(target, edge.target);
        let target_rank = component
            .ranks
            .iter()
            .find(|rank| rank.block == target)
            .unwrap();
        let target_block = walk.blocks.iter().find(|block| block.id == target).unwrap();
        if let Some(position) = target_block
            .parameters
            .iter()
            .position(|parameter| parameter.id == target_rank.value)
        {
            assert_eq!(
                arguments[position], edge.successor_rank,
                "rank substitution follows this exact successor"
            );
        }
    }
}

#[test]
fn source_without_rank_keeps_the_same_authored_cycle_without_termination_evidence() {
    assert_eq!(CUSTOMER.matches(RANK_CLAUSE).count(), 1);
    let unranked = CUSTOMER.replacen(RANK_CLAUSE, "", 1);
    let (module, proof, semantic_bytes, proof_bytes) = support::publish(&unranked);
    assert!(support::has_cycle(support::walk(&module)));
    assert!(
        module
            .machines
            .iter()
            .all(|machine| machine.ranked_scc.is_none())
    );
    assert!(
        proof.control_cycles.is_empty(),
        "partial correctness must not manufacture termination"
    );
    execution::execute_cases(&module, &semantic_bytes, &proof_bytes);
}
