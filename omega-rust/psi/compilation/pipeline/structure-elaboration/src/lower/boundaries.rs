//! Emit calls of exact boundary requirements, preserving their two argument
//! lanes and the requirement's own normal or external completion behavior.

use semantic_vocabulary::{
    BoundaryCompletion, BoundaryMachineId, InvocationSite, ObligationId, OperationId,
};
use terminal_check_tables::Checked;
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_psi::{CrashRouteBucket, OperationKind, Terminator};

use super::calls::{Arguments, EvaluatedCallTarget};
use super::{Lowering, Value};
use crate::{Outcome, Signature, SourceOrigin, ValueType, canonical};

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
    pub(super) fn emit_boundary_call(
        &mut self,
        boundary: BoundaryMachineId,
        arguments: &Arguments,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        result: Option<ValueType>,
        operation: OperationId,
        requirements: Vec<ObligationId>,
        _crash_continuations: Vec<CrashRouteBucket>,
    ) -> Checked<Value> {
        if self.boundaries.completion(boundary) != Some(BoundaryCompletion::NormalReturn) {
            return Err(self.internal("external completion used as a returning boundary operation"));
        }
        if !arguments.erased.is_empty() {
            return Err(self.internal("boundary call cannot discard erased actuals"));
        }
        // A boundary's crash routes live on its declaration. The common call
        // owner already minted coverage obligations from those exact routes;
        // this operation has no separate continuation roster in Terminal Psi.
        let kind = OperationKind::BoundaryCall {
            boundary,
            arguments: arguments.scalars.clone(),
            structural_arguments,
            completion_receipts: Vec::new(),
            requirement_obligations: requirements,
        };
        match result {
            Some(ValueType::Scalar(scalar) | ValueType::Qualified(scalar, _)) => {
                self.emit(operation, Some(scalar), kind)
            }
            Some(ValueType::Record(record)) => {
                let (root, exact) = self.produce(operation, record, kind)?;
                self.unknown_contents(exact, operation)?;
                Ok(Value::Record(root))
            }
            None => self.emit(operation, None, kind),
            Some(ValueType::Rigid(_)) => {
                Err(self.internal("boundary call retains an unresolved rigid result"))
            }
        }
    }
    pub(super) fn invoke_boundary_no_normal(
        &mut self,
        boundary: BoundaryMachineId,
        arguments: Arguments,
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
        let before = self.here()?;
        let block = self.current()?;
        let loans = self.loans(&arguments.loans, before, source);
        let edge = self.no_successor_edge(source)?;
        self.terminal_loan_dispositions(
            edge,
            &loans,
            terminal_check_tables::edges::Fate::Abandoned,
        )?;
        let requirements = self.invocation(
            InvocationSite::ExternalCompletion(edge),
            EvaluatedCallTarget::Boundary {
                boundary,
                build: None,
            },
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
        )?;
        for bucket in crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Block(block),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
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
