use super::{ObligationId, Proposition, ScalarBlockInvariantArrival, restore_seeds};
use crate::TerminalMachineSelection;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use terminal_psi::ScalarBlockInvariant;

fn seed(header: u64, obligation: u64) -> ScalarBlockInvariant {
    ScalarBlockInvariant {
        machine: MachineId::new(1).unwrap(),
        header: BlockId::new(header).unwrap(),
        predicate: Proposition::Truth,
        arrivals: vec![ScalarBlockInvariantArrival {
            edge: EdgeId::new(header).unwrap(),
            obligation: ObligationId::new(obligation).unwrap(),
        }],
    }
}

#[test]
fn rejected_expansion_restores_all_seed_rows_and_arrival_identities_once() {
    let original = vec![seed(2, 7), seed(3, 8)];
    let mut seeds = Some(original.clone());
    let mut expanded = original.clone();
    expanded[0].predicate = Proposition::Falsehood;
    expanded.remove(1);
    expanded.push(seed(4, 9));
    assert!(restore_seeds(&mut expanded, &mut seeds));
    assert_eq!(expanded, original);
    assert!(seeds.is_none());
    // The next proof/drop round may reject an original seed. Restoration must
    // not resurrect it or restart discovery indefinitely.
    expanded.remove(0);
    assert!(!restore_seeds(&mut expanded, &mut seeds));
    assert_eq!(expanded, original[1..]);
}

#[test]
fn unchanged_seed_roster_is_not_a_new_retry() {
    let mut original = vec![seed(2, 7)];
    let mut seeds = Some(original.clone());
    assert!(!restore_seeds(&mut original, &mut seeds));
    assert!(seeds.is_none());
}

fn guarded_join_source(right_reason: i32) -> lowered_psi::LoweredPsi {
    let source = format!(
        "machine root(flag: bool, count: u64) -> i32 {{
            transition flag {{ true -> report(10, count) false -> report({right_reason}, count) }}
            state report(reason: i32 [0..=100], count: u64) -> i32 {{
                transition count <= 4 {{ true -> finish(reason, count) false -> 99 }}
            }}
            state finish(reason: i32 [0..=100], count: u64 [0..=4]) -> i32 {{
                reason + count as i32
            }}
        }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    crate::machine_lowering::lower_machine(&checked, TerminalMachineSelection::Name("root"))
        .unwrap()
}

#[test]
fn guarded_operation_demands_retain_exact_join_arrival_proofs() {
    for reason in [20, 100] {
        let lowered = guarded_join_source(reason);
        assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn guarded_operation_demands_do_not_authorize_overflowing_arrivals_or_missing_guards() {
    let original = guarded_join_source(20);
    for remove_guard in [false, true] {
        let mut changed = original.clone();
        changed.semantic_module.scalar_block_invariants.clear();
        changed.proof_bundle = terminal_verifier::ProofBundle::default();
        let operation = changed
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(operation.kind,
                    terminal_psi::OperationKind::IntegerConstant { value }
                        if value == if remove_guard {
                            semantic_vocabulary::IntegerValue::Unsigned(4)
                        } else {
                            semantic_vocabulary::IntegerValue::Signed(20)
                        }
                )
            })
            .unwrap();
        operation.kind = terminal_psi::OperationKind::IntegerConstant {
            value: if remove_guard {
                semantic_vocabulary::IntegerValue::Unsigned(u64::MAX.into())
            } else {
                semantic_vocabulary::IntegerValue::Signed(i32::MAX.into())
            },
        };
        // This changes Terminal semantics, not authored annotations: candidate
        // discovery must still prove every actual arrival and reject the now
        // unsafe addition/cast instead of importing the original source bounds.
        assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
    }
}
