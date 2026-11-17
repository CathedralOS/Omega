/*
Emission needs a few facts that were settled before instruction selection
and that the current machine program does not carry: the admitted entry,
callback and interrupt cohorts, the optimized abstract program that source
attribution names, and the identities that tie physical evidence to that
program. The native driver copies exactly those facts out of stage 02's
output into `EmissionFacts` and hands the record to fragment emission.
Emission never holds stage 02's output or the optimization unit; an explicit
correspondence audit keeps those as separate replay evidence (pipeline.md
"Portable materialization and consumer ownership").

Every field is shared immutable data: the abstract program is the same `Arc`
stage 01 published, so building the record copies no program.
*/

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
use verified_optimization_input::AdmittedLoweringFacts;

/// What fragment, object and image emission read from before instruction
/// selection. The native driver builds it once per target.
#[derive(Debug, Clone)]
pub struct EmissionFacts {
    /// The admitted Terminal module, entry, callback and interrupt cohorts.
    pub lowering: AdmittedLoweringFacts,
    /// The optimized abstract program, which semantic attribution and
    /// physical evidence occurrences name.
    pub abstract_program: Arc<AbstractOperationPlan>,
    /// Identities binding the Terminal program to the optimized abstract
    /// program, for physical evidence scope.
    pub projection: OptimizedProgramProjection,
}

/// Identities stage 01 published for its validated abstract projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramProjection {
    pub identity: OptimizedAbstractPlanProjectionIdentity,
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub final_unit: OptimizationUnitIdentity,
}
