//! Emit calls of exact boundary requirements, preserving their two argument
//! lanes and the requirement's own normal or external completion behavior.

use semantic_vocabulary::{BoundaryCompletion, BoundaryMachineId, InvocationSite};
use terminal_check_tables::Checked;
use terminal_psi::Terminator;

use super::Lowering;
use super::calls::{Arguments, EvaluatedCall, EvaluatedCallTarget};
use crate::{Outcome, Signature, SourceOrigin, canonical};

impl Lowering<'_> {
    pub(super) fn boundary_signature(&self, boundary: BoundaryMachineId) -> Checked<Signature> {
        self.boundaries
            .signature(boundary)
            .cloned()
            .ok_or_else(|| self.internal("boundary call has no retained signature"))
    }
    pub(super) fn boundary_outcome(&self, boundary: BoundaryMachineId) -> Checked<Outcome> {
        self.boundaries
            .outcome(boundary)
            .ok_or_else(|| self.internal("boundary call has no retained completion contract"))
    }
    pub(super) fn invoke_boundary_no_normal(
        &mut self,
        boundary: BoundaryMachineId,
        arguments: Arguments,
        acknowledgement: language_core::CallOperationalAcknowledgement,
        source: SourceOrigin,
    ) -> Checked<()> {
        let Some(BoundaryCompletion::External(kind)) = self.boundaries.completion(boundary) else {
            return Err(
                self.internal("no-normal boundary call has no external completion contract")
            );
        };
        if !arguments.erased.is_empty() {
            return Err(self.internal("external completion cannot discard erased actuals"));
        }
        let signature = self.boundary_signature(boundary)?;
        let crash_continuations = canonical::substituted_routes(
            &self.boundary_outcome(boundary)?.crash_routes,
            &arguments.scalars,
            self.id,
        )?;
        if !crash_continuations.is_empty() {
            return Err(self.internal("external completion that declares a crash route"));
        }
        let before = self.here()?;
        let block = self.current()?;
        let loans = self.loans(&arguments.loans, before, source);
        let edge = self.no_successor_edge(Self::terminal_loan_dispositions(&loans), source)?;
        let requirements = self.invocation(
            InvocationSite::ExternalCompletion(edge),
            EvaluatedCall {
                target: EvaluatedCallTarget::Boundary { boundary },
                acknowledgement,
            },
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
        )?;
        let structural_arguments = self.structural_arguments(
            arguments.structural,
            terminal_check_tables::obligations::ExecutionSite::Block(block),
            source,
        )?;
        self.terminate(Terminator::ExternalCompletion {
            edge,
            boundary,
            kind,
            arguments: arguments.scalars,
            structural_arguments,
            requirement_obligations: requirements,
            frontier_lower_bound: Vec::new(),
        })
    }
}
