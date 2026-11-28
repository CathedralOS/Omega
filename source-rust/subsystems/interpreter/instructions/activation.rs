//! Hand control back to the owner of a producer when its checked park call runs.

#[path = "activation/driver.rs"]
mod driver;
#[path = "activation/funding.rs"]
mod funding;
#[path = "activation/results.rs"]
mod results;

use crate::engine::execution::CompletedReturn;
use crate::storage::storage_addresses::ScalarStorageAddress;
use crate::{TerminalEffectWait, TerminalInterpretError, TerminalSuspension};
pub(crate) use driver::OwnedActivations;

/// Installed programs keep the checked field path and reply type for each
/// park. Stepping does not revisit domain policies or search record fields.
#[derive(Default)]
pub(crate) struct ParkSites(std::sync::OnceLock<Vec<ParkSite>>);
type ParkShape = terminal_semantics::calls::owned_activation::ParkShape;
type ParkSite = (semantic_vocabulary::BoundaryMachineId, Result<ParkShape, &'static str>);

impl ParkSites {
    pub(crate) fn at(&self, module: &terminal_psi::TerminalModule, id: semantic_vocabulary::BoundaryMachineId)
        -> Result<ParkShape, TerminalInterpretError>
    {
        use crate::boundary::errors::malformed;
        use language_core::sealed_intrinsics::ActivationOperation;
        use terminal_psi::CheckedBoundaryExecution;
        let sites = self.0.get_or_init(|| {
            let mut sites: Vec<_> = module.interfaces.boundary_machines.iter()
                .filter(|b| b.declaration.as_ref().is_some_and(|d| d.execution ==
                    CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Park)))
                .map(|b| (b.id, terminal_semantics::calls::owned_activation::park_shape(module, b)))
                .collect();
            sites.sort_unstable_by_key(|site| site.0);
            sites
        });
        let index = sites.binary_search_by_key(&id, |site| site.0).map_err(|_| malformed())?;
        sites[index].1.map_err(|_| malformed())
    }
}

/// Prepared by the ordinary value-transfer machinery for the stopped call.
/// A reply includes its owned value and custody, not just a readiness bit.
pub(crate) struct TransferredReply {
    pub(crate) completed: CompletedReturn,
    pub(crate) source_claims: Option<ReplyClaims>,
}

pub(crate) struct ReplyClaims {
    pub(crate) place: semantic_vocabulary::PlaceId,
    pub(crate) held: std::collections::BTreeMap<
        semantic_vocabulary::ClaimId,
        crate::engine::execution::LiveClaim,
    >,
}

/// A qualified request and permit both borrow the publication's anchor.
/// The checked declaration supplies the path to that reference. Moving the
/// handle must not change which publication can answer it.
pub(crate) fn publication_slot(
    execution: &mut crate::engine::execution::TerminalExecution<'_>,
    handle: &terminal_psi::StructuralArgument,
    anchor: terminal_psi::TerminalIdentityIndex,
) -> Result<ScalarStorageAddress, TerminalInterpretError> {
    use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};
    let mut path = handle.path.clone();
    path.extend([
        StructuralPathSegment::Field(anchor),
        StructuralPathSegment::Referent,
    ]);
    execution.borrowed_storage_identity(&StructuralArgument {
        place: handle.place,
        path,
        access: StructuralAccess::SharedBorrow,
    })
}

/// Only the execution owner holds this state. Reporting the suspension or
/// comparing a mismatched permit never consumes the producer or its reply.
pub(crate) struct ParkedActivation {
    pub(crate) suspension: TerminalSuspension,
    slot: ScalarStorageAddress,
    reply_type: std::sync::Arc<str>,
}

/// Only the driver stepping a child supplies this output. A plain entry
/// invocation and a foreign import adapter cannot answer a caller-owned park.
#[derive(Default)]
pub(crate) enum StepControl {
    #[default]
    Running,
    Waiting(Box<ParkedActivation>),
    Answered(Box<(TerminalSuspension, TransferredReply)>),
}

impl StepControl {
    pub(crate) fn park(
        &mut self,
        suspension: TerminalSuspension,
        slot: ScalarStorageAddress,
        reply_type: std::sync::Arc<str>,
    ) -> Result<(), TerminalInterpretError> {
        if !matches!(self, Self::Running) {
            return Err(crate::boundary::errors::malformed());
        }
        *self = Self::Waiting(Box::new(ParkedActivation {
            suspension,
            slot,
            reply_type,
        }));
        Ok(())
    }

    /// Check the occurrence before transferring anything out of the sender.
    /// A rejected or repeated answer must leave its permit and payload there.
    pub(crate) fn answer_from<S>(
        &mut self,
        occurrence: &TerminalSuspension,
        slot: &ScalarStorageAddress,
        reply_type: &str,
        sender: &mut S,
        transfer: impl FnOnce(&mut S) -> Result<TransferredReply, TerminalInterpretError>,
    ) -> Result<bool, TerminalInterpretError> {
        let Self::Waiting(waiting) = self else {
            return Ok(false);
        };
        if !waiting.accepts(occurrence, slot, reply_type) {
            return Ok(false);
        }
        let answer = transfer(sender)?;
        *self = Self::Answered(Box::new((occurrence.clone(), answer)));
        Ok(true)
    }

    pub(crate) fn stopped_at(&self) -> Option<&TerminalEffectWait> {
        match self {
            Self::Running => None,
            Self::Waiting(parked) => Some(&parked.suspension.site),
            Self::Answered(answer) => Some(&answer.0.site),
        }
    }
}

impl ParkedActivation {
    /// Compare the admitted type identities, not module-local type numbers.
    /// On success the caller owns the actual answer and the saved state;
    /// answering does not itself run the producer.
    fn accepts(
        &self,
        occurrence: &TerminalSuspension,
        permit_slot: &ScalarStorageAddress,
        reply_type: &str,
    ) -> bool {
        &self.suspension == occurrence
            && &self.slot == permit_slot
            && self.reply_type.as_ref() == reply_type
    }
}

impl crate::engine::execution::TerminalExecution<'_> {
    pub(crate) fn execute_activation_call(
        &mut self,
        operation: &terminal_psi::Operation,
        control: language_core::sealed_intrinsics::ActivationOperation,
        boundary_declaration: &terminal_psi::BoundaryMachineDeclaration,
    ) -> Result<crate::engine::execution::OperationFlow, TerminalInterpretError> {
        use crate::boundary::errors::malformed;
        use language_core::sealed_intrinsics::ActivationOperation;
        use terminal_psi::StructuralAccess;
        let terminal_psi::OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        match control {
            ActivationOperation::Prepare => {
                return self.prepare_activation(operation, boundary_declaration);
            }
            ActivationOperation::StepFixed => {
                return self.step_activation(operation, boundary_declaration);
            }
            ActivationOperation::Resume => {
                return self.answer_activation(operation, boundary_declaration);
            }
            ActivationOperation::Park => {}
        }
        let owner =
            self.owned_step
                .as_mut()
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "park requires a caller stepping this activation",
                ))?;
        let [request] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        if !arguments.is_empty() || request.access != StructuralAccess::Owned {
            return Err(malformed());
        }
        match owner {
            StepControl::Running => {}
            StepControl::Waiting(_) => return Err(malformed()),
            StepControl::Answered(_) => {
                let StepControl::Answered(answer) = std::mem::take(owner) else {
                    unreachable!()
                };
                return self.finish_owned_park(operation, request, completion_receipts, answer.1);
            }
        }
        let shape = self.program.park_sites.at(&self.program.catalogs, boundary_declaration.id)?;
        let slot = publication_slot(self, request, shape.anchor_identity)?;
        let reply = self
            .program
            .catalogs
            .identities
            .text(shape.reply)
            .ok_or(malformed())?;
        let site = crate::TerminalEffectWait {
            program: self.program.identity.clone(),
            machine: self.current_machine,
            block: self.current,
            operation: operation.id,
            boundary: *boundary,
        };
        let occurrence = crate::suspension::ParkOccurrence::fresh(
            self.local_structural_identities.exposure_namespace,
            &mut self.last_park,
        )?;
        self.owned_step.as_mut().ok_or(malformed())?.park(
            crate::TerminalSuspension { site, occurrence },
            slot,
            reply.into(),
        )?;
        self.publish_parked_activation()
    }

    pub(crate) fn finish_owned_park(
        &mut self,
        operation: &terminal_psi::Operation,
        request: &terminal_psi::StructuralArgument,
        receipts: &[terminal_psi::CompletionReceipt],
        mut reply: TransferredReply,
    ) -> Result<crate::engine::execution::OperationFlow, TerminalInterpretError> {
        use crate::boundary::errors::malformed;
        use crate::boundary::results::TerminalExecutionResult;
        use crate::engine::execution::OperationFlow;
        use terminal_psi::OperationResult;
        let remaining = crate::engine::custody::complete_claims(
            &self.live_claims,
            std::slice::from_ref(request),
            receipts,
        )?;
        if reply.completed.rebound_claims.is_some() || reply.completed.destination.is_some() {
            return Err(malformed());
        }
        use crate::engine::terminators::completion::ReturnDestination;
        let destination = match (&operation.result, &reply.completed.value) {
            (OperationResult::Unit, TerminalExecutionResult::Unit) => ReturnDestination::Unit,
            (OperationResult::Scalar(dest), TerminalExecutionResult::Scalar(value))
                if dest.scalar_type == value.scalar_type() =>
            {
                ReturnDestination::Scalar(dest.id)
            }
            (OperationResult::Structural(dest), TerminalExecutionResult::Structural(value))
                if value.value.structural_type == dest.structural_type
                    && value.claims.iter().copied().eq(reply
                        .completed
                        .returned_claims
                        .iter()
                        .map(|row| row.claim)) =>
            {
                let active = self.active_claims_in_value(&value.value, &dest.claims)?;
                if !active.is_empty() || !reply.completed.returned_claims.is_empty() {
                    let source = reply.source_claims.take().ok_or(malformed())?;
                    let mut destination = dest.clone();
                    destination.claims = active;
                    // The sender has to transfer the real claims at the original
                    // paths, exactly as on return from a checked machine.
                    reply.completed.rebound_claims =
                        Some(crate::engine::custody::rebind_structural_result_claims(
                            &remaining,
                            &source.held,
                            source.place,
                            &destination,
                            &reply.completed.returned_claims,
                        )?);
                }
                reply.completed.destination = Some(dest.clone());
                ReturnDestination::Structural(dest, None)
            }
            (OperationResult::Structural(dest), TerminalExecutionResult::ScalarArray(value))
                if value.value.structural_type == dest.structural_type
                    && dest.claims.is_empty() =>
            {
                reply.completed.destination = Some(dest.clone());
                ReturnDestination::Structural(dest, None)
            }
            _ => return Err(malformed()),
        };
        if reply.source_claims.is_some()
            || (!reply.completed.returned_claims.is_empty()
                && reply.completed.rebound_claims.is_none())
        {
            return Err(malformed());
        }
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        if !self.stage_constructor_operand(request, &mut frontier, &mut consumed)? {
            return Err(malformed());
        }
        let loans = self.loan_completions_for_operation(operation.id)?;
        self.commit_constructor_operands(frontier, &consumed, remaining)?;
        self.install_completed_value(&mut reply.completed, destination, &loans)?;
        Ok(OperationFlow::Advance)
    }
}
