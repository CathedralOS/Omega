//! Freeze the inputs to an explicitly requested target correspondence audit.
//! The emitted facts and the archive receive the same fresh origin. A retained
//! object can only use the archive issued with its own publication.

use super::SemanticEmissionFacts;
use crate::AdmittedLoweringFacts;
use legalized_operations::PreparedExecutionPlan;
use optimization_unit::OptimizedAbstractProgram;
use std::sync::Arc;
use target_operations::TargetOperationPlan;

#[derive(Debug, Clone)]
pub struct RetainedTargetOperations(Arc<TargetOperationsForReplay>);

#[derive(Debug)]
pub struct TargetOperationsForReplay {
    publication: SemanticEmissionFacts,
    optimized: OptimizedAbstractProgram,
    lowering: AdmittedLoweringFacts,
    working: Arc<TargetOperationPlan>,
    prepared: Arc<PreparedExecutionPlan>,
}

impl SemanticEmissionFacts {
    pub fn retain_target_operations(
        &mut self,
        optimized: OptimizedAbstractProgram,
        lowering: AdmittedLoweringFacts,
        working: Arc<TargetOperationPlan>,
        prepared: Arc<PreparedExecutionPlan>,
    ) -> Result<RetainedTargetOperations, &'static str> {
        let plan = prepared.plan();
        if self.target_audit.is_some()
            || !self.admission.belongs_to(&lowering)
            || self.program.psi != working.psi
            || self.program.entry != working.entry
            || self.projection.identity != optimized.projection()
            || self.projection.terminal != optimized.projection_psi()
            || self.projection.final_unit != optimized.final_unit()
            || optimized.plan().psi != working.psi
            || lowering.closed_module().identity() != working.psi
            || plan.psi != working.psi
            || plan.target != working.target
            || plan.entry != working.entry
            || plan.profile != working.profile
            || prepared.receipt().optimization_unit() != optimized.unit().identity
            || prepared.receipt().work_schedule() != optimized.unit().work_schedule
        {
            return Err(
                "retained target operations differ from the emitted program or its sealed execution plan",
            );
        }
        self.target_audit = Some(Arc::new(()));
        Ok(RetainedTargetOperations(Arc::new(
            TargetOperationsForReplay {
                publication: self.clone(),
                optimized,
                lowering,
                working,
                prepared,
            },
        )))
    }
}

impl RetainedTargetOperations {
    pub fn target_for_correspondence(&self) -> &TargetOperationsForReplay {
        &self.0
    }

    pub fn belongs_to(&self, publication: &SemanticEmissionFacts) -> bool {
        let original = &self.0.publication;
        original
            .target_audit
            .as_ref()
            .zip(publication.target_audit.as_ref())
            .is_some_and(|(expected, offered)| Arc::ptr_eq(expected, offered))
            && original == publication
    }
}

impl TargetOperationsForReplay {
    pub fn optimized(&self) -> &OptimizedAbstractProgram {
        &self.optimized
    }
    pub fn lowering_facts(&self) -> &AdmittedLoweringFacts {
        &self.lowering
    }
    pub fn target(&self) -> target::NativeTarget {
        self.working.target
    }
    pub fn execution_plan(&self) -> &PreparedExecutionPlan {
        &self.prepared
    }
    pub fn working_program_for_validation(&self) -> &TargetOperationPlan {
        &self.working
    }
    pub fn boundary_origins(&self) -> BoundaryOriginFacts<'_> {
        BoundaryOriginFacts::from_lowering(&self.lowering)
    }
}

/// Exact boundary sources used to verify one target execution plan.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryOriginFacts<'a> {
    pub program_entry: Option<target_operations::ProgramEntryOrigin>,
    pub assumed_bindings:
        &'a [operation_evidence::provider_calls::InstalledAssumedBindingEvidence],
    pub target_context: Option<&'a terminal_installation::TerminalTargetContext>,
    pub module: &'a terminal_psi::TerminalModule,
}

impl<'a> BoundaryOriginFacts<'a> {
    pub fn from_lowering(input: &'a AdmittedLoweringFacts) -> Self {
        Self {
            program_entry: input.program_entry().map(|call| {
                target_operations::ProgramEntryOrigin {
                    boundary: call.boundary(),
                    entry: call.entry(),
                    source_signature: call.source_signature(),
                }
            }),
            assumed_bindings: input.assumed_bindings(),
            target_context: input.target_context(),
            module: input.module(),
        }
    }
}

/// The target stage issues its facts together with any explicitly requested
/// original. Each subsequent stage replaces the facts when it changes form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPublication<Facts> {
    pub facts: Facts,
    original: Option<RetainedTargetOperations>,
}

impl BodyPublication<SemanticEmissionFacts> {
    pub fn new(
        facts: SemanticEmissionFacts,
        original: Option<RetainedTargetOperations>,
    ) -> Result<Self, &'static str> {
        if original
            .as_ref()
            .is_some_and(|target| !target.belongs_to(&facts))
        {
            return Err("target replay belongs to another publication");
        }
        Ok(Self { facts, original })
    }
}

impl<Facts> BodyPublication<Facts> {
    pub fn map_facts<Next>(self, map: impl FnOnce(Facts) -> Next) -> BodyPublication<Next> {
        BodyPublication {
            facts: map(self.facts),
            original: self.original,
        }
    }

    pub fn original_for_audit(&self) -> Option<&RetainedTargetOperations> {
        self.original.as_ref()
    }

    pub fn release_original(&mut self) {
        self.original = None;
    }
}

impl PartialEq for RetainedTargetOperations {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RetainedTargetOperations {}
