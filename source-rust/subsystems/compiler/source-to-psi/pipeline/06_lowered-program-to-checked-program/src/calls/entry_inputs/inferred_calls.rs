//! Keep the origin of new input requirements through canonical clause reordering.
//! Only final, discharged calls appear in the retained invocation.
use super::{Arrival, CallPosition};
use crate::{Discharge, Discharged, refusal};
use semantic_vocabulary::{
    InvocationSite, MachineId, ObligationId, Proposition, TargetMachineCallable,
};
use terminal_check_tables::{
    Checked, Construct, InvocationId, ProgramPoint, RefusalReason, edges::Callee,
    obligations::ObligationKind, program::Elaboration,
};

#[derive(Debug, Default)]
pub(crate) struct InferredCalls {
    clauses: Vec<(MachineId, Proposition)>,
    calls: Vec<AddedCall>,
}

#[derive(Debug)]
struct AddedCall {
    origin: usize,
    caller: MachineId,
    invocation: InvocationId,
    point: ProgramPoint,
    position: CallPosition,
    obligation: ObligationId,
}

impl InferredCalls {
    pub(super) fn add(&mut self, callee: MachineId, clause: &Proposition) -> usize {
        let ordinal = self.clauses.len();
        self.clauses.push((callee, clause.clone()));
        ordinal
    }

    pub(super) fn call(&mut self, origin: usize, site: &Arrival, obligation: ObligationId) {
        self.calls.push(AddedCall {
            origin,
            caller: site.caller,
            invocation: site.invocation,
            point: site.before,
            position: site.position,
            obligation,
        });
    }

    pub(crate) fn checked(
        self,
        program: &Elaboration,
        discharge: &Discharge,
    ) -> Checked<terminal_psi::InvocationPrivateInputs> {
        let mut clauses = Vec::with_capacity(self.clauses.len());
        for (callee, predicate) in self.clauses {
            let ordinal = program
                .module
                .machine(callee)
                .and_then(|machine| {
                    machine
                        .requires
                        .iter()
                        .position(|clause| clause == &predicate)
                })
                .and_then(|i| u32::try_from(i).ok())
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        callee,
                        Construct::Machine,
                        "the inferred input lost its canonical clause",
                    )
                })?;
            clauses.push((callee, ordinal, predicate));
        }
        let mut calls = Vec::with_capacity(self.calls.len());
        for added in self.calls {
            let mismatch = || {
                refusal(
                    RefusalReason::ForeignIdentity,
                    added.caller,
                    Construct::Obligation(added.obligation),
                    "the inferred input no longer belongs to its checked call",
                )
            };
            let &(callee, clause, _) = clauses.get(added.origin).ok_or_else(mismatch)?;
            let tables = program
                .tables
                .machines
                .get(added.caller)
                .ok_or_else(mismatch)?;
            let row = tables
                .obligations
                .get(added.obligation)
                .ok_or_else(mismatch)?;
            let call = tables
                .invocations
                .get(added.invocation)
                .ok_or_else(mismatch)?;
            let ordinal = clause as usize;
            if row.kind
                != (ObligationKind::CallRequirement {
                    invocation: added.invocation,
                    clause,
                })
                || row.created_at != added.point
                || call.callee != Callee::Declared(TargetMachineCallable::Machine(callee))
                || call.requirements.get(ordinal) != Some(&added.obligation)
            {
                return Err(mismatch());
            }
            let machine = program.module.machine(added.caller).ok_or_else(mismatch)?;
            let (site, physical) = match added.position {
                CallPosition::Operation { block, operation } => {
                    let op = machine
                        .blocks
                        .get(block)
                        .and_then(|block| block.operations.get(operation))
                        .ok_or_else(mismatch)?;
                    (
                        InvocationSite::Operation(op.id),
                        op.declared_invocation().ok_or_else(mismatch)?,
                    )
                }
                CallPosition::NoNormal { block } => machine
                    .blocks
                    .get(block)
                    .and_then(|block| block.terminator.declared_invocation())
                    .ok_or_else(mismatch)?,
            };
            if site != call.site
                || physical.required_contract.is_some()
                || physical.callee != TargetMachineCallable::Machine(callee)
                || physical.requirement_obligations.get(ordinal) != Some(&added.obligation)
            {
                return Err(mismatch());
            }
            if matches!(
                discharge
                    .machines
                    .get(added.caller)
                    .and_then(|m| m.verdicts.get(added.obligation)),
                Some(Ok(Discharged::Certificate(_)))
            ) {
                calls.push((added.caller, site, callee, clause, added.obligation));
            }
        }
        terminal_psi::InvocationPrivateInputs::from_checked(clauses, calls).map_err(|detail| {
            refusal(
                RefusalReason::ForeignIdentity,
                MachineId::new(1).expect("one is nonzero"),
                Construct::Machine,
                detail,
            )
        })
    }
}
