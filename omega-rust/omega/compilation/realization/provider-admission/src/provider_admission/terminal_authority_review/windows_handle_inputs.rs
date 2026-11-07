//! Prepare actual executable and native operand inputs once for handle replay.
//!
//! The ordinary selected-provider owner has already admitted bodies and foreign
//! entry plans. This module reuses those joins, then reconstructs current scalar
//! origins. It does not manufacture a permission row or classify a close.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{IdTable, IntegerValue, MachineId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{BoundaryParameterKind, OperationKind};
use terminal_semantics::descriptor_lifetimes::DescriptorPosition;
use terminal_semantics::descriptor_results::{
    DescriptorMachine, ScalarCaptureAnalysis, terminal_borrowed_storage_graph_from,
};

use super::context::ReviewContext;
use super::windows_handle_activations::ActivationPaths;
use super::windows_handle_calls::{
    CallTarget, Caller, HandleCallGraph, RuntimeArguments, RuntimeCall,
};
use super::windows_handle_conditions::HandleFlowConditions;
use super::windows_handle_contracts::{ReviewedHandleCall, WindowsHandleContract, reviewed_call};
use super::windows_handle_origins::{HandleOrigins, ScalarSubject};
use super::windows_handle_values::HandleValueReader;

pub(super) struct NativeHandleInput<'a> {
    pub call: RuntimeCall<'a>,
    pub reviewed: ReviewedHandleCall<'a>,
    /// Original source-level scalar parameter ordinal to its exact SSA operand.
    /// Native pointer/count expansion stays in the reviewed physical call plan.
    pub scalars: BTreeMap<usize, ScalarSubject>,
}

pub(super) struct WindowsHandleInputs<'a> {
    pub calls: HandleCallGraph<'a>,
    pub storage: Vec<DescriptorMachine>,
    pub natives: BTreeMap<Caller, NativeHandleInput<'a>>,
    /// Unknown native executions remain intervening calls even with no handle
    /// operand. Absence of a passed integer is not a preservation contract.
    pub opaque_calls: BTreeMap<Caller, RuntimeCall<'a>>,
    pub intrinsic_calls: BTreeMap<Caller, RuntimeCall<'a>>,
    pub origins: HandleOrigins,
    pub activations: ActivationPaths,
    pub conditions: IdTable<MachineId, Option<HandleFlowConditions>>,
    scalar_captures: IdTable<MachineId, Option<ScalarCaptureAnalysis>>,
    pub close_sites: BTreeSet<Caller>,
    pub duplicate_sites: BTreeSet<Caller>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NativeSuccessRequirement {
    DuplicateNonzero,
    ModeStatusZero,
    FileNotInvalid,
}

pub(super) struct NativeSuccessProof {
    caller: Caller,
    observation: DescriptorPosition,
    status: ValueId,
    requirement: NativeSuccessRequirement,
}

impl<'a> WindowsHandleInputs<'a> {
    /// Prove success from a taken canonical guard after this exact native
    /// result. The same immutable owner supplies body, graph and conditions;
    /// no external receipt, caller-provided status or goal seeds the proof.
    pub(super) fn native_success_before(
        &mut self,
        caller: Caller,
        observation: DescriptorPosition,
        requirement: NativeSuccessRequirement,
    ) -> Result<NativeSuccessProof, String> {
        let input = self
            .natives
            .get(&caller)
            .ok_or("native success query has no reviewed original call")?;
        let expected = match requirement {
            NativeSuccessRequirement::DuplicateNonzero => WindowsHandleContract::DuplicateHandle,
            NativeSuccessRequirement::ModeStatusZero => WindowsHandleContract::QueryFileMode,
            NativeSuccessRequirement::FileNotInvalid => WindowsHandleContract::CreateFile,
        };
        if input.reviewed.contract != expected {
            return Err("native success query changes the reviewed API result protocol".into());
        }
        let machine = self
            .calls
            .machine(caller.machine)
            .ok_or("native result has no reached canonical body")?;
        let operation = machine
            .block(&caller.block)
            .and_then(|block| block.operations.get(caller.operation_index))
            .ok_or("native success query lost its original result operation")?;
        if !matches!(&operation.kind, OperationKind::BoundaryCall { boundary, .. } if Some(*boundary) == input.call.boundary)
            || caller.site
                != super::windows_handle_calls::CallSite::Invocation(
                    semantic_vocabulary::InvocationSite::Operation(operation.id),
                )
        {
            return Err("native success query substitutes its original boundary occurrence".into());
        }
        let status = operation
            .result
            .scalar_ref()
            .ok_or("native success query has no actual scalar result")?;
        let ScalarType::Integer(integer_type) = status.scalar_type else {
            return Err("native success status has a noninteger carrier".into());
        };
        let (literal, polarity) = match requirement {
            NativeSuccessRequirement::DuplicateNonzero => (0, false),
            NativeSuccessRequirement::ModeStatusZero => (0, true),
            NativeSuccessRequirement::FileNotInvalid => (-1, false),
        };
        let literal = IntegerValue::for_sign(integer_type.sign(), literal)
            .map_err(|_| "native success sentinel does not fit its admitted result carrier")?;
        let source = (
            caller.block,
            u32::try_from(caller.operation_index)
                .map_err(|_| "native result ordinal exceeds its canonical position")?,
        );
        self.scalar_captures.get_mut(caller.machine).and_then(Option::as_mut)
            .ok_or("native status has no prepared scalar-version context")?
            .require(source, observation, &[status.id])
        .map_err(|reason| {
            format!(
                "native success status is not this completed result at {observation:?}: {reason:?}"
            )
        })?;
        let condition = ScalarTerm::IntegerEqual {
            scalar_type: integer_type,
            left: Box::new(ScalarTerm::value(status.id, status.scalar_type)),
            right: Box::new(ScalarTerm::Integer {
                scalar_type: integer_type,
                value: literal,
            }),
        };
        let conditions: &super::windows_handle_conditions::ScalarConditions = self
            .conditions
            .get(caller.machine)
            .and_then(Option::as_ref)
            .and_then(|machine| machine.snapshots.get(&observation))
            .ok_or("native success observation has no actual reachable guard state")?;
        if !conditions.proves(&condition, polarity) {
            return Err(format!(
                "native {:?} at {caller:?} has no proved success guard before {observation:?}",
                input.reviewed.contract
            ));
        }
        Ok(NativeSuccessProof {
            caller,
            observation,
            status: status.id,
            requirement,
        })
    }

    pub(super) fn prepare(context: &'a ReviewContext<'_>) -> Result<Self, String> {
        let calls = HandleCallGraph::new(context)?;
        let module = context.input.module();
        let mut selected_bodies = Vec::new();
        let mut natives = BTreeMap::new();
        let mut opaque_calls = BTreeMap::new();
        let mut intrinsic_calls = BTreeMap::new();
        let mut roots = Vec::new();
        let mut close_sites = BTreeSet::new();
        let mut duplicate_sites = BTreeSet::new();
        for alternatives in calls.calls.values() {
            for &call in alternatives {
                match call.target {
                    CallTarget::Machine(callee) => {
                        let Some(boundary) = call.boundary else {
                            continue;
                        };
                        selected_bodies.push((boundary, callee));
                    }
                    CallTarget::Boundary(boundary) => {
                        let Some(reviewed) = reviewed_call(context, boundary)? else {
                            if opaque_calls.insert(call.caller, call).is_some() {
                                return Err(format!(
                                    "opaque handle intervention {:?} has conflicting executable alternatives",
                                    call.caller
                                ));
                            }
                            continue;
                        };
                        if !call.normal_return {
                            return Err(format!(
                                "reviewed handle API {boundary:?} has no normal completion"
                            ));
                        }
                        let RuntimeArguments::Explicit { scalar, .. } = call.arguments else {
                            return Err("native handle API has no actual explicit arguments".into());
                        };
                        let mut scalars = BTreeMap::new();
                        let mut scalar_index = 0;
                        for (ordinal, kind) in
                            reviewed.declaration.parameter_order.iter().enumerate()
                        {
                            if *kind == BoundaryParameterKind::Scalar {
                                let value = *scalar
                                    .get(scalar_index)
                                    .ok_or("native handle API omits an actual scalar argument")?;
                                scalar_index += 1;
                                let subject = ScalarSubject::Value {
                                    machine: call.caller.machine,
                                    value,
                                };
                                scalars.insert(ordinal, subject);
                            }
                        }
                        if scalar_index != scalar.len() {
                            return Err("native handle API has extra scalar arguments outside its admitted declaration".into());
                        }
                        if reviewed.contract == WindowsHandleContract::CloseHandle {
                            close_sites.insert(call.caller);
                        }
                        if reviewed.contract == WindowsHandleContract::DuplicateHandle {
                            duplicate_sites.insert(call.caller);
                        }
                        roots.extend(scalars.values().cloned());
                        if natives
                            .insert(
                                call.caller,
                                NativeHandleInput {
                                    call,
                                    reviewed,
                                    scalars,
                                },
                            )
                            .is_some()
                        {
                            return Err(format!(
                                "handle API {:?} has conflicting executable alternatives",
                                call.caller
                            ));
                        }
                    }
                    CallTarget::Intrinsic(_) => {
                        if intrinsic_calls.insert(call.caller, call).is_some() {
                            return Err(format!(
                                "intrinsic handle intervention {:?} has conflicting executable alternatives",
                                call.caller
                            ));
                        }
                    }
                }
            }
        }
        selected_bodies.sort_unstable();
        for pair in selected_bodies.windows(2) {
            if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
                return Err(format!(
                    "handle storage graph substitutes selected body for {:?}",
                    pair[0].0
                ));
            }
        }
        selected_bodies.dedup();
        let storage = terminal_borrowed_storage_graph_from(
            module,
            calls.machines().map(|(identity, _)| identity),
            &selected_bodies,
        )
        .map_err(|reason| format!("handle replay cannot reconstruct actual storage: {reason}"))?;
        let (origins, conditions, scalar_captures) = {
            let mut reader = HandleValueReader::new(context, &calls, &storage, module)?;
            let mut conditions = Vec::new();
            let mut scalar_captures = Vec::new();
            for machine in &storage {
                if calls.machine(machine.id).is_some() {
                    let mut captures = ScalarCaptureAnalysis::new(machine).map_err(|reason| {
                        format!("native scalar-version context is unavailable: {reason}")
                    })?;
                    let local_expressions = reader.local_expressions(machine.id)?;
                    conditions.push((
                        machine.id,
                        Some(HandleFlowConditions::derive(
                            machine,
                            calls
                                .machine(machine.id)
                                .ok_or("native guard has no reached canonical body")?,
                            &local_expressions,
                            &mut captures,
                            reader.scalar_captures(machine.id)?,
                        )?),
                    ));
                    scalar_captures.push((machine.id, Some(captures)));
                }
            }
            conditions.sort_by_key(|(identity, _)| *identity);
            scalar_captures.sort_by_key(|(identity, _)| *identity);
            let conditions = IdTable::from_rows_with_gaps(conditions, 65_536)
                .map_err(|reason| format!("handle condition inventory is invalid: {reason:?}"))?;
            let scalar_captures =
                IdTable::from_rows_with_gaps(scalar_captures, 65_536).map_err(|reason| {
                    format!("handle scalar-version inventory is invalid: {reason:?}")
                })?;
            (
                HandleOrigins::derive(module, &calls, &mut reader, roots)?,
                conditions,
                scalar_captures,
            )
        };
        let activations = ActivationPaths::derive(&origins)?;
        Ok(Self {
            calls,
            storage,
            natives,
            opaque_calls,
            intrinsic_calls,
            origins,
            activations,
            conditions,
            scalar_captures,
            close_sites,
            duplicate_sites,
        })
    }
}
