//! Prepare actual executable and native operand inputs once for handle replay.
//!
//! The ordinary selected-provider owner has already admitted bodies and foreign
//! entry plans. This module reuses those joins, then reconstructs current scalar
//! origins. It does not manufacture a permission row or classify a close.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{IdTable, MachineId};
use terminal_psi::BoundaryParameterKind;
use terminal_semantics::descriptor_results::{
    DescriptorMachine, ResidentValueCaptures, TerminalScalarCaptures,
    terminal_borrowed_storage_graph_from,
};

use super::context::ReviewContext;
use super::windows_handle_calls::{
    CallTarget, Caller, HandleCallGraph, RuntimeArguments, RuntimeCall,
};
use super::windows_handle_contracts::{ReviewedHandleCall, WindowsHandleContract, reviewed_call};
use super::windows_handle_origins::{HandleOrigins, ScalarSubject};
use super::windows_handle_values::{HandleValueReader, LocalScalarExpressions};

pub(super) struct NativeHandleInput<'a> {
    pub call: RuntimeCall<'a>,
    pub reviewed: ReviewedHandleCall<'a>,
    /// Original source-level scalar parameter ordinal to its exact SSA operand.
    /// Native pointer/count expansion stays in the reviewed physical call plan.
    pub scalars: BTreeMap<usize, ScalarSubject>,
}

/// Immutable scalar rosters belong to the same body and borrowed-storage
/// graph coordinate. Replay does
/// not rebuild a typing/capture inventory at each operation or caller edge.
pub(super) struct PreparedHandleMachine {
    pub storage_index: usize,
    pub expressions: LocalScalarExpressions,
    pub operands: TerminalScalarCaptures,
    pub resident_transfers: ResidentValueCaptures,
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
    pub machines: IdTable<MachineId, Option<PreparedHandleMachine>>,
    pub close_sites: BTreeSet<Caller>,
    pub duplicate_sites: BTreeSet<Caller>,
}

impl<'a> WindowsHandleInputs<'a> {
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
        let (origins, machines) = {
            let mut reader = HandleValueReader::new(context, &calls, &storage, module)?;
            let mut machines = Vec::new();
            for (storage_index, machine) in storage.iter().enumerate() {
                let expressions = reader.local_expressions(machine.id)?;
                machines.push((machine.id, (storage_index, expressions)));
            }
            let origins = HandleOrigins::derive(module, &calls, &mut reader, roots)?;
            let (mut operands, mut resident_transfers) = reader.into_captures();
            machines.sort_by_key(|(identity, _)| *identity);
            let machines = machines
                .into_iter()
                .map(|(identity, (storage_index, expressions))| {
                    let operands = operands
                        .get_mut(identity)
                        .and_then(Option::take)
                        .ok_or("canonical replay lost its prepared scalar operand roster")?;
                    let resident_transfers = resident_transfers
                        .get_mut(identity)
                        .and_then(Option::take)
                        .ok_or("canonical replay lost its prepared resident transfer roster")?;
                    Ok((
                        identity,
                        Some(PreparedHandleMachine {
                            storage_index,
                            expressions,
                            operands,
                            resident_transfers,
                        }),
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let machines = IdTable::from_rows_with_gaps(machines, 65_536).map_err(|reason| {
                format!("handle machine replay inventory is invalid: {reason:?}")
            })?;
            (origins, machines)
        };
        Ok(Self {
            calls,
            storage,
            natives,
            opaque_calls,
            intrinsic_calls,
            origins,
            machines,
            close_sites,
            duplicate_sites,
        })
    }
}
