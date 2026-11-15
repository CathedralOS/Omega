//! Generated cleanup entries use ordinary invocation and requirement rows.
//! Only the source producer pass mints them; discharge checks the exact rows.

use semantic_vocabulary::InvocationSite;
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{OwnedCleanupInvocation, StructuralArgument, TerminalAffineCleanupAction};

pub(crate) fn inputs(
    cleanup: &OwnedCleanupInvocation,
) -> terminal_psi::ModuleInvocationInputs<'_, semantic_vocabulary::TargetMachineCallable> {
    terminal_psi::ModuleInvocationInputs {
        callee: semantic_vocabulary::TargetMachineCallable::Machine(cleanup.cleanup_machine),
        required_contract: None,
        arguments: &[],
        erased_arguments: &[],
        erased_proof_arguments: &[],
        structural_arguments: std::slice::from_ref(&cleanup.subject),
        claim_transfers: &cleanup.claim_transfers,
        completion_receipts: &[],
        requirement_obligations: &cleanup.requirement_obligations,
        crash_continuations: &[],
    }
}

pub(crate) fn exact_place(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    tables: &MachineTables,
    argument: &StructuralArgument,
) -> Checked<ExactPlaceId> {
    let concerns = Construct::Machine;
    let fail = || {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine.id,
            concerns,
            "cleanup actual has no exact interned owned place",
        )
    };
    let root = machine
        .structural_place_type(argument.place)
        .ok_or_else(fail)?;
    let path = terminal_semantics::canonical_inline_region(
        module.structural_types.iter(),
        root,
        &argument.path,
    )
    .ok_or_else(fail)?;
    let mut place = *tables.roots.get(argument.place).ok_or_else(fail)?;
    for segment in path {
        use semantic_vocabulary::CanonicalStructuralPathSegment as C;
        use terminal_check_tables::bindings::Segment;
        let segment = match segment {
            C::Field(field) => Segment::Field(field),
            C::Case(case) => Segment::Case(case),
            C::FixedIndex(index) => Segment::FixedIndex(index),
            C::Referent => return Err(fail()),
        };
        let row = tables.places.get(place).ok_or_else(fail)?;
        place = row
            .children
            .binary_search_by_key(&segment, |(segment, _)| *segment)
            .ok()
            .map(|index| row.children[index].1)
            .ok_or_else(fail)?;
    }
    Ok(place)
}

/// Source-side completion of generated rows, before private inference. A
/// partially supplied invocation is never repaired; the receiving checker
/// validates the same rows without calling this producer routine.
pub(crate) fn materialize(elaboration: &mut Elaboration) -> Checked<()> {
    let mut pending = Vec::new();
    for machine in &elaboration.module.machines {
        for (block_index, block) in machine.blocks.iter().enumerate() {
            for (edge, actions) in block.terminator.cleanup_rosters() {
                for (ordinal, action) in actions.iter().enumerate() {
                    let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action else {
                        continue;
                    };
                    let ordinal = crate::position(ordinal, machine.id)?;
                    let target = terminal_semantics::nominal_cleanup::owned_target(
                        elaboration.module.structural_types.iter(),
                        elaboration.module.machines.iter(),
                        cleanup,
                    )
                    .map_err(|detail| {
                        crate::refusal(
                            RefusalReason::ForeignIdentity,
                            machine.id,
                            Construct::Edge(edge),
                            detail,
                        )
                    })?;
                    if !target.contract.erased_scalar_formals.is_empty()
                        || !target.contract.erased_proof_formals.is_empty()
                    {
                        return Err(crate::refusal(
                            RefusalReason::Unsupported,
                            machine.id,
                            Construct::Edge(edge),
                            "owned cleanup needs its explicit erased argument correspondence",
                        ));
                    }
                    pending.push((
                        machine.id,
                        block_index,
                        edge,
                        ordinal,
                        cleanup.clone(),
                        target.contract.requires.len(),
                    ));
                }
            }
        }
    }
    for (id, block, edge, action_ordinal, cleanup, clauses) in pending {
        let site = InvocationSite::Cleanup {
            edge,
            action_ordinal,
        };
        let tables = elaboration.tables.machines.get_mut(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "cleanup caller has no tables",
            )
        })?;
        let mut existing = tables
            .invocations
            .iter()
            .filter(|(_, call)| call.site == site);
        if existing.next().is_some() {
            if existing.next().is_some() {
                return Err(crate::refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Edge(edge),
                    "cleanup has duplicate invocation rows",
                ));
            }
            continue;
        }
        if !cleanup.requirement_obligations.is_empty() {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup requirements have no owning invocation row",
            ));
        }
        let caller = elaboration.module.machine(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "cleanup caller is absent",
            )
        })?;
        let actual = exact_place(&elaboration.module, caller, tables, &cleanup.subject)?;
        let invocation = tables.invocations.push(InvocationRow {
            site,
            callee: Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(
                cleanup.cleanup_machine,
            )),
            required_contract: None,
            actuals: vec![Actual::Place(actual)],
            requirements: Vec::new(),
        });
        let sources = elaboration.sources.get_mut(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup caller has no source correspondence",
            )
        })?;
        let source = *terminal_check_tables::row(&sources.edges, edge, id, Construct::Edge(edge))?;
        let mut requirements = Vec::new();
        for clause in 0..clauses {
            let obligation = tables.obligations.push(ObligationRow {
                kind: ObligationKind::CallRequirement {
                    invocation,
                    clause: crate::position(clause, id)?,
                },
                created_at: ProgramPoint::BeforeCleanup {
                    edge,
                    action_ordinal,
                },
            });
            if sources.obligations.push(source) != obligation {
                return Err(crate::refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Edge(edge),
                    "cleanup obligation lost its source correspondence",
                ));
            }
            requirements.push(obligation);
        }
        tables
            .invocations
            .get_mut(invocation)
            .expect("the invocation was minted")
            .requirements
            .clone_from(&requirements);
        let caller = elaboration
            .module
            .machines
            .iter_mut()
            .find(|machine| machine.id == id)
            .expect("the caller was checked");
        let actions = actions_mut(&mut caller.blocks[block].terminator, edge).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup edge disappeared",
            )
        })?;
        let TerminalAffineCleanupAction::InvokeOwned(actual) =
            &mut actions[action_ordinal as usize]
        else {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup owner changed while minting its call",
            ));
        };
        actual.requirement_obligations = requirements;
    }
    Ok(())
}

fn actions_mut(
    terminator: &mut terminal_psi::Terminator,
    selected: semantic_vocabulary::EdgeId,
) -> Option<&mut Vec<TerminalAffineCleanupAction>> {
    use terminal_psi::Terminator as T;
    match terminator {
        T::Jump {
            edge,
            cleanup_actions,
            ..
        }
        | T::Return {
            edge,
            cleanup_actions,
            ..
        }
        | T::ReturnUnit {
            edge,
            cleanup_actions,
        }
        | T::ReturnStructural {
            edge,
            cleanup_actions,
            ..
        } => (*edge == selected).then_some(cleanup_actions),
        T::Conditional {
            when_true,
            when_false,
            ..
        } => {
            if when_true.edge == selected {
                Some(&mut when_true.cleanup_actions)
            } else if when_false.edge == selected {
                Some(&mut when_false.cleanup_actions)
            } else {
                None
            }
        }
        T::StructuralCase { cases, .. } => cases
            .iter_mut()
            .find(|case| case.edge == selected)
            .map(|case| &mut case.cleanup_actions),
        _ => None,
    }
}
