/*
Composed state-graph lowering (`composed_control`, `state_graph`): live-state discovery, callable
admission, state and successor-edge emission, case dispatch, and captured loan tracking.
*/

pub(crate) use crate::unit::*;

pub(crate) mod attached_unit {
    pub(crate) use crate::unit::attached_unit::*;
    pub(crate) mod composed_control {
        use super::{CheckedTrees, LoweringError};
        pub(crate) mod admission {
            use super::super::operation_frame::OperationFrame;
            use super::super::{
                CheckedBoundaryMachinePlan, CheckedUnitEffectOperationPlan, unsupported,
            };
            use super::{CheckedTrees, LoweringError};
            use crate::unit::attached_unit::admission::retain_call_boundaries;
            use crate::unit::attached_unit::bodies::UnitPlans;
            use crate::unit::attached_unit::call_closure::BoundaryIdentity;

            pub(crate) fn admit_calls<'a>(
                checked: &'a CheckedTrees,
                call_states: &[&checked_trees::CheckedComposedUnitControlStatePlan],
            ) -> Result<Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>, LoweringError>
            {
                let plans = UnitPlans::published(&checked.facts.flow);
                let mut boundaries = Vec::new();
                for state in call_states {
                    for operation in state.operation_dependencies() {
                        retain_call_boundaries(
                            checked,
                            plans,
                            std::iter::once(operation),
                            &mut boundaries,
                        )?;
                        match operation {
                            _ if OperationFrame::consumes_prepared_operands(operation) => {}
                            CheckedUnitEffectOperationPlan::EstablishLocal(
                                checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
                                | checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine {
                                    ..
                                },
                            ) => {}
                            _ if OperationFrame::lowers(operation) => {}
                            _ => {
                                return unsupported(
                                    "composed Unit call state contains a non-call operation",
                                );
                            }
                        }
                    }
                }
                boundaries.sort_by_key(|(_, identity)| identity.order_key());
                if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
                    return unsupported(
                        "composed Unit boundaries have duplicate canonical identities",
                    );
                }
                Ok(boundaries)
            }

            pub(crate) fn validate_contract(
                checked: &CheckedTrees,
                plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
            ) -> Result<(), LoweringError> {
                if checked
                    .facts
                    .contract_plans
                    .for_machine(plan.machine)
                    .is_none()
                {
                    return unsupported(
                        "composed Unit control is missing its canonical checked contract",
                    );
                }
                Ok(())
            }
        }
        pub(crate) mod callable {
            use super::super::super::{
                MachineId, ServiceId, ServiceReachId, StructuralParameterDeclaration,
                StructuralTypeDeclaration,
            };
            use super::super::operation_frame::BoundaryParameters;
            use super::super::{
                CheckedBoundaryMachinePlan, StructuralTypeId, TerminalMachine, ValueDeclaration,
                lookup_machine_id,
            };
            use super::{CheckedTrees, LoweringError, catalogs, scalar_calls, state_graph};
            use crate::shared::terminal_identities::StructuralDomainBinding;
            use crate::unit::attached_unit::call_closure::BoundaryIdentity;
            use crate::unit::attached_unit::signatures::{self, MachineSignature};
            use std::borrow::Cow;

            pub(crate) struct CallableBody<'a> {
                plan: checked_trees::CheckedComposedUnitControlMachinePlan,
                graph: state_graph::AdmittedGraph<'a>,
            }

            pub(crate) fn admit<'a>(
                checked: &'a CheckedTrees,
                source: &checked_trees::CheckedComposedUnitControlMachinePlan,
            ) -> Result<CallableBody<'a>, LoweringError> {
                let plan = state_graph::prepare(source, &checked.facts.values.structural_values)?;
                let graph = state_graph::admit(checked, &plan)?;
                Ok(CallableBody { plan, graph })
            }

            impl<'a> CallableBody<'a> {
                pub(crate) fn boundaries(
                    &self,
                ) -> &[(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)] {
                    &self.graph.boundaries
                }
            }

            pub(crate) struct SharedCatalog<'a> {
                pub scalar_applications:
                    &'a std::sync::Arc<super::super::scalar_applications::SourceScalarApplications>,
                pub structural_types: &'a [StructuralTypeDeclaration],
                pub type_ids:
                    &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
                pub domain_ids: &'a [StructuralDomainBinding],
                pub domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
                pub service_ids: &'a [(ServiceReachId, ServiceId)],
                pub boundary_parameters: &'a [BoundaryParameters],
                pub machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
                pub signatures: &'a [MachineSignature],
                pub scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
                pub closure: &'a [symbols::SymbolHandle],
                pub prepared_scalar_qualifications:
                    &'a crate::expression_preparation::qualifications::PreparedScalarQualifications,
                pub dynamic_dispatch:
                    &'a std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
            }

            pub(crate) struct EmissionCounters<'a> {
                pub place: &'a mut u64,
                pub value: &'a mut u64,
                pub block: &'a mut u64,
                pub operation: &'a mut u64,
                pub edge: &'a mut u64,
                pub call_obligation: &'a mut u64,
            }

            pub(crate) fn emit(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
                admitted: CallableBody<'_>,
                parameters: Vec<StructuralParameterDeclaration>,
                scalar_parameters: Vec<ValueDeclaration>,
                shared: SharedCatalog<'_>,
                counters: EmissionCounters<'_>,
            ) -> Result<
                (
                    TerminalMachine,
                    super::ComposedOccurrences,
                    Vec<terminal_psi::ScalarBlockInvariant>,
                    Vec<terminal_psi::ScalarQualificationCoercion>,
                ),
                LoweringError,
            > {
                let identity = lookup_machine_id(shared.machine_ids, plan.machine)?;
                let CallableBody {
                    plan,
                    graph: admitted,
                } = admitted;
                let plan = &plan;
                let mut catalogs = catalogs::ComposedCatalogs {
                    scalar_applications: shared.scalar_applications,
                    cleanup_control: &checked.facts.flow.terminal_unit_effects.cleanup_control,
                    temporary_places: Vec::new(),
                    result_places: Vec::new(),
                    structural_types: Cow::Borrowed(shared.structural_types),
                    type_ids: Cow::Borrowed(shared.type_ids),
                    domain_ids: Cow::Borrowed(shared.domain_ids),
                    domain_declarations: shared.domain_declarations,
                    boundary_parameters: Cow::Borrowed(shared.boundary_parameters),
                    signatures: Cow::Borrowed(shared.signatures),
                    closure: shared.closure,
                    prepared_scalar_qualifications: Some(shared.prepared_scalar_qualifications),
                    dynamic_dispatch: Some(std::sync::Arc::clone(shared.dynamic_dispatch)),
                    service_ids: Cow::Borrowed(shared.service_ids),
                    scalar_calls: scalar_calls::ComposedScalarCalls::from_shared(
                        identity,
                        shared.machine_ids.to_vec(),
                        shared.scalar_requirement_counts.to_vec(),
                        *counters.call_obligation,
                        shared.prepared_scalar_qualifications.catalog(),
                    )
                    .with_callable_namespace(
                        shared.service_ids,
                        shared.boundary_parameters,
                        shared.domain_ids,
                        shared.domain_declarations,
                        shared
                            .signatures
                            .iter()
                            .filter_map(|signature| {
                                signature
                                    .cleanup_receiver
                                    .map(|receiver| (signature.source, receiver))
                            })
                            .collect(),
                        shared.signatures,
                        shared.structural_types,
                    ),
                    root_crash_routes: crate::unit::effective_crash_routes(checked, plan.machine)?,
                    next_place: *counters.place,
                    next_value: *counters.value,
                    next_block: *counters.block,
                    next_operation: *counters.operation,
                    next_edge: *counters.edge,
                };
                let entry_erased_formals = signatures::find(shared.signatures, plan.machine)?
                    .erased_scalar_parameters
                    .clone();
                let (mut machine, occurrences, scalar_block_invariants, qualification_coercions) =
                    state_graph::emit(
                        identities,
                        checked,
                        plan,
                        admitted,
                        identity,
                        parameters,
                        scalar_parameters,
                        entry_erased_formals,
                        &mut catalogs,
                    )?;
                let signature = signatures::find(shared.signatures, plan.machine)?;
                machine.contract.requires = signature.requires.clone();
                let view_roster: Vec<(u32, StructuralParameterDeclaration)> = plan
                    .states
                    .first()
                    .into_iter()
                    .flat_map(|entry| entry.structural_parameters.iter())
                    .zip(machine.structural_parameters.iter())
                    .map(|(source, parameter)| (source.position, parameter.clone()))
                    .collect();
                let entry = plan
                    .states
                    .first()
                    .ok_or(LoweringError::Unsupported("Unit graph has no entry state"))?;
                machine.contract.ensures = super::super::normal_guarantees::lower(
                    identities,
                    checked,
                    entry.state,
                    &machine.parameters,
                    &machine.result,
                    &signature.erased_scalar_parameters,
                    &crate::scalar_lowering::contracts::ContractViewNamespace {
                        scalar_applications: shared.scalar_applications,
                        constant_values: &checked.typed.constant_recipe_closure.values,
                        parameters: &view_roster,
                        result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                        structural_types: &catalogs.structural_types,
                    },
                    &mut catalogs.scalar_calls.next_call_obligation,
                )?;
                *counters.place = catalogs.next_place;
                *counters.value = catalogs.next_value;
                *counters.block = catalogs.next_block;
                *counters.operation = catalogs.next_operation;
                *counters.edge = catalogs.next_edge;
                *counters.call_obligation = catalogs.scalar_calls.next_call_obligation;
                Ok((
                    machine,
                    occurrences,
                    scalar_block_invariants,
                    qualification_coercions,
                ))
            }
        }
        pub(crate) mod catalogs {
            use super::super::super::{ServiceId, ServiceReachId, StructuralTypeDeclaration};
            use super::super::{StructuralPlaceDeclaration, StructuralTypeId};
            use super::scalar_calls;
            use crate::shared::terminal_identities::StructuralDomainBinding;
            use crate::unit::attached_unit::operation_frame::BoundaryParameters;
            use crate::unit::attached_unit::signatures::MachineSignature;
            use std::borrow::Cow;

            pub(crate) struct ComposedCatalogs<'a> {
                pub(crate) scalar_applications:
                    &'a std::sync::Arc<super::super::scalar_applications::SourceScalarApplications>,
                pub(crate) cleanup_control: &'a checked_trees::CheckedCleanupControlPlans,
                pub(crate) structural_types: Cow<'a, [StructuralTypeDeclaration]>,
                pub(crate) type_ids:
                    Cow<'a, [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)]>,
                pub(crate) domain_ids: Cow<'a, [StructuralDomainBinding]>,
                pub(crate) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
                pub(crate) boundary_parameters: Cow<'a, [BoundaryParameters]>,
                pub(crate) signatures: Cow<'a, [MachineSignature]>,
                pub(crate) closure: &'a [symbols::SymbolHandle],
                pub(crate) prepared_scalar_qualifications: Option<
                    &'a crate::expression_preparation::qualifications::PreparedScalarQualifications,
                >,
                pub(crate) dynamic_dispatch:
                    Option<std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>>,
                pub(crate) service_ids: Cow<'a, [(ServiceReachId, ServiceId)]>,
                pub(crate) next_place: u64,
                pub(crate) temporary_places: Vec<StructuralPlaceDeclaration>,
                pub(crate) result_places: Vec<StructuralPlaceDeclaration>,
                pub(crate) scalar_calls: scalar_calls::ComposedScalarCalls<'a>,
                pub(crate) root_crash_routes: Vec<checked_trees::CrashRouteBucket>,
                pub(crate) next_value: u64,
                pub(crate) next_block: u64,
                pub(crate) next_operation: u64,
                pub(crate) next_edge: u64,
            }
        }
        pub(crate) mod emission {
            use super::super::super::StructuralParameterDeclaration;
            use super::super::argument_schedule::{self, Step};
            use super::super::bodies::UnitPlans;
            use super::super::operation_frame::{
                CallInputs, Callees, CallerCustody, OperationEmission, OperationFrame,
                StructuralResults, StructuralTypeRoster,
            };
            use super::super::primitive_locals::PrimitiveLocal;
            use super::super::{CheckedUnitEffectOperationPlan, ValueDeclaration, unsupported};
            use super::{CheckedTrees, LoweringError, catalogs, literal_arguments};
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::emission::operation_emission::calls::CallEmissionContext;
            use crate::unit::StructuralTypeId;
            use std::borrow::Cow;

            pub(crate) struct OperationSequence {
                windows: crate::emission::borrowed_window::BorrowedWindowLedger,
                primitive_locals: Vec<PrimitiveLocal>,
            }

            impl OperationSequence {
                pub(crate) fn new(
                    evaluation: &super::super::argument_evaluation::Evaluation,
                ) -> Self {
                    Self {
                        windows: Default::default(),
                        primitive_locals: evaluation.parameter_storage.clone(),
                    }
                }

                pub(crate) fn branch(&self) -> Self {
                    Self {
                        windows: self.windows.clone(),
                        primitive_locals: self.primitive_locals.clone(),
                    }
                }

                pub(crate) fn require_closed(&self) -> Result<(), LoweringError> {
                    self.windows.require_closed()
                }

                pub(crate) fn finish_state(
                    &self,
                    checked: &CheckedTrees,
                    state: &checked_trees::CheckedComposedUnitControlStatePlan,
                ) -> Result<(), LoweringError> {
                    match state
                        .terminator
                        .all_paths_crash(&state.operations, &checked.facts.flow.guarded_exits)
                    {
                        Some(true) => Ok(()),
                        Some(false) => self.require_closed(),
                        None => unsupported("composed abandonment roster is stale"),
                    }
                }
            }

            pub(crate) struct ComposedOperationCatalogs<'frame, 'catalogs> {
                pub(crate) structural_types:
                    &'frame mut Cow<'catalogs, [crate::unit::StructuralTypeDeclaration]>,
                type_ids:
                    &'frame [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
                service_ids: &'frame [(crate::unit::ServiceReachId, crate::unit::ServiceId)],
                signatures: &'frame [super::super::signatures::MachineSignature],
                boundary_parameters: &'frame [super::super::operation_frame::BoundaryParameters],
                domain_ids: &'frame [crate::shared::terminal_identities::StructuralDomainBinding],
                domain_declarations: &'frame [terminal_psi::StructuralDomainDeclaration],
                closure: &'frame [symbols::SymbolHandle],
                temporary_places: &'frame mut Vec<crate::unit::StructuralPlaceDeclaration>,
                result_places: &'frame mut Vec<crate::unit::StructuralPlaceDeclaration>,
                next_place: &'frame mut u64,
            }

            impl<'catalogs> catalogs::ComposedCatalogs<'catalogs> {
                pub(crate) fn operation_catalogs(
                    &mut self,
                ) -> (
                    ComposedOperationCatalogs<'_, 'catalogs>,
                    &mut super::scalar_calls::ComposedScalarCalls<'catalogs>,
                ) {
                    (
                        ComposedOperationCatalogs {
                            structural_types: &mut self.structural_types,
                            type_ids: &self.type_ids,
                            service_ids: &self.service_ids,
                            signatures: &self.signatures,
                            boundary_parameters: &self.boundary_parameters,
                            domain_ids: &self.domain_ids,
                            domain_declarations: self.domain_declarations,
                            closure: self.closure,
                            temporary_places: &mut self.temporary_places,
                            result_places: &mut self.result_places,
                            next_place: &mut self.next_place,
                        },
                        &mut self.scalar_calls,
                    )
                }
            }

            impl ComposedOperationCatalogs<'_, '_> {
                #[allow(clippy::too_many_arguments)]
                pub(crate) fn operation_frame<'frame, 'calls>(
                    &'frame mut self,
                    checked: &'frame CheckedTrees,
                    machine: symbols::SymbolHandle,
                    state: &'frame checked_trees::CheckedComposedUnitControlStatePlan,
                    parameters: &'frame [StructuralParameterDeclaration],
                    erased_parameters: &'frame [ValueDeclaration],
                    sequence: &'frame mut OperationSequence,
                    emission: OperationEmission<'frame, 'calls>,
                ) -> OperationFrame<'frame, 'calls> {
                    let scalar_result = match &state.terminator {
                        checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                            completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
                        } => Some(binding),
                        _ => None,
                    };
                    OperationFrame {
                        checked,
                        machine,
                        state: state.state,
                        scalar_result,
                        scalar_parameter_count: state.scalar_parameters.len(),
                        source_value_count: emission.source_value_count,
                        parameters: emission.evaluation.current_parameters(parameters),
                        structural_types: match &mut *self.structural_types {
                            Cow::Owned(types) => StructuralTypeRoster::Owned(types),
                            Cow::Borrowed(types) => StructuralTypeRoster::Published(types),
                        },
                        type_ids: self.type_ids,
                        service_ids: self.service_ids,
                        primitive_locals: &sequence.primitive_locals,
                        results: StructuralResults::StateGraph {
                            state,
                            places: self.result_places,
                        },
                        private_places: self.temporary_places,
                        windows: &mut sequence.windows,
                        evaluation: emission.evaluation,
                        values: emission.values,
                        next_place: self.next_place,
                        next_value: emission.next_value,
                        next_block: emission.next_block,
                        next_edge: emission.next_edge,
                        calls: emission.calls,
                        operations: emission.operations,
                        callees: Callees {
                            plans: UnitPlans::published(&checked.facts.flow),
                            signatures: self.signatures,
                            boundaries: self.boundary_parameters,
                            domain_ids: self.domain_ids,
                            domain_declarations: self.domain_declarations,
                            closure: self.closure,
                        },
                        caller: CallerCustody {
                            erased_scalar_parameters: erased_parameters,
                            erased_proof_parameters: &state.erased_proof_parameters,
                            local_places: &[],
                        },
                    }
                }
            }

            pub(crate) struct StateFrame<'f> {
                checked: &'f CheckedTrees,
                machine: symbols::SymbolHandle,
                state: &'f checked_trees::CheckedComposedUnitControlStatePlan,
                erased_parameters: &'f [ValueDeclaration],
            }

            impl<'f> StateFrame<'f> {
                pub(crate) fn new(
                    checked: &'f CheckedTrees,
                    machine: symbols::SymbolHandle,
                    state: &'f checked_trees::CheckedComposedUnitControlStatePlan,
                    erased_parameters: &'f [ValueDeclaration],
                ) -> Self {
                    Self {
                        checked,
                        machine,
                        state,
                        erased_parameters,
                    }
                }

                pub(crate) fn evaluate<T>(
                    &self,
                    catalogs: &mut catalogs::ComposedCatalogs<'_>,
                    parameters: &[StructuralParameterDeclaration],
                    sequence: &mut OperationSequence,
                    values: FrameValues<'_>,
                    evaluate: impl FnOnce(OperationFrame<'_, '_>) -> Result<T, LoweringError>,
                ) -> Result<T, LoweringError> {
                    let (mut operation_catalogs, scalar_calls) = catalogs.operation_catalogs();
                    with_call_namespace(scalar_calls, |calls| {
                        let frame = operation_catalogs.operation_frame(
                            self.checked,
                            self.machine,
                            self.state,
                            parameters,
                            self.erased_parameters,
                            sequence,
                            OperationEmission {
                                source_value_count: values.source_value_count,
                                evaluation: values.evaluation,
                                values: values.values,
                                next_value: values.next_value,
                                next_block: values.next_block,
                                next_edge: values.next_edge,
                                calls,
                                operations: values.operations,
                            },
                        );
                        evaluate(frame)
                    })
                }
            }

            pub(crate) struct FrameValues<'f> {
                pub(crate) source_value_count: usize,
                pub(crate) evaluation: &'f mut super::super::argument_evaluation::Evaluation,
                pub(crate) values: &'f mut Vec<ValueDeclaration>,
                pub(crate) next_value: &'f mut u64,
                pub(crate) next_block: &'f mut u64,
                pub(crate) next_edge: &'f mut u64,
                pub(crate) operations: &'f mut OperationBuffer,
            }

            pub(crate) fn with_call_namespace<T>(
                scalar_calls: &mut super::scalar_calls::ComposedScalarCalls<'_>,
                run: impl FnOnce(&mut CallEmissionContext<'_>) -> Result<T, LoweringError>,
            ) -> Result<T, LoweringError> {
                let mut calls = scalar_calls.emission_context();
                let result = run(&mut calls);
                scalar_calls.next_call_obligation = calls.next_obligation_identity;
                result
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn emit_call_operations(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: &checked_trees::CheckedComposedUnitControlStatePlan,
                planned_operations: &[CheckedUnitEffectOperationPlan],
                catalogs: &mut catalogs::ComposedCatalogs,
                parameters: &[StructuralParameterDeclaration],
                evaluation: &mut super::super::argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                erased_parameters: &[ValueDeclaration],
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                sequence: &mut OperationSequence,
            ) -> Result<(), LoweringError> {
                macro_rules! frame_values {
                    ($source_value_count:expr) => {
                        FrameValues {
                            source_value_count: $source_value_count,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        }
                    };
                }
                macro_rules! evaluate_frame {
                    ($source_value_count:expr, $body:expr $(,)?) => {
                        StateFrame::new(checked, machine, state, erased_parameters).evaluate(
                            catalogs,
                            parameters,
                            sequence,
                            frame_values!($source_value_count),
                            $body,
                        )
                    };
                }
                let schedule = argument_schedule::build_operations(
                    checked,
                    machine,
                    state.state,
                    planned_operations,
                )?;
                let mut schedule = schedule.iter().peekable();
                let mut retained_source_count = None;
                let mut staged_scalar_slots = vec![
                        super::super::argument_evaluation::ScalarArgumentCaptures::default();
                        planned_operations.len()
                    ];
                let mut staged_ranges = vec![Vec::new(); planned_operations.len()];
                let mut staged_selector_slots = vec![Vec::new(); planned_operations.len()];
                let mut staged_structural_arguments = vec![Vec::new(); planned_operations.len()];
                while let Some(step) = schedule.next() {
                    let (operation_index, staged) = match *step {
                        Step::CaptureBefore(statement) => {
                            evaluation.prepare_storage_before(
                                identities,
                                checked,
                                state.state,
                                statement,
                                &catalogs.structural_types,
                                catalogs.domain_declarations,
                                values,
                                &mut catalogs.next_place,
                                next_value,
                                operations,
                                &mut catalogs.temporary_places,
                            )?;
                            continue;
                        }
                        Step::Begin => {
                            if retained_source_count.replace(values.len()).is_some() {
                                return unsupported("composed argument group was already active");
                            }
                            continue;
                        }
                        Step::End => {
                            if retained_source_count.take().is_none() {
                                return unsupported("composed argument group was not active");
                            }
                            continue;
                        }
                        Step::Argument { operation, ordinal } => {
                            let source_value_count =
                                retained_source_count.ok_or(LoweringError::Unsupported(
                                    "composed scalar operand has no source prefix",
                                ))?;
                            let value = evaluate_frame!(source_value_count, |frame| {
                                let (mut environment, emission) = frame.split();
                                emission.evaluation.argument_at(
                                    identities,
                                    checked,
                                    machine,
                                    state.state,
                                    &planned_operations[operation],
                                    ordinal,
                                    source_value_count,
                                    emission.values,
                                    emission.next_value,
                                    emission.next_block,
                                    emission.next_edge,
                                    emission.operations,
                                    emission.calls,
                                    &mut environment,
                                )
                            })?;
                            staged_scalar_slots[operation].record(ordinal, values.len());
                            values.push(value);
                            continue;
                        }
                        Step::Selector {
                            operation,
                            argument,
                            segment,
                        } => {
                            let source_value_count =
                                retained_source_count.ok_or(LoweringError::Unsupported(
                                    "composed selector has no source prefix",
                                ))?;
                            let value = evaluate_frame!(source_value_count, |frame| {
                                let (mut environment, emission) = frame.split();
                                emission.evaluation.call_selector(
                                    identities,
                                    checked,
                                    machine,
                                    state.state,
                                    &planned_operations[operation],
                                    argument,
                                    segment,
                                    source_value_count,
                                    emission.values,
                                    emission.next_value,
                                    emission.next_block,
                                    emission.next_edge,
                                    emission.operations,
                                    emission.calls,
                                    &mut environment,
                                )
                            })?;
                            staged_selector_slots[operation].push(values.len());
                            values.push(value);
                            continue;
                        }
                        Step::Structural {
                            operation,
                            argument,
                            complete,
                        } => {
                            let source_value_count =
                                retained_source_count.ok_or(LoweringError::Unsupported(
                                    "composed structural actual has no source prefix",
                                ))?;
                            let arguments = planned_operations[operation].structural_operands();
                            let offset = arguments[..argument]
                                .iter()
                                .map(|source| source.call_selector_positions().count())
                                .sum::<usize>();
                            let selectors = staged_selector_slots[operation][offset..].to_vec();
                            if staged_structural_arguments[operation].is_empty() {
                                staged_structural_arguments[operation] =
                                    vec![None; arguments.len()];
                            }
                            let previous = staged_structural_arguments[operation][argument].take();
                            let captured = evaluate_frame!(source_value_count, |frame| {
                                frame.capture_argument(
                                    identities,
                                    &planned_operations[operation],
                                    argument,
                                    &selectors,
                                    previous,
                                    complete,
                                )
                            })?;
                            staged_structural_arguments[operation][argument] = Some(captured);
                            continue;
                        }
                        Step::Subslice { operation, ordinal } => {
                            let source_value_count =
                                retained_source_count.ok_or(LoweringError::Unsupported(
                                    "composed range operand has no source prefix",
                                ))?;
                            if staged_ranges[operation]
                                .iter()
                                .any(|(prior, _)| *prior == ordinal)
                            {
                                return unsupported(
                                    "composed range operand is evaluated more than once",
                                );
                            }
                            let place = literal_arguments::stage_range(
                                identities,
                                checked,
                                state,
                                &planned_operations[operation],
                                ordinal,
                                source_value_count,
                                catalogs,
                                evaluation,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                                machine,
                                parameters,
                                erased_parameters,
                                sequence,
                            )?;
                            staged_ranges[operation].push((ordinal, place));
                            continue;
                        }
                        Step::Call(index) | Step::Operation(index)
                            if retained_source_count.is_some() =>
                        {
                            (index, true)
                        }
                        Step::Ordinary(index) if retained_source_count.is_none() => (index, false),
                        _ => {
                            return unsupported(
                                "composed call schedule disagrees with its argument group",
                            );
                        }
                    };
                    let operation = &planned_operations[operation_index];
                    if matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ReleaseReference {
                            defer_to_exit: true,
                            ..
                        }
                    ) {
                        continue;
                    }
                    let source_value_count = retained_source_count.unwrap_or(values.len());
                    let staged_scalars = staged_scalar_slots[operation_index].values(values);
                    let staged_selectors = staged_selector_slots[operation_index]
                        .iter()
                        .map(|slot| {
                            values.get(*slot).copied().ok_or(LoweringError::Unsupported(
                                "composed selector lost its retained value",
                            ))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let establishes_local = matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
                        )
                    );
                    let call_operands = if establishes_local || OperationFrame::lowers(operation) {
                        None
                    } else if OperationFrame::consumes_prepared_operands(operation) {
                        Some(literal_arguments::evaluate(
                            identities,
                            checked,
                            machine,
                            state,
                            operation,
                            staged.then_some(literal_arguments::StagedArguments {
                                scalars: &staged_scalars,
                                ranges: &staged_ranges[operation_index],
                            }),
                            catalogs,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                            parameters,
                            erased_parameters,
                            sequence,
                        )?)
                    } else {
                        return unsupported("composed operation has no frame emission route");
                    };
                    if matches!(step, Step::Call(_)) && matches!(schedule.peek(), Some(Step::End)) {
                        values.truncate(source_value_count);
                    }
                    let mut established_local = None;
                    evaluate_frame!(source_value_count, |frame| {
                        match &call_operands {
                            None if establishes_local => {
                                let local =
                                    frame.establish_primitive_local(identities, operation)?;
                                established_local = Some(local);
                            }
                            None => frame.emit(identities, operation)?,
                            Some((arguments, byte_places)) => frame.emit_call(
                                identities,
                                operation,
                                CallInputs {
                                    evaluated_scalar_arguments: arguments.as_deref(),
                                    byte_places,
                                    evaluated_selectors: staged
                                        .then_some(staged_selectors.as_slice()),
                                    completed_structural_arguments: (!staged_structural_arguments
                                        [operation_index]
                                        .is_empty())
                                    .then_some(
                                        staged_structural_arguments[operation_index].as_slice(),
                                    ),
                                },
                            )?,
                        }
                        Ok(())
                    })?;
                    if let Some(local) = established_local {
                        catalogs.temporary_places.push(local.declaration);
                        sequence.primitive_locals.push(local);
                    }
                    if operations.invocation_completion.is_some() {
                        break;
                    }
                }
                sequence.finish_state(checked, state)
            }
        }
        pub(crate) mod literal_arguments {
            use super::super::{
                CheckedUnitEffectOperationPlan, PlaceId, ValueDeclaration, lookup_type_id,
                unsupported,
            };
            use super::{CheckedTrees, LoweringError, catalogs};
            use crate::emission::operation_emission::buffer::OperationBuffer;
            use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
            use crate::unit::attached_unit::byte_subslices;
            use crate::unit::attached_unit::composed_control::emission::{FrameValues, StateFrame};

            pub(crate) struct StagedArguments<'a> {
                pub scalars: &'a [ValueDeclaration],
                pub ranges: &'a [(usize, PlaceId)],
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn evaluate(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                machine: symbols::SymbolHandle,
                state: &checked_trees::CheckedComposedUnitControlStatePlan,
                operation: &CheckedUnitEffectOperationPlan,
                staged: Option<StagedArguments<'_>>,
                catalogs: &mut catalogs::ComposedCatalogs,
                evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                parameters: &[crate::unit::StructuralParameterDeclaration],
                erased_parameters: &[ValueDeclaration],
                sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
            ) -> Result<(Option<Vec<ValueDeclaration>>, Vec<PlaceId>), LoweringError> {
                let state_plan = state;
                let state = state.state;
                let arguments = byte_subslices::arguments(operation);
                if !arguments.iter().any(|argument| {
                    argument.byte_sequence_literal().is_some()
                        || byte_subslices::range(argument).is_some()
                }) {
                    if let Some(staged) = staged {
                        return Ok((Some(staged.scalars.to_vec()), Vec::new()));
                    }
                    let arguments =
                        StateFrame::new(checked, machine, state_plan, erased_parameters).evaluate(
                            catalogs,
                            parameters,
                            sequence,
                            FrameValues {
                                source_value_count: values.len(),
                                evaluation,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            },
                            |frame| {
                                let (mut environment, emission) = frame.split();
                                emission.evaluation.arguments(
                                    identities,
                                    checked,
                                    machine,
                                    state,
                                    operation,
                                    emission.values,
                                    emission.next_value,
                                    emission.next_block,
                                    emission.next_edge,
                                    emission.operations,
                                    emission.calls,
                                    &mut environment,
                                )
                            },
                        )?;
                    return Ok((arguments, Vec::new()));
                }
                let Some(call) = operation.call() else {
                    return unsupported("literal call escaped the composed Unit operation family");
                };
                let target_parameters =
                    super::super::call_parameters::structural_parameters(checked, call)?;
                let (scalar_count, coordinate) = (call.scalar_arguments.len(), call.coordinate);
                let parameter_qualifications = target_parameters
                    .iter()
                    .map(|parameter| parameter.qualifications.clone())
                    .collect::<Vec<_>>();
                let authored = crate::shared::call_sites::locate(checked, state, coordinate)?;
                let signature = crate::shared::declarations::target_signature(
                    checked,
                    machine,
                    authored.source_target,
                )?;
                let positions =
                    super::super::call_parameters::structural_positions(checked, call, &signature)?
                        .into_iter()
                        .map(|position| position as usize)
                        .collect::<Vec<_>>();
                let scalar_positions =
                    super::super::call_parameters::scalar_positions(checked, call, &signature)?;
                if positions.len() != arguments.len()
                    || scalar_positions.len() != scalar_count
                    || positions.iter().any(|position| {
                        *position >= signature.parameters.len()
                            || scalar_positions.contains(position)
                    })
                    || positions.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return unsupported("literal call lost its authored parameter partition");
                }
                let source_value_count = values.len();
                let mut scalar_slots = Vec::new();
                let mut byte_places = Vec::new();
                let mut structural_ordinal = 0;
                for argument_position in 0..signature.parameters.len() {
                    if positions.get(structural_ordinal) == Some(&argument_position) {
                        let argument = &arguments[structural_ordinal];
                        if argument.byte_sequence_literal().is_some() {
                            byte_places.push(establish(
                                identities,
                                argument,
                                parameter_qualifications.get(structural_ordinal).ok_or(
                                    LoweringError::Unsupported(
                                        "literal call target parameter is absent",
                                    ),
                                )?,
                                catalogs,
                                next_value,
                                operations,
                            )?);
                        } else if byte_subslices::range(argument).is_some() {
                            let place = if let Some(staged) = &staged {
                                staged
                                    .ranges
                                    .iter()
                                    .find_map(|(ordinal, place)| {
                                        (*ordinal == structural_ordinal).then_some(*place)
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "scheduled call lost its staged range operand",
                                    ))?
                            } else {
                                stage_range(
                                    identities,
                                    checked,
                                    state_plan,
                                    operation,
                                    structural_ordinal,
                                    source_value_count,
                                    catalogs,
                                    evaluation,
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                    machine,
                                    parameters,
                                    erased_parameters,
                                    sequence,
                                )?
                            };
                            byte_places.push(place);
                        }
                        structural_ordinal += 1;
                    } else if staged.is_none()
                        && scalar_positions.get(scalar_slots.len()) == Some(&argument_position)
                    {
                        let value =
                            StateFrame::new(checked, machine, state_plan, erased_parameters)
                                .evaluate(
                                    catalogs,
                                    parameters,
                                    sequence,
                                    FrameValues {
                                        source_value_count: values.len(),
                                        evaluation,
                                        values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                    },
                                    |frame| {
                                        let (mut environment, emission) = frame.split();
                                        emission.evaluation.argument_at(
                                            identities,
                                            checked,
                                            machine,
                                            state,
                                            operation,
                                            scalar_slots.len(),
                                            source_value_count,
                                            emission.values,
                                            emission.next_value,
                                            emission.next_block,
                                            emission.next_edge,
                                            emission.operations,
                                            emission.calls,
                                            &mut environment,
                                        )
                                    },
                                )?;
                        scalar_slots.push(values.len());
                        values.push(value);
                    }
                }
                if let Some(staged) = staged {
                    if staged.scalars.len() != scalar_count {
                        return unsupported("scheduled call lost its completed scalar operands");
                    }
                    return Ok((Some(staged.scalars.to_vec()), byte_places));
                }
                let scalars = scalar_slots
                    .into_iter()
                    .map(|slot| {
                        values.get(slot).copied().ok_or(LoweringError::Unsupported(
                            "literal call lost an evaluated scalar operand",
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                values.truncate(source_value_count);
                Ok((Some(scalars), byte_places))
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn stage_range(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                checked: &CheckedTrees,
                state: &checked_trees::CheckedComposedUnitControlStatePlan,
                operation: &CheckedUnitEffectOperationPlan,
                ordinal: usize,
                source_value_count: usize,
                catalogs: &mut catalogs::ComposedCatalogs,
                evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                values: &mut Vec<ValueDeclaration>,
                next_value: &mut u64,
                next_block: &mut u64,
                next_edge: &mut u64,
                operations: &mut OperationBuffer,
                machine: symbols::SymbolHandle,
                parameters: &[crate::unit::StructuralParameterDeclaration],
                erased_parameters: &[ValueDeclaration],
                sequence: &mut super::emission::OperationSequence,
            ) -> Result<PlaceId, LoweringError> {
                let bindings = evaluation
                    .scalar_bindings
                    .clone()
                    .with_structural_parameters(&evaluation.structural_parameters)
                    .with_resolved_structural_observations(
                        &evaluation.structural_fields,
                        &evaluation.structural_cases,
                    )
                    .with_primitive_storage(&evaluation.primitive_storage)
                    .with_structural_locals(&evaluation.structural_locals)
                    .with_view_locals(&evaluation.view_locals)
                    .with_element_views(&evaluation.element_views);
                StateFrame::new(checked, machine, state, erased_parameters).evaluate(
                    catalogs,
                    parameters,
                    sequence,
                    FrameValues {
                        source_value_count,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        let context = byte_subslices::StagingContext {
                            state: state.state,
                            structural_types: environment.structural_types,
                            type_ids: environment.type_ids,
                            bindings: &bindings,
                        };
                        byte_subslices::emit(
                            identities,
                            checked,
                            &context,
                            operation,
                            ordinal,
                            emission.evaluation,
                            source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    },
                )
            }

            fn establish(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                parameter_qualifications: &[language_semantics::SemanticDomainId],
                catalogs: &mut catalogs::ComposedCatalogs,
                next_value: &mut u64,
                operations: &mut OperationBuffer,
            ) -> Result<PlaceId, LoweringError> {
                let bytes = argument
                    .byte_sequence_literal()
                    .ok_or(LoweringError::Unsupported(
                        "literal call argument has no payload",
                    ))?;
                let structural_type = lookup_type_id(&catalogs.type_ids, &argument.type_identity)?;
                if !argument.path.is_empty() {
                    return unsupported("literal call argument is not a whole byte carrier");
                }
                let qualifications = parameter_qualifications
                    .iter()
                    .map(|domain| {
                        lookup_domain_id_for_carrier(&catalogs.domain_ids, *domain, structural_type)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let declaration =
                    crate::emission::structural_byte_sequence_store::emit_call_literal(
                        identities,
                        bytes,
                        structural_type,
                        qualifications,
                        &catalogs.structural_types,
                        &mut catalogs.temporary_places,
                        &mut catalogs.next_place,
                        next_value,
                        &mut catalogs.scalar_calls.next_call_obligation,
                        operations,
                    )?;
                Ok(declaration.id)
            }
        }
        pub(crate) mod scalar_calls {

            pub(crate) use crate::scalar_lowering::call_closure::embedded::EmbeddedScalarCalls as ComposedScalarCalls;
        }
        pub(crate) mod state_graph {

            /*
            We lower the shared checked state graph through the ordinary call catalog. The
            historical Unit name now includes normal scalar-sum results: returning a case
            must preserve its payload, not force the caller onto a separate graph template.

            Preparation and emission have different jobs. admission.rs retains the states,
            call boundaries, parameter aliases and claim transport emission.rs needs before
            allocating blocks and places. The checked plan already selects this producer;
            there is no speculative preparation, source-shape retry or discarded unaliased
            claim resolution ahead of the real graph records.

            Invocation parameters are immutable, so a backedge to the authored entry needs
            a one-shot invocation block followed by an ordinary parameterized state block.
            A persistent receiver keeps its invocation place; transferred views and scalar
            values use their state-edge bindings. Do not turn self into another per-edge
            view or overwrite invocation parameters to make a loop fit.

            Local results need not become successor parameters. result_custody.rs rejoins
            their exact establishment and authored transfer roster; each ordinary edge
            disposes its own remaining affine locals after evaluating successor arguments.
            That partition uses existing Terminal edge cleanup, not a global result-drop
            flag. Copy payloads retain dominance without acquiring disposal obligations.
            */

            /*
            A primitive scalar result reuses the ordinary single-state completion rather
            than a graph-specific one. A final expression's value is the binding its last
            operation establishes in the `Return` role (the operation frame evaluates it),
            or an earlier immutable binding a final name reads. Value-only transition exits
            run the ordinary exit evaluator (`Evaluation::scalar_control_result`) inside
            the state and return its joined value. Either way the state then disposes the
            same whole roots a Unit return does.
            */

            /*
            returns.rs checks the selected nominal case and complete field roster against
            the authored return. We evaluate fields once in authored order, then encode them
            in declaration order; those orders can differ. The construction place and the
            machine result place are distinct. Bounded fields need their declaration-range
            proofs even when the surrounding control graph is otherwise admitted.

            Ordered value exits use guarded.rs and the ordinary structural-value emitter.
            Source-to-execution coverage belongs to tests/omega: the ownership state-graph
            fixture composes direct projections, retained local aliases, calls and backedges.
            */
            use crate::shared::declarations::state_declaration;
            use checked_trees::{
                CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
                CheckedStructuralControlSuccessorPlan,
            };

            pub(crate) mod admission {
                use super::super::super::{CheckedBoundaryMachinePlan, unsupported};
                use super::super::{CheckedTrees, LoweringError};
                use super::{
                    CheckedComposedUnitControlMachinePlan, claims, entry_aliases,
                    exclusive_handoffs, topology,
                };
                use crate::shared::checked_facts::flow_facts;
                use crate::unit::attached_unit::call_closure::BoundaryIdentity;

                pub(crate) struct AdmittedGraph<'a> {
                    pub(crate) live: Vec<bool>,
                    pub(crate) authored: Vec<(
                        &'a checked_trees::machine::Machine,
                        &'a checked_trees::state::State,
                    )>,
                    pub(crate) claim_transport: claims::ClaimTransport,
                    pub(crate) entry_aliases: Vec<std::collections::BTreeMap<u32, u32>>,
                    pub(crate) exclusive_handoffs: Vec<std::collections::BTreeSet<u32>>,
                    pub(crate) boundaries: Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
                }

                pub(crate) fn admit<'a>(
                    checked: &'a CheckedTrees,
                    plan: &CheckedComposedUnitControlMachinePlan,
                ) -> Result<AdmittedGraph<'a>, LoweringError> {
                    super::super::admission::validate_contract(checked, plan)?;
                    if plan.states.is_empty() || !plan.body_qualifications.is_empty() {
                        return unsupported(
                            "Unit graph has unsupported qualification or provider custody",
                        );
                    }
                    let machine =
                        checked
                            .machine(plan.machine)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph has no authored machine",
                            ))?;
                    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                        || !checked.machine_type_parameters(machine).is_empty()
                    {
                        return unsupported("Unit graph requires a closed checked body");
                    }
                    let authored = plan
                        .states
                        .iter()
                        .map(|state| {
                            crate::shared::declarations::state_declaration(checked, state.state)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    checked
                        .machine_states(machine)
                        .first()
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph has no authored entry state",
                        ))?;
                    if let Some(identity) = plan.result.structural_identity() {
                        let mut declarations = flow_facts(checked)
                            .terminal_unit_effects
                            .structural_types
                            .iter()
                            .filter(|declaration| declaration.identity == identity);
                        declarations.next().ok_or(LoweringError::Unsupported(
                            "Unit graph result carrier declaration is absent",
                        ))?;
                        if declarations.next().is_some() {
                            return unsupported(
                                "Unit graph result carrier declaration is ambiguous",
                            );
                        }
                    }
                    let live = topology::live(plan)?;
                    let mut entry_aliases =
                        vec![std::collections::BTreeMap::new(); plan.states.len()];
                    entry_aliases::alias_forwarded_entry_claims(plan, &live, &mut entry_aliases)?;
                    entry_aliases::alias_entry_parameter_loans(plan, &live, &mut entry_aliases)?;
                    let claim_transport =
                        claims::bind_invocation_claims(plan, &live, &entry_aliases)?;
                    let exclusive_handoffs = exclusive_handoffs::promote_shared_handoffs(
                        checked,
                        plan,
                        &live,
                        &entry_aliases,
                    )?;
                    let states = plan
                        .states
                        .iter()
                        .zip(&live)
                        .filter_map(|(state, live)| live.then_some(state))
                        .collect::<Vec<_>>();
                    let boundaries = super::super::admission::admit_calls(checked, &states)?;
                    Ok(AdmittedGraph {
                        live,
                        authored,
                        claim_transport,
                        entry_aliases,
                        exclusive_handoffs,
                        boundaries,
                    })
                }
            }
            pub(crate) mod custody_frontiers {
                use super::super::LoweringError;
                use checked_trees::{
                    CheckedComposedUnitControlMachinePlan as Plan, CheckedStructuralAccess,
                    CheckedStructuralControlSuccessorPlan as Edge,
                    CheckedStructuralControlTransferSourcePlan as Source,
                    CheckedUnitStructuralArgumentSourcePlan as Root,
                };
                use typed_trees::type_identity::TypeIdentityId;

                #[derive(Clone, Debug, PartialEq, Eq)]
                struct Owner {
                    birth: (usize, u32, u32),
                    carrier: TypeIdentityId,
                    parent: Option<Box<Owner>>,
                }

                impl Owner {
                    fn contains_birth(&self, birth: (usize, u32, u32)) -> bool {
                        self.birth == birth
                            || self
                                .parent
                                .as_ref()
                                .is_some_and(|p| p.contains_birth(birth))
                    }
                }

                #[derive(Clone, Debug, PartialEq, Eq)]
                struct Frontier {
                    source: usize,
                    parameters: Vec<Option<Owner>>,
                }

                pub(crate) fn prepare(
                    source: &Plan,
                    values: &checked_trees::CheckedStructuralValuePlans,
                ) -> Result<Plan, LoweringError> {
                    let Some(entry) = source.states.first() else {
                        return Err(LoweringError::Unsupported("custody frontier has no entry"));
                    };
                    let mut frontiers = vec![Frontier {
                        source: 0,
                        parameters: vec![None; entry.structural_parameters.len()],
                    }];
                    let mut states = Vec::new();
                    let mut cursor = 0;
                    while cursor < frontiers.len() {
                        let frontier = frontiers[cursor].clone();
                        let mut state = source.states[frontier.source].clone();
                        for edge in state.terminator.successors_mut() {
                            let target = source.successor_position(edge).ok_or(
                                LoweringError::Unsupported(
                                    "custody frontier successor disappeared",
                                ),
                            )?;
                            let incoming = arrive(source, values, &frontier, target, edge)?;
                            let occurrence = match frontiers
                                .iter()
                                .position(|candidate| candidate == &incoming)
                            {
                                Some(position) => position,
                                None => {
                                    let position = frontiers.len();
                                    frontiers.push(incoming);
                                    position
                                }
                            };
                            edge.target_occurrence =
                                Some(u32::try_from(occurrence).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "custody frontier occurrence exceeds u32",
                                    )
                                })?);
                        }
                        states.push(state);
                        cursor += 1;
                    }
                    let Plan {
                        machine,
                        result,
                        result_reference_sources,
                        natural_ranks,
                        rank_components,
                        preserving_rank_edges,
                        body_qualifications,
                        contract_report_fingerprint,
                        contract_commitment,
                        contract_service_reach,
                        service_reach,
                        states: _,
                        declaration_requirements,
                    } = source;
                    Ok(Plan {
                        machine: *machine,
                        result: result.clone(),
                        result_reference_sources: result_reference_sources.clone(),
                        natural_ranks: natural_ranks.clone(),
                        rank_components: rank_components.clone(),
                        preserving_rank_edges: preserving_rank_edges.clone(),
                        body_qualifications: body_qualifications.clone(),
                        contract_report_fingerprint: *contract_report_fingerprint,
                        contract_commitment: *contract_commitment,
                        contract_service_reach: *contract_service_reach,
                        service_reach: *service_reach,
                        states,
                        // Split states keep their set positions.
                        declaration_requirements: declaration_requirements.clone(),
                    })
                }

                fn arrive(
                    plan: &Plan,
                    values: &checked_trees::CheckedStructuralValuePlans,
                    source: &Frontier,
                    target: usize,
                    edge: &Edge,
                ) -> Result<Frontier, LoweringError> {
                    let mut parameters =
                        vec![None; plan.states[target].structural_parameters.len()];
                    for transfer in &edge.transfers {
                        let index = transfer.target_parameter_index as usize;
                        let parameter = &plan.states[target].structural_parameters[index];
                        if parameter.access == CheckedStructuralAccess::Owned {
                            continue;
                        }
                        let subject = super::result_custody::loan_subject(&transfer.source, values);
                        let parent_index = subject.and_then(|(subject, _)| match subject.source {
                            Root::Parameter { parameter_index } => Some(parameter_index as usize),
                            _ => None,
                        });
                        let (forwarded, carrier) = match &transfer.source {
                            Source::Parameter { index } => (Some(*index as usize), None),
                            Source::ProjectedLoan {
                                subject,
                                carrier_type_identity,
                                ..
                            } => {
                                let forwarded = parent_index.filter(|index| {
                                    subject.path.is_empty()
                                        && plan.states[source.source].structural_parameters[*index]
                                            .access
                                            == parameter.access
                                });
                                (
                                    forwarded,
                                    carrier_type_identity
                                        .is_valid()
                                        .then_some(*carrier_type_identity),
                                )
                            }
                            Source::EstablishedValue { establishment, .. } if subject.is_some() => {
                                (
                                    None,
                                    establishment
                                        .structural_result_binding()
                                        .map(|result| result.type_identity),
                                )
                            }
                            _ => (None, None),
                        };
                        parameters[index] = if let Some(index) = forwarded {
                            source.parameters[index].clone()
                        } else if let Some(carrier) = carrier.filter(|carrier| carrier.is_valid()) {
                            let parent =
                                parent_index.and_then(|index| source.parameters[index].clone());
                            let birth = (
                                source.source,
                                edge.statement_ordinal,
                                transfer.target_parameter_index,
                            );
                            if parent
                                .as_ref()
                                .is_some_and(|parent| parent.contains_birth(birth))
                            {
                                return Err(LoweringError::Unsupported(
                                    "captured state loan has an unbounded parent lifetime",
                                ));
                            }
                            Some(Owner {
                                birth,
                                carrier,
                                parent: parent.map(Box::new),
                            })
                        } else {
                            None
                        };
                    }
                    Ok(Frontier {
                        source: target,
                        parameters,
                    })
                }
            }
            pub(crate) mod emission {
                //! Emit a composed-control machine: `emit` allocates the state block
                //! namespaces and the invocation entry, walks every authored state through
                //! `StateGraphEmission::emit_state`, then assembles the machine from the
                //! blocks, ranks and places that walk accumulated.
                //!
                //! Source requirements are obligations of Psi checking, not a second language
                //! limited to this emitter's scalar vocabulary. A checked state retains both
                //! its closed projections and its unprojected authored expressions. The latter
                //! can cross an internal header only when every actual incoming edge carries
                //! the exact completed source-arrival decision. Closed declaration/runtime
                //! requirements remain in the emitted invariant; an unprojected expression
                //! never becomes Truth, an opaque certified atom, or an optimizer premise.
                use super::super::super::super::{
                    MachineId, StructuralArgument, StructuralParameterDeclaration, block_id,
                };
                use super::super::super::{
                    Block, MachineContract, StructuralPlaceDeclaration, StructuralPlaceKind,
                    TerminalMachine, Terminator, ValueDeclaration, allocate_dense, contract_id,
                    edge_id, lookup_claim_id, lookup_type_id, lower_checked_crash_route_buckets,
                    lower_installation_machine_service_ceiling, lower_unit_parameters,
                    obligation_id, unsupported,
                };
                use super::super::{CheckedTrees, LoweringError, catalogs};
                use super::{
                    AdmittedGraph, CheckedComposedUnitControlMachinePlan, ranking, returns,
                    successors,
                };
                use crate::emission::claim_bindings::CompletedValueClaim;
                use crate::emission::claim_bindings::UnreceivedResultClaim;
                use crate::emission::operation_emission::place_declarations::retain_block_parameters;
                use crate::expression_preparation::bindings::structural_paths::structural_access_can_supply;
                use crate::expression_preparation::bindings::structural_paths::terminal_access;
                use crate::expression_preparation::computation_graph::value_call_operations;
                use crate::scalar_lowering::computations::produced_values::prepare;
                use crate::scalar_lowering::contracts::erased_formal_declarations;
                use crate::scalar_lowering::contracts::erased_proof_formal_declarations;
                use crate::shared::checked_facts::{flow_facts, value_facts};
                use crate::shared::terminal_identities::value_id;
                use crate::unit::TerminalMachineResult;
                use crate::unit::attached_unit::claims::LoweredUnitClaims;
                use crate::unit::attached_unit::operation_frame::claim_established_here;
                use crate::unit::attached_unit::parameters::lower_unit_scalar_parameter_declarations;
                use crate::unit::attached_unit::signatures::find;
                use crate::unit::claim_id;
                use checked_trees::CheckedErasedProofParameterPlan;
                use semantic_vocabulary::ScalarTerm;
                use semantic_vocabulary::{BlockId, EdgeId, ValueId};
                use std::collections::BTreeMap;

                pub(crate) mod captured_loans {
                    use crate::LoweringError;
                    use crate::shared::terminal_identities::lookup_type_id;
                    use checked_trees::{
                        CheckedComposedUnitControlMachinePlan,
                        CheckedStructuralControlTransferSourcePlan as Source,
                        CheckedUnitStructuralArgumentSourcePlan as Root,
                    };
                    use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
                    use terminal_psi::{
                        StructuralAccess, StructuralArgument, StructuralMultiplicity,
                        StructuralParameterDeclaration, StructuralPlaceDeclaration,
                        TerminalAffineCleanupAction, TerminalMachine, Terminator,
                    };
                    use typed_trees::type_identity::TypeIdentityId;

                    #[derive(Default)]
                    pub(crate) struct CapturedLoans {
                        nodes: Vec<Vec<usize>>,
                        owners: Vec<Vec<Owner>>,
                        ancestry: Vec<(usize, Vec<usize>)>,
                    }

                    struct Owner {
                        group: usize,
                        declaration: StructuralParameterDeclaration,
                    }

                    fn root(links: &[usize], mut index: usize) -> usize {
                        while links[index] != index {
                            index = links[index];
                        }
                        index
                    }

                    fn depends_on(
                        parents: &[(usize, usize)],
                        child: usize,
                        ancestor: usize,
                    ) -> bool {
                        let mut pending = vec![child];
                        let mut visited = Vec::new();
                        while let Some(current) = pending.pop() {
                            if current == ancestor {
                                return true;
                            }
                            if visited.contains(&current) {
                                continue;
                            }
                            visited.push(current);
                            pending.extend(parents.iter().filter_map(|(child, parent)| {
                                (*child == current).then_some(*parent)
                            }));
                        }
                        false
                    }

                    fn forwarded(source: &Source) -> Option<u32> {
                        match source {
                            Source::Parameter { index } => Some(*index),
                            Source::ProjectedLoan { subject, .. } if subject.path.is_empty() => {
                                match subject.source {
                                    Root::Parameter { parameter_index } => Some(parameter_index),
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }

                    impl CapturedLoans {
                        pub(crate) fn plan(
                            plan: &CheckedComposedUnitControlMachinePlan,
                            values: &checked_trees::CheckedStructuralValuePlans,
                            live: &[bool],
                            views: &[Vec<StructuralParameterDeclaration>],
                            headers: &[BlockId],
                            aliases: &[std::collections::BTreeMap<u32, u32>],
                            catalogs: &mut crate::unit::attached_unit::composed_control::catalogs::ComposedCatalogs<'_>,
                            places: &mut Vec<StructuralPlaceDeclaration>,
                        ) -> Result<Self, LoweringError> {
                            let mut links = Vec::new();
                            let mut nodes = Vec::new();
                            for state in &plan.states {
                                nodes.push(
                                    state
                                        .structural_parameters
                                        .iter()
                                        .map(|_| {
                                            let index = links.len();
                                            links.push(index);
                                            index
                                        })
                                        .collect::<Vec<_>>(),
                                );
                            }
                            for (position, state) in plan
                                .states
                                .iter()
                                .enumerate()
                                .filter(|(position, _)| live[*position])
                            {
                                for edge in state.successors() {
                                    let target = plan.successor_position(edge).ok_or(
                                        LoweringError::Unsupported(
                                            "captured loan target disappeared",
                                        ),
                                    )?;
                                    for transfer in &edge.transfers {
                                        if position != 0
                                            && target != 0
                                            && let Some(source) = forwarded(&transfer.source)
                                            && state.structural_parameters[source as usize].access
                                                == plan.states[target].structural_parameters
                                                    [transfer.target_parameter_index as usize]
                                                    .access
                                            && matches!(
                                                state.structural_parameters[source as usize].access,
                                                checked_trees::checked_borrowed_access_pair_pattern!()
                                            )
                                            && matches!(
                                                plan.states[target].structural_parameters
                                                    [transfer.target_parameter_index as usize]
                                                    .access,
                                                checked_trees::checked_borrowed_access_pair_pattern!()
                                            )
                                        {
                                            let left =
                                                root(&links, nodes[position][source as usize]);
                                            let right = root(
                                                &links,
                                                nodes[target]
                                                    [transfer.target_parameter_index as usize],
                                            );
                                            let coexist =
                                                nodes.iter().enumerate().any(|(state, members)| {
                                                    live[state]
                                                        && members.iter().any(|member| {
                                                            root(&links, *member) == left
                                                        })
                                                        && members.iter().any(|member| {
                                                            root(&links, *member) == right
                                                        })
                                                });
                                            if !coexist {
                                                links[right] = left;
                                            }
                                        }
                                    }
                                }
                            }
                            for state in &mut nodes {
                                for index in state {
                                    *index = root(&links, *index);
                                }
                            }
                            let mut types: Vec<(usize, TypeIdentityId)> = Vec::new();
                            let mut shared_byte_carriers = Vec::new();
                            for (position, state) in plan
                                .states
                                .iter()
                                .enumerate()
                                .filter(|(position, _)| live[*position])
                            {
                                for edge in state.successors() {
                                    let target = plan.successor_position(edge).ok_or(
                                        LoweringError::Unsupported(
                                            "captured loan target disappeared",
                                        ),
                                    )?;
                                    for transfer in &edge.transfers {
                                        let carrier_type_identity = match &transfer.source {
                                            Source::ProjectedLoan {
                                                carrier_type_identity,
                                                subject,
                                                ..
                                            } if subject.path.iter().any(|segment| {
                                                matches!(segment,
                                                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                                                    | checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(_))
                                            }) =>
                                            {
                                                Some(*carrier_type_identity)
                                            }
                                            Source::EstablishedValue { establishment, .. }
                                                if super::super::result_custody::loan_subject(
                                                    &transfer.source,
                                                    values,
                                                )
                                                .is_some() =>
                                            {
                                                establishment
                                                    .structural_result_binding()
                                                    .map(|result| result.type_identity)
                                            }
                                            _ => None,
                                        };
                                        if let Some(carrier_type_identity) = carrier_type_identity
                                            .filter(|identity| identity.is_valid())
                                        {
                                            let group = nodes[target]
                                                [transfer.target_parameter_index as usize];
                                            if let Some((_, previous)) = types
                                                .iter()
                                                .find(|(candidate, _)| *candidate == group)
                                            {
                                                if *previous != carrier_type_identity {
                                                    let shape = |identity| {
                                                        let carrier = lookup_type_id(
                                                            &catalogs.type_ids,
                                                            identity,
                                                        )?;
                                                        terminal_psi::find_structural_type(
                                                            &catalogs.structural_types,
                                                            carrier,
                                                        )
                                                        .map(|declaration| {
                                                            declaration.shape.clone()
                                                        })
                                                        .ok_or(LoweringError::Unsupported(
                                                            "captured loan descriptor has no shape",
                                                        ))
                                                    };
                                                    return Err(LoweringError::CapturedLoanDescriptorMismatch {
                                                        source_state: position,
                                                        target_state: target,
                                                        target_parameter: transfer.target_parameter_index,
                                                        earlier: shape(previous)?,
                                                        incoming: shape(&carrier_type_identity)?,
                                                        source_access: super::super::result_custody::loan_subject(
                                                            &transfer.source,
                                                            values,
                                                        )
                                                        .map_or(
                                                            checked_trees::CheckedStructuralAccess::Owned,
                                                            |(subject, _)| subject.access,
                                                        ),
                                                        target_access: plan.states[target].structural_parameters
                                                            [transfer.target_parameter_index as usize]
                                                            .access,
                                                    });
                                                }
                                            } else {
                                                types.push((group, carrier_type_identity));
                                            }
                                            let carrier = lookup_type_id(
                                                &catalogs.type_ids,
                                                &carrier_type_identity,
                                            )?;
                                            if plan.states[target].structural_parameters
                                                [transfer.target_parameter_index as usize]
                                                .access
                                                == checked_trees::CheckedStructuralAccess::SharedBorrow
                                                && terminal_psi::find_structural_type(
                                                    &catalogs.structural_types,
                                                    carrier,
                                                )
                                                .is_some_and(|declaration| {
                                                    matches!(
                                                        declaration.shape,
                                                        terminal_psi::StructuralTypeShape::ByteSequence(
                                                            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                                        )
                                                    )
                                                })
                                                && !shared_byte_carriers.contains(&group)
                                            {
                                                shared_byte_carriers.push(group);
                                            }
                                        }
                                    }
                                }
                                let _ = position;
                            }
                            let mut parents = Vec::new();
                            for (position, state) in plan
                                .states
                                .iter()
                                .enumerate()
                                .filter(|(position, _)| live[*position])
                            {
                                for edge in state.successors() {
                                    let target = plan.successor_position(edge).ok_or(
                                        LoweringError::Unsupported(
                                            "captured loan target disappeared",
                                        ),
                                    )?;
                                    for transfer in &edge.transfers {
                                        if let Some((subject, _)) =
                                            super::super::result_custody::loan_subject(
                                                &transfer.source,
                                                values,
                                            )
                                            && let Root::Parameter { parameter_index } =
                                                subject.source
                                        {
                                            let parent = nodes[position][parameter_index as usize];
                                            let child = nodes[target]
                                                [transfer.target_parameter_index as usize];
                                            if !(subject.path.is_empty() && child == parent)
                                                && !parents.contains(&(child, parent))
                                            {
                                                parents.push((child, parent));
                                            }
                                        }
                                    }
                                }
                            }
                            let original_parents = parents.clone();
                            for (child, _) in &original_parents {
                                let alternatives = types
                                    .iter()
                                    .filter_map(|(ancestor, _)| {
                                        original_parents
                                            .iter()
                                            .any(|(candidate, parent)| {
                                                candidate == child
                                                    && depends_on(
                                                        &original_parents,
                                                        *parent,
                                                        *ancestor,
                                                    )
                                            })
                                            .then_some(*ancestor)
                                    })
                                    .collect::<Vec<_>>();
                                for (index, left) in alternatives.iter().enumerate() {
                                    for right in &alternatives[index + 1..] {
                                        let left_root = root(&links, *left);
                                        let right_root = root(&links, *right);
                                        if left_root == right_root {
                                            continue;
                                        }
                                        if types
                                            .iter()
                                            .find(|(group, _)| group == left)
                                            .map(|(_, identity)| identity)
                                            != types
                                                .iter()
                                                .find(|(group, _)| group == right)
                                                .map(|(_, identity)| identity)
                                        {
                                            continue;
                                        }
                                        let coexist = types
                                            .iter()
                                            .filter(|(group, _)| root(&links, *group) == left_root)
                                            .any(|(left, _)| {
                                                types
                                                    .iter()
                                                    .filter(|(group, _)| root(&links, *group) == right_root)
                                                    .any(|(right, _)| {
                                                        depends_on(&original_parents, *left, *right)
                                                            || depends_on(&original_parents, *right, *left)
                                                            || nodes.iter().enumerate().any(|(state, members)| {
                                                                live[state]
                                                                    && members.iter().any(|first| {
                                                                        members.iter().any(|second| {
                                                                            first != second
                                                                                && depends_on(
                                                                                    &original_parents,
                                                                                    *first,
                                                                                    *left,
                                                                                )
                                                                                && depends_on(
                                                                                    &original_parents,
                                                                                    *second,
                                                                                    *right,
                                                                                )
                                                                        })
                                                                    })
                                                            })
                                                    })
                                            });
                                        if !coexist {
                                            links[right_root] = left_root;
                                        }
                                    }
                                }
                            }
                            for state in &mut nodes {
                                for group in state {
                                    *group = root(&links, *group);
                                }
                            }
                            for (group, _) in &mut types {
                                *group = root(&links, *group);
                            }
                            for group in &mut shared_byte_carriers {
                                *group = root(&links, *group);
                            }
                            types.sort_by_key(|(group, _)| *group);
                            types.dedup_by_key(|(group, _)| *group);
                            for (child, parent) in &mut parents {
                                *child = root(&links, *child);
                                *parent = root(&links, *parent);
                            }
                            parents.sort_unstable();
                            parents.dedup();
                            let mut ancestry = Vec::new();
                            let mut groups = Vec::new();
                            for group in nodes.iter().flatten() {
                                if !groups.contains(group) {
                                    groups.push(*group);
                                }
                            }
                            for group in &groups {
                                let mut ancestors = Vec::new();
                                let mut pending = vec![*group];
                                while let Some(child) = pending.pop() {
                                    for (_, parent) in
                                        parents.iter().filter(|(candidate, _)| *candidate == child)
                                    {
                                        if parent == group {
                                            return Err(LoweringError::Unsupported(
                                                "captured loan has a cyclic parent lifetime",
                                            ));
                                        }
                                        if !ancestors.contains(parent) {
                                            ancestors.push(*parent);
                                            pending.push(*parent);
                                        }
                                    }
                                }
                                ancestry.push((*group, ancestors));
                            }
                            types.sort_by_key(|(group, _)| {
                                std::cmp::Reverse(
                                    ancestry
                                        .iter()
                                        .find(|(candidate, _)| candidate == group)
                                        .map_or(0, |(_, ancestors)| ancestors.len()),
                                )
                            });
                            let mut owners = Vec::new();
                            for (position, state) in nodes.iter().enumerate() {
                                let mut roster = Vec::new();
                                if live[position] {
                                    for (group, identity) in &types {
                                        if !state.contains(group)
                                            && !ancestry.iter().any(|(child, ancestors)| {
                                                state.contains(child) && ancestors.contains(group)
                                            })
                                        {
                                            continue;
                                        }
                                        if position == 0 {
                                            return Err(LoweringError::Unsupported(
                                                "captured state loan requires invocation carrier arrival",
                                            ));
                                        }
                                        let structural_type =
                                            lookup_type_id(&catalogs.type_ids, identity)?;
                                        let place = super::super::super::super::place_id(
                                            super::super::super::super::allocate_dense(
                                                &mut catalogs.next_place,
                                            )?,
                                        );
                                        let index = views[position].len() - aliases[position].len()
                                            + roster.len();
                                        let parameter = StructuralParameterDeclaration {
                                            place,
                                            position: u32::try_from(index).map_err(|_| {
                                                LoweringError::Unsupported(
                                                    "captured loan roster exceeds u32",
                                                )
                                            })?,
                                            structural_type,
                                            multiplicity: if shared_byte_carriers.contains(group) {
                                                StructuralMultiplicity::Unrestricted
                                            } else {
                                                StructuralMultiplicity::Affine
                                            },
                                            access: StructuralAccess::Owned,
                                            qualifications: Vec::new(),
                                            projected_qualifications: Vec::new(),
                                            additional_input_qualifications: Vec::new(),
                                        };
                                        places.push(StructuralPlaceDeclaration {
                                            id: place,
                                            kind: StructuralPlaceKind::BlockParameter {
                                                block: headers[position],
                                                position: parameter.position,
                                            },
                                        });
                                        roster.push(Owner {
                                            group: *group,
                                            declaration: parameter,
                                        });
                                    }
                                }
                                owners.push(roster);
                            }
                            Ok(Self {
                                nodes,
                                owners,
                                ancestry,
                            })
                        }

                        pub(crate) fn group(&self, state: usize, parameter: u32) -> Option<usize> {
                            let group = *self.nodes.get(state)?.get(parameter as usize)?;
                            self.owners[state]
                                .iter()
                                .any(|owner| owner.group == group)
                                .then_some(group)
                        }

                        pub(crate) fn declarations(
                            &self,
                            state: usize,
                        ) -> impl Iterator<Item = StructuralParameterDeclaration> + '_
                        {
                            self.owners[state]
                                .iter()
                                .map(|owner| owner.declaration.clone())
                        }

                        pub(crate) fn parent_carrier(
                            &self,
                            state: usize,
                            parameter: u32,
                        ) -> Option<&StructuralParameterDeclaration> {
                            let group = *self.nodes.get(state)?.get(parameter as usize)?;
                            self.owners[state]
                                .iter()
                                .find(|owner| owner.group == group)
                                .map(|owner| &owner.declaration)
                        }

                        pub(crate) fn arrivals(
                            &self,
                            source: usize,
                            target: usize,
                            edge: &checked_trees::CheckedStructuralControlSuccessorPlan,
                            captures: &[(usize, PlaceId)],
                        ) -> Result<Vec<StructuralArgument>, LoweringError>
                        {
                            self.owners[target]
                                .iter()
                                .map(|owner| {
                                    let captures = captures
                                        .iter()
                                        .filter(|(group, _)| *group == owner.group)
                                        .collect::<Vec<_>>();
                                    if captures.len() > 1 {
                                        return Err(LoweringError::Unsupported(
                                            "one state loan owner receives multiple captures",
                                        ));
                                    }
                                    let place = if let Some((_, place)) = captures.first() {
                                        *place
                                    } else {
                                        let mut forwarded_groups = edge.transfers.iter().filter_map(|transfer| {
                                            (self.nodes[target][transfer.target_parameter_index as usize]
                                                == owner.group)
                                                .then(|| {
                                                    forwarded(&transfer.source)
                                                        .map(|parameter| self.nodes[source][parameter as usize])
                                                })
                                                .flatten()
                                        });
                                        let forwarded_group = forwarded_groups.next();
                                        if forwarded_groups.any(|group| Some(group) != forwarded_group) {
                                            return Err(LoweringError::Unsupported(
                                                "captured loan arrival has ambiguous source occurrences",
                                            ));
                                        }
                                        let retained_parent = self.ancestry.iter().any(|(child, ancestors)| {
                                            self.nodes[target].contains(child) && ancestors.contains(&owner.group)
                                        });
                                        if forwarded_group.is_none() && !retained_parent {
                                            return Err(LoweringError::Unsupported(
                                                "captured loan arrival does not forward its source resource",
                                            ));
                                        }
                                        self.owners[source]
                                            .iter()
                                            .find(|previous| previous.group == forwarded_group.unwrap_or(owner.group))
                                            .ok_or(LoweringError::Unsupported(
                                                "captured loan arrival has no matching owner",
                                            ))?
                                            .declaration
                                            .place
                                    };
                                    Ok(StructuralArgument {
                                        place,
                                        path: Vec::new(),
                                        access: StructuralAccess::Owned,
                                    })
                                })
                                .collect()
                        }

                        pub(crate) fn finish(
                            &self,
                            machine: &mut TerminalMachine,
                            headers: &[BlockId],
                        ) -> Result<(), LoweringError> {
                            for (position, owners) in self
                                .owners
                                .iter()
                                .enumerate()
                                .filter(|(_, owners)| !owners.is_empty())
                            {
                                for owner in owners {
                                    if owner.declaration.multiplicity
                                        != StructuralMultiplicity::Affine
                                    {
                                        continue;
                                    }
                                    let mut region =
                                        vec![(headers[position], owner.declaration.place)];
                                    let mut cursor = 0;
                                    while cursor < region.len() {
                                        let (block, place) = region[cursor];
                                        let block = machine
                                            .blocks
                                            .iter()
                                            .find(|candidate| candidate.id == block)
                                            .ok_or(LoweringError::Unsupported(
                                                "captured loan private block disappeared",
                                            ))?;
                                        let arrivals = match &block.terminator {
                                            Terminator::Jump {
                                                target,
                                                structural_arguments,
                                                ..
                                            } => vec![(*target, structural_arguments.as_slice())],
                                            Terminator::Conditional {
                                                when_true,
                                                when_false,
                                                ..
                                            } => vec![
                                                (
                                                    when_true.target,
                                                    when_true.structural_arguments.as_slice(),
                                                ),
                                                (
                                                    when_false.target,
                                                    when_false.structural_arguments.as_slice(),
                                                ),
                                            ],
                                            Terminator::StructuralCase { cases, .. } => cases
                                                .iter()
                                                .map(|arm| {
                                                    (
                                                        arm.target,
                                                        arm.structural_arguments.as_slice(),
                                                    )
                                                })
                                                .collect(),
                                            _ => Vec::new(),
                                        };
                                        for (target, arguments) in arrivals {
                                            if headers.contains(&target) {
                                                continue;
                                            }
                                            let destination = machine
                                                .blocks
                                                .iter()
                                                .find(|block| block.id == target)
                                                .ok_or(LoweringError::Unsupported(
                                                    "captured loan private target disappeared",
                                                ))?;
                                            let moved = arguments
                                                .iter()
                                                .zip(&destination.structural_parameters)
                                                .filter(|(argument, _)| {
                                                    argument.place == place
                                                        && argument.access
                                                            == StructuralAccess::Owned
                                                        && argument.path.is_empty()
                                                })
                                                .map(|(_, parameter)| parameter.place)
                                                .collect::<Vec<_>>();
                                            if moved.len() > 1 {
                                                return Err(LoweringError::Unsupported(
                                                    "captured loan duplicates a private owner",
                                                ));
                                            }
                                            let arriving = moved.first().copied().unwrap_or(place);
                                            if let Some((_, previous)) =
                                                region.iter().find(|(block, _)| *block == target)
                                            {
                                                if *previous != arriving {
                                                    return Err(LoweringError::Unsupported(
                                                        "captured loan private join changes owner correspondence",
                                                    ));
                                                }
                                            } else {
                                                region.push((target, arriving));
                                            }
                                        }
                                        cursor += 1;
                                    }
                                    for (block, place) in region {
                                        let block = machine
                                            .blocks
                                            .iter_mut()
                                            .find(|candidate| candidate.id == block)
                                            .ok_or(LoweringError::Unsupported(
                                                "captured loan private block disappeared",
                                            ))?;
                                        let retire = |target: BlockId,
                                                      arguments: &[StructuralArgument],
                                                      actions: &mut Vec<
                                            TerminalAffineCleanupAction,
                                        >| {
                                            if headers.contains(&target)
                                                && !arguments.iter().any(|argument| {
                                                    argument.place == place
                                                        && argument.access
                                                            == StructuralAccess::Owned
                                                        && argument.path.is_empty()
                                                })
                                            {
                                                actions.push(
                                                    TerminalAffineCleanupAction::DiscardRoot(place),
                                                );
                                            }
                                        };
                                        match &mut block.terminator {
                                            Terminator::Jump {
                                                target,
                                                structural_arguments,
                                                cleanup_actions,
                                                ..
                                            } => retire(
                                                *target,
                                                structural_arguments,
                                                cleanup_actions,
                                            ),
                                            Terminator::Conditional {
                                                when_true,
                                                when_false,
                                                ..
                                            } => {
                                                for arm in [when_true, when_false] {
                                                    retire(
                                                        arm.target,
                                                        &arm.structural_arguments,
                                                        &mut arm.cleanup_actions,
                                                    );
                                                }
                                            }
                                            Terminator::StructuralCase { cases, .. } => {
                                                for arm in cases {
                                                    retire(
                                                        arm.target,
                                                        &arm.structural_arguments,
                                                        &mut arm.cleanup_actions,
                                                    );
                                                }
                                            }
                                            terminal_psi::terminator_return_pair_pattern!(
                                                cleanup_actions,
                                                ..
                                            )
                                            | Terminator::ReturnStructural {
                                                cleanup_actions,
                                                ..
                                            } => cleanup_actions.push(
                                                TerminalAffineCleanupAction::DiscardRoot(place),
                                            ),
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Ok(())
                        }
                    }
                }
                pub(crate) mod completion_expressions {
                    //! A Unit expression selects ordinary invocation completions. The subject is
                    //! evaluated once; only selected predicates and the selected call execute.
                    //! Unit never acquires a scalar slot or structural result home.

                    use super::StateGraphEmission;
                    use crate::emission::operation_emission::buffer::{
                        OperationBuffer, scalar_result_declaration,
                    };
                    use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
                    use crate::emission::value_successor;
                    use crate::shared::lowering_error::LoweringError;
                    use crate::shared::terminal_identities::{
                        allocate_dense, block_id, edge_id, value_id,
                    };
                    use crate::unit::attached_unit::argument_evaluation::Evaluation;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, OperationSequence, StateFrame,
                    };
                    use checked_trees::{
                        CheckedScalarExpressionRole as Role, CheckedUnitCompletionExpression,
                        CheckedUnitCompletionKind as Kind,
                        CheckedUnitCompletionSelection as Selection,
                        CheckedUnitScalarCompletionCondition as Condition,
                    };
                    use terminal_psi::{
                        Block, StructuralParameterDeclaration, SuccessorEdge, Terminator,
                        ValueDeclaration,
                    };

                    #[derive(Clone, Copy)]
                    enum Arm<'a> {
                        Scalar(&'a checked_trees::CheckedUnitScalarCompletionArm, usize),
                        Structural(&'a checked_trees::CheckedUnitStructuralCompletionArm),
                    }

                    impl Arm<'_> {
                        fn computation(
                            &self,
                        ) -> Option<checked_trees::CheckedScalarComputationHandle>
                        {
                            match self {
                                Self::Scalar(arm, _) => arm.condition.computation(),
                                Self::Structural(arm) => arm.predicate,
                            }
                        }
                        fn completion(&self) -> &CheckedUnitCompletionExpression {
                            match self {
                                Self::Scalar(arm, _) => &arm.completion,
                                Self::Structural(arm) => &arm.completion,
                            }
                        }
                    }

                    impl StateGraphEmission<'_, '_> {
                        #[allow(clippy::too_many_arguments)]
                        pub(super) fn completion_expression(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            parameters: &[StructuralParameterDeclaration],
                            values: Vec<ValueDeclaration>,
                            established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
                            payload_parameters: &[ValueDeclaration],
                            discards: &[semantic_vocabulary::PlaceId],
                            plan: &checked_trees::CheckedUnitCompletionPlan,
                            evaluation: &mut Evaluation,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            sequence: &OperationSequence,
                        ) -> Result<SuccessorEdge, LoweringError> {
                            self.completion_node(
                                identities,
                                position,
                                parameters,
                                values,
                                established,
                                payload_parameters,
                                discards,
                                plan,
                                &plan.expression,
                                evaluation,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                                sequence,
                            )
                        }

                        #[allow(clippy::too_many_arguments)]
                        fn completion_node(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            parameters: &[StructuralParameterDeclaration],
                            mut values: Vec<ValueDeclaration>,
                            established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
                            payload_parameters: &[ValueDeclaration],
                            discards: &[semantic_vocabulary::PlaceId],
                            plan: &checked_trees::CheckedUnitCompletionPlan,
                            node: &CheckedUnitCompletionExpression,
                            evaluation: &mut Evaluation,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            sequence: &OperationSequence,
                        ) -> Result<SuccessorEdge, LoweringError> {
                            let checked = self.checked;
                            let dispatch = match &node.kind {
                                Kind::Invocation(invocation) => {
                                    return self.conditional_return_arm(
                                        identities,
                                        position,
                                        parameters,
                                        values,
                                        established,
                                        payload_parameters,
                                        discards,
                                        &checked_trees::CheckedConditionalReturnArm::EvaluateUnit {
                                            statement_ordinal: plan.statement_ordinal,
                                            operations: invocation.clone(),
                                        },
                                        evaluation,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        sequence,
                                    );
                                }
                                Kind::Selection(selection) => selection,
                            };
                            let state = &self.plan.states[position];
                            let owner = self.admitted.authored[position].0.symbol;
                            let entry = block_id(allocate_dense(next_block)?);
                            let mut current = evaluation.branch(entry, operations.len());
                            crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
                &mut current.structural_fields,
                established,
            );
                            current.parameters = payload_parameters.to_vec();
                            let mut sequence = sequence.branch();
                            let source_count = values.len();
                            let (scalar, structural) = match dispatch {
                                Selection::Scalar { subject, arms } => {
                                    let completed = self.completion_scalar(
                                        identities,
                                        position,
                                        parameters,
                                        plan.statement_ordinal,
                                        Role::StructuralValueSubject {
                                            expression: node.expression,
                                        },
                                        *subject,
                                        &mut current,
                                        &mut values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?;
                                    values.push(completed);
                                    (Some(arms.as_slice()), None)
                                }
                                Selection::Structural {
                                    expression: subject_expression,
                                    value,
                                    alias,
                                    place,
                                    arms,
                                } => {
                                    let request = value.is_valid().then(|| crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
                    identities, checked, *value, &self.catalogs.structural_types,
                )).transpose()?;
                                    StateFrame::new(checked, owner, state, &self.state_erased[position]).evaluate(
                    self.catalogs,
                    parameters,
                    &mut sequence,
                    FrameValues {
                        source_value_count: values.len(),
                        evaluation: &mut current,
                        values: &mut values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        if let Some(request) = request {
                            environment.produce_value(
                                identities, *value, request.structural_type,
                                emission.evaluation, emission.values, emission.next_value,
                                emission.next_block, emission.next_edge, emission.calls, emission.operations,
                            )
                        } else {
                            let captured = crate::shared::terminal_identities::place_id(allocate_dense(environment.next_place)?);
                            crate::unit::attached_unit::structural_values::emission::completed_projection::ProjectionEmission {
                                statement: plan.statement_ordinal,
                                environment: &mut environment, calls: emission.calls,
                                evaluation: emission.evaluation, values: emission.values,
                                next_value: emission.next_value, next_block: emission.next_block,
                                next_edge: emission.next_edge, operations: emission.operations,
                            }.capture_subject(identities, *subject_expression, place, *alias, captured)
                        }
                    },
                )?;
                                    if value.is_valid()
                                        && operations.invocation_completion.is_none()
                                    {
                                        current.bind_completed_value_alias(
                                            *value, *alias, operations,
                                        )?;
                                        current.completed_operand_scope.push(*value);
                                    }
                                    (None, Some(arms.as_slice()))
                                }
                            };
                            if let Some(terminator) =
                                operations.take_invocation_completion(next_edge)?
                            {
                                finish(&mut current, operations, terminator);
                                evaluation.blocks.extend(current.blocks);
                                return Ok(value_successor(
                                    edge_id(allocate_dense(next_edge)?),
                                    entry,
                                ));
                            }
                            let arms = scalar
                                .into_iter()
                                .flatten()
                                .map(|arm| Arm::Scalar(arm, source_count))
                                .chain(structural.into_iter().flatten().map(Arm::Structural));
                            let arm_count =
                                scalar.map_or(0, <[_]>::len) + structural.map_or(0, <[_]>::len);
                            for (ordinal, arm) in arms.enumerate() {
                                let pattern = arm.computation();
                                let condition = if let Some(pattern) = pattern {
                                    let pattern = self.completion_scalar(
                                        identities,
                                        position,
                                        parameters,
                                        plan.statement_ordinal,
                                        Role::StructuralValuePattern {
                                            source_arm: match arm {
                                                Arm::Scalar(arm, _) => arm.source_arm,
                                                Arm::Structural(arm) => arm.source_arm,
                                            },
                                        },
                                        pattern,
                                        &mut current,
                                        &mut values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?;
                                    let kind = match arm {
                                        Arm::Scalar(arm, subject) => match arm.condition {
                                            Condition::Boolean(_) => {
                                                Some(terminal_psi::OperationKind::BooleanEqual {
                                                    left: values[subject].id,
                                                    right: pattern.id,
                                                })
                                            }
                                            Condition::Integer(_) => Some(
                                                LoweredIntegerComparisonKind::Equal
                                                    .operation(values[subject].id, pattern.id),
                                            ),
                                            Condition::Predicate(_) | Condition::Default => None,
                                        },
                                        Arm::Structural(_) => None,
                                    };
                                    if let Some(kind) = kind {
                                        let condition = value_id(allocate_dense(next_value)?);
                                        let id = operations.allocate();
                                        operations.push(terminal_psi::Operation::unsuspended(
                                            id,
                                            scalar_result_declaration(
                                                condition,
                                                semantic_vocabulary::ScalarType::Boolean,
                                            ),
                                            kind,
                                        ));
                                        Some(condition)
                                    } else {
                                        Some(pattern.id)
                                    }
                                } else {
                                    None
                                };
                                if let Some(terminator) =
                                    operations.take_invocation_completion(next_edge)?
                                {
                                    finish(&mut current, operations, terminator);
                                    break;
                                }
                                let observations = operations.storage_observations.clone();
                                let decision_end = operations.len();
                                let mut selected_values = values.clone();
                                selected_values.truncate(source_count);
                                let selected = self.completion_node(
                                    identities,
                                    position,
                                    parameters,
                                    selected_values,
                                    &[],
                                    &[],
                                    discards,
                                    plan,
                                    arm.completion(),
                                    &mut current,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                    &sequence,
                                )?;
                                operations.storage_observations = observations;
                                let condition = match condition {
                                    Some(condition) if ordinal + 1 < arm_count => condition,
                                    _ => {
                                        finish_at(
                                            &mut current,
                                            operations,
                                            decision_end,
                                            jump(selected),
                                        );
                                        break;
                                    }
                                };
                                let fallback = block_id(allocate_dense(next_block)?);
                                finish_at(
                                    &mut current,
                                    operations,
                                    decision_end,
                                    Terminator::Conditional {
                                        condition,
                                        when_true: selected,
                                        when_false: value_successor(
                                            edge_id(allocate_dense(next_edge)?),
                                            fallback,
                                        ),
                                    },
                                );
                                current.current = fallback;
                                current.operation_start = operations.len();
                                current.parameters.clear();
                                current.block_structural_parameters.clear();
                                operations.storage_observations.clear();
                            }
                            evaluation.blocks.extend(current.blocks);
                            Ok(value_successor(edge_id(allocate_dense(next_edge)?), entry))
                        }

                        #[allow(clippy::too_many_arguments)]
                        fn completion_scalar(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            parameters: &[StructuralParameterDeclaration],
                            statement: u32,
                            role: Role,
                            computation: checked_trees::CheckedScalarComputationHandle,
                            evaluation: &mut Evaluation,
                            values: &mut Vec<ValueDeclaration>,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            sequence: &mut OperationSequence,
                        ) -> Result<ValueDeclaration, LoweringError> {
                            StateFrame::new(
                                self.checked,
                                self.admitted.authored[position].0.symbol,
                                &self.plan.states[position],
                                &self.state_erased[position],
                            )
                            .evaluate(
                                self.catalogs,
                                parameters,
                                sequence,
                                FrameValues {
                                    source_value_count: values.len(),
                                    evaluation,
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                },
                                |mut frame| {
                                    frame.source_value(
                                        identities,
                                        statement,
                                        role,
                                        &checked_trees::CheckedCallScalarArgument::Computation(
                                            computation,
                                        ),
                                    )
                                },
                            )
                        }
                    }

                    fn finish(
                        evaluation: &mut Evaluation,
                        operations: &OperationBuffer,
                        terminator: Terminator,
                    ) {
                        finish_at(evaluation, operations, operations.len(), terminator)
                    }

                    fn finish_at(
                        evaluation: &mut Evaluation,
                        operations: &OperationBuffer,
                        end: usize,
                        terminator: Terminator,
                    ) {
                        evaluation.blocks.push(Block {
                            id: evaluation.current,
                            parameters: std::mem::take(&mut evaluation.parameters),
                            structural_parameters: std::mem::take(
                                &mut evaluation.block_structural_parameters,
                            ),
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            operations: operations[evaluation.operation_start..end].to_vec(),
                            terminator,
                        });
                    }

                    fn jump(edge: SuccessorEdge) -> Terminator {
                        Terminator::Jump {
                            edge: edge.edge,
                            target: edge.target,
                            arguments: edge.arguments,
                            structural_arguments: edge.structural_arguments,
                            erased_arguments: edge.erased_arguments,
                            erased_proof_arguments: edge.erased_proof_arguments,
                            cleanup_actions: edge.cleanup_actions,
                        }
                    }
                }
                mod conditional_return {

                    use super::super::super::super::super::{
                        StructuralParameterDeclaration, SuccessorEdge, block_id,
                    };
                    use super::super::super::super::{
                        Block, CheckedScalarExpressionRole, Terminator, ValueDeclaration,
                        allocate_dense, edge_id, terminal_scalar_type, unsupported,
                    };
                    use super::super::super::LoweringError;
                    use super::StateGraphEmission;
                    use crate::emission::operation_emission::buffer::{
                        OperationBuffer, StructuralBinding,
                    };
                    use crate::emission::value_successor;
                    use crate::expression_preparation::bindings::structural_fields::establish_case_payloads;
                    use crate::expression_preparation::qualifications::SourceScalarReturn;
                    use crate::shared::checked_facts::value_facts;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, StateFrame,
                    };

                    enum PreparedArmCompletion {
                        Edge(Terminator),
                        Structural(StructuralBinding),
                    }

                    #[derive(Clone, Copy)]
                    enum OrdinaryArm<'a> {
                        EvaluateNoNormal {
                            operations: &'a checked_trees::CheckedOperandSequence,
                        },
                        EvaluateUnit {
                            operations: &'a checked_trees::CheckedOperandSequence,
                        },
                        Crash {
                            statement_ordinal: &'a u32,
                        },
                        Unit,
                        Structural(&'a checked_trees::CheckedUnitEffectOperationPlan),
                        Scalar {
                            statement_ordinal: &'a u32,
                        },
                        Call {
                            operands: &'a checked_trees::CheckedOperandSequence,
                            operation: &'a checked_trees::CheckedUnitEffectOperationPlan,
                            completions: &'a checked_trees::CheckedOperandSequence,
                        },
                    }

                    impl StateGraphEmission<'_, '_> {
                        pub(crate) fn conditional_return_arm(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            state_parameters: &[StructuralParameterDeclaration],
                            arm_namespace: Vec<ValueDeclaration>,
                            established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
                            payload_parameters: &[ValueDeclaration],
                            dispatch_discards: &[semantic_vocabulary::PlaceId],
                            return_arm: &checked_trees::CheckedConditionalReturnArm,
                            evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            sequence: &crate::unit::attached_unit::composed_control::emission::OperationSequence,
                        ) -> Result<SuccessorEdge, LoweringError> {
                            let ordinary = match return_arm {
                                checked_trees::CheckedConditionalReturnArm::Completion(plan) => {
                                    return self.completion_expression(
                                        identities,
                                        position,
                                        state_parameters,
                                        arm_namespace,
                                        established,
                                        payload_parameters,
                                        dispatch_discards,
                                        plan,
                                        evaluation,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        sequence,
                                    );
                                }
                                checked_trees::CheckedConditionalReturnArm::EvaluateNoNormal {
                                    operations,
                                    ..
                                } => OrdinaryArm::EvaluateNoNormal { operations },
                                checked_trees::CheckedConditionalReturnArm::EvaluateUnit {
                                    operations,
                                    ..
                                } => OrdinaryArm::EvaluateUnit { operations },
                                checked_trees::CheckedConditionalReturnArm::Crash {
                                    statement_ordinal,
                                } => OrdinaryArm::Crash { statement_ordinal },
                                checked_trees::CheckedConditionalReturnArm::Unit { .. } => {
                                    OrdinaryArm::Unit
                                }
                                checked_trees::CheckedConditionalReturnArm::Structural(
                                    operation,
                                ) => OrdinaryArm::Structural(operation),
                                checked_trees::CheckedConditionalReturnArm::Scalar {
                                    statement_ordinal,
                                    ..
                                } => OrdinaryArm::Scalar { statement_ordinal },
                                checked_trees::CheckedConditionalReturnArm::Call {
                                    operands,
                                    operation,
                                    completions,
                                } => OrdinaryArm::Call {
                                    operands,
                                    operation,
                                    completions,
                                },
                            };
                            let checked = self.checked;
                            let plan = self.plan;
                            let state = &plan.states[position];
                            let owner = self.admitted.authored[position].0.symbol;
                            let mut sequence = sequence.branch();
                            let target = block_id(allocate_dense(next_block)?);
                            let retained_bindings = operations.structural_values.len();
                            let mut arm_evaluation = evaluation.branch(target, operations.len());
                            establish_case_payloads(
                                &mut arm_evaluation.structural_fields,
                                established,
                            );
                            arm_evaluation.parameters = payload_parameters.to_vec();
                            let mut arm_values = arm_namespace;
                            let copied_values =
                                if let Some(statement) = return_arm.statement_ordinal() {
                                    self.capture_dispatch_copies(
                                        identities,
                                        position,
                                        statement,
                                        state_parameters,
                                        &mut arm_evaluation,
                                        &mut arm_values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?
                                } else {
                                    Vec::new()
                                };
                            let decided = operations[evaluation.operation_start..]
                                .iter()
                                .filter_map(|operation| {
                                    operation.result.scalar().map(|value| value.id)
                                })
                                .collect::<std::collections::BTreeSet<_>>();
                            operations
                                .storage_observations
                                .bytes
                                .retain(|(_, value)| decided.contains(value));
                            operations
                                .storage_observations
                                .elements
                                .retain(|(_, value)| decided.contains(value));
                            operations.storage_observations.field_bytes.clear();
                            operations.storage_observations.cases.clear();
                            let completion = match ordinary {
                                OrdinaryArm::EvaluateNoNormal {
                                    operations: selected,
                                    ..
                                }
                                | OrdinaryArm::EvaluateUnit {
                                    operations: selected,
                                    ..
                                } => {
                                    let planned = selected.iter().cloned().collect::<Vec<_>>();
                                    super::super::super::emission::emit_call_operations(
                                        identities,
                                        checked,
                                        owner,
                                        state,
                                        &planned,
                                        self.catalogs,
                                        state_parameters,
                                        &mut arm_evaluation,
                                        &mut arm_values,
                                        &self.state_erased[position],
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?;
                                    let terminator = if let Some(terminator) =
                                        operations.take_invocation_completion(next_edge)?
                                    {
                                        terminator
                                    } else if matches!(ordinary, OrdinaryArm::EvaluateUnit { .. }) {
                                        Terminator::ReturnUnit {
                                            edge: edge_id(allocate_dense(next_edge)?),
                                            cleanup_actions: Vec::new(),
                                        }
                                    } else {
                                        return unsupported(
                                            "stopped return operand lost its exact invocation",
                                        );
                                    };
                                    PreparedArmCompletion::Edge(terminator)
                                }
                                OrdinaryArm::Crash { statement_ordinal } => {
                                    PreparedArmCompletion::Edge(self.crash_completion(
                                        identities,
                                        position,
                                        *statement_ordinal,
                                        next_edge,
                                    )?)
                                }
                                OrdinaryArm::Unit => {
                                    PreparedArmCompletion::Edge(Terminator::ReturnUnit {
                                        edge: edge_id(allocate_dense(next_edge)?),
                                        cleanup_actions: Vec::new(),
                                    })
                                }
                                OrdinaryArm::Structural(operation) => {
                                    super::super::super::emission::emit_call_operations(
                                        identities,
                                        checked,
                                        owner,
                                        state,
                                        std::slice::from_ref(operation),
                                        self.catalogs,
                                        state_parameters,
                                        &mut arm_evaluation,
                                        &mut arm_values,
                                        &self.state_erased[position],
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?;
                                    if let Some(terminator) =
                                        operations.take_invocation_completion(next_edge)?
                                    {
                                        PreparedArmCompletion::Edge(terminator)
                                    } else {
                                        PreparedArmCompletion::Structural(
                                            super::super::guarded::return_binding(
                                                operation, operations,
                                            )?,
                                        )
                                    }
                                }
                                // The selected arm evaluates its operands and invokes its call
                                // once. Ordinary completion then forwards the exact result the
                                // call bound, with cleanup appropriate to that result's custody.
                                OrdinaryArm::Call {
                                    operands,
                                    operation,
                                    completions,
                                } => {
                                    let planned = operands
                                        .iter()
                                        .chain(std::iter::once(operation))
                                        .chain(completions.iter())
                                        .cloned()
                                        .collect::<Vec<_>>();
                                    super::super::super::emission::emit_call_operations(
                                        identities,
                                        checked,
                                        owner,
                                        state,
                                        &planned,
                                        self.catalogs,
                                        state_parameters,
                                        &mut arm_evaluation,
                                        &mut arm_values,
                                        &self.state_erased[position],
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                        &mut sequence,
                                    )?;

                                    if let Some(terminator) =
                                        operations.take_invocation_completion(next_edge)?
                                    {
                                        PreparedArmCompletion::Edge(terminator)
                                    } else {
                                        let result = operation
                                            .call()
                                            .ok_or(LoweringError::Unsupported(
                                                "Unit graph call arm lost its call result",
                                            ))?
                                            .result();
                                        match result {
                                            checked_trees::CheckedUnitCallResult::NoNormal => {
                                                return unsupported(
                                                    "nonreturning call arm lost its pending invocation",
                                                );
                                            }
                                            checked_trees::CheckedUnitCallResult::Structural {
                                                ..
                                            } => PreparedArmCompletion::Structural(
                                                super::super::guarded::return_binding(
                                                    operation, operations,
                                                )?,
                                            ),
                                            checked_trees::CheckedUnitCallResult::Unit => {
                                                PreparedArmCompletion::Edge(
                                                    Terminator::ReturnUnit {
                                                        edge: edge_id(allocate_dense(next_edge)?),
                                                        cleanup_actions: Vec::new(),
                                                    },
                                                )
                                            }
                                            checked_trees::CheckedUnitCallResult::Scalar(
                                                binding,
                                            ) => {
                                                let value = arm_call_result(
                                                    binding,
                                                    state.scalar_parameters.len(),
                                                    &arm_evaluation,
                                                    &arm_values,
                                                )?;
                                                PreparedArmCompletion::Edge(Terminator::Return {
                                                    edge: edge_id(allocate_dense(next_edge)?),
                                                    value: value.id,
                                                    cleanup_actions: Vec::new(),
                                                })
                                            }
                                        }
                                    }
                                }
                                // The arm alone evaluates the value checking retained under its
                                // `Return` role, then disposes the roots a scalar return does.
                                OrdinaryArm::Scalar {
                                    statement_ordinal, ..
                                } => {
                                    let role = CheckedScalarExpressionRole::Return;
                                    let retained = match checked.facts.values.scalar_expressions.expression_at(
                        state.state,
                        *statement_ordinal,
                        role,
                    ) {
                        Some(expression) => {
                            checked_trees::CheckedCallScalarArgument::Pure(expression.clone())
                        }
                        None => checked_trees::CheckedCallScalarArgument::Computation(
                            value_facts(checked)
                                .scalar_computations
                                .root_at(state.state, *statement_ordinal, role)
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph scalar return arm lost its retained value",
                                ))?
                                .root,
                        ),
                    };
                                    let value = StateFrame::new(
                                        checked,
                                        owner,
                                        state,
                                        &self.state_erased[position],
                                    )
                                    .evaluate(
                                        self.catalogs,
                                        state_parameters,
                                        &mut sequence,
                                        FrameValues {
                                            source_value_count: arm_values.len(),
                                            evaluation: &mut arm_evaluation,
                                            values: &mut arm_values,
                                            next_value,
                                            next_block,
                                            next_edge,
                                            operations,
                                        },
                                        |mut frame| {
                                            frame.source_value(
                                                identities,
                                                *statement_ordinal,
                                                role,
                                                &retained,
                                            )
                                        },
                                    )?;
                                    PreparedArmCompletion::Edge(Terminator::Return {
                                        edge: edge_id(allocate_dense(next_edge)?),
                                        value: value.id,
                                        cleanup_actions: Vec::new(),
                                    })
                                }
                            };
                            if !matches!(
                                &completion,
                                PreparedArmCompletion::Edge(Terminator::Crash { .. })
                            ) {
                                sequence.require_closed()?;
                            }
                            let terminator = match completion {
                                PreparedArmCompletion::Edge(
                                    terminator @ Terminator::InvokeNoNormal { .. },
                                ) => terminator,
                                PreparedArmCompletion::Edge(
                                    terminator @ (Terminator::Crash { .. }
                                    | Terminator::ExternalCompletion { .. }),
                                ) => terminator,
                                completion => {
                                    let statement_ordinal = return_arm.statement_ordinal().ok_or(
                                        LoweringError::Unsupported(
                                            "return arm lost its source coordinate",
                                        ),
                                    )?;
                                    let exit = super::super::cleanup::coordinate(
                                        self.catalogs.cleanup_control,
                                        state,
                                        statement_ordinal as usize,
                                        false,
                                    )?;
                                    if let PreparedArmCompletion::Edge(Terminator::Return {
                                        edge,
                                        ..
                                    }) = &completion
                                    {
                                        operations.source_returns.push(SourceScalarReturn {
                                            edge: *edge,
                                            exits: vec![exit],
                                        });
                                    }
                                    let mut regions = super::super::cleanup::for_exit_regions(
                                        identities,
                                        self.claims.published(),
                                        checked,
                                        state,
                                        exit,
                                        self.catalogs,
                                        state_parameters,
                                        &arm_evaluation,
                                        operations,
                                    )?;
                                    super::super::cleanup::selection(
                                        state.state,
                                        &state.operations,
                                        &arm_evaluation,
                                        operations,
                                        &mut regions,
                                    )?;
                                    let temporary_discards = dispatch_discards
                                        .iter()
                                        .copied()
                                        .filter(|place| {
                                            !regions.iter().any(|region| {
                                                region
                                                    .action
                                                    .physical()
                                                    .is_some_and(|action| action.place() == *place)
                                            })
                                        })
                                        .map(|place| {
                                            crate::unit::unit_cleanup::CleanupRegion::unconditional(
                                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
                            )
                                        })
                                        .collect::<Vec<_>>();
                                    regions.splice(0..0, temporary_discards);
                                    let copies = self.complete_dispatch_copies(
                                        &copied_values,
                                        &arm_evaluation,
                                        operations,
                                    )?;
                                    regions.splice(0..0, copies);
                                    match completion {
                                        PreparedArmCompletion::Structural(binding) => {
                                            super::super::guarded::emit_return_binding(
                                                identities,
                                                checked,
                                                binding,
                                                regions,
                                                self.catalogs,
                                                &mut arm_evaluation,
                                                &mut arm_values,
                                                next_value,
                                                next_block,
                                                next_edge,
                                                operations,
                                            )?
                                        }
                                        PreparedArmCompletion::Edge(mut terminator) => {
                                            let completed = arm_evaluation.emit_cleanup_regions(
                                                identities,
                                                checked,
                                                regions,
                                                &mut arm_values,
                                                next_value,
                                                next_block,
                                                next_edge,
                                                operations,
                                            )?;
                                            match &mut terminator {
                                                terminal_psi::terminator_return_pair_pattern!(
                                                    cleanup_actions,
                                                    ..
                                                ) => {
                                                    *cleanup_actions = completed;
                                                }
                                                _ => {
                                                    return unsupported(
                                                        "return arm did not produce a completion terminator",
                                                    );
                                                }
                                            }
                                            terminator
                                        }
                                    }
                                }
                            };

                            arm_evaluation.blocks.push(Block {
                                id: arm_evaluation.current,
                                parameters: arm_evaluation.parameters,
                                erased_scalar_formals: Vec::new(),
                                erased_proof_formals: Vec::new(),
                                structural_parameters: arm_evaluation.block_structural_parameters,
                                operations: operations[arm_evaluation.operation_start..].to_vec(),
                                terminator,
                            });
                            evaluation.blocks.extend(arm_evaluation.blocks);
                            operations.structural_values.truncate(retained_bindings);
                            operations.storage_observations.clear();
                            Ok(value_successor(edge_id(allocate_dense(next_edge)?), target))
                        }
                    }

                    fn arm_call_result(
                        binding: &checked_trees::CheckedUnitScalarResultBindingPlan,
                        scalar_parameter_count: usize,
                        evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
                        values: &[ValueDeclaration],
                    ) -> Result<ValueDeclaration, LoweringError> {
                        let slot = usize::try_from(binding.binding_ordinal)
                            .ok()
                            .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph call arm result ordinal overflows",
                            ))?;
                        let position = evaluation.scalar_bindings.immutable_position(slot)?;
                        let value =
                            values
                                .get(position)
                                .copied()
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph call arm result binding is absent",
                                ))?;
                        if position + 1 != values.len()
                            || value.scalar_type != terminal_scalar_type(binding.primitive_type)?
                        {
                            return unsupported(
                                "Unit graph call arm result is not its call's binding",
                            );
                        }
                        Ok(value)
                    }
                }
                pub(crate) mod dispatch_copies {
                    //! Materialize the selected arm's checked payload values before its operands.
                    //! They use the ordinary copied-value producer and expression-root namespace;
                    //! no live storage read may substitute the former source after sibling effects.

                    use super::super::super::{CheckedTrees, catalogs};
                    use super::super::CheckedComposedUnitControlStatePlan;
                    use super::StateGraphEmission;
                    use crate::emission::operation_emission::buffer::OperationBuffer;
                    use crate::shared::lowering_error::{LoweringError, unsupported};
                    use crate::unit::attached_unit::argument_evaluation::Evaluation;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, StateFrame,
                    };
                    use terminal_psi::{StructuralParameterDeclaration, ValueDeclaration};

                    /// One private copy born on this selected target. Its exact value handle
                    /// distinguishes the saved owner from every authored by-value read.
                    pub(crate) struct CompletedCopy {
                        binding: crate::emission::operation_emission::buffer::ComputedValueBinding,
                    }

                    /// Selected edges share one payload producer, after their decision and before
                    /// their operands. The exact outgoing target keeps sibling captures separate.
                    pub(in crate::unit_composed_control::attached_unit::composed_control::state_graph)
                    struct DispatchCopies
                    <'a, 'c> {
                        checked: &'a CheckedTrees,
                        state: &'a CheckedComposedUnitControlStatePlan,
                        owner: symbols::SymbolHandle,
                        catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
                        erased_parameters: &'a [ValueDeclaration],
                    }

                    impl<'a, 'c> DispatchCopies<'a, 'c> {
                        pub(in crate::unit_composed_control::attached_unit::composed_control::state_graph) fn new(
                            checked: &'a CheckedTrees,
                            state: &'a CheckedComposedUnitControlStatePlan,
                            owner: symbols::SymbolHandle,
                            catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
                            erased_parameters: &'a [ValueDeclaration],
                        ) -> Self {
                            Self {
                                checked,
                                state,
                                owner,
                                catalogs,
                                erased_parameters,
                            }
                        }

                        pub(in crate::unit_composed_control::attached_unit::composed_control::state_graph) fn capture(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            copied_payloads: &[checked_trees::CheckedSelectedPayloadCopy],
                            parameters: &[StructuralParameterDeclaration],
                            frame: FrameValues<'_>,
                            sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
                        ) -> Result<Vec<CompletedCopy>, LoweringError> {
                            let checked = self.checked;
                            let state = self.state;
                            let owner = self.owner;
                            let FrameValues {
                                evaluation,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                                ..
                            } = frame;
                            let copies = crate::scalar_lowering::branch_states::structural_values::selected_copies(
                identities,
                checked,
                copied_payloads,
                &self.catalogs.structural_types,
            )?;
                            let mut completed = Vec::new();
                            for copy in copies {
                                let structural_type = copy.structural_type;
                                StateFrame::new(checked, owner, state, self.erased_parameters)
                                    .evaluate(
                                        self.catalogs,
                                        parameters,
                                        sequence,
                                        FrameValues {
                                            source_value_count: values.len(),
                                            evaluation,
                                            values,
                                            next_value,
                                            next_block,
                                            next_edge,
                                            operations,
                                        },
                                        |frame| {
                                            let (mut environment, emission) = frame.split();
                                            environment.produce_value(
                                                identities,
                                                copy.value,
                                                structural_type,
                                                emission.evaluation,
                                                emission.values,
                                                emission.next_value,
                                                emission.next_block,
                                                emission.next_edge,
                                                emission.calls,
                                                emission.operations,
                                            )
                                        },
                                    )?;
                                let binding = evaluation
                                    .saved_value_binding(copy.value, operations)?
                                    .clone();
                                if binding.structural_type != structural_type {
                                    return unsupported(
                                        "selected copy has a different realized carrier",
                                    );
                                }
                                completed.push(CompletedCopy { binding });
                            }
                            Ok(completed)
                        }
                    }

                    /// Copy and DescriptorCleanup authorize only this private value's death.
                    pub(in crate::unit_composed_control::attached_unit::composed_control::state_graph) fn complete_dispatch_copies(
                        copies: &[CompletedCopy],
                        evaluation: &Evaluation,
                        operations: &OperationBuffer,
                        types: &[terminal_psi::StructuralTypeDeclaration],
                    ) -> Result<Vec<crate::unit::unit_cleanup::CleanupRegion>, LoweringError>
                    {
                        let mut regions = Vec::new();
                        for copy in copies.iter().rev() {
                            // Ordinary cleanup tracks the physical carrier's multiplicity and
                            // selected residuals. Source Copy never grants disposal of another
                            // root, a nominal hook, or a borrowed pointee.
                            regions.extend(evaluation.normal_discard_regions(
                                copy.binding.argument.place,
                                types,
                                operations,
                            )?);
                        }
                        Ok(regions)
                    }

                    impl StateGraphEmission<'_, '_> {
                        #[allow(clippy::too_many_arguments)]
                        pub(crate) fn capture_dispatch_copies(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            statement_ordinal: u32,
                            parameters: &[StructuralParameterDeclaration],
                            evaluation: &mut Evaluation,
                            values: &mut Vec<ValueDeclaration>,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
                        ) -> Result<Vec<CompletedCopy>, LoweringError> {
                            let state = &self.plan.states[position];
                            let Some(transition) = self
                                .checked
                                .facts
                                .executable_bindings
                                .transition(state.state, statement_ordinal as usize)
                            else {
                                return Ok(Vec::new());
                            };
                            let key = (
                                statement_ordinal,
                                transition.primary.arena_index(),
                                transition.primary.generation(),
                            );
                            let selected = state
                                .selected_copies
                                .binary_search_by_key(&key, |row| {
                                    (
                                        row.statement_ordinal,
                                        row.transition_target.arena_index(),
                                        row.transition_target.generation(),
                                    )
                                })
                                .ok()
                                .and_then(|index| state.selected_copies.get(index))
                                .ok_or(LoweringError::Unsupported(
                                    "selected edge lost its checked payload births",
                                ))?;
                            DispatchCopies::new(
                                self.checked,
                                state,
                                self.admitted.authored[position].0.symbol,
                                self.catalogs,
                                &self.state_erased[position],
                            )
                            .capture(
                                identities,
                                &selected.values,
                                parameters,
                                FrameValues {
                                    source_value_count: values.len(),
                                    evaluation,
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                },
                                sequence,
                            )
                        }

                        pub(crate) fn complete_dispatch_copies(
                            &self,
                            copies: &[CompletedCopy],
                            evaluation: &Evaluation,
                            operations: &OperationBuffer,
                        ) -> Result<Vec<crate::unit::unit_cleanup::CleanupRegion>, LoweringError>
                        {
                            complete_dispatch_copies(
                                copies,
                                evaluation,
                                operations,
                                &self.catalogs.structural_types,
                            )
                        }
                    }
                }
                pub(crate) mod entry_rank {

                    use super::{
                        Block, LoweringError, StateGraphEmission, Terminator, ValueDeclaration,
                        ranking, unsupported,
                    };
                    use crate::emission::operation_emission::buffer::OperationBuffer;
                    use crate::unit::attached_unit::argument_evaluation::Evaluation;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, OperationSequence, StateFrame,
                    };

                    impl StateGraphEmission<'_, '_> {
                        #[allow(clippy::too_many_arguments)]
                        pub(crate) fn initialize_invocation_rank(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            roots: &[&checked_trees::CheckedScalarComputationRoot],
                            template: &Evaluation,
                            receiving: &[ValueDeclaration],
                            body_operations: &mut OperationBuffer,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                        ) -> Result<(), LoweringError> {
                            let entry = self.invocation_entry.ok_or(LoweringError::Unsupported(
                                "ranked invocation has no one-shot observation entry",
                            ))?;
                            let checked = self.checked;
                            let state = &self.plan.states[0];
                            let owner = self.admitted.authored[0].0.symbol;
                            let mut evaluation = template.branch(entry, 0);
                            evaluation.entry = entry;
                            evaluation.parameter_storage.clear();
                            evaluation.previous_parameter_homes.clear();
                            evaluation.enclosing_structural_arrivals.clear();
                            evaluation.erased_scalar_formals = self.invocation_erased.clone();
                            let parameters = state
                                .structural_parameters
                                .iter()
                                .zip(&self.parameters)
                                .map(|(source, parameter)| {
                                    let mut parameter = parameter.clone();
                                    parameter.position = source.position;
                                    parameter
                                })
                                .collect::<Vec<_>>();
                            evaluation.structural_parameters = state
                                .structural_parameters
                                .iter()
                                .zip(&parameters)
                                .map(|(source, parameter)| (source.position, parameter.clone()))
                                .collect();
                            evaluation.current_claims = Default::default();
                            evaluation.current_claims.ingress(
                                parameters
                                    .iter()
                                    .filter(|parameter| {
                                        parameter.access == terminal_psi::StructuralAccess::Owned
                                    })
                                    .map(|parameter| parameter.place),
                                &self.claims.entry_claims,
                            )?;
                            evaluation.primitive_storage =
                                crate::unit::attached_unit::parameters::primitive_reference_storage(
                                    checked,
                                    state.state,
                                    &state.structural_parameters,
                                    &evaluation.structural_parameters,
                                )?;
                            let mut values = self.scalar_parameters.clone();
                            evaluation.scalar_bindings =
                                crate::expression_preparation::bindings::ScalarBindings::entry(
                                    values.iter().map(|value| value.scalar_type),
                                    &evaluation.primitive_storage,
                                    &evaluation.structural_parameters,
                                    &self.catalogs.structural_types,
                                    &evaluation.place_observations,
                                );
                            evaluation.structural_fields =
                                crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                                    &evaluation.structural_parameters,
                                    &self.catalogs.structural_types,
                                );
                            evaluation.structural_cases = crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
                                &evaluation.structural_parameters, &self.catalogs.structural_types,
                            );
                            evaluation.element_views =
                                crate::expression_preparation::bindings::element_views(
                                    &evaluation.structural_parameters,
                                    &self.catalogs.structural_types,
                                );
                            let scalar_slots = values
                                .iter()
                                .enumerate()
                                .map(|(position, value)| (position, value.scalar_type))
                                .collect::<Vec<_>>();
                            let structural_actuals = parameters
                                .iter()
                                .map(|parameter| {
                                    (
                                        terminal_psi::StructuralArgument {
                                            place: parameter.place,
                                            path: Vec::new(),
                                            access: parameter.access,
                                        },
                                        parameter.structural_type,
                                    )
                                })
                                .collect::<Vec<_>>();
                            let bindings = ranking::formal_bindings(
                                checked,
                                state,
                                &evaluation.scalar_bindings,
                                &scalar_slots,
                                &structural_actuals,
                            )?;
                            let mut operations = OperationBuffer::new(
                                body_operations.next_identity - 1,
                                self.catalogs.next_place,
                                std::sync::Arc::clone(&body_operations.reference_formations),
                                std::sync::Arc::clone(&self.dynamic_dispatch),
                            );
                            let mut sequence = OperationSequence::new(&evaluation);
                            let observed =
                                StateFrame::new(checked, owner, state, &self.invocation_erased)
                                    .evaluate(
                                        self.catalogs,
                                        &parameters,
                                        &mut sequence,
                                        FrameValues {
                                            source_value_count: values.len(),
                                            evaluation: &mut evaluation,
                                            values: &mut values,
                                            next_value,
                                            next_block,
                                            next_edge,
                                            operations: &mut operations,
                                        },
                                        |frame| {
                                            let (mut environment, emission) = frame.split();
                                            roots
                                                .iter()
                                                .map(|root| {
                                                    emission.evaluation.retained_observation(
                                                        identities,
                                                        checked,
                                                        root,
                                                        &bindings,
                                                        emission.values,
                                                        emission.next_value,
                                                        emission.next_block,
                                                        emission.next_edge,
                                                        emission.operations,
                                                        emission.calls,
                                                        &mut environment,
                                                    )
                                                })
                                                .collect::<Result<Vec<_>, LoweringError>>()
                                        },
                                    )?;
                            sequence.require_closed()?;
                            if observed.len() != receiving.len()
                                || !observed.iter().zip(receiving).all(|(observed, receiving)| {
                                    observed.value_type() == receiving.value_type()
                                })
                                || operations.invocation_completion.is_some()
                            {
                                return unsupported(
                                    "invocation rank lost its complete normal value",
                                );
                            }
                            let bridge = self
                                .blocks
                                .iter()
                                .position(|block| block.id == entry)
                                .ok_or(LoweringError::Unsupported(
                                    "invocation observation lost its original bridge",
                                ))?;
                            let mut bridge = self.blocks.remove(bridge);
                            let Terminator::Jump { arguments, .. } = &mut bridge.terminator else {
                                return unsupported(
                                    "invocation observation bridge is not a completed arrival",
                                );
                            };
                            arguments.extend(observed.iter().map(|value| value.id));
                            evaluation.blocks.push(Block {
                                id: evaluation.current,
                                parameters: std::mem::take(&mut evaluation.parameters),
                                structural_parameters: std::mem::take(
                                    &mut evaluation.block_structural_parameters,
                                ),
                                erased_scalar_formals: Vec::new(),
                                erased_proof_formals: bridge.erased_proof_formals,
                                operations: operations[evaluation.operation_start..].to_vec(),
                                terminator: bridge.terminator,
                            });
                            self.blocks.extend(evaluation.blocks);
                            self.qualification_coercions
                                .append(&mut evaluation.qualification_coercions);
                            self.scalar_block_invariants
                                .append(&mut operations.scalar_block_invariants);
                            self.loan_completions
                                .append(&mut operations.loan_completions);
                            self.block_entry_claims
                                .append(&mut operations.block_entry_claims);
                            body_operations.next_identity = operations.next_identity;
                            self.occurrences.retain(operations);
                            Ok(())
                        }
                    }
                }
                pub(crate) mod guarded_chain {

                    use super::super::super::super::super::{
                        BlockId, StructuralParameterDeclaration, block_id,
                    };
                    use super::super::super::super::{Operation, ValueDeclaration, allocate_dense};
                    use super::super::super::LoweringError;
                    use super::StateGraphEmission;
                    use super::state::{
                        ChainGuard, guarded_arm_may_read_payloads, plan_short_circuit_guard,
                    };
                    use crate::emission::operation_emission::buffer::OperationBuffer;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, StateFrame,
                    };

                    pub(crate) type GuardedDraft = (
                        BlockId,
                        Vec<ValueDeclaration>,
                        Vec<StructuralParameterDeclaration>,
                        Vec<Operation>,
                        ChainGuard,
                        Vec<ValueDeclaration>,
                    );

                    impl StateGraphEmission<'_, '_> {
                        pub(crate) fn stage_guarded_chain(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            arms: &[checked_trees::CheckedGuardedJumpArmPlan],
                            values: &mut Vec<ValueDeclaration>,
                            evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                            next_value: &mut u64,
                            next_block: &mut u64,
                            next_edge: &mut u64,
                            operations: &mut OperationBuffer,
                            state_parameters: &[StructuralParameterDeclaration],
                            sequence: &crate::unit::attached_unit::composed_control::emission::OperationSequence,
                        ) -> Result<
                            (Vec<BlockId>, Vec<GuardedDraft>, Vec<ValueDeclaration>),
                            LoweringError,
                        > {
                            let checked = self.checked;
                            let plan = self.plan;
                            let state = &plan.states[position];
                            let owner = self.admitted.authored[position].0.symbol;
                            let mut guarded_decisions = Vec::new();
                            let mut guarded_drafts = Vec::new();
                            let guarded_first_namespace = values.clone();
                            for _ in 1..arms.len() {
                                guarded_decisions.push(block_id(allocate_dense(next_block)?));
                            }
                            let (resume_current, resume_start, entry_parameters, entry_structural) = (
                                evaluation.current,
                                evaluation.operation_start,
                                std::mem::take(&mut evaluation.parameters),
                                std::mem::take(&mut evaluation.block_structural_parameters),
                            );
                            for (index, arm) in arms.iter().enumerate().skip(1) {
                                let decision = guarded_decisions[index - 1];
                                let mut sequence = sequence.branch();
                                evaluation.current = decision;
                                evaluation.operation_start = operations.len();
                                evaluation.parameters = Vec::new();
                                evaluation.block_structural_parameters = Vec::new();
                                let reads_payloads = guarded_arm_may_read_payloads(arm);
                                let guard = if let Some(expression) = evaluation.branch_guard(
                                    identities,
                                    checked,
                                    state.state,
                                    arm.statement_ordinal(),
                                    values,
                                    reads_payloads,
                                )? {
                                    ChainGuard::Decision(plan_short_circuit_guard(
                                        &expression,
                                        values,
                                        &self.catalogs.structural_types,
                                        &evaluation.structural_parameters,
                                        evaluation.shared_qualifications.catalog(),
                                        next_value,
                                        reads_payloads,
                                    )?)
                                } else {
                                    let guard = StateFrame::new(
                                        checked,
                                        owner,
                                        state,
                                        &self.state_erased[position],
                                    )
                                    .evaluate(
                                        self.catalogs,
                                        state_parameters,
                                        &mut sequence,
                                        FrameValues {
                                            source_value_count: values.len(),
                                            evaluation,
                                            values,
                                            next_value,
                                            next_block,
                                            next_edge,
                                            operations,
                                        },
                                        |mut frame| {
                                            frame.guard_value(identities, arm.statement_ordinal())
                                        },
                                    )?;
                                    ChainGuard::Value(guard.id)
                                };
                                sequence.finish_state(checked, state)?;
                                let expanded = evaluation.current != decision;
                                guarded_drafts.push((
                                    evaluation.current,
                                    if expanded {
                                        std::mem::take(&mut evaluation.parameters)
                                    } else {
                                        Vec::new()
                                    },
                                    if expanded {
                                        std::mem::take(&mut evaluation.block_structural_parameters)
                                    } else {
                                        Vec::new()
                                    },
                                    operations[evaluation.operation_start..].to_vec(),
                                    guard,
                                    values.clone(),
                                ));
                            }
                            evaluation.current = resume_current;
                            evaluation.operation_start = resume_start;
                            evaluation.parameters = entry_parameters;
                            evaluation.block_structural_parameters = entry_structural;
                            Ok((guarded_decisions, guarded_drafts, guarded_first_namespace))
                        }
                    }
                }
                pub(crate) mod loan_chains {

                    use crate::LoweringError;
                    use checked_trees::{
                        CheckedComposedUnitControlMachinePlan, CheckedStructuralAccess,
                        CheckedStructuralControlTransferSourcePlan,
                        CheckedUnitStructuralArgumentSourcePlan,
                    };
                    use language_semantics::Multiplicity;
                    use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind};
                    use std::collections::{BTreeMap, BTreeSet};
                    use terminal_psi::{
                        StructuralAccess, StructuralArgument, StructuralMultiplicity,
                        StructuralParameterDeclaration, TerminalAffineCleanupAction,
                        TerminalMachine, Terminator,
                    };

                    pub(crate) mod retained_homes {
                        use super::{
                            ChainBacking, ChainExit, DisposalPosition, Forwarding, LoanChain,
                            end_on_lender_exits, private_region,
                        };
                        use crate::LoweringError;
                        use semantic_vocabulary::{BlockId, OperationId, PlaceId};
                        use std::collections::{BTreeMap, BTreeSet};
                        use terminal_psi::{
                            StructuralMultiplicity, StructuralTypeDeclaration,
                            TerminalAffineCleanupAction, TerminalMachine,
                        };

                        #[derive(Clone, Default)]
                        struct Lifetime {
                            members: BTreeSet<BlockId>,
                            carriers: BTreeSet<PlaceId>,
                        }

                        impl Lifetime {
                            fn include(&mut self, other: &Self) -> bool {
                                let previous = (self.members.len(), self.carriers.len());
                                self.members.extend(&other.members);
                                self.carriers.extend(&other.carriers);
                                previous != (self.members.len(), self.carriers.len())
                            }
                        }

                        struct Home {
                            place: PlaceId,
                            lender: BlockId,
                            lifetime: Lifetime,
                            implicit_carrier: bool,
                        }

                        #[allow(clippy::too_many_arguments)]
                        pub(crate) fn finish(
                            machine: &mut TerminalMachine,
                            chains: &[LoanChain],
                            states: &[BlockId],
                            headers: &BTreeSet<BlockId>,
                            captures: &super::super::captured_loans::CapturedLoans,
                            types: &[StructuralTypeDeclaration],
                            entry_homes: &[PlaceId],
                            parents: &[(PlaceId, PlaceId)],
                            producers: &BTreeMap<PlaceId, OperationId>,
                            parameters: &[PlaceId],
                            next_obligation: &mut u64,
                        ) -> Result<(), LoweringError> {
                            let owned_homes = machine
                                .structural_parameters
                                .iter()
                                .chain(
                                    machine
                                        .blocks
                                        .iter()
                                        .flat_map(|block| &block.structural_parameters),
                                )
                                .filter(|parameter| {
                                    parameter.access == terminal_psi::StructuralAccess::Owned
                                        && parameter.multiplicity
                                            != StructuralMultiplicity::Unrestricted
                                })
                                .map(|parameter| parameter.place)
                                .chain(
                                    machine
                                        .blocks
                                        .iter()
                                        .flat_map(|block| &block.operations)
                                        .filter_map(|operation| match &operation.result {
                                            terminal_psi::OperationResult::Structural(result)
                                                if result.multiplicity
                                                    != StructuralMultiplicity::Unrestricted =>
                                            {
                                                Some(result.place)
                                            }
                                            _ => None,
                                        }),
                                )
                                .collect::<BTreeSet<_>>();
                            let mut parents = parents.to_vec();
                            for operation in
                                machine.blocks.iter().flat_map(|block| &block.operations)
                            {
                                if let terminal_psi::OperationKind::EstablishReference {
                                    source,
                                    ..
                                } = &operation.kind
                                    && owned_homes.contains(&source.place)
                                    && !source.path.iter().any(|segment| {
                                        matches!(
                                            segment,
                                            terminal_psi::StructuralPathSegment::Referent
                                        )
                                    })
                                    && let terminal_psi::OperationResult::Structural(result) =
                                        &operation.result
                                    && result.place != source.place
                                    && !parents.contains(&(result.place, source.place))
                                {
                                    parents.push((result.place, source.place));
                                }
                            }
                            let parents = parents.as_slice();
                            let availability =
                                super::super::loaned_local_homes::HomeAvailability::new(machine)?;
                            let mut homes: Vec<Home> = Vec::new();
                            let mut supplied = carrier_lifetimes(machine, headers, parents)?;
                            for chain in chains {
                                let mut lifetime = Lifetime::default();
                                for &(position, dense, parameter) in &chain.members {
                                    lifetime.members.insert(states[position]);
                                    lifetime.carriers.insert(parameter);
                                    if let Some(carrier) = captures.parent_carrier(position, dense)
                                    {
                                        lifetime.carriers.insert(carrier.place);
                                    }
                                }
                                supplied.extend(
                                    chain
                                        .source_homes
                                        .iter()
                                        .map(|source| (*source, lifetime.clone())),
                                );
                                if chain.multiplicity != StructuralMultiplicity::Affine {
                                    continue;
                                }
                                if let ChainBacking::LenderHome { home, lender } = chain.backing {
                                    let home = home.ok_or(LoweringError::Unsupported(
                                        "state loan chain lost its retained home",
                                    ))?;
                                    if entry_homes.contains(&home) {
                                        continue;
                                    }
                                    if let Some(existing) =
                                        homes.iter_mut().find(|existing| existing.place == home)
                                    {
                                        if existing.lender != states[lender] {
                                            return Err(LoweringError::Unsupported(
                                                "retained home has distinct birth states",
                                            ));
                                        }
                                        existing.lifetime.include(&lifetime);
                                    } else {
                                        homes.push(Home {
                                            place: home,
                                            lender: states[lender],
                                            lifetime,
                                            implicit_carrier: true,
                                        });
                                    }
                                }
                            }
                            loop {
                                let mut changed = false;
                                for &(dependent, parent) in parents {
                                    if entry_homes.contains(&parent) {
                                        continue;
                                    }
                                    let mut lifetime = Lifetime::default();
                                    for (_, source) in
                                        supplied.iter().filter(|(source, _)| *source == dependent)
                                    {
                                        lifetime.include(source);
                                    }
                                    if let Some(source) =
                                        homes.iter().find(|home| home.place == dependent)
                                    {
                                        lifetime.include(&source.lifetime);
                                    }
                                    if lifetime.members.is_empty() {
                                        continue;
                                    }
                                    lifetime.carriers.insert(dependent);
                                    if let Some(home) =
                                        homes.iter_mut().find(|home| home.place == parent)
                                    {
                                        changed |= home.lifetime.include(&lifetime);
                                    } else {
                                        homes.push(Home {
                                            place: parent,
                                            lender: birth_header(
                                                machine,
                                                parent,
                                                headers,
                                                &availability,
                                            )?,
                                            lifetime,
                                            implicit_carrier: false,
                                        });
                                        changed = true;
                                    }
                                }
                                if !changed {
                                    break;
                                }
                            }
                            let mut remaining = (0..homes.len()).collect::<Vec<_>>();
                            while !remaining.is_empty() {
                                let ready = remaining
                                    .iter()
                                    .position(|candidate| {
                                        !parents.iter().any(|(child, parent)| {
                                            *parent == homes[*candidate].place
                                                && remaining
                                                    .iter()
                                                    .any(|other| homes[*other].place == *child)
                                        })
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "retained backing dependencies are cyclic",
                                    ))?;
                                let position = remaining.remove(ready);
                                let home = &homes[position];
                                availability.require_in_regions(
                                    machine,
                                    home.place,
                                    &home.lifetime.members,
                                )?;
                                if home.lifetime.members.contains(&home.lender) {
                                    return Err(LoweringError::Unsupported(
                                        "retained parent is reestablished inside its own loan lifetime",
                                    ));
                                }
                                let actions = super::super::loaned_local_homes::whole_home_actions(
                                    machine, home.place,
                                )?;
                                let actions = match actions {
                                    Some(actions) => actions,
                                    None if home.implicit_carrier
                                        && terminal_semantics::structural_store_value_admitted(
                                            machine,
                                            types.iter(),
                                            terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                                            home.place,
                                        ) =>
                                    {
                                        vec![TerminalAffineCleanupAction::DiscardRoot(home.place)]
                                    }
                                    None => {
                                        return Err(LoweringError::Unsupported(
                                            "retained backing lacks its checked whole-home disposition",
                                        ));
                                    }
                                };
                                let carriers =
                                    home.lifetime.carriers.iter().copied().collect::<Vec<_>>();
                                let birth =
                                    machine.blocks[availability.birth(machine, home.place)?].id;
                                let source_exits = private_region(machine, birth, headers)
                                    .into_iter()
                                    .filter(|block| {
                                        machine.block(block).is_some_and(|block| {
                                            matches!(
                                                block.terminator,
                                                terminal_psi::terminator_return_pair_pattern!(..)
                                                    | terminal_psi::Terminator::ReturnStructural { .. }
                                            ) || block
                                                .terminator
                                                .successor_targets()
                                                .iter()
                                                .any(|target| headers.contains(target))
                                        })
                                    })
                                    .collect();
                                availability.require_in_regions(
                                    machine,
                                    home.place,
                                    &source_exits,
                                )?;
                                retain_storage(
                                    machine,
                                    home.place,
                                    home.lender,
                                    &home.lifetime.members,
                                    headers,
                                );
                                end_on_lender_exits(
                                    machine,
                                    home.lender,
                                    headers,
                                    home.place,
                                    producers,
                                    &home.lifetime.members,
                                    &carriers,
                                    &actions,
                                    next_obligation,
                                )?;
                                let earlier = earlier_homes(position, &homes, producers, machine)?;
                                for header in &home.lifetime.members {
                                    let exit = ChainExit {
                                        headers,
                                        storage: home.place,
                                        cleanup: &actions,
                                        forwarded: Forwarding::IntoMember(&home.lifetime.members),
                                        dependent_carriers: &carriers,
                                        position: DisposalPosition::RetainedHome {
                                            earlier_homes: earlier.clone(),
                                            machine_parameters: parameters,
                                        },
                                    };
                                    let region = private_region(machine, *header, headers);
                                    for block in machine
                                        .blocks
                                        .iter_mut()
                                        .filter(|block| region.contains(&block.id))
                                    {
                                        exit.dispose(&mut block.terminator, next_obligation)?;
                                    }
                                }
                            }
                            Ok(())
                        }

                        fn retain_storage(
                            machine: &mut TerminalMachine,
                            home: PlaceId,
                            lender: BlockId,
                            members: &BTreeSet<BlockId>,
                            headers: &BTreeSet<BlockId>,
                        ) {
                            let mut region = private_region(machine, lender, headers);
                            for member in members {
                                region.extend(private_region(machine, *member, headers));
                            }
                            for block in machine
                                .blocks
                                .iter_mut()
                                .filter(|block| region.contains(&block.id))
                            {
                                match &mut block.terminator {
                                    terminal_psi::Terminator::Jump {
                                        cleanup_actions, ..
                                    }
                                    | terminal_psi::Terminator::ReturnStructural {
                                        cleanup_actions,
                                        ..
                                    }
                                    | terminal_psi::terminator_return_pair_pattern!(
                                        cleanup_actions,
                                        ..
                                    ) => {
                                        cleanup_actions.retain(|action| action.place() != home);
                                    }
                                    terminal_psi::Terminator::Conditional {
                                        when_true,
                                        when_false,
                                        ..
                                    } => {
                                        when_true
                                            .cleanup_actions
                                            .retain(|action| action.place() != home);
                                        when_false
                                            .cleanup_actions
                                            .retain(|action| action.place() != home);
                                    }
                                    terminal_psi::Terminator::StructuralCase { cases, .. } => {
                                        for case in cases {
                                            case.cleanup_actions
                                                .retain(|action| action.place() != home);
                                        }
                                    }
                                    terminal_psi::Terminator::InvokeNoNormal { .. }
                                    | terminal_psi::Terminator::Crash { .. }
                                    | terminal_psi::Terminator::ExternalCompletion { .. } => {}
                                }
                            }
                        }

                        fn carrier_lifetimes(
                            machine: &TerminalMachine,
                            headers: &BTreeSet<BlockId>,
                            parents: &[(PlaceId, PlaceId)],
                        ) -> Result<Vec<(PlaceId, Lifetime)>, LoweringError>
                        {
                            let mut dependent_carriers = parents
                                .iter()
                                .map(|(child, _)| *child)
                                .collect::<BTreeSet<_>>();
                            loop {
                                let mut changed = false;
                                for block in &machine.blocks {
                                    for (target, arguments) in block
                                        .terminator
                                        .successor_targets()
                                        .into_iter()
                                        .zip(block.terminator.successor_block_arguments())
                                    {
                                        let target = machine.block(&target).ok_or(
                                            LoweringError::Unsupported(
                                                "owned lifetime transport lost its target",
                                            ),
                                        )?;
                                        if target.structural_parameters.len() != arguments.len() {
                                            return Err(LoweringError::Unsupported(
                                                "owned lifetime transport has mismatched arity",
                                            ));
                                        }
                                        for (parameter, argument) in
                                            target.structural_parameters.iter().zip(arguments)
                                        {
                                            if !dependent_carriers.contains(&argument.place) {
                                                continue;
                                            }
                                            if argument.access
                                                == terminal_psi::StructuralAccess::Owned
                                                && !argument.path.is_empty()
                                            {
                                                return Err(LoweringError::Unsupported(
                                                    "retained owned transport needs an exact projected dependency frontier",
                                                ));
                                            }
                                            changed |= dependent_carriers.insert(parameter.place);
                                        }
                                    }
                                }
                                if !changed {
                                    break;
                                }
                            }
                            let mut supplied: Vec<(PlaceId, Lifetime)> = Vec::new();
                            for block in machine
                                .blocks
                                .iter()
                                .filter(|block| headers.contains(&block.id))
                            {
                                for parameter in &block.structural_parameters {
                                    if dependent_carriers.contains(&parameter.place) {
                                        supplied.push((
                                            parameter.place,
                                            Lifetime {
                                                members: BTreeSet::from([block.id]),
                                                carriers: BTreeSet::from([parameter.place]),
                                            },
                                        ));
                                    }
                                }
                            }
                            loop {
                                let mut changed = false;
                                for block in &machine.blocks {
                                    for (target, arguments) in block
                                        .terminator
                                        .successor_targets()
                                        .into_iter()
                                        .zip(block.terminator.successor_block_arguments())
                                    {
                                        let target = machine.block(&target).ok_or(
                                            LoweringError::Unsupported(
                                                "owned lifetime transport lost its target",
                                            ),
                                        )?;
                                        for (parameter, argument) in
                                            target.structural_parameters.iter().zip(arguments)
                                        {
                                            if argument.access
                                                == terminal_psi::StructuralAccess::Owned
                                                && !argument.path.is_empty()
                                            {
                                                continue;
                                            }
                                            let Some((_, lifetime)) = supplied
                                                .iter()
                                                .find(|(place, _)| *place == parameter.place)
                                            else {
                                                continue;
                                            };
                                            let lifetime = lifetime.clone();
                                            if let Some((_, current)) = supplied
                                                .iter_mut()
                                                .find(|(place, _)| *place == argument.place)
                                            {
                                                changed |= current.include(&lifetime);
                                            } else {
                                                supplied.push((argument.place, lifetime));
                                                changed = true;
                                            }
                                        }
                                    }
                                }
                                if !changed {
                                    return Ok(supplied);
                                }
                            }
                        }

                        fn birth_header(
                            machine: &TerminalMachine,
                            home: PlaceId,
                            headers: &BTreeSet<BlockId>,
                            availability: &super::super::loaned_local_homes::HomeAvailability,
                        ) -> Result<BlockId, LoweringError> {
                            let birth = machine.blocks[availability.birth(machine, home)?].id;
                            let mut owners = headers
                                .iter()
                                .copied()
                                .chain((!headers.contains(&machine.entry)).then_some(machine.entry))
                                .filter(|header| {
                                    private_region(machine, *header, headers).contains(&birth)
                                });
                            let owner = owners.next().ok_or(LoweringError::Unsupported(
                                "retained parent has no birth region",
                            ))?;
                            if owners.next().is_some() {
                                return Err(LoweringError::Unsupported(
                                    "retained parent has ambiguous birth regions",
                                ));
                            }
                            Ok(owner)
                        }

                        fn earlier_homes(
                            current: usize,
                            homes: &[Home],
                            producers: &BTreeMap<PlaceId, OperationId>,
                            machine: &TerminalMachine,
                        ) -> Result<Vec<PlaceId>, LoweringError> {
                            let mut pending = vec![current];
                            let mut visited = vec![current];
                            let mut earlier = Vec::new();
                            let birth = |place| {
                                producers
                                    .get(&place)
                                    .map(|producer| (1, producer.get()))
                                    .or_else(|| {
                                        machine
                                            .structural_parameters
                                            .iter()
                                            .chain(
                                                machine
                                                    .blocks
                                                    .iter()
                                                    .flat_map(|block| &block.structural_parameters),
                                            )
                                            .position(|parameter| parameter.place == place)
                                            .map(|position| (0, position as u64))
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "retained home has no ordered birth",
                                    ))
                            };
                            while let Some(younger) = pending.pop() {
                                for (candidate, home) in homes.iter().enumerate() {
                                    if candidate == younger {
                                        continue;
                                    }
                                    let precedes = if home.lender == homes[younger].lender {
                                        birth(home.place)? < birth(homes[younger].place)?
                                    } else {
                                        home.lifetime.members.contains(&homes[younger].lender)
                                    };
                                    if !precedes {
                                        continue;
                                    }
                                    if candidate == current {
                                        return Err(LoweringError::Unsupported(
                                            "retained homes have cyclic birth dependencies",
                                        ));
                                    }
                                    if !visited.contains(&candidate) {
                                        visited.push(candidate);
                                        pending.push(candidate);
                                        earlier.push(home.place);
                                    }
                                }
                            }
                            Ok(earlier)
                        }
                    }

                    #[derive(Debug, Default)]
                    pub(crate) struct LoanChains {
                        chains: Vec<LoanChain>,
                    }

                    #[derive(Debug)]
                    pub(crate) struct LoanChain {
                        backing: ChainBacking,
                        multiplicity: StructuralMultiplicity,
                        members: Vec<(usize, u32, PlaceId)>,
                        source_homes: Vec<PlaceId>,
                    }

                    #[derive(Debug, PartialEq, Eq)]
                    enum ChainBacking {
                        LenderHome {
                            home: Option<PlaceId>,
                            lender: usize,
                        },
                        Transferred,
                    }

                    impl LoanChain {
                        fn contains(&self, position: usize, dense: u32) -> bool {
                            self.members
                                .iter()
                                .any(|(member, index, _)| *member == position && *index == dense)
                        }
                    }

                    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                    pub(crate) enum ChainArrival {
                        Retained,
                        Transferred,
                    }

                    enum LoanSource {
                        Local {
                            binding: u32,
                            requires_stable_home: bool,
                        },
                        Completed {
                            binding: u32,
                            requires_stable_home: bool,
                        },
                        Parameter(u32),
                        ProjectedParameter(u32),
                        Foreign,
                        Other,
                    }

                    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
                    enum HomeSource {
                        Parameter(u32),
                        Local {
                            binding: u32,
                            completed: Option<(usize, usize)>,
                        },
                    }

                    struct HomeType {
                        type_identity: typed_trees::type_identity::TypeIdentityId,
                        multiplicity: Multiplicity,
                    }

                    enum Arrival {
                        Local {
                            lender: usize,
                            source: HomeSource,
                            requires_stable_home: bool,
                        },
                        Forward,
                        Projected,
                        Foreign,
                    }

                    impl LoanChains {
                        pub(crate) fn plan(
                            checked: &checked_trees::CheckedTrees,
                            plan: &CheckedComposedUnitControlMachinePlan,
                            values: &checked_trees::CheckedStructuralValuePlans,
                            live: &[bool],
                            aliased: &[BTreeMap<u32, u32>],
                            entry_reentered: bool,
                            state_views: &[Vec<StructuralParameterDeclaration>],
                            presents: &dyn Fn(
                                typed_trees::type_identity::TypeIdentityId,
                                &checked_trees::CheckedUnitStructuralParameterPlan,
                            ) -> bool,
                        ) -> Result<Self, LoweringError> {
                            let mut chains = Self::default();
                            let block_owned = |position: usize| position != 0 || entry_reentered;
                            let mut nodes = Vec::new();
                            let mut node_of = Vec::with_capacity(plan.states.len());
                            for (position, state) in plan.states.iter().enumerate() {
                                let mut dense_nodes =
                                    Vec::with_capacity(state.structural_parameters.len());
                                for (dense, parameter) in
                                    state.structural_parameters.iter().enumerate()
                                {
                                    let candidate = live.get(position).copied().unwrap_or(false)
                                        && block_owned(position)
                                        && matches!(
                                            parameter.access,
                                            checked_trees::checked_borrowed_access_pair_pattern!()
                                        )
                                        && !aliased.get(position).is_some_and(|aliased| {
                                            aliased.contains_key(&(dense as u32))
                                        });
                                    dense_nodes.push(candidate.then(|| {
                                        nodes.push((position, dense as u32));
                                        nodes.len() - 1
                                    }));
                                }
                                node_of.push(dense_nodes);
                            }
                            if nodes.is_empty() {
                                return Ok(chains);
                            }
                            let node = |position: usize, dense: u32| -> Option<usize> {
                                node_of
                                    .get(position)
                                    .and_then(|dense_nodes: &Vec<Option<usize>>| {
                                        dense_nodes.get(dense as usize)
                                    })
                                    .copied()
                                    .flatten()
                            };
                            let mut arrivals: Vec<Vec<Arrival>> =
                                (0..nodes.len()).map(|_| Vec::new()).collect();
                            let mut escapes = vec![false; nodes.len()];
                            let mut links: Vec<usize> = (0..nodes.len()).collect();
                            if entry_reentered {
                                for entry_node in node_of[0].iter().flatten() {
                                    arrivals[*entry_node].push(Arrival::Foreign);
                                }
                            }
                            let mut entry_loans: BTreeMap<u32, Vec<Option<usize>>> =
                                BTreeMap::new();
                            for (source, state) in plan.states.iter().enumerate() {
                                if !live.get(source).copied().unwrap_or(false) {
                                    continue;
                                }
                                for (edge_index, edge) in state.successors().into_iter().enumerate()
                                {
                                    let target = plan.successor_position(edge).ok_or(
                                        LoweringError::Unsupported(
                                            "Unit graph loan chain target state is absent",
                                        ),
                                    )?;
                                    for (dense, (parameter, transfer)) in plan.states[target]
                                        .structural_parameters
                                        .iter()
                                        .zip(&edge.transfers)
                                        .enumerate()
                                    {
                                        let destination = node(target, dense as u32);
                                        for source_loan in loan_sources(
                                            checked,
                                            state,
                                            edge.statement_ordinal,
                                            edge.target_state,
                                            parameter.position,
                                            transfer,
                                            &state.operations,
                                            values,
                                        ) {
                                            match source_loan? {
                                                LoanSource::Local {
                                                    binding,
                                                    requires_stable_home,
                                                } => {
                                                    if let Some(destination) = destination {
                                                        arrivals[destination].push(
                                                            Arrival::Local {
                                                                lender: source,
                                                                source: HomeSource::Local {
                                                                    binding,
                                                                    completed: None,
                                                                },
                                                                requires_stable_home,
                                                            },
                                                        );
                                                    }
                                                    if !block_owned(source)
                                                        && matches!(
                                                            parameter.access,
                                                            checked_trees::checked_borrowed_access_pair_pattern!()
                                                        )
                                                    {
                                                        entry_loans
                                                            .entry(binding)
                                                            .or_default()
                                                            .push(destination);
                                                    }
                                                }
                                                LoanSource::Completed {
                                                    binding,
                                                    requires_stable_home,
                                                } => {
                                                    if let Some(destination) = destination {
                                                        arrivals[destination].push(
                                                            Arrival::Local {
                                                                lender: source,
                                                                source: HomeSource::Local {
                                                                    binding,
                                                                    completed: Some((
                                                                        edge_index, dense,
                                                                    )),
                                                                },
                                                                requires_stable_home,
                                                            },
                                                        );
                                                    }
                                                }
                                                LoanSource::Parameter(index)
                                                | LoanSource::ProjectedParameter(index)
                                                    if plan.states[source]
                                                        .structural_parameters
                                                        .get(index as usize)
                                                        .is_some_and(|parameter| {
                                                            parameter.access
                                                                == CheckedStructuralAccess::Owned
                                                        }) =>
                                                {
                                                    if let Some(destination) = destination {
                                                        arrivals[destination].push(
                                                            Arrival::Local {
                                                                lender: source,
                                                                source: HomeSource::Parameter(
                                                                    index,
                                                                ),
                                                                requires_stable_home: true,
                                                            },
                                                        );
                                                    }
                                                }
                                                LoanSource::Parameter(index) => {
                                                    match (node(source, index), destination) {
                                                        (Some(origin), Some(destination)) => {
                                                            link(&mut links, origin, destination);
                                                            arrivals[destination]
                                                                .push(Arrival::Forward);
                                                        }
                                                        (Some(origin), None) => {
                                                            escapes[origin] = true
                                                        }
                                                        (None, Some(destination)) => {
                                                            arrivals[destination]
                                                                .push(Arrival::Foreign);
                                                        }
                                                        (None, None) => {}
                                                    }
                                                }
                                                LoanSource::ProjectedParameter(index) => {
                                                    match (node(source, index), destination) {
                                                        (Some(origin), Some(destination)) => {
                                                            link(&mut links, origin, destination);
                                                            arrivals[destination]
                                                                .push(Arrival::Projected);
                                                        }
                                                        (Some(origin), None) => {
                                                            escapes[origin] = true
                                                        }
                                                        (None, Some(destination)) => {
                                                            arrivals[destination]
                                                                .push(Arrival::Foreign);
                                                        }
                                                        (None, None) => {}
                                                    }
                                                }
                                                LoanSource::Foreign => {
                                                    if let Some(destination) = destination {
                                                        arrivals[destination]
                                                            .push(Arrival::Foreign);
                                                    }
                                                }
                                                LoanSource::Other => {
                                                    if let Some(destination) = destination {
                                                        arrivals[destination]
                                                            .push(Arrival::Foreign);
                                                    }
                                                    if !block_owned(source)
                                                        && let Some(binding) = projected_local(
                                                            &transfer.source,
                                                            &state.operations,
                                                            values,
                                                        )
                                                    {
                                                        entry_loans
                                                            .entry(binding)
                                                            .or_default()
                                                            .push(None);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            let mut home_members = Vec::new();
                            for (destination, incoming) in arrivals.iter().enumerate() {
                                for arrival in incoming {
                                    if let Arrival::Local { lender, source, .. } = arrival {
                                        let home = (*lender, *source);
                                        if let Some((_, first)) = home_members
                                            .iter()
                                            .find(|(candidate, _)| *candidate == home)
                                        {
                                            link(&mut links, *first, destination);
                                        } else {
                                            home_members.push((home, destination));
                                        }
                                    }
                                }
                            }
                            let roots = (0..nodes.len())
                                .map(|node| representative(&mut links, node))
                                .collect::<Vec<_>>();
                            let exclusive = |index: usize| {
                                let (position, dense) = nodes[index];
                                plan.states[position].structural_parameters[dense as usize].access
                                    == CheckedStructuralAccess::MutableBorrow
                            };
                            let local_result = |lender: usize, source: HomeSource| {
                                let HomeSource::Local { binding, completed } = source else {
                                    let HomeSource::Parameter(index) = source else {
                                        unreachable!()
                                    };
                                    let parameter = plan.states[lender]
                                        .structural_parameters
                                        .get(index as usize)
                                        .ok_or(LoweringError::Unsupported(
                                            "state loan home lost its owned parameter",
                                        ))?;
                                    return Ok(HomeType {
                                        type_identity: parameter.type_identity,
                                        multiplicity: parameter.multiplicity,
                                    });
                                };
                                let result = if let Some((edge, dense)) = completed {
                                    plan.states[lender]
                                        .successors()
                                        .into_iter()
                                        .nth(edge)
                                        .and_then(|edge| edge.transfers.get(dense))
                                        .and_then(|transfer| match &transfer.source {
                                            CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                                                operands,
                                                establishment,
                                                ..
                                            } => operands
                                                .iter()
                                                .chain(std::iter::once(establishment.as_ref()))
                                                .filter_map(|operation| operation.structural_result_binding())
                                                .find(|result| result.binding_ordinal == binding),
                                            _ => None,
                                        })
                                        .filter(|result| result.binding_ordinal == binding)
                                } else {
                                    let mut matching = plan.states[lender]
                                        .operations
                                        .iter()
                                        .filter_map(|operation| {
                                            operation.structural_result_binding()
                                        })
                                        .filter(|result| result.binding_ordinal == binding);
                                    let result = matching.next();
                                    if matching.next().is_some() {
                                        None
                                    } else {
                                        result
                                    }
                                };
                                result
                                    .map(|result| HomeType {
                                        type_identity: result.type_identity,
                                        multiplicity: result.multiplicity,
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "Unit graph state loan chain names no exact completed home",
                                    ))
                            };
                            let mut seeded = BTreeSet::new();
                            for (index, arrivals) in arrivals.iter().enumerate() {
                                for arrival in arrivals {
                                    if let Arrival::Local { lender, source, .. } = *arrival
                                        && (exclusive(index)
                                            || local_result(lender, source).is_ok_and(|result| {
                                                result.multiplicity == Multiplicity::Affine
                                            }))
                                    {
                                        seeded.insert(roots[index]);
                                    }
                                }
                            }
                            if seeded.is_empty() {
                                return Ok(chains);
                            }
                            let moves = |destination: &Option<usize>| {
                                destination.is_some_and(|node| seeded.contains(&roots[node]))
                            };
                            if entry_loans.values().any(|destinations| {
                                destinations.iter().any(moves) && !destinations.iter().all(moves)
                            }) {
                                return Err(LoweringError::Unsupported(
                                    "Unit graph entry local both backs a later state loan chain and retains an entry home",
                                ));
                            }
                            for root in seeded {
                                let mut members = Vec::new();
                                let mut lenders = BTreeSet::new();
                                let mut multiplicity = None;
                                let mut derived = false;
                                for (index, &(position, dense)) in nodes.iter().enumerate() {
                                    if roots[index] != root {
                                        continue;
                                    }
                                    if escapes[index] {
                                        return Err(LoweringError::Unsupported(
                                            "Unit graph state-local loan chain is re-lent beyond its exclusive members",
                                        ));
                                    }
                                    let parameter = &plan.states[position].structural_parameters
                                        [dense as usize];
                                    for arrival in &arrivals[index] {
                                        let (lender, source) = match arrival {
                                            Arrival::Local {
                                                lender,
                                                source,
                                                requires_stable_home,
                                            } => {
                                                derived |= requires_stable_home;
                                                if !requires_stable_home
                                                    && !presents(
                                                        local_result(*lender, *source)?
                                                            .type_identity,
                                                        parameter,
                                                    )
                                                {
                                                    return Err(LoweringError::Unsupported(
                                                        "Unit graph state loan chain changes its referent type",
                                                    ));
                                                }
                                                (*lender, *source)
                                            }
                                            Arrival::Forward => continue,
                                            Arrival::Projected => {
                                                derived = true;
                                                continue;
                                            }
                                            Arrival::Foreign => {
                                                return Err(LoweringError::Unsupported(
                                                    "Unit graph state loan joins a state-local loan chain with a caller-owned or projected loan",
                                                ));
                                            }
                                        };
                                        let result = local_result(lender, source)?;
                                        let home = match result.multiplicity {
                                            Multiplicity::Affine => StructuralMultiplicity::Affine,
                                            Multiplicity::Unrestricted => {
                                                StructuralMultiplicity::Unrestricted
                                            }
                                            Multiplicity::Linear => {
                                                return Err(LoweringError::Unsupported(
                                                    "Unit graph state loan chain home is linear",
                                                ));
                                            }
                                        };
                                        if *multiplicity.get_or_insert(home) != home {
                                            return Err(LoweringError::Unsupported(
                                                "Unit graph state loan chain mixes home multiplicities",
                                            ));
                                        }
                                        lenders.insert((lender, source));
                                    }
                                    let place = state_views
                                        .get(position)
                                        .and_then(|views| views.get(dense as usize))
                                        .ok_or(LoweringError::Unsupported(
                                            "Unit graph state loan chain member has no block declaration",
                                        ))?
                                        .place;
                                    members.push((position, dense, place));
                                }
                                let backing = match lenders.iter().next() {
                                    Some((lender, _))
                                        if lenders.len() == 1
                                            && !members
                                                .iter()
                                                .any(|(position, _, _)| position == lender) =>
                                    {
                                        ChainBacking::LenderHome {
                                            home: None,
                                            lender: *lender,
                                        }
                                    }
                                    _ if derived => {
                                        return Err(LoweringError::Unsupported(
                                            "Unit graph state-local loan chain is re-lent beyond its exclusive members",
                                        ));
                                    }
                                    _ => ChainBacking::Transferred,
                                };
                                chains.chains.push(LoanChain {
                                    backing,
                                    multiplicity: multiplicity.ok_or(
                                        LoweringError::Unsupported(
                                            "Unit graph state loan chain has no lender",
                                        ),
                                    )?,
                                    members,
                                    source_homes: Vec::new(),
                                });
                            }
                            Ok(chains)
                        }

                        pub(crate) fn arrival(
                            &self,
                            position: usize,
                            dense: u32,
                        ) -> Option<ChainArrival> {
                            Some(match self.member(position, dense)?.backing {
                                ChainBacking::LenderHome { .. } => ChainArrival::Retained,
                                ChainBacking::Transferred => ChainArrival::Transferred,
                            })
                        }

                        pub(crate) fn lend(
                            &mut self,
                            position: usize,
                            dense: u32,
                            home: PlaceId,
                        ) -> Result<(), LoweringError> {
                            let Some(chain) = self
                                .chains
                                .iter_mut()
                                .find(|chain| chain.contains(position, dense))
                            else {
                                return Ok(());
                            };
                            if !chain.source_homes.contains(&home) {
                                chain.source_homes.push(home);
                            }
                            if let ChainBacking::LenderHome { home: recorded, .. } =
                                &mut chain.backing
                                && *recorded.get_or_insert(home) != home
                            {
                                return Err(LoweringError::Unsupported(
                                    "Unit graph state loan chain lender lends from two homes",
                                ));
                            }
                            Ok(())
                        }

                        pub(crate) fn declare(
                            &self,
                            position: usize,
                            dense: u32,
                            declaration: &mut StructuralParameterDeclaration,
                        ) {
                            if let Some(chain) = self.member(position, dense)
                                && chain.backing == ChainBacking::Transferred
                            {
                                declaration.access = StructuralAccess::Owned;
                                declaration.multiplicity = chain.multiplicity;
                            }
                        }

                        fn member(&self, position: usize, dense: u32) -> Option<&LoanChain> {
                            self.chains
                                .iter()
                                .find(|chain| chain.contains(position, dense))
                        }

                        pub(crate) fn finish(
                            &self,
                            machine: &mut TerminalMachine,
                            state_ids: &[BlockId],
                            live: &[bool],
                            captured_loans: &super::captured_loans::CapturedLoans,
                            types: &[terminal_psi::StructuralTypeDeclaration],
                            retained_entry_homes: &[PlaceId],
                            parents: &[(PlaceId, PlaceId)],
                            next_obligation: &mut u64,
                        ) -> Result<(), LoweringError> {
                            if self.chains.is_empty() && parents.is_empty() {
                                return Ok(());
                            }
                            let headers = state_ids
                                .iter()
                                .zip(live)
                                .filter(|(_, live)| **live)
                                .map(|(header, _)| *header)
                                .collect::<BTreeSet<_>>();
                            let producers = machine
                                .structural_places
                                .iter()
                                .filter_map(|place| match place.kind {
                                    StructuralPlaceKind::OperationResult { producer, .. } => {
                                        Some((place.id, producer))
                                    }
                                    _ => None,
                                })
                                .collect::<BTreeMap<_, _>>();
                            let machine_parameters = machine
                                .structural_parameters
                                .iter()
                                .map(|parameter| parameter.place)
                                .collect::<Vec<_>>();
                            for chain in &self.chains {
                                if chain.backing != ChainBacking::Transferred {
                                    continue;
                                }
                                if chain.multiplicity != StructuralMultiplicity::Affine {
                                    continue;
                                }
                                let dependent_carriers = chain
                                    .members
                                    .iter()
                                    .filter_map(|(position, dense, _)| {
                                        captured_loans.parent_carrier(*position, *dense)
                                    })
                                    .map(|carrier| carrier.place)
                                    .collect::<Vec<_>>();
                                for &(position, _, parameter) in &chain.members {
                                    let header = state_ids[position];
                                    let exit = match chain.backing {
                                        ChainBacking::LenderHome { .. } => continue,
                                        ChainBacking::Transferred => ChainExit {
                                            headers: &headers,
                                            storage: parameter,
                                            cleanup: &[],
                                            forwarded: Forwarding::ByArgument,
                                            dependent_carriers: &dependent_carriers,
                                            position: DisposalPosition::BlockParameter {
                                                position: block_position(
                                                    machine, header, parameter,
                                                )?,
                                                header_parameters: header_parameters(
                                                    machine, header,
                                                )?,
                                                machine_parameters: &machine_parameters,
                                            },
                                        },
                                    };
                                    let region = private_region(machine, header, &headers);
                                    for block in machine
                                        .blocks
                                        .iter_mut()
                                        .filter(|block| region.contains(&block.id))
                                    {
                                        exit.dispose(&mut block.terminator, next_obligation)?;
                                    }
                                }
                            }
                            retained_homes::finish(
                                machine,
                                &self.chains,
                                state_ids,
                                &headers,
                                captured_loans,
                                types,
                                retained_entry_homes,
                                parents,
                                &producers,
                                &machine_parameters,
                                next_obligation,
                            )?;
                            Ok(())
                        }
                    }

                    fn end_on_lender_exits(
                        machine: &mut TerminalMachine,
                        header: BlockId,
                        headers: &BTreeSet<BlockId>,
                        home: PlaceId,
                        producers: &BTreeMap<PlaceId, OperationId>,
                        members: &BTreeSet<BlockId>,
                        dependent_carriers: &[PlaceId],
                        cleanup: &[TerminalAffineCleanupAction],
                        next_obligation: &mut u64,
                    ) -> Result<(), LoweringError> {
                        let region = private_region(machine, header, headers);
                        let (birth, position) = if let Some(producer) =
                            producers.get(&home).copied()
                        {
                            let birth = machine
                                .blocks
                                .iter()
                                .find(|block| {
                                    region.contains(&block.id)
                                        && block
                                            .operations
                                            .iter()
                                            .any(|operation| operation.id == producer)
                                })
                                .map(|block| block.id)
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph state loan chain home is not established by its lender",
                                ))?;
                            (
                                birth,
                                DisposalPosition::OperationResult {
                                    producer,
                                    producers,
                                },
                            )
                        } else if let Some(parameter) = machine.signature_structural_parameter(home)
                        {
                            let parameters = machine
                                .structural_parameters
                                .iter()
                                .map(|parameter| (parameter.place, parameter.position))
                                .collect();
                            (
                                header,
                                DisposalPosition::BlockParameter {
                                    position: parameter.position,
                                    header_parameters: parameters,
                                    machine_parameters: &[],
                                },
                            )
                        } else {
                            let block = machine
                                .blocks
                                .iter()
                                .find(|block| {
                                    region.contains(&block.id)
                                        && block
                                            .structural_parameters
                                            .iter()
                                            .any(|parameter| parameter.place == home)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "retained state loan home has no dominating birth",
                                ))?;
                            let parameter = block
                                .structural_parameters
                                .iter()
                                .find(|parameter| parameter.place == home)
                                .ok_or(LoweringError::Unsupported(
                                    "retained state loan home lost its birth parameter",
                                ))?;
                            (
                                block.id,
                                DisposalPosition::BlockParameter {
                                    position: parameter.position,
                                    header_parameters: header_parameters(machine, block.id)?,
                                    machine_parameters: &[],
                                },
                            )
                        };
                        let established = private_region(machine, birth, headers);
                        let exit = ChainExit {
                            headers,
                            storage: home,
                            cleanup,
                            forwarded: Forwarding::ByLoan(members),
                            dependent_carriers,
                            position,
                        };
                        for block in machine
                            .blocks
                            .iter_mut()
                            .filter(|block| established.contains(&block.id))
                        {
                            exit.dispose(&mut block.terminator, next_obligation)?;
                        }
                        Ok(())
                    }

                    fn loan_sources<'a>(
                        checked: &'a checked_trees::CheckedTrees,
                        state: &'a checked_trees::CheckedComposedUnitControlStatePlan,
                        statement: u32,
                        target: symbols::SymbolHandle,
                        argument: u32,
                        transfer: &'a checked_trees::CheckedStructuralControlTransferPlan,
                        operations: &'a [checked_trees::CheckedUnitEffectOperationPlan],
                        values: &'a checked_trees::CheckedStructuralValuePlans,
                    ) -> impl Iterator<Item = Result<LoanSource, LoweringError>> + 'a
                    {
                        let legacy = transfer
                            .incoming_loan_resources
                            .is_empty()
                            .then(|| Ok(loan_source(checked, transfer, operations, values)));
                        transfer
                            .incoming_loan_resources
                            .iter()
                            .map(move |handle| {
                                if transfer.retained_loan_backing.is_some() {
                                    return Err(LoweringError::Unsupported(
                                        "state loan ingress disagrees with owned backing",
                                    ));
                                }
                                let facts = &checked.facts;
                                if !facts.borrow.call_loan_resources.is_valid(*handle) {
                                    return Err(LoweringError::Unsupported(
                                        "state loan ingress resource is stale",
                                    ));
                                }
                                let resource = facts.borrow.call_loan_resources.get(*handle);
                                if resource.state_symbol != state.state
                                    || !resource.owner_path.is_empty()
                                    || !facts.flow.control.calls.is_valid(resource.call)
                                {
                                    return Err(LoweringError::Unsupported(
                                        "state loan ingress has no exact current namespace",
                                    ));
                                }
                                let call = facts.flow.control.calls.get(resource.call);
                                let namespace = facts
                                    .flow
                                    .control
                                    .state(resource.machine_symbol, state.state)
                                    .is_some_and(|flow| flow.calls.handles().any(|handle| handle == resource.call));
                                if !namespace
                                    || call.target_symbol != target
                                    || call.statement_index != statement as usize
                                    || call.argument_operands().handles().nth(argument as usize)
                                        != Some(resource.owner_operand)
                                {
                                    return Err(LoweringError::Unsupported(
                                        "state loan ingress belongs to another captured operand",
                                    ));
                                }
                                let path = if resource.backing_path.is_empty() {
                                    &[][..]
                                } else {
                                    facts
                                        .borrow
                                        .call_loan_backing_paths
                                        .span(resource.backing_path)
                                        .ok_or(LoweringError::Unsupported(
                                            "state loan ingress backing path is stale",
                                        ))?
                                };
                                let (symbol, projected) = if resource.backing_incoming_reference.is_valid() {
                                    if !facts
                                        .borrow
                                        .incoming_references
                                        .is_valid(resource.backing_incoming_reference)
                                    {
                                        return Err(LoweringError::Unsupported(
                                            "state loan ingress identity is stale",
                                        ));
                                    }
                                    let incoming = facts
                                        .borrow
                                        .incoming_references
                                        .get(resource.backing_incoming_reference);
                                    if incoming.machine_symbol != resource.machine_symbol
                                        || incoming.state_symbol != state.state
                                    {
                                        return Err(LoweringError::Unsupported(
                                            "state loan ingress origin belongs to another invocation",
                                        ));
                                    }
                                    let owner = if incoming.owner_path.is_empty() {
                                        &[][..]
                                    } else {
                                        facts
                                            .borrow
                                            .owner_segments
                                            .span(incoming.owner_path)
                                            .ok_or(LoweringError::Unsupported(
                                                "state loan ingress owner path is stale",
                                            ))?
                                    };
                                    let symbol = match incoming.origin {
                                        checked_trees::IncomingReferenceOrigin::ParameterReferent { symbol }
                                        | checked_trees::IncomingReferenceOrigin::Parameter { symbol } => symbol,
                                        checked_trees::IncomingReferenceOrigin::Invocation { .. }
                                        | checked_trees::IncomingReferenceOrigin::Output { .. }
                                        | checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. } => {
                                            return Ok(LoanSource::Foreign);
                                        }
                                    };
                                    (symbol, !path.is_empty() || !owner.is_empty())
                                } else {
                                    let symbol =
                                        resource
                                            .parent_lifetime
                                            .root
                                            .as_symbol()
                                            .ok_or(LoweringError::Unsupported(
                                                "ordinary state loan ingress has no formal root",
                                            ))?;
                                    let (machine, typed_state) = validation::find_state(&checked.typed, state.state)
                                        .ok_or(LoweringError::Unsupported(
                                            "ordinary state loan ingress state is absent",
                                        ))?;
                                    let parameter = checked
                                        .typed
                                        .state_parameter_by_symbol(typed_state, symbol)
                                        .ok_or(LoweringError::Unsupported(
                                            "ordinary state loan ingress formal is absent",
                                        ))?;
                                    if validation::reference_shell(&checked.typed, parameter.type_reference).is_none()
                                        || !validation::state_reference_parameter_binding_is_stable(
                                            &checked.typed,
                                            machine,
                                            typed_state,
                                            symbol,
                                        )
                                    {
                                        return Err(LoweringError::Unsupported(
                                            "ordinary state loan ingress binding is not established",
                                        ));
                                    }
                                    (symbol, !path.is_empty())
                                };
                                let typed_state =
                                    checked
                                        .typed
                                        .state_by_symbol(state.state)
                                        .ok_or(LoweringError::Unsupported(
                                            "state loan ingress state is absent",
                                        ))?;
                                let position = checked
                                    .typed
                                    .state_parameters(typed_state)
                                    .iter()
                                    .position(|parameter| parameter.symbol == symbol)
                                    .ok_or(LoweringError::Unsupported(
                                        "state loan ingress formal is absent",
                                    ))?;
                                let dense = state
                                    .structural_parameters
                                    .iter()
                                    .position(|parameter| parameter.position as usize == position)
                                    .ok_or(LoweringError::Unsupported(
                                        "state loan ingress has no structural formal",
                                    ))?;
                                if state.structural_parameters[dense].access == CheckedStructuralAccess::Owned {
                                    return Ok(LoanSource::Foreign);
                                }
                                let dense = u32::try_from(dense).map_err(|_| {
                                    LoweringError::Unsupported("state loan ingress formal index overflows")
                                })?;
                                Ok(if projected {
                                    LoanSource::ProjectedParameter(dense)
                                } else {
                                    LoanSource::Parameter(dense)
                                })
                            })
                            .chain(legacy)
                    }

                    fn loan_source(
                        checked: &checked_trees::CheckedTrees,
                        transfer: &checked_trees::CheckedStructuralControlTransferPlan,
                        operations: &[checked_trees::CheckedUnitEffectOperationPlan],
                        values: &checked_trees::CheckedStructuralValuePlans,
                    ) -> LoanSource {
                        if let Some(backing) =
                            super::super::result_custody::transferred_loan_backing(
                                checked, operations, transfer,
                            )
                        {
                            return if backing.completed {
                                LoanSource::Completed {
                                    binding: backing.binding,
                                    requires_stable_home: backing.retiring_carrier.is_none(),
                                }
                            } else {
                                LoanSource::Local {
                                    binding: backing.binding,
                                    requires_stable_home: backing.retiring_carrier.is_none(),
                                }
                            };
                        }
                        if let Some((subject, projected)) =
                            super::super::result_custody::loan_subject(&transfer.source, values)
                        {
                            if let Some(binding) = subject.source.current_result_binding(operations)
                            {
                                return LoanSource::Local {
                                    binding,
                                    requires_stable_home: projected || !subject.path.is_empty(),
                                };
                            }
                            return match subject.source {
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                } if !projected && subject.path.is_empty() => {
                                    LoanSource::Parameter(parameter_index)
                                }
                                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                } => LoanSource::ProjectedParameter(parameter_index),
                                _ => LoanSource::Other,
                            };
                        }
                        match &transfer.source {
                            CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                                establishment,
                                presentation,
                                ..
                            } => {
                                if !presentation.path.is_empty() {
                                    return LoanSource::Other;
                                }
                                establishment
                                    .structural_result_binding()
                                    .map(|result| LoanSource::Completed {
                                        binding: result.binding_ordinal,
                                        requires_stable_home: false,
                                    })
                                    .unwrap_or(LoanSource::Other)
                            }
                            CheckedStructuralControlTransferSourcePlan::StructuralResult {
                                binding_ordinal,
                            } => LoanSource::Local {
                                binding: *binding_ordinal,
                                requires_stable_home: false,
                            },
                            CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                LoanSource::Parameter(*index)
                            }
                            _ => LoanSource::Other,
                        }
                    }

                    fn projected_local(
                        source: &CheckedStructuralControlTransferSourcePlan,
                        operations: &[checked_trees::CheckedUnitEffectOperationPlan],
                        values: &checked_trees::CheckedStructuralValuePlans,
                    ) -> Option<u32> {
                        super::super::result_custody::loan_subject(source, values)?
                            .0
                            .source
                            .current_result_binding(operations)
                    }

                    fn representative(links: &mut [usize], mut node: usize) -> usize {
                        while links[node] != node {
                            links[node] = links[links[node]];
                            node = links[node];
                        }
                        node
                    }

                    fn link(links: &mut [usize], first: usize, second: usize) {
                        let first = representative(links, first);
                        let second = representative(links, second);
                        if first != second {
                            links[second.max(first)] = first.min(second);
                        }
                    }

                    fn header_parameters(
                        machine: &TerminalMachine,
                        header: BlockId,
                    ) -> Result<Vec<(PlaceId, u32)>, LoweringError> {
                        Ok(machine
                            .block(&header)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph state loan chain member block is absent",
                            ))?
                            .structural_parameters
                            .iter()
                            .map(|parameter| (parameter.place, parameter.position))
                            .collect())
                    }

                    fn block_position(
                        machine: &TerminalMachine,
                        header: BlockId,
                        place: PlaceId,
                    ) -> Result<u32, LoweringError> {
                        header_parameters(machine, header)?
                            .into_iter()
                            .find(|(candidate, _)| *candidate == place)
                            .map(|(_, position)| position)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph state loan chain member left its state block",
                            ))
                    }

                    fn private_region(
                        machine: &TerminalMachine,
                        header: BlockId,
                        headers: &BTreeSet<BlockId>,
                    ) -> BTreeSet<BlockId> {
                        let mut region = BTreeSet::from([header]);
                        let mut pending = vec![header];
                        while let Some(current) = pending.pop() {
                            let Some(block) = machine.block(&current) else {
                                continue;
                            };
                            let targets = block.terminator.successor_targets();
                            for target in targets {
                                if !headers.contains(&target) && region.insert(target) {
                                    pending.push(target);
                                }
                            }
                        }
                        region
                    }

                    enum Forwarding<'a> {
                        IntoMember(&'a BTreeSet<BlockId>),
                        ByArgument,
                        ByLoan(&'a BTreeSet<BlockId>),
                    }

                    enum DisposalPosition<'a> {
                        OperationResult {
                            producer: OperationId,
                            producers: &'a BTreeMap<PlaceId, OperationId>,
                        },
                        RetainedHome {
                            earlier_homes: Vec<PlaceId>,
                            machine_parameters: &'a [PlaceId],
                        },
                        BlockParameter {
                            position: u32,
                            header_parameters: Vec<(PlaceId, u32)>,
                            machine_parameters: &'a [PlaceId],
                        },
                    }

                    struct ChainExit<'a> {
                        headers: &'a BTreeSet<BlockId>,
                        storage: PlaceId,
                        cleanup: &'a [TerminalAffineCleanupAction],
                        forwarded: Forwarding<'a>,
                        position: DisposalPosition<'a>,
                        dependent_carriers: &'a [PlaceId],
                    }

                    impl ChainExit<'_> {
                        fn dispose(
                            &self,
                            terminator: &mut Terminator,
                            next_obligation: &mut u64,
                        ) -> Result<(), LoweringError> {
                            match terminator {
                                Terminator::Jump {
                                    target,
                                    structural_arguments,
                                    cleanup_actions,
                                    ..
                                } => self.leave(
                                    *target,
                                    structural_arguments,
                                    cleanup_actions,
                                    next_obligation,
                                ),
                                Terminator::Conditional {
                                    when_true,
                                    when_false,
                                    ..
                                } => {
                                    for arm in [when_true, when_false] {
                                        self.leave(
                                            arm.target,
                                            &arm.structural_arguments,
                                            &mut arm.cleanup_actions,
                                            next_obligation,
                                        )?;
                                    }
                                    Ok(())
                                }
                                Terminator::StructuralCase { cases, .. } => {
                                    for arm in cases {
                                        self.leave(
                                            arm.target,
                                            &arm.structural_arguments,
                                            &mut arm.cleanup_actions,
                                            next_obligation,
                                        )?;
                                    }
                                    Ok(())
                                }
                                Terminator::ReturnStructural {
                                    source,
                                    cleanup_actions,
                                    ..
                                } => {
                                    if *source == self.storage {
                                        if matches!(self.forwarded, Forwarding::ByLoan(_)) {
                                            return Ok(());
                                        }
                                        return Err(LoweringError::Unsupported(
                                            "Unit graph state loan chain storage cannot escape as an owned return",
                                        ));
                                    }
                                    self.insert_actions(cleanup_actions, next_obligation)
                                }
                                terminal_psi::terminator_return_pair_pattern!(
                                    cleanup_actions,
                                    ..
                                ) => self.insert_actions(cleanup_actions, next_obligation),
                                Terminator::InvokeNoNormal { .. }
                                | Terminator::Crash { .. }
                                | Terminator::ExternalCompletion { .. } => Ok(()),
                            }
                        }

                        fn leave(
                            &self,
                            target: BlockId,
                            arguments: &[StructuralArgument],
                            discards: &mut Vec<TerminalAffineCleanupAction>,
                            next_obligation: &mut u64,
                        ) -> Result<(), LoweringError> {
                            if !self.headers.contains(&target) {
                                return Ok(());
                            }
                            match self.forwarded {
                                Forwarding::IntoMember(members) if members.contains(&target) => {
                                    return Ok(());
                                }
                                Forwarding::ByArgument => {
                                    if let Some(argument) = arguments
                                        .iter()
                                        .find(|argument| argument.place == self.storage)
                                    {
                                        if !argument.path.is_empty()
                                            || argument.access != StructuralAccess::Owned
                                        {
                                            return Err(LoweringError::Unsupported(
                                                "Unit graph transferred loan leaves its state without moving its storage",
                                            ));
                                        }
                                        return Ok(());
                                    }
                                }
                                Forwarding::ByLoan(members) => {
                                    if members.contains(&target)
                                        || arguments.iter().any(|argument| {
                                            argument.place == self.storage
                                                && argument.path.is_empty()
                                                && argument.access == StructuralAccess::Owned
                                        })
                                    {
                                        return Ok(());
                                    }
                                }
                                Forwarding::IntoMember(_) => {}
                            }
                            self.insert_actions(discards, next_obligation)
                        }

                        fn insert_actions(
                            &self,
                            actions: &mut Vec<TerminalAffineCleanupAction>,
                            next_obligation: &mut u64,
                        ) -> Result<(), LoweringError> {
                            if matches!(self.forwarded, Forwarding::ByLoan(_))
                                && actions.iter().any(|action| action.place() == self.storage)
                            {
                                return Ok(());
                            }
                            let mut places = actions
                                .iter()
                                .map(TerminalAffineCleanupAction::place)
                                .collect::<Vec<_>>();
                            self.insert(&mut places)?;
                            let at = places
                                .iter()
                                .position(|place| *place == self.storage)
                                .expect("inserted loan storage");
                            if self.cleanup.is_empty() {
                                actions.insert(
                                    at,
                                    TerminalAffineCleanupAction::DiscardRoot(self.storage),
                                );
                            } else {
                                let mut selected = self.cleanup.to_vec();
                                for action in &mut selected {
                                    if let TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                                        action
                                    {
                                        for obligation in &mut cleanup.requirement_obligations {
                                            *obligation =
                                                crate::shared::terminal_identities::obligation_id(
                                                    *next_obligation,
                                                );
                                            *next_obligation =
                                                next_obligation
                                                    .checked_add(1)
                                                    .ok_or(LoweringError::Unsupported(
                                                        "retained cleanup exhausted obligation identities",
                                                    ))?;
                                        }
                                    }
                                }
                                actions.splice(at..at, selected);
                            }
                            Ok(())
                        }

                        fn insert(&self, discards: &mut Vec<PlaceId>) -> Result<(), LoweringError> {
                            if discards.contains(&self.storage) {
                                return Err(LoweringError::Unsupported(
                                    "Unit graph state loan chain storage is already disposed on this exit",
                                ));
                            }
                            let at = match &self.position {
                                DisposalPosition::OperationResult {
                                    producer,
                                    producers,
                                } => discards.iter().position(|discard| {
                                    producers
                                        .get(discard)
                                        .is_none_or(|existing| existing < producer)
                                }),
                                DisposalPosition::BlockParameter {
                                    position,
                                    header_parameters,
                                    machine_parameters,
                                } => discards.iter().position(|discard| {
                                    machine_parameters.contains(discard)
                                        || header_parameters.iter().any(|(candidate, earlier)| {
                                            candidate == discard && earlier < position
                                        })
                                }),
                                DisposalPosition::RetainedHome {
                                    earlier_homes,
                                    machine_parameters,
                                } => discards.iter().position(|discard| {
                                    machine_parameters.contains(discard)
                                        || earlier_homes.contains(discard)
                                }),
                            };
                            let after_carriers = discards
                                .iter()
                                .rposition(|place| self.dependent_carriers.contains(place))
                                .map_or(0, |position| position + 1);
                            discards.insert(
                                at.unwrap_or(discards.len()).max(after_carriers),
                                self.storage,
                            );
                            Ok(())
                        }
                    }
                }
                pub(crate) mod loaned_local_homes {

                    use crate::LoweringError;
                    use semantic_vocabulary::PlaceId;
                    use std::collections::{BTreeMap, BTreeSet};
                    use terminal_psi::{
                        StructuralAccess, TerminalAffineCleanupAction, TerminalMachine, Terminator,
                    };

                    pub(crate) fn finish(
                        machine: &mut TerminalMachine,
                        homes: &BTreeMap<u32, PlaceId>,
                        dependencies: &[(PlaceId, PlaceId)],
                    ) -> Result<Vec<PlaceId>, LoweringError> {
                        if homes.is_empty() {
                            return Ok(Vec::new());
                        }
                        let (retained, parents) =
                            retained_reference_parents(machine, homes, dependencies)?;
                        authenticate_home_dispositions(machine, &retained)?;
                        for block in &mut machine.blocks {
                            match &mut block.terminator {
                                Terminator::Jump {
                                    cleanup_actions, ..
                                } => {
                                    cleanup_actions
                                        .retain(|action| !retained.contains(&action.place()));
                                }
                                Terminator::Conditional {
                                    when_true,
                                    when_false,
                                    ..
                                } => {
                                    for arm in [when_true, when_false] {
                                        arm.cleanup_actions
                                            .retain(|action| !retained.contains(&action.place()));
                                    }
                                }
                                Terminator::StructuralCase { cases, .. } => {
                                    for arm in cases {
                                        arm.cleanup_actions
                                            .retain(|action| !retained.contains(&action.place()));
                                    }
                                }
                                Terminator::ReturnStructural {
                                    source,
                                    cleanup_actions,
                                    ..
                                } => {
                                    if retained.contains(source) {
                                        return Err(LoweringError::Unsupported(
                                            "state loan backing cannot escape as an owned return",
                                        ));
                                    }
                                    merge_home_actions(cleanup_actions, &retained, &parents)?;
                                }
                                terminal_psi::terminator_return_pair_pattern!(
                                    cleanup_actions,
                                    ..
                                ) => {
                                    merge_home_actions(cleanup_actions, &retained, &parents)?;
                                }
                                _ => {}
                            }
                        }
                        Ok(retained)
                    }

                    fn authenticate_home_dispositions(
                        machine: &TerminalMachine,
                        homes: &[PlaceId],
                    ) -> Result<(), LoweringError> {
                        for home in homes {
                            let has_whole_disposition = whole_home_disposition(machine, *home)?;
                            if !has_whole_disposition {
                                return Err(LoweringError::Unsupported(
                                    "state loan home has no checked whole-home no-code disposition",
                                ));
                            }
                        }
                        Ok(())
                    }

                    pub(crate) fn whole_home_actions(
                        machine: &TerminalMachine,
                        home: PlaceId,
                    ) -> Result<Option<Vec<TerminalAffineCleanupAction>>, LoweringError>
                    {
                        let rosters = machine
                            .blocks
                            .iter()
                            .flat_map(|block| block.terminator.cleanup_rosters())
                            .map(|(_, actions)| {
                                actions
                                    .iter()
                                    .filter(|action| action.place() == home)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                            .filter(|actions| !actions.is_empty())
                            .collect::<Vec<_>>();
                        let Some(first) = rosters.first() else {
                            return Ok(None);
                        };
                        let expected = normalized_home_actions(first, home);
                        if rosters
                            .iter()
                            .skip(1)
                            .all(|actions| normalized_home_actions(actions, home) == expected)
                        {
                            return Ok(Some(first.clone()));
                        }
                        if let Some(whole) = rosters.iter().find(|actions| {
                            matches!(
                                actions.as_slice(),
                                [TerminalAffineCleanupAction::DiscardRoot(place)] if *place == home
                            )
                        }) && rosters.iter().all(|actions| {
                            actions == whole
                                || actions.iter().all(|action| {
                                    matches!(
                                        action,
                                        TerminalAffineCleanupAction::DiscardResidual(_)
                                    )
                                })
                        }) {
                            return Ok(Some(whole.clone()));
                        }
                        Err(LoweringError::Unsupported(
                            "retained state loan home has different selected cleanup partitions",
                        ))
                    }

                    fn normalized_home_actions(
                        actions: &[TerminalAffineCleanupAction],
                        home: PlaceId,
                    ) -> Vec<TerminalAffineCleanupAction> {
                        actions
                            .iter()
                            .filter(|action| action.place() == home)
                            .cloned()
                            .map(|mut action| {
                                if let TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                                    &mut action
                                {
                                    cleanup.requirement_obligations.clear();
                                }
                                action
                            })
                            .collect()
                    }

                    pub(crate) fn whole_home_disposition(
                        machine: &TerminalMachine,
                        home: PlaceId,
                    ) -> Result<bool, LoweringError> {
                        Ok(whole_home_actions(machine, home)?.is_some_and(|actions| {
                            actions == [TerminalAffineCleanupAction::DiscardRoot(home)]
                        }))
                    }

                    fn retained_reference_parents(
                        machine: &mut TerminalMachine,
                        homes: &BTreeMap<u32, PlaceId>,
                        dependencies: &[(PlaceId, PlaceId)],
                    ) -> Result<(Vec<PlaceId>, Vec<(PlaceId, PlaceId)>), LoweringError>
                    {
                        let mut retained = homes.values().copied().collect::<Vec<_>>();
                        let original_home_count = retained.len();
                        let mut parents = Vec::new();
                        let mut position = 0;
                        while position < retained.len() {
                            let home = retained[position];
                            position += 1;
                            for &(dependent, parent) in dependencies
                                .iter()
                                .filter(|(dependent, _)| *dependent == home)
                            {
                                if !parents.contains(&(dependent, parent)) {
                                    parents.push((dependent, parent));
                                }
                                if !retained.contains(&parent) {
                                    retained.push(parent);
                                }
                            }
                        }
                        if retained.len() > original_home_count {
                            let availability = HomeAvailability::new(machine)?;
                            for parent in &retained[original_home_count..] {
                                preserve_parent_home(machine, *parent, &availability)?;
                            }
                        }
                        Ok((retained, parents))
                    }

                    pub(crate) struct HomeAvailability {
                        dominators: Vec<BTreeSet<usize>>,
                        normal_returns: Vec<usize>,
                        entry: usize,
                    }

                    impl HomeAvailability {
                        pub(crate) fn new(
                            machine: &TerminalMachine,
                        ) -> Result<Self, LoweringError> {
                            let entry = machine
                                .blocks
                                .iter()
                                .position(|block| block.id == machine.entry)
                                .ok_or(LoweringError::Unsupported(
                                    "retained home graph has no entry",
                                ))?;
                            let successors = machine
                                .blocks
                                .iter()
                                .map(|block| {
                                    block
                                        .terminator
                                        .successor_targets()
                                        .into_iter()
                                        .map(|target| {
                                            machine
                                                .blocks
                                                .iter()
                                                .position(|block| block.id == target)
                                                .ok_or(LoweringError::Unsupported(
                                                    "retained home graph lost a successor",
                                                ))
                                        })
                                        .collect::<Result<Vec<_>, _>>()
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let mut reachable = vec![false; machine.blocks.len()];
                            let mut pending = vec![entry];
                            while let Some(position) = pending.pop() {
                                if !reachable[position] {
                                    reachable[position] = true;
                                    pending.extend(successors[position].iter().copied());
                                }
                            }
                            let normal_returns = machine
                                .blocks
                                .iter()
                                .enumerate()
                                .filter_map(|(position, block)| {
                                    (reachable[position]
                                        && matches!(
                                            block.terminator,
                                            terminal_psi::terminator_return_pair_pattern!(..)
                                                | Terminator::ReturnStructural { .. }
                                        ))
                                    .then_some(position)
                                })
                                .collect();
                            Ok(Self {
                                dominators: crate::emission::control_flow::block_dominators(
                                    Some(entry),
                                    &successors,
                                ),
                                normal_returns,
                                entry,
                            })
                        }
                    }

                    impl HomeAvailability {
                        pub(crate) fn birth(
                            &self,
                            machine: &TerminalMachine,
                            parent: PlaceId,
                        ) -> Result<usize, LoweringError> {
                            if machine.signature_structural_parameter(parent).is_some() {
                                Ok(self.entry)
                            } else {
                                machine
                                    .blocks
                                    .iter()
                                    .position(|block| {
                                        block
                                            .structural_parameters
                                            .iter()
                                            .any(|parameter| parameter.place == parent)
                                            || block.operations.iter().any(|operation| {
                                                operation
                                                    .result
                                                    .structural()
                                                    .is_some_and(|result| result.place == parent)
                                            })
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "retained parent lost its home birth",
                                    ))
                            }
                        }

                        pub(crate) fn require_in_regions(
                            &self,
                            machine: &TerminalMachine,
                            parent: PlaceId,
                            members: &BTreeSet<semantic_vocabulary::BlockId>,
                        ) -> Result<(), LoweringError> {
                            let birth = self.birth(machine, parent)?;
                            for member in members {
                                let position = machine
                                    .blocks
                                    .iter()
                                    .position(|block| block.id == *member)
                                    .ok_or(LoweringError::Unsupported(
                                        "retained parent lost a lifetime member",
                                    ))?;
                                if !self.dominators[position].contains(&birth) {
                                    return Err(LoweringError::Unsupported(
                                        "retained parent is not available on every lifetime arrival",
                                    ));
                                }
                            }
                            Ok(())
                        }
                    }

                    fn preserve_parent_home(
                        machine: &mut TerminalMachine,
                        parent: PlaceId,
                        availability: &HomeAvailability,
                    ) -> Result<(), LoweringError> {
                        let birth = availability.birth(machine, parent)?;
                        if availability
                            .normal_returns
                            .iter()
                            .any(|position| !availability.dominators[*position].contains(&birth))
                        {
                            return Err(LoweringError::Unsupported(
                                "retained reference parent is not available on every normal return",
                            ));
                        }
                        for block in &mut machine.blocks {
                            let mut changes_carrier = false;
                            for operation in &mut block.operations {
                                operation.kind.map_structural_uses_with_access(
                                    &mut |place, access| {
                                        changes_carrier |= place == parent
                                            && access != StructuralAccess::SharedBorrow;
                                        place
                                    },
                                );
                            }
                            let consumes = |arguments: &[terminal_psi::StructuralArgument]| {
                                arguments.iter().any(|argument| {
                                    argument.place == parent
                                        && argument.access != StructuralAccess::SharedBorrow
                                })
                            };
                            changes_carrier |= match &block.terminator {
                                Terminator::Jump {
                                    structural_arguments,
                                    ..
                                }
                                | Terminator::InvokeNoNormal {
                                    structural_arguments,
                                    ..
                                }
                                | Terminator::ExternalCompletion {
                                    structural_arguments,
                                    ..
                                } => consumes(structural_arguments),
                                Terminator::Conditional {
                                    when_true,
                                    when_false,
                                    ..
                                } => {
                                    consumes(&when_true.structural_arguments)
                                        || consumes(&when_false.structural_arguments)
                                }
                                Terminator::ReturnStructural { source, .. } => *source == parent,
                                Terminator::StructuralCase { .. }
                                | terminal_psi::terminator_return_pair_pattern!(..)
                                | Terminator::Crash { .. } => false,
                            };
                            changes_carrier |=
                                block.terminator.cleanup_rosters().any(|(_, actions)| {
                                    actions.iter().any(|action| {
                                        action.place() == parent
                                            && *action
                                                != TerminalAffineCleanupAction::DiscardRoot(parent)
                                    })
                                });
                            if changes_carrier {
                                return Err(LoweringError::Unsupported(
                                    "retained reference parent has consuming or mutable runtime demand",
                                ));
                            }
                        }
                        Ok(())
                    }

                    fn merge_home_actions(
                        actions: &mut Vec<TerminalAffineCleanupAction>,
                        homes: &[PlaceId],
                        parents: &[(PlaceId, PlaceId)],
                    ) -> Result<(), LoweringError> {
                        let mut deferred_home_actions = Vec::new();
                        actions.retain(|action| {
                            if homes.contains(&action.place()) {
                                deferred_home_actions.push(action.clone());
                                false
                            } else {
                                true
                            }
                        });
                        for home in homes {
                            if !deferred_home_actions
                                .iter()
                                .any(|action| action.place() == *home)
                            {
                                deferred_home_actions
                                    .push(TerminalAffineCleanupAction::DiscardRoot(*home));
                            }
                        }
                        while !deferred_home_actions.is_empty() {
                            let ready_home_position = deferred_home_actions
                                .iter()
                                .position(|action| {
                                    !parents.iter().any(|(child, parent)| {
                                        *parent == action.place()
                                            && deferred_home_actions.iter().any(|pending_action| {
                                                pending_action.place() == *child
                                            })
                                    })
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "retained reference cleanup has cyclic formation dependencies",
                                ))?;
                            actions.push(deferred_home_actions.remove(ready_home_position));
                        }
                        for (child, parent) in parents {
                            let child_position =
                                actions.iter().rposition(|action| action.place() == *child);
                            let parent_position =
                                actions.iter().position(|action| action.place() == *parent);
                            if !matches!((child_position, parent_position),
                                (Some(child), Some(parent)) if child < parent)
                            {
                                return Err(LoweringError::Unsupported(
                                    "retained reference cleanup does not follow formation dependencies",
                                ));
                            }
                        }
                        Ok(())
                    }
                }
                pub(crate) mod seal {

                    use super::super::super::super::super::{
                        CheckedComposedUnitControlTerminatorPlan, StructuralParameterDeclaration,
                        ValueId,
                    };
                    use super::super::super::super::{Block, Terminator, unsupported};
                    use super::super::super::LoweringError;
                    use super::StateGraphEmission;
                    use crate::emission::operation_emission::buffer::OperationBuffer;
                    use crate::emission::structural_byte_sequence_store::retain_literal_declarations;
                    use semantic_vocabulary::EdgeId;

                    impl StateGraphEmission<'_, '_> {
                        pub(crate) fn preserve_edges(
                            &mut self,
                            edges: impl Iterator<Item = EdgeId>,
                            rank: &[ValueId],
                        ) {
                            self.rank_edges.extend(edges.map(|edge| {
                                (
                                    edge,
                                    (
                                        rank.to_vec(),
                                        terminal_psi::TerminalNaturalRankComparison::Preserving,
                                    ),
                                )
                            }));
                        }

                        pub(crate) fn seal_state(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                            position: usize,
                            mut terminator: Terminator,
                            edge_blocks: Vec<Block>,
                            mut evaluation: crate::unit::attached_unit::argument_evaluation::Evaluation,
                            mut operations: OperationBuffer,
                            frame: SealFrame<'_>,
                        ) -> Result<(), LoweringError> {
                            let SealFrame {
                                state_parameters,
                                current_rank,
                                is_guarded_return,
                                dispatches,
                                body_end,
                                mut next_value,
                                mut next_block,
                                mut next_edge,
                            } = frame;
                            let current_parameters =
                                evaluation.current_parameters(state_parameters);
                            let state_parameters = current_parameters.as_ref();
                            let plan = self.plan;
                            let state = &plan.states[position];
                            if !is_guarded_return
                                && matches!(
                                    &terminator,
                                    terminal_psi::Terminator::ReturnStructural { .. }
                                )
                                && let CheckedComposedUnitControlTerminatorPlan::ReturnStructural {
                                    result,
                                } = &state.terminator
                            {
                                let statement_index = match &result.completion {
                                    checked_trees::CheckedStructuralReturnCompletion::Value {
                                        statement_index,
                                        ..
                                    } => *statement_index as usize,
                                    checked_trees::CheckedStructuralReturnCompletion::Call {
                                        coordinate,
                                    } => coordinate.statement_index as usize,
                                };
                                let exit = super::super::cleanup::coordinate(
                                    self.catalogs.cleanup_control,
                                    state,
                                    statement_index,
                                    false,
                                )?;
                                let mut regions = super::super::cleanup::for_exit_regions(
                                    identities,
                                    self.claims.published(),
                                    self.checked,
                                    state,
                                    exit,
                                    self.catalogs,
                                    state_parameters,
                                    &evaluation,
                                    &operations,
                                )?;
                                super::super::cleanup::selection(
                                    state.state,
                                    &state.operations,
                                    &evaluation,
                                    &operations,
                                    &mut regions,
                                )?;
                                let mut values = Vec::new();
                                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal,
                                } = result.source
                                {
                                    let binding =
                                        super::super::case_emission::result(state, binding_ordinal, &operations)?
                                            .into_owned();
                                    let source = binding.place();
                                    let (completion, arrival) =
                                        crate::unit::attached_unit::structural_returns::materialize(
                                            identities,
                                            self.checked,
                                            &terminal_psi::StructuralArgument {
                                                place: source,
                                                path: binding.path().to_vec(),
                                                access: terminal_psi::StructuralAccess::Owned,
                                            },
                                            binding.structural_type(),
                                            binding.multiplicity(),
                                            terminator,
                                            regions,
                                            &mut self.catalogs.next_place,
                                            &mut self.catalogs.temporary_places,
                                            &mut evaluation,
                                            &mut values,
                                            &mut next_value,
                                            &mut next_block,
                                            &mut next_edge,
                                            &mut operations,
                                        )?;
                                    terminator = completion;
                                    if let Some(arrival) = arrival {
                                        if let Some(rank) = current_rank {
                                            self.preserve_edges(terminator.edges(), rank);
                                        }
                                        evaluation.blocks.push(arrival);
                                    }
                                } else {
                                    let actions = evaluation.emit_cleanup_regions(
                                        identities,
                                        self.checked,
                                        regions,
                                        &mut values,
                                        &mut next_value,
                                        &mut next_block,
                                        &mut next_edge,
                                        &mut operations,
                                    )?;
                                    let Terminator::ReturnStructural {
                                        cleanup_actions, ..
                                    } = &mut terminator
                                    else {
                                        return unsupported("structural return lost its ordinary completion");
                                    };
                                    *cleanup_actions = actions;
                                }
                            }
                            if let Some(rank) = current_rank {
                                if matches!(
                                    state.terminator,
                                    CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
                                ) {
                                    self.block_ranks.extend(
                                        edge_blocks.iter().map(|block| (block.id, rank.to_vec())),
                                    );
                                    self.preserve_edges(
                                        edge_blocks
                                            .iter()
                                            .flat_map(|block| block.terminator.edges()),
                                        rank,
                                    );
                                    self.preserve_edges(terminator.edges(), rank);
                                }
                                self.preserve_edges(
                                    evaluation
                                        .blocks
                                        .iter()
                                        .flat_map(|block| block.terminator.edges()),
                                    rank,
                                );
                            }

                            evaluation.blocks.push(Block {
                                id: evaluation.current,
                                parameters: std::mem::take(&mut evaluation.parameters),
                                erased_scalar_formals: Vec::new(),
                                erased_proof_formals: std::mem::take(
                                    &mut evaluation.completion_proof_formals,
                                ),
                                structural_parameters: std::mem::take(
                                    &mut evaluation.block_structural_parameters,
                                ),
                                operations: operations[evaluation.operation_start
                                    ..if dispatches || !edge_blocks.is_empty() {
                                        body_end
                                    } else {
                                        operations.len()
                                    }]
                                    .to_vec(),
                                terminator,
                            });
                            let root = evaluation
                                .blocks
                                .iter_mut()
                                .find(|block| block.id == self.state_ids[position])
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph state root disappeared during evaluation",
                                ))?;
                            if position != 0 || self.entry_reentered {
                                let aliased = &self.admitted.entry_aliases;
                                let loan_chains = &self.loan_chains;
                                let exclusive_handoffs =
                                    self.admitted.exclusive_handoffs.get(position);
                                root.structural_parameters =
                                    std::mem::take(&mut self.state_views[position])
                                        .into_iter()
                                        .enumerate()
                                        .filter(|(dense, _)| {
                                            !aliased.get(position).is_some_and(|aliased| {
                                                aliased.contains_key(&(*dense as u32))
                                            })
                                        })
                                        .map(|(dense, mut parameter)| {
                                            loan_chains.declare(
                                                position,
                                                dense as u32,
                                                &mut parameter,
                                            );
                                            if exclusive_handoffs.is_some_and(|promoted| {
                                                promoted.contains(&(dense as u32))
                                            }) {
                                                parameter.access =
                                                    terminal_psi::StructuralAccess::MutableBorrow;
                                            }
                                            parameter
                                        })
                                        .collect();
                                root.structural_parameters
                                    .extend(self.captured_loans.declarations(position));
                                root.erased_scalar_formals = self.state_erased[position].clone();
                                root.erased_proof_formals =
                                    crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                                        identities,
                                        self.checked,
                                        &self.state_erased_proof[position],
                                    );
                            } else {
                                crate::scalar_lowering::contracts::bind_invocation_proof_references(
                                    root,
                                );
                            }
                            if let Some(rank) = current_rank {
                                self.block_ranks.extend(
                                    evaluation
                                        .blocks
                                        .iter()
                                        .map(|block| (block.id, rank.to_vec())),
                                );
                            }
                            retain_literal_declarations(
                                &operations,
                                &mut self.catalogs.temporary_places,
                            )?;
                            evaluation.blocks.extend(edge_blocks);
                            let reference_retirements =
                                super::super::cleanup::observed_reference_retirements(
                                    state,
                                    &operations,
                                    |place| evaluation.descriptor_retirement(place, &operations),
                                )?;
                            super::super::cleanup::retire_observed_references(
                                &reference_retirements,
                                &operations,
                                &mut evaluation.blocks,
                            );
                            crate::emission::private_borrow_transport::forward_private_state_borrows(
                                self.state_ids[position],
                                &mut evaluation.blocks,
                                &self.catalogs.structural_types,
                                &mut self.catalogs.next_place,
                                &mut self.structural_places,
                            )?;
                            self.qualification_coercions
                                .append(&mut evaluation.qualification_coercions);
                            self.blocks.extend(evaluation.blocks);
                            self.catalogs.next_value = next_value;
                            self.catalogs.next_block = next_block;
                            self.catalogs.next_edge = next_edge;
                            self.catalogs.next_operation = operations.next_identity;
                            self.scalar_block_invariants
                                .append(&mut operations.scalar_block_invariants);
                            self.loan_completions
                                .append(&mut operations.loan_completions);
                            self.block_entry_claims
                                .append(&mut operations.block_entry_claims);
                            self.occurrences.retain(operations);
                            Ok(())
                        }
                    }

                    pub(crate) struct SealFrame<'s> {
                        pub(crate) state_parameters: &'s [StructuralParameterDeclaration],
                        pub(crate) current_rank: Option<&'s [ValueId]>,
                        pub(crate) is_guarded_return: bool,
                        pub(crate) dispatches: bool,
                        pub(crate) body_end: usize,
                        pub(crate) next_value: u64,
                        pub(crate) next_block: u64,
                        pub(crate) next_edge: u64,
                    }
                }
                pub(crate) mod state {

                    use super::super::super::super::super::{
                        CheckedComposedUnitControlTerminatorPlan, StructuralCaseSuccessorEdge,
                        SuccessorEdge, block_id,
                    };
                    use super::super::super::super::{
                        Block, Terminator, ValueDeclaration, allocate_dense, edge_id, unsupported,
                    };
                    use super::super::super::LoweringError;
                    use super::super::{
                        CheckedStructuralControlSuccessorPlan, case_emission, ranking,
                    };
                    use super::StateGraphEmission;
                    use crate::emission::boolean_control::LoweredBooleanDecision;
                    use crate::emission::case_payload_dispatch::PlannedGuard;
                    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
                    use crate::emission::operation_emission::buffer::{
                        OperationBuffer, StorageObservations,
                    };
                    use crate::emission::value_successor;
                    use crate::expression_preparation::bindings::ScalarBindings;
                    use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
                    use crate::expression_preparation::qualifications::PreparedScalarQualifications;
                    use crate::expression_preparation::qualifications::SourceScalarReturn;
                    use crate::proofs::crash_routes::{
                        lower_checked_crash_exit, lower_structural_crash_predicates,
                    };
                    use crate::scalar_lowering::computations::observations::prepare;
                    use crate::scalar_lowering::contracts::erased_proof_formal_declarations;
                    use crate::shared::terminal_identities::lookup_type_id;
                    use crate::unit::attached_unit::composed_control::emission::OperationSequence;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, StateFrame,
                    };
                    use crate::unit::attached_unit::signatures::find;

                    impl StateGraphEmission<'_, '_> {
                        pub(crate) fn crash_completion(
                            &self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            statement_ordinal: u32,
                            next_edge: &mut u64,
                        ) -> Result<Terminator, LoweringError> {
                            let state = &self.plan.states[position];
                            let crash = lower_checked_crash_exit(
                                self.checked,
                                self.admitted.authored[position].0.symbol,
                                state.state,
                                statement_ordinal,
                                &self.claims.source_claims,
                            )?;
                            Ok(Terminator::Crash {
                                edge: edge_id(allocate_dense(next_edge)?),
                                cause: crash.cause,
                                site_guard: self.crash_site_guards(
                                    identities,
                                    position,
                                    &crash.site_guard,
                                )?,
                                frontier_lower_bound: crash.frontier_lower_bound,
                            })
                        }

                        fn crash_site_guards(
                            &self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                            predicates: &[checked_trees::CheckedBooleanExpression],
                        ) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError>
                        {
                            if predicates.is_empty() {
                                return Ok(Vec::new());
                            }
                            let owner = self.admitted.authored[position].0;
                            if owner.symbol == self.plan.machine {
                                let signature = find(&self.catalogs.signatures, owner.symbol)?;
                                return lower_structural_crash_predicates(
                                    identities,
                                    predicates,
                                    &signature.scalar_parameters,
                                    &signature.predicate_parameters,
                                    &self.catalogs.structural_types,
                                    &signature.runtime_requirements,
                                );
                            }

                            let entry = self.checked.machine_states(owner).first().ok_or(
                                LoweringError::Unsupported(
                                    "fused crash owner has no invocation entry",
                                ),
                            )?;
                            let entry_position = self
                                .admitted
                                .authored
                                .iter()
                                .position(|(candidate, state)| {
                                    candidate.symbol == owner.symbol && state.symbol == entry.symbol
                                })
                                .filter(|position| self.admitted.live[*position])
                                .ok_or(LoweringError::Unsupported(
                                    "fused crash predicate has no live invocation entry namespace",
                                ))?;
                            let actual_values = &self.state_values[entry_position];
                            let actual_places = self.state_contract_rosters[entry_position]
                                .iter()
                                .map(|(position, declaration)| {
                                    let mut parameter = declaration.clone();
                                    parameter.position = *position;
                                    parameter
                                })
                                .collect::<Vec<_>>();
                            let requirements =
                                crate::unit::attached_unit::entry_requirements::lower(
                                    identities,
                                    self.checked,
                                    self.catalogs.scalar_applications,
                                    owner.symbol,
                                    actual_values,
                                    &self.state_erased[entry_position],
                                    &actual_places,
                                    &self.catalogs.structural_types,
                                )?;
                            lower_structural_crash_predicates(
                                identities,
                                predicates,
                                actual_values,
                                &actual_places,
                                &self.catalogs.structural_types,
                                &requirements.runtime_requirements,
                            )
                        }

                        pub(crate) fn emit_state(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                            position: usize,
                        ) -> Result<(), LoweringError> {
                            let checked = self.checked;
                            let plan = self.plan;
                            let state = &plan.states[position];
                            let owner = self.admitted.authored[position].0.symbol;
                            let state_parameters = self.state_views[position]
                                .iter()
                                .zip(&state.structural_parameters)
                                .map(|(parameter, source)| {
                                    let mut parameter = parameter.clone();
                                    parameter.position = source.position;
                                    parameter
                                })
                                .collect::<Vec<_>>();
                            let mut operations = OperationBuffer::new(
                self.catalogs.next_operation - 1,
                self.catalogs.next_place,
                std::sync::Arc::new(
                    crate::emission::reference_formations::ReferenceFormations::prepare(
                        checked,
                        state.state,
                        state
                            .operation_dependencies()
                            .flat_map(|operation| operation.with_value_calls()),
                        state.successors().into_iter(),
                    )?,
                ),
                std::sync::Arc::clone(&self.dynamic_dispatch),
            );
                            let mut evaluation = crate::unit::attached_unit::argument_evaluation::Evaluation {
                scalar_applications: std::sync::Arc::clone(self.catalogs.scalar_applications),
                completed_operand_scope: Vec::new(),
                current_claims: Default::default(),
                shared_qualifications: self
                    .catalogs
                    .prepared_scalar_qualifications
                    .cloned()
                    .unwrap_or_else(PreparedScalarQualifications::empty),
                machine: Some(self.terminal_machine),
                qualification_coercions: Vec::new(),
                structural_types: std::sync::Arc::from([]),
                structural_value_owners: Vec::new(),
                selection_cleanups: Vec::new(),
                computation_address_roots: Vec::new(),
                structural_locals: Vec::new(),
                completed_cleanup_homes: Vec::new(),
                retired_structural_bindings: Vec::new(),
                retired_structural_homes: Default::default(),
                view_locals: Vec::new(),
                element_views: std::collections::BTreeMap::new(),
                place_observations:
                    crate::scalar_lowering::computations::fields::prepare_place_observations(
                        identities,
                        checked,
                        plan.machine,
                        state.state,
                        &self.catalogs.structural_types,
                    ),
                record_fields: self.record_fields.clone(),
                literals: crate::scalar_lowering::computations::literals::prepare(
                    identities,
                    checked,
                    plan.machine,
                    &self.catalogs.structural_types,
                    &mut self.catalogs.next_place,
                )?,
                produced_values: self.produced_values.clone(),
                observations: prepare(checked, &self.owners, &mut self.catalogs.next_place)?,
                subslices: crate::scalar_lowering::computations::subslices::prepare(
                    identities,
                    checked,
                    &self.owners,
                    &self.catalogs.structural_types,
                    &mut self.catalogs.next_place,
                )?,
                cases: crate::scalar_lowering::computations::cases::prepare(
                    identities,
                    checked,
                    &self.owners,
                    &self.catalogs.structural_types,
                    &mut self.catalogs.next_place,
                )?,
                primitive_storage: Vec::new(),
                parameter_storage: Vec::new(),
                scalar_bindings: ScalarBindings::new(std::iter::empty()),
                structural_fields: Default::default(),
                structural_cases: Vec::new(),
                previous_parameter_homes: Vec::new(),
                enclosing_structural_arrivals: Vec::new(),
                structural_parameters: state
                    .structural_parameters
                    .iter()
                    .zip(&state_parameters)
                    .map(|(source, parameter)| (source.position, parameter.clone()))
                    .collect(),
                erased_scalar_formals: self.state_erased[position].clone(),
                erased_proof_formals: state.erased_proof_parameters.clone(),
                completion_proof_formals: Vec::new(),
                entry: self.state_ids[position],
                block_structural_parameters: Vec::new(),
                current: self.state_ids[position],
                parameters: if position == 0 && !self.entry_reentered {
                    Vec::new()
                } else {
                    self.state_values[position].clone()
                },
                operation_start: 0,
                blocks: Vec::new(),
            };
                            let mut entry_names = self.claims.entry_claims.clone();
                            entry_names.extend(self.block_entry_claims.iter().cloned());
                            evaluation.current_claims.ingress(
                                state_parameters
                                    .iter()
                                    .filter(|parameter| {
                                        parameter.access == terminal_psi::StructuralAccess::Owned
                                    })
                                    .map(|parameter| parameter.place)
                                    .chain(
                                        self.captured_loans
                                            .declarations(position)
                                            .map(|parameter| parameter.place),
                                    ),
                                &entry_names,
                            )?;
                            evaluation.primitive_storage =
                crate::unit::attached_unit::parameters::primitive_reference_storage(
                    checked,
                    state.state,
                    &state.structural_parameters,
                    &evaluation.structural_parameters,
                )?;
                            let mut values = self.state_values[position].clone();
                            let parameter_storage =
                super::super::super::super::primitive_locals::establish_parameter_storage(
                    &state.parameter_storage,
                    &state.scalar_parameters,
                    &values,
                    &self.catalogs.type_ids,
                    super::super::super::super::primitive_locals::PrimitiveStorageCatalog {
                        types: &self.catalogs.structural_types,
                        domains: self.catalogs.domain_declarations,
                        scalars: self.catalogs.scalar_calls.scalar_qualifications(),
                    },
                    &mut self.catalogs.next_place,
                    &mut self.catalogs.scalar_calls.next_call_obligation,
                    &mut operations,
                )?;
                            for storage in &parameter_storage {
                                self.catalogs
                                    .temporary_places
                                    .push(storage.local.declaration);
                            }
                            evaluation.parameter_storage = parameter_storage
                                .iter()
                                .map(|storage| storage.local)
                                .collect();
                            let mut sequence = OperationSequence::new(&evaluation);
                            let mut next_value = self.catalogs.next_value;
                            let mut next_block = self.catalogs.next_block;
                            let mut next_edge = self.catalogs.next_edge;
                            evaluation.scalar_bindings = ScalarBindings::entry(
                                values.iter().map(|value| value.scalar_type),
                                &evaluation.primitive_storage,
                                &evaluation.structural_parameters,
                                &self.catalogs.structural_types,
                                &evaluation.place_observations,
                            );
                            super::super::super::super::primitive_locals::register_mut_formal_storage_homes(
                &mut evaluation,
                &parameter_storage,
            )?;
                            evaluation.structural_types =
                                self.catalogs.structural_types.as_ref().into();
                            evaluation.structural_fields =
                crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                    &evaluation.structural_parameters,
                    &self.catalogs.structural_types,
                );
                            evaluation.structural_cases =
                                crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
                                    &evaluation.structural_parameters,
                                    &self.catalogs.structural_types,
                                );
                            evaluation.element_views =
                                crate::expression_preparation::bindings::element_views(
                                    &evaluation.structural_parameters,
                                    &self.catalogs.structural_types,
                                );
                            let current_rank =
                                if let Some(roots) = ranking::observations(checked, plan, state)? {
                                    let scalar_slots = values
                                        .iter()
                                        .enumerate()
                                        .map(|(position, value)| (position, value.scalar_type))
                                        .collect::<Vec<_>>();
                                    let structural_actuals = state_parameters
                                        .iter()
                                        .map(|parameter| {
                                            (
                                                terminal_psi::StructuralArgument {
                                                    place: parameter.place,
                                                    path: Vec::new(),
                                                    access: parameter.access,
                                                },
                                                parameter.structural_type,
                                            )
                                        })
                                        .collect::<Vec<_>>();
                                    let bindings = ranking::formal_bindings(
                                        checked,
                                        state,
                                        &evaluation.scalar_bindings,
                                        &scalar_slots,
                                        &structural_actuals,
                                    )?;
                                    if roots.len() != self.rank_parameters[position].len() {
                                        return unsupported(
                                            "ranked header lost an ordered component parameter",
                                        );
                                    }
                                    let ranks = roots
                    .iter()
                    .zip(&self.rank_parameters[position])
                    .map(|(root, parameter)| {
                        let rank_type =
                            crate::scalar_lowering::computations::computation_value_type(
                                identities,
                                checked,
                                &evaluation.shared_qualifications,
                                root.root,
                                &bindings,
                                &values
                                    .iter()
                                    .map(|value| value.value_type())
                                    .collect::<Vec<_>>(),
                            )?;
                        if rank_type.scalar_type.as_integer().is_none() {
                            return unsupported(
                                "Unit graph rank observation lost its integer carrier",
                            );
                        }
                        Ok(ValueDeclaration {
                            id: *parameter,
                            scalar_type: rank_type.scalar_type,
                            qualifications: rank_type.qualifications,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                                    evaluation.parameters.extend(ranks.iter().copied());
                                    if position == 0 {
                                        self.initialize_invocation_rank(
                                            identities,
                                            &roots,
                                            &evaluation,
                                            &ranks,
                                            &mut operations,
                                            &mut next_value,
                                            &mut next_block,
                                            &mut next_edge,
                                        )?;
                                    }
                                    Some(ranks.iter().map(|rank| rank.id).collect::<Vec<_>>())
                                } else {
                                    None
                                };
                            super::super::super::emission::emit_call_operations(
                                identities,
                                checked,
                                owner,
                                state,
                                &state.operations,
                                self.catalogs,
                                &state_parameters,
                                &mut evaluation,
                                &mut values,
                                &self.state_erased[position],
                                &mut next_value,
                                &mut next_block,
                                &mut next_edge,
                                &mut operations,
                                &mut sequence,
                            )?;
                            let current_parameters =
                                evaluation.current_parameters(&state_parameters);
                            let state_parameters = current_parameters.as_ref();
                            if operations.invocation_completion.is_some() {
                                sequence.require_closed()?;
                                let terminator = operations
                                    .take_invocation_completion(&mut next_edge)?
                                    .ok_or(LoweringError::Unsupported(
                                        "external completion lost its evaluated invocation",
                                    ))?;
                                let body_end = operations.len();
                                return self.seal_state(
                                    identities,
                                    position,
                                    terminator,
                                    Vec::new(),
                                    evaluation,
                                    operations,
                                    super::seal::SealFrame {
                                        state_parameters,
                                        current_rank: current_rank.as_deref(),
                                        is_guarded_return: false,
                                        dispatches: false,
                                        body_end,
                                        next_value,
                                        next_block,
                                        next_edge,
                                    },
                                );
                            }
                            let bindings = evaluation.contextual_bindings();
                            let mut branch_guard = None;
                            let conditional_true_ordinal = match &state.terminator {
                CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => {
                    Some(when_true.statement_ordinal)
                }
                CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    jump,
                    return_arm,
                    return_when_true,
                    ..
                } => {
                    if *return_when_true {
                        return_arm.statement_ordinal()
                    } else {
                        Some(jump.statement_ordinal)
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                    when_true,
                    ..
                } => when_true.statement_ordinal(),
                _ => None,
            };
                            let capture_boundary = match conditional_true_ordinal {
                                Some(statement) => statement,
                                None => state.statement_count,
                            };
                            evaluation.prepare_storage_before(
                                identities,
                                self.checked,
                                state.state,
                                capture_boundary,
                                &self.catalogs.structural_types,
                                self.catalogs.domain_declarations,
                                &values,
                                &mut self.catalogs.next_place,
                                &mut next_value,
                                &mut operations,
                                &mut self.catalogs.temporary_places,
                            )?;
                            let first_successor_reads_payloads = match &state.terminator {
                CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => {
                    successor_may_read_payloads(when_true)
                }
                CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    jump,
                    return_when_true: false,
                    ..
                } => successor_may_read_payloads(jump),
                CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    return_arm, ..
                }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                    when_true: return_arm,
                    ..
                } => return_may_read_payloads(return_arm),
                CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } => {
                    arms.first().is_some_and(guarded_arm_may_read_payloads)
                }
                _ => false,
            };
                            let entry_guard_ordinal = match conditional_true_ordinal {
                                Some(true_ordinal) => Some(true_ordinal),
                                None => match &state.terminator {
                                    CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
                                        arms,
                                        ..
                                    } => {
                                        let Some(first) = arms.first() else {
                                            return unsupported(
                                                "guarded jump chain lost its first arm",
                                            );
                                        };
                                        if arms.len() < 2 {
                                            return unsupported(
                                                "guarded jump chain lost its ordered arms",
                                            );
                                        }
                                        Some(first.statement_ordinal())
                                    }
                                    _ => None,
                                },
                            };
                            let condition = if let Some(guard_ordinal) = entry_guard_ordinal {
                                branch_guard = evaluation.branch_guard(
                                    identities,
                                    checked,
                                    state.state,
                                    guard_ordinal,
                                    &values,
                                    first_successor_reads_payloads,
                                )?;
                                if branch_guard.is_some() {
                                    None
                                } else {
                                    let condition = StateFrame::new(
                                        checked,
                                        owner,
                                        state,
                                        &self.state_erased[position],
                                    )
                                    .evaluate(
                                        self.catalogs,
                                        state_parameters,
                                        &mut sequence,
                                        FrameValues {
                                            source_value_count: values.len(),
                                            evaluation: &mut evaluation,
                                            values: &mut values,
                                            next_value: &mut next_value,
                                            next_block: &mut next_block,
                                            next_edge: &mut next_edge,
                                            operations: &mut operations,
                                        },
                                        |mut frame| frame.guard_value(identities, guard_ordinal),
                                    )?;
                                    Some(condition.id)
                                }
                            } else {
                                None
                            };
                            let body_end = operations.len();
                            let case_bindings =
                                evaluation.completed_bindings(&operations, std::iter::empty());
                            let case_sources = case_emission::SourceContext {
                                checked,
                                state,
                                parameters: state_parameters,
                                bindings: &case_bindings,
                            };
                            let prepared_cases = case_emission::prepare(
                                identities,
                                &case_sources,
                                self.catalogs,
                                &operations,
                                &mut next_value,
                            )?;
                            let guarded_return = super::super::guarded::emit(
                                identities,
                                checked,
                                state,
                                self.catalogs,
                                state_parameters,
                                self.claims.published(),
                                &mut evaluation,
                                &mut values,
                                &self.state_erased[position],
                                &mut next_value,
                                &mut next_block,
                                &mut next_edge,
                                &mut operations,
                                owner,
                                &mut sequence,
                            )?;
                            let is_guarded_return = guarded_return.is_some();
                            let mut scalar_return_exits = Vec::new();
                            let mut scalar_return = match &state.terminator {
                                CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                                    completion: checked_trees::CheckedScalarReturnPlan::Exits(exits),
                                } => {
                                    let cleanups = super::super::cleanup::scalar_returns(
                                        identities,
                                        self.claims.published(),
                                        self.checked,
                                        state,
                                        exits,
                                        self.catalogs,
                                        state_parameters,
                                        &evaluation,
                                        &operations,
                                    )?;
                                    scalar_return_exits.extend(cleanups.iter().map(|row| row.exit));
                                    let value = StateFrame::new(
                                        checked,
                                        owner,
                                        state,
                                        &self.state_erased[position],
                                    )
                                    .evaluate(
                                        self.catalogs,
                                        state_parameters,
                                        &mut sequence,
                                        FrameValues {
                                            source_value_count: values.len(),
                                            evaluation: &mut evaluation,
                                            values: &mut values,
                                            next_value: &mut next_value,
                                            next_block: &mut next_block,
                                            next_edge: &mut next_edge,
                                            operations: &mut operations,
                                        },
                                        |frame| {
                                            let (mut environment, emission) = frame.split();
                                            emission.evaluation.scalar_control_result(
                                                identities,
                                                checked,
                                                owner,
                                                state.state,
                                                exits,
                                                &cleanups,
                                                emission.values,
                                                emission.next_value,
                                                emission.next_block,
                                                emission.next_edge,
                                                emission.operations,
                                                emission.calls,
                                                &mut environment,
                                            )
                                        },
                                    )?;
                                    Some(value)
                                }
                                CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                                    completion:
                                        checked_trees::CheckedScalarReturnPlan::Binding(binding),
                                } => {
                                    let slot = usize::try_from(binding.binding_ordinal)
                                        .ok()
                                        .and_then(|ordinal| {
                                            ordinal.checked_add(state.scalar_parameters.len())
                                        })
                                        .ok_or(LoweringError::Unsupported(
                                            "Unit graph scalar return binding ordinal overflows",
                                        ))?;
                                    let position =
                                        evaluation.scalar_bindings.immutable_position(slot)?;
                                    let value = values.get(position).copied().ok_or(
                                        LoweringError::Unsupported(
                                            "Unit graph scalar return binding is absent",
                                        ),
                                    )?;
                                    Some(value)
                                }
                                _ => None,
                            };
                            let planned_guard = match &branch_guard {
                                Some(expression) => Some(plan_short_circuit_guard(
                                    expression,
                                    &values,
                                    &self.catalogs.structural_types,
                                    &evaluation.structural_parameters,
                                    evaluation.shared_qualifications.catalog(),
                                    &mut next_value,
                                    first_successor_reads_payloads,
                                )?),
                                None => None,
                            };
                            let inherited_lengths = operations.storage_observations.clone();
                            let (guarded_decisions, guarded_drafts, guarded_first_namespace) =
                                if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
                                    arms,
                                    ..
                                } = &state.terminator
                                {
                                    self.stage_guarded_chain(
                                        identities,
                                        position,
                                        arms,
                                        &mut values,
                                        &mut evaluation,
                                        &mut next_value,
                                        &mut next_block,
                                        &mut next_edge,
                                        &mut operations,
                                        state_parameters,
                                        &sequence,
                                    )?
                                } else {
                                    (Vec::new(), Vec::new(), Vec::new())
                                };
                            let mut conditional_return_edge =
                if let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    return_arm,
                    return_when_true,
                    ..
                } = &state.terminator
                {
                    let arm_namespace = match &planned_guard {
                        Some((_, namespace)) if *return_when_true => namespace.clone(),
                        _ => values.clone(),
                    };
                    Some(self.conditional_return_arm(
                        identities,
                        position,
                        state_parameters,
                        arm_namespace,
                        match &planned_guard {
                            Some((planned, _)) if *return_when_true => {
                                planned.established.as_slice()
                            }
                            _ => &[],
                        },
                        &[],
                        &[],
                        return_arm,
                        &mut evaluation,
                        &mut next_value,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &sequence,
                    )?)
                } else {
                    None
                };
                            let mut completion_edges =
                if let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                    when_true,
                    when_false,
                    ..
                } = &state.terminator
                {
                    let true_namespace = match &planned_guard {
                        Some((_, namespace)) => namespace.clone(),
                        None => values.clone(),
                    };
                    let true_edge = self.conditional_return_arm(
                        identities,
                        position,
                        state_parameters,
                        true_namespace,
                        planned_guard
                            .as_ref()
                            .map_or(&[], |(guard, _)| guard.established.as_slice()),
                        &[],
                        &[],
                        when_true,
                        &mut evaluation,
                        &mut next_value,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &sequence,
                    )?;
                    let false_edge = self.conditional_return_arm(
                        identities,
                        position,
                        state_parameters,
                        values.clone(),
                        &[],
                        &[],
                        &[],
                        when_false,
                        &mut evaluation,
                        &mut next_value,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &sequence,
                    )?;
                    Some((true_edge, false_edge))
                } else {
                    None
                };
                            let mut shared_return_blocks = Vec::new();
                            let closed_return_edges = if let Some(prepared) = &prepared_cases {
                                prepared
                                    .cases
                                    .iter()
                                    .map(|case| {
                                        match &case.outcome {
                        checked_trees::CheckedClosedSumCaseOutcomePlan::Return(return_arm)
                            if case.values.is_empty()
                                && shared_return_blocks.iter().any(|(ordinal, _)| {
                                    *ordinal == return_arm.statement_ordinal()
                                }) =>
                        {
                            let (_, target) = shared_return_blocks
                                .iter()
                                .find(|(ordinal, _)| *ordinal == return_arm.statement_ordinal())
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph shared return arm lost its block",
                                ))?;
                            let staged = block_id(allocate_dense(&mut next_block)?);
                            evaluation.blocks.push(Block::plain(
                                staged,
                                Vec::new(),
                                Terminator::Jump {
                                    edge: edge_id(allocate_dense(&mut next_edge)?),
                                    target: *target,
                                    arguments: Vec::new(),
                                    erased_arguments: Vec::new(),
                                    erased_proof_arguments: Vec::new(),
                                    structural_arguments: Vec::new(),
                                    cleanup_actions: Vec::new(),
                                },
                            ));
                            Ok(Some(value_successor(
                                edge_id(allocate_dense(&mut next_edge)?),
                                staged,
                            )))
                        }
                        checked_trees::CheckedClosedSumCaseOutcomePlan::Return(return_arm) => {
                            let payload_parameters = case
                                .values
                                .iter()
                                .map(|(_, value)| *value)
                                .collect::<Vec<_>>();
                            let mut namespace = values.clone();
                            namespace.extend(&payload_parameters);
                            let established = case
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(ordinal, field)| EstablishedCasePayload {
                                    source: prepared.source,
                                    path: prepared.path.clone(),
                                    case: case.identity,
                                    field: *field,
                                    position: values.len() + ordinal,
                                })
                                .collect::<Vec<_>>();
                            let edge = self.conditional_return_arm(
                                identities,
                                position,
                                state_parameters,
                                namespace,
                                &established,
                                &payload_parameters,
                                &[],
                                return_arm,
                                &mut evaluation,
                                &mut next_value,
                                &mut next_block,
                                &mut next_edge,
                                &mut operations,
                                &sequence,
                            )?;
                            if case.values.is_empty() {
                                shared_return_blocks
                                    .push((return_arm.statement_ordinal(), edge.target));
                            }
                            Ok(Some(edge))
                        }
                        checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(_) => Ok(None),
                    }
                                    })
                                    .collect::<Result<Vec<_>, LoweringError>>()?
                            } else {
                                Vec::new()
                            };
                            let mut guarded_arm_return_edges: Vec<Option<SuccessorEdge>> =
                                if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
                                    arms,
                                    ..
                                } = &state.terminator
                                {
                                    let mut staged = Vec::with_capacity(arms.len());
                                    for (index, arm) in arms.iter().enumerate() {
                                        match arm {
                                            checked_trees::CheckedGuardedJumpArmPlan::Jump(_) => {
                                                staged.push(None)
                                            }
                                            checked_trees::CheckedGuardedJumpArmPlan::Return(
                                                plan,
                                            ) => {
                                                let (namespace, established) = if index == 0 {
                                                    match &planned_guard {
                                                        Some((guard, namespace)) => (
                                                            namespace.clone(),
                                                            guard.established.as_slice(),
                                                        ),
                                                        None => (
                                                            guarded_first_namespace.clone(),
                                                            &[][..],
                                                        ),
                                                    }
                                                } else {
                                                    let draft = &guarded_drafts[index - 1];
                                                    match &draft.4 {
                                                        ChainGuard::Decision((
                                                            guard,
                                                            namespace,
                                                        )) => (
                                                            namespace.clone(),
                                                            guard.established.as_slice(),
                                                        ),
                                                        ChainGuard::Value(_) => {
                                                            (draft.5.clone(), &[][..])
                                                        }
                                                    }
                                                };
                                                staged.push(Some(self.conditional_return_arm(
                                                    identities,
                                                    position,
                                                    state_parameters,
                                                    namespace,
                                                    established,
                                                    &[],
                                                    &[],
                                                    &plan.return_arm,
                                                    &mut evaluation,
                                                    &mut next_value,
                                                    &mut next_block,
                                                    &mut next_edge,
                                                    &mut operations,
                                                    &sequence,
                                                )?));
                                            }
                                        }
                                    }
                                    staged
                                } else {
                                    Vec::new()
                                };
                            let mut guarded_return_edge =
                                if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
                                    fallback:
                                        checked_trees::CheckedGuardedJumpFallbackPlan::Return(
                                            return_arm,
                                        ),
                                    ..
                                } = &state.terminator
                                {
                                    Some(self.conditional_return_arm(
                                        identities,
                                        position,
                                        state_parameters,
                                        values.clone(),
                                        &[],
                                        &[],
                                        &[],
                                        return_arm,
                                        &mut evaluation,
                                        &mut next_value,
                                        &mut next_block,
                                        &mut next_edge,
                                        &mut operations,
                                        &sequence,
                                    )?)
                                } else {
                                    None
                                };
                            let mut ordinary_return_cleanup = match &state.terminator {
                                CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                                    completion: checked_trees::CheckedScalarReturnPlan::Exits(_),
                                } => Some(Vec::new()),
                                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                                | CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                                    ..
                                } => {
                                    let [cleanup] = self
                                        .catalogs
                                        .cleanup_control
                                        .normal_exits(state.normal_exit_cleanups)
                                    else {
                                        return unsupported(
                                            "ordinary return has no unique source cleanup coordinate",
                                        );
                                    };
                                    scalar_return_exits.push(cleanup.exit);
                                    let mut regions = super::super::cleanup::for_exit_regions(
                                        identities,
                                        self.claims.published(),
                                        self.checked,
                                        state,
                                        cleanup.exit,
                                        self.catalogs,
                                        state_parameters,
                                        &evaluation,
                                        &operations,
                                    )?;
                                    super::super::cleanup::selection(
                                        state.state,
                                        &state.operations,
                                        &evaluation,
                                        &operations,
                                        &mut regions,
                                    )?;
                                    let return_position = scalar_return.map(|value| {
                                        values
                                            .iter()
                                            .position(|candidate| candidate.id == value.id)
                                            .unwrap_or_else(|| {
                                                values.push(value);
                                                values.len() - 1
                                            })
                                    });
                                    let actions = evaluation.emit_cleanup_regions(
                                        identities,
                                        self.checked,
                                        regions,
                                        &mut values,
                                        &mut next_value,
                                        &mut next_block,
                                        &mut next_edge,
                                        &mut operations,
                                    )?;
                                    if let Some(position) = return_position {
                                        scalar_return = Some(values[position]);
                                    }
                                    Some(actions)
                                }
                                _ => None,
                            };
                            let mut edge_blocks = Vec::new();
                            macro_rules! short_circuit {
                                ($planned:expr, $values:expr, $edges:expr) => {
                                    emit_short_circuit_decision(
                                        identities,
                                        self.checked,
                                        $planned,
                                        $values,
                                        $edges,
                                        ShortCircuitFrame {
                                            erased_scalar_formals: &self.state_erased[position],
                                            erased_proof_formals: &self.state_erased_proof
                                                [position],
                                            current_rank: current_rank.as_deref(),
                                            inherited_lengths: &inherited_lengths,
                                        },
                                        &mut self.block_ranks,
                                        &mut self.rank_edges,
                                        &mut edge_blocks,
                                        &mut evaluation.blocks,
                                        &mut next_block,
                                        &mut next_value,
                                        &mut next_edge,
                                        &mut operations,
                                    )?
                                };
                            }
                            let frame = super::successor_edge::SuccessorFrame {
                                position,
                                state_parameters,
                                bindings: &bindings,
                                current_rank: current_rank.as_deref(),
                                inherited_lengths: &inherited_lengths,
                                sequence: &sequence,
                            };
                            let mut successor = |edge: &CheckedStructuralControlSuccessorPlan,
                                 payload_values: &[(u32, ValueDeclaration)],
                                 case_edge: bool,
                                 values: &[ValueDeclaration],
                                 established: &[EstablishedCasePayload]|
             -> Result<SuccessorEdge, LoweringError> {
                self.successor_edge(
                    identities,
                    &frame,
                    &mut super::successor_edge::EmittedState {
                        operations: &mut operations,
                        evaluation: &mut evaluation,
                        next_value: &mut next_value,
                        next_block: &mut next_block,
                        next_edge: &mut next_edge,
                        edge_blocks: &mut edge_blocks,
                    },
                    edge,
                    payload_values,
                    case_edge,
                    values,
                    established,
                )
            };
                            let terminator = match &state.terminator {
                CheckedComposedUnitControlTerminatorPlan::CompleteExpression { completion } => {
                    // This branch owns emission; no optional staged edge is joined
                    // back to its source terminator after preparation.
                    let edge = self.completion_expression(
                        identities,
                        position,
                        state_parameters,
                        values.clone(),
                        &[],
                        &[],
                        &[],
                        completion,
                        &mut evaluation,
                        &mut next_value,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &sequence,
                    )?;
                    Terminator::Jump {
                        edge: edge.edge,
                        target: edge.target,
                        arguments: edge.arguments,
                        structural_arguments: edge.structural_arguments,
                        erased_arguments: edge.erased_arguments,
                        erased_proof_arguments: edge.erased_proof_arguments,
                        cleanup_actions: edge.cleanup_actions,
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal { .. }
                | CheckedComposedUnitControlTerminatorPlan::EvaluateNoNormal => {
                    return unsupported("nonreturning state lost its evaluated invocation");
                }

                CheckedComposedUnitControlTerminatorPlan::Guarded { .. } => guarded_return.ok_or(
                    LoweringError::Unsupported("guarded structural emission absent"),
                )?,
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                    let source = match result.source {
                                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                            binding_ordinal,
                                        } => case_emission::result(state, binding_ordinal, &operations)?.place(),
                                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                            parameter_index,
                                        } => {
                                            state_parameters
                                                .get(parameter_index as usize)
                                                .ok_or(LoweringError::Unsupported(
                                                    "structural graph return parameter missing",
                                                ))?
                                                .place
                                        }
                                        _ => return unsupported("structural graph return needs a whole owned value"),
                                    };
                    let source = terminal_psi::StructuralArgument {
                        place: source,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    };
                    let returned_claims = crate::unit::attached_unit::claims::lower_return_claims(
                        identities,
                        checked,
                        owner,
                        state.state,
                        result,
                        &source,
                        &operations,
                        &evaluation.current_claims,
                        &self.claims.source_claims,
                        lookup_type_id(&self.catalogs.type_ids, &result.type_identity)?,
                        &self.catalogs.structural_types,
                    )?;
                    let identity_parameters = self.state_contract_rosters[position]
                        .iter()
                        .map(|(_, parameter)| parameter.clone())
                        .collect::<Vec<_>>();
                    let state_identities = crate::unit::attached_unit::claims::lower_result_identity(
                        checked,
                        owner,
                        state.state,
                        &state.structural_parameters,
                        &identity_parameters,
                        &self.machine_result,
                        &self.claims.source_claims,
                    )?;
                    if let Some(entry) = self.content_entry_claims.iter().find(|entry| {
                        returned_claims
                            .iter()
                            .any(|binding| binding.claim == entry.claim)
                            && !state_identities.iter().any(|identity| {
                                identity.claim == entry.claim
                                    && identity.input == entry.input
                                    && identity.projections == entry.projections
                            })
                    }) {
                        let reason = if state_identities.is_empty() {
                            "structural return lost its checked content identity guarantee: returning state has no checked content rows"
                        } else if !state_identities
                            .iter()
                            .any(|identity| identity.claim == entry.claim)
                        {
                            "structural return lost its checked content identity guarantee: no checked row names the returned claim"
                        } else if !state_identities
                            .iter()
                            .any(|identity| identity.claim == entry.claim && identity.input == entry.input)
                        {
                            "structural return lost its checked content identity guarantee: checked row names another input place"
                        } else {
                            "structural return lost its checked content identity guarantee: checked projections disagree"
                        };
                        return unsupported(reason);
                    }
                    let invocation_identities = state_identities
                        .into_iter()
                        .filter(|identity| {
                            self.content_entry_claims.iter().any(|entry| {
                                identity.claim == entry.claim
                                    && identity.input == entry.input
                                    && identity.projections == entry.projections
                            })
                        })
                        .collect::<Vec<_>>();
                    if let Some(common) = &mut self.content_identity_reshuffles {
                        let keys = invocation_identities
                            .iter()
                            .map(|identity| {
                                (
                                    identity.claim,
                                    &identity.input,
                                    &identity.output,
                                    &identity.projections,
                                )
                            })
                            .collect::<std::collections::BTreeSet<_>>();
                        common.retain(|identity| {
                            keys.contains(&(
                                identity.claim,
                                &identity.input,
                                &identity.output,
                                &identity.projections,
                            ))
                        });
                    } else {
                        self.content_identity_reshuffles = Some(invocation_identities);
                    }
                    Terminator::ReturnStructural {
                        edge: edge_id(allocate_dense(&mut next_edge)?),
                        source: source.place,
                        returned_claims,
                        cleanup_actions: Vec::new(),
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                    let cleanup_actions =
                        ordinary_return_cleanup
                            .take()
                            .ok_or(LoweringError::Unsupported(
                                "ordinary return lost its selected cleanup actions",
                            ))?;
                    let edge = edge_id(allocate_dense(&mut next_edge)?);
                    match (&state.terminator, scalar_return) {
                        (
                            CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. },
                            Some(value),
                        ) => Terminator::Return {
                            edge,
                            value: value.id,
                            cleanup_actions,
                        },
                        (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, None) => {
                            Terminator::ReturnUnit {
                                edge,
                                cleanup_actions,
                            }
                        }
                        _ => return unsupported("Unit graph return lost its scalar value"),
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::Crash { statement_ordinal } => {
                    self.crash_completion(identities, position, *statement_ordinal, &mut next_edge)?
                }
                CheckedComposedUnitControlTerminatorPlan::Jump { successor: edge } => {
                    let edge = successor(edge, &[], false, &values, &[])?;
                    Terminator::Jump {
                        edge: edge.edge,
                        target: edge.target,
                        arguments: edge.arguments,
                        erased_arguments: edge.erased_arguments,
                        erased_proof_arguments: edge.erased_proof_arguments,
                        structural_arguments: edge.structural_arguments,
                        cleanup_actions: edge.cleanup_actions,
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. } => {
                    let (when_true, when_false) = match &state.terminator {
                        CheckedComposedUnitControlTerminatorPlan::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => (
                            match &planned_guard {
                                Some((planned, namespace)) => successor(
                                    when_true,
                                    &[],
                                    false,
                                    namespace,
                                    &planned.established,
                                )?,
                                None => successor(when_true, &[], false, &values, &[])?,
                            },
                            successor(when_false, &[], false, &values, &[])?,
                        ),
                        CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                            jump,
                            return_when_true,
                            ..
                        } => {
                            let jump_edge = match &planned_guard {
                                Some((planned, namespace)) if !*return_when_true => {
                                    successor(jump, &[], false, namespace, &planned.established)?
                                }
                                _ => successor(jump, &[], false, &values, &[])?,
                            };
                            let return_edge = conditional_return_edge.take().ok_or(
                                LoweringError::Unsupported(
                                    "Unit graph conditional return lost its staged edge",
                                ),
                            )?;
                            if *return_when_true {
                                (return_edge, jump_edge)
                            } else {
                                (jump_edge, return_edge)
                            }
                        }
                        CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                            ..
                        } => completion_edges.take().ok_or(LoweringError::Unsupported(
                            "Unit graph conditional completion lost its staged edges",
                        ))?,
                        _ => unreachable!(),
                    };
                    if let Some((planned, guard_values)) = &planned_guard {
                        short_circuit!(planned, guard_values, [when_true, when_false])
                    } else {
                        Terminator::Conditional {
                            condition: condition.ok_or(LoweringError::Unsupported(
                                "Unit graph conditional lost its guard",
                            ))?,
                            when_true,
                            when_false,
                        }
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                    let mut selected = Vec::with_capacity(arms.len());
                    for (index, arm) in arms.iter().enumerate() {
                        let plan = match arm {
                            checked_trees::CheckedGuardedJumpArmPlan::Jump(plan) => plan,
                            checked_trees::CheckedGuardedJumpArmPlan::Return(_) => {
                                selected.push(guarded_arm_return_edges[index].take().ok_or(
                                    LoweringError::Unsupported(
                                        "guarded jump arm lost its staged return edge",
                                    ),
                                )?);
                                continue;
                            }
                        };
                        let planned = if index == 0 {
                            planned_guard.as_ref()
                        } else {
                            match &guarded_drafts[index - 1].4 {
                                ChainGuard::Decision(planned) => Some(planned),
                                ChainGuard::Value(_) => None,
                            }
                        };
                        selected.push(match planned {
                            Some((planned, namespace)) => successor(
                                &plan.successor,
                                &[],
                                false,
                                namespace,
                                &planned.established,
                            )?,
                            None => {
                                let namespace = if index == 0 {
                                    guarded_first_namespace.as_slice()
                                } else {
                                    guarded_drafts[index - 1].5.as_slice()
                                };
                                successor(&plan.successor, &[], false, namespace, &[])?
                            }
                        });
                    }
                    let mut selected = selected.into_iter();
                    let first = selected.next().ok_or(LoweringError::Unsupported(
                        "guarded jump chain lost its first successor",
                    ))?;
                    let mut fallback_edge = Some(match fallback {
                        checked_trees::CheckedGuardedJumpFallbackPlan::Successor(fallback) => {
                            successor(fallback, &[], false, &values, &[])?
                        }
                        checked_trees::CheckedGuardedJumpFallbackPlan::Return(_) => {
                            guarded_return_edge
                                .take()
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph guarded return lost its staged edge",
                                ))?
                        }
                    });
                    let draft_count = guarded_drafts.len();
                    for (
                        index,
                        (id, parameters, structural_parameters, operations_draft, guard, _),
                    ) in guarded_drafts.into_iter().enumerate()
                    {
                        let when_false = if index + 1 == draft_count {
                            fallback_edge.take().ok_or(LoweringError::Unsupported(
                                "guarded jump chain lost its fallback edge",
                            ))?
                        } else {
                            decision_edge(
                                guarded_decisions[index + 1],
                                self.state_erased_proof[position].len(),
                                current_rank.as_deref(),
                                &mut self.rank_edges,
                                &mut next_edge,
                            )?
                        };
                        let when_true = selected.next().ok_or(LoweringError::Unsupported(
                            "guarded jump chain lost an ordered successor",
                        ))?;
                        let terminator = match guard {
                            ChainGuard::Value(condition) => Terminator::Conditional {
                                condition,
                                when_true,
                                when_false,
                            },
                            ChainGuard::Decision((planned, namespace)) => {
                                short_circuit!(&planned, &namespace, [when_true, when_false])
                            }
                        };
                        if let Some(rank) = current_rank.as_ref() {
                            self.block_ranks.insert(id, rank.clone());
                        }
                        edge_blocks.push(Block {
                            id,
                            parameters,
                            erased_scalar_formals: self.state_erased[position].clone(),
                            erased_proof_formals: erased_proof_formal_declarations(
                                identities,
                                self.checked,
                                &self.state_erased_proof[position],
                            ),
                            structural_parameters,
                            operations: operations_draft,
                            terminator,
                        });
                    }
                    let first_false = decision_edge(
                        guarded_decisions[0],
                        self.state_erased_proof[position].len(),
                        current_rank.as_deref(),
                        &mut self.rank_edges,
                        &mut next_edge,
                    )?;
                    match &planned_guard {
                        Some((planned, namespace)) => {
                            short_circuit!(planned, namespace, [first, first_false])
                        }
                        None => Terminator::Conditional {
                            condition: condition.ok_or(LoweringError::Unsupported(
                                "guarded jump chain lost its first guard",
                            ))?,
                            when_true: first,
                            when_false: first_false,
                        },
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } => {
                    let prepared = prepared_cases.as_ref().ok_or(LoweringError::Unsupported(
                        "Unit graph case terminator lost its prepared payloads",
                    ))?;
                    let cases = prepared
                                        .cases
                                        .iter()
                                        .zip(closed_return_edges)
                                        .map(|(case, return_edge)| {
                                            let edge = match &case.outcome {
                                                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                                    successor_plan,
                                                ) => {
                                                    let established = case
                                                        .values
                                                        .iter()
                                                        .zip(&case.fields)
                                                        .enumerate()
                                                        .map(|(ordinal, ((_, _), field))| {
                                                            crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload {
                                                                source: prepared.source,
                                                                path: prepared.path.clone(),
                                                                case: case.identity,
                                                                field: *field,
                                                                position: values.len() + ordinal,
                                                            }
                                                        })
                                                        .collect::<Vec<_>>();
                                                    successor(
                                                        successor_plan,
                                                        &case.values,
                                                        true,
                                                        &values,
                                                        &established,
                                                    )?
                                                }
                                                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(_) => {
                                                    return_edge.ok_or(LoweringError::Unsupported(
                                                        "Unit graph returning case lost its edge",
                                                    ))?
                                                }
                                            };
                                            Ok(StructuralCaseSuccessorEdge {
                     structural_arguments: Vec::new(),
                                                edge: edge.edge,
                                                target: edge.target,
                                                case: case.identity,
                                                payload_fields: case.fields.clone(),
                                                cleanup_actions: Vec::new(),
                                            })
                                        })
                                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    Terminator::StructuralCase {
                        source: prepared.source,
                        path: prepared.path.clone(),
                        cases,
                    }
                }
            };
                            if let Terminator::Return { edge, .. } = terminator {
                                operations.source_returns.push(SourceScalarReturn {
                                    edge,
                                    exits: scalar_return_exits,
                                });
                            }
                            sequence.finish_state(checked, state)?;
                            let dispatches = condition.is_some()
                                || branch_guard.is_some()
                                || prepared_cases.is_some()
                                || matches!(
                                    state.terminator,
                                    CheckedComposedUnitControlTerminatorPlan::CompleteExpression { .. }
                                );
                            self.seal_state(
                                identities,
                                position,
                                terminator,
                                edge_blocks,
                                evaluation,
                                operations,
                                super::seal::SealFrame {
                                    state_parameters,
                                    current_rank: current_rank.as_deref(),
                                    is_guarded_return,
                                    dispatches,
                                    body_end,
                                    next_value,
                                    next_block,
                                    next_edge,
                                },
                            )
                        }
                    }

                    pub(crate) fn successor_may_read_payloads(
                        successor: &checked_trees::CheckedStructuralControlSuccessorPlan,
                    ) -> bool {
                        successor.scalar_arguments.iter().any(|argument| {
            argument.source == checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
        })
                    }

                    pub(crate) fn guarded_arm_may_read_payloads(
                        arm: &checked_trees::CheckedGuardedJumpArmPlan,
                    ) -> bool {
                        match arm {
                            checked_trees::CheckedGuardedJumpArmPlan::Jump(plan) => {
                                successor_may_read_payloads(&plan.successor)
                            }
                            checked_trees::CheckedGuardedJumpArmPlan::Return(plan) => {
                                return_may_read_payloads(&plan.return_arm)
                            }
                        }
                    }

                    fn return_may_read_payloads(
                        arm: &checked_trees::CheckedConditionalReturnArm,
                    ) -> bool {
                        !matches!(
                            arm,
                            checked_trees::CheckedConditionalReturnArm::Unit { .. }
                                | checked_trees::CheckedConditionalReturnArm::Crash { .. }
                        )
                    }

                    pub(crate) fn plan_short_circuit_guard(
                        expression: &LoweredBooleanReturnExpression,
                        values: &[ValueDeclaration],
                        structural_types: &[terminal_psi::StructuralTypeDeclaration],
                        structural_parameters: &[(
                            u32,
                            terminal_psi::StructuralParameterDeclaration,
                        )],
                        qualifications: &terminal_psi::ScalarQualificationCatalog,
                        next_value: &mut u64,
                        successor_reads_payloads: bool,
                    ) -> Result<(PlannedGuard, Vec<ValueDeclaration>), LoweringError>
                    {
                        let decision =
                            crate::emission::boolean_control::lower_boolean_control_decision(
                                expression,
                                LoweredBooleanDecision::Value(
                                    LoweredBooleanReturnExpression::Constant { value: true },
                                ),
                                LoweredBooleanDecision::Value(
                                    LoweredBooleanReturnExpression::Constant { value: false },
                                ),
                            );
                        let mut namespace = values.to_vec();
                        let declared_cases = |place| {
                            let (_, parameter) = structural_parameters
                                .iter()
                                .find(|(_, parameter)| parameter.place == place)?;
                            match &terminal_psi::find_structural_type(
                                structural_types,
                                parameter.structural_type,
                            )?
                            .shape
                            {
                                terminal_psi::StructuralTypeShape::Sum { cases }
                                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                                    Some(cases.as_slice())
                                }
                                _ => None,
                            }
                        };
                        let planned = crate::emission::case_payload_dispatch::plan(
                            decision,
                            &declared_cases,
                            qualifications,
                            &mut namespace,
                            next_value,
                            successor_reads_payloads,
                        )?;
                        Ok((planned, namespace))
                    }

                    pub(crate) enum ChainGuard {
                        Value(semantic_vocabulary::ValueId),
                        Decision((PlannedGuard, Vec<ValueDeclaration>)),
                    }

                    struct ShortCircuitFrame<'a> {
                        erased_scalar_formals: &'a [ValueDeclaration],
                        erased_proof_formals:
                            &'a [checked_trees::CheckedErasedProofParameterPlan],
                        current_rank: Option<&'a [semantic_vocabulary::ValueId]>,
                        inherited_lengths: &'a StorageObservations,
                    }

                    #[allow(clippy::too_many_arguments)]
                    fn emit_short_circuit_decision(
                        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                        checked: &checked_trees::CheckedTrees,
                        planned: &PlannedGuard,
                        guard_values: &[ValueDeclaration],
                        [when_true, when_false]: [SuccessorEdge; 2],
                        frame: ShortCircuitFrame<'_>,
                        block_ranks: &mut std::collections::BTreeMap<
                            semantic_vocabulary::BlockId,
                            Vec<semantic_vocabulary::ValueId>,
                        >,
                        rank_edges: &mut std::collections::BTreeMap<
                            semantic_vocabulary::EdgeId,
                            (
                                Vec<semantic_vocabulary::ValueId>,
                                terminal_psi::TerminalNaturalRankComparison,
                            ),
                        >,
                        edge_blocks: &mut Vec<Block>,
                        decision_blocks: &mut Vec<Block>,
                        next_block: &mut u64,
                        next_value: &mut u64,
                        next_edge: &mut u64,
                        operations: &mut OperationBuffer,
                    ) -> Result<Terminator, LoweringError> {
                        operations
                            .storage_observations
                            .clone_from(frame.inherited_lengths);
                        let true_block = block_id(allocate_dense(next_block)?);
                        let false_block = block_id(allocate_dense(next_block)?);
                        for (id, successor) in [(true_block, when_true), (false_block, when_false)]
                        {
                            if let Some(rank) = frame.current_rank {
                                block_ranks.insert(id, rank.to_vec());
                            }
                            edge_blocks.push(Block {
                                id,
                                parameters: Vec::new(),
                                erased_scalar_formals: frame.erased_scalar_formals.to_vec(),
                                erased_proof_formals: erased_proof_formal_declarations(
                                    identities,
                                    checked,
                                    frame.erased_proof_formals,
                                ),
                                structural_parameters: Vec::new(),
                                operations: Vec::new(),
                                terminator: Terminator::Jump {
                                    edge: successor.edge,
                                    target: successor.target,
                                    arguments: successor.arguments,
                                    erased_arguments: successor.erased_arguments,
                                    erased_proof_arguments: successor
                                        .erased_proof_arguments
                                        .clone(),
                                    structural_arguments: successor.structural_arguments,
                                    cleanup_actions: successor.cleanup_actions,
                                },
                            });
                        }
                        let decision = &planned.decision;
                        let tests =
                            crate::emission::boolean_control::boolean_guard_decision_block_count(
                                decision,
                            );
                        let decision_block = block_id(*next_block);
                        *next_block = next_block
                            .checked_add(u64::try_from(tests).map_err(|_| {
                                LoweringError::Unsupported(
                                    "guard decision count exceeds identities",
                                )
                            })?)
                            .ok_or(LoweringError::Unsupported(
                                "guard decision identities overflow",
                            ))?;
                        let (root, nested) =
                            crate::emission::boolean_control::emit_inlined_boolean_guard_blocks(
                                decision,
                                guard_values,
                                Vec::new(),
                                &crate::emission::boolean_control::LoweredBooleanDecisionTarget {
                                    block: true_block,
                                    arguments: Vec::new(),
                                },
                                &crate::emission::boolean_control::LoweredBooleanDecisionTarget {
                                    block: false_block,
                                    arguments: Vec::new(),
                                },
                                decision_block,
                                block_id(decision_block.get().checked_add(1).ok_or(
                                    LoweringError::Unsupported(
                                        "guard decision identities overflow",
                                    ),
                                )?),
                                next_value,
                                next_edge,
                                operations,
                            );
                        decision_blocks.push(root);
                        decision_blocks.extend(nested);
                        let edge = edge_id(allocate_dense(next_edge)?);
                        if let Some(rank) = frame.current_rank {
                            rank_edges.insert(
                                edge,
                                (
                                    rank.to_vec(),
                                    terminal_psi::TerminalNaturalRankComparison::Preserving,
                                ),
                            );
                        }
                        Ok(Terminator::Jump {
                            edge,
                            target: decision_block,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        })
                    }

                    fn decision_edge(
                        target: semantic_vocabulary::BlockId,
                        erased_proof_count: usize,
                        current_rank: Option<&[semantic_vocabulary::ValueId]>,
                        rank_edges: &mut std::collections::BTreeMap<
                            semantic_vocabulary::EdgeId,
                            (
                                Vec<semantic_vocabulary::ValueId>,
                                terminal_psi::TerminalNaturalRankComparison,
                            ),
                        >,
                        next_edge: &mut u64,
                    ) -> Result<SuccessorEdge, LoweringError> {
                        let edge = edge_id(allocate_dense(next_edge)?);
                        if let Some(rank) = current_rank {
                            rank_edges.insert(
                                edge,
                                (
                                    rank.to_vec(),
                                    terminal_psi::TerminalNaturalRankComparison::Preserving,
                                ),
                            );
                        }
                        Ok(SuccessorEdge {
                            edge,
                            target,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: (0..erased_proof_count)
                                .map(|position| semantic_vocabulary::ProofTerm::Formal {
                                    position: u32::try_from(position)
                                        .expect("erased-proof roster positions fit u32"),
                                })
                                .collect(),
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        })
                    }
                }
                pub(crate) mod successor_edge {

                    use super::super::super::super::super::{
                        StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
                        SuccessorEdge, ValueId, block_id,
                    };
                    use super::super::super::super::{
                        Block, CheckedScalarExpression, CheckedScalarExpressionRole, Operation,
                        OperationKind, OperationResult, StructuralMultiplicity,
                        StructuralPlaceDeclaration, StructuralPlaceKind, Terminator,
                        ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
                        edge_id, emit_direct_expression, place_id, terminal_scalar_type,
                        unsupported, validate_direct_parameter_types,
                    };
                    use super::super::super::LoweringError;
                    use super::super::{
                        CheckedStructuralControlSuccessorPlan, case_emission, case_leaf_copy,
                        ranking, scalars, subslices,
                    };
                    use super::StateGraphEmission;
                    use crate::emission::borrowed_window::terminal_multiplicity;
                    use crate::emission::operation_emission::buffer::{
                        OperationBuffer, StorageObservations, structural_result_declaration,
                    };
                    use crate::emission::operation_emission::place_declarations::operation_result_place;
                    use crate::expression_preparation::bindings::ScalarBindings;
                    use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
                    use crate::expression_preparation::bindings::structural_fields::establish_case_payloads;
                    use crate::proofs::crash_routes::lowered_erased_scalar_term;
                    use crate::scalar_lowering::contracts::lowered_proof_term;
                    use crate::shared::checked_facts::value_facts;
                    use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
                    use crate::shared::terminal_identities::lookup_type_id;
                    use crate::unit::attached_unit::composed_control::emission::{
                        FrameValues, StateFrame, with_call_namespace,
                    };

                    impl StateGraphEmission<'_, '_> {
                        pub(crate) fn successor_edge(
                            &mut self,
                            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                            frame: &SuccessorFrame<'_>,
                            emitted: &mut EmittedState<'_>,
                            edge: &CheckedStructuralControlSuccessorPlan,
                            payload_values: &[(u32, ValueDeclaration)],
                            case_edge: bool,
                            values: &[ValueDeclaration],
                            established: &[EstablishedCasePayload],
                        ) -> Result<SuccessorEdge, LoweringError> {
                            let checked = self.checked;
                            let plan = self.plan;
                            let SuccessorFrame {
                                position,
                                state_parameters,
                                bindings,
                                current_rank,
                                inherited_lengths,
                                sequence,
                            } = *frame;
                            let state = &plan.states[position];
                            let mut sequence = sequence.branch();
                            let arrival_decided = checked.facts.call_requires_decided(
                                state.state,
                                edge.statement_ordinal,
                                edge.target_state,
                            );
                            let owner = self.admitted.authored[position].0.symbol;
                            let EmittedState {
                                operations,
                                evaluation,
                                next_value,
                                next_block,
                                next_edge,
                                edge_blocks,
                            } = emitted;
                            let current_parameters =
                                evaluation.current_parameters(state_parameters);
                            let state_parameters = current_parameters.as_ref();
                            operations
                                .storage_observations
                                .clone_from(inherited_lengths);
                            let target =
                                plan.successor_position(edge)
                                    .ok_or(LoweringError::Unsupported(
                                        "Unit graph target disappeared during emission",
                                    ))?;
                            let operation_start = operations.len();
                            let staged = block_id(allocate_dense(next_block)?);
                            let mut edge_evaluation = evaluation.branch(staged, operation_start);
                            let mut edge_values = values.to_vec();
                            for (_, value) in payload_values {
                                edge_values.push(*value);
                            }
                            establish_case_payloads(
                                &mut edge_evaluation.structural_fields,
                                established,
                            );
                            edge_evaluation.parameters =
                                payload_values.iter().map(|(_, value)| *value).collect();
                            let copied_values = self.capture_dispatch_copies(
                                identities,
                                position,
                                edge.statement_ordinal,
                                state_parameters,
                                &mut edge_evaluation,
                                &mut edge_values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                                &mut sequence,
                            )?;
                            let case_bindings =
                                edge_evaluation.completed_bindings(operations, std::iter::empty());
                            let mut arguments = Vec::new();
                            let mut structural_arguments = Vec::new();
                            let mut retiring_loan_carriers = Vec::new();
                            let mut capture_parameters = Vec::new();
                            let mut captured_loans = Vec::new();
                            let target_state = &plan.states[target];
                            for transfer in &edge.transfers {
                                for (dependent, parent) in &transfer.retained_loan_parents {
                                    let dependent = edge_evaluation.value_storage(
                                        dependent,
                                        state_parameters,
                                        |binding| {
                                            Ok(case_emission::result(state, binding, operations)?
                                                .as_ref()
                                                .into())
                                        },
                                    )?;
                                    let parent = edge_evaluation.value_storage(
                                        parent,
                                        state_parameters,
                                        |binding| {
                                            Ok(case_emission::result(state, binding, operations)?
                                                .as_ref()
                                                .into())
                                        },
                                    )?;
                                    if !dependent.path.is_empty() || !parent.path.is_empty() {
                                        return unsupported(
                                            "state loan parent is not a complete owned home",
                                        );
                                    }
                                    let relation = (dependent.place, parent.place);
                                    if relation.0 != relation.1
                                        && !self.retained_loan_parents.contains(&relation)
                                    {
                                        self.retained_loan_parents.push(relation);
                                    }
                                }
                            }
                            let lane_bound = target_state
                                .structural_parameters
                                .iter()
                                .map(|parameter| parameter.position)
                                .chain(
                                    target_state
                                        .scalar_parameters
                                        .iter()
                                        .map(|p| p.source_position),
                                )
                                .chain(
                                    target_state
                                        .erased_scalar_parameters
                                        .iter()
                                        .map(|p| p.source_position),
                                )
                                .chain(
                                    target_state
                                        .erased_proof_parameters
                                        .iter()
                                        .map(|p| p.source_position),
                                )
                                .map(|position| position as usize + 1)
                                .max()
                                .unwrap_or(0);
                            for argument_position in 0..lane_bound {
                                let runtime_row =
                                    target_state.structural_parameters.iter().any(|parameter| {
                                        parameter.position as usize == argument_position
                                    }) || target_state.scalar_parameters.iter().any(|parameter| {
                                        parameter.source_position as usize == argument_position
                                    });
                                if !runtime_row {
                                    let erased_row = target_state
                                        .erased_scalar_parameters
                                        .iter()
                                        .any(|parameter| {
                                            parameter.source_position as usize == argument_position
                                        })
                                        || target_state.erased_proof_parameters.iter().any(
                                            |parameter| {
                                                parameter.source_position as usize
                                                    == argument_position
                                            },
                                        );
                                    if erased_row {
                                        continue;
                                    }
                                }
                                if let Some((target_index, (target_parameter, transfer))) =
                                    target_state
                                        .structural_parameters
                                        .iter()
                                        .zip(&edge.transfers)
                                        .enumerate()
                                        .find(|(_, (parameter, _))| {
                                            parameter.position as usize == argument_position
                                        })
                                {
                                    if let Some(root) = self
                                        .admitted
                                        .entry_aliases
                                        .get(target)
                                        .and_then(|aliased| aliased.get(&(target_index as u32)))
                                    {
                                        let entry_place = self
                                            .parameters
                                            .get(*root as usize)
                                            .map(|parameter| parameter.place);
                                        let source_place = match transfer.source {
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                                                index,
                                            } => state_parameters
                                                .get(index as usize)
                                                .map(|parameter| parameter.place),
                                            _ => {
                                                return unsupported(
                                                    "Unit graph aliased successor is not a whole parameter transfer",
                                                );
                                            }
                                        };
                                        if source_place != entry_place {
                                            return unsupported(
                                                "Unit graph aliased successor lost its entry place",
                                            );
                                        }
                                        continue;
                                    }
                                    let mut argument_path = Vec::new();
                                    let mut completed_binding = None;
                                    let mut captured_backing = None;
                                    let loan_backing =
                                        super::super::result_custody::transferred_loan_backing(
                                            checked,
                                            &state.operations,
                                            transfer,
                                        );
                                    if loan_backing.is_some()
                                        && !transfer.incoming_loan_resources.is_empty()
                                    {
                                        return unsupported(
                                            "state loan ingress contradicts local owned backing",
                                        );
                                    }
                                    let mut loan_backing_home = None;
                                    let mut place = match transfer.source {
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { operations: ref selected } => {
                                                let state_bindings = operations.structural_values.clone();
                                                let planned = selected.iter().cloned().collect::<Vec<_>>();
                                                let emitted = super::super::super::emission::emit_call_operations(
                                                    identities, checked, owner, state, &planned, self.catalogs,
                                                    state_parameters, &mut edge_evaluation, &mut edge_values,
                                                    &self.state_erased[position], next_value, next_block, next_edge,
                                                    operations, &mut sequence,
                                                );
                                                operations.structural_values = state_bindings;
                                                emitted?;
                                                let terminator = operations.take_invocation_completion(next_edge)?
                                                    .ok_or(LoweringError::Unsupported("stopped successor operand lost its exact invocation"))?;
                                                let selection_edge = edge_id(allocate_dense(next_edge)?);
                                                if let Some(rank) = current_rank {
                                                    self.block_ranks.insert(staged, rank.to_vec());
                                                    self.block_ranks.insert(edge_evaluation.current, rank.to_vec());
                                                    self.block_ranks.extend(edge_evaluation.blocks.iter().map(|block| (block.id, rank.to_vec())));
                                                    self.preserve_edges(edge_evaluation.blocks.iter().flat_map(|block| block.terminator.edges()), rank);
                                                    self.rank_edges.insert(selection_edge, (rank.to_vec(), terminal_psi::TerminalNaturalRankComparison::Preserving));
                                                }
                                                edge_blocks.extend(edge_evaluation.blocks);
                                                edge_blocks.push(Block {
                                                    id: edge_evaluation.current,
                                                    parameters: edge_evaluation.parameters,
                                                    erased_scalar_formals: self.state_erased[position].clone(),
                                                    erased_proof_formals: crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                                                        identities, checked, &self.state_erased_proof[position],
                                                    ),
                                                    structural_parameters: edge_evaluation.block_structural_parameters,
                                                    operations: operations[edge_evaluation.operation_start..].to_vec(),
                                                    terminator,
                                                });
                                                return Ok(SuccessorEdge {
                                                    edge: selection_edge, target: staged, arguments: Vec::new(),
                                                    erased_arguments: Vec::new(),
                                                    erased_proof_arguments: (0..self.state_erased_proof[position].len()).map(|position| {
                                                        semantic_vocabulary::ProofTerm::Formal {
                                                            position: u32::try_from(position).expect("erased-proof roster positions fit u32"),
                                                        }
                                                    }).collect(),
                                                    structural_arguments: Vec::new(), cleanup_actions: Vec::new(),
                                                });
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { ref operands, ref establishment, ref presentation } => {
                                                let Some(result) = establishment.structural_result_binding() else {
                                                    return unsupported(
                                                        "Unit graph established argument has no result binding",
                                                    );
                                                };
                                                let state_bindings = operations.structural_values.clone();
                                                let selected_operations = operands.iter()
                                                    .chain(std::iter::once(establishment.as_ref()))
                                                    .cloned().collect::<Vec<_>>();
                                                let emitted = super::super::super::emission::emit_call_operations(
                                                    identities,
                                                    checked,
                                                    owner,
                                                    state,
                                                    &selected_operations,
                                                    self.catalogs,
                                                    state_parameters,
                                                    &mut edge_evaluation,
                                                    &mut edge_values,
                                                    &self.state_erased[position],
                                                    next_value,
                                                    next_block,
                                                    next_edge,
                                                    operations,
                                                    &mut sequence,
                                                )
                                                .and_then(|()| {
                                                    case_emission::result(state, result.binding_ordinal, operations)
                                                        .map(|binding| binding.into_owned())
                                                        .and_then(|binding| {
                                                            let place = binding.place();
                                                            let suffix = crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                                                identities, &presentation.path, binding.structural_type(),
                                                                &self.catalogs.structural_types,
                                                            )?;
                                                            let prepared = binding.path().iter().cloned()
                                                                .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                                                .chain(suffix).collect::<Vec<_>>();
                                                            argument_path = with_call_namespace(
                                                                &mut self.catalogs.scalar_calls,
                                                                |calls| crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                                                    &prepared, place,
                                                                    Some(&edge_values), &mut std::iter::empty(), Some(calls),
                                                                    Some((next_value, operations)),
                                                                ),
                                                            )?;
                                                            if super::super::result_custody::loan_subject(&transfer.source, &checked.facts.values.structural_values).is_some() {
                                                                let source = operations.reference_backings.source(place).or_else(|| {
                                                                    if std::env::var_os("OMEGA_STATE_REFERENCE_DIAGNOSTICS").is_some() {
                                                                        let producer = operations.iter().find(|operation| operation.result.structural().is_some_and(|result| result.place == place));
                                                                        eprintln!("state reference backing unavailable: place={place:?} binding={binding:?} producer={:?}", producer.map(|operation| (&operation.id, &operation.kind)));
                                                                    }
                                                                    None
                                                                }).ok_or(LoweringError::Unsupported("completed state reference lost its emitted formation"))?;
                                                                captured_backing = Some((source.place, source.path.clone()));
                                                                if let Some(group) = self.captured_loans.group(target, target_index as u32) {
                                                                    captured_loans.push((group, place));
                                                                }
                                                            }
                                                            if let Some(backing) = loan_backing {
                                                                let home = case_emission::result(state, backing.binding, operations)?.into_owned();
                                                                captured_backing = Some((
                                                                    home.place(),
                                                                    home.path().to_vec(),
                                                                ));
                                                                loan_backing_home = Some(home);
                                                            }
                                                            completed_binding = Some(binding);
                                                            Ok(place)
                                                        })
                                                });
                                                operations.structural_values = state_bindings;
                                                emitted?
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
                                                let binding = case_emission::result(state, binding_ordinal, operations)?;
                                                argument_path = binding.path().to_vec();
                                                binding.place()
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                                state_parameters.get(index as usize).ok_or(
                                                    LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                                                )?.place
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(_) => {
                                                let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                                                let emitted = subslices::emit(identities, checked, state, edge.statement_ordinal, &transfer.source, state_parameters, &mut edge_evaluation, destination, bindings, &mut edge_values, next_value, next_block, next_edge, operations, self.catalogs, &mut self.structural_places,
                                                    owner, &self.state_erased[position], &mut sequence,
                                                )?;
                                                self.structural_places.push(emitted);
                                                destination
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                                                ref subject,
                                                backing_type,
                                                ref source_contract,
                                                formation_expression,
                                                carrier_type_identity,
                                                ..
                                            } => {
                                                let storage = edge_evaluation.value_storage(
                                                    &subject.source, state_parameters, |binding| {
                                                        let produced = case_emission::result(state, binding, operations)?;
                                                        Ok(produced.as_ref().into())
                                                    },
                                                )?;
                                                let mut selectors = Vec::new();
                                                                            let source_value_count = edge_values.len();
                                                                            for segment in &subject.path {
                                                                                if let checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(handle) = segment {
                                                                                    let root = value_facts(checked)
                                                                                        .scalar_computations
                                                                                        .roots
                                                                                        .iter()
                                                                                        .map(|(_, root)| root)
                                                                                        .find(|root| {
                                                                                            root.root == *handle
                                                                                                && root.machine == owner
                                                                                                && root.state == state.state
                                                                                                && root.statement_ordinal == edge.statement_ordinal
                                                                                                && matches!(root.role, CheckedScalarExpressionRole::TransitionSelector {
                                                                                                argument_ordinal, ..
                                                                                            } if argument_ordinal as usize == argument_position)
                                                                                        })
                                                                                        .ok_or(LoweringError::Unsupported(
                                                                                            "state loan selector lost its exact source occurrence",
                                                                                        ))?;
                                                                                    let value = StateFrame::new(
                                                                                        checked,
                                                                                        owner,
                                                                                        state,
                                                                                        &self.state_erased[position],
                                                                                    )
                                                                                    .evaluate(
                                                                                        self.catalogs,
                                                                                        state_parameters,
                                                                                        &mut sequence,
                                                                                        FrameValues {
                                                                                            source_value_count,
                                                                                            evaluation: &mut edge_evaluation,
                                                                                            values: &mut edge_values,
                                                                                            next_value,
                                                                                            next_block,
                                                                                            next_edge,
                                                                                            operations,
                                                                                        },
                                                                                        |mut frame| {
                                                                                            frame.source_value(
                                                                                                identities,
                                                                                                edge.statement_ordinal,
                                                                                                root.role,
                                                                                                &checked_trees::CheckedCallScalarArgument::Computation(
                                                                                                    *handle,
                                                                                                ),
                                                                                            )
                                                                                        },
                                                                                    )?;
                                                                                    selectors.push(value.id);
                                                                                }
                                                                            }
                                                                            let suffix =
                                                                                crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                                                                    identities,
                                                                                    &subject.path,
                                                                                    storage.selected_type,
                                                                                    &self.catalogs.structural_types,
                                                                                )?;
                                                                            let root = storage.place;
                                                                            let prepared = storage.path.into_iter()
                                                                                .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                                                                .chain(suffix).collect::<Vec<_>>();
                                                                            let path = with_call_namespace(
                                                                                &mut self.catalogs.scalar_calls,
                                                                                |calls| crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                                                                    &prepared, root, Some(&edge_values), &mut selectors.into_iter(),
                                                                                    Some(calls), Some((next_value, operations)),
                                                                                ),
                                                                            )?;
                                                let target_carrier = self.captured_loans.parent_carrier(target, target_index as u32);
                                                let whole_parameter = subject.path.is_empty()
                                                    && matches!(subject.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. });
                                                if (carrier_type_identity.is_valid() || target_carrier.is_some() && !whole_parameter)
                                                    && let Some(group) = self.captured_loans.group(target, target_index as u32)
                                                {
                                                    let carrier_type = if carrier_type_identity.is_valid() {
                                                        lookup_type_id(
                                                            &self.catalogs.type_ids, &carrier_type_identity,
                                                        )?
                                                    } else {
                                                        target_carrier.ok_or(LoweringError::Unsupported("captured header lost its descriptor declaration"))?.structural_type
                                                    };
                                                    captured_backing = Some((root, path.clone()));
                                                    let (source_place, source_path) = match subject.source {
                                                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                                                            if let Some(parent) = self.captured_loans.parent_carrier(position, parameter_index) => {
                                                                let mut parent_path = if terminal_psi::find_structural_type(
                                                                    &self.catalogs.structural_types, parent.structural_type,
                                                                ).is_some_and(|declaration| matches!(declaration.shape,
                                                                    terminal_psi::StructuralTypeShape::Reference { .. })) {
                                                                    vec![terminal_psi::StructuralPathSegment::Referent]
                                                                } else { Vec::new() };
                                                                parent_path.extend(path);
                                                                (parent.place, parent_path)
                                                            }
                                                        _ => (root, path),
                                                    };
                                                    let source = StructuralArgument {
                                                        place: source_place, path: source_path,
                                                        access: crate::expression_preparation::bindings::structural_paths::terminal_access(subject.access),
                                                    };
                                                    let first_operand_operation = operations.len();
                                                    let carrier = crate::unit::attached_unit::reference_results::emit_carrier(
                                                        identities,
                                                        crate::unit::attached_unit::reference_results::Formation {
                                                            checked,
                                                            statement_index: edge.statement_ordinal as usize,
                                                            expression: formation_expression,
                                                            backing_type,
                                                            source_contract,
                                                            qualification_decisions: Default::default(),
                                                            domain_ids: &self.catalogs.domain_ids,
                                                        },
                                                        carrier_type, source, crate::unit::attached_unit::reference_results::PublishedReferenceStorage::Declared, &self.catalogs.structural_types,
                                                        &edge_evaluation,
                                                        place_id(allocate_dense(&mut self.catalogs.next_place)?), operations,
                                                    )?;
                                                    edge_evaluation.current_claims.publish_operations(
                                                        &operations[first_operand_operation..],
                                                    )?;
                                                    captured_loans.push((group, carrier.id));
                                                    self.structural_places.push(carrier);
                                                    argument_path = if terminal_psi::find_structural_type(
                                                        &self.catalogs.structural_types, carrier_type,
                                                    ).is_some_and(|declaration| matches!(declaration.shape,
                                                        terminal_psi::StructuralTypeShape::Reference { .. })) {
                                                        vec![terminal_psi::StructuralPathSegment::Referent]
                                                    } else { Vec::new() };
                                                    carrier.id
                                                } else {
                                                    argument_path = path;
                                                    root
                                                }
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                                                ref subject, ref case_identity, ref field_identity, ref path,
                                            } => {
                                                let context = case_emission::SourceContext {
                                                    checked,
                                                    state,
                                                    parameters: state_parameters,
                                                    bindings: &case_bindings,
                                                };
                                                let source = case_emission::source(
                                                    identities, &context, self.catalogs, subject, operations,
                                                )?;
                                                let (source, root_type, subject_type) = (
                                                    source.argument,
                                                    source.root_type,
                                                    source.declaration.id,
                                                );
                                                if matches!(subject.access, checked_trees::CheckedStructuralAccess::WriteOnlyBorrow) {
                                                    return unsupported(
                                                        "Unit graph case-payload subject is write-only",
                                                    );
                                                }
                                                let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                                                let first_operand_operation = operations.len();
                                                self.structural_places.push(case_leaf_copy::emit(
                                                    identities,
                                                    &self.catalogs.structural_types,
                                                    state_parameters,
                                                    source.place,
                                                    root_type,
                                                    subject_type,
                                                    &source.path,
                                                    case_identity,
                                                    field_identity,
                                                    path,
                                                    target_parameter,
                                                    destination,
                                                    operations,
                                                )?);
                                                edge_evaluation.current_claims.publish_operations(
                                                    &operations[first_operand_operation..],
                                                )?;
                                                destination
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                                                ref bytes,
                                            } => {
                                                let destination =
                                                    place_id(allocate_dense(&mut self.catalogs.next_place)?);
                                                let structural_type = lookup_type_id(
                                                    &self.catalogs.type_ids,
                                                    &target_parameter.type_identity,
                                                )?;
                                                let declaration_ordinal =
                                                    crate::emission::next_byte_sequence_literal_ordinal(
                                                        &self.catalogs.temporary_places,
                                                    )?;
                                                self.catalogs.temporary_places.push(
                                                    StructuralPlaceDeclaration {
                                                        id: destination,
                                                        kind: StructuralPlaceKind::ByteSequenceLiteral {
                                                            declaration_ordinal,
                                                            structural_type,
                                                        },
                                                    },
                                                );
                                                let qualifications = target_parameter
                                                    .qualifications
                                                    .iter()
                                                    .map(|domain| {
                                                        lookup_domain_id_for_carrier(
                                                            &self.catalogs.domain_ids,
                                                            *domain,
                                                            structural_type,
                                                        )
                                                    })
                                                    .collect::<Result<Vec<_>, _>>()?;
                                                let first_operand_operation = operations.len();
                                                let producer = operations.allocate();
                                                operations.push(Operation::unsuspended(
                                                    producer,
                                                    OperationResult::Unit,
                                                    OperationKind::EstablishByteSequenceLiteral {
                                                        destination,
                                                        bytes: bytes.clone(),
                                                        qualifications,
                                                    },
                                                ));
                                                edge_evaluation.current_claims.publish_operations(
                                                    &operations[first_operand_operation..],
                                                )?;
                                                destination
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
                                                ref subject,
                                            } => {
                                                let storage = edge_evaluation.value_storage(
                                                    &subject.source, state_parameters, |binding| {
                                                        let produced = case_emission::result(state, binding, operations)?;
                                                        Ok(produced.as_ref().into())
                                                    },
                                                )?;
                                                let source = storage.place;
                                                let suffix = crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                                    identities, &subject.path, storage.selected_type, &self.catalogs.structural_types,
                                                )?;
                                                let prepared = storage.path.into_iter()
                                                    .map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static)
                                                    .chain(suffix).collect::<Vec<_>>();
                                                let path = with_call_namespace(
                                                    &mut self.catalogs.scalar_calls,
                                                    |calls| crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                                                        &prepared, source, Some(&edge_values), &mut std::iter::empty(),
                                                        Some(calls), Some((next_value, operations)),
                                                    ),
                                                )?;
                                                if target_parameter.multiplicity != language_semantics::Multiplicity::Unrestricted {
                                                    argument_path = path;
                                                    source
                                                } else {
                                                let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                                                let structural_type = lookup_type_id(
                                                    &self.catalogs.type_ids,
                                                    &target_parameter.type_identity,
                                                )?;
                                                let first_operand_operation = operations.len();
                                                let producer = operations.allocate();
                                                operations.push(Operation::unsuspended(
                                                    producer,
                                                    structural_result_declaration(
                                                        destination,
                                                        structural_type,
                                                        StructuralMultiplicity::Unrestricted,
                                                    ),
                                                    OperationKind::StructuralLeafCopy { source, path },
                                                ));
                                                self.catalogs.temporary_places.push(
                                                    operation_result_place(destination, producer, structural_type),
                                                );
                                                edge_evaluation.current_claims.publish_operations(
                                                    &operations[first_operand_operation..],
                                                )?;
                                                destination
                                                }
                                            }
                                        };
                                    for parent in &transfer.retained_operand_parents {
                                        let parent = edge_evaluation.value_storage(
                                            parent,
                                            state_parameters,
                                            |binding| {
                                                Ok(case_emission::result(
                                                    state, binding, operations,
                                                )?
                                                .as_ref()
                                                .into())
                                            },
                                        )?;
                                        if !parent.path.is_empty() {
                                            return unsupported(
                                                "state operand parent is not a complete owned home",
                                            );
                                        }
                                        let dependent = self.state_contract_rosters[target]
                                            .iter()
                                            .find(|(position, _)| {
                                                *position == target_parameter.position
                                            })
                                            .map(|(_, parameter)| parameter.place)
                                            .ok_or(LoweringError::Unsupported(
                                                "state operand parent lost its target carrier",
                                            ))?;
                                        let relation = (dependent, parent.place);
                                        if relation.0 != relation.1
                                            && !self.retained_loan_parents.contains(&relation)
                                        {
                                            self.retained_loan_parents.push(relation);
                                        }
                                    }
                                    if let Some(backing) = loan_backing
                                        && loan_backing_home.is_none()
                                    {
                                        let home = case_emission::result(
                                            state,
                                            backing.binding,
                                            operations,
                                        )?
                                        .into_owned();
                                        captured_backing =
                                            Some((home.place(), home.path().to_vec()));
                                        loan_backing_home = Some(home);
                                    }
                                    let chain_arrival =
                                        self.loan_chains.arrival(target, target_index as u32);
                                    if chain_arrival.is_some() {
                                        let parameter = match &transfer.source {
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                                                index,
                                            } => Some(*index),
                                            _ => super::super::result_custody::loan_subject(
                                                &transfer.source,
                                                &checked.facts.values.structural_values,
                                            )
                                            .and_then(|(subject, _)| match subject.source {
                                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                                    parameter_index,
                                                } => Some(parameter_index),
                                                _ => None,
                                            }),
                                        };
                                        if let Some(index) = parameter
                                            && state
                                                .structural_parameters
                                                .get(index as usize)
                                                .is_some_and(|parameter| {
                                                    parameter.access
                                        == checked_trees::CheckedStructuralAccess::Owned
                                                })
                                        {
                                            let home = state_parameters
                                                .get(index as usize)
                                                .map_or(place, |param| param.place);
                                            self.loan_chains.lend(
                                                target,
                                                target_index as u32,
                                                home,
                                            )?;
                                        }
                                        let lender = if let Some(home) = loan_backing_home.as_ref()
                                        {
                                            Some(std::borrow::Cow::Borrowed(home))
                                        } else {
                                            match &transfer.source {
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { .. } => {
                                                if let Some(binding) = super::super::result_custody::loan_subject(&transfer.source, &checked.facts.values.structural_values)
                                                    .and_then(|(subject, _)| subject.source.current_result_binding(&state.operations)) {
                                                    Some(case_emission::result(state, binding, operations)?)
                                                } else {
                                                    Some(std::borrow::Cow::Borrowed(completed_binding.as_ref().ok_or(
                                                        LoweringError::Unsupported("Unit graph completed loan lost its emitted home"),
                                                    )?))
                                                }
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
                                                subject.source.current_result_binding(&state.operations)
                                                    .map(|binding| case_emission::result(state, binding, operations))
                                                    .transpose()?
                                            }
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
                                                Some(case_emission::result(state, *binding_ordinal, operations)?)
                                            }
                                            _ => None,
                                        }
                                        };
                                        if let Some(result) = lender {
                                            if !result.claims().is_empty()
                                || !result.path().is_empty()
                                || (chain_arrival
                                    == Some(super::loan_chains::ChainArrival::Transferred)
                                    && !captured_backing
                                        .as_ref()
                                        .map_or(&argument_path, |(_, path)| path)
                                        .is_empty())
                            {
                                return unsupported(
                                    "Unit graph state loan chain home is not plain whole storage",
                                );
                            }
                                            self.loan_chains.lend(
                                                target,
                                                target_index as u32,
                                                captured_backing
                                                    .as_ref()
                                                    .map_or(place, |(backing, _)| *backing),
                                            )?;
                                        }
                                    }
                                    if chain_arrival.is_none()
                                        && transfer.incoming_loan_resources.is_empty()
                                        && matches!(
                            target_parameter.access,
                            checked_trees::CheckedStructuralAccess::MutableBorrow
                                | checked_trees::CheckedStructuralAccess::SharedBorrow
                        ) {
                                        let loaned_binding = loan_backing.map(|backing| backing.binding).or_else(|| match &transfer.source {
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => Some(*binding_ordinal),
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { .. }
                                            | checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue { .. } =>
                                                super::super::result_custody::loan_subject(&transfer.source, &checked.facts.values.structural_values)
                                                    .and_then(|(subject, _)| subject.source.current_result_binding(&state.operations)),
                                            checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(argument) => {
                                                crate::unit::attached_unit::view_ranges::range(argument).and_then(|range| {
                                                    match range.root {
                                                        checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                                                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                                                                .current_result_binding(&state.operations)
                                                        }
                                                        _ => None,
                                                    }
                                                })
                                            }
                                            _ => None,
                                        });
                                        if let Some(binding_ordinal) = loaned_binding {
                                            let result = case_emission::result(
                                                state,
                                                binding_ordinal,
                                                operations,
                                            )?;
                                            let backing =
                                                super::super::result_custody::local_loan_backing(
                                                    checked,
                                                    state,
                                                    binding_ordinal,
                                                )
                                                .map(|backing| {
                                                    case_emission::result(
                                                        state, backing, operations,
                                                    )
                                                    .map(|home| (backing, home))
                                                })
                                                .transpose()?
                                                .filter(|(_, home)| {
                                                    home.multiplicity()
                                                        == StructuralMultiplicity::Affine
                                                });
                                            let needs_home = matches!(
                                target_parameter.access,
                                checked_trees::CheckedStructuralAccess::MutableBorrow
                                    | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                            ) || result.multiplicity()
                                                == StructuralMultiplicity::Affine
                                                || backing.is_some();
                                            let subslice_view = matches!(
                                                transfer.source,
                                                checked_trees::CheckedStructuralControlTransferSourcePlan::ViewSubslice(
                                                    _
                                                )
                                            );
                                            let lent_home = if subslice_view {
                                                result.place()
                                            } else {
                                                captured_backing
                                                    .as_ref()
                                                    .map_or(place, |(backing, _)| *backing)
                                            };
                                            let unrestricted_subslice = subslice_view
                                                && result.multiplicity()
                                                    != StructuralMultiplicity::Affine
                                                && backing.is_none();
                                            if needs_home
                                                && !unrestricted_subslice
                                                && (position != 0 || self.entry_reentered)
                                            {
                                                return unsupported(
                                                    "Unit graph local loan requires an activation entry home",
                                                );
                                            }
                                            let homes = (result.multiplicity()
                                                == StructuralMultiplicity::Affine)
                                                .then_some((binding_ordinal, lent_home))
                                                .into_iter()
                                                .chain(backing.map(|(backing, home)| {
                                                    (backing, home.place())
                                                }));
                                            for (binding, home) in homes {
                                                if let Some(previous) =
                                                    self.loaned_local_homes.insert(binding, home)
                                                    && previous != home
                                                {
                                                    return unsupported(
                                                        "Unit graph local loan is lent from two entry homes",
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    if chain_arrival
                                        == Some(super::loan_chains::ChainArrival::Transferred)
                                        && let Some(backing) = loan_backing
                                    {
                                        let carrier =
                            backing.retiring_carrier.ok_or(LoweringError::Unsupported(
                                "Unit graph transferred loan still requires its captured address",
                            ))?;
                                        let descriptor =
                                            case_emission::result(state, carrier, operations)?;
                                        if descriptor.place() != place
                                            || !descriptor.path().is_empty()
                                            || argument_path.as_slice()
                                                != [terminal_psi::StructuralPathSegment::Referent]
                                        {
                                            return unsupported(
                                                "Unit graph loan retirement lost its exact whole carrier",
                                            );
                                        }
                                        retiring_loan_carriers.push(place);
                                        let home = loan_backing_home
                            .as_ref()
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph transferred loan lost its independent owned backing",
                            ))?;
                                        place = home.place();
                                        argument_path.clear();
                                    }
                                    let mut capture = StructuralParameterDeclaration {
                                        place,
                                        position: u32::try_from(capture_parameters.len()).map_err(
                                            |_| {
                                                LoweringError::Unsupported(
                                                    "cleanup capture structural roster exceeds u32",
                                                )
                                            },
                                        )?,
                                        structural_type: lookup_type_id(
                                            &self.catalogs.type_ids,
                                            &target_parameter.type_identity,
                                        )?,
                                        multiplicity: terminal_multiplicity(
                                            target_parameter.multiplicity,
                                        ),
                                        access: StructuralAccess::Owned,
                                        qualifications: Vec::new(),
                                        projected_qualifications: Vec::new(),
                                        additional_input_qualifications: Vec::new(),
                                    };
                                    self.loan_chains.declare(
                                        target,
                                        target_index as u32,
                                        &mut capture,
                                    );
                                    let endpoint = capture.structural_type;
                                    capture_parameters.push(capture);
                                    let mut argument = StructuralArgument {
                        place,
                        path: argument_path,
                        access: match target_parameter.access {
                            _ if chain_arrival
                                == Some(super::loan_chains::ChainArrival::Transferred) =>
                            {
                                StructuralAccess::Owned
                            }
                            _ if self.admitted.exclusive_handoffs.get(target).is_some_and(
                                |promoted| promoted.contains(&(target_index as u32)),
                            ) =>
                            {
                                StructuralAccess::MutableBorrow
                            }
                            checked_trees::CheckedStructuralAccess::Owned => {
                                StructuralAccess::Owned
                            }
                            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                                StructuralAccess::MutableBorrow
                            }
                            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                                StructuralAccess::WriteOnlyBorrow
                            }
                            checked_trees::CheckedStructuralAccess::SharedBorrow => {
                                StructuralAccess::SharedBorrow
                            }
                        },
                    };
                                    if argument.access != StructuralAccess::Owned {
                                        let root = edge_evaluation
                            .structural_root_parameters()
                            .find(|parameter| parameter.place == place)
                            .map(|parameter| parameter.structural_type)
                            .or_else(|| {
                                operations
                                    .structural_result_contract(place)
                                    .map(|result| result.structural_type)
                            })
                            .or_else(|| {
                                self.structural_places
                                    .iter()
                                    .chain(&self.catalogs.temporary_places)
                                    .find(|declaration| declaration.id == place)
                                    .and_then(|declaration| {
                                        declaration.kind.produced_structural_type()
                                    })
                            })
                            .ok_or(LoweringError::Unsupported(
                                "state argument lost its published source declaration",
                            ))?;
                                        crate::expression_preparation::bindings::structural_paths::complete_emitted_argument_path(
                                            root, &mut argument.path, endpoint, argument.access,
                                            &self.catalogs.structural_types,
                                        )?;
                                    }
                                    structural_arguments.push(argument);
                                    continue;
                                }
                                if let Some((scalar_position, _)) =
                                    target_state.scalar_parameters.iter().enumerate().find(
                                        |(_, parameter)| {
                                            parameter.source_position as usize == argument_position
                                        },
                                    )
                                    && let Some((_, payload)) = payload_values
                                        .iter()
                                        .find(|(position, _)| *position as usize == scalar_position)
                                {
                                    arguments.push(payload.id);
                                    continue;
                                }
                                let transfer = edge
                                    .scalar_arguments
                                    .iter()
                                    .find(|transfer| {
                                        transfer.argument_ordinal as usize == argument_position
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "Unit graph successor argument position missing",
                                    ))?;
                                let expression = match transfer.source {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                        index,
                    } => bindings.expression(
                        identities,
                        &checked.facts.proven_scalar_indexes,
                        &CheckedScalarExpression::Parameter {
                            position: index as usize,
                            primitive_type: transfer.primitive_type,
                        },
                    )?,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                        let value = scalars::successor_value(checked, state, edge, transfer)?;
                        let value =
                            StateFrame::new(checked, owner, state, &self.state_erased[position])
                                .evaluate(
                                    self.catalogs,
                                    state_parameters,
                                    &mut sequence,
                                    FrameValues {
                                        source_value_count: edge_values.len(),
                                        evaluation: &mut edge_evaluation,
                                        values: &mut edge_values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                    },
                                    |mut frame| {
                                        frame.source_value(
                                            identities,
                                            edge.statement_ordinal,
                                            CheckedScalarExpressionRole::TransitionArgument {
                                                argument_ordinal: transfer.argument_ordinal,
                                            },
                                            &value,
                                        )
                                    },
                                )?;
                        if value.scalar_type != terminal_scalar_type(transfer.primitive_type)? {
                            return unsupported("Unit graph successor value has the wrong carrier");
                        }
                        arguments.push(value.id);
                        continue;
                    }
                };
                                if expression.scalar_type()
                                    != terminal_scalar_type(transfer.primitive_type)?
                                    || direct_expression_contains_short_circuit(&expression)
                                {
                                    return unsupported(
                                        "Unit graph successor needs a matching branch-free value",
                                    );
                                }
                                validate_direct_parameter_types(
                                    &expression,
                                    &edge_values
                                        .iter()
                                        .map(|value| value.scalar_type)
                                        .collect::<Vec<_>>(),
                                )?;
                                arguments.push(emit_direct_expression(
                                    &expression,
                                    &edge_values,
                                    next_value,
                                    operations,
                                ));
                            }
                            let selected_exit = match edge.completion {
                                checked_trees::CheckedSuccessorCompletion::NoNormalInvocation => {
                                    None
                                }
                                checked_trees::CheckedSuccessorCompletion::StateArrival
                                | checked_trees::CheckedSuccessorCompletion::ReturningInvocation => {
                                    super::super::cleanup::optional_coordinate(
                                        self.catalogs.cleanup_control,
                                        state,
                                        edge.statement_ordinal as usize,
                                        false,
                                    )?
                                }
                            };
                            let mut cleanup_regions = if let Some(exit) = selected_exit {
                                let mut cleanup_regions = super::super::cleanup::for_exit_regions(
                                    identities,
                                    self.claims.published(),
                                    self.checked,
                                    state,
                                    exit,
                                    self.catalogs,
                                    state_parameters,
                                    &edge_evaluation,
                                    operations,
                                )?;
                                super::super::cleanup::selection(
                                    state.state,
                                    &state.operations,
                                    &edge_evaluation,
                                    operations,
                                    &mut cleanup_regions,
                                )?;
                                cleanup_regions
                            } else {
                                Vec::new()
                            };
                            let erased_arguments = edge
                                .erased_arguments
                                .iter()
                                .map(|argument| {
                                    let expression = bindings.erased_actual_at(
                                        identities,
                                        checked,
                                        state.state,
                                        edge.statement_ordinal,
                                        CheckedScalarExpressionRole::TransitionArgument {
                                            argument_ordinal: argument.argument_ordinal,
                                        },
                                    )?;
                                    lowered_erased_scalar_term(
                                        &expression,
                                        &edge_values,
                                        &self.state_erased[position],
                                    )
                                })
                                .collect::<Result<Vec<_>, LoweringError>>()?;
                            let erased_proof_arguments = edge
                                .erased_proof_arguments
                                .iter()
                                .map(|term| {
                                    crate::scalar_lowering::contracts::checked_proof_term(
                                        identities,
                                        checked,
                                        term,
                                        &self.state_erased_proof[position],
                                    )
                                    .and_then(|term| {
                                        lowered_proof_term(
                                            checked,
                                            &term,
                                            &edge_values,
                                            &self.state_erased[position],
                                        )
                                    })
                                })
                                .collect::<Result<Vec<_>, LoweringError>>()?;
                            let arriving_comparison =
                                if plan.preserving_rank_edges.iter().any(|weak| {
                                    weak.source_state == state.state
                                        && weak.statement_ordinal == edge.statement_ordinal
                                        && weak.target_state == edge.target_state
                                }) {
                                    terminal_psi::TerminalNaturalRankComparison::Preserving
                                } else {
                                    terminal_psi::TerminalNaturalRankComparison::Strict
                                };
                            let arriving_rank = if let Some(roots) =
                                ranking::observations(checked, plan, target_state)?
                            {
                                let mut scalar_slots = Vec::with_capacity(arguments.len());
                                for (argument, formal) in
                                    arguments.iter().zip(&target_state.scalar_parameters)
                                {
                                    scalar_slots.push((
                                        edge_values.len(),
                                        terminal_scalar_type(formal.primitive_type)?,
                                    ));
                                    edge_values.push(ValueDeclaration {
                                        id: *argument,
                                        scalar_type: terminal_scalar_type(formal.primitive_type)?,
                                        qualifications: Default::default(),
                                    });
                                }
                                let mut emitted = structural_arguments.iter();
                                let structural_actuals = target_state
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .map(|(dense, formal)| {
                        let argument = if let Some(entry) =
                            self.admitted.entry_aliases[target].get(&(dense as u32))
                        {
                            let retained = self.parameters.get(*entry as usize).ok_or(
                                LoweringError::Unsupported(
                                    "Unit graph observation lost its retained entry alias",
                                ),
                            )?;
                            StructuralArgument {
                                place: retained.place,
                                path: Vec::new(),
                                access: retained.access,
                            }
                        } else {
                            emitted.next().cloned().ok_or(LoweringError::Unsupported(
                                "Unit graph observation lost its completed structural actual",
                            ))?
                        };
                        Ok((
                            argument,
                            lookup_type_id(&self.catalogs.type_ids, &formal.type_identity)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                                if emitted.next().is_some()
                                    || arguments.len() != target_state.scalar_parameters.len()
                                {
                                    return unsupported(
                                        "Unit graph observation has excess completed actuals",
                                    );
                                }
                                let backing = edge_evaluation.completed_bindings(
                                    operations,
                                    self.catalogs
                                        .temporary_places
                                        .iter()
                                        .chain(&self.structural_places)
                                        .cloned(),
                                );
                                let bindings = ranking::formal_bindings(
                                    checked,
                                    target_state,
                                    &backing,
                                    &scalar_slots,
                                    &structural_actuals,
                                )?;
                                let observed = StateFrame::new(
                                    checked,
                                    owner,
                                    state,
                                    &self.state_erased[position],
                                )
                                .evaluate(
                                    self.catalogs,
                                    state_parameters,
                                    &mut sequence,
                                    FrameValues {
                                        source_value_count: edge_values.len(),
                                        evaluation: &mut edge_evaluation,
                                        values: &mut edge_values,
                                        next_value,
                                        next_block,
                                        next_edge,
                                        operations,
                                    },
                                    |frame| {
                                        let (mut environment, emission) = frame.split();
                                        roots
                                            .iter()
                                            .map(|root| {
                                                emission.evaluation.retained_observation(
                                                    identities,
                                                    checked,
                                                    root,
                                                    &bindings,
                                                    emission.values,
                                                    emission.next_value,
                                                    emission.next_block,
                                                    emission.next_edge,
                                                    emission.operations,
                                                    emission.calls,
                                                    &mut environment,
                                                )
                                            })
                                            .collect::<Result<Vec<_>, LoweringError>>()
                                    },
                                )?;
                                if observed
                                    .iter()
                                    .any(|value| value.scalar_type.as_integer().is_none())
                                {
                                    return unsupported(
                                        "Unit graph rank observation lost its integer carrier",
                                    );
                                }
                                Some(observed.iter().map(|value| value.id).collect::<Vec<_>>())
                            } else {
                                None
                            };
                            match (&self.rank_parameters[target][..], arriving_rank.as_ref()) {
                                ([], None) => {}
                                (parameters, Some(values))
                                    if !parameters.is_empty()
                                        && parameters.len() == values.len() =>
                                {
                                    arguments.extend(values.iter().copied());
                                }
                                _ => {
                                    return unsupported(
                                        "ranked arrival lost its complete ordered observations",
                                    );
                                }
                            }
                            sequence.require_closed()?;
                            let owner_arguments = self.captured_loans.arrivals(
                                position,
                                target,
                                edge,
                                &captured_loans,
                            )?;
                            for (argument, mut declaration) in owner_arguments
                                .iter()
                                .zip(self.captured_loans.declarations(target))
                            {
                                declaration.place = argument.place;
                                declaration.position = u32::try_from(capture_parameters.len())
                                    .map_err(|_| {
                                        LoweringError::Unsupported(
                                            "state capture roster exceeds u32",
                                        )
                                    })?;
                                capture_parameters.push(declaration);
                            }
                            structural_arguments.extend(owner_arguments);
                            let target = self.state_ids[target];
                            if cleanup_regions.iter().any(|region| {
                                !region.conditions.is_empty() || region.selected_region.is_some()
                            }) {
                                edge_evaluation.capture_structural_arguments(
                                    identities,
                                    self.checked,
                                    &mut structural_arguments,
                                    &capture_parameters,
                                    &cleanup_regions,
                                    &mut edge_values,
                                    &mut self.catalogs.next_place,
                                    &mut self.catalogs.temporary_places,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                )?;
                            }
                            let copied_regions = self.complete_dispatch_copies(
                                &copied_values,
                                &edge_evaluation,
                                operations,
                            )?;
                            cleanup_regions.splice(0..0, copied_regions);
                            let cleanup_actions = edge_evaluation.emit_cleanup_regions(
                                identities,
                                self.checked,
                                cleanup_regions,
                                &mut edge_values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?;
                            for carrier in retiring_loan_carriers {
                                if structural_arguments
                    .iter()
                    .any(|argument| argument.place == carrier)
                    || !cleanup_actions.iter().any(|action| {
                        matches!(action,
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                                | terminal_psi::TerminalAffineCleanupAction::RetireHome(place)
                                if *place == carrier
                        )
                    })
                {
                    return unsupported(
                        "Unit graph owned loan arrival lacks its selected carrier retirement",
                    );
                }
                            }
                            if case_edge
                                || !established.is_empty()
                                || !edge_evaluation.blocks.is_empty()
                                || operations.len() > operation_start
                            {
                                let backedge = edge_id(allocate_dense(next_edge)?);
                                let selection_edge = edge_id(allocate_dense(next_edge)?);
                                self.arrival_edges.entry(target).or_default().push(backedge);
                                if arrival_decided {
                                    self.decided_arrival_edges.insert(backedge);
                                }
                                if let Some(rank) = current_rank {
                                    self.block_ranks.insert(staged, rank.to_vec());
                                    self.block_ranks
                                        .insert(edge_evaluation.current, rank.to_vec());
                                    self.block_ranks.extend(
                                        edge_evaluation
                                            .blocks
                                            .iter()
                                            .map(|block| (block.id, rank.to_vec())),
                                    );
                                    self.preserve_edges(
                                        edge_evaluation
                                            .blocks
                                            .iter()
                                            .flat_map(|block| block.terminator.edges()),
                                        rank,
                                    );
                                    self.rank_edges.insert(
                                        selection_edge,
                                        (
                                            rank.to_vec(),
                                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                                        ),
                                    );
                                    if let Some(after) = arriving_rank.as_ref() {
                                        self.rank_edges
                                            .insert(backedge, (after.clone(), arriving_comparison));
                                    }
                                }
                                edge_blocks.extend(edge_evaluation.blocks);
                                edge_blocks.push(Block {
                    id: edge_evaluation.current,
                    parameters: edge_evaluation.parameters,
                    erased_scalar_formals: self.state_erased[position].clone(),
                    erased_proof_formals:
                        crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                            identities,
                            self.checked,
                            &self.state_erased_proof[position],
                        ),
                    structural_parameters: edge_evaluation.block_structural_parameters,
                    operations: operations[edge_evaluation.operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge: backedge,
                        target,
                        arguments,
                        structural_arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        cleanup_actions,
                    },
                });
                                Ok(SuccessorEdge {
                                    edge: selection_edge,
                                    target: staged,
                                    arguments: Vec::new(),
                                    erased_arguments: Vec::new(),
                                    erased_proof_arguments: (0..self.state_erased_proof[position]
                                        .len())
                                        .map(|position| semantic_vocabulary::ProofTerm::Formal {
                                            position: u32::try_from(position)
                                                .expect("erased-proof roster positions fit u32"),
                                        })
                                        .collect(),
                                    structural_arguments: Vec::new(),
                                    cleanup_actions: Vec::new(),
                                })
                            } else {
                                let successor_edge = edge_id(allocate_dense(next_edge)?);
                                self.arrival_edges
                                    .entry(target)
                                    .or_default()
                                    .push(successor_edge);
                                if arrival_decided {
                                    self.decided_arrival_edges.insert(successor_edge);
                                }
                                if let Some(after) = arriving_rank.as_ref() {
                                    self.rank_edges.insert(
                                        successor_edge,
                                        (after.clone(), arriving_comparison),
                                    );
                                }
                                Ok(SuccessorEdge {
                                    edge: successor_edge,
                                    target,
                                    arguments,
                                    structural_arguments,
                                    erased_arguments,
                                    erased_proof_arguments,
                                    cleanup_actions,
                                })
                            }
                        }
                    }

                    #[derive(Clone, Copy)]
                    pub(crate) struct SuccessorFrame<'f> {
        pub(crate) position: usize,
        pub(crate) state_parameters: &'f [StructuralParameterDeclaration],
        pub(crate) bindings: &'f ScalarBindings,
        pub(crate) current_rank: Option<&'f [ValueId]>,
        pub(crate) inherited_lengths: &'f StorageObservations,
        pub(crate) sequence:
            &'f crate::unit::attached_unit::composed_control::emission::OperationSequence,
    }

                    pub(crate) struct EmittedState<'e> {
                        pub(crate) operations: &'e mut OperationBuffer,
                        pub(crate) evaluation:
                            &'e mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                        pub(crate) next_value: &'e mut u64,
                        pub(crate) next_block: &'e mut u64,
                        pub(crate) next_edge: &'e mut u64,
                        pub(crate) edge_blocks: &'e mut Vec<Block>,
                    }
                }

                /// One composed-control machine's emission in flight: the checked plan and
                /// admitted graph, the shared catalogs and their identity counters, the per
                /// state block namespaces, and the blocks, ranks and occurrences accumulated
                /// so far. `emit_state` (in `emission/state.rs`) emits one authored state.
                pub(crate) struct StateGraphEmission<'a, 'c> {
                    checked: &'a CheckedTrees,
                    plan: &'a CheckedComposedUnitControlMachinePlan,
                    /// Fused source states emit into this one Terminal machine namespace.
                    terminal_machine: MachineId,
                    admitted: AdmittedGraph<'a>,
                    parameters: Vec<StructuralParameterDeclaration>,
                    scalar_parameters: Vec<ValueDeclaration>,
                    catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
                    machine_result: TerminalMachineResult,
                    claims: LoweredUnitClaims,
                    content_entry_claims: Vec<terminal_psi::ContentEntryClaim>,
                    content_identity_reshuffles:
                        Option<Vec<terminal_psi::ContentIdentityReshuffle>>,
                    loan_completions: Vec<terminal_psi::LoanCompletion>,
                    block_entry_claims: Vec<terminal_psi::EntryClaim>,
                    entry_reentered: bool,
                    invocation_entry: Option<BlockId>,
                    invocation_erased: Vec<ValueDeclaration>,
                    rank_parameters: Vec<Vec<ValueId>>,
                    state_ids: Vec<BlockId>,
                    state_views: Vec<Vec<StructuralParameterDeclaration>>,
                    state_contract_rosters: &'a [Vec<(u32, StructuralParameterDeclaration)>],
                    state_values: Vec<Vec<ValueDeclaration>>,
                    /// Each emitted state's erased-formal roster: the machine contract
                    /// formals for a plain entry, fresh block-level declarations otherwise.
                    state_erased: Vec<Vec<ValueDeclaration>>,
                    /// Each emitted state's erased-proof roster in checked order — the same
                    /// positions the state's own blocks declare and its edges forward.
                    state_erased_proof: Vec<Vec<CheckedErasedProofParameterPlan>>,
                    /// Jump and Conditional edge identities emitted so far, keyed by their
                    /// target block — the exact roster a header invariant must cite.
                    arrival_edges: BTreeMap<BlockId, Vec<EdgeId>>,
                    /// Arrival edges whose source transition checking proved the target's
                    /// whole `requires` contract for (`CheckFacts::call_requires_decided`).
                    decided_arrival_edges: std::collections::BTreeSet<EdgeId>,
                    structural_places: Vec<StructuralPlaceDeclaration>,
                    blocks: Vec<Block>,
                    scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
                    qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
                    loaned_local_homes: BTreeMap<u32, semantic_vocabulary::PlaceId>,
                    retained_loan_parents:
                        Vec<(semantic_vocabulary::PlaceId, semantic_vocabulary::PlaceId)>,
                    /// Exclusive loan chains a later state's local backs.
                    loan_chains: loan_chains::LoanChains,
                    captured_loans: captured_loans::CapturedLoans,
                    occurrences: super::super::ComposedOccurrences,
                    block_ranks: BTreeMap<BlockId, Vec<ValueId>>,
                    rank_edges: BTreeMap<
                        EdgeId,
                        (Vec<ValueId>, terminal_psi::TerminalNaturalRankComparison),
                    >,
                    owners: Vec<symbols::SymbolHandle>,
                    record_fields: Vec<crate::scalar_lowering::computations::fields::Binding>,
                    produced_values:
                        Vec<crate::scalar_lowering::computations::produced_values::Slot>,
                    dynamic_dispatch:
                        std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
                }

                pub(crate) fn emit(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    plan: &CheckedComposedUnitControlMachinePlan,
                    admitted: AdmittedGraph<'_>,
                    terminal_machine: MachineId,
                    parameters: Vec<StructuralParameterDeclaration>,
                    scalar_parameters: Vec<ValueDeclaration>,
                    entry_erased_formals: Vec<ValueDeclaration>,
                    catalogs: &mut catalogs::ComposedCatalogs,
                ) -> Result<
                    (
                        TerminalMachine,
                        super::super::ComposedOccurrences,
                        Vec<terminal_psi::ScalarBlockInvariant>,
                        Vec<terminal_psi::ScalarQualificationCoercion>,
                    ),
                    LoweringError,
                > {
                    let result_places_start = catalogs.result_places.len();
                    let mut structural_places = parameters
                        .iter()
                        .map(|parameter| StructuralPlaceDeclaration {
                            id: parameter.place,
                            kind: StructuralPlaceKind::Parameter {
                                position: parameter.position,
                            },
                        })
                        .collect::<Vec<_>>();
                    let machine_result = returns::result(
                        identities,
                        checked,
                        plan.machine,
                        plan.states[0].state,
                        &plan.result,
                        &plan.result_reference_sources,
                        &parameters,
                        catalogs,
                        &mut structural_places,
                    )?;
                    let entry = &plan.states[0];
                    let mut claims = crate::unit::attached_unit::claims::lower_unit_entry_claims(
                        identities,
                        plan.machine,
                        entry.state,
                        &entry.entry_claims,
                        &parameters,
                        &catalogs.structural_types,
                    )?;
                    // A fused graph retains states authored by sibling machines. Its namespace
                    // reserves only those states' authenticated establishments and completions,
                    // under each declaration's owner. Reservation creates no live custody;
                    // the actual producer establishes it when the retained state executes.
                    let state_owners = plan
                        .states
                        .iter()
                        .map(|state| Ok((state.state, super::owner(checked, state.state)?)))
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    // Every composed state borrows this fixed table. Moved-local rows retain
                    // their source identity and resolve its existing binding during emission.
                    let mut next_claim = u64::try_from(claims.entry_claims.len())
                        .map_err(|_| {
                            LoweringError::Unsupported("Unit entry claim count exceeds u64")
                        })?
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit claim identities exhaust their namespace",
                        ))?;
                    // Reserve destination names before expression/call emission borrows the
                    // namespace. Reservation is not establishment: each block arrival must
                    // bind these names to the claims captured from its outgoing arguments.
                    for (position, state) in plan.states.iter().enumerate() {
                        if !admitted.live[position] {
                            continue;
                        }
                        for entry in &state.entry_claims {
                            if !admitted
                                .claim_transport
                                .aliases
                                .iter()
                                .any(|(alias, _)| *alias == entry.claim_identity)
                                && !claims
                                    .source_claims
                                    .iter()
                                    .any(|(identity, _)| *identity == entry.claim_identity)
                            {
                                let claim = claim_id(allocate_dense(&mut next_claim)?);
                                claims.source_claims.push((entry.claim_identity, claim));
                            }
                        }
                    }
                    let mut unique_states = Vec::with_capacity(state_owners.len());
                    for (state, _) in &state_owners {
                        if !unique_states.contains(state) {
                            unique_states.push(*state);
                        }
                    }
                    let mut permission_handles = Vec::new();
                    for &state in &unique_states {
                        flow_facts(checked)
                            .ownership
                            .with_state_permissions(state, |handles| {
                                permission_handles.extend_from_slice(handles);
                            });
                    }
                    if unique_states.len() > 1 {
                        permission_handles.sort_unstable_by_key(|handle| handle.arena_index());
                        permission_handles.dedup();
                    }
                    for handle in permission_handles {
                        let event = checked.facts.flow.ownership.permissions.get(handle);
                        let statement_index = match event.source {
                            language_semantics::PermissionEventSource::Statement {
                                statement_index,
                            }
                            | language_semantics::PermissionEventSource::Call {
                                statement_index,
                                ..
                            } => statement_index,
                            _ => continue,
                        };
                        let Some((state, machine)) = state_owners
                            .iter()
                            .find(|(state, _)| *state == event.state_symbol)
                        else {
                            continue;
                        };
                        if !claim_established_here(*machine, *state, statement_index, event)
                            || claims
                                .source_claims
                                .iter()
                                .any(|(identity, _)| *identity == event.claim_identity)
                        {
                            continue;
                        }
                        let claim = claim_id(allocate_dense(&mut next_claim)?);
                        claims.source_claims.push((event.claim_identity, claim));
                    }
                    let mut selection_binding_handles = Vec::new();
                    for &state in &unique_states {
                        flow_facts(checked)
                            .ownership
                            .with_state_selection_place_bindings(state, |handles| {
                                selection_binding_handles.extend_from_slice(handles);
                            });
                    }
                    if unique_states.len() > 1 {
                        selection_binding_handles
                            .sort_unstable_by_key(|handle| handle.arena_index());
                        selection_binding_handles.dedup();
                    }
                    for handle in selection_binding_handles {
                        let binding = flow_facts(checked)
                            .ownership
                            .selection_place_bindings
                            .get(handle);
                        if state_owners.iter().any(|(state, machine)| {
                            *state == binding.state && *machine == binding.machine
                        }) && !claims
                            .source_claims
                            .iter()
                            .any(|(identity, _)| *identity == binding.destination.claim_identity)
                        {
                            let claim = claim_id(allocate_dense(&mut next_claim)?);
                            claims
                                .source_claims
                                .push((binding.destination.claim_identity, claim));
                        }
                    }
                    // A structural call result establishes the claims it carries when the
                    // call completes. A received result keeps the receiving local's identity
                    // (or an input's, returned on every exit); a result no place receives is
                    // named by its call and path alone. A state's invocation-storage alias
                    // handed back as a result keeps its invocation binding, joined below.
                    let invocation_alias =
                        |identity: language_semantics::PermissionClaimIdentity| {
                            admitted
                                .claim_transport
                                .aliases
                                .iter()
                                .any(|(successor, _)| *successor == identity)
                        };
                    for (state, (_, owner)) in plan.states.iter().zip(&state_owners) {
                        // Scalar operands retain structural producers in the computation graph,
                        // outside the statement operation list. Reserve both dependency lanes
                        // in this exact state's namespace before either lane emits.
                        let computed_operations =
                            value_call_operations(checked, *owner, Some(state.state))?;
                        for operation in state
                            .operation_dependencies()
                            .flat_map(
                                checked_trees::CheckedUnitEffectOperationPlan::with_value_calls,
                            )
                            .chain(computed_operations)
                        {
                            let Some((call, custody)) = operation
                                .call()
                                .and_then(|call| Some((call, call.structural_custody()?)))
                            else {
                                continue;
                            };
                            for received in &custody.result_claims {
                                if received.claim
                                    == language_semantics::PermissionClaimIdentity::Unknown
                                {
                                    if claims.unreceived_results.iter().any(|claim| {
                                        claim.state == state.state
                                            && claim.coordinate == call.coordinate
                                            && claim.path == received.path
                                    }) {
                                        continue;
                                    }
                                    let claim = claim_id(allocate_dense(&mut next_claim)?);
                                    claims.unreceived_results.push(UnreceivedResultClaim {
                                        state: state.state,
                                        coordinate: call.coordinate,
                                        path: received.path.clone(),
                                        claim,
                                    });
                                } else if !invocation_alias(received.claim)
                                    && !claims
                                        .source_claims
                                        .iter()
                                        .any(|(identity, _)| *identity == received.claim)
                                {
                                    let claim = claim_id(allocate_dense(&mut next_claim)?);
                                    claims.source_claims.push((received.claim, claim));
                                }
                            }
                        }
                    }
                    // Private result joins share the published namespace of ordinary calls.
                    // Reserving a typed path is not establishment; arrivals transport actual IDs.
                    let mut structural_node_handles = Vec::new();
                    for &state in &unique_states {
                        value_facts(checked)
                            .structural_values
                            .with_state_nodes(state, |handles| {
                                structural_node_handles.extend_from_slice(handles);
                            });
                    }
                    if unique_states.len() > 1 {
                        structural_node_handles.sort_unstable_by_key(|handle| handle.arena_index());
                        structural_node_handles.dedup();
                    }
                    for value in structural_node_handles {
                        let node = checked.facts.values.structural_values.nodes.get(value);
                        if !plan.states.iter().any(|state| state.state == node.state)
                            || !matches!(
                                node.kind,
                                checked_trees::CheckedStructuralValueKind::Dispatch { .. }
                                    | checked_trees::CheckedStructuralValueKind::Projection { .. }
                            )
                        {
                            continue;
                        }
                        for path in
                            crate::emission::claim_bindings::completed_value_paths(checked, value)?
                        {
                            claims.completed_values.push(CompletedValueClaim {
                                value,
                                path,
                                claim: claim_id(allocate_dense(&mut next_claim)?),
                            });
                        }
                    }
                    // Proven invocation storage aliases have no block argument or new name.
                    for (successor, root) in &admitted.claim_transport.aliases {
                        let claim = lookup_claim_id(&claims.source_claims, *root)?;
                        claims.source_claims.push((*successor, claim));
                    }
                    catalogs
                        .scalar_calls
                        .bind_claim_namespace(claims.published());
                    let content_entry_claims =
                        crate::proofs::content_conservation::lower_whole_content_entry_claims(
                            identities,
                            checked,
                            &catalogs.structural_types,
                            &entry.structural_parameters,
                            &parameters,
                            &entry.entry_claims,
                            &claims.source_claims,
                        )?;
                    let entry_reentered = plan
                        .states
                        .iter()
                        .zip(&admitted.live)
                        .filter_map(|(state, live)| live.then_some(state))
                        .flat_map(successors)
                        .any(|successor| plan.successor_position(successor) == Some(0));
                    // Invocation parameters are immutable. Reentering the authored entry
                    // therefore uses an ordinary parameterized block behind a one-shot jump.
                    let entry_ranked =
                        ranking::observations(checked, plan, &plan.states[0])?.is_some();
                    let invocation_entry = if entry_reentered || entry_ranked {
                        Some(block_id(allocate_dense(&mut catalogs.next_block)?))
                    } else {
                        None
                    };
                    let mut state_ids = Vec::new();
                    let mut state_views = Vec::new();
                    let mut state_contract_rosters = Vec::new();
                    let mut state_values = Vec::new();
                    let mut state_erased = Vec::new();
                    let mut state_erased_proof = Vec::new();
                    for (position, state) in plan.states.iter().enumerate() {
                        state_ids.push(block_id(allocate_dense(&mut catalogs.next_block)?));
                        if !admitted.live[position] {
                            // Dead positions keep roster alignment but own no emitted block.
                            state_views.push(Vec::new());
                            state_contract_rosters.push(Vec::new());
                            state_values.push(Vec::new());
                            state_erased.push(Vec::new());
                            state_erased_proof.push(Vec::new());
                            continue;
                        }
                        if position != 0 || entry_reentered {
                            let mut block_parameters = lower_unit_parameters(
                                identities,
                                &state.structural_parameters,
                                &catalogs.type_ids,
                                &catalogs.structural_types,
                                &catalogs.domain_ids,
                                &mut catalogs.next_place,
                            )?;
                            let mut block_position = 0_u32;
                            for (dense, parameter) in block_parameters.iter_mut().enumerate() {
                                if let Some(root) = admitted
                                    .entry_aliases
                                    .get(position)
                                    .and_then(|aliased| aliased.get(&(dense as u32)))
                                {
                                    // A forwarded whole entry parameter keeps the entry
                                    // parameter's place; the block does not re-declare it.
                                    parameter.place = parameters
                                        .get(*root as usize)
                                        .ok_or(LoweringError::Unsupported(
                                            "Unit graph entry alias lost its entry parameter",
                                        ))?
                                        .place;
                                } else {
                                    // Exclusive plain storage crosses an edge by address.
                                    // Retain write-only access on the arriving name: moving
                                    // the loan cannot grant observation of its referent.
                                    // Entry aliases are not block parameters. Only
                                    // transferred structural values occupy its dense
                                    // namespace.
                                    parameter.position = block_position;
                                    block_position = block_position.checked_add(1).ok_or(
                                        LoweringError::Unsupported(
                                            "Unit graph block parameter count exceeds u32",
                                        ),
                                    )?;
                                }
                            }
                            structural_places.extend(
                                block_parameters
                                    .iter()
                                    .enumerate()
                                    .filter(|(dense, _)| {
                                        !admitted.entry_aliases.get(position).is_some_and(
                                            |aliased| aliased.contains_key(&(*dense as u32)),
                                        )
                                    })
                                    .map(|(_, parameter)| StructuralPlaceDeclaration {
                                        id: parameter.place,
                                        kind: StructuralPlaceKind::BlockParameter {
                                            block: state_ids[position],
                                            position: parameter.position,
                                        },
                                    }),
                            );
                            // A retained `requires` resolves member operands through the
                            // block's structural decls; `seal` drains this roster into the
                            // block, so the contract namespace keeps its own copy.
                            state_contract_rosters.push(
                                state
                                    .structural_parameters
                                    .iter()
                                    .zip(block_parameters.iter())
                                    .map(|(source, parameter)| (source.position, parameter.clone()))
                                    .collect(),
                            );
                            state_views.push(block_parameters);
                            state_values.push(lower_unit_scalar_parameter_declarations(
                                checked,
                                state.state,
                                &state.scalar_parameters,
                                catalogs.prepared_scalar_qualifications.ok_or(
                                    LoweringError::Unsupported(
                                        "Unit graph state formals have no qualification namespace",
                                    ),
                                )?,
                                &mut catalogs.next_value,
                            )?);
                            state_erased.push(erased_formal_declarations(
                                &state.erased_scalar_parameters,
                                &mut catalogs.next_value,
                            )?);
                            state_erased_proof.push(state.erased_proof_parameters.clone());
                        } else {
                            state_contract_rosters.push(
                                state
                                    .structural_parameters
                                    .iter()
                                    .zip(parameters.iter())
                                    .map(|(source, parameter)| (source.position, parameter.clone()))
                                    .collect(),
                            );
                            state_views.push(parameters.clone());
                            state_values.push(scalar_parameters.clone());
                            // A plain entry shares the machine contract formals; the block
                            // does not redeclare them.
                            state_erased.push(entry_erased_formals.clone());
                            state_erased_proof.push(state.erased_proof_parameters.clone());
                        }
                    }
                    let mut block_entry_claims = Vec::new();
                    // Rank values arrive alongside the source telescope, not by reading a
                    // later SSA completion backward through its own observation control.
                    let rank_parameters = plan
                        .states
                        .iter()
                        .enumerate()
                        .map(|(position, state)| {
                            if !admitted.live[position] {
                                return Ok(Vec::new());
                            }
                            let Some(roots) = ranking::observations(checked, plan, state)? else {
                                return Ok(Vec::new());
                            };
                            roots
                                .iter()
                                .map(|_| Ok(value_id(allocate_dense(&mut catalogs.next_value)?)))
                                .collect::<Result<Vec<_>, LoweringError>>()
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    for (position, state) in plan.states.iter().enumerate() {
                        if !admitted.live[position] || (position == 0 && !entry_reentered) {
                            continue;
                        }
                        let lowered = crate::unit::attached_unit::claims::lower_unit_entry_claims(
                            identities,
                            state_owners[position].1,
                            state.state,
                            &state.entry_claims,
                            &state_views[position],
                            &catalogs.structural_types,
                        )?;
                        for (source, mut binding) in
                            state.entry_claims.iter().zip(lowered.entry_claims)
                        {
                            if admitted.entry_aliases[position]
                                .contains_key(&source.parameter_index)
                            {
                                continue;
                            }
                            binding.claim =
                                lookup_claim_id(&claims.source_claims, source.claim_identity)?;
                            block_entry_claims.push(binding);
                        }
                    }
                    block_entry_claims.sort_by_key(|binding| binding.claim);
                    let mut blocks = Vec::new();
                    let mut arrival_edges = BTreeMap::new();
                    if let Some(entry) = invocation_entry {
                        let edge = edge_id(allocate_dense(&mut catalogs.next_edge)?);
                        arrival_edges
                            .entry(state_ids[0])
                            .or_insert_with(Vec::new)
                            .push(edge);
                        blocks.push(Block {
                            id: entry,
                            parameters: Vec::new(),
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: erased_proof_formal_declarations(
                                identities,
                                checked,
                                &plan.states[0].erased_proof_parameters,
                            ),
                            structural_parameters: Vec::new(),
                            operations: Vec::new(),
                            terminator: Terminator::Jump {
                                edge,
                                target: state_ids[0],
                                arguments: scalar_parameters
                                    .iter()
                                    .filter(|_| entry_reentered)
                                    .map(|parameter| parameter.id)
                                    .collect(),
                                // A reentered entry forwards the machine's proof-only
                                // formals into the authored block's own erased declarations.
                                erased_arguments: entry_erased_formals
                                    .iter()
                                    .filter(|_| entry_reentered)
                                    .map(|parameter| {
                                        ScalarTerm::value(parameter.id, parameter.scalar_type)
                                    })
                                    .collect(),
                                // The machine proof roster forwards positionally into the
                                // entry block's identical declaration.
                                erased_proof_arguments: (0..plan.states[0]
                                    .erased_proof_parameters
                                    .len())
                                    .filter(|_| entry_reentered)
                                    .map(|position| semantic_vocabulary::ProofTerm::Formal {
                                        position: u32::try_from(position)
                                            .expect("erased-proof roster positions fit u32"),
                                    })
                                    .collect(),
                                structural_arguments: parameters
                                    .iter()
                                    .enumerate()
                                    .filter(|_| entry_reentered)
                                    .filter(|(dense, _)| {
                                        !admitted.entry_aliases.first().is_some_and(|aliased| {
                                            aliased.contains_key(&(*dense as u32))
                                        })
                                    })
                                    .map(|(_, parameter)| StructuralArgument {
                                        place: parameter.place,
                                        path: Vec::new(),
                                        access: parameter.access,
                                    })
                                    .collect(),
                                cleanup_actions: Vec::new(),
                            },
                        });
                    }
                    // A lender's stored view keeps its declared access ceiling, while a member
                    // parameter names the canonical view of the same pointer and count. The
                    // chain carries that one descriptor, so it presents the member when the
                    // geometry is unchanged and the ceiling supplies the member's access.
                    let presents =
            |stored: typed_trees::type_identity::TypeIdentityId,
             member: &checked_trees::CheckedUnitStructuralParameterPlan| {
                if stored == member.type_identity {
                    return true;
                }
                let (Ok(stored), Ok(formal)) = (
                    lookup_type_id(&catalogs.type_ids, &stored),
                    lookup_type_id(&catalogs.type_ids, &member.type_identity),
                ) else {
                    return false;
                };
                let requested = terminal_access(member.access);
                terminal_semantics::borrowed_referent_presentation(
                    catalogs.structural_types.iter(),
                    stored,
                    formal,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    && terminal_psi::find_structural_type(
                        catalogs.structural_types.as_ref(),
                        stored,
                    )
                    .is_some_and(|declaration| match declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView { access, .. }
                        | terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                        ) => access
                            .is_none_or(|access| structural_access_can_supply(access, requested)),
                        _ => false,
                    })
            };
                    let loan_chains = loan_chains::LoanChains::plan(
                        checked,
                        plan,
                        &checked.facts.values.structural_values,
                        &admitted.live,
                        &admitted.entry_aliases,
                        entry_reentered,
                        &state_views,
                        &presents,
                    )?;
                    let occurrences = super::super::ComposedOccurrences {
                        state_blocks: plan
                            .states
                            .iter()
                            .zip(&state_ids)
                            .zip(&admitted.live)
                            .filter(|(_, live)| **live)
                            .map(|((state, block), _)| lowered_psi::LoweredStateBlock {
                                machine: terminal_machine,
                                state: state.state,
                                block: *block,
                            })
                            .collect(),
                        ..Default::default()
                    };
                    let captured_loans = captured_loans::CapturedLoans::plan(
                        plan,
                        &checked.facts.values.structural_values,
                        &admitted.live,
                        &state_views,
                        &state_ids,
                        &admitted.entry_aliases,
                        catalogs,
                        &mut structural_places,
                    )?;
                    let block_ranks = std::collections::BTreeMap::new();
                    let rank_edges = std::collections::BTreeMap::new();
                    let mut owners = Vec::with_capacity(admitted.authored.len());
                    for (owner, _) in &admitted.authored {
                        if !owners.contains(&owner.symbol) {
                            owners.push(owner.symbol);
                        }
                    }
                    let record_fields = crate::scalar_lowering::computations::fields::prepare(
                        identities,
                        checked,
                        plan.machine,
                        &owners,
                        &catalogs.structural_types,
                    )?;
                    let produced_values =
                        prepare(identities, checked, &owners, &catalogs.structural_types)?;
                    let dynamic_dispatch = catalogs.dynamic_dispatch.clone().unwrap_or_else(|| {
                        std::sync::Arc::new(
                            crate::unit::dynamic_dispatch::DynamicDispatchIndex::new(checked),
                        )
                    });
                    let mut emission = StateGraphEmission {
                        checked,
                        plan,
                        terminal_machine,
                        admitted,
                        parameters,
                        scalar_parameters,
                        catalogs,
                        machine_result,
                        claims,
                        content_entry_claims,
                        content_identity_reshuffles: None,
                        loan_completions: Vec::new(),
                        block_entry_claims,
                        entry_reentered,
                        invocation_entry,
                        invocation_erased: entry_erased_formals.clone(),
                        rank_parameters,
                        state_ids,
                        state_views,
                        state_contract_rosters: &state_contract_rosters,
                        state_values,
                        state_erased,
                        state_erased_proof,
                        arrival_edges,
                        decided_arrival_edges: std::collections::BTreeSet::new(),
                        structural_places,
                        blocks,
                        loaned_local_homes: BTreeMap::new(),
                        retained_loan_parents: Vec::new(),
                        loan_chains,
                        captured_loans,
                        occurrences,
                        scalar_block_invariants: Vec::new(),
                        qualification_coercions: Vec::new(),
                        block_ranks,
                        rank_edges,
                        owners,
                        record_fields,
                        produced_values,
                        dynamic_dispatch,
                    };
                    for position in 0..plan.states.len() {
                        if !emission.admitted.live[position] {
                            continue;
                        }
                        emission.emit_state(identities, position)?;
                    }
                    let live_states = std::mem::take(&mut emission.admitted.live);
                    let StateGraphEmission {
                        parameters,
                        scalar_parameters,
                        catalogs,
                        machine_result,
                        claims,
                        content_entry_claims,
                        content_identity_reshuffles,
                        loan_completions,
                        mut block_entry_claims,
                        state_ids,
                        state_views: _,
                        state_values,
                        state_erased,
                        state_erased_proof: _,
                        arrival_edges,
                        decided_arrival_edges,
                        mut structural_places,
                        mut blocks,
                        mut scalar_block_invariants,
                        mut qualification_coercions,
                        loaned_local_homes,
                        retained_loan_parents,
                        loan_chains,
                        captured_loans,
                        mut occurrences,
                        mut block_ranks,
                        mut rank_edges,
                        ..
                    } = emission;
                    let content_identity_reshuffles =
                        content_identity_reshuffles.unwrap_or_default();
                    // A retained `requires` becomes the header's scalar block invariant:
                    // every incoming Jump or Conditional edge discharges it before the block
                    // runs. A plain entry publishes its clauses on the machine contract
                    // instead — invocation is an arrival without an edge proof.
                    for (position, state) in plan.states.iter().enumerate() {
                        let declaration = checked.state_by_symbol(state.state).ok_or(
                            LoweringError::Unsupported(
                                "state requirement has no source declaration",
                            ),
                        )?;
                        let owner = checked.machine_holding_state(state.state).ok_or(
                            LoweringError::Unsupported("state requirement has no machine owner"),
                        )?;
                        let expected = checked
                            .state_contracts(declaration)
                            .iter()
                            .filter(|contract| {
                                contract.kind
                                    == checked_trees::signature::SignatureContractKind::Requires
                            })
                            .flat_map(|contract| contract.facts.handles())
                            .filter(|fact| {
                                matches!(
                                    checked.proof_facts.get(*fact),
                                    checked_trees::domain::ProofFact::Proposition(_)
                                )
                            })
                            .collect::<Vec<_>>();
                        let actual = state
                            .requires
                            .iter()
                            .filter_map(|requirement| match requirement {
                                checked_trees::CheckedStateContractRequirement::SourceFact(
                                    fact,
                                ) => Some(*fact),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        if expected != actual
                            || actual.iter().any(|fact| {
                                checked
                                    .facts
                                    .proof
                                    .state_requires_fact(
                                        &checked.typed,
                                        owner.symbol,
                                        state.state,
                                        *fact,
                                    )
                                    .is_none()
                            })
                        {
                            return unsupported(
                                "state source requirement lost its exact declared proof roster",
                            );
                        }
                        if position == 0 && !actual.is_empty() {
                            return unsupported(
                                "source invocation requirement needs its invocation evidence owner",
                            );
                        }
                        let declaration_requires = plan.declaration_requirements(state);
                        if (state.requires.is_empty() && declaration_requires.is_empty())
                            || (position == 0 && !entry_reentered)
                            || !live_states[position]
                        {
                            continue;
                        }
                        let header = state_ids[position];
                        let case_arrival = blocks.iter().any(|block| {
                            matches!(
                                &block.terminator,
                                Terminator::StructuralCase { cases, .. }
                                    if cases.iter().any(|case| case.target == header)
                            )
                        });
                        if case_arrival {
                            return unsupported(
                                "Unit graph requires header reached by case dispatch",
                            );
                        }
                        let incoming_edges =
                            arrival_edges.get(&header).cloned().unwrap_or_default();
                        let requires_source_decision = state.requires.iter().any(|requirement| {
                            matches!(
                                requirement,
                                checked_trees::CheckedStateContractRequirement::SourceExpression(_)
                                    | checked_trees::CheckedStateContractRequirement::SourceFact(_)
                            )
                        });
                        // Source checking owns the complete authored contract, including
                        // predicates outside this scalar vocabulary. Consume its exact call
                        // decision at every actual arrival; never replace an unrepresented
                        // requirement with Truth or infer its discharge from its spelling.
                        if requires_source_decision
                            && (incoming_edges.is_empty()
                                || incoming_edges
                                    .iter()
                                    .any(|edge| !decided_arrival_edges.contains(edge)))
                        {
                            return unsupported(
                                "state source requirement lacks a checked arrival decision",
                            );
                        }
                        let projected_requires = state
                            .requires
                            .iter()
                            .filter_map(|requirement| {
                                match requirement {
                    checked_trees::CheckedStateContractRequirement::Closed(clause) => {
                        Some(Some(clause.clone()))
                    }
                    checked_trees::CheckedStateContractRequirement::SourceExpression(_)
                    | checked_trees::CheckedStateContractRequirement::SourceFact(_) => None,
                }
                            })
                            .chain(
                                declaration_requires
                                    .iter()
                                    .map(|clause| Some(clause.clone())),
                            )
                            .collect::<Vec<_>>();
                        // A state's `requires` cites its own parameter positions; the roster
                        // pairs them with the terminal places each block view carried at
                        // seal (the receiver entry points at the original invocation
                        // storage).
                        let view_roster = &state_contract_rosters[position];
                        let Some(predicate) = crate::scalar_lowering::contracts::clauses(
                identities,
                &projected_requires,
                &state_values[position],
                &state_erased[position],
                &crate::scalar_lowering::contracts::ContractViewNamespace {
                    scalar_applications: catalogs.scalar_applications,
                    constant_values: &checked.typed.constant_recipe_closure.values,
                    result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                    parameters: view_roster,
                    structural_types: &catalogs.structural_types,
                },
            )?
            else {
                continue;
            };
                        // An arrival whose transition checking discharged in full needs no
                        // certificate, provided every row is a plain clause the call check
                        // judged; a float range rides its own catalog and keeps the search.
                        // The invocation-entry arrival has no transition and never qualifies.
                        let checked_rows = projected_requires.iter().all(|row| {
                            matches!(
                                row,
                                Some(
                                    checked_trees::ClosedScalarContractValue::Predicate(_)
                                        | checked_trees::ClosedScalarContractValue::Boolean(_)
                                        | checked_trees::ClosedScalarContractValue::Integer(_)
                                )
                            )
                        });
                        let mut arrivals = Vec::new();
                        for edge in incoming_edges {
                            let obligation = obligation_id(allocate_dense(
                                &mut catalogs.scalar_calls.next_call_obligation,
                            )?);
                            if checked_rows && decided_arrival_edges.contains(&edge) {
                                occurrences.source_checked_obligations.push(obligation);
                            }
                            arrivals.push(terminal_psi::ScalarBlockInvariantArrival {
                                edge,
                                obligation,
                            });
                        }
                        if arrivals.is_empty() {
                            return unsupported("Unit graph requires header has no edge arrivals");
                        }
                        scalar_block_invariants.push(terminal_psi::ScalarBlockInvariant {
                            machine: terminal_machine,
                            header,
                            predicate,
                            arrivals,
                        });
                    }
                    blocks.sort_by_key(|block| block.id);
                    let existing_result_and_temp_places: std::collections::HashSet<
                        semantic_vocabulary::PlaceId,
                    > = catalogs
                        .result_places
                        .iter()
                        .chain(&catalogs.temporary_places)
                        .map(|existing| existing.id)
                        .collect();
                    structural_places.extend(
                        blocks
                            .iter()
                            .flat_map(|block| {
                                crate::scalar_lowering::computations::arrays::declarations(
                                    &block.operations,
                                )
                                .chain(
                                    crate::scalar_lowering::computations::cases::declarations(
                                        &block.operations,
                                    ),
                                )
                            })
                            .filter(|place| !existing_result_and_temp_places.contains(&place.id)),
                    );
                    structural_places.append(&mut catalogs.temporary_places);
                    structural_places.extend(catalogs.result_places.drain(result_places_start..));
                    retain_block_parameters(blocks.iter(), &mut structural_places)?;
                    crate::emission::operation_emission::place_declarations::retain_operation_results(
            blocks.iter().flat_map(|block| &block.operations),
            &mut structural_places,
        )?;
                    structural_places.sort_by_key(|place| place.id);
                    crate::emission::claim_bindings::seal_block_entry_claims(
                        &mut block_entry_claims,
                    )?;
                    let mut machine = TerminalMachine {
                        assembly_target_requirement: checked
                            .typed
                            .machine(plan.machine)
                            .map(|machine| machine.assembly_target_requirement)
                            .ok_or(crate::shared::lowering_error::LoweringError::Unsupported(
                                "assembly target requirement has no checked machine",
                            ))?,
                        closed_reach_application: None,
                        declared_service_reach:
                            crate::unit::attached_unit::lower_declared_service_reach(
                                checked,
                                plan.machine,
                                &catalogs.service_ids,
                            )?,
                        id: terminal_machine,
                        attachment: None,
                        declaration: None,
                        nominal_cleanup_subject: None,
                        parameters: scalar_parameters,
                        parameter_order: find(&catalogs.signatures, plan.machine)?
                            .parameter_order
                            .clone(),
                        structural_places,
                        structural_parameters: parameters,
                        entry_claims: claims.entry_claims,
                        block_entry_claims,
                        loan_completions,
                        ranked_scc: None,
                        result: machine_result,
                        published_service_ceiling: lower_installation_machine_service_ceiling(
                            checked,
                            plan.machine,
                            plan.contract_service_reach,
                            plan.service_reach,
                            &catalogs.service_ids,
                        )?,
                        content_entry_claims,
                        content_identity_reshuffles,
                        content_partition_compositions: Vec::new(),
                        entry: invocation_entry.unwrap_or(state_ids[0]),
                        blocks,
                        contract: MachineContract {
                            id: contract_id(terminal_machine.get()),
                            requires: Vec::new(),
                            source_checked_requires: Vec::new(),
                            ensures: Vec::new(),
                            crash_routes: Vec::new(),
                            erased_scalar_formals: entry_erased_formals,
                            erased_proof_formals: erased_proof_formal_declarations(
                                identities,
                                checked,
                                &plan.states[0].erased_proof_parameters,
                            ),
                            outcome_specific_ensures: Vec::new(),
                        },
                    };
                    captured_loans.finish(&mut machine, &state_ids)?;
                    let retained_entry_homes = loaned_local_homes::finish(
                        &mut machine,
                        &loaned_local_homes,
                        &retained_loan_parents,
                    )?;
                    loan_chains.finish(
                        &mut machine,
                        &state_ids,
                        &live_states,
                        &captured_loans,
                        &catalogs.structural_types,
                        &retained_entry_homes,
                        &retained_loan_parents,
                        &mut catalogs.scalar_calls.next_call_obligation,
                    )?;
                    let continuation_value_uses = machine
                        .blocks
                        .iter()
                        .flat_map(|block| {
                            block_ranks
                                .get(&block.id)
                                .into_iter()
                                .flatten()
                                .copied()
                                .chain(block.terminator.edges().flat_map(|edge| {
                                    rank_edges
                                        .get(&edge)
                                        .into_iter()
                                        .flat_map(|(rank, _)| rank.iter().copied())
                                }))
                                .map(move |value| (block.id, value))
                        })
                        .collect::<Vec<_>>();
                    let mut continuation_maps =
                        crate::emission::continuation_specialization::complete(
                            &mut machine,
                            &occurrences.selected_cleanup_groups,
                            &catalogs.structural_types,
                            crate::emission::continuation_specialization::CompanionRows {
                                completed_cleanup_frontiers: &mut occurrences
                                    .completed_cleanup_frontiers,
                                source_returns: &mut occurrences.source_returns,
                                continuation_value_uses: &continuation_value_uses,
                                claim_introductions: &mut occurrences.claim_introductions,
                                claim_edge_bindings: &occurrences.claim_edge_bindings,
                                source_calls: &mut occurrences.source_calls,
                                scalar_block_invariants: &mut scalar_block_invariants,
                                qualification_coercions: &mut qualification_coercions,
                                selected_comparisons: &mut occurrences.selected_comparisons,
                                source_checked_obligations: &mut occurrences
                                    .source_checked_obligations,
                            },
                            crate::emission::continuation_specialization::Counters {
                                next_place: &mut catalogs.next_place,
                                next_value: &mut catalogs.next_value,
                                next_block: &mut catalogs.next_block,
                                next_edge: &mut catalogs.next_edge,
                                next_operation: &mut catalogs.next_operation,
                                next_obligation: &mut catalogs.scalar_calls.next_call_obligation,
                            },
                        )?;
                    if !continuation_maps.is_empty() {
                        let original_states = std::mem::take(&mut occurrences.state_blocks);
                        let original_ranks = std::mem::take(&mut block_ranks);
                        let original_edges = std::mem::take(&mut rank_edges);
                        for namespace in &mut continuation_maps {
                            let [(original, physical)] = namespace.blocks.as_slice() else {
                                return unsupported(
                                    "selected continuation has no unique owning block",
                                );
                            };
                            let (original, physical) = (*original, *physical);
                            for state in original_states
                                .iter()
                                .filter(|state| state.block == original)
                            {
                                occurrences
                                    .state_blocks
                                    .push(lowered_psi::LoweredStateBlock {
                                        block: physical,
                                        ..*state
                                    });
                            }
                            if let Some(rank) = original_ranks.get(&original) {
                                block_ranks.insert(
                                    physical,
                                    rank.iter().map(|value| namespace.value(*value)).collect(),
                                );
                            }
                            for (original, physical) in namespace.edges.clone() {
                                if let Some((rank, comparison)) = original_edges.get(&original) {
                                    rank_edges.insert(
                                        physical,
                                        (
                                            rank.iter()
                                                .map(|value| namespace.value(*value))
                                                .collect(),
                                            *comparison,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    // Authored entry/local cleanup already belongs to the composed emitter.
                    // Complete checked no-code carrier frontiers after private loans and their
                    // continuations have reached their final emitted graph. Plain owned type
                    // capability is independent of scheduled source death rows; descriptors
                    // keep their existing loan-continuation custody.
                    crate::emission::affine_cleanup::complete(
                        &occurrences.completed_cleanup_frontiers,
                        &[],
                        &machine.structural_parameters,
                        &crate::emission::affine_cleanup::no_code_type_ids(
                            checked,
                            &catalogs.type_ids,
                        ),
                        &machine.structural_places,
                        machine.entry,
                        &mut machine.blocks,
                        &catalogs.structural_types,
                    )?;
                    // Crash ceilings live in the invocation signature, independent of the
                    // state's block-local value roster. Predicate parameters retain authored
                    // structural positions and scalar formals retain their dense coordinates.
                    // Publishing the graph must use that complete namespace just as ordinary
                    // machine emission does.
                    let signature = find(&catalogs.signatures, plan.machine)?;
                    machine.contract.crash_routes = if signature.predicate_parameters.is_empty() {
                        lower_checked_crash_route_buckets(
                            identities,
                            &catalogs.root_crash_routes,
                            &signature.scalar_parameters,
                        )?
                    } else {
                        crate::proofs::crash_routes::lower_structural_crash_route_buckets(
                            identities,
                            &catalogs.root_crash_routes,
                            &signature.scalar_parameters,
                            &signature.predicate_parameters,
                            &catalogs.structural_types,
                            &signature.runtime_requirements,
                        )?
                    };
                    ranking::retain(&mut machine, &block_ranks, &rank_edges)?;
                    Ok((
                        machine,
                        occurrences,
                        scalar_block_invariants,
                        qualification_coercions,
                    ))
                }
            }

            pub(crate) mod claims {
                use super::super::LoweringError;
                use super::CheckedComposedUnitControlMachinePlan;
                use language_semantics::PermissionClaimIdentity;

                pub(crate) struct ClaimTransport {
                    pub(crate) aliases: Vec<(PermissionClaimIdentity, PermissionClaimIdentity)>,
                }

                pub(crate) fn bind_invocation_claims(
                    plan: &CheckedComposedUnitControlMachinePlan,
                    live: &[bool],
                    entry_aliases: &[std::collections::BTreeMap<u32, u32>],
                ) -> Result<ClaimTransport, LoweringError> {
                    let mut aliases = Vec::new();
                    for (position, state) in plan.states.iter().enumerate() {
                        if !live[position] {
                            continue;
                        }
                        for claim in &state.entry_claims {
                            let Some(root_parameter) = entry_aliases
                                .get(position)
                                .and_then(|parameters| parameters.get(&claim.parameter_index))
                            else {
                                continue;
                            };
                            let root = plan.states[0]
                                .entry_claims
                                .iter()
                                .find(|entry| {
                                    entry.parameter_index == *root_parameter
                                        && entry.path == claim.path
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph invocation alias has no matching claim path",
                                ))?;
                            if claim.claim_identity != root.claim_identity {
                                aliases.push((claim.claim_identity, root.claim_identity));
                            }
                        }
                    }
                    Ok(ClaimTransport { aliases })
                }
            }
            pub(crate) mod entry_aliases {
                use std::collections::{BTreeMap, BTreeSet};

                use super::super::super::LoweringError;
                use super::{CheckedComposedUnitControlMachinePlan, successors};
                use checked_trees::{
                    CheckedComposedUnitControlStatePlan, CheckedStoreRoot, CheckedStructuralAccess,
                    CheckedStructuralControlTransferSourcePlan,
                };

                fn preserves_owned_entry_custody(
                    state: &CheckedComposedUnitControlStatePlan,
                    index: u32,
                ) -> bool {
                    let Some(parameter) = state.structural_parameters.get(index as usize) else {
                        return false;
                    };
                    !state
                        .operations
                        .iter()
                        .filter_map(|operation| operation.store())
                        .any(|store| {
                            store.destination.root
                                == (CheckedStoreRoot::Parameter {
                                    position: parameter.position,
                                })
                                && store.installed_value().is_some()
                        })
                }

                pub(crate) fn alias_entry_parameter_loans(
                    plan: &CheckedComposedUnitControlMachinePlan,
                    live: &[bool],
                    aliased: &mut [BTreeMap<u32, u32>],
                ) -> Result<(), LoweringError> {
                    let Some(entry) = plan.states.first() else {
                        return Ok(());
                    };
                    let entry_reentered = plan
                        .states
                        .iter()
                        .zip(live)
                        .filter_map(|(state, live)| live.then_some(state))
                        .flat_map(successors)
                        .any(|successor| plan.successor_position(successor) == Some(0));
                    if entry_reentered {
                        return Ok(());
                    }
                    let roots =
                        entry
                            .structural_parameters
                            .iter()
                            .enumerate()
                            .filter(|(_, parameter)| {
                                matches!(
                                    parameter.access,
                                    checked_trees::checked_borrowed_access_pair_pattern!()
                                )
                            });
                    for (root, root_parameter) in roots {
                        let root = u32::try_from(root).map_err(|_| {
                            LoweringError::Unsupported("Unit graph entry position exceeds u32")
                        })?;
                        let mut candidates = BTreeSet::new();
                        for (position, state) in plan.states.iter().enumerate().skip(1) {
                            if !live[position] {
                                continue;
                            }
                            for (index, parameter) in state.structural_parameters.iter().enumerate()
                            {
                                let index = u32::try_from(index).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "Unit graph parameter index exceeds u32",
                                    )
                                })?;
                                if matches!(
                                    parameter.access,
                                    checked_trees::checked_borrowed_access_pair_pattern!()
                                ) && parameter.access == root_parameter.access
                                    && parameter.type_identity == root_parameter.type_identity
                                    && parameter.multiplicity == root_parameter.multiplicity
                                    && parameter.qualifications == root_parameter.qualifications
                                    && parameter.projected_qualifications
                                        == root_parameter.projected_qualifications
                                    && !aliased[position].contains_key(&index)
                                {
                                    candidates.insert((position, index));
                                }
                            }
                        }
                        loop {
                            let rejected = candidates
                                .iter()
                                .copied()
                                .filter(|&(position, index)| {
                                    let mut incoming = plan
                                        .states
                                        .iter()
                                        .enumerate()
                                        .filter(|(source, _)| live[*source])
                                        .flat_map(|(source, state)| {
                                            successors(state)
                                                .into_iter()
                                                .filter(move |edge| plan.successor_position(edge) == Some(position))
                                                .map(move |edge| (source, edge))
                                        })
                                        .peekable();
                                    incoming.peek().is_none()
                                        || !incoming.all(|(source, edge)| {
                                            edge.transfers
                                                .iter()
                                                .find(|transfer| transfer.target_parameter_index == index)
                                                .is_some_and(|transfer| match transfer.source {
                                                    CheckedStructuralControlTransferSourcePlan::Parameter {
                                                        index: source_index,
                                                    } => {
                                                        (source == 0 && source_index == root)
                                                            || candidates.contains(&(source, source_index))
                                                    }
                                                    _ => false,
                                                })
                                        })
                                })
                                .collect::<Vec<_>>();
                            if rejected.is_empty() {
                                break;
                            }
                            for candidate in rejected {
                                candidates.remove(&candidate);
                            }
                        }
                        for (position, index) in candidates {
                            aliased[position].insert(index, root);
                        }
                    }
                    Ok(())
                }

                pub(crate) fn alias_forwarded_entry_claims(
                    plan: &CheckedComposedUnitControlMachinePlan,
                    live: &[bool],
                    aliased: &mut [BTreeMap<u32, u32>],
                ) -> Result<(), LoweringError> {
                    let Some(entry) = plan.states.first() else {
                        return Ok(());
                    };
                    let entry_reentered = plan
                        .states
                        .iter()
                        .zip(live)
                        .filter_map(|(state, live)| live.then_some(state))
                        .flat_map(successors)
                        .any(|successor| plan.successor_position(successor) == Some(0));
                    let claimed = |position: usize, index: u32| {
                        let state = &plan.states[position];
                        state
                            .entry_claims
                            .iter()
                            .any(|claim| claim.parameter_index == index)
                            && state.structural_parameters.get(index as usize).is_some_and(
                                |parameter| parameter.access == CheckedStructuralAccess::Owned,
                            )
                    };
                    let incoming = |position: usize| {
                        plan.states
                            .iter()
                            .enumerate()
                            .filter(|(source, _)| live[*source])
                            .flat_map(|(source, state)| {
                                successors(state)
                                    .into_iter()
                                    .filter(move |edge| {
                                        plan.successor_position(edge) == Some(position)
                                    })
                                    .map(move |edge| (source, edge))
                            })
                            .collect::<Vec<_>>()
                    };
                    let mut roots = BTreeMap::new();
                    for (position, state) in plan.states.iter().enumerate() {
                        if !live[position] || (position == 0 && !entry_reentered) {
                            continue;
                        }
                        for claim in &state.entry_claims {
                            if claimed(position, claim.parameter_index) {
                                roots.insert((position, claim.parameter_index), None);
                            }
                        }
                    }
                    if entry_reentered {
                        for claim in &entry.entry_claims {
                            if let Some(root) = roots.get_mut(&(0, claim.parameter_index)) {
                                *root = Some(claim.parameter_index);
                            }
                        }
                    }
                    let arriving_root =
                        |roots: &BTreeMap<(usize, u32), Option<u32>>,
                         source: usize,
                         index: u32,
                         parameter: &checked_trees::CheckedUnitStructuralParameterPlan| {
                            let source_parameter = plan.states[source]
                                .structural_parameters
                                .get(index as usize)?;
                            if source_parameter.access != CheckedStructuralAccess::Owned
                                || source_parameter.multiplicity != parameter.multiplicity
                            {
                                return None;
                            }
                            if source == 0 && !entry_reentered {
                                Some(index)
                            } else {
                                roots.get(&(source, index)).copied().flatten()
                            }
                        };
                    loop {
                        let mut changed = false;
                        let candidates = roots.keys().copied().collect::<Vec<_>>();
                        for (position, index) in candidates {
                            let parameter =
                                &plan.states[position].structural_parameters[index as usize];
                            let mut root = roots[&(position, index)];
                            let mut rejected = false;
                            for (source, edge) in incoming(position) {
                                let forwarded = edge
                                    .transfers
                                    .iter()
                                    .find(|transfer| transfer.target_parameter_index == index)
                                    .and_then(|transfer| match transfer.source {
                                        CheckedStructuralControlTransferSourcePlan::Parameter {
                                            index,
                                        } => Some(index),
                                        _ => None,
                                    });
                                let Some(source_index) = forwarded else {
                                    rejected = true;
                                    break;
                                };
                                if !preserves_owned_entry_custody(
                                    &plan.states[source],
                                    source_index,
                                ) {
                                    rejected = true;
                                    break;
                                }
                                if roots.get(&(source, source_index)) == Some(&None) {
                                    continue;
                                }
                                match (arriving_root(&roots, source, source_index, parameter), root)
                                {
                                    (None, _) => {
                                        rejected = true;
                                        break;
                                    }
                                    (Some(arriving), Some(settled)) if arriving != settled => {
                                        rejected = true;
                                        break;
                                    }
                                    (Some(arriving), _) => root = Some(arriving),
                                }
                            }
                            if rejected {
                                roots.remove(&(position, index));
                                changed = true;
                            } else if root != roots[&(position, index)] {
                                roots.insert((position, index), root);
                                changed = true;
                            }
                        }
                        if !changed {
                            break;
                        }
                    }
                    for ((position, index), root) in roots {
                        let Some(root) = root else {
                            return Err(LoweringError::Unsupported(
                                "Unit graph forwarded claim has no entry arrival",
                            ));
                        };
                        aliased[position].insert(index, root);
                    }
                    Ok(())
                }
            }
            pub(crate) mod exclusive_handoffs {
                use std::collections::BTreeSet;

                use super::super::super::LoweringError;
                use super::super::CheckedTrees;
                use super::{CheckedComposedUnitControlMachinePlan, successors};
                use checked_trees::{
                    CheckedStorageRoot, CheckedStructuralAccess,
                    CheckedStructuralControlTransferSourcePlan,
                    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
                    CheckedUnitStructuralTypeShape,
                };
                use language_semantics::Multiplicity;

                pub(crate) fn promote_shared_handoffs(
                    checked: &CheckedTrees,
                    plan: &CheckedComposedUnitControlMachinePlan,
                    live: &[bool],
                    aliased: &[std::collections::BTreeMap<u32, u32>],
                ) -> Result<Vec<BTreeSet<u32>>, LoweringError> {
                    let mut promoted = vec![BTreeSet::new(); plan.states.len()];
                    let entry_reentered = plan.states.first().is_some_and(|_| {
                        plan.states
                            .iter()
                            .zip(live)
                            .filter_map(|(state, live)| live.then_some(state))
                            .flat_map(successors)
                            .any(|successor| plan.successor_position(successor) == Some(0))
                    });
                    let is_aliased = |position: usize, index: u32| {
                        aliased
                            .get(position)
                            .is_some_and(|aliased| aliased.contains_key(&index))
                    };
                    let block_loan_referent = |parameter: &CheckedUnitStructuralParameterPlan| {
                        checked_trees::checked_structural_type_by_identity(
                            &checked.facts.flow.terminal_unit_effects.structural_types,
                            parameter.type_identity,
                        )
                        .is_some_and(|shape| {
                            matches!(
                                shape.shape,
                                CheckedUnitStructuralTypeShape::Record { .. }
                                    | CheckedUnitStructuralTypeShape::FixedArray { .. }
                            )
                        })
                    };
                    let plain_loan = |parameter: &CheckedUnitStructuralParameterPlan| {
                        parameter.multiplicity == Multiplicity::Unrestricted
                            && parameter.qualifications.is_empty()
                            && block_loan_referent(parameter)
                    };
                    let mut candidates = BTreeSet::new();
                    for (position, state) in plan.states.iter().enumerate() {
                        if !live.get(position).copied().unwrap_or(false) || position == 0 {
                            continue;
                        }
                        for (index, parameter) in state.structural_parameters.iter().enumerate() {
                            let index = u32::try_from(index).map_err(|_| {
                                LoweringError::Unsupported("Unit graph parameter index exceeds u32")
                            })?;
                            if parameter.access == CheckedStructuralAccess::SharedBorrow
                                && plain_loan(parameter)
                                && !is_aliased(position, index)
                            {
                                candidates.insert((position, index));
                            }
                        }
                    }
                    if candidates.is_empty() {
                        return Ok(promoted);
                    }
                    loop {
                        let rejected = candidates
                            .iter()
                            .copied()
                            .filter(|&(position, index)| {
                                let target_type = &plan.states[position].structural_parameters
                                    [index as usize]
                                    .type_identity;
                                let mut incoming = plan
                                    .states
                                    .iter()
                                    .enumerate()
                                    .filter(|(source, _)| {
                                        live.get(*source).copied().unwrap_or(false)
                                    })
                                    .flat_map(|(source, state)| {
                                        successors(state)
                                            .into_iter()
                                            .filter(move |edge| {
                                                plan.successor_position(edge) == Some(position)
                                            })
                                            .map(move |edge| (source, edge))
                                    })
                                    .peekable();
                                incoming.peek().is_none()
                                    || !incoming.all(|(source, edge)| {
                                        let Some(forwarded) = edge
                                            .transfers
                                            .iter()
                                            .find(|transfer| {
                                                transfer.target_parameter_index == index
                                            })
                                            .and_then(|transfer| whole_parameter(&transfer.source))
                                        else {
                                            return false;
                                        };
                                        let Some(supplier) = plan.states[source]
                                            .structural_parameters
                                            .get(forwarded as usize)
                                        else {
                                            return false;
                                        };
                                        let exclusive = (supplier.access
                                            == CheckedStructuralAccess::MutableBorrow
                                            && plain_loan(supplier)
                                            && (source != 0 || entry_reentered))
                                            || candidates.contains(&(source, forwarded));
                                        exclusive
                                            && supplier.type_identity == *target_type
                                            && !is_aliased(source, forwarded)
                                            && edge
                                                .transfers
                                                .iter()
                                                .filter(|transfer| {
                                                    names_parameter(&transfer.source, forwarded)
                                                })
                                                .count()
                                                == 1
                                    })
                            })
                            .collect::<Vec<_>>();
                        if rejected.is_empty() {
                            break;
                        }
                        for candidate in rejected {
                            candidates.remove(&candidate);
                        }
                    }
                    for (position, index) in candidates {
                        promoted[position].insert(index);
                    }
                    Ok(promoted)
                }

                fn whole_parameter(
                    source: &CheckedStructuralControlTransferSourcePlan,
                ) -> Option<u32> {
                    match source {
                        CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            Some(*index)
                        }
                        CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                            subject,
                            ..
                        } if subject.path.is_empty() => match subject.source {
                            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index,
                            } => Some(parameter_index),
                            _ => None,
                        },
                        _ => None,
                    }
                }

                fn names_parameter(
                    source: &CheckedStructuralControlTransferSourcePlan,
                    index: u32,
                ) -> bool {
                    match source {
                        CheckedStructuralControlTransferSourcePlan::Parameter { index: forwarded } => {
                            *forwarded == index
                        }
                        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => matches!(
                            subject.source,
                            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                                if parameter_index == index
                        ),
                        CheckedStructuralControlTransferSourcePlan::ViewSubslice(argument) => {
                            crate::unit::attached_unit::view_ranges::range(argument).is_some_and(|range| {
                                matches!(range.root, CheckedStorageRoot::Parameter { index: lent } if lent == index)
                            })
                        }
                        _ => false,
                    }
                }
            }
            pub(crate) mod ranking {
                use super::super::super::super::{
                    BlockId, IntegerType, TerminalRankedScc, ValueId,
                };
                use super::super::super::{TerminalMachine, Terminator, unsupported};
                use super::super::LoweringError;
                use super::{
                    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
                };
                use crate::expression_preparation::bindings::CapturedStructuralFormal;
                use crate::shared::checked_facts::value_facts;
                use std::collections::BTreeMap;
                use terminal_psi::{
                    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
                    TerminalNaturalRankComparison, TerminalNaturalRankEdge,
                };

                pub(crate) fn observations<'a>(
                    checked: &'a checked_trees::CheckedTrees,
                    plan: &CheckedComposedUnitControlMachinePlan,
                    state: &CheckedComposedUnitControlStatePlan,
                ) -> Result<
                    Option<Vec<&'a checked_trees::CheckedScalarComputationRoot>>,
                    LoweringError,
                > {
                    let Some(rank) = plan
                        .natural_ranks
                        .iter()
                        .find(|rank| rank.state == state.state)
                    else {
                        return Ok(None);
                    };
                    let components = plan
                        .rank_components
                        .span(rank.components)
                        .filter(|values| !values.is_empty())
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph rank lost its ordered component span",
                        ))?;
                    let mut observations = Vec::with_capacity(components.len());
                    for (component, value) in components.iter().enumerate() {
                        let component = u32::try_from(component).map_err(|_| {
                            LoweringError::Unsupported("rank component ordinal exceeds u32")
                        })?;
                        let mut roots = value_facts(checked)
                            .scalar_computations
                            .roots
                            .iter()
                            .map(|(_, root)| root)
                            .filter(|root| {
                                root.state == state.state
                                    && root.root == value.observation
                                    && root.statement_ordinal == 0
                                    && root.role
                                        == checked_trees::CheckedScalarExpressionRole::RankObservation { component }
                            });
                        let root = roots.next().ok_or(LoweringError::Unsupported(
                            "Unit graph rank lost its retained observation root",
                        ))?;
                        if roots.next().is_some() || !value.observation.is_valid() {
                            return unsupported(
                                "Unit graph rank duplicates its observation custody",
                            );
                        }
                        observations.push(root);
                    }
                    Ok(Some(observations))
                }

                pub(crate) fn formal_bindings(
                    checked: &checked_trees::CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    bindings: &crate::expression_preparation::bindings::ScalarBindings,
                    scalar_slots: &[(usize, semantic_vocabulary::ScalarType)],
                    structural_actuals: &[(
                        terminal_psi::StructuralArgument,
                        semantic_vocabulary::StructuralTypeId,
                    )],
                ) -> Result<crate::expression_preparation::bindings::ScalarBindings, LoweringError>
                {
                    if scalar_slots.len() != state.scalar_parameters.len()
                        || structural_actuals.len() != state.structural_parameters.len()
                    {
                        return unsupported(
                            "Unit graph observation lost its complete formal telescope",
                        );
                    }
                    let (_, declaration) =
                        crate::shared::declarations::state_declaration(checked, state.state)?;
                    let parameters = checked.state_parameters(declaration);
                    let formals = state
                        .structural_parameters
                        .iter()
                        .zip(structural_actuals)
                        .map(|(formal, (argument, structural_type))| {
                            let parameter = parameters.get(formal.position as usize).ok_or(
                                LoweringError::Unsupported(
                                    "Unit graph observation lost its exact structural formal",
                                ),
                            )?;
                            Ok(CapturedStructuralFormal {
                                source_position: formal.position,
                                symbol: parameter.symbol,
                                argument: argument.clone(),
                                structural_type: *structural_type,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    bindings.capture_formal_observations(scalar_slots, &formals)
                }

                pub(crate) fn retain(
                    machine: &mut TerminalMachine,
                    ranks: &BTreeMap<BlockId, Vec<ValueId>>,
                    edges: &BTreeMap<
                        semantic_vocabulary::EdgeId,
                        (Vec<ValueId>, TerminalNaturalRankComparison),
                    >,
                ) -> Result<(), LoweringError> {
                    if ranks.is_empty() {
                        return Ok(());
                    }
                    let mut components = Vec::new();
                    for members in terminal_semantics::control_graph::control_cycle_members(machine)
                        .map_err(LoweringError::InvalidControlGraph)?
                    {
                        let mut observations = arena::Arena::new();
                        let mut retained_edges = Vec::new();
                        let mut retained_ranks = Vec::new();
                        for block in &machine.blocks {
                            if !members.contains(&block.id) {
                                continue;
                            }
                            retained_ranks.push(TerminalBlockNaturalRank {
                                block: block.id,
                                values: retain_rank_values(
                                    machine,
                                    &mut observations,
                                    ranks.get(&block.id).ok_or(LoweringError::Unsupported(
                                        "Unit graph cyclic rank lost a block",
                                    ))?,
                                )?,
                            });
                            let successors = match &block.terminator {
                                Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
                                Terminator::Conditional {
                                    when_true,
                                    when_false,
                                    ..
                                } => vec![
                                    (when_true.edge, when_true.target),
                                    (when_false.edge, when_false.target),
                                ],
                                _ => Vec::new(),
                            };
                            for (edge, target) in successors {
                                if !members.contains(&target) {
                                    continue;
                                }
                                let (successor_rank, comparison) =
                                    edges.get(&edge).ok_or(LoweringError::Unsupported(
                                        "Unit graph cyclic rank lost an edge",
                                    ))?;
                                retained_edges.push(TerminalNaturalRankEdge {
                                    edge,
                                    source: block.id,
                                    target,
                                    successor_ranks: retain_rank_values(
                                        machine,
                                        &mut observations,
                                        successor_rank,
                                    )?,
                                    comparison: *comparison,
                                });
                            }
                        }
                        retained_ranks.sort_by_key(|rank| rank.block);
                        retained_edges.sort_by_key(|edge| edge.edge);
                        let component = TerminalNaturalCycle {
                            observations,
                            ranks: retained_ranks,
                            edges: retained_edges,
                        };
                        if component.ranks.is_empty()
                            || component
                                .ranks
                                .iter()
                                .any(|rank| component.rank_values(rank.values).is_none())
                            || component
                                .edges
                                .iter()
                                .any(|edge| component.rank_values(edge.successor_ranks).is_none())
                        {
                            return unsupported(
                                "cyclic rank lost its complete ordered carrier signature",
                            );
                        }
                        components.push(component);
                    }
                    if components.is_empty() {
                        return Ok(());
                    }
                    machine.ranked_scc = Some(TerminalRankedScc::Natural(components));
                    Ok(())
                }

                fn retain_rank_values(
                    machine: &TerminalMachine,
                    observations: &mut arena::Arena<TerminalIntegerRankValue>,
                    values: &[ValueId],
                ) -> Result<arena::HandleSpan<TerminalIntegerRankValue>, LoweringError>
                {
                    if values.is_empty() {
                        return unsupported("cyclic rank has no ordered observations");
                    }
                    let values = values
                        .iter()
                        .map(|value| {
                            Ok(TerminalIntegerRankValue {
                                integer_type: rank_type(machine, *value)?,
                                value: *value,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    Ok(observations.insert_many(values))
                }

                fn rank_type(
                    machine: &TerminalMachine,
                    value: ValueId,
                ) -> Result<IntegerType, LoweringError> {
                    machine
                        .parameters
                        .iter()
                        .chain(machine.block_parameters())
                        .copied()
                        .chain(
                            machine
                                .operations()
                                .filter_map(|operation| operation.result.as_scalar()),
                        )
                        .find_map(|declaration| {
                            if declaration.id == value
                                && let Some(integer) = declaration.scalar_type.as_integer()
                            {
                                Some(integer)
                            } else {
                                None
                            }
                        })
                        .ok_or(LoweringError::Unsupported(
                            "natural rank has no declared integer carrier",
                        ))
                }
            }
            pub(crate) mod topology {

                use super::super::LoweringError;
                use super::{
                    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
                };

                pub(crate) fn live(
                    plan: &CheckedComposedUnitControlMachinePlan,
                ) -> Result<Vec<bool>, LoweringError> {
                    CheckedComposedUnitControlStatePlan::live_mask(&plan.states)
                        .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))
                }
            }

            pub(crate) mod scalars {
                use super::super::super::{CheckedScalarExpressionRole, unsupported};
                use super::super::{CheckedTrees, LoweringError};
                use super::CheckedComposedUnitControlStatePlan;

                pub(crate) fn successor_value(
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    edge: &super::CheckedStructuralControlSuccessorPlan,
                    argument: &checked_trees::CheckedStructuralScalarArgumentPlan,
                ) -> Result<checked_trees::CheckedCallScalarArgument, LoweringError>
                {
                    let role = CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: argument.argument_ordinal,
                    };
                    let pure = &checked.facts.values.scalar_expressions;
                    let computations = &checked.facts.values.scalar_computations;
                    let mut roots =
                        computations
                            .roots
                            .iter()
                            .map(|(_, root)| root)
                            .filter(|root| {
                                root.state == state.state
                                    && root.statement_ordinal == edge.statement_ordinal
                                    && root.role == role
                            });
                    let root = roots.next();
                    if roots.next().is_some() {
                        return unsupported(
                            "Unit graph successor computation has duplicate source roots",
                        );
                    }
                    if let Some(root) = root {
                        let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
                            binding.state == state.state
                                && binding.statement_ordinal == edge.statement_ordinal
                                && binding.role == role
                        }) || pure.expressions.iter().any(|expression| {
                            expression.state == state.state
                                && expression.statement_ordinal == edge.statement_ordinal
                                && expression.role == role
                        });
                        if has_pure
                            || !computations.nodes.is_valid(root.root)
                            || computations.nodes.get(root.root).primitive_type
                                != argument.primitive_type
                        {
                            return unsupported(
                                "Unit graph successor computation disagrees with its argument",
                            );
                        }
                        Ok(checked_trees::CheckedCallScalarArgument::Computation(
                            root.root,
                        ))
                    } else {
                        let (_, value) = pure
                            .bound_expression_at(state.state, edge.statement_ordinal, role)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph scalar successor has no checked source expression",
                            ))?;
                        Ok(checked_trees::CheckedCallScalarArgument::Pure(
                            value.clone(),
                        ))
                    }
                }
            }

            pub(crate) mod case_emission {
                use super::super::super::super::{
                    CheckedComposedUnitControlTerminatorPlan, StructuralCaseId, StructuralFieldId,
                    StructuralParameterDeclaration,
                };
                use super::super::super::{
                    PlaceId, ValueDeclaration, terminal_scalar_type, unsupported,
                };
                use super::super::{LoweringError, catalogs};
                use super::CheckedComposedUnitControlStatePlan;
                use crate::emission::operation_emission::buffer::{
                    OperationBuffer, StructuralBinding,
                };
                use crate::shared::terminal_identities::fresh_field_snapshot;

                pub(crate) struct PreparedCase<'a> {
                    pub outcome: &'a checked_trees::CheckedClosedSumCaseOutcomePlan,
                    pub identity: StructuralCaseId,
                    pub fields: Vec<StructuralFieldId>,
                    pub values: Vec<(u32, ValueDeclaration)>,
                }

                pub(crate) struct PreparedCases<'a> {
                    pub source: PlaceId,
                    pub path: Vec<terminal_psi::StructuralPathSegment>,
                    pub cases: Vec<PreparedCase<'a>>,
                }

                pub(crate) struct SourceContext<'a> {
                    pub checked: &'a checked_trees::CheckedTrees,
                    pub state: &'a CheckedComposedUnitControlStatePlan,
                    pub parameters: &'a [StructuralParameterDeclaration],
                    pub bindings: &'a crate::expression_preparation::bindings::ScalarBindings,
                }

                pub(crate) struct CaseSource<'a> {
                    pub argument: terminal_psi::StructuralArgument,
                    pub root_type: semantic_vocabulary::StructuralTypeId,
                    pub declaration: &'a terminal_psi::StructuralTypeDeclaration,
                }

                pub(crate) fn source<'a>(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    context: &SourceContext<'_>,
                    catalogs: &'a catalogs::ComposedCatalogs,
                    subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
                    operations: &OperationBuffer,
                ) -> Result<CaseSource<'a>, LoweringError> {
                    let SourceContext {
                        checked,
                        state,
                        parameters,
                        bindings,
                    } = context;
                    let published = match subject.source {
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                            let parameter =
                                parameters
                                    .get(parameter_index as usize)
                                    .ok_or(LoweringError::Unsupported(
                                        "Unit graph case parameter missing",
                                    ))?;
                            Some((parameter.place, parameter.structural_type))
                        }
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        } => {
                            let produced = result(state, binding_ordinal, operations)?;
                            Some((produced.place(), produced.structural_type()))
                        }
                        _ => None,
                    };
                    let (argument, root_type) = if let Some((place, structural_type)) = published {
                        let prepared_path =
                            crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                                identities,
                                &subject.path,
                                structural_type,
                                &catalogs.structural_types,
                            )?;
                        let path = prepared_path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static(segment) => Ok(segment),
                            _ => unsupported("case dispatch runtime projection requires a selector evidence owner"),
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                        (
                            terminal_psi::StructuralArgument {
                                place,
                                path,
                                access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                                    subject.access,
                                ),
                            },
                            structural_type,
                        )
                    } else {
                        let (_, authored) =
                            crate::shared::declarations::state_declaration(checked, state.state)?;
                        let argument = bindings.borrowed_structural_argument(
                            identities,
                            checked,
                            subject,
                            checked.state_parameters(authored),
                        )?;
                        let root_type = bindings.structural_place_type(argument.place).ok_or(
                            LoweringError::Unsupported(
                                "Unit graph case subject source unsupported",
                            ),
                        )?;
                        (argument, root_type)
                    };
                    let declaration = terminal_semantics::runtime_structural_path_tip(
                        catalogs.structural_types.iter(),
                        root_type,
                        &argument.path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph case projection has no exact declared carrier",
                    ))?;
                    Ok(CaseSource {
                        argument,
                        root_type,
                        declaration,
                    })
                }

                pub(crate) fn prepare<'a>(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    context: &SourceContext<'a>,
                    catalogs: &catalogs::ComposedCatalogs,
                    operations: &OperationBuffer,
                    next_value: &mut u64,
                ) -> Result<Option<PreparedCases<'a>>, LoweringError> {
                    let state = context.state;
                    let CheckedComposedUnitControlTerminatorPlan::ClosedSum {
                        subject, cases, ..
                    } = &state.terminator
                    else {
                        return Ok(None);
                    };
                    let resolved = source(identities, context, catalogs, subject, operations)?;
                    let declaration = resolved.declaration;
                    let Some(declared) = declaration.shape.sum_cases() else {
                        return unsupported("Unit graph case subject is not a sum");
                    };
                    let qualifications = catalogs
                        .prepared_scalar_qualifications
                        .map(|prepared| prepared.catalog().fields.as_slice())
                        .unwrap_or(&[]);
                    let cases = declared
                        .iter()
                        .map(|declared| {
                            let mut matching = cases.iter().filter(|case| {
                                case.case_identity.spelling()
                                    == identities.text(declared.identity).unwrap_or_default()
                            });
                            let case = matching.next().ok_or(LoweringError::Unsupported(
                                "Unit graph declared case lost its successor",
                            ))?;
                            if matching.next().is_some() {
                                return unsupported("Unit graph declared case has duplicate successors");
                            }
                            let mut fields = Vec::new();
                            let mut values = Vec::new();
                            for payload in &case.payloads {
                                let field = terminal_psi::field_by_identity(
                                    &declared.fields,
                                    identities.intern(&payload.field_identity.spelling()),
                                )
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph payload field is missing",
                                ))?;
                                let scalar_type = terminal_scalar_type(payload.primitive_type)?;
                                fields.push(field.id);
                                values.push((
                                    payload.target_scalar_parameter_index,
                                    fresh_field_snapshot(next_value, scalar_type, field.id, qualifications)?,
                                ));
                            }
                            let shared_return = if let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) =
                                &case.outcome
                            {
                                fields.is_empty()
                                    && cases.iter().any(|other| {
                                        other.case_identity != case.case_identity
                                            && matches!(
                                                &other.outcome,
                                                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(other_arm)
                                                    if other_arm.statement_ordinal() == arm.statement_ordinal()
                                            )
                                    })
                            } else {
                                false
                            };
                            for field in declared.fields.iter().filter(|_| !shared_return) {
                                if field.relevance.is_erased() || fields.contains(&field.id) {
                                    continue;
                                }
                                if matches!(
                                    field.field_type,
                                    terminal_psi::StructuralFieldType::Atomic(_)
                                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                                ) {
                                    continue;
                                }
                                let Some(scalar_type) = field.field_type.scalar_type() else {
                                    continue;
                                };
                                fields.push(field.id);
                                values.push((
                                    u32::MAX,
                                    fresh_field_snapshot(next_value, scalar_type, field.id, qualifications)?,
                                ));
                            }
                            Ok(PreparedCase {
                                outcome: &case.outcome,
                                identity: declared.id,
                                fields,
                                values,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    Ok(Some(PreparedCases {
                        source: resolved.argument.place,
                        path: resolved.argument.path,
                        cases,
                    }))
                }

                pub(crate) fn result<'a>(
                    state: &CheckedComposedUnitControlStatePlan,
                    binding_ordinal: u32,
                    operations: &'a OperationBuffer,
                ) -> Result<std::borrow::Cow<'a, StructuralBinding>, LoweringError>
                {
                    if let Some((_, produced)) = operations
                        .structural_values
                        .iter()
                        .find(|(ordinal, _)| *ordinal == binding_ordinal)
                    {
                        return Ok(std::borrow::Cow::Borrowed(produced));
                    }
                    let mut bindings = state.operations.iter().filter_map(|operation| {
                        let call = operation.call()?;
                        let result = call.structural_result()?;
                        (call.discard_result_on_return() == Some(false)
                            && result.binding_ordinal == binding_ordinal)
                            .then_some((&call.coordinate, result))
                    });
                    let (coordinate, binding) = bindings.next().ok_or(
                        LoweringError::Unsupported("Unit graph result binding missing"),
                    )?;
                    if bindings.next().is_some()
                        || coordinate.statement_index != binding.statement_index
                    {
                        return unsupported("Unit graph result binding duplicated or reordered");
                    }
                    let mut occurrences = operations.source_calls.iter().filter(|occurrence| {
                        occurrence.matches_statement(
                            state.state,
                            coordinate.statement_index as usize,
                            coordinate.call_ordinal as usize,
                        )
                    });
                    let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
                        "Unit graph result source call missing",
                    ))?;
                    if occurrences.next().is_some() {
                        return unsupported("Unit graph result source call duplicated");
                    }
                    let operation = operations
                        .iter()
                        .find(|operation| {
                            occurrence.terminal_site.operation() == Some(operation.id)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph result operation missing",
                        ))?;
                    let Some(produced) = operation.result.as_structural() else {
                        return unsupported("Unit graph call lost structural result");
                    };
                    Ok(std::borrow::Cow::Owned(produced.clone().into()))
                }
            }
            pub(crate) mod case_leaf_copy {
                use super::super::super::{
                    Operation, OperationKind, OperationResult, PlaceId, StructuralMultiplicity,
                    StructuralPlaceDeclaration, StructuralTypeId, unsupported,
                };
                use super::super::LoweringError;
                use crate::emission::operation_emission::buffer::OperationBuffer;
                use crate::emission::operation_emission::place_declarations::operation_result_place;
                use crate::emission::structural_observations;
                use checked_trees::{
                    CheckedMemberIdentity, CheckedUnitStructuralParameterPlan,
                    CheckedUnitStructuralPathSegment,
                };
                use language_semantics::Multiplicity;
                use terminal_psi::StructuralTypeDeclaration;

                pub(crate) fn emit(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    types: &[StructuralTypeDeclaration],
                    parameters: &[terminal_psi::StructuralParameterDeclaration],
                    source: PlaceId,
                    root_type: StructuralTypeId,
                    subject_type: StructuralTypeId,
                    subject_path: &[terminal_psi::StructuralPathSegment],
                    case_identity: &CheckedMemberIdentity,
                    field_identity: &CheckedMemberIdentity,
                    payload_path: &[CheckedUnitStructuralPathSegment],
                    target: &CheckedUnitStructuralParameterPlan,
                    destination: PlaceId,
                    operations: &mut OperationBuffer,
                ) -> Result<StructuralPlaceDeclaration, LoweringError> {
                    let mut path = Vec::with_capacity(payload_path.len() + 2);
                    path.push(CheckedUnitStructuralPathSegment::Case(
                        case_identity.clone(),
                    ));
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        field_identity.clone(),
                    ));
                    path.extend_from_slice(payload_path);
                    let selected = structural_observations::stored_value_path(
                        identities,
                        subject_type,
                        &path,
                        types,
                    )?;
                    let canonical = terminal_semantics::canonical_static_projection(
                        types.iter(),
                        root_type,
                        subject_path,
                    )
                    .and_then(|(mut prefix, _)| {
                        prefix.extend(selected.static_observation_path()?);
                        Some(prefix)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph case-payload copy requires static observation coordinates",
                    ))?;
                    let leaf = selected.field_type()?.as_structural().ok_or(
                        LoweringError::Unsupported(
                            "Unit graph case payload is not a structural leaf",
                        ),
                    )?;
                    if target.multiplicity != Multiplicity::Unrestricted {
                        return unsupported(
                            "case leaf observation requires copyable payload custody",
                        );
                    }
                    let (whole, projected, additional) = if let Some(parameter) = parameters
                        .iter()
                        .find(|parameter| parameter.place == source)
                    {
                        (
                            &parameter.qualifications,
                            &parameter.projected_qualifications,
                            &parameter.additional_input_qualifications,
                        )
                    } else {
                        let result = operations.structural_result_contract(source).ok_or(
                            LoweringError::Unsupported(
                                "case leaf copy lost its source membership contract",
                            ),
                        )?;
                        (
                            &result.qualifications,
                            &result.projected_qualifications,
                            &result.additional_input_qualifications,
                        )
                    };
                    let storage_path = terminal_semantics::canonical_structural_storage_path(
                        types.iter(),
                        root_type,
                        &canonical,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "case leaf copy lost its exact storage path",
                    ))?;
                    let (qualifications, projected_qualifications) =
                        terminal_psi::StructuralPathQualification::select(
                            whole,
                            projected,
                            &storage_path,
                        );
                    let additional_input_qualifications =
                        terminal_psi::StructuralPathQualification::select_rows(
                            additional,
                            &storage_path,
                        );
                    let producer = operations.allocate();
                    operations.push(Operation::unsuspended(
                        producer,
                        OperationResult::Structural(terminal_psi::StructuralOperationResult {
                            qualification_establishments: Vec::new(),
                            place: destination,
                            structural_type: leaf,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications,
                            projected_qualifications,
                            additional_input_qualifications,
                            claims: Vec::new(),
                        }),
                        OperationKind::StructuralCaseLeafCopy {
                            source,
                            path: canonical,
                        },
                    ));
                    Ok(operation_result_place(destination, producer, leaf))
                }
            }
            pub(crate) mod cleanup {
                use super::super::{CheckedTrees, LoweringError, catalogs};
                use crate::emission::operation_emission::buffer::OperationBuffer;
                use crate::unit::attached_unit::argument_evaluation::{
                    Evaluation, ScalarReturnCleanup,
                };
                use crate::unit::attached_unit::completion_cleanup::{
                    self, CompletionCleanupCatalog, NormalExitSource,
                };
                use crate::unit::attached_unit::composed_control::emission::with_call_namespace;
                use crate::unit::unit_cleanup::CleanupRegion;
                use checked_trees::{
                    CheckedComposedUnitControlStatePlan, CheckedScalarBranchDestination,
                    FlowClaimJoinExit,
                };
                use terminal_psi::StructuralParameterDeclaration;

                pub(crate) mod reference_reads {

                    use crate::LoweringError;
                    use crate::emission::operation_emission::buffer::OperationBuffer;
                    use checked_trees::{
                        CheckedComposedUnitControlStatePlan, CheckedUnitEffectOperationPlan,
                    };
                    use semantic_vocabulary::PlaceId;
                    use terminal_psi::{Block, TerminalAffineCleanupAction, Terminator};

                    pub(crate) fn observed_reference_retirements(
                        state: &CheckedComposedUnitControlStatePlan,
                        operations: &OperationBuffer,
                        mut retirement: impl FnMut(
                            PlaceId,
                        )
                            -> Result<TerminalAffineCleanupAction, LoweringError>,
                    ) -> Result<Vec<(PlaceId, TerminalAffineCleanupAction)>, LoweringError>
                    {
                        state
                            .operations
                            .iter()
                            .filter_map(|operation| {
                                let CheckedUnitEffectOperationPlan::ReleaseReference {
                                    source:
                                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                            binding_ordinal,
                                        },
                                    path,
                                    defer_to_exit: true,
                                    ..
                                } = operation
                                else {
                                    return None;
                                };
                                if !path.is_empty() {
                                    return None;
                                }
                                operations
                                    .structural_values
                                    .iter()
                                    .find(|(ordinal, _)| ordinal == binding_ordinal)
                                    .filter(|(_, binding)| binding.path().is_empty())
                                    .map(|(_, binding)| binding.place())
                            })
                            .map(|place| retirement(place).map(|action| (place, action)))
                            .collect()
                    }

                    pub(crate) fn retire_observed_references(
                        retirements: &[(PlaceId, TerminalAffineCleanupAction)],
                        operations: &OperationBuffer,
                        blocks: &mut [Block],
                    ) {
                        let successors = blocks
                            .iter()
                            .map(|block| {
                                block
                                    .terminator
                                    .successor_edges()
                                    .into_iter()
                                    .map(|(edge, target)| {
                                        (edge, blocks.iter().position(|block| block.id == target))
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();
                        for (source, retirement) in retirements {
                            let source = *source;
                            let mut uses = vec![false; blocks.len()];
                            let mut observations = vec![false; blocks.len()];
                            let mut derived_storage = false;
                            for (position, block) in blocks.iter_mut().enumerate() {
                                for operation in &mut block.operations {
                                    let produces_storage = operation.result.structural().is_some();
                                    operation.kind.map_structural_uses(&mut |place| {
                                        if place == source {
                                            uses[position] = true;
                                            derived_storage |= produces_storage;
                                        }
                                        place
                                    });
                                    observations[position] |=
                                        operation.kind.structural_observation_source()
                                            == Some(source);
                                }
                                block.terminator.map_structural_uses(&mut |place| {
                                    uses[position] |= place == source;
                                    place
                                });
                                observations[position] |= matches!(block.terminator,
                                    Terminator::StructuralCase { source: observed, .. } if observed == source);
                            }
                            if derived_storage {
                                continue;
                            }
                            let mut future_uses = uses;
                            loop {
                                let mut changed = false;
                                for position in 0..blocks.len() {
                                    if !future_uses[position]
                                        && successors[position].iter().any(|(_, target)| {
                                            target.is_some_and(|target| future_uses[target])
                                        })
                                    {
                                        future_uses[position] = true;
                                        changed = true;
                                    }
                                }
                                if !changed {
                                    break;
                                }
                            }
                            let mut retirement_edges = Vec::new();
                            let mut dead = vec![false; blocks.len()];
                            let mut pending = Vec::new();
                            for (position, edges) in successors.iter().enumerate() {
                                if !observations[position] {
                                    continue;
                                }
                                for (edge, target) in edges {
                                    if operations
                                        .selected_cleanup_groups
                                        .iter()
                                        .any(|group| group.edge == *edge)
                                    {
                                        continue;
                                    }
                                    if target.is_none_or(|target| !future_uses[target])
                                        && !forwards_descriptor(
                                            &blocks[position].terminator,
                                            *edge,
                                            source,
                                        )
                                    {
                                        retirement_edges.push(*edge);
                                        if let Some(target) = target {
                                            pending.push(*target);
                                        }
                                    }
                                }
                            }
                            if retirement_edges.is_empty() {
                                continue;
                            }
                            while let Some(position) = pending.pop() {
                                if dead[position] {
                                    continue;
                                }
                                dead[position] = true;
                                pending.extend(
                                    successors[position]
                                        .iter()
                                        .filter_map(|(_, target)| *target),
                                );
                            }
                            if successors.iter().enumerate().any(|(position, edges)| {
                                !dead[position]
                                    && edges.iter().any(|(edge, target)| {
                                        target.is_some_and(|target| dead[target])
                                            && !retirement_edges.contains(edge)
                                    })
                            }) {
                                continue;
                            }
                            for (position, block) in blocks.iter_mut().enumerate() {
                                if dead[position] {
                                    remove_retirement(&mut block.terminator, retirement);
                                }
                                promote_retirement(
                                    &mut block.terminator,
                                    source,
                                    retirement,
                                    &retirement_edges,
                                );
                            }
                        }
                    }

                    fn forwards_descriptor(
                        terminator: &Terminator,
                        edge: semantic_vocabulary::EdgeId,
                        source: PlaceId,
                    ) -> bool {
                        match terminator {
                            Terminator::Jump {
                                edge: selected,
                                structural_arguments,
                                ..
                            } if *selected == edge => structural_arguments
                                .iter()
                                .any(|argument| argument.place == source),
                            Terminator::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => [when_true, when_false].into_iter().any(|selected| {
                                selected.edge == edge
                                    && selected
                                        .structural_arguments
                                        .iter()
                                        .any(|argument| argument.place == source)
                            }),
                            _ => false,
                        }
                    }

                    fn promote_retirement(
                        terminator: &mut Terminator,
                        source: PlaceId,
                        retirement: &TerminalAffineCleanupAction,
                        edges: &[semantic_vocabulary::EdgeId],
                    ) {
                        let promote = |edge, actions: &mut Vec<TerminalAffineCleanupAction>| {
                            if edges.contains(&edge)
                                && !actions.iter().any(|action| action.place() == source)
                            {
                                actions.insert(0, retirement.clone());
                            }
                        };
                        match terminator {
                            Terminator::Jump {
                                edge,
                                cleanup_actions,
                                ..
                            } => promote(*edge, cleanup_actions),
                            Terminator::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => {
                                for edge in [when_true, when_false] {
                                    promote(edge.edge, &mut edge.cleanup_actions);
                                }
                            }
                            Terminator::StructuralCase { cases, .. } => {
                                for case in cases {
                                    promote(case.edge, &mut case.cleanup_actions);
                                }
                            }
                            _ => {}
                        }
                    }

                    fn remove_retirement(
                        terminator: &mut Terminator,
                        retirement: &TerminalAffineCleanupAction,
                    ) {
                        let retain = |actions: &mut Vec<TerminalAffineCleanupAction>| {
                            actions.retain(|action| action != retirement);
                        };
                        match terminator {
                            Terminator::Jump {
                                cleanup_actions, ..
                            }
                            | terminal_psi::return_or_return_structural_pattern!(
                                cleanup_actions,
                                ..
                            )
                            | Terminator::ReturnUnit {
                                cleanup_actions, ..
                            } => retain(cleanup_actions),
                            Terminator::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => {
                                retain(&mut when_true.cleanup_actions);
                                retain(&mut when_false.cleanup_actions);
                            }
                            Terminator::StructuralCase { cases, .. } => {
                                for case in cases {
                                    retain(&mut case.cleanup_actions);
                                }
                            }
                            Terminator::InvokeNoNormal { .. }
                            | Terminator::Crash { .. }
                            | Terminator::ExternalCompletion { .. } => {}
                        }
                    }
                }
                pub(crate) use reference_reads::{
                    observed_reference_retirements, retire_observed_references,
                };

                fn source<'a>(
                    state: &'a CheckedComposedUnitControlStatePlan,
                    control: &'a checked_trees::CheckedCleanupControlPlans,
                ) -> NormalExitSource<'a> {
                    NormalExitSource {
                        completion: state.normal_exit_cleanups.completion,
                        cleanups: control.normal_exits(state.normal_exit_cleanups),
                        trivial_affine_discards: &[],
                        operations: &state.operations,
                        parameters: &state.structural_parameters,
                    }
                }

                pub(crate) fn optional_coordinate(
                    control: &checked_trees::CheckedCleanupControlPlans,
                    state: &CheckedComposedUnitControlStatePlan,
                    statement_index: usize,
                    continuation: bool,
                ) -> Result<Option<FlowClaimJoinExit>, LoweringError> {
                    completion_cleanup::optional_coordinate(
                        source(state, control),
                        statement_index,
                        continuation,
                    )
                }

                pub(crate) fn destination(
                    control: &checked_trees::CheckedCleanupControlPlans,
                    state: &CheckedComposedUnitControlStatePlan,
                    destination: &CheckedScalarBranchDestination,
                ) -> Result<FlowClaimJoinExit, LoweringError> {
                    completion_cleanup::destination(source(state, control), destination)
                }
                pub(crate) fn coordinate(
                    control: &checked_trees::CheckedCleanupControlPlans,
                    state: &CheckedComposedUnitControlStatePlan,
                    statement_index: usize,
                    continuation: bool,
                ) -> Result<FlowClaimJoinExit, LoweringError> {
                    completion_cleanup::coordinate(
                        source(state, control),
                        statement_index,
                        continuation,
                    )
                }
                pub(crate) use completion_cleanup::selection;
                pub(crate) fn scalar_returns(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    control: &checked_trees::CheckedUnitScalarControlPlan,
                    catalogs: &mut catalogs::ComposedCatalogs<'_>,
                    parameters: &[StructuralParameterDeclaration],
                    evaluation: &Evaluation,
                    operations: &OperationBuffer,
                ) -> Result<Vec<ScalarReturnCleanup>, LoweringError> {
                    with_call_namespace(&mut catalogs.scalar_calls, |calls| {
                        let mut catalog = CompletionCleanupCatalog {
                            control: catalogs.cleanup_control,
                            types: &catalogs.structural_types,
                            type_ids: &catalogs.type_ids,
                            calls,
                        };
                        completion_cleanup::scalar_returns(
                            identities,
                            claims.bindings,
                            checked,
                            source(state, catalogs.cleanup_control),
                            control,
                            &mut catalog,
                            parameters,
                            evaluation,
                            operations,
                        )
                    })
                }
                pub(crate) fn for_exit_regions(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    exit: FlowClaimJoinExit,
                    catalogs: &mut catalogs::ComposedCatalogs<'_>,
                    parameters: &[StructuralParameterDeclaration],
                    evaluation: &Evaluation,
                    operations: &OperationBuffer,
                ) -> Result<Vec<CleanupRegion>, LoweringError> {
                    with_call_namespace(&mut catalogs.scalar_calls, |calls| {
                        let mut catalog = CompletionCleanupCatalog {
                            control: catalogs.cleanup_control,
                            types: &catalogs.structural_types,
                            type_ids: &catalogs.type_ids,
                            calls,
                        };
                        completion_cleanup::for_exit_regions(
                            identities,
                            claims.bindings,
                            checked,
                            source(state, catalogs.cleanup_control),
                            exit,
                            &mut catalog,
                            parameters,
                            evaluation,
                            operations,
                        )
                    })
                }
            }
            pub(crate) mod edges {
                use super::{
                    CheckedComposedUnitControlStatePlan, CheckedStructuralControlSuccessorPlan,
                };

                pub(crate) fn successors(
                    state: &CheckedComposedUnitControlStatePlan,
                ) -> Vec<&CheckedStructuralControlSuccessorPlan> {
                    state.successors()
                }
            }
            pub(crate) mod guarded {

                use super::super::super::super::{
                    CheckedComposedUnitControlTerminatorPlan, StructuralParameterDeclaration,
                    SuccessorEdge, block_id,
                };
                use super::super::super::{
                    Block, CheckedUnitEffectOperationPlan, ScalarType, Terminator,
                    ValueDeclaration, allocate_dense, edge_id, unsupported,
                };
                use super::super::{CheckedTrees, LoweringError, catalogs};
                use super::CheckedComposedUnitControlStatePlan;
                use super::emission::dispatch_copies::{
                    CompletedCopy, DispatchCopies, complete_dispatch_copies,
                };
                use crate::emission::operation_emission::buffer::{
                    OperationBuffer, StructuralBinding,
                };
                use crate::emission::value_successor;
                use crate::shared::checked_facts::flow_facts;
                use crate::unit::attached_unit::composed_control::emission::{
                    FrameValues, StateFrame,
                };

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn emit(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    catalogs: &mut catalogs::ComposedCatalogs,
                    parameters: &[StructuralParameterDeclaration],
                    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
                    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                    values: &mut Vec<ValueDeclaration>,
                    erased_parameters: &[ValueDeclaration],
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    owner: symbols::SymbolHandle,
                    sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
                ) -> Result<Option<Terminator>, LoweringError> {
                    let CheckedComposedUnitControlTerminatorPlan::Guarded {
                        arms,
                        fallback,
                        return_values,
                    } = &state.terminator
                    else {
                        return Ok(None);
                    };
                    let guards = flow_facts(checked).guarded_exits.exits.span(*arms).ok_or(
                        LoweringError::Unsupported("guarded structural guards are stale"),
                    )?;
                    let state_frame = StateFrame::new(checked, owner, state, erased_parameters);
                    for (position, guard) in guards.iter().enumerate() {
                        let condition = state_frame.evaluate(
                            catalogs,
                            parameters,
                            sequence,
                            FrameValues {
                                source_value_count: values.len(),
                                evaluation,
                                values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            },
                            |mut frame| {
                                frame.guard_value(identities, guard.guard_statement_ordinal)
                            },
                        )?;
                        if condition.scalar_type != ScalarType::Boolean {
                            return unsupported("ordered return guard is not Boolean");
                        }
                        let selected = block_id(allocate_dense(next_block)?);
                        let exhausted = position + 1 == guards.len() && fallback.is_none();
                        let remaining = if exhausted {
                            selected
                        } else {
                            block_id(allocate_dense(next_block)?)
                        };
                        let edge =
                            |target, counter: &mut u64| -> Result<SuccessorEdge, LoweringError> {
                                Ok(value_successor(edge_id(allocate_dense(counter)?), target))
                            };
                        evaluation.blocks.push(Block {
                            id: evaluation.current,
                            parameters: std::mem::take(&mut evaluation.parameters),
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            structural_parameters: std::mem::take(
                                &mut evaluation.block_structural_parameters,
                            ),
                            operations: operations[evaluation.operation_start..].to_vec(),
                            terminator: Terminator::Conditional {
                                condition: condition.id,
                                when_true: edge(selected, next_edge)?,
                                when_false: edge(remaining, next_edge)?,
                            },
                        });
                        let retained_bindings = operations.structural_values.len();
                        let mut selected_evaluation = evaluation.branch(selected, operations.len());
                        let mut selected_values = values.clone();
                        let mut selected_sequence = sequence.branch();
                        operations.storage_observations.clear();
                        let terminator = emit_return(
                            identities,
                            checked,
                            state,
                            &guard.destination,
                            &return_values[position],
                            catalogs,
                            parameters,
                            claims,
                            &mut selected_evaluation,
                            &mut selected_values,
                            erased_parameters,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                            owner,
                            &mut selected_sequence,
                        )?;
                        if exhausted {
                            evaluation.blocks.append(&mut selected_evaluation.blocks);
                            std::mem::swap(&mut evaluation.blocks, &mut selected_evaluation.blocks);
                            *evaluation = selected_evaluation;
                            *values = selected_values;
                            return Ok(Some(terminator));
                        }

                        selected_evaluation.blocks.push(Block {
                            id: selected_evaluation.current,
                            parameters: selected_evaluation.parameters,
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            structural_parameters: selected_evaluation.block_structural_parameters,
                            operations: operations[selected_evaluation.operation_start..].to_vec(),
                            terminator,
                        });
                        evaluation.blocks.extend(selected_evaluation.blocks);
                        evaluation.current = remaining;
                        evaluation.operation_start = operations.len();
                        operations.structural_values.truncate(retained_bindings);
                        operations.storage_observations.clear();
                    }
                    let fallback_destination = fallback.as_ref().ok_or(
                        LoweringError::Unsupported("ordered return lost its fallback destination"),
                    )?;
                    let fallback = return_values.last().filter(|_| fallback.is_some()).ok_or(
                        LoweringError::Unsupported(
                            "ordered return lost its exhaustive final destination",
                        ),
                    )?;
                    emit_return(
                        identities,
                        checked,
                        state,
                        fallback_destination,
                        fallback,
                        catalogs,
                        parameters,
                        claims,
                        evaluation,
                        values,
                        erased_parameters,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        owner,
                        sequence,
                    )
                    .map(Some)
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn emit_return(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    destination: &checked_trees::CheckedScalarBranchDestination,
                    returned: &checked_trees::CheckedGuardedReturnValue,
                    catalogs: &mut catalogs::ComposedCatalogs,
                    parameters: &[StructuralParameterDeclaration],
                    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
                    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                    values: &mut Vec<ValueDeclaration>,
                    erased_parameters: &[ValueDeclaration],
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    owner: symbols::SymbolHandle,
                    sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
                ) -> Result<Terminator, LoweringError> {
                    let operation = &returned.operation;
                    let copied_values =
                        DispatchCopies::new(checked, state, owner, catalogs, erased_parameters)
                            .capture(
                                identities,
                                &returned.copied_payloads,
                                parameters,
                                FrameValues {
                                    source_value_count: values.len(),
                                    evaluation,
                                    values,
                                    next_value,
                                    next_block,
                                    next_edge,
                                    operations,
                                },
                                sequence,
                            )?;
                    super::super::emission::emit_call_operations(
                        identities,
                        checked,
                        owner,
                        state,
                        std::slice::from_ref(operation),
                        catalogs,
                        parameters,
                        evaluation,
                        values,
                        erased_parameters,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        sequence,
                    )?;
                    if operations.invocation_completion.is_some() {
                        sequence.require_closed()?;
                        return operations.take_invocation_completion(next_edge)?.ok_or(
                            LoweringError::Unsupported(
                                "ordered no-normal return lost its evaluated invocation",
                            ),
                        );
                    }
                    if !operation.has_normal_continuation() {
                        return unsupported(
                            "ordered no-normal value lost its evaluated invocation",
                        );
                    }
                    let exit =
                        super::cleanup::destination(catalogs.cleanup_control, state, destination)?;
                    complete_structural_return(
                        identities,
                        claims,
                        checked,
                        state,
                        exit,
                        operation,
                        catalogs,
                        parameters,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        &copied_values,
                    )
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn complete_structural_return(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    exit: checked_trees::FlowClaimJoinExit,
                    operation: &CheckedUnitEffectOperationPlan,
                    catalogs: &mut catalogs::ComposedCatalogs,
                    parameters: &[StructuralParameterDeclaration],
                    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    copied_values: &[CompletedCopy],
                ) -> Result<Terminator, LoweringError> {
                    let binding = return_binding(operation, operations)?;
                    let mut regions = super::cleanup::for_exit_regions(
                        identities, claims, checked, state, exit, catalogs, parameters, evaluation,
                        operations,
                    )?;
                    super::cleanup::selection(
                        state.state,
                        &state.operations,
                        evaluation,
                        operations,
                        &mut regions,
                    )?;
                    regions.splice(
                        0..0,
                        complete_dispatch_copies(
                            copied_values,
                            evaluation,
                            operations,
                            &catalogs.structural_types,
                        )?,
                    );
                    emit_return_binding(
                        identities, checked, binding, regions, catalogs, evaluation, values,
                        next_value, next_block, next_edge, operations,
                    )
                }

                pub(crate) fn return_binding(
                    operation: &CheckedUnitEffectOperationPlan,
                    operations: &OperationBuffer,
                ) -> Result<StructuralBinding, LoweringError> {
                    let result =
                        operation
                            .structural_result_binding()
                            .ok_or(LoweringError::Unsupported(
                                "ordered return has no structural producer",
                            ))?;
                    let binding = operations
                        .structural_values
                        .iter()
                        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
                        .map(|(_, value)| value.clone())
                        .ok_or(LoweringError::Unsupported(
                            "ordered return value was not established",
                        ))?;
                    Ok(binding)
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn emit_return_binding(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    binding: StructuralBinding,
                    regions: Vec<crate::unit::unit_cleanup::CleanupRegion>,
                    catalogs: &mut catalogs::ComposedCatalogs,
                    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                ) -> Result<Terminator, LoweringError> {
                    let source = binding.place();
                    let mut returned_claims = binding.claims().to_vec();
                    returned_claims.sort_by_key(|binding| binding.claim);
                    let completion = Terminator::ReturnStructural {
                        edge: edge_id(allocate_dense(next_edge)?),
                        source,
                        returned_claims,
                        cleanup_actions: Vec::new(),
                    };
                    let (completion, arrival) =
                        crate::unit::attached_unit::structural_returns::materialize(
                            identities,
                            checked,
                            &terminal_psi::StructuralArgument {
                                place: source,
                                path: binding.path().to_vec(),
                                access: terminal_psi::StructuralAccess::Owned,
                            },
                            binding.structural_type(),
                            binding.multiplicity(),
                            completion,
                            regions,
                            &mut catalogs.next_place,
                            &mut catalogs.temporary_places,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        )?;
                    if let Some(arrival) = arrival {
                        evaluation.blocks.push(arrival);
                    }
                    Ok(completion)
                }
            }
            pub(crate) mod result_custody {
                use super::super::super::CheckedUnitEffectOperationPlan;
                use super::super::CheckedTrees;
                use super::CheckedComposedUnitControlStatePlan;

                #[derive(Clone, Copy)]
                pub(crate) struct LoanBacking {
                    pub(crate) binding: u32,
                    pub(crate) completed: bool,
                    pub(crate) retiring_carrier: Option<u32>,
                }

                pub(crate) fn local_loan_backing(
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    binding_ordinal: u32,
                ) -> Option<u32> {
                    let (position, operation) =
                        state.operations.iter().enumerate().find(|(_, operation)| {
                            operation
                                .structural_result_binding()
                                .is_some_and(|result| result.binding_ordinal == binding_ordinal)
                        })?;
                    let lent = established_loan_source(checked, operation)?;
                    inline_local_backing(state.operations[..position].iter(), lent)
                }

                pub(crate) fn transferred_loan_backing(
                    checked: &CheckedTrees,
                    operations: &[CheckedUnitEffectOperationPlan],
                    transfer: &checked_trees::CheckedStructuralControlTransferPlan,
                ) -> Option<LoanBacking> {
                    use checked_trees::CheckedStructuralControlTransferSourcePlan as Transfer;
                    if let Some(binding) = transfer.retained_loan_backing {
                        if let Transfer::ProjectedLoan { subject, .. } = &transfer.source
                            && subject.source.current_result_binding(operations) == Some(binding)
                            && !subject.path.contains(
                                &checked_trees::CheckedUnitStructuralPathSegment::Referent,
                            )
                        {
                            return None;
                        }
                        let retiring_carrier = match &transfer.source {
                            Transfer::ProjectedLoan {
                                subject,
                                retired_carrier_backing: Some(backing),
                                ..
                            } if *backing == binding => {
                                subject.source.current_result_binding(operations)
                            }
                            _ => None,
                        };
                        return Some(LoanBacking {
                            binding,
                            completed: false,
                            retiring_carrier,
                        });
                    }
                    match &transfer.source {
                        Transfer::EstablishedValue {
                            operands,
                            establishment,
                            presentation,
                        } => {
                            if !matches!(
                                presentation.path.first(),
                                Some(checked_trees::CheckedUnitStructuralPathSegment::Referent)
                            ) {
                                return None;
                            }
                            let lent = established_loan_source(checked, establishment)?;
                            let binding = inline_local_backing(
                                operations.iter().chain(operands.iter()),
                                lent,
                            )?;
                            let completed = operands.iter().any(|operation| {
                                operation
                                    .structural_result_binding()
                                    .is_some_and(|result| result.binding_ordinal == binding)
                            });
                            Some(LoanBacking {
                                binding,
                                completed,
                                retiring_carrier: None,
                            })
                        }
                        Transfer::StructuralResult { binding_ordinal } => {
                            let carrier = *binding_ordinal;
                            let (position, operation) =
                                operations.iter().enumerate().find(|(_, operation)| {
                                    operation
                                        .structural_result_binding()
                                        .is_some_and(|result| result.binding_ordinal == carrier)
                                })?;
                            let lent = established_loan_source(checked, operation)?;
                            let binding =
                                inline_local_backing(operations[..position].iter(), lent)?;
                            Some(LoanBacking {
                                binding,
                                completed: false,
                                retiring_carrier: None,
                            })
                        }
                        _ => None,
                    }
                }

                fn inline_local_backing<'plans>(
                    operations: impl DoubleEndedIterator<Item = &'plans CheckedUnitEffectOperationPlan>,
                    lent: &checked_trees::CheckedUnitStructuralArgumentPlan,
                ) -> Option<u32> {
                    if lent
                        .path
                        .contains(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
                    {
                        return None;
                    }
                    lent.source.current_result_binding(operations)
                }

                fn established_loan_source<'plans>(
                    checked: &'plans CheckedTrees,
                    operation: &'plans CheckedUnitEffectOperationPlan,
                ) -> Option<&'plans checked_trees::CheckedUnitStructuralArgumentPlan>
                {
                    operation
                        .established_reference_source(&checked.facts.values.structural_values)
                        .or_else(|| {
                            let value = match operation {
                                CheckedUnitEffectOperationPlan::EstablishLocal(
                                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                                        value,
                                        ..
                                    },
                                ) => *value,
                                _ => return None,
                            };
                            let nodes = &checked.facts.values.structural_values.nodes;
                            match &nodes.get(nodes.is_valid(value).then_some(value)?).kind {
                                checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                                    source,
                                    ..
                                }
                                | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                                    source,
                                    ..
                                } => Some(source),
                                _ => None,
                            }
                        })
                }

                pub(crate) fn loan_subject<'plans>(
                    source: &'plans checked_trees::CheckedStructuralControlTransferSourcePlan,
                    values: &'plans checked_trees::CheckedStructuralValuePlans,
                ) -> Option<(
                    &'plans checked_trees::CheckedUnitStructuralArgumentPlan,
                    bool,
                )> {
                    use checked_trees::{
                        CheckedStructuralControlTransferSourcePlan as Source,
                        CheckedUnitStructuralPathSegment as Path,
                    };
                    match source {
                        Source::ProjectedLoan { subject, .. } => Some((subject, false)),
                        Source::EstablishedValue {
                            establishment,
                            presentation,
                            ..
                        } if matches!(presentation.path.first(), Some(Path::Referent)) => {
                            establishment
                                .established_reference_source(values)
                                .map(|subject| (subject, presentation.path.len() != 1))
                        }
                        _ => None,
                    }
                }
            }
            pub(crate) mod returns {
                use super::super::super::super::StructuralResultDeclaration;
                use super::super::super::{
                    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalMachineResult,
                    ValueDeclaration, allocate_dense, lookup_type_id, place_id,
                    terminal_scalar_type, value_id,
                };
                use super::super::{CheckedTrees, LoweringError, catalogs};
                use crate::emission::borrowed_window::terminal_multiplicity;
                use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
                use checked_trees::CheckedControlResultPlan;

                pub(crate) fn result(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    machine: symbols::SymbolHandle,
                    entry_state: symbols::SymbolHandle,
                    plan: &CheckedControlResultPlan,
                    reference_sources: &[checked_trees::CheckedReturnedReferencePlan],
                    parameters: &[terminal_psi::StructuralParameterDeclaration],
                    catalogs: &mut catalogs::ComposedCatalogs,
                    places: &mut Vec<StructuralPlaceDeclaration>,
                ) -> Result<TerminalMachineResult, LoweringError> {
                    let result = match plan {
                        CheckedControlResultPlan::Unit => return Ok(TerminalMachineResult::Unit),
                        CheckedControlResultPlan::Scalar { primitive_type } => {
                            let declared = crate::unit::attached_unit::parameters::unit_scalar_result_type(
                                checked,
                                entry_state,
                                terminal_scalar_type(*primitive_type)?,
                                catalogs
                                    .prepared_scalar_qualifications
                                    .ok_or(LoweringError::Unsupported(
                                        "Unit graph scalar result has no qualification namespace",
                                    ))?,
                            )?;
                            return Ok(TerminalMachineResult::Scalar(ValueDeclaration {
                                qualifications: declared.qualifications,
                                id: value_id(allocate_dense(&mut catalogs.next_value)?),
                                scalar_type: declared.scalar_type,
                            }));
                        }
                        CheckedControlResultPlan::Structural(result) => result,
                    };
                    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
                    places.push(StructuralPlaceDeclaration {
                        id: place,
                        kind: StructuralPlaceKind::Result,
                    });
                    let structural_type =
                        lookup_type_id(&catalogs.type_ids, &result.type_identity)?;
                    let reference_sources =
                        crate::unit::attached_unit::parameters::lower_reference_result_sources(
                            identities,
                            checked,
                            reference_sources,
                            structural_type,
                            &catalogs.structural_types,
                            parameters,
                        )?;
                    Ok(TerminalMachineResult::Structural(
                        StructuralResultDeclaration {
                            reference_sources,
                            place,
                            structural_type,
                            multiplicity: terminal_multiplicity(result.multiplicity),
                            qualifications: {
                                let mut qualifications = result
                                    .qualifications
                                    .iter()
                                    .map(|domain| {
                                        lookup_domain_id_for_carrier(&catalogs.domain_ids, *domain, structural_type)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                                qualifications.extend(
                                    crate::unit::attached_unit::parameters::authored_result_whole_domains(
                                        checked, machine,
                                    )
                                    .iter()
                                    .map(|domain| {
                                        lookup_domain_id_for_carrier(&catalogs.domain_ids, *domain, structural_type)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                );
                                qualifications.sort();
                                qualifications.dedup();
                                qualifications
                            },
                            projected_qualifications:
                                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                                    identities,
                                    &crate::unit::attached_unit::parameters::result_projected_qualifications(
                                        checked,
                                        machine,
                                        checked
                                            .typed
                                            .machine(machine)
                                            .and_then(|machine| checked.typed.machine_states(machine).first())
                                            .ok_or(LoweringError::Unsupported("result has no entry state"))?
                                            .return_type,
                                    )?,
                                    &catalogs.domain_ids,
                                    structural_type,
                                    &catalogs.structural_types,
                                )?,
                        },
                    ))
                }
            }
            pub(crate) mod subslices {
                use super::super::super::super::StructuralParameterDeclaration;
                use super::super::super::view_ranges::{self, ViewRangeSite};
                use super::super::super::{
                    PlaceId, StructuralPlaceDeclaration, ValueDeclaration, unsupported,
                };
                use super::super::catalogs::ComposedCatalogs;
                use super::super::{CheckedTrees, LoweringError};
                use super::CheckedComposedUnitControlStatePlan;
                use crate::emission::operation_emission::buffer::OperationBuffer;
                use crate::unit::attached_unit::argument_evaluation::Evaluation;
                use crate::unit::attached_unit::composed_control::emission::{
                    FrameValues, StateFrame,
                };
                use checked_trees::CheckedStructuralControlTransferSourcePlan;

                pub(crate) fn transfer_range(
                    source: &CheckedStructuralControlTransferSourcePlan,
                ) -> Option<&checked_trees::CheckedUnitStructuralArgumentPlan> {
                    match source {
                        CheckedStructuralControlTransferSourcePlan::ViewSubslice(argument) => {
                            Some(argument)
                        }
                        _ => None,
                    }
                }

                #[allow(clippy::too_many_arguments)]
                pub(crate) fn emit(
                    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                    checked: &CheckedTrees,
                    state: &CheckedComposedUnitControlStatePlan,
                    statement_ordinal: u32,
                    transfer: &CheckedStructuralControlTransferSourcePlan,
                    state_parameters: &[StructuralParameterDeclaration],
                    evaluation: &mut Evaluation,
                    destination: PlaceId,
                    bindings: &crate::expression_preparation::bindings::ScalarBindings,
                    values: &mut Vec<ValueDeclaration>,
                    next_value: &mut u64,
                    next_block: &mut u64,
                    next_edge: &mut u64,
                    operations: &mut OperationBuffer,
                    catalogs: &mut ComposedCatalogs<'_>,
                    places: &mut Vec<StructuralPlaceDeclaration>,
                    owner: symbols::SymbolHandle,
                    erased_parameters: &[ValueDeclaration],
                    sequence: &mut super::super::emission::OperationSequence,
                ) -> Result<StructuralPlaceDeclaration, LoweringError> {
                    let Some(argument) = transfer_range(transfer) else {
                        return unsupported("Unit graph transfer is not a view subslice");
                    };
                    let range = view_ranges::range(argument).ok_or(LoweringError::Unsupported(
                        "Unit graph subslice transfer lost its range",
                    ))?;
                    let (source, establishment) = view_ranges::stage_source(
                        identities,
                        checked,
                        state.state,
                        statement_ordinal,
                        argument,
                        catalogs.structural_types.as_ref(),
                        bindings,
                        &mut catalogs.next_place,
                        values,
                        next_value,
                        operations,
                    )?;
                    if let Some(declaration) = establishment {
                        places.push(declaration);
                    }
                    let site = ViewRangeSite {
                        state: state.state,
                        statement: statement_ordinal,
                        range: range.endpoints(),
                    };

                    let source_value_count = values.len();
                    let result_type = source.structural_type;
                    StateFrame::new(checked, owner, state, erased_parameters).evaluate(
                        catalogs,
                        state_parameters,
                        sequence,
                        FrameValues {
                            source_value_count,
                            evaluation,
                            values,
                            next_value,
                            next_block,
                            next_edge,
                            operations,
                        },
                        |frame| {
                            let (mut environment, emission) = frame.split();
                            view_ranges::emit(
                                identities,
                                checked,
                                site,
                                source,
                                result_type,
                                destination,
                                emission.evaluation,
                                source_value_count,
                                emission.values,
                                emission.next_value,
                                emission.next_block,
                                emission.next_edge,
                                emission.operations,
                                emission.calls,
                                &mut environment,
                            )
                        },
                    )
                }
            }

            pub(crate) use admission::AdmittedGraph;
            pub(crate) use admission::admit;
            pub(crate) use custody_frontiers::prepare;
            pub(crate) use edges::successors;
            pub(crate) use emission::emit;
            pub(crate) use topology::live;

            pub(crate) fn owner(
                checked: &super::CheckedTrees,
                state: symbols::SymbolHandle,
            ) -> Result<symbols::SymbolHandle, super::LoweringError> {
                Ok(state_declaration(checked, state)?.0.symbol)
            }
        }
        pub(crate) use callable::admit as admit_callable;
        pub(crate) use state_graph::case_emission::result as state_graph_result;
        pub(crate) use state_graph::live;

        #[derive(Default)]
        pub(crate) struct ComposedOccurrences {
            pub(crate) source_returns:
                Vec<crate::expression_preparation::qualifications::SourceScalarReturn>,
            pub(crate) claim_introductions: Vec<crate::emission::claim_bindings::ClaimIntroduction>,
            pub(crate) claim_edge_bindings: Vec<crate::emission::claim_bindings::ClaimEdgeBinding>,
            pub(crate) completed_cleanup_frontiers:
                Vec<crate::emission::selected_continuations::CompletedCleanupFrontier>,
            pub(crate) selected_cleanup_groups:
                Vec<crate::emission::selected_continuations::SelectedCleanupGroup>,
            pub(crate) quotient_adaptations:
                Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
            pub(crate) quotient_calls:
                Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
            pub(crate) source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
            pub(crate) selected_comparisons: Vec<lowered_psi::LoweredSelectedComparisonOccurrence>,
            pub(crate) source_checked_obligations: Vec<semantic_vocabulary::ObligationId>,
            pub(crate) state_blocks: Vec<lowered_psi::LoweredStateBlock>,
        }

        impl ComposedOccurrences {
            fn retain(
                &mut self,
                operations: crate::emission::operation_emission::buffer::OperationBuffer,
            ) {
                self.source_returns.extend(operations.source_returns);
                self.completed_cleanup_frontiers
                    .extend(operations.completed_cleanup_frontiers);
                self.selected_cleanup_groups
                    .extend(operations.selected_cleanup_groups);
                self.claim_introductions
                    .extend(operations.claim_introductions);
                self.claim_edge_bindings
                    .extend(operations.claim_edge_bindings);
                self.source_calls.extend(operations.source_calls);
                self.quotient_adaptations
                    .extend(operations.quotient_adaptations);
                self.quotient_calls.extend(operations.quotient_calls);
                self.selected_comparisons
                    .extend(operations.selected_comparisons);
                self.source_checked_obligations
                    .extend(operations.source_checked_obligations);
            }
        }
    }
}
