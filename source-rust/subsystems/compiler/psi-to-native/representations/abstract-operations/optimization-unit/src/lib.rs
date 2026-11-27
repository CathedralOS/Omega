#![forbid(unsafe_code)]

/*
Target lowering (native stage 04) reads the abstract program through
`PsiOptimizationUnit`: the plan's blocks and operations plus the scalar
definitions and uses, control-flow edges, work settlements, effect links and
ownership events derived from them once. `OptimizedAbstractProgram` pairs that
unit with the unchanged `AbstractOperationPlan` and the three identities later
stages bind to (`OptimizedAbstractProgram::unoptimized` in `unoptimized`).

The unit carries two kinds of data:
1. The executable graph and its derived dataflow (`functions`,
   `root_service_reach`), sealed under an `OptimizationUnitIdentity`.
2. Custody attached once from the verified Terminal module (`structural_types`,
   `structural_domains`, `services`, `accepted_obligation_facts`,
   `runtime_evidence`).

`reconstruct_psi_optimization_unit_seed` in `construction` seeds the unit and
`proof` attaches the module context and accepted obligation facts. The sibling
`optimization-unit-semantics` crate answers descriptor, reference and
callable-value queries over it. The native abstract-operations optimizer that
rewrote units, with its rewrite candidates, ledger, ownership frontiers and
pre-physical manifest, is archived under
`_recycling/source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/`.
*/

use std::{collections::BTreeSet, sync::Arc};

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    ValueBinding,
};
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationIdentityBundleIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, IntegerValue, MachineId, ObligationId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId, WorkScheduleIdentity,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity, TerminalRootServiceReach,
};

mod construction;
mod dominance;
mod graph;
mod identity;
#[cfg(test)]
mod machine_references_tests;
mod proof;
mod structural_places;
mod unoptimized;

pub use construction::{
    OptimizationUnitBuildError, operation_definition, operation_uses,
    reconstruct_psi_optimization_function_seed, reconstruct_psi_optimization_unit_seed,
};
pub(crate) use dominance::DominatorSets;
pub(crate) use graph::ValueUse;
pub use graph::{
    CycleComponentId, EffectLink, OptimizationBlock, OptimizationEdge, OptimizationFact,
    OptimizationNode, OwnershipEvent, PsiOptimizationFunction, PsiProvenance, ValueDefinition,
    ValueDefinitionSite, WorkSettlement, encode_value_definition_site_identity,
};
pub(crate) use identity::recompute_psi_optimization_unit_identity;
pub(crate) use proof::RuntimeEvidenceCatalog;
pub use proof::{
    AcceptedObligationFact, AcceptedObligationFactIndexError, AcceptedObligationOwner,
    SourceCheckedScalarReceiving, attach_accepted_obligation_facts, attach_verified_module_context,
    validate_runtime_evidence_context,
};
pub use structural_places::{
    Establishment, Position, declared_structural_type, dominators, field_evidence,
    observed_field_declaration, shape,
};
pub use unoptimized::UnoptimizedAbstractProgramError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub entry: MachineId,
    pub structural_types: abstract_operations::StructuralTypeCatalog, // Retained after the full Terminal module is discarded.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>, // Empty on bare lowering seeds; attached before rewrites run.
    pub services: Arc<[ServiceDeclaration]>, // Complete boundary-service hierarchy for call reach and effect replay.
    pub root_service_reach: TerminalRootServiceReach, // Current-revision closure of services executable from `entry`.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog,
    pub callables: terminal_psi::TerminalCallableCatalog,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    pub identities: terminal_psi::TerminalIdentityTable, // Sealed module table that every `TerminalIdentityIndex` resolves against.
    pub runtime_evidence: Option<Arc<RuntimeEvidenceCatalog>>, // Present on all verified units; `None` only on bare seeds.
    pub functions: Vec<PsiOptimizationFunction>,
}

/// The machine a direct call transfers control to, whether or not the callee
/// returns normally. Call-graph analysis and the pruning validator both read
/// callees through `module_invocation`, so they cannot disagree about reachability.
fn direct_call_machine(operation: &AbstractOperation) -> Option<MachineId> {
    operation
        .module_invocation()
        .map(|invocation| invocation.callee)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallMachineReference {
    Declaration(MachineId),
    Callable {
        callee: semantic_vocabulary::ValueId,
        contract: semantic_vocabulary::CallableContractId,
    },
}

impl PsiOptimizationUnit {
    pub fn function(&self, machine: MachineId) -> Option<&PsiOptimizationFunction> {
        self.function_index(machine)
            .map(|index| &self.functions[index])
    }

    pub fn function_mut(&mut self, machine: MachineId) -> Option<&mut PsiOptimizationFunction> {
        self.function_index(machine)
            .map(|index| &mut self.functions[index])
    }

    pub fn function_index(&self, machine: MachineId) -> Option<usize> {
        self.functions
            .binary_search_by_key(&machine, |function| function.machine)
            .ok()
    }

    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &OptimizationBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    pub fn block_nodes(&self) -> impl Iterator<Item = &OptimizationNode> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.block_nodes())
    }

    pub fn call_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = CallMachineReference> + 'a {
        let (machine, dynamic, callable) = if let Some(callee) = direct_call_machine(operation) {
            (Some(callee), None, None)
        } else {
            match operation {
                AbstractOperation::CallStructural { callee, .. } => match callee {
                    abstract_operations::AbstractCallTarget::Machine(_) => (None, None, None),
                    abstract_operations::AbstractCallTarget::Dynamic {
                        interface, slot, ..
                    } => (None, Some((*interface, *slot)), None),
                    abstract_operations::AbstractCallTarget::Callable { callee, contract } => (
                        None,
                        None,
                        Some(CallMachineReference::Callable {
                            callee: *callee,
                            contract: *contract,
                        }),
                    ),
                },
                _ => (None, None, None),
            }
        };
        machine
            .into_iter()
            .chain(
                self.dynamic_dispatch
                    .tables
                    .iter()
                    .filter_map(move |table| {
                        let (interface, slot) = dynamic?;
                        (table.interface == interface).then_some(())?;
                        *table.rows.get(usize::try_from(slot).ok()?)?
                    }),
            )
            .map(CallMachineReference::Declaration)
            .chain(callable)
    }

    pub fn operation_machine_references<'a>(
        &'a self,
        operation: &'a AbstractOperation,
    ) -> impl Iterator<Item = MachineId> + 'a {
        let (machine, table, interface) = if let Some(callee) = direct_call_machine(operation) {
            (Some(callee), None, None)
        } else {
            match operation {
                AbstractOperation::EstablishNativeCodeLayout { machine, .. } => {
                    (Some(*machine), None, None)
                }
                AbstractOperation::CallStructural { callee, .. } => match callee {
                    abstract_operations::AbstractCallTarget::Machine(_) => (None, None, None),
                    abstract_operations::AbstractCallTarget::Dynamic { interface, .. } => {
                        (None, None, Some(*interface))
                    }
                    // Runtime code has no nominal reference. Its construction
                    // retains the selected body through SelectMachine below.
                    abstract_operations::AbstractCallTarget::Callable { .. } => (None, None, None),
                },
                AbstractOperation::SelectMachine { descriptor, .. } => (
                    self.callables
                        .descriptor(*descriptor)
                        .map(|row| row.machine),
                    None,
                    None,
                ),
                AbstractOperation::SelectImplementation { table, .. } => (None, Some(*table), None),
                _ => (None, None, None),
            }
        };
        machine.into_iter().chain(
            self.dynamic_dispatch
                .tables
                .iter()
                .filter(move |candidate| {
                    table == Some(candidate.id) || interface == Some(candidate.interface)
                })
                .flat_map(|table| table.rows.iter().copied().flatten()),
        )
    }
}

/// The unchanged plan, the unit target lowering reads, and the identities later
/// stages compare: `optimization` names the selections the plan was lowered
/// under, `projection` binds the Terminal program to the unit, and
/// `pre_physical_manifest` binds the projection to the selections and budget.
#[derive(Debug)]
pub struct OptimizedAbstractProgram {
    pub plan: Arc<AbstractOperationPlan>,
    pub unit: PsiOptimizationUnit,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
}

impl OptimizedAbstractProgram {
    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub fn shared_program(&self) -> Arc<AbstractOperationPlan> {
        Arc::clone(&self.plan)
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn optimization(&self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }

    pub const fn projection(&self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn pre_physical_manifest(&self) -> PrePhysicalOptimizationManifestIdentity {
        self.pre_physical_manifest
    }
}

pub(crate) use semantic_vocabulary::identity_encoding::encode_scalar_type;
