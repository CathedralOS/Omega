//! Exact scalar-call signatures retain the kind of checked body that owns them.
use super::super::{
    CheckedUnitStructuralParameterPlan, ScalarType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, terminal_scalar_type,
};
use super::{CheckedTrees, LoweringError, PrimitiveType, unsupported};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::scalar_graph::scalar_graph_lowering::ScalarContractMode;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::PreparedScalarMachine;

/// The checked body a scalar call lowers: the Unit body that owns its scalar
/// completion, else its scalar graph. Stage 04 plans a Unit body for a
/// graph-owned machine too, so the graph is only the fallback for a body the
/// Unit planner cannot express yet. A machine with a boundary-scalar or
/// structural-scalar return plan always also owns a Unit body, which lowers
/// its boundary call or structural frontier through the shared Unit
/// assembler; those plans only describe the callee to call admission.
pub(crate) enum CheckedScalarCallee<'checked> {
    Graph(&'checked checked_trees::CheckedScalarMachineGraph),
    /// A Unit-closure body that owns a scalar completion: an ordinary
    /// single-state body's scalar result or control, or a state graph's
    /// scalar result. The shared Unit assembler emits it with the closure.
    Operations(crate::unit::attached_unit::bodies::UnitBody<'checked>),
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
        let graph = graphs.next();
        let mut operation_bodies = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .filter(|plan| plan.machine == source);
        let operation_body = operation_bodies.next();
        let mut graph_bodies = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .filter(|plan| plan.machine == source);
        let graph_body = graph_bodies.next();
        if graphs.next().is_some()
            || operation_bodies.next().is_some()
            || graph_bodies.next().is_some()
            || (operation_body.is_some() && graph_body.is_some())
        {
            return unsupported("scalar callee has duplicate checked body ownership");
        }
        if let Some(plan) = graph_body.filter(|plan| {
            matches!(
                plan.result,
                checked_trees::CheckedControlResultPlan::Scalar { .. }
            )
        }) {
            return Ok(Self::Operations(
                crate::unit::attached_unit::bodies::UnitBody::Composed(plan),
            ));
        }
        if let Some(plan) = operation_body
            .filter(|plan| plan.scalar_result.is_some() || plan.scalar_control.is_some())
        {
            if plan.structural_result.is_none() {
                return Ok(Self::Operations(
                    crate::unit::attached_unit::bodies::UnitBody::Ordinary(plan),
                ));
            }
            if graph.is_none() {
                return unsupported("scalar callee has conflicting checked result ownership");
            }
        }
        match graph {
            Some(graph) => Ok(Self::Graph(graph)),
            None => Err(LoweringError::InvalidUnitMachinePlan {
                machine: checked.symbols.display_path(source, "::"),
                reason: "scalar callee has no checked executable body",
                gap: checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .gap_diagnostic(source, &checked.symbols),
            }),
        }
    }

    pub(crate) fn structural_parameters(&self) -> &'checked [CheckedUnitStructuralParameterPlan] {
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .map_or(&[], |state| &state.structural_parameters),
            Self::Operations(body) => body
                .entry()
                .map_or(&[][..], |entry| entry.structural_parameters),
        }
    }

    pub(crate) fn source_machine(&self) -> symbols::SymbolHandle {
        match self {
            Self::Graph(plan) => plan.machine,
            Self::Operations(body) => body.machine(),
        }
    }

    pub(crate) fn requires_structural_frame(&self) -> bool {
        !self.structural_parameters().is_empty()
            || matches!(self, Self::Operations(_))
            || matches!(self, Self::Graph(graph) if graph.states.iter().any(|state| !state.primitive_locals.is_empty() || !state.unit_operations.is_empty()))
    }

    pub(crate) fn entry_claims(&self) -> &[checked_trees::CheckedUnitEntryClaimPlan] {
        match self {
            Self::Operations(body) => body.entry().map_or(&[][..], |entry| entry.entry_claims),
            Self::Graph(_) => &[],
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
            Self::Operations(body) => Ok(body.entry()?.state),
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
            Self::Operations(body) => Ok(body
                .entry()?
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.primitive_type)
                .collect()),
        }
    }

    /// Proof-only erased scalar formals as (authored position, primitive type)
    /// pairs in authored order. Empty for callees with no erased roster.
    pub(crate) fn erased_parameters(&self) -> Vec<(u32, PrimitiveType)> {
        fn erased(
            parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
        ) -> Vec<(u32, PrimitiveType)> {
            parameters
                .iter()
                .map(|parameter| (parameter.source_position, parameter.primitive_type))
                .collect()
        }
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .map(|state| erased(&state.erased_scalar_parameters))
                .unwrap_or_default(),
            Self::Operations(body) => body
                .entry()
                .map(|entry| erased(entry.erased_scalar_parameters))
                .unwrap_or_default(),
        }
    }

    /// Proof-only erased formals in authored order. Empty for callees with
    /// no erased-proof roster.
    pub(crate) fn erased_proof_parameters(
        &self,
    ) -> Vec<&checked_trees::CheckedErasedProofParameterPlan> {
        match self {
            Self::Graph(graph) => graph
                .states
                .first()
                .map(|state| state.erased_proof_parameters.iter().collect())
                .unwrap_or_default(),
            Self::Operations(body) => body
                .entry()
                .map(|entry| entry.erased_proof_parameters.iter().collect())
                .unwrap_or_default(),
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
                .and_then(|state| {
                    // A callee with a structural result is not admissible in
                    // a scalar call position: the value never reaches a
                    // scalar slot.
                    state
                        .result_type
                        .ok_or(LoweringError::Unsupported(
                            "scalar callee's result is structural",
                        ))
                        .and_then(terminal_scalar_type)
                }),
            Self::Operations(body) => body
                .scalar_result_type()
                .ok_or(LoweringError::Unsupported(
                    "scalar operation body has no checked scalar result",
                ))
                .and_then(terminal_scalar_type),
        }
    }

    pub(crate) fn prepare(
        self,
        checked: &CheckedTrees,
        qualifications: &PreparedScalarQualifications,
        contract_mode: ScalarContractMode,
        structural_parameters: &[StructuralParameterDeclaration],
        primitive_locals: &[crate::scalar_graph::scalar_graph_lowering::primitive_locals::PrimitiveLocal],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<PreparedScalarCallee, LoweringError> {
        match self {
            Self::Operations(_) => unsupported(
                "scalar operation bodies require shared Unit assembly before scalar preparation",
            ),
            Self::Graph(graph) => {
                let prepared =
                    crate::scalar_graph::scalar_graph_lowering::prepare_scalar_graph_in_namespace(
                        checked,
                        qualifications,
                        graph,
                        contract_mode,
                        structural_parameters,
                        primitive_locals,
                        structural_types,
                        next_place,
                    )?;
                if contract_mode != ScalarContractMode::RootDirectResult
                    && (!prepared.identity_reshuffles.structural_places.is_empty()
                        || !prepared.identity_reshuffles.entry_claims.is_empty()
                        || !prepared.identity_reshuffles.reshuffles.is_empty()
                        || !prepared.partition_compositions.structural_places.is_empty()
                        || !prepared.partition_compositions.compositions.is_empty())
                {
                    return unsupported(
                        "embedded scalar call structural/content effects require a dedicated terminal slice",
                    );
                }
                Ok(PreparedScalarCallee::Graph(prepared))
            }
        }
    }
}

pub(crate) enum PreparedScalarCallee {
    Graph(PreparedScalarMachine),
}

impl PreparedScalarCallee {
    pub(crate) fn source_machine(&self) -> symbols::SymbolHandle {
        match self {
            Self::Graph(graph) => graph.source_machine,
        }
    }

    pub(crate) fn requirement_count(&self) -> usize {
        match self {
            Self::Graph(graph) => graph.requirement_count(),
        }
    }
}
