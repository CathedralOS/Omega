//! Exact scalar-call signatures retain the kind of checked body that owns them.

use super::*;

pub(crate) enum CheckedScalarCallee<'checked> {
    Graph(&'checked checked_trees::CheckedScalarMachineGraph),
    Boundary(&'checked CheckedBoundaryScalarReturnMachinePlan),
    Structural(&'checked CheckedStructuralScalarReturnMachinePlan),
}

impl<'checked> CheckedScalarCallee<'checked> {
    pub(crate) fn find(
        checked: &'checked CheckedTrees,
        source: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let callee = Self::find_for_unit_call(checked, source)?;
        if !callee.structural_parameters().is_empty() || !callee.entry_claims().is_empty() {
            return unsupported("scalar callee requires structural call custody");
        }
        Ok(callee)
    }

    /// Select a body whose caller retains an explicit structural transfer lane.
    /// Scalar-only edges must continue to use the narrower `find` entry.
    pub(crate) fn find_for_unit_call(
        checked: &'checked CheckedTrees,
        source: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let mut graphs = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter()
            .filter(|plan| plan.machine == source);
        let mut boundaries = checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .filter(|plan| plan.machine == source);
        let mut structural = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .filter(|plan| plan.machine == source);
        let selected = (graphs.next(), boundaries.next(), structural.next());
        if graphs.next().is_some() || boundaries.next().is_some() || structural.next().is_some() {
            return unsupported("scalar callee has duplicate checked body ownership");
        }
        match selected {
            (Some(graph), None, None) => Ok(Self::Graph(graph)),
            (None, Some(boundary), None) => {
                crate::boundary_scalar_return::validate_boundary_scalar_return(checked, boundary)?;
                Ok(Self::Boundary(boundary))
            }
            (None, None, Some(plan)) => {
                crate::structural_scalar_return::validate_scalar_callee(checked, plan)?;
                Ok(Self::Structural(plan))
            }
            (None, None, None) => unsupported("scalar callee has no checked executable body"),
            _ => unsupported("scalar callee has ambiguous checked body ownership"),
        }
    }

    pub(crate) fn structural_parameters(&self) -> &[CheckedUnitStructuralParameterPlan] {
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .map_or(&[], |state| &state.structural_parameters),
            Self::Boundary(plan) => &plan.structural_parameters,
            Self::Structural(plan) => &plan.structural_parameters,
        }
    }

    pub(crate) fn source_machine(&self) -> symbols::SymbolHandle {
        match self {
            Self::Graph(plan) => plan.machine,
            Self::Boundary(plan) => plan.machine,
            Self::Structural(plan) => plan.machine,
        }
    }

    pub(crate) fn requires_structural_frame(&self) -> bool {
        !self.structural_parameters().is_empty()
            || matches!(self, Self::Graph(graph) if graph.states.iter().any(|state| !state.primitive_locals.is_empty() || !state.unit_operations.is_empty()))
    }

    pub(crate) fn entry_claims(&self) -> &[checked_trees::CheckedUnitEntryClaimPlan] {
        match self {
            Self::Boundary(plan) => &plan.entry_claims,
            Self::Graph(_) | Self::Structural(_) => &[],
        }
    }

    pub(crate) fn entry_state(&self) -> Result<symbols::SymbolHandle, LoweringError> {
        match self {
            Self::Graph(graph) => {
                graph
                    .states
                    .first()
                    .map(|state| state.state)
                    .ok_or(LoweringError::Unsupported(
                        "scalar callee has no checked entry state",
                    ))
            }
            Self::Boundary(boundary) => Ok(boundary.state),
            Self::Structural(plan) => Ok(plan.state),
        }
    }

    pub(crate) fn parameter_types(&self) -> Result<Vec<PrimitiveType>, LoweringError> {
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .map(|state| state.parameter_types.clone())
                .ok_or(LoweringError::Unsupported(
                    "scalar callee has no checked entry state",
                )),
            Self::Boundary(plan) => Ok(plan
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.primitive_type)
                .collect()),
            Self::Structural(plan) => Ok(plan
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.primitive_type)
                .collect()),
        }
    }

    pub(crate) fn result_type(&self) -> Result<ScalarType, LoweringError> {
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .ok_or(LoweringError::Unsupported(
                    "scalar callee has no checked entry state",
                ))
                .and_then(|state| terminal_scalar_type(state.result_type)),
            Self::Boundary(boundary) => terminal_scalar_type(boundary.result_type),
            Self::Structural(plan) => terminal_scalar_type(plan.result_type),
        }
    }

    pub(crate) fn prepare(
        self,
        checked: &CheckedTrees,
        source: symbols::SymbolHandle,
        embedded_root: bool,
        structural_parameters: &[StructuralParameterDeclaration],
        primitive_locals: &[crate::scalar_graph_lowering::primitive_locals::PrimitiveLocal],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<PreparedScalarCallee<'checked>, LoweringError> {
        match self {
            Self::Graph(graph) => {
                if source != graph.machine {
                    return unsupported("scalar graph preparation substituted its source owner");
                }
                let prepared = crate::scalar_graph_lowering::prepare_scalar_graph_in_namespace(
                    checked,
                    graph,
                    embedded_root,
                    structural_parameters,
                    primitive_locals,
                    structural_types,
                    next_place,
                )?;
                if !prepared.identity_reshuffles.structural_places.is_empty()
                    || !prepared.identity_reshuffles.entry_claims.is_empty()
                    || !prepared.identity_reshuffles.reshuffles.is_empty()
                    || !prepared.partition_compositions.structural_places.is_empty()
                    || !prepared.partition_compositions.compositions.is_empty()
                {
                    return unsupported(
                        "embedded scalar call structural/content effects require a dedicated terminal slice",
                    );
                }
                Ok(PreparedScalarCallee::Graph(prepared))
            }
            Self::Boundary(plan) => Ok(PreparedScalarCallee::Boundary {
                result_type: terminal_scalar_type(plan.result_type)?,
                requirement_count: usize::from(
                    !crate::boundary_scalar_return::checked_requirements(checked, plan)?.is_empty(),
                ),
                plan,
            }),
            Self::Structural(plan) => {
                crate::structural_scalar_return::validate_scalar_callee(checked, plan)?;
                Ok(PreparedScalarCallee::Structural {
                    plan,
                    result_type: terminal_scalar_type(plan.result_type)?,
                })
            }
        }
    }
}

pub(crate) enum PreparedScalarCallee<'checked> {
    Graph(PreparedScalarMachine),
    Boundary {
        plan: &'checked CheckedBoundaryScalarReturnMachinePlan,
        result_type: ScalarType,
        requirement_count: usize,
    },
    Structural {
        plan: &'checked CheckedStructuralScalarReturnMachinePlan,
        result_type: ScalarType,
    },
}

impl PreparedScalarCallee<'_> {
    pub(crate) fn source_machine(&self) -> symbols::SymbolHandle {
        match self {
            Self::Graph(graph) => graph.source_machine,
            Self::Boundary { plan, .. } => plan.machine,
            Self::Structural { plan, .. } => plan.machine,
        }
    }

    pub(crate) fn result_type(&self) -> ScalarType {
        match self {
            Self::Graph(graph) => graph.result_type,
            Self::Boundary { result_type, .. } => *result_type,
            Self::Structural { result_type, .. } => *result_type,
        }
    }

    pub(crate) fn requirement_count(&self) -> usize {
        match self {
            Self::Graph(graph) => graph.contract.requirement_count(),
            Self::Boundary {
                requirement_count, ..
            } => *requirement_count,
            Self::Structural { .. } => 0,
        }
    }
}
