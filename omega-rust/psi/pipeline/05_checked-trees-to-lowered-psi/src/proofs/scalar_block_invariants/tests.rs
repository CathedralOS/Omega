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
    let checked = crate::front_end::checked_program(&source);
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

/// A loop over storage, not state parameters: `self.i` indexes the array and
/// `self.d` divides, so each operation's question names a field read whose
/// bound only the loop header can carry.
const FIELD_COUNTER: &str = "
    data Poly { coeffs: [i32; 5] in Wrapping; i: i32; r: i32 in Wrapping; }
    machine Poly::run(&mut self) {
        self.i = 2;
        transition { _ -> eval() }
        state eval(&mut self) {
            transition self.i < 5 { true -> step() _ -> done() }
        }
        state step(&mut self) {
            self.r = self.r + self.coeffs[self.i];
            self.i = self.i + 1;
            transition { _ -> eval() }
        }
        state done(&mut self) { }
    }";

const FIELD_DIVISOR: &str = "
    data Trial { n: i32; d: i32; }
    machine Trial::run(&mut self) {
        self.n = 169;
        self.d = 2;
        transition { _ -> probe() }
        state probe(&mut self) {
            transition self.d >= 0 && self.d <= 200 { true -> square() _ -> done() }
        }
        state square(&mut self) {
            transition self.d * self.d > self.n { true -> done() _ -> test() }
        }
        state test(&mut self) {
            transition self.n % self.d == 0 { true -> done() _ -> advance() }
        }
        state advance(&mut self) {
            transition self.d < 200 { true -> bump() _ -> done() }
        }
        state bump(&mut self) {
            self.d = self.d + 1;
            transition { _ -> probe() }
        }
        state done(&mut self) { }
    }";

fn lower_field_loop(source: &str, machine: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    let lowered =
        crate::machine_lowering::lower_machine(&checked, TerminalMachineSelection::Name(machine))
            .unwrap_or_else(|error| panic!("{machine} lowers: {error:?}"));
    assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{machine} verifies: {error:?}"));
    lowered
}

/// Replace the loop's stored start value and drop every proposal and proof,
/// so the header invariant must be rediscovered against the changed entry.
fn restart_field_loop(lowered: &lowered_psi::LoweredPsi, start: i128, changed_start: i128) {
    let mut changed = lowered.clone();
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
                    if value == semantic_vocabulary::IntegerValue::Signed(start))
        })
        .expect("the loop stores its start value");
    operation.kind = terminal_psi::OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Signed(changed_start),
    };
    assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
}

#[test]
fn a_field_counter_bounds_its_index_through_the_loop_header() {
    // The index question is stated in mathematical integers (`0 <= i`); the
    // header carries it as the typed bound over the stored field.
    let lowered = lower_field_loop(FIELD_COUNTER, "Poly::run");
    // Starting below zero breaks the bound at entry, so no invariant can be
    // retained and the index keeps its unanswered question.
    restart_field_loop(&lowered, 2, -3);
}

#[test]
fn a_decreasing_field_counter_bounds_its_index_through_the_loop_header() {
    // The guard bounds the counter from below. The upper bound the `as u64`
    // index needs is the invariant the entry store establishes and each
    // decrement preserves; the header's `0 <= i` guess fails at the exit
    // arrival and must not take that invariant down with it.
    let source = "
        data Rev { nums: [i32; 5] in Wrapping; i: i32; acc: i32 in Wrapping; }
        machine Rev::run(&mut self) {
            self.i = 4;
            transition { _ -> head() }
            state head(&mut self) {
                transition self.i >= 0 { true -> add() _ -> done() }
            }
            state add(&mut self) {
                self.acc = self.acc + self.nums[self.i];
                self.i = self.i - 1;
                transition { _ -> head() }
            }
            state done(&mut self) { }
        }";
    let lowered = lower_field_loop(source, "Rev::run");
    // Starting past the array's end breaks the upper bound at entry.
    restart_field_loop(&lowered, 4, 5);
}

#[test]
fn a_wrapping_field_counter_bounds_its_index_through_the_loop_header() {
    // `self.i + 1` in Wrapping is a signed wrapping sum: the header's
    // `0 <= i` survives the backedge only with the guard's no-wrap headroom.
    let source = FIELD_COUNTER.replace("i: i32;", "i: i32 in Wrapping;");
    let lowered = lower_field_loop(&source, "Poly::run");
    restart_field_loop(&lowered, 2, -3);
}

#[test]
fn a_guarded_field_divisor_discharges_the_remainder_it_feeds() {
    // The remainder asks `d <= -2 || 1 <= d || (d <= -1 && ...)`; the header
    // guard `0 <= d` refutes all but `1 <= d`, which only value transport
    // proves once the disjunction is split.
    let lowered = lower_field_loop(FIELD_DIVISOR, "Trial::run");
    // A zero start satisfies the header guard but not `1 <= d`.
    restart_field_loop(&lowered, 2, 0);
}
