use crate::TerminalMachineSelection;

const GUARDED_VIEW_LOOP: &str = "
    data Entry [copy] { value: i32 in Wrapping; }
    data Grid { entries: [Entry; 4]; }
    machine Grid::run(&mut self) -> i32 in Wrapping {
        let view: &[Entry] = self.entries.as_slice();
        transition view.len > 0 { true -> scan(view, 0, 0) false -> 0 }
        state scan(&mut self, entries: &[Entry], index: u64 in Wrapping,
                   total: i32 in Wrapping) -> i32 in Wrapping {
            let current: Entry = entries[index];
            let next_total: i32 in Wrapping = total + 1;
            let next_index: u64 in Wrapping = index + 1;
            let has_next: bool = next_index < entries.len;
            transition has_next {
                true -> scan(entries, next_index, next_total)
                false -> next_total
            }
        }
    }";

#[test]
fn guarded_loop_parameters_retain_operation_demands() {
    for source in [
        GUARDED_VIEW_LOOP.to_owned(),
        GUARDED_VIEW_LOOP.replace("index + 1", "index + 2"),
    ] {
        let checked = crate::front_end::checked_program(&source);
        let lowered = crate::machine_lowering::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Grid::run"),
        )
        .expect("each guarded arrival establishes the next read's index bound");
        assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the loop invariant and every operation replay independently");
    }
}

#[test]
fn a_loop_guard_that_admits_the_extent_does_not_prove_an_index_bound() {
    let source = GUARDED_VIEW_LOOP.replace("next_index < entries.len", "next_index <= entries.len");
    let diagnostics = crate::front_end::checked_program_result(&source)
        .expect_err("the source checker rejects the out-of-bounds arrival");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index"))
    );

    let checked = crate::front_end::checked_program(GUARDED_VIEW_LOOP);
    let mut changed = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Grid::run"),
    )
    .unwrap();
    let header = changed
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ElementViewRead { .. }
                )
            })
        })
        .expect("the loop body reads an element");
    let guard = header
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerLessThan { .. }
            )
        })
        .expect("the loop backedge has a strict bound");
    let terminal_psi::OperationKind::IntegerLessThan { left, right } = guard.kind else {
        unreachable!("selected a strict comparison");
    };
    guard.kind = terminal_psi::OperationKind::IntegerLessOrEqual { left, right };
    assert!(
        terminal_verifier::verify_module(
            &changed.semantic_module,
            &changed.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "old evidence must not authorize the changed arrival"
    );
    changed.semantic_module.scalar_block_invariants.clear();
    changed.proof_bundle = terminal_verifier::ProofBundle::default();
    assert!(
        crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err(),
        "a candidate bound is not authority when the backedge can invalidate it"
    );
}

#[test]
fn loop_parameter_demands_are_not_specific_to_indexing() {
    let checked = crate::front_end::checked_program(
        "
        data Calculator { result: u64 in Wrapping; }
        machine Calculator::run(&mut self) {
            transition { _ -> divide(4) }
            state divide(&mut self, divisor: u64 in Wrapping) {
                self.result = 100 / divisor;
                let next: u64 in Wrapping = divisor - 1;
                transition next > 0 {
                    true -> divide(next)
                    false -> done()
                }
            }
            state done(&mut self) { }
        }",
    );
    let lowered = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Calculator::run"),
    )
    .expect("the same all-arrival rule retains a nonzero divisor");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("division and induction obligations replay independently");
}

const CONTRACTED_FIELD_LOOP: &str = "
    data Grid where 0 <= bound, bound <= 2 { bound: u32; result: u32; }
    machine Grid::run(&mut self) {
        self.bound = 2;
        transition { _ -> row(0) }
        state row(&mut self, coordinate: u32)
            requires 0 <= coordinate && coordinate <= self.bound
        {
            self.result = coordinate * 4;
            transition coordinate + 1 <= self.bound {
                true -> row(coordinate + 1)
                false -> done()
            }
        }
        state done(&mut self) { }
    }";

#[test]
fn a_changing_storage_seed_does_not_discard_unrelated_loop_bounds() {
    let source =
        CONTRACTED_FIELD_LOOP.replace("self.bound = 2;", "self.bound = 2; self.result = 0;");
    let checked = crate::front_end::checked_program(&source);
    let lowered = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Grid::run"),
    )
    .expect("discarding result=0 must retain the authored contract and constant bound");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("every arrival still proves the retained invariant");
}

#[test]
fn authored_arrival_contracts_compose_with_initialized_loop_storage() {
    let checked = crate::front_end::checked_program(CONTRACTED_FIELD_LOOP);
    let lowered = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Grid::run"),
    )
    .expect("the coordinate contract and retained storage jointly bound the product");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay checks every arrival and arithmetic operation");

    let mut changed = lowered.clone();
    let value = changed
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::IntegerConstant { value }
                if *value == semantic_vocabulary::IntegerValue::Unsigned(2) =>
            {
                Some(value)
            }
            _ => None,
        })
        .expect("the bound initialization is explicit");
    *value = semantic_vocabulary::IntegerValue::Unsigned(u32::MAX.into());
    assert!(
        terminal_verifier::verify_module(
            &changed.semantic_module,
            &changed.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a changed initialization cannot reuse the induction evidence"
    );

    for rounds in [0, super::MAX_ROSTER_ROUNDS] {
        let mut changed = lowered.clone();
        changed.proof_bundle = terminal_verifier::ProofBundle::default();
        changed.semantic_module.scalar_block_invariants[0].predicate =
            semantic_vocabulary::Proposition::Falsehood;
        let mandatory = changed.semantic_module.scalar_block_invariants.clone();
        super::retain_provable_bounded(&mut changed, rounds).unwrap();
        assert!(
            super::preserves_mandatory_roster(
                &mandatory,
                &changed.semantic_module.scalar_block_invariants,
            ),
            "failed inference never deletes an authored requirement"
        );
        assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
    }
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

#[test]
fn bounded_wrapping_product_retains_digit_bound_across_a_join() {
    let source = r#"
        domain [u8; 1]::Utf8 requires valid_utf8(self);
        machine narrow(value: i32) -> u8 { (value as u8 in Trapping) as u8 }
        data Product { text: [u8; 1] in Utf8; tens: i32 in Wrapping; byte: u8; }
        machine Product::run(&mut self, left: i32 [1..=9], right: i32 [1..=9]) {
            self.text = " ";
            self.tens = (left as i32 in Wrapping) * (right as i32 in Wrapping);
            self.tens = self.tens / 10;
            transition self.tens > 0 { true -> digit() _ -> space() }
            state digit(&mut self) {
                self.byte = narrow((self.tens + 48) as i32);
                transition { _ -> write() }
            }
            state space(&mut self) {
                self.byte = 32;
                transition { _ -> write() }
            }
            state write(&mut self) { self.text[0] = self.byte; }
        }
    "#;
    let lowered = lower_field_loop(source, "Product::run");
    let mut changed = lowered.clone();
    changed.semantic_module.scalar_block_invariants.clear();
    changed.proof_bundle = terminal_verifier::ProofBundle::default();
    let alternate = changed
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::IntegerConstant { value }
                if *value == semantic_vocabulary::IntegerValue::Unsigned(32) =>
            {
                Some(value)
            }
            _ => None,
        })
        .expect("space branch has a literal byte");
    *alternate = semantic_vocabulary::IntegerValue::Unsigned(128);
    assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
}

#[test]
fn nested_loop_preserves_field_bounds_for_a_computed_joined_byte() {
    let source = "
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine narrow(value: i32) -> u8 {
            (value as u8 in Trapping) as u8
        }
        data Nested {
            values: [u8; 4] in Utf8;
            i: i32 in Wrapping;
            remaining: i32 in Wrapping;
            slot: i32 in Wrapping;
            chosen: u8;
        }
        machine Nested::run(&mut self) {
            self.values = \"    \";
            self.i = 2;
            transition { _ -> outer() }
            state outer(&mut self) {
                transition 0 <= self.i && self.i < 40 { true -> prepare() _ -> done() }
            }
            state prepare(&mut self) {
                self.remaining = 2;
                transition { _ -> inner() }
            }
            state inner(&mut self) {
                transition self.remaining > 0 { true -> step() _ -> select() }
            }
            state step(&mut self) {
                self.remaining = self.remaining - 1;
                transition { _ -> inner() }
            }
            state select(&mut self) {
                self.slot = self.i / 10;
                transition self.slot > 0 { true -> high() _ -> low() }
            }
            state high(&mut self) {
                self.chosen = narrow((self.slot + 48) as i32);
                transition { _ -> store() }
            }
            state low(&mut self) {
                self.chosen = 32;
                transition { _ -> store() }
            }
            state store(&mut self) {
                self.remaining = 1;
                transition 0 <= self.remaining && self.remaining < 4 {
                    true -> write()
                    _ -> done()
                }
            }
            state write(&mut self) {
                self.values[self.remaining] = self.chosen;
                self.i = self.i + 1;
                transition { _ -> outer() }
            }
            state done(&mut self) { }
        }";
    let lowered = lower_field_loop(source, "Nested::run");

    // Rebuild all evidence against changed Terminal semantics. A weaker loop
    // guard or an unsafe alternative cannot inherit the original byte bound.
    use semantic_vocabulary::IntegerValue::{Signed, Unsigned};
    for (original, replacement) in [(Signed(40), Signed(900)), (Unsigned(32), Unsigned(128))] {
        let mut changed = lowered.clone();
        changed.semantic_module.scalar_block_invariants.clear();
        changed.proof_bundle = terminal_verifier::ProofBundle::default();
        let value = changed
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::IntegerConstant { value } if *value == original => {
                    Some(value)
                }
                _ => None,
            })
            .expect("the guard or alternative has its own literal");
        *value = replacement;
        assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
    }
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

/// A cyclic machine whose header receives a bounded machine parameter, so
/// optional inference proposes an entry-range invariant, while nothing in the
/// module needs an invariant to prove.
const BOUNDED_CYCLE: &str = "
    machine countdown(start: u64 [0..=8]) {
        transition { _ -> head(start) }
        state head(remaining: u64) {
            transition remaining > 0 { true -> step(remaining) _ -> done() }
        }
        state step(remaining: u64) {
            let decremented: u64 = remaining - 1;
            transition { _ -> head(decremented) }
        }
        state done() { }
    }";

/// The roster pass's repeated whole-module work draws from a named aggregate
/// budget. A spent budget declines the optional inference — accounted as an
/// exhaustion — and the module still stands on its own questions.
#[test]
fn an_exhausted_roster_round_budget_declines_inference_without_rejecting() {
    let checked = crate::front_end::checked_program(BOUNDED_CYCLE);
    let mut lowered = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("countdown"),
    )
    .expect("the bounded cycle lowers");
    // The first lowering must have retained the entry-range proposal, or
    // restarting the pass would have nothing to decline.
    assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
    // Restart optional inference from an empty roster so the pass proposes
    // the same candidates as the first lowering did.
    lowered.semantic_module.scalar_block_invariants.clear();
    lowered.proof_bundle = terminal_verifier::ProofBundle::default();
    let original = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .expect("the cleared module reconstructs");
    let exhaustions =
        crate::proofs::proof_timing::BUDGET_EXHAUSTIONS.load(std::sync::atomic::Ordering::Relaxed);
    let mut declines = crate::proofs::ProofSearchDeclines::default();
    super::retain_provable_roster_bounded(&mut lowered, &original, 0, &mut declines)
        .expect("a declined round is not a rejection");
    assert!(lowered.semantic_module.scalar_block_invariants.is_empty());
    assert!(
        crate::proofs::proof_timing::BUDGET_EXHAUSTIONS.load(std::sync::atomic::Ordering::Relaxed)
            > exhaustions,
        "the declined rounds are accounted",
    );
    // The ledger names the capacity so an obligation left unmet reports the
    // `Incomplete` refusal rather than a disproof of the module. A declined
    // round is a unit draw: its demand is one past the 128-round provision.
    let unmet = semantic_vocabulary::ObligationId::new(u64::MAX - 1).unwrap();
    assert_eq!(
        declines.unproved_obligation_error(unmet),
        crate::lowering_error::LoweringError::ProofSearchCapacityExhausted {
            capacity: crate::lowering_error::ProofSearchCapacity::ScalarInvariantRosterRounds,
            requested: 129,
            obligation: unmet,
        },
    );
    // The module stands on its own: ordinary finalization re-runs inference
    // under the production budget, and the finished module verifies.
    crate::proofs::operation_proofs::finalize_operation_proofs(&mut lowered)
        .expect("the module still finalizes");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the module still verifies");
}

/// The ledger publishes the first decline of each capacity with that draw's
/// total demand: a refused bulk expansion reports the pool it had already
/// drawn plus the whole refused amount, and a later unit refusal of the same
/// capacity does not overwrite it.
#[test]
fn the_decline_ledger_keeps_each_capacitys_first_declined_demand() {
    use crate::lowering_error::{LoweringError, ProofSearchCapacity};

    let unmet = semantic_vocabulary::ObligationId::new(7).unwrap();
    let mut declines = crate::proofs::ProofSearchDeclines::default();
    assert_eq!(
        declines.unproved_obligation_error(unmet),
        LoweringError::OperationProofUnavailable(unmet),
    );
    let requested = ProofSearchCapacity::ScalarInvariantCandidateWork.limit() + 6;
    declines.record_demand(ProofSearchCapacity::ScalarInvariantCandidateWork, requested);
    declines.record(ProofSearchCapacity::ScalarInvariantCandidateWork);
    declines.record(ProofSearchCapacity::ScalarInvariantRosterRounds);
    let refusal = declines.unproved_obligation_error(unmet);
    assert_eq!(
        refusal,
        LoweringError::ProofSearchCapacityExhausted {
            capacity: ProofSearchCapacity::ScalarInvariantCandidateWork,
            requested,
            obligation: unmet,
        },
    );
    let exhaustion = refusal.incomplete_capacity().unwrap();
    assert!(exhaustion.requested > exhaustion.capacity.limit());
    assert_eq!(exhaustion.obligation, unmet);
}

#[test]
fn fresh_obligation_identities_decline_past_their_provision_or_the_identity_space() {
    use crate::lowering_error::{LoweringError, ProofSearchCapacity};
    use crate::proofs::{FreshObligations, ProofSearchDeclines};

    let capacity = ProofSearchCapacity::TerminalObligationIdentities;
    let unmet = semantic_vocabulary::ObligationId::new(3).unwrap();
    let mut declines = ProofSearchDeclines::default();
    let mut fresh = FreshObligations {
        next: 7,
        minted: capacity.limit() - 1,
    };
    assert_eq!(
        fresh.mint(&mut declines).map(|identity| identity.get()),
        Some(8)
    );
    assert_eq!(
        declines.unproved_obligation_error(unmet),
        LoweringError::OperationProofUnavailable(unmet),
    );
    assert!(fresh.mint(&mut declines).is_none());
    let exhaustion = declines
        .unproved_obligation_error(unmet)
        .incomplete_capacity()
        .expect("the spent provision is an accounted decline");
    assert_eq!(exhaustion.capacity, capacity);
    assert_eq!(exhaustion.requested, capacity.limit() + 1);
    // Both publishable: the compiler request's scalar resources stay below
    // INT32_MAX.
    assert!(exhaustion.requested < i32::MAX as u64);

    let mut declines = ProofSearchDeclines::default();
    let mut fresh = FreshObligations {
        next: u64::MAX,
        minted: 0,
    };
    assert!(fresh.mint(&mut declines).is_none());
    assert_eq!(
        declines
            .unproved_obligation_error(unmet)
            .incomplete_capacity()
            .map(|exhaustion| exhaustion.capacity),
        Some(capacity),
    );
}
