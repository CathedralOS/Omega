//! Read scalar operands from their current canonical value or storage version.
//!
//! Native output slots have analysis-owned identities, not invented SSA values.
//! A native output still owes its API success guard; a returned projection still
//! owes substitution of the actual checked callee and all its returning paths.
//! This reader identifies those obligations without declaring them proved.

use std::collections::BTreeMap;

use semantic_vocabulary::{MachineId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{BoundaryParameterKind, Operation, OperationKind, TerminalModule, Terminator};
use terminal_semantics::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition, descriptor_location,
};
use terminal_semantics::descriptors::descriptor_results::{
    DescriptorAnalysis, DescriptorCasePredicate, DescriptorContentsQuery, DescriptorMachine,
    DescriptorPayloadTarget, FlowFeasibility, PayloadEntryScalarIdentity, PayloadMembershipHistory,
    require_unchanged_entry_storage, unchanged_stored_projection_sources_in,
};

use super::context::ReviewContext;
use super::windows_handle_calls::{
    CallSite, CallTarget, Caller, HandleCallGraph, RuntimeArguments,
};
use super::windows_handle_contracts::{WindowsHandleContract, reviewed_call};

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
    pub value: ScalarTerm,
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
    graph_index: BTreeMap<MachineId, usize>,
    types: BTreeMap<MachineId, BTreeMap<ValueId, ScalarType>>,
    producers: BTreeMap<(MachineId, ValueId), ValueProducer<'a>>,
    native_writes:
        BTreeMap<(MachineId, DescriptorPosition), Vec<(DescriptorLocation, StoredScalarSource)>>,
    histories: BTreeMap<MachineId, PayloadMembershipHistory<'a>>,
    analysis: DescriptorAnalysis<'a>,
}

impl<'a> HandleValueReader<'a> {
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
                    matches!(source,
                StoredScalarSource::NativeOutput { caller: actual, parameter: slot }
                    if *actual == caller && *slot == parameter)
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
            .machines
            .get(&identity)
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
        let mut graph_index = BTreeMap::new();
        for (index, machine) in graph.iter().enumerate() {
            if graph_index.insert(machine.id, index).is_some() {
                return Err(format!(
                    "handle storage graph repeats machine {:?}",
                    machine.id
                ));
            }
        }
        let mut types = BTreeMap::new();
        let mut producers = BTreeMap::new();
        for (&identity, machine) in &calls.machines {
            if !graph_index.contains_key(&identity) {
                return Err(format!(
                    "handle storage graph omits executable machine {identity:?}"
                ));
            }
            let mut value_types = BTreeMap::new();
            for parameter in machine.parameters.iter().chain(machine.block_parameters()) {
                if value_types
                    .insert(parameter.id, parameter.scalar_type)
                    .is_some()
                {
                    return Err(format!(
                        "handle value {identity:?}/{:?} repeats a binder",
                        parameter.id
                    ));
                }
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
                    if value_types.insert(result.id, result.scalar_type).is_some()
                        || producers
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
            types.insert(identity, value_types);
        }
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
                    let machine = calls.machines[&call.caller.machine];
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
            histories: BTreeMap::new(),
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
            .machines
            .get(&identity)
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
            .get(&identity)
            .ok_or("handle scalar has no descriptor graph")?];
        if let Some(producer) = self.producers.get(&(identity, value)) {
            let operation = producer.operation;
            let literal = match operation.kind {
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err("canonical integer literal has a noninteger result".into());
                    };
                    Some(ScalarTerm::Integer { scalar_type, value })
                }
                OperationKind::BooleanConstant { value } => Some(ScalarTerm::Boolean(value)),
                _ => None,
            };
            if let Some(literal) = literal {
                return Ok(ScalarDefinition::Expression(literal));
            }
            let value_types = &self.types[&identity];
            let equation =
                terminal_semantics::goal_free_scalar_leaf_semantics(operation, value_types)
                    .map_err(|error| {
                        format!(
                            "handle scalar operation has invalid canonical semantics: {error:?}"
                        )
                    })?
                    .map(|semantics| semantics.result_equation().clone())
                    .or(terminal_semantics::proof_bearing_scalar_leaf_semantics(
                        operation,
                        value_types,
                    )
                    .map_err(|error| {
                        format!("handle scalar operation has invalid checked semantics: {error:?}")
                    })?
                    .map(|semantics| semantics.result_equation().clone()));
            if let Some(semantic_vocabulary::Proposition::Equal(_, right)) = equation {
                return Ok(ScalarDefinition::Expression(right));
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
                let mut matching = rows.iter().filter(|(_, term)| matches!(term, ScalarTerm::Value { id, .. } if *id == value));
                let (location, _) = matching.next()?;
                matching.next().is_none().then_some(location)
            }).ok_or_else(|| format!("handle value {identity:?}/{value:?} has no supported exact scalar origin at {position:?}"))?;
            return self.read_storage(identity, position, observed);
        }
        let mut arrivals = Vec::new();
        for (position, edges) in &descriptor.payload_scalar_bindings {
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
        let machine = self.calls.machines[&callee];
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
                if cleanup.cleanup_receiver != Some(formal.place)
                    || !formal.is_self
                    || formal.structural_type != cleanup.structural_type
                {
                    return Err(
                        "captured handle projection changes the nominal cleanup receiver".into(),
                    );
                }
                (cleanup.place, cleanup.path.as_slice())
            }
        };
        let caller = self.calls.machines[&call.caller.machine];
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
            || !self.calls.machines.contains_key(&callee)
            || !self.calls.calls.get(&call.caller).is_some_and(|originals| {
                originals.iter().any(|original| {
                    matches!(original.target, CallTarget::Machine(actual) if actual == callee)
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
            .machines
            .get(&identity)
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
            .machines
            .get(&identity)
            .ok_or_else(|| format!("handle storage names unreached machine {identity:?}"))?;
        let descriptor = &self.graph[*self
            .graph_index
            .get(&identity)
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
        let history = self.histories.entry(identity).or_insert_with(|| {
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
        let sources = unchanged_stored_projection_sources_in(
            &self.module.structural_types,
            self.graph,
            &mut self.analysis,
            identity,
            position,
            observed,
            |write_position| {
                let producer = machine
                    .block(&write_position.0)
                    .and_then(|block| block.operations.get(write_position.1 as usize));
                // Completed-call equations can establish equal numeric data,
                // but cannot authenticate a handle's producing event. Only
                // actual constructors and stores are plain writers here;
                // calls have the separate occurrence identities below.
                let actual_store = producer.is_some_and(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::EstablishPrimitiveLocal { .. }
                            | OperationKind::EstablishScalarArray { .. }
                            | OperationKind::EstablishRecord { .. }
                            | OperationKind::EstablishScalarCase { .. }
                            | OperationKind::EstablishStructuralCase { .. }
                            | OperationKind::WriteOnlyPrimitiveStore { .. }
                            | OperationKind::StructuralScalarFieldStore { .. }
                    )
                });
                let mut written = if actual_store {
                    descriptor
                        .scalar_payload_writes
                        .get(&write_position)
                        .into_iter()
                        .flatten()
                        .map(|(location, term)| {
                            (
                                location.clone(),
                                StoredScalarSource::Scalar {
                                    position: write_position,
                                    value: term.clone(),
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
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
