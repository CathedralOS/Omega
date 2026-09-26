//! The two result lanes of dynamic lowering and the custody they share.
//!
//! A scalar and a Unit call plan carry the same checked custody apart from
//! their result. [`DynamicCallView`] borrows that custody from either plan. A
//! [`DynamicCall`] lane supplies only what its plan adds: the result it binds
//! and the selected body's agreement with it, the scalar lane's caller store
//! and Unit continuation, its forwarded helper bodies, and how its single-call
//! lowering retains source machines. Validation, the single-call lowering,
//! the join and the forwarded helper chain are each written once over it.

use crate::unit::dynamic_composed_unit::LoweredDynamicDispatch;
use crate::unit::dynamic_composed_unit::applications::terminal_callable_result;
use crate::unit::dynamic_composed_unit::forwarded_helpers::{
    ForwardedHelperBody, ForwardedHelperCompletion,
};
use crate::unit::{LoweredPsi, LoweringError, MachineId, terminal_scalar_type, unsupported};
use checked_trees::{
    CheckedDynamicDescriptorTransferPlan, CheckedDynamicRealizationBodyPlan,
    CheckedDynamicRealizationCallablePlan, CheckedDynamicScalarCallOrigin,
    CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan, CheckedDynamicUnitCallOrigin,
    CheckedDynamicUnitCallPlan, CheckedDynamicUnitContinuationPlan, CheckedStructuralAccess,
    CheckedStructuralScalarFieldStorePlan, CheckedUnitCallCoordinate,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralPathSegment,
    DynamicConformanceBindingFact, MachineContractCommitment,
};
use language_semantics::{Multiplicity, ServiceReachSummary};
use semantic_vocabulary::ScalarType;
use symbols::SymbolHandle;
use terminal_psi::ClosedConformanceCallableResult;

#[derive(Clone)]
pub(crate) struct LoweredDynamicRealization {
    pub(crate) source_machine: symbols::SymbolHandle,
    pub(crate) source_state: symbols::SymbolHandle,
    /// The checked plan's identity for this callable: the bare normalized
    /// overload identity `CheckedDynamic*CallPlan::realization_identity` and
    /// the callable roster retain. For a finite-family tuple instance this is
    /// the bare instance identity, not the commitment-wrapped Terminal one.
    pub(crate) checked_identity: String,
    /// The Terminal-side callable identity
    /// (`checked_evidence_machine_identity`): the bare identity for a
    /// nongeneric realization, or the specialization-application-wrapped
    /// identity for a tuple's instance. Registry entries, row references and
    /// dispatch rows all name this form.
    pub(crate) callable_identity: String,
    pub(crate) machine: semantic_vocabulary::MachineId,
    pub(crate) result: ClosedConformanceCallableResult,
}

/// The identities one forwarded helper machine owns.
#[derive(Clone, Copy)]
pub(crate) struct ForwardedHelperIds {
    pub(crate) machine: semantic_vocabulary::MachineId,
    pub(crate) block: semantic_vocabulary::BlockId,
    pub(crate) operation: semantic_vocabulary::OperationId,
    pub(crate) edge: semantic_vocabulary::EdgeId,
    /// The call result and return value a scalar helper binds; a Unit
    /// helper's call and return bind none.
    pub(crate) scalar_values: Option<ForwardedHelperValues>,
}

#[derive(Clone, Copy)]
pub(crate) struct ForwardedHelperValues {
    /// The result of the helper's own call.
    pub(crate) call: semantic_vocabulary::ValueId,
    /// The value the helper returns.
    pub(crate) result: semantic_vocabulary::ValueId,
}

#[derive(Clone, Copy)]
pub(crate) enum DynamicLoweringLane<'a> {
    Direct,
    Rebound(&'a CheckedDynamicSelectionPlan),
    Stored(&'a checked_trees::CheckedDynamicStoredDescriptorPlan),
}

/// The final helper of a forwarded call: its machine and state, the
/// coordinate of its dispatching call and the descriptor parameter it calls
/// through.
#[derive(Clone, Copy)]
pub(crate) struct ForwardedOrigin {
    pub(crate) machine: SymbolHandle,
    pub(crate) state: SymbolHandle,
    pub(crate) coordinate: CheckedUnitCallCoordinate,
    pub(crate) parameter: SymbolHandle,
}

/// One call plan's checked custody, independent of its result.
pub(crate) struct DynamicCallView<'a> {
    /// The final forwarded helper; `None` for a local call.
    pub(crate) forwarded: Option<ForwardedOrigin>,
    pub(crate) forwarding_transfers: &'a [CheckedDynamicDescriptorTransferPlan],
    pub(crate) caller_machine: SymbolHandle,
    pub(crate) caller_state: SymbolHandle,
    pub(crate) caller_attachment_type_identity: &'a str,
    pub(crate) caller_multiplicity: Multiplicity,
    pub(crate) caller_parameter_access: CheckedStructuralAccess,
    pub(crate) caller_contract_report_fingerprint: u64,
    pub(crate) caller_contract_commitment: MachineContractCommitment,
    pub(crate) caller_service_reach: ServiceReachSummary,
    pub(crate) coordinate: CheckedUnitCallCoordinate,
    pub(crate) receiver_binding: SymbolHandle,
    pub(crate) selection: &'a DynamicConformanceBindingFact,
    pub(crate) source_parameter_position: u32,
    pub(crate) source_access: CheckedStructuralAccess,
    pub(crate) source_field: SymbolHandle,
    pub(crate) source_path: &'a [CheckedUnitStructuralPathSegment],
    pub(crate) source_type_identity: &'a str,
    pub(crate) source_multiplicity: Multiplicity,
    pub(crate) target_trait: SymbolHandle,
    pub(crate) selected_conformance: SymbolHandle,
    pub(crate) declaring_trait: SymbolHandle,
    pub(crate) requirement: SymbolHandle,
    pub(crate) requirement_identity: &'a str,
    pub(crate) realization_machine: SymbolHandle,
    pub(crate) realization_state: SymbolHandle,
    pub(crate) realization_identity: &'a str,
    pub(crate) family_tuple: &'a [String],
    pub(crate) realization_callables: &'a [CheckedDynamicRealizationCallablePlan],
    pub(crate) realization_contract_report_fingerprint: u64,
    pub(crate) realization_contract_commitment: MachineContractCommitment,
    pub(crate) checked_call_service_reach: ServiceReachSummary,
}

/// Both plans name their shared custody identically; only the origin enum
/// differs, so each lane converts its origin and borrows the rest here.
macro_rules! dynamic_call_view {
    ($plan:expr, $forwarded:expr) => {{
        let plan = $plan;
        DynamicCallView {
            forwarded: $forwarded,
            forwarding_transfers: &plan.forwarding_transfers,
            caller_machine: plan.caller_machine,
            caller_state: plan.caller_state,
            caller_attachment_type_identity: &plan.caller_attachment_type_identity,
            caller_multiplicity: plan.caller_multiplicity,
            caller_parameter_access: plan.caller_parameter_access,
            caller_contract_report_fingerprint: plan.caller_contract_report_fingerprint,
            caller_contract_commitment: plan.caller_contract_commitment,
            caller_service_reach: plan.caller_service_reach,
            coordinate: plan.coordinate,
            receiver_binding: plan.receiver_binding,
            selection: &plan.selection,
            source_parameter_position: plan.source_parameter_position,
            source_access: plan.source_access,
            source_field: plan.source_field,
            source_path: &plan.source_path,
            source_type_identity: &plan.source_type_identity,
            source_multiplicity: plan.source_multiplicity,
            target_trait: plan.target_trait,
            selected_conformance: plan.selected_conformance,
            declaring_trait: plan.declaring_trait,
            requirement: plan.requirement,
            requirement_identity: &plan.requirement_identity,
            realization_machine: plan.realization_machine,
            realization_state: plan.realization_state,
            realization_identity: &plan.realization_identity,
            family_tuple: &plan.family_tuple,
            realization_callables: &plan.realization_callables,
            realization_contract_report_fingerprint: plan.realization_contract_report_fingerprint,
            realization_contract_commitment: plan.realization_contract_commitment,
            checked_call_service_reach: plan.checked_call_service_reach,
        }
    }};
}

/// What one call plan decides beyond the custody every dynamic call shares.
/// Lowering names a result shape only through [`Self::result`].
pub(crate) trait DynamicCall {
    /// The custody both lanes' call plans share.
    fn view(&self) -> DynamicCallView<'_>;

    /// The scalar result the call binds, or `None` for a Unit call.
    fn result(&self) -> Option<&CheckedUnitScalarResultBindingPlan>;

    /// Agreement of the selected realization's checked body with this call.
    fn validate_selected_body(
        &self,
        body: &CheckedDynamicRealizationBodyPlan,
    ) -> Result<(), LoweringError>;

    /// The caller-side field store that precedes the selection, if retained.
    fn caller_store(&self) -> Option<&CheckedStructuralScalarFieldStorePlan>;

    /// The checked Unit control the call's result immediately selects.
    fn unit_continuation(&self) -> Option<&CheckedDynamicUnitContinuationPlan>;

    /// How many forwarded helper bodies the plan retains.
    fn helper_body_count(&self) -> usize;

    /// Result agreement between two join branches beyond the shared caller ABI.
    fn results_match(&self, other: &Self) -> bool;

    /// Agreement between two join branches' forwarded helper bodies.
    fn helper_bodies_match(&self, other: &Self) -> bool;

    /// The retained body of the forwarded helper at `index`, outermost first.
    fn helper_body(&self, index: usize) -> Option<ForwardedHelperBody<'_>>;

    /// How a single-call lowering retains the source machines it closed over.
    fn retain_sources(
        &self,
        terminal: LoweredPsi,
        realizations: &[LoweredDynamicRealization],
        helpers: &[ForwardedHelperIds],
    ) -> Result<LoweredDynamicDispatch, LoweringError>;

    /// The call's scalar result type, or `None` for a Unit call.
    fn result_type(&self) -> Result<Option<ScalarType>, LoweringError> {
        self.result()
            .map(|result| terminal_scalar_type(result.primitive_type))
            .transpose()
    }

    /// The closed conformance result the selected realization must expose.
    fn callable_result(&self) -> Result<ClosedConformanceCallableResult, LoweringError> {
        self.result()
            .map_or(Ok(ClosedConformanceCallableResult::Unit), |result| {
                terminal_callable_result(result.primitive_type)
            })
    }
}

impl DynamicCall for CheckedDynamicScalarCallPlan {
    fn view(&self) -> DynamicCallView<'_> {
        dynamic_call_view!(
            self,
            match self.origin {
                CheckedDynamicScalarCallOrigin::Local => None,
                CheckedDynamicScalarCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } => Some(ForwardedOrigin {
                    machine,
                    state,
                    coordinate,
                    parameter,
                }),
            }
        )
    }

    fn result(&self) -> Option<&CheckedUnitScalarResultBindingPlan> {
        Some(&self.result)
    }

    /// The selected body returns this call's result type, and the plan
    /// retains exactly its return expression and field stores.
    fn validate_selected_body(
        &self,
        body: &CheckedDynamicRealizationBodyPlan,
    ) -> Result<(), LoweringError> {
        let CheckedDynamicRealizationBodyPlan::Scalar {
            result_type,
            return_expression,
            structural_scalar_field_stores,
        } = body
        else {
            return unsupported("direct dynamic scalar call selected a Unit body");
        };
        if *result_type != self.result.primitive_type
            || *return_expression != self.realization_return_expression
            || *structural_scalar_field_stores != self.realization_structural_scalar_field_stores
        {
            return unsupported("direct dynamic selected body drifted from checked custody");
        }
        Ok(())
    }

    fn caller_store(&self) -> Option<&CheckedStructuralScalarFieldStorePlan> {
        self.caller_structural_scalar_field_store.as_ref()
    }

    fn unit_continuation(&self) -> Option<&CheckedDynamicUnitContinuationPlan> {
        self.unit_continuation.as_ref()
    }

    fn helper_body_count(&self) -> usize {
        self.forwarding_helpers.len()
    }

    /// The branch results bind one scalar type, and neither branch retains a
    /// caller store or Unit continuation the join would not lower.
    fn results_match(&self, other: &Self) -> bool {
        self.result.primitive_type == other.result.primitive_type
            && [self, other].into_iter().all(|plan| {
                plan.caller_structural_scalar_field_store.is_none()
                    && plan.unit_continuation.is_none()
            })
    }

    fn helper_bodies_match(&self, other: &Self) -> bool {
        self.forwarding_helpers == other.forwarding_helpers
    }

    /// A scalar helper's call binds this call's result type among its
    /// locals, and its checked scalar control returns.
    fn helper_body(&self, index: usize) -> Option<ForwardedHelperBody<'_>> {
        let body = self.forwarding_helpers.get(index)?;
        Some(ForwardedHelperBody {
            machine: body.machine,
            state: body.state,
            call_statement_index: body.call_result.statement_index,
            scalar_locals: &body.scalar_locals,
            completion: ForwardedHelperCompletion::Scalar {
                call_result: &body.call_result,
                control: &body.scalar_control,
            },
        })
    }

    fn retain_sources(
        &self,
        terminal: LoweredPsi,
        realizations: &[LoweredDynamicRealization],
        helpers: &[ForwardedHelperIds],
    ) -> Result<LoweredDynamicDispatch, LoweringError> {
        retain_dynamic_source_owners(terminal, &self.view(), realizations, helpers, Vec::new())
            .map(LoweredDynamicDispatch::SourceMapped)
    }
}

impl DynamicCall for CheckedDynamicUnitCallPlan {
    fn view(&self) -> DynamicCallView<'_> {
        dynamic_call_view!(
            self,
            match self.origin {
                CheckedDynamicUnitCallOrigin::Local => None,
                CheckedDynamicUnitCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } => Some(ForwardedOrigin {
                    machine,
                    state,
                    coordinate,
                    parameter,
                }),
            }
        )
    }

    fn result(&self) -> Option<&CheckedUnitScalarResultBindingPlan> {
        None
    }

    fn validate_selected_body(
        &self,
        body: &CheckedDynamicRealizationBodyPlan,
    ) -> Result<(), LoweringError> {
        if !matches!(body, CheckedDynamicRealizationBodyPlan::Unit) {
            return unsupported("dynamic Unit call selected a scalar body");
        }
        Ok(())
    }

    fn caller_store(&self) -> Option<&CheckedStructuralScalarFieldStorePlan> {
        None
    }

    fn unit_continuation(&self) -> Option<&CheckedDynamicUnitContinuationPlan> {
        None
    }

    fn helper_body_count(&self) -> usize {
        self.forwarding_helpers.len()
    }

    /// A Unit call binds no result.
    fn results_match(&self, _other: &Self) -> bool {
        true
    }

    fn helper_bodies_match(&self, other: &Self) -> bool {
        self.forwarding_helpers == other.forwarding_helpers
    }

    /// A Unit helper's call binds nothing and its body returns Unit.
    fn helper_body(&self, index: usize) -> Option<ForwardedHelperBody<'_>> {
        let body = self.forwarding_helpers.get(index)?;
        Some(ForwardedHelperBody {
            machine: body.machine,
            state: body.state,
            call_statement_index: body.call_statement_index,
            scalar_locals: &body.scalar_locals,
            completion: ForwardedHelperCompletion::Unit,
        })
    }

    /// A Unit call's lowering is mapped through its entry: the caller and
    /// the realization it selects.
    fn retain_sources(
        &self,
        terminal: LoweredPsi,
        _realizations: &[LoweredDynamicRealization],
        _helpers: &[ForwardedHelperIds],
    ) -> Result<LoweredDynamicDispatch, LoweringError> {
        Ok(LoweredDynamicDispatch::EntryOnly {
            terminal,
            source_machines: vec![self.caller_machine, self.realization_machine],
        })
    }
}

/// The exact catalog of source owners a scalar lowering retains: the caller
/// (unless `sources` already names it), each realization and each forwarded
/// helper's source machine.
pub(crate) fn retain_dynamic_source_owners(
    terminal: LoweredPsi,
    plan: &DynamicCallView<'_>,
    realizations: &[LoweredDynamicRealization],
    helpers: &[ForwardedHelperIds],
    mut sources: Vec<(symbols::SymbolHandle, MachineId)>,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    if !sources
        .iter()
        .any(|(source, _)| *source == plan.caller_machine)
    {
        sources.push((plan.caller_machine, terminal.semantic_module.entry));
    }
    sources.extend(
        realizations
            .iter()
            .map(|realization| (realization.source_machine, realization.machine)),
    );
    for (index, helper) in helpers.iter().enumerate() {
        let Some(origin) = plan.forwarded else {
            return unsupported("dynamic helper has no forwarded source owner");
        };
        let source = plan
            .forwarding_transfers
            .get(index)
            .map_or(origin.machine, |transfer| transfer.caller_machine);
        sources.push((source, helper.machine));
    }
    crate::producer_result::SourceMappedLowered::new(terminal, sources)
}
