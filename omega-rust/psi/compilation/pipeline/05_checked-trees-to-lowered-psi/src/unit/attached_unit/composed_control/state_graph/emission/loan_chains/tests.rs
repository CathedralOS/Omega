//! Loan chains backed by a later state's local, lowered from source and
//! re-checked by independent Terminal verification, with tampered modules
//! and plans as negative controls.

use crate::TerminalMachineSelection;
use semantic_vocabulary::{BlockId, PlaceId};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralMultiplicity, TerminalMachine, TerminalModule,
    Terminator,
};

const SCORE: &str = "data Score { value: u64 in Wrapping; bonus: u64 in Wrapping; }
     data Ledger { total: u64 in Wrapping; }";

fn lowered(source: &str) -> TerminalModule {
    let checked = crate::front_end::checked_program(&format!("{SCORE}\n{source}"));
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("route"))
        .expect("the loan chain lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("independent verification admits the loan chain");
    lowered.semantic_module
}

fn refusal(source: &str) -> String {
    let checked = crate::front_end::checked_program(&format!("{SCORE}\n{source}"));
    crate::lower_machine(&checked, TerminalMachineSelection::Name("route"))
        .expect_err("the loan chain is refused")
        .to_string()
}

fn machine(module: &TerminalModule) -> &TerminalMachine {
    module
        .machines
        .iter()
        .find(|machine| !machine.blocks.is_empty())
        .expect("the lowered route machine")
}

fn machine_mut(module: &mut TerminalModule) -> &mut TerminalMachine {
    module
        .machines
        .iter_mut()
        .find(|machine| !machine.blocks.is_empty())
        .expect("the lowered route machine")
}

/// Each lent local's home, in operation order.
fn homes(machine: &TerminalMachine) -> Vec<PlaceId> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::EstablishZeroedStructural {}))
        .filter_map(|operation| operation.result.structural().map(|result| result.place))
        .collect()
}

/// The blocks declaring a `Score` structural parameter, with that parameter.
fn score_parameters(
    module: &TerminalModule,
) -> Vec<(BlockId, terminal_psi::StructuralParameterDeclaration)> {
    let machine = machine(module);
    let score = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::EstablishZeroedStructural {}))
        .and_then(|operation| operation.result.structural())
        .expect("a lent Score local")
        .structural_type;
    machine
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .filter(move |parameter| parameter.structural_type == score)
                .map(move |parameter| (block.id, parameter.clone()))
        })
        .collect()
}

fn return_discards(machine: &TerminalMachine) -> Vec<(BlockId, Vec<PlaceId>)> {
    machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some((block.id, trivial_affine_discards.clone())),
            _ => None,
        })
        .collect()
}

const LENDER_HOME: &str = "machine route(ledger: &mut Ledger, start: u64) {
         transition start > 2 {
             true -> prepare(ledger, start)
             _ -> refused(ledger)
         }
         state prepare(ledger: &mut Ledger, start: u64) {
             let mut score: Score;
             score.value = start as u64 in Wrapping;
             transition { _ -> bump(ledger, &mut score) }
         }
         state bump(ledger: &mut Ledger, score: &mut Score) {
             score.bonus = score.value + 67;
             ledger.total = score.bonus;
         }
         state refused(ledger: &mut Ledger) {}
     }";

#[test]
fn later_lender_home_dies_where_its_loan_chain_returns() {
    let module = lowered(LENDER_HOME);
    let machine = machine(&module);
    let [home] = homes(machine)[..] else {
        panic!("one lent home");
    };
    // The successor keeps its exclusive view of the lender's home.
    let [(bump, parameter)] = &score_parameters(&module)[..] else {
        panic!("one Score loan parameter");
    };
    assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
    // Only the chain's own return disposes of the home; the refused path
    // never established it.
    let returns = return_discards(machine);
    assert_eq!(
        returns
            .iter()
            .filter(|(_, discards)| discards.contains(&home))
            .map(|(block, _)| *block)
            .collect::<Vec<_>>(),
        vec![*bump],
        "the home dies once, at the loan holder's return"
    );

    // Negative controls: the activation-entry shape (a discard at every
    // return) and a chain end that forgets the home both fail verification.
    let mut everywhere = module.clone();
    for block in &mut machine_mut(&mut everywhere).blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut block.terminator
            && !trivial_affine_discards.contains(&home)
        {
            trivial_affine_discards.insert(0, home);
        }
    }
    assert!(
        terminal_verifier::validate_module(&everywhere).is_err(),
        "a return that never visited the lender cannot discard its home"
    );
    let mut forgotten = module.clone();
    for block in &mut machine_mut(&mut forgotten).blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut block.terminator
        {
            trivial_affine_discards.retain(|place| *place != home);
        }
    }
    assert!(
        terminal_verifier::validate_module(&forgotten).is_err(),
        "the loan chain's return must dispose of the home"
    );
}

#[test]
fn forwarded_chain_keeps_the_home_live_until_its_last_member_leaves() {
    let module = lowered(
        "machine route(ledger: &mut Ledger, start: u64) {
             transition start > 2 {
                 true -> prepare(ledger, start)
                 _ -> refused(ledger)
             }
             state prepare(ledger: &mut Ledger, start: u64) {
                 let mut score: Score;
                 score.value = start as u64 in Wrapping;
                 transition { _ -> bump(ledger, &mut score) }
             }
             state bump(ledger: &mut Ledger, score: &mut Score) {
                 score.bonus = score.value + 60;
                 transition score.bonus == 63 {
                     true -> settle(ledger, score)
                     _ -> refused(ledger)
                 }
             }
             state settle(ledger: &mut Ledger, tally: &mut Score) {
                 tally.value = tally.bonus + 7;
                 ledger.total = tally.value;
             }
             state refused(ledger: &mut Ledger) {}
         }",
    );
    let machine = machine(&module);
    let [home] = homes(machine)[..] else {
        panic!("one lent home");
    };
    let members = score_parameters(&module);
    assert_eq!(
        members.len(),
        2,
        "both chain members keep an exclusive view"
    );
    assert!(
        members
            .iter()
            .all(|(_, parameter)| parameter.access == StructuralAccess::MutableBorrow)
    );
    let settle = members
        .iter()
        .map(|(block, _)| *block)
        .max()
        .expect("the forwarded member");
    // `bump` forwards on one arm (no discard) and ends the chain on the
    // other; `settle` ends it at its return.
    let mut edge_discards = 0;
    for block in &machine.blocks {
        if let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &block.terminator
        {
            for arm in [when_true, when_false] {
                let discards = arm.trivial_affine_discards.contains(&home);
                assert_eq!(
                    discards,
                    arm.target != settle && members.iter().any(|(member, _)| *member == block.id),
                    "only the member edge that does not forward ends the chain"
                );
                edge_discards += usize::from(discards);
            }
        }
    }
    assert_eq!(edge_discards, 1);
    assert!(
        return_discards(machine)
            .iter()
            .any(|(block, discards)| *block == settle && discards.contains(&home))
    );
}

const TWO_LENDERS: &str = "machine route(ledger: &mut Ledger, start: u64) {
         transition start > 5 {
             true -> first(ledger, start)
             _ -> second(ledger, start)
         }
         state first(ledger: &mut Ledger, start: u64) {
             let mut opening: Score;
             opening.value = start as u64 in Wrapping;
             transition { _ -> settle(ledger, &mut opening) }
         }
         state second(ledger: &mut Ledger, start: u64) {
             let mut closing: Score;
             closing.value = start as u64 in Wrapping;
             transition { _ -> settle(ledger, &mut closing) }
         }
         state settle(ledger: &mut Ledger, score: &mut Score) {
             score.bonus = score.value + 66;
             ledger.total = score.bonus;
         }
     }";

#[test]
fn several_lenders_move_their_storage_into_one_owned_parameter() {
    let module = lowered(TWO_LENDERS);
    let machine = machine(&module);
    let lent = homes(machine);
    assert_eq!(lent.len(), 2, "each lender establishes its own home");
    let [(settle, parameter)] = &score_parameters(&module)[..] else {
        panic!("one Score member");
    };
    assert_eq!(parameter.access, StructuralAccess::Owned);
    assert_eq!(parameter.multiplicity, StructuralMultiplicity::Affine);
    // Each lender moves its own home whole into the member.
    let mut arrivals = Vec::new();
    for block in &machine.blocks {
        if let Terminator::Jump {
            target,
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &block.terminator
            && target == settle
        {
            let argument = structural_arguments
                .iter()
                .find(|argument| lent.contains(&argument.place))
                .expect("a lender's home arrives");
            assert_eq!(argument.access, StructuralAccess::Owned);
            assert!(!trivial_affine_discards.contains(&argument.place));
            arrivals.push(argument.place);
        }
    }
    arrivals.sort();
    let mut expected = lent.clone();
    expected.sort();
    assert_eq!(arrivals, expected);
    assert!(
        return_discards(machine)
            .iter()
            .any(|(block, discards)| block == settle && discards.contains(&parameter.place)),
        "the moved storage dies at the member's return"
    );

    // Negative controls: an exclusive view cannot accept a moved home, and
    // the moved storage cannot leak past the return.
    let mut viewed = module.clone();
    for block in &mut machine_mut(&mut viewed).blocks {
        for declaration in &mut block.structural_parameters {
            if declaration.place == parameter.place {
                declaration.access = StructuralAccess::MutableBorrow;
                declaration.multiplicity = StructuralMultiplicity::Unrestricted;
            }
        }
    }
    assert!(terminal_verifier::validate_module(&viewed).is_err());
    let mut leaked = module.clone();
    for block in &mut machine_mut(&mut leaked).blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut block.terminator
        {
            trivial_affine_discards.retain(|place| *place != parameter.place);
        }
    }
    assert!(terminal_verifier::validate_module(&leaked).is_err());
}

#[test]
fn caller_owned_join_into_a_later_lenders_chain_is_refused() {
    let diagnostic = refusal(
        "machine route(ledger: &mut Ledger, start: u64) {
             transition start > 2 {
                 true -> prepare(start)
                 _ -> settle(ledger)
             }
             state prepare(start: u64) {
                 let mut local: Ledger;
                 local.total = start as u64 in Wrapping;
                 transition { _ -> settle(&mut local) }
             }
             state settle(ledger: &mut Ledger) {
                 ledger.total = ledger.total + 1;
             }
         }",
    );
    assert!(
        diagnostic.contains("joins a state-local loan chain with a caller-owned or projected loan"),
        "{diagnostic}"
    );
}

/// Plan the composed `route` graph's chains directly. Lowering replays the
/// checked plan through admission first; this reaches the planner's own
/// checks with a forged plan.
fn planned(
    checked: &checked_trees::CheckedTrees,
) -> Result<super::LoanChains, crate::LoweringError> {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() > 1)
        .expect("the composed route plan");
    let mut next_place = 0;
    let state_views = plan
        .states
        .iter()
        .map(|state| {
            (0..state.structural_parameters.len())
                .map(|dense| {
                    next_place += 1;
                    terminal_psi::StructuralParameterDeclaration {
                        place: PlaceId::new(next_place).expect("nonzero place"),
                        position: dense as u32,
                        is_self: false,
                        structural_type: semantic_vocabulary::StructuralTypeId::new(1)
                            .expect("nonzero type"),
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::MutableBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    }
                })
                .collect()
        })
        .collect::<Vec<_>>();
    super::LoanChains::plan(
        plan,
        &vec![true; plan.states.len()],
        &vec![std::collections::BTreeMap::new(); plan.states.len()],
        false,
        &state_views,
    )
}

fn route_plan(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.states.len() > 1)
        .expect("the composed route plan")
}

#[test]
fn planner_chooses_backing_and_refuses_forged_homes() {
    use super::ChainArrival;
    // States in authored order: the route body, then its named states; the
    // `&mut Score` member is each successor's second structural parameter.
    let lender_home = crate::front_end::checked_program(&format!("{SCORE}\n{LENDER_HOME}"));
    let chains = planned(&lender_home).expect("one later lender plans");
    assert_eq!(chains.arrival(2, 1), Some(ChainArrival::Retained));
    assert_eq!(
        chains.arrival(2, 0),
        None,
        "a caller-owned loan is no member"
    );
    let two_lenders = crate::front_end::checked_program(&format!("{SCORE}\n{TWO_LENDERS}"));
    let chains = planned(&two_lenders).expect("two later lenders plan");
    assert_eq!(chains.arrival(3, 1), Some(ChainArrival::Transferred));

    // A lending edge naming a result its lender never established.
    let mut missing = lender_home.clone();
    let mut rewired = false;
    for state in &mut route_plan(&mut missing).states {
        if let checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
            &mut state.terminator
        {
            for transfer in &mut successor.transfers {
                if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                    subject,
                    ..
                } = &mut transfer.source
                    && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } = &mut subject.source
                {
                    *binding_ordinal += 7;
                    rewired = true;
                }
            }
        }
    }
    assert!(rewired, "the lending edge lends a whole local");
    let diagnostic = planned(&missing)
        .expect_err("a missing lent result is refused")
        .to_string();
    assert!(diagnostic.contains("names no local result"), "{diagnostic}");
    // Lowering's admission replay rejects the same forgery first.
    assert!(crate::lower_machine(&missing, TerminalMachineSelection::Name("route")).is_err());

    // A lent home relabeled linear cannot die at the chain's ends.
    let mut linear = lender_home.clone();
    for state in &mut route_plan(&mut linear).states {
        for operation in &mut state.operations {
            if let Some((result, _)) = operation.discardable_structural_result_mut() {
                result.multiplicity = language_semantics::Multiplicity::Linear;
            }
        }
    }
    let diagnostic = planned(&linear)
        .expect_err("a linear lent home is refused")
        .to_string();
    assert!(diagnostic.contains("home is linear"), "{diagnostic}");
}

#[test]
fn shared_re_loan_of_a_later_lenders_chain_is_refused() {
    let diagnostic = refusal(
        "machine route(ledger: &mut Ledger, start: u64) {
             transition start > 2 {
                 true -> prepare(ledger, start)
                 _ -> refused(ledger)
             }
             state prepare(ledger: &mut Ledger, start: u64) {
                 let mut score: Score;
                 score.value = start as u64 in Wrapping;
                 transition { _ -> bump(ledger, &mut score) }
             }
             state bump(ledger: &mut Ledger, score: &mut Score) {
                 score.bonus = score.value + 67;
                 transition { _ -> observe(ledger, score) }
             }
             state observe(ledger: &mut Ledger, score: &Score) {
                 ledger.total = score.bonus;
             }
             state refused(ledger: &mut Ledger) {}
         }",
    );
    assert!(
        diagnostic.contains("re-lent beyond its exclusive members"),
        "{diagnostic}"
    );
}
