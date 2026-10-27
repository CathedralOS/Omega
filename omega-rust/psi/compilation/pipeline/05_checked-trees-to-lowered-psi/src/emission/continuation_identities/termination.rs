//! Edge operands, cleanup and proof terms use the predecessor's namespace.

use super::IdentityMap;
use crate::lowering_error::LoweringError;
use semantic_vocabulary::{BlockId, EdgeId};
use terminal_psi::{SuccessorEdge, TerminalAffineCleanupAction, Terminator};

impl IdentityMap {
    fn target(&mut self, edge: EdgeId, target: BlockId) -> BlockId {
        self.edge_targets
            .iter()
            .find_map(|(original, replacement)| (*original == edge).then_some(*replacement))
            .unwrap_or_else(|| self.block_id(target))
    }

    fn successor(&mut self, successor: &mut SuccessorEdge) {
        successor.target = self.target(successor.edge, successor.target);
        successor.edge = self.edge(successor.edge);
        for argument in &mut successor.arguments {
            *argument = self.value(*argument);
        }
        for argument in &mut successor.erased_arguments {
            self.scalar_term(argument);
        }
        for argument in &mut successor.erased_proof_arguments {
            self.proof_term(argument);
        }
        for argument in &mut successor.structural_arguments {
            self.argument(argument);
        }
        self.cleanup(&mut successor.cleanup_actions);
    }

    pub(crate) fn cleanup(&mut self, actions: &mut [TerminalAffineCleanupAction]) {
        for action in actions {
            match action {
                TerminalAffineCleanupAction::DiscardRoot(place)
                | TerminalAffineCleanupAction::RetireHome(place) => *place = self.place(*place),
                TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    discard.place = self.place(discard.place);
                    self.path(&mut discard.path);
                }
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place = self.place(cleanup.place);
                    self.path(&mut cleanup.path);
                    for claim in &mut cleanup.consumed_claims {
                        *claim = self.claim(*claim);
                    }
                    cleanup.consumed_claims.sort();
                    for obligation in &mut cleanup.requirement_obligations {
                        *obligation = self.obligation(*obligation);
                    }
                    // cleanup_receiver belongs to the called hook's signature.
                }
            }
        }
    }

    pub(super) fn terminator(&mut self, terminator: &mut Terminator) -> Result<(), LoweringError> {
        match terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                cleanup_actions,
            } => {
                *target = self.target(*edge, *target);
                *edge = self.edge(*edge);
                for argument in arguments {
                    *argument = self.value(*argument);
                }
                for argument in erased_arguments {
                    self.scalar_term(argument);
                }
                for argument in erased_proof_arguments {
                    self.proof_term(argument);
                }
                for argument in structural_arguments {
                    self.argument(argument);
                }
                self.cleanup(cleanup_actions);
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                *condition = self.value(*condition);
                self.successor(when_true);
                self.successor(when_false);
            }
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                *source = self.place(*source);
                self.path(path);
                for case in cases {
                    case.target = self.target(case.edge, case.target);
                    case.edge = self.edge(case.edge);
                    self.cleanup(&mut case.cleanup_actions);
                }
            }
            Terminator::Return {
                edge,
                value,
                cleanup_actions,
            } => {
                *edge = self.edge(*edge);
                *value = self.value(*value);
                self.cleanup(cleanup_actions);
            }
            Terminator::ReturnUnit {
                edge,
                cleanup_actions,
            } => {
                *edge = self.edge(*edge);
                self.cleanup(cleanup_actions);
            }
            Terminator::ReturnStructural {
                edge,
                source,
                returned_claims,
                cleanup_actions,
            } => {
                *edge = self.edge(*edge);
                *source = self.place(*source);
                for claim in returned_claims.iter_mut() {
                    claim.claim = self.claim(claim.claim);
                    self.path(&mut claim.path);
                }
                returned_claims.sort_by_key(|row| row.claim);
                self.cleanup(cleanup_actions);
            }
            Terminator::Crash {
                edge,
                site_guard,
                frontier_lower_bound,
                ..
            } => {
                *edge = self.edge(*edge);
                for guard in site_guard.iter_mut() {
                    let mut proposition = guard.proposition().clone();
                    self.proposition(&mut proposition)?;
                    *guard = terminal_psi::CrashPredicateTerm::new(proposition);
                }
                site_guard.sort();
                for claim in frontier_lower_bound.iter_mut() {
                    *claim = self.claim(*claim);
                }
                frontier_lower_bound.sort();
            }
            Terminator::InvokeNoNormal {
                edge,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => {
                *edge = self.edge(*edge);
                for argument in arguments {
                    *argument = self.value(*argument);
                }
                for argument in erased_arguments {
                    self.scalar_term(argument);
                }
                for argument in erased_proof_arguments {
                    self.proof_term(argument);
                }
                for argument in structural_arguments {
                    self.argument(argument);
                }
                for transfer in claim_transfers.iter_mut() {
                    transfer.claim = self.claim(transfer.claim);
                    self.path(&mut transfer.path);
                }
                claim_transfers.sort();
                for obligation in requirement_obligations {
                    *obligation = self.obligation(*obligation);
                }
                self.crash_routes(crash_continuations)?;
            }
            Terminator::ExternalCompletion {
                edge,
                arguments,
                structural_arguments,
                requirement_obligations,
                frontier_lower_bound,
                ..
            } => {
                *edge = self.edge(*edge);
                for argument in arguments {
                    *argument = self.value(*argument);
                }
                for argument in structural_arguments {
                    self.argument(argument);
                }
                for obligation in requirement_obligations {
                    *obligation = self.obligation(*obligation);
                }
                for claim in frontier_lower_bound.iter_mut() {
                    *claim = self.claim(*claim);
                }
                frontier_lower_bound.sort();
            }
        }
        Ok(())
    }
}
