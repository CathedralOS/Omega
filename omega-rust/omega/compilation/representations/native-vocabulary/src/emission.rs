//! Provenance and semantic obligations carried out of native planning.
use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};

/// Exact native reach used to check an emitted object's boundary calls and
/// callback registrations after the checked target selection has been released.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedExecutionScope {
    machines: Vec<MachineId>,
    boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
    callbacks: Vec<EmittedCallbackOccurrence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmittedCallbackOccurrence {
    pub application_index: u32,
    pub terminal_operation: OperationId,
}

impl EmittedExecutionScope {
    pub fn new(
        mut machines: Vec<MachineId>,
        mut boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
        callbacks: Vec<EmittedCallbackOccurrence>,
    ) -> Self {
        machines.sort_unstable();
        boundaries.sort_unstable();
        Self {
            machines,
            boundaries,
            callbacks,
        }
    }

    pub fn machines(&self) -> &[MachineId] {
        &self.machines
    }

    pub fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
    }

    pub fn contains_boundary(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub fn callbacks(&self) -> &[EmittedCallbackOccurrence] {
        &self.callbacks
    }

    pub fn boundary_application_demands(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<Vec<terminal_psi::TerminalBoundaryApplicationDemand>, &'static str> {
        if module
            .boundary_application_demands
            .windows(2)
            .any(|rows| rows[0].terminal_operation().get() >= rows[1].terminal_operation().get())
        {
            return Err("Terminal boundary demand catalog is not strictly ordered");
        }
        let catalog = &module.boundary_application_demands;
        if catalog.is_empty() {
            return Ok(Vec::new());
        }
        // Only operations named by a demand need an owner. Still visit every
        // machine so a duplicate in dormant code cannot satisfy the check.
        let mut owners = vec![None; catalog.len()];
        for machine in &module.machines {
            for operation in machine.operations() {
                if let Ok(position) = catalog.binary_search_by_key(&operation.id.get(), |demand| {
                    demand.terminal_operation().get()
                }) && owners[position].replace(machine.id).is_some()
                {
                    return Err("Terminal boundary demand has no unique operation owner");
                }
            }
        }
        let mut demands = Vec::new();
        for (demand, owner) in catalog.iter().zip(owners) {
            let owner = owner.ok_or("Terminal boundary demand has no operation owner")?;
            if self.contains_machine(owner) {
                demands.push(demand.clone());
            }
        }
        Ok(demands)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionProgramFacts {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub entry: MachineId,
    pub functions: Vec<EmissionFunctionFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionFunctionFacts {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub parameters: Vec<crate::values::AbstractParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub result: crate::values::AbstractFunctionResult,
    pub entry_claims: Vec<terminal_psi::EntryClaim>,
    pub source_occurrences: Vec<EmissionOperationProvenance>,
    pub requires_graph_storage_replay: bool,
    pub descriptors: Vec<DescriptorProducerFact>,
    pub primitive_locals: Vec<PrimitiveLocalFact>,
    pub scalar_call_results: Vec<ScalarCallResultFact>,
    pub ram_obligations: Vec<RamObligation>,
}

/// One original ordinal, including operations without a source operation ID.
/// These coordinates carry attribution; they cannot reconstruct executable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmissionOperationProvenance {
    pub operation: Option<OperationId>,
    pub boundary: Option<(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::InvocationSite,
    )>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorProducerFact {
    pub operation: OperationId,
    pub descriptor: DescriptorProducer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorProducer {
    ByteSubslice(terminal_psi::StructuralOperationResult),
    ElementView(terminal_psi::StructuralOperationResult),
    ByteLiteral {
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveLocalFact {
    pub operation: OperationId,
    pub result: terminal_psi::StructuralOperationResult,
    pub initializer: crate::values::AbstractResult,
}

impl EmissionFunctionFacts {
    pub fn primitive_local(&self, place: PlaceId) -> Option<&PrimitiveLocalFact> {
        let mut matching = self
            .primitive_locals
            .iter()
            .filter(|row| row.result.place == place);
        let result = matching.next()?;
        matching.next().is_none().then_some(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarCallResultFact {
    pub operation: OperationId,
    pub target: ScalarCallOrigin,
    pub result: crate::values::AbstractResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarCallOrigin {
    Authored(crate::calls::AbstractCallTarget),
    Boundary(semantic_vocabulary::BoundaryMachineId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamObligation {
    pub operation: Option<OperationId>,
    pub source: RamSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamSource {
    PlacedBacking,
    Boundary(OperationId),
    ExternalCompletion(semantic_vocabulary::EdgeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramProjection {
    pub identity: OptimizedAbstractPlanProjectionIdentity,
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub final_unit: OptimizationUnitIdentity,
}
