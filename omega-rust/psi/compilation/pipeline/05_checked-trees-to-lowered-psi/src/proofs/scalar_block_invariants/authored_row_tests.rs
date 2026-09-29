//! Authored rows stay outside the optional pass's prove-or-drop transaction.

use crate::TerminalMachineSelection;

/// `Alignment::size` returns a constant from each case arm. Its guarantee
/// needs the join's literal-range row, which only this pass proposes. The
/// `row` state's `requires` clause is an authored arrival contract on
/// another header of the calling machine.
const SIZED_LOOP: &str = "
    data Alignment [copy] { case One; case Two; }
    machine Alignment::size(&self) -> u64
        ensures 1 <= result && result <= 2
    {
        transition self {
            Alignment::One -> (1)
            Alignment::Two -> (2)
        }
    }
    data Grid where 0 <= bound, bound <= 2 {
        bound: u32;
        result: u32;
        width: u64;
        alignment: Alignment;
    }
    machine Grid::run(&mut self) {
        self.bound = 2;
        self.width = self.alignment.size();
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
fn an_unprovable_authored_row_does_not_discard_another_machines_inference() {
    let checked = crate::front_end::checked_program(SIZED_LOOP);
    let lowered = crate::machine_lowering::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Grid::run"),
    )
    .expect("the sized loop lowers and proves");
    let roster = &lowered.semantic_module.scalar_block_invariants;
    let authored_machine = roster
        .iter()
        .find(|row| {
            matches!(&row.predicate, semantic_vocabulary::Proposition::Conjunction(members)
                if members.len() >= 2)
        })
        .map(|row| (row.machine, row.header))
        .expect("the requires clause is an authored arrival contract");
    let inferred = roster
        .iter()
        .filter(|row| row.machine != authored_machine.0)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        !inferred.is_empty(),
        "the size join needs an inferred literal-range row"
    );

    // Restart inference with only the authored row, made unprovable. The
    // optional search cannot prove it, but it is not the search's to drop.
    let mut changed = lowered.clone();
    changed.proof_bundle = terminal_verifier::ProofBundle::default();
    changed
        .semantic_module
        .scalar_block_invariants
        .retain(|row| (row.machine, row.header) == authored_machine);
    changed.semantic_module.scalar_block_invariants[0].predicate =
        semantic_vocabulary::Proposition::Falsehood;
    let mandatory = changed.semantic_module.scalar_block_invariants.clone();
    super::retain_provable_bounded(&mut changed, super::MAX_ROSTER_ROUNDS)
        .expect("optional inference never rejects");
    let retained = &changed.semantic_module.scalar_block_invariants;
    assert!(
        retained.contains(&mandatory[0]),
        "the authored row is retained exactly as supplied"
    );
    for row in &inferred {
        assert!(
            retained.iter().any(|candidate| (
                candidate.machine,
                candidate.header,
                &candidate.predicate
            ) == (row.machine, row.header, &row.predicate)),
            "the size join's proved row survives an unrelated authored failure: {row:?}"
        );
    }
    // The authored row is still an obligation: finalization refuses it.
    assert!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut changed).is_err());
}
