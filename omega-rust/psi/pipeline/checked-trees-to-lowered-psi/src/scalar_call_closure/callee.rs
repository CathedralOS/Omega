//! Exact scalar-call signatures retain the kind of checked body that owns them.

use super::*;

pub(crate) enum CheckedScalarCallee<'checked> {
    Graph(&'checked checked_trees::CheckedScalarMachineGraph),
    Boundary(&'checked CheckedBoundaryScalarReturnMachinePlan),
}

impl<'checked> CheckedScalarCallee<'checked> {
    pub(crate) fn find(
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
        let selected = (graphs.next(), boundaries.next());
        if graphs.next().is_some() || boundaries.next().is_some() {
            return unsupported("scalar callee has duplicate checked body ownership");
        }
        match selected {
            (Some(graph), None) => Ok(Self::Graph(graph)),
            (None, Some(boundary)) => {
                // Ordinary scalar calls have no structural-argument or claim
                // lane. A static attachment is metadata, not an erased receiver.
                if !boundary.structural_parameters.is_empty() || !boundary.entry_claims.is_empty() {
                    return unsupported("scalar boundary callee requires structural call custody");
                }
                crate::boundary_scalar_return::validate_boundary_scalar_return(checked, boundary)?;
                Ok(Self::Boundary(boundary))
            }
            (Some(_), Some(_)) => unsupported("scalar callee has ambiguous checked body ownership"),
            (None, None) => {
                unsupported("scalar callee has no checked graph or boundary-return body")
            }
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
        }
    }

    pub(crate) fn prepare(
        self,
        checked: &CheckedTrees,
        source: symbols::SymbolHandle,
        embedded_root: bool,
    ) -> Result<PreparedScalarCallee<'checked>, LoweringError> {
        match self {
            Self::Graph(graph) => {
                let prepared = if embedded_root {
                    prepare_embedded_scalar_graph_machine(checked, source, graph)?
                } else {
                    prepare_scalar_graph_machine(checked, source, graph)?
                };
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
}

impl PreparedScalarCallee<'_> {
    pub(crate) fn source_machine(&self) -> symbols::SymbolHandle {
        match self {
            Self::Graph(graph) => graph.source_machine,
            Self::Boundary { plan, .. } => plan.machine,
        }
    }

    pub(crate) fn result_type(&self) -> ScalarType {
        match self {
            Self::Graph(graph) => graph.result_type,
            Self::Boundary { result_type, .. } => *result_type,
        }
    }

    pub(crate) fn requirement_count(&self) -> usize {
        match self {
            Self::Graph(graph) => graph.contract.requirement_count(),
            Self::Boundary {
                requirement_count, ..
            } => *requirement_count,
        }
    }
}
