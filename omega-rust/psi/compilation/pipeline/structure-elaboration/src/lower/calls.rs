/*
The call family of lowering: invocation rows and their `CallRequirement`
obligations, calls as operations, no-normal calls as terminators, and the
crash continuations and `CrashCoverage` obligations of both.
*/

use semantic_vocabulary::{InvocationSite, MachineId, ObligationId, ScalarType, ValueId};
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_check_tables::{Checked, Construct, ProgramPoint, Refusal, RefusalReason};
use terminal_psi::{OperationKind, Terminator};

use super::{Lowering, Value};
use crate::body::{Expression, ExpressionKind};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{SourceOrigin, Unserved, refuse};

impl Lowering<'_> {
    fn outcome(&self, callee: MachineId) -> Checked<&'_ Outcome> {
        terminal_check_tables::row(self.outcomes, callee, self.id, Construct::Machine)
    }

    /// The callee and arguments of a call in terminal position whose callee
    /// has no normal completion; such a call is the block's terminator.
    pub(super) fn no_normal<'e>(
        &self,
        expression: &'e Expression,
    ) -> Checked<Option<(MachineId, &'e [Expression])>> {
        match &expression.kind {
            ExpressionKind::Call { callee, arguments }
                if !self.outcome(*callee)?.completes_normally =>
            {
                Ok(Some((*callee, arguments)))
            }
            ExpressionKind::Call { .. }
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_)
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::ShortCircuit { .. } => Ok(None),
        }
    }

    /// A call of a machine with no normal completion, in terminal position:
    /// the arguments, then the invocation as the block's terminator, with the
    /// callee's routes substituted as its crash continuations and one
    /// `CrashCoverage` obligation per surviving cause at this site.
    pub(super) fn invoke_no_normal(
        &mut self,
        callee: MachineId,
        arguments: &[Expression],
        source: SourceOrigin,
    ) -> Checked<()> {
        let mut actuals = Vec::with_capacity(arguments.len());
        for argument in arguments {
            actuals.push(self.scalar(argument)?);
        }
        let crash_continuations =
            canonical::substituted_routes(&self.outcome(callee)?.crash_routes, &actuals, self.id)?;
        let site = self.current()?;
        let before = self.here()?;
        let edge = self.no_successor_edge(source)?;
        let requirements = self.invocation(
            InvocationSite::InvokeNoNormal(edge),
            callee,
            &actuals,
            before,
            source,
        )?;
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Block(site),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        self.terminate(Terminator::InvokeNoNormal {
            edge,
            static_reach_binding: None,
            callee,
            arguments: actuals,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: requirements,
            crash_continuations,
        })
    }

    /// The invocation row of one call and its `CallRequirement` obligations,
    /// one per callee `requires` clause, created at the point before it.
    fn invocation(
        &mut self,
        site: InvocationSite,
        callee: MachineId,
        actuals: &[ValueId],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Checked<Vec<ObligationId>> {
        let clauses =
            terminal_check_tables::row(self.signatures, callee, self.id, Construct::Machine)?
                .requires
                .len();
        let invocation = self.tables.invocations.push(InvocationRow {
            site,
            callee: Callee::Machine(callee),
            actuals: actuals.iter().copied().map(Actual::Value).collect(),
            requirements: Vec::new(),
        });
        let mut requirements = Vec::with_capacity(clauses);
        for clause in 0..clauses {
            let clause = self.position(clause)?;
            requirements.push(self.obligation(
                ObligationKind::CallRequirement { invocation, clause },
                before,
                source,
            ));
        }
        let id = self.id;
        self.tables
            .invocations
            .get_mut(invocation)
            .ok_or(Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Invocation(invocation),
                detail: String::from("invocation is past the end of this machine"),
            })?
            .requirements
            .clone_from(&requirements);
        Ok(requirements)
    }

    pub(super) fn call(
        &mut self,
        callee: MachineId,
        arguments: &[Expression],
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // A call is an operation only when its callee may return. One with
        // no normal completion is a terminator (`invoke_no_normal`), reached
        // only in terminal position.
        let outcome = self.outcome(callee)?;
        if !outcome.completes_normally {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Call("no-normal call inside an expression"),
            ));
        }
        let mut actuals = Vec::with_capacity(arguments.len());
        for argument in arguments {
            actuals.push(self.scalar(argument)?);
        }
        // A callee that may return and also publishes crash routes makes
        // this operation a crash site: its routes, with each formal replaced
        // by the actual at its position, are the call's crash continuations,
        // and the site owes one `CrashCoverage` per surviving cause, as an
        // `InvokeNoNormal` terminator does at its block.
        let crash_continuations =
            canonical::substituted_routes(&self.outcome(callee)?.crash_routes, &actuals, self.id)?;
        let before = self.here()?;
        let operation = self.operation_identity(source);
        let requirements = self.invocation(
            InvocationSite::Operation(operation),
            callee,
            &actuals,
            before,
            source,
        )?;
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Operation(operation),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        let kind = match result {
            Some(_) => OperationKind::Call {
                callee,
                arguments: actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: requirements,
                crash_continuations,
            },
            None => OperationKind::CallUnit {
                callee,
                arguments: actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: requirements,
                crash_continuations,
            },
        };
        self.emit(operation, result, kind)
    }
}
