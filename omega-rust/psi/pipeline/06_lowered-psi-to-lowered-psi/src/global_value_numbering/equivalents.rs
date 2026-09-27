//! Duplicate removal across dominator-equivalent scalar computations.
//!
//! Scanning in reverse postorder meets every dominating definition before the
//! blocks it dominates, so a single pass suffices: each eligible operation's
//! operands are first resolved through duplicates already collapsed to their
//! canonical survivors, then matched against the surviving operations seen so
//! far. The canonical survivor is the first match in scan order, and a
//! duplicate's uses substitute the surviving result once — survivors are never
//! remapped, so the substitution is never transitive.
//!
//! Blocks unreachable from the entry never enter the scan order and are left
//! untouched; module validation already rejects them on the public entrance.

use crate::retained_identities::proof_values::{
    call_erased_arguments, crash_continuations, retain_crash_routes, retain_erased_arguments,
    retain_proposition,
};
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind, TerminalMachine, Terminator, ValueDeclaration};

/// One surviving computation later duplicates may equal: the operation kind
/// with operands resolved through already-collapsed duplicates, the block that
/// must dominate a candidate, and the result declaration that must match.
struct Leader {
    kind: OperationKind,
    block: BlockId,
    result: ValueDeclaration,
}

pub(super) fn deduplicate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &BTreeSet<ValueId>,
) {
    // Ranking evidence names exact value identities over the covered cyclic
    // components: it is proof and termination custody, not a use list the
    // substitution can rewrite. Covered member blocks keep their exact
    // contents; duplicates outside them still collapse.
    let coverage = crate::retained_identities::ranked_coverage::ranked_coverage(machine);
    let mut retained_values = retained_values.clone();
    retained_values.extend(coverage.values.iter().copied());
    for proposition in &machine.contract.requires {
        retain_proposition(proposition, &mut retained_values);
    }
    for clause in &machine.contract.ensures {
        retain_proposition(&clause.proposition, &mut retained_values);
    }
    for guarantee in &machine.contract.outcome_specific_ensures {
        retain_proposition(&guarantee.proposition, &mut retained_values);
    }
    retain_crash_routes(&machine.contract.crash_routes, &mut retained_values);
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    for operation in &operations {
        retain_crash_routes(crash_continuations(&operation.kind), &mut retained_values);
        retain_erased_arguments(call_erased_arguments(&operation.kind), &mut retained_values);
    }
    for block in &machine.blocks {
        if let Terminator::Crash { site_guard, .. } = &block.terminator {
            for term in site_guard {
                retain_proposition(term.proposition(), &mut retained_values);
            }
        }
    }
    for occurrence in source_calls {
        if operations
            .iter()
            .any(|operation| operation.id == occurrence.terminal_operation)
        {
            retained_values.extend(
                occurrence
                    .source_values_before_call
                    .iter()
                    .map(|value| value.id),
            );
        }
    }
    let order = reverse_postorder(machine);
    let dominators = dominators(machine);
    let mut representative: BTreeMap<ValueId, ValueId> = BTreeMap::new();
    let mut removed = BTreeSet::new();
    let mut leaders: Vec<Leader> = Vec::new();
    for block_id in order {
        let Some(block) = machine.blocks.iter().find(|block| block.id == block_id) else {
            continue;
        };
        for operation in &block.operations {
            let Some(result) = operation.result.scalar() else {
                continue;
            };
            // A static reach binder position is semantic call evidence carried
            // by the operation row; removal would orphan it.
            if operation.static_reach_binding.is_some()
                || !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
            {
                continue;
            }
            let mut kind = operation.kind.clone();
            kind.map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
            // A value named by proof or custody sidecars keeps its own
            // identity; it may still canonicalize a later duplicate. A
            // covered member's operation stays exact, and a result used
            // inside a member block is never substituted away.
            if !retained_values.contains(&result.id)
                && !coverage.blocks.contains(&block_id)
                && !coverage.member_uses.contains(&result.id)
                && let Some(leader) = leaders.iter().find(|leader| {
                    leader.kind == kind
                        && dominators
                            .get(&block_id)
                            .is_some_and(|blocks| blocks.contains(&leader.block))
                        && leader.result.scalar_type == result.scalar_type
                        && leader.result.qualifications == result.qualifications
                })
            {
                representative.insert(result.id, leader.result.id);
                removed.insert(operation.id);
                continue;
            }
            leaders.push(Leader {
                kind,
                block: block_id,
                result,
            });
        }
    }
    if removed.is_empty() {
        return;
    }
    for block in &mut machine.blocks {
        block
            .operations
            .retain(|operation| !removed.contains(&operation.id));
        for operation in &mut block.operations {
            operation
                .kind
                .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
        }
        block
            .terminator
            .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
    }
}

/// Reachable blocks in reverse postorder: every dominator precedes each block
/// it dominates, so the leader scan meets a dominating definition first.
fn reverse_postorder(machine: &TerminalMachine) -> Vec<BlockId> {
    let successors = machine
        .blocks
        .iter()
        .map(|block| {
            let targets = match &block.terminator {
                Terminator::Jump { target, .. } => vec![*target],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.target).collect()
                }
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnUnitPartialAffine { .. }
                | Terminator::ReturnUnitNominalAffine { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. } => Vec::new(),
            };
            (block.id, targets)
        })
        .collect::<BTreeMap<_, _>>();
    let mut entered = BTreeSet::from([machine.entry]);
    let mut finished = Vec::new();
    let mut pending = vec![(machine.entry, 0usize)];
    while let Some((block, position)) = pending.last_mut() {
        let Some(target) = successors
            .get(block)
            .and_then(|targets| targets.get(*position))
            .copied()
        else {
            finished.push(*block);
            pending.pop();
            continue;
        };
        *position += 1;
        if entered.insert(target) {
            pending.push((target, 0));
        }
    }
    finished.reverse();
    finished
}

/// Dominator sets over the reachable graph: a block dominates another only
/// when every path from the entry passes through it.
fn dominators(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let blocks = machine
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut predecessors = blocks
        .iter()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in targets {
            if let Some(incoming) = predecessors.get_mut(&target) {
                incoming.insert(block.id);
            }
        }
    }
    let mut dominators = blocks
        .iter()
        .map(|block| {
            (
                *block,
                if *block == machine.entry {
                    BTreeSet::from([*block])
                } else {
                    blocks.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in blocks
            .iter()
            .copied()
            .filter(|block| *block != machine.entry)
        {
            let Some(incoming) = predecessors.get(&block) else {
                continue;
            };
            let Some(first) = incoming.iter().next() else {
                continue;
            };
            let mut common = dominators
                .get(first)
                .cloned()
                .unwrap_or_else(|| BTreeSet::from([*first]));
            for predecessor in incoming.iter().skip(1) {
                let predecessor_dominators = dominators
                    .get(predecessor)
                    .cloned()
                    .unwrap_or_else(|| BTreeSet::from([*predecessor]));
                common.retain(|candidate| predecessor_dominators.contains(candidate));
            }
            common.insert(block);
            if dominators.get(&block) != Some(&common) {
                dominators.insert(block, common);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Machine-level boundary coverage for cases a validated module cannot
    //! express: ranking evidence, retained proof values, reach bindings, and
    //! result-declaration mismatches.
    use super::{
        BTreeSet, BlockId, OperationKind, TerminalMachine, Terminator, ValueDeclaration, ValueId,
        deduplicate,
    };
    use lowered_psi::LoweredSourceCallOccurrence;
    use semantic_vocabulary::{
        ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
        OperationId, Proposition, ScalarTerm, ScalarType, StructuralCaseId, StructuralTypeId,
    };
    use terminal_psi::{
        Block, ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
        MachineContract, Operation, OperationResult, OutcomeSpecificEnsure, OutcomeSpecificGuard,
        TerminalMachineResult,
    };

    fn i32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    }

    fn i64_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
    }

    fn i32(ordinal: u64) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: i32_type(),
        }
    }

    fn machine(blocks: Vec<Block>) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: vec![i32(1), i32(2)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(i32(9)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks,
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn block(ordinal: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(ordinal).unwrap(),
            parameters: Vec::new(),
            operations,
            terminator,
        }
    }

    fn constant(ordinal: u64, result: u64, value: i128) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(ordinal).unwrap(),
            result: OperationResult::Scalar(i32(result)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(value),
            },
        }
    }

    fn return_value(ordinal: u64, value: u64) -> Terminator {
        Terminator::Return {
            edge: EdgeId::new(ordinal).unwrap(),
            value: ValueId::new(value).unwrap(),
            cleanup_actions: Vec::new(),
        }
    }

    #[test]
    fn equal_kind_with_different_result_type_is_not_a_duplicate() {
        let mut machine = machine(vec![block(
            1,
            vec![
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(10).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: ValueId::new(10).unwrap(),
                        scalar_type: i64_type(),
                    }),
                    kind: OperationKind::IntegerWiden {
                        operand: ValueId::new(1).unwrap(),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(11).unwrap(),
                    result: OperationResult::Scalar(i32(11)),
                    kind: OperationKind::IntegerWiden {
                        operand: ValueId::new(1).unwrap(),
                    },
                },
            ],
            return_value(1, 10),
        )]);
        deduplicate(&mut machine, &[], &BTreeSet::new());
        assert_eq!(
            machine.blocks[0].operations.len(),
            2,
            "a narrower result declaration is a different value"
        );
    }

    fn equal_v11_seven() -> Proposition {
        Proposition::Equal(
            ScalarTerm::Value {
                id: ValueId::new(11).unwrap(),
                scalar_type: i32_type(),
            },
            ScalarTerm::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(7),
            },
        )
    }

    /// Three equal constants in one block: `v11` is the duplicate a carrier
    /// names, `v12` is the unnamed duplicate that still collapses to `v10`
    /// and supplies the observable contrast.
    fn three_constant_machine() -> TerminalMachine {
        machine(vec![block(
            1,
            vec![
                constant(10, 10, 7),
                constant(11, 11, 7),
                constant(12, 12, 7),
            ],
            return_value(1, 12),
        )])
    }

    fn deduplication_keeps_named_result(machine: &mut TerminalMachine) {
        deduplicate(machine, &[], &BTreeSet::new());
        let remaining = machine.blocks[0]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>();
        assert!(
            remaining.contains(&OperationId::new(11).unwrap())
                && !remaining.contains(&OperationId::new(12).unwrap()),
            "the carrier-named duplicate keeps its identity: {remaining:?}"
        );
        let Terminator::Return { value, .. } = &machine.blocks[0].terminator else {
            panic!("entry keeps its return")
        };
        assert_eq!(
            *value,
            ValueId::new(10).unwrap(),
            "the unnamed duplicate substitutes the first surviving leader"
        );
    }

    #[test]
    fn contract_and_guard_propositions_retain_named_results() {
        // Positive: ensures clauses, outcome-specific guarantees, contract
        // crash-route predicates, operation crash continuations, and
        // crash-terminator site guards each keep the duplicate's producer.
        // Boundary: a `Truth` alternative retains nothing, so both duplicates
        // collapse.
        let mut ensures_machine = three_constant_machine();
        ensures_machine.contract.ensures.push(ContractClause {
            obligation: ObligationId::new(1).unwrap(),
            proposition: equal_v11_seven(),
        });
        deduplication_keeps_named_result(&mut ensures_machine);

        let mut outcome_machine = three_constant_machine();
        outcome_machine
            .contract
            .outcome_specific_ensures
            .push(OutcomeSpecificEnsure {
                guard: OutcomeSpecificGuard {
                    result_type: StructuralTypeId::new(1).unwrap(),
                    result_case: StructuralCaseId::new(1).unwrap(),
                },
                position: 0,
                obligation: ObligationId::new(2).unwrap(),
                proposition: equal_v11_seven(),
                evidence: None,
            });
        deduplication_keeps_named_result(&mut outcome_machine);

        let mut route_machine = three_constant_machine();
        route_machine.contract.crash_routes.push(CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                equal_v11_seven(),
            ))],
        });
        deduplication_keeps_named_result(&mut route_machine);

        let mut continuation_machine = three_constant_machine();
        continuation_machine.blocks[0].operations.insert(
            0,
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(20).unwrap(),
                result: OperationResult::Scalar(i32(20)),
                kind: OperationKind::Call {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    callee: MachineId::new(9).unwrap(),
                    arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                            equal_v11_seven(),
                        ))],
                    }],
                },
            },
        );
        deduplication_keeps_named_result(&mut continuation_machine);

        // The retention scan visits every block's terminator, so a crash site
        // outside the reachable order still retains the named producer.
        let mut guard_machine = three_constant_machine();
        guard_machine.blocks.push(block(
            2,
            vec![],
            Terminator::Crash {
                edge: EdgeId::new(2).unwrap(),
                cause: CrashCause::Trap,
                site_guard: vec![CrashPredicateTerm::new(equal_v11_seven())],
                frontier_lower_bound: Vec::new(),
            },
        ));
        deduplication_keeps_named_result(&mut guard_machine);

        let mut truth_machine = three_constant_machine();
        truth_machine.contract.crash_routes.push(CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        });
        deduplicate(&mut truth_machine, &[], &BTreeSet::new());
        assert_eq!(
            truth_machine.blocks[0].operations.len(),
            1,
            "a Truth alternative retains nothing"
        );
    }

    #[test]
    fn a_recorded_call_join_retains_named_results() {
        // Positive: a recorded source-call join keeps the producer of every
        // captured environment value. Boundary: a join naming an operation
        // absent from this machine demands nothing.
        let occurrence = |operation: u64| LoweredSourceCallOccurrence {
            source_site: None,
            source_state: symbols::SymbolHandle::from_arena_index(1),
            statement_index: 0,
            call_ordinal: 0,
            terminal_operation: OperationId::new(operation).unwrap(),
            source_target: symbols::SymbolHandle::from_arena_index(2),
            source_values_before_call: vec![i32(11)],
        };

        let mut present = three_constant_machine();
        deduplicate(&mut present, &[occurrence(10)], &BTreeSet::new());
        assert_eq!(
            present.blocks[0]
                .operations
                .iter()
                .map(|operation| operation.id)
                .collect::<Vec<_>>(),
            vec![OperationId::new(10).unwrap(), OperationId::new(11).unwrap()],
            "the joined capture keeps its producer"
        );

        let mut absent = three_constant_machine();
        deduplicate(&mut absent, &[occurrence(99)], &BTreeSet::new());
        assert_eq!(
            absent.blocks[0].operations.len(),
            1,
            "a join naming no operation in this machine retains nothing"
        );
    }
}
