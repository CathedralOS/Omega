//! Read scalar operands from their current canonical value or storage version.
//!
//! Native output slots have analysis-owned identities, not invented SSA values.
//! A native output still owes its API success guard; a returned projection still
//! owes substitution of the actual checked callee and all its returning paths.
//! This reader identifies those obligations without declaring them proved.

use std::collections::BTreeMap;

use semantic_vocabulary::{BlockId, IdTable, MachineId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{BoundaryParameterKind, Operation, TerminalModule, Terminator};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    CapturedScalarOperand, DescriptorAnalysis, DescriptorCasePredicate, DescriptorContentsQuery,
    DescriptorMachine, DescriptorPayloadTarget, FlowFeasibility, PayloadEntryScalarIdentity,
    PayloadMembershipHistory, ResidentValueCapture, ResidentValueCaptures, TerminalScalarCaptures,
    TerminalScalarExpressions, projected_stored_sources_in, require_unchanged_entry_storage,
    terminal_resident_value_transfers,
};

use super::context::ReviewContext;
use super::windows_handle_calls::{
    CallSite, CallTarget, Caller, HandleCallGraph, RuntimeArguments,
};
use super::windows_handle_contracts::{WindowsHandleContract, reviewed_call};

pub(super) struct LocalScalarExpressions {
    pub identity: MachineId,
    pub definitions: IdTable<ValueId, Option<(ScalarType, Option<ScalarTerm>)>>,
    pub producers: IdTable<ValueId, Option<DescriptorPosition>>,
    parameters: IdTable<ValueId, Option<ScalarParameterSite>>,
    dominators: terminal_semantics::control_graph::DominatorTree,
}

enum ScalarParameterSite {
    Entry,
    Block(BlockId),
}

impl LocalScalarExpressions {
    pub(super) fn available_at(&self, value: ValueId, observation: DescriptorPosition) -> bool {
        if let Some(Some(source)) = self.producers.get(value) {
            return if source.0 == observation.0 {
                source.1 < observation.1
            } else {
                self.dominators.dominates(source.0, observation.0)
            };
        }
        match self.parameters.get(value).and_then(Option::as_ref) {
            Some(ScalarParameterSite::Entry) => true,
            Some(ScalarParameterSite::Block(block)) => {
                self.dominators.dominates(*block, observation.0)
            }
            None => false,
        }
    }
}

/// Every alternative is retained. A join is not permission to choose whichever
/// source would make a later close valid.
#[derive(Clone, Debug)]
pub(super) enum ScalarDefinition {
    Parameter(usize),
    Expression(ScalarTerm),
    EntryStorage(PayloadEntryScalarIdentity),
    /// A borrowed referent captured at entry, not the formal carrier's owned
    /// storage. Current backing and absence of intervening writes are replayed.
    BorrowedEntry(DescriptorLocation),
    /// The requested payload is inactive at this exact cut. It is no value;
    /// replay must match the caller's actual selected-case guard before it can
    /// exclude this returning alternative.
    InactiveCase(DescriptorCasePredicate),
    Stored(Vec<StoredScalarSource>),
    Arrivals(Vec<ScalarArrival>),
    CallResult(Caller),
}

#[derive(Clone, Debug)]
pub(super) struct ScalarArrival {
    pub predecessor: DescriptorPosition,
    pub successor_ordinal: usize,
    pub value: CapturedScalarOperand,
}

pub(super) struct ScalarReturn {
    pub position: DescriptorPosition,
    pub value: ScalarReturnValue,
}

pub(super) enum ScalarReturnValue {
    Value(ValueId),
    Storage(DescriptorLocation),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum StoredScalarSource {
    Scalar {
        position: DescriptorPosition,
        value: ScalarTerm,
    },
    /// Exact output parameter of one admitted native call. Neither successful
    /// initialization nor a live handle is inferred merely from this identity.
    NativeOutput { caller: Caller, parameter: usize },
    /// Copy/move captures this source before its destination is established.
    /// Later reads do not reload the source home after it may have changed.
    Transport {
        position: DescriptorPosition,
        source: DescriptorLocation,
    },
    /// The actual call's structural result owns this projection. The consumer
    /// enters that callee's real ReturnStructural edges before using its value.
    CallResult {
        caller: Caller,
        path: Vec<DescriptorPathSegment>,
    },
}

struct ValueProducer<'a> {
    position: DescriptorPosition,
    operation: &'a Operation,
}

/// Prepared once for an executable closure. Payload histories share the same
/// reference/view inventory as address resolution; neither reconstructs source
/// types as if they were observed contents.
pub(super) struct HandleValueReader<'a> {
    module: &'a TerminalModule,
    calls: &'a HandleCallGraph<'a>,
    graph: &'a [DescriptorMachine],
    graph_index: IdTable<MachineId, Option<usize>>,
    types: IdTable<MachineId, Option<TerminalScalarExpressions<'a>>>,
    producers: BTreeMap<(MachineId, ValueId), ValueProducer<'a>>,
    captured_scalars: IdTable<MachineId, Option<TerminalScalarCaptures>>,
    resident_transfers: IdTable<MachineId, Option<ResidentValueCaptures>>,
    native_writes:
        BTreeMap<(MachineId, DescriptorPosition), Vec<(DescriptorLocation, StoredScalarSource)>>,
    histories: IdTable<MachineId, Option<PayloadMembershipHistory<'a>>>,
    analysis: DescriptorAnalysis<'a>,
}

impl<'a> HandleValueReader<'a> {
    /// Immutable equations for ordinary scalar leaves. Native results and
    /// storage observations stay their original SSA binders; no call summary,
    /// current storage fact or source qualifier is substituted here.
    pub(super) fn local_expressions(
        &self,
        identity: MachineId,
    ) -> Result<LocalScalarExpressions, String> {
        let types = self
            .types
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("scalar expression catalog names an unreached machine")?;
        let machine = self
            .calls
            .machine(identity)
            .ok_or("handle machine has no reached canonical body")?;
        let mut block_rows = machine
            .blocks
            .iter()
            .map(|block| (block.id, true))
            .collect::<Vec<_>>();
        block_rows.sort_by_key(|(identity, _)| *identity);
        let block_count = block_rows.len();
        let blocks = IdTable::from_rows_with_gaps(block_rows, 65_536).map_err(|reason| {
            format!("scalar expression block namespace is invalid: {reason:?}")
        })?;
        if !blocks.get(machine.entry).copied().unwrap_or(false) {
            return Err("scalar expression control graph has no entry".into());
        }
        let outgoing = terminal_semantics::control_graph::successors(machine);
        if outgoing
            .values()
            .flatten()
            .any(|(_, target)| !blocks.get(*target).copied().unwrap_or(false))
            || terminal_semantics::control_graph::reverse_postorder(machine).len() != block_count
        {
            return Err("scalar expression control graph has absent or unreachable blocks".into());
        }
        let definitions = types
            .scalar_types()
            .map(|(value, scalar_type)| {
                let expression = match self.producers.get(&(identity, value)) {
                    Some(producer) => types.unexpanded_operation(producer.operation)?,
                    None => None,
                };
                Ok((value, Some((scalar_type, expression))))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let definitions = IdTable::from_rows_with_gaps(definitions, 65_536)
            .map_err(|reason| format!("scalar definition inventory is invalid: {reason:?}"))?;
        let producers = IdTable::from_rows_with_gaps(
            types.scalar_types().map(|(value, _)| {
                (
                    value,
                    self.producers
                        .get(&(identity, value))
                        .map(|producer| producer.position),
                )
            }),
            65_536,
        )
        .map_err(|reason| format!("scalar producer inventory is invalid: {reason:?}"))?;
        let mut parameters = machine
            .parameters
            .iter()
            .map(|parameter| (parameter.id, Some(ScalarParameterSite::Entry)))
            .collect::<Vec<_>>();
        for block in &machine.blocks {
            parameters.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.id, Some(ScalarParameterSite::Block(block.id)))),
            );
        }
        parameters.sort_by_key(|(value, _)| *value);
        let parameters = IdTable::from_rows_with_gaps(parameters, 65_536)
            .map_err(|reason| format!("scalar parameter inventory is invalid: {reason:?}"))?;
        Ok(LocalScalarExpressions {
            identity,
            definitions,
            producers,
            parameters,
            dominators: terminal_semantics::control_graph::dominators(machine),
        })
    }

    pub(super) fn scalar_captures(
        &self,
        identity: MachineId,
    ) -> Result<&TerminalScalarCaptures, String> {
        self.captured_scalars
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("scalar captures name an unreached machine".into())
    }

    pub(super) fn require_native_output(
        &self,
        caller: Caller,
        parameter: usize,
    ) -> Result<(), String> {
        let position = (
            caller.block,
            u32::try_from(caller.operation_index)
                .map_err(|_| "native output ordinal exceeds its canonical position")?,
        );
        let known = self
            .native_writes
            .get(&(caller.machine, position))
            .is_some_and(|writes| {
                writes.iter().any(|(_, source)| {
                    matches!(*source,
                StoredScalarSource::NativeOutput { caller: actual, parameter: slot }
                    if actual == caller && slot == parameter)
                })
            });
        if known {
            Ok(())
        } else {
            Err(format!(
                "handle output {:?}/{parameter} has no reviewed actual native writer",
                caller
            ))
        }
    }

    pub(super) fn value_position(
        &self,
        identity: MachineId,
        value: ValueId,
    ) -> Result<DescriptorPosition, String> {
        if let Some(producer) = self.producers.get(&(identity, value)) {
            return Ok(producer.position);
        }
        let machine = self
            .calls
            .machine(identity)
            .ok_or("handle value position names an unreached machine")?;
        if machine
            .parameters
            .iter()
            .any(|parameter| parameter.id == value)
        {
            return Ok((machine.entry, 0));
        }
        machine
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.id == value)
                    .then_some((block.id, 0))
            })
            .ok_or("handle value position has no canonical binder".into())
    }

    pub(super) fn new(
        context: &ReviewContext<'_>,
        calls: &'a HandleCallGraph<'a>,
        graph: &'a [DescriptorMachine],
        module: &'a TerminalModule,
    ) -> Result<Self, String> {
        let mut graph_rows = graph
            .iter()
            .enumerate()
            .map(|(index, machine)| (machine.id, Some(index)))
            .collect::<Vec<_>>();
        graph_rows.sort_by_key(|(identity, _)| *identity);
        let graph_index = IdTable::from_rows_with_gaps(graph_rows, 65_536)
            .map_err(|reason| format!("handle storage graph inventory is invalid: {reason:?}"))?;
        let mut type_rows = Vec::new();
        let mut producers = BTreeMap::new();
        for (identity, machine) in calls.machines() {
            if graph_index.get(identity).and_then(Option::as_ref).is_none() {
                return Err(format!(
                    "handle storage graph omits executable machine {identity:?}"
                ));
            }
            let mut defined = Vec::new();
            for parameter in machine.parameters.iter().chain(machine.block_parameters()) {
                defined.push(parameter.id);
            }
            for block in &machine.blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let Some(result) = operation.result.scalar_ref() else {
                        continue;
                    };
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(
                            |_| "handle operation ordinal exceeds its canonical position",
                        )?,
                    );
                    defined.push(result.id);
                    if producers
                        .insert(
                            (identity, result.id),
                            ValueProducer {
                                position,
                                operation,
                            },
                        )
                        .is_some()
                    {
                        return Err(format!(
                            "handle value {identity:?}/{:?} repeats a producer",
                            result.id
                        ));
                    }
                }
            }
            defined.sort_unstable();
            if defined.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(format!("handle value {identity:?} repeats a scalar binder"));
            }
            type_rows.push((identity, Some(TerminalScalarExpressions::new(machine))));
        }
        let types = IdTable::from_rows_with_gaps(type_rows, 65_536)
            .map_err(|reason| format!("handle scalar typing inventory is invalid: {reason:?}"))?;
        let captured_scalars = calls
            .machines()
            .map(|(identity, machine)| {
                TerminalScalarCaptures::new(module, machine)
                    .map(|captures| (identity, Some(captures)))
                    .map_err(|reason| {
                        format!("handle scalar captures are incomplete for {identity:?}: {reason}")
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let captured_scalars = IdTable::from_rows_with_gaps(captured_scalars, 65_536)
            .map_err(|reason| format!("handle scalar capture inventory is invalid: {reason:?}"))?;
        let resident_transfers = calls
            .machines()
            .map(|(identity, machine)| {
                let index = graph_index
                    .get(identity)
                    .copied()
                    .flatten()
                    .ok_or("handle resident capture has no storage graph coordinate")?;
                terminal_resident_value_transfers(machine, &graph[index])
                    .map(|transfers| (identity, Some(transfers)))
                    .map_err(|reason| {
                        format!(
                            "handle resident captures are incomplete for {identity:?}: {reason}"
                        )
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let resident_transfers =
            IdTable::from_rows_with_gaps(resident_transfers, 65_536).map_err(|reason| {
                format!("handle resident capture inventory is invalid: {reason:?}")
            })?;
        let histories = graph_index.map(|_, _| None);
        let mut native_writes = BTreeMap::new();
        for alternatives in calls.calls.values() {
            for call in alternatives {
                let CallTarget::Boundary(boundary) = call.target else {
                    continue;
                };
                let Some(reviewed) = reviewed_call(context, boundary)? else {
                    continue;
                };
                let parameters: &[usize] = match reviewed.contract {
                    WindowsHandleContract::DuplicateHandle => &[3],
                    WindowsHandleContract::QueryFileMode => &[2],
                    WindowsHandleContract::ReadFile | WindowsHandleContract::WriteFile => &[2],
                    _ => &[],
                };
                let RuntimeArguments::Explicit { structural, .. } = call.arguments else {
                    return Err("native output call has no explicit argument custody".into());
                };
                for &parameter in parameters {
                    if reviewed.declaration.parameter_order.get(parameter)
                        != Some(&BoundaryParameterKind::Structural)
                    {
                        return Err(format!(
                            "reviewed native output {boundary:?}/{parameter} changed parameter lane"
                        ));
                    }
                    let structural_index = reviewed.declaration.parameter_order[..parameter]
                        .iter()
                        .filter(|kind| **kind == BoundaryParameterKind::Structural)
                        .count();
                    let argument = structural.get(structural_index).ok_or_else(|| {
                        format!("native output {boundary:?}/{parameter} has no actual storage")
                    })?;
                    let machine = calls
                        .machine(call.caller.machine)
                        .ok_or("native output caller has no reached canonical body")?;
                    let root_type = machine
                        .structural_place_type(argument.place)
                        .ok_or("native output storage has no canonical carrier")?;
                    let location = descriptor_location(
                        &module.structural_types,
                        argument.place,
                        root_type,
                        &argument.path,
                        None,
                    )
                    .ok_or("native output storage has no exact supported projection")?;
                    let position = (
                        call.caller.block,
                        u32::try_from(call.caller.operation_index).map_err(
                            |_| "native output operation ordinal exceeds its canonical position",
                        )?,
                    );
                    native_writes
                        .entry((call.caller.machine, position))
                        .or_insert_with(Vec::new)
                        .push((
                            location,
                            StoredScalarSource::NativeOutput {
                                caller: call.caller,
                                parameter,
                            },
                        ));
                }
            }
        }
        Ok(Self {
            module,
            calls,
            graph,
            graph_index,
            types,
            producers,
            native_writes,
            captured_scalars,
            resident_transfers,
            histories,
            analysis: DescriptorAnalysis::with_borrowed_storage_slots(
                &module.structural_types,
                graph,
            ),
        })
    }

    pub(super) fn read(
        &mut self,
        identity: MachineId,
        value: ValueId,
    ) -> Result<ScalarDefinition, String> {
        let machine = self
            .calls
            .machine(identity)
            .ok_or_else(|| format!("handle scalar names unreached machine {identity:?}"))?;
        if let Some(index) = machine
            .parameters
            .iter()
            .position(|parameter| parameter.id == value)
        {
            return Ok(ScalarDefinition::Parameter(index));
        }
        let descriptor = &self.graph[*self
            .graph_index
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("handle scalar has no descriptor graph")?];
        if let Some(producer) = self.producers.get(&(identity, value)) {
            let operation = producer.operation;
            if let Some(expression) = self
                .types
                .get(identity)
                .and_then(Option::as_ref)
                .ok_or("handle scalar has no prepared type inventory")?
                .unexpanded_operation(operation)?
            {
                return Ok(ScalarDefinition::Expression(expression));
            }
            let position = producer.position;
            let caller = Caller {
                machine: identity,
                block: position.0,
                operation_index: position.1 as usize,
                site: CallSite::Invocation(semantic_vocabulary::InvocationSite::Operation(
                    operation.id,
                )),
            };
            if self.calls.calls.contains_key(&caller) {
                return Ok(ScalarDefinition::CallResult(caller));
            }
            let observed = descriptor.scalar_payload_reads.get(&position).and_then(|rows| {
                let mut matching = rows.iter().filter(|(_, term)| matches!(*term, ScalarTerm::Value { id, .. } if id == value));
                let (location, _) = matching.next()?;
                matching.next().is_none().then_some(location)
            }).ok_or_else(|| format!("handle value {identity:?}/{value:?} has no supported exact scalar origin at {position:?}"))?;
            return self.read_storage(identity, position, observed);
        }
        let mut arrivals = Vec::new();
        for (position, edges) in &self.scalar_captures(identity)?.arrivals {
            for (successor_ordinal, bindings) in edges.iter().enumerate() {
                for (parameter, term) in bindings {
                    if *parameter == value {
                        arrivals.push(ScalarArrival {
                            predecessor: *position,
                            successor_ordinal,
                            value: term.clone(),
                        });
                    }
                }
            }
        }
        if arrivals.is_empty() {
            Err(format!(
                "handle value {identity:?}/{value:?} is neither an entry parameter, producer nor bound arrival"
            ))
        } else {
            Ok(ScalarDefinition::Arrivals(arrivals))
        }
    }

    /// Bind one signature scalar to the original invocation operand. A caller
    /// must retain the invocation context when it reads this value; substituting
    /// an equal value from another call would change the handle occurrence.
    pub(super) fn scalar_actual(
        &self,
        call: &super::windows_handle_calls::RuntimeCall<'_>,
        callee: MachineId,
        parameter: usize,
    ) -> Result<ValueId, String> {
        self.require_machine_call(call, callee)?;
        let RuntimeArguments::Explicit { scalar, .. } = call.arguments else {
            return Err("scalar formal has no explicit invocation operand".into());
        };
        scalar.get(parameter).copied().ok_or_else(|| {
            format!(
                "handle caller {:?} omits scalar formal {parameter} of {callee:?}",
                call.caller
            )
        })
    }

    /// Rebase an immutable captured formal projection onto this call's actual
    /// storage. The version query still runs at the caller's exact pre-call cut;
    /// a callee formal is never interpreted as a caller-local allocation.
    pub(super) fn storage_actual(
        &self,
        call: &super::windows_handle_calls::RuntimeCall<'_>,
        callee: MachineId,
        entry: &PayloadEntryScalarIdentity,
    ) -> Result<DescriptorLocation, String> {
        self.storage_actual_location(
            call,
            callee,
            &DescriptorLocation {
                root: entry.root,
                path: entry.path.clone(),
            },
        )
    }

    pub(super) fn storage_actual_location(
        &self,
        call: &super::windows_handle_calls::RuntimeCall<'_>,
        callee: MachineId,
        entry: &DescriptorLocation,
    ) -> Result<DescriptorLocation, String> {
        self.require_machine_call(call, callee)?;
        let machine = self
            .calls
            .machine(callee)
            .ok_or("handle callee has no reached canonical body")?;
        let parameter = machine
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == entry.root)
            .ok_or("captured handle projection is not rooted at a signature formal")?;
        let (place, path) = match call.arguments {
            RuntimeArguments::Explicit { structural, .. } => {
                let actual = structural.get(parameter).ok_or(
                    "captured handle projection has no matching actual structural argument",
                )?;
                (actual.place, actual.path.as_slice())
            }
            RuntimeArguments::Cleanup(cleanup) => {
                let formal = &machine.structural_parameters[parameter];
                if !formal.is_cleanup_receiver(cleanup) {
                    return Err(
                        "captured handle projection changes the nominal cleanup receiver".into(),
                    );
                }
                (cleanup.place, cleanup.path.as_slice())
            }
        };
        let caller = self
            .calls
            .machine(call.caller.machine)
            .ok_or("handle caller has no reached canonical body")?;
        let root_type = caller
            .structural_place_type(place)
            .ok_or("captured handle actual has no declared storage carrier")?;
        let mut location =
            descriptor_location(&self.module.structural_types, place, root_type, path, None)
                .ok_or("captured handle actual has no exact supported storage projection")?;
        location.path.extend_from_slice(&entry.path);
        Ok(location)
    }

    fn require_machine_call(
        &self,
        call: &super::windows_handle_calls::RuntimeCall<'_>,
        callee: MachineId,
    ) -> Result<(), String> {
        if !matches!(call.target, CallTarget::Machine(actual) if actual == callee)
            || self.calls.machine(callee).is_none()
            || !self.calls.calls.get(&call.caller).is_some_and(|originals| {
                originals.iter().any(|original| {
                    matches!(original.target, CallTarget::Machine(actual) if actual == callee)
                        && original.boundary == call.boundary
                        && original.normal_return == call.normal_return
                        && match (original.arguments, call.arguments) {
                            (
                                RuntimeArguments::Explicit {
                                    scalar: left,
                                    structural: left_storage,
                                },
                                RuntimeArguments::Explicit {
                                    scalar: right,
                                    structural: right_storage,
                                },
                            ) => {
                                std::ptr::eq(left, right)
                                    && std::ptr::eq(left_storage, right_storage)
                            }
                            (RuntimeArguments::Cleanup(left), RuntimeArguments::Cleanup(right)) => {
                                std::ptr::eq(left, right)
                            }
                            _ => false,
                        }
                })
            })
        {
            return Err(format!(
                "handle substitution changes the actual callee at {:?}",
                call.caller
            ));
        }
        Ok(())
    }

    /// All real normal return edges, without choosing one by its desired
    /// contents. The replay owner retains their guards and ordered cleanup.
    pub(super) fn returns(
        &self,
        identity: MachineId,
        projection: Option<&[DescriptorPathSegment]>,
    ) -> Result<Vec<ScalarReturn>, String> {
        let machine = self
            .calls
            .machine(identity)
            .ok_or("handle return names an unreached machine")?;
        let mut returns = Vec::new();
        for block in &machine.blocks {
            let position = (
                block.id,
                u32::try_from(block.operations.len())
                    .map_err(|_| "handle return ordinal exceeds its canonical position")?,
            );
            let value = match (&block.terminator, projection) {
                (Terminator::Return { value, .. }, None) => ScalarReturnValue::Value(*value),
                (Terminator::ReturnStructural { source, .. }, Some(path)) => {
                    ScalarReturnValue::Storage(DescriptorLocation {
                        root: *source,
                        path: path.to_vec(),
                    })
                }
                (
                    Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::ReturnStructural { .. },
                    _,
                ) => {
                    return Err(format!(
                        "handle return at {identity:?}/{position:?} changes its runtime result lane"
                    ));
                }
                _ => continue,
            };
            returns.push(ScalarReturn { position, value });
        }
        if returns.is_empty() {
            return Err(format!(
                "handle scalar result of {identity:?} has no actual normal return"
            ));
        }
        Ok(returns)
    }

    /// Observe an actual projection at a call or return cut. This never creates
    /// an executable read, SSA binder or access permission for that projection.
    pub(super) fn read_storage(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        observed: &DescriptorLocation,
    ) -> Result<ScalarDefinition, String> {
        let machine = self
            .calls
            .machine(identity)
            .ok_or_else(|| format!("handle storage names unreached machine {identity:?}"))?;
        let descriptor = &self.graph[*self
            .graph_index
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("handle storage has no descriptor graph")?];
        for (ordinal, segment) in observed.path.iter().enumerate() {
            let DescriptorPathSegment::Case(case) = segment else {
                continue;
            };
            let predicate = DescriptorCasePredicate {
                subject: DescriptorLocation {
                    root: observed.root,
                    path: observed.path[..ordinal].to_vec(),
                },
                case: *case,
            };
            if self
                .analysis
                .case_feasibility(identity, position, &predicate)
                == FlowFeasibility::ProvedImpossible
            {
                return Ok(ScalarDefinition::InactiveCase(predicate));
            }
        }
        let history = self
            .histories
            .get_mut(identity)
            .ok_or("handle storage history has no prepared machine coordinate")?
            .get_or_insert_with(|| {
                PayloadMembershipHistory::new(
                    &self.module.structural_types,
                    &self.module.structural_domains,
                    self.graph,
                    machine,
                )
            });
        let (entry_snapshot, snapshot_detail) = match history.scalar_snapshot_in(
            &mut self.analysis,
            position,
            observed.root,
            &observed.path,
        ) {
            Ok(snapshot) => (
                snapshot.entry,
                "current storage has no captured entry identity",
            ),
            Err(reason) => (None, reason),
        };
        let writers = &self.native_writes;
        let calls = &self.calls.calls;
        let captures = self
            .captured_scalars
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("handle storage has no canonical scalar captures")?;
        let transfers = self
            .resident_transfers
            .get(identity)
            .and_then(Option::as_ref)
            .ok_or("handle storage has no canonical resident captures")?;
        let sources = projected_stored_sources_in(
            &self.module.structural_types,
            self.graph,
            &mut self.analysis,
            identity,
            position,
            observed,
            |write_position| {
                let mut written = match captures.writes.get(&write_position) {
                    Some(Ok(rows)) => rows
                        .iter()
                        .map(|(location, actual)| {
                            (
                                location.clone(),
                                StoredScalarSource::Scalar {
                                    position: write_position,
                                    value: actual.clone(),
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                    // This exact writer has no producing value to offer.
                    // The shared version owner still checks its original
                    // invalidation footprint against the observed home; an
                    // unrelated or later unresolved writer is not global taint.
                    Some(Err(_)) => Vec::new(),
                    None => Vec::new(),
                };
                if let Some(ResidentValueCapture::Complete(transfers)) =
                    transfers.get(&write_position)
                {
                    written.extend(transfers.iter().map(|transfer| {
                        (
                            transfer.destination.clone(),
                            StoredScalarSource::Transport {
                                position: write_position,
                                source: transfer.source.clone(),
                            },
                        )
                    }));
                }
                written.extend(
                    writers
                        .get(&(identity, write_position))
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
                if let Some(root) = descriptor.payload_producers.get(&write_position)
                    && *root == observed.root
                    && let Some(operation) = machine
                        .block(&write_position.0)
                        .and_then(|block| block.operations.get(write_position.1 as usize))
                {
                    let caller = Caller {
                        machine: identity,
                        block: write_position.0,
                        operation_index: write_position.1 as usize,
                        site: CallSite::Invocation(semantic_vocabulary::InvocationSite::Operation(
                            operation.id,
                        )),
                    };
                    if calls.contains_key(&caller) {
                        written.push((
                            observed.clone(),
                            StoredScalarSource::CallResult {
                                caller,
                                path: observed.path.clone(),
                            },
                        ));
                    }
                }
                written
            },
            |source, suffix| match source {
                StoredScalarSource::Transport {
                    position,
                    mut source,
                } => {
                    source.path.extend_from_slice(suffix);
                    Some(StoredScalarSource::Transport { position, source })
                }
                StoredScalarSource::CallResult { caller, mut path } => {
                    path.extend_from_slice(suffix);
                    Some(StoredScalarSource::CallResult { caller, path })
                }
                source => suffix.is_empty().then_some(source),
            },
        );
        match (sources, entry_snapshot) {
            (Some(sources), _) => Ok(ScalarDefinition::Stored(sources)),
            (None, Some(entry)) => Ok(ScalarDefinition::EntryStorage(entry)),
            (None, None) => {
                let query = DescriptorContentsQuery::from_location(observed);
                let root_type = machine
                    .structural_place_type(observed.root)
                    .ok_or("borrowed handle observation has no declared carrier")?;
                query
                    .validate(&self.module.structural_types, root_type)
                    .ok_or("borrowed handle observation has no valid typed projection")?;
                let targets = self.analysis.payload_targets(identity, position, &query)
                    .ok_or_else(|| format!("handle storage {identity:?}/{observed:?} has no current backing at {position:?}: {snapshot_detail}"))?;
                let [target @ DescriptorPayloadTarget::Incoming(entry)] = targets.as_slice() else {
                    return Err(format!(
                        "handle storage {identity:?}/{observed:?} has no unchanged producing event or unique captured entry referent at {position:?}: {snapshot_detail}"
                    ));
                };
                require_unchanged_entry_storage(&self.module.structural_types, self.graph,
                    &mut self.analysis, identity, position, target)
                    .map_err(|reason| format!("borrowed handle contents changed before {identity:?}/{position:?}: {reason}"))?;
                Ok(ScalarDefinition::BorrowedEntry(
                    entry.exact_location().ok_or(
                        "borrowed handle entry denotes an unresolved family of storage projections",
                    )?,
                ))
            }
        }
    }
}
