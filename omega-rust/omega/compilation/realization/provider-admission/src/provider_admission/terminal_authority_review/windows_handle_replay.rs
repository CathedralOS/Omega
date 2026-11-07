//! Track each call’s values and handle aliases through its matching return.
//!
//! Source occurrence identity is prepared from the retained artifact and the
//! original selected calls. It never hashes a proof to invent another source
//! occurrence. The resource ledger and scalar snapshots then belong to those
//! same activations; a callee's return restores its actual caller, rather than
//! selecting another incoming call with a convenient argument.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    EdgeId, IdTable, IntegerType, IntegerValue, InvocationSite, MachineId, ScalarTerm, ScalarType,
    StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_psi::{Operation, OperationKind};
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
use terminal_semantics::descriptor_results::{DescriptorMachine, ResidentValueCapture};

use crate::digest_encoder::DomainDigestEncoder;

use super::context::ReviewContext;
use super::windows_handle_aliases::{
    CapturedHandleRegion, HandleAliases, HandleStorageBacking, HandleStorageOwner,
    HandleStorageRange, HandleWordAliases,
};
use super::windows_handle_calls::{CallSite, CallTarget, Caller, HandleCallGraph, RuntimeCall};
use super::windows_handle_inputs::{PreparedHandleMachine, WindowsHandleInputs};
use super::windows_handle_lifetimes::{
    HandleLifetimes, NativeOccurrence, ScalarOccurrence, ScalarUseSite,
};
use super::windows_handle_storage::{
    CapturedHandleReferences, CapturedHandleSequence, CapturedHandleStorage, HandlePointerKind,
    HandleReferences, HandleScalarStorage, HandleStorageBindings, HandleStorageGeometry,
};
use super::windows_handle_values::LocalScalarExpressions;

struct ReplayActivation {
    machine: MachineId,
    source: DomainDigestEncoder,
}

/// These are analysis coordinates, not manufactured MachineId/PlaceId rows.
/// Re-entering the same source call reuses its universally checked scope; fresh
/// resource instances and retained caller aliases remain separate ledger data.
pub(super) struct HandleReplayActivations {
    rows: Vec<ReplayActivation>,
    children: BTreeMap<(usize, Caller, MachineId), usize>,
}

impl HandleReplayActivations {
    pub(super) fn new(
        artifact: [u8; 32],
        context: &ReviewContext<'_>,
        calls: &HandleCallGraph<'_>,
    ) -> Result<Self, String> {
        if artifact == [0; 32] || calls.entry != context.input.plan().entry {
            return Err("handle replay has no matching retained artifact and entry".into());
        }
        let mut source = DomainDigestEncoder::new(b"omega.windows.handle.source-context.v1\0");
        source.raw(&artifact);
        source.bytes(context.selected.identity_digest().as_bytes());
        source.u64(calls.entry.get());
        Ok(Self {
            rows: vec![ReplayActivation {
                machine: calls.entry,
                source,
            }],
            children: BTreeMap::new(),
        })
    }

    pub(super) fn enter<'a>(
        &mut self,
        parent: usize,
        caller: Caller,
        callee: MachineId,
        calls: &HandleCallGraph<'a>,
    ) -> Result<(usize, RuntimeCall<'a>), String> {
        let parent_scope = self
            .rows
            .get(parent)
            .ok_or("handle call has no actual caller activation")?;
        if parent_scope.machine != caller.machine {
            return Err("handle call substitutes another containing activation".into());
        }
        let mut candidates =
            calls.calls.get(&caller).into_iter().flatten().filter(
                |call| matches!(call.target, CallTarget::Machine(actual) if actual == callee),
            );
        let call = *candidates
            .next()
            .ok_or("handle activation has no original selected machine call")?;
        if candidates.next().is_some() || calls.machine(callee).is_none() {
            return Err("handle activation has an ambiguous or absent selected body".into());
        }
        let key = (parent, caller, callee);
        if let Some(&index) = self.children.get(&key) {
            return Ok((index, call));
        }
        if self.rows.len() >= 65_536 {
            return Err("handle replay activation contexts exceed their bounded inventory".into());
        }
        let mut source = parent_scope.source.clone();
        encode_caller(&mut source, caller);
        source.u64(callee.get());
        match call.boundary {
            Some(boundary) => {
                source.byte(1);
                source.u64(boundary.get());
            }
            None => source.byte(0),
        }
        let index = self.rows.len();
        self.rows.push(ReplayActivation {
            machine: callee,
            source,
        });
        self.children.insert(key, index);
        Ok((index, call))
    }

    pub(super) fn native(
        &self,
        activation: usize,
        caller: Caller,
        context: &ReviewContext<'_>,
        calls: &HandleCallGraph<'_>,
    ) -> Result<NativeOccurrence, String> {
        let scope = self
            .rows
            .get(activation)
            .ok_or("native handle call has no actual activation")?;
        if scope.machine != caller.machine {
            return Err("native handle call substitutes another containing activation".into());
        }
        let mut candidates = calls
            .calls
            .get(&caller)
            .into_iter()
            .flatten()
            .filter(|call| matches!(call.target, CallTarget::Boundary(_)));
        let call = candidates
            .next()
            .ok_or("native handle occurrence has no original boundary execution")?;
        if candidates.next().is_some() {
            return Err("native handle occurrence has an ambiguous boundary execution".into());
        }
        let CallTarget::Boundary(boundary) = call.target else {
            return Err("native handle occurrence is not its original boundary target".into());
        };
        let CallSite::Invocation(site @ InvocationSite::Operation(_)) = caller.site else {
            return Err(
                "native handle transition has no original result-producing operation".into(),
            );
        };
        let mechanism = context
            .mechanism(boundary)
            .ok_or("native handle occurrence lost its admitted physical mechanism")?;
        let effects::TerminalMechanismIdentity::NormalizedForeign(foreign) = *mechanism else {
            return Err(
                "Windows handle source occurrence is not an original foreign import".into(),
            );
        };
        if foreign.target() != target::TargetProfile::WindowsX64
            || foreign.argument_contract()
                != effects::NormalizedForeignArgumentContract::AdmittedPlan
            || foreign.implementation_contract().is_zero()
        {
            return Err(
                "Windows handle source occurrence contains a narrowed or incomplete foreign key"
                    .into(),
            );
        }
        let binding = context
            .foreign_binding(boundary)
            .ok_or("Windows handle source occurrence has no original foreign settlement")?;
        let target_operations::NormalizedForeignCallMechanism::Import { locator, .. } =
            &binding.mechanism
        else {
            return Err("Windows handle source occurrence is not its settled import".into());
        };
        if locator.target() != target::TargetProfile::WindowsX64
            || locator.identity_digest() != foreign.locator_identity()
        {
            return Err(
                "Windows handle source occurrence changes its original import locator".into(),
            );
        }
        let mut source = scope.source.clone();
        encode_caller(&mut source, caller);
        source.u64(boundary.get());
        // The original admitted key has no checked-argument proof. The source
        // context therefore cannot change when a proposed proof changes.
        source.bytes(&effects::terminal_mechanism_identity_bytes(*mechanism));
        Ok(NativeOccurrence {
            machine: scope.machine,
            site,
            context: source.finish(),
            boundary,
        })
    }

    pub(super) fn scalar(
        &self,
        activation: usize,
        site: ScalarUseSite,
    ) -> Result<ScalarOccurrence, String> {
        let scope = self
            .rows
            .get(activation)
            .ok_or("handle observation has no actual activation")?;
        Ok(ScalarOccurrence {
            machine: scope.machine,
            site,
            context: scope.source.clone().finish(),
        })
    }
}

fn encode_caller(encoder: &mut DomainDigestEncoder, caller: Caller) {
    encoder.u64(caller.machine.get());
    encoder.u64(caller.block.get());
    encoder.len(caller.operation_index);
    match caller.site {
        CallSite::Invocation(site) => {
            encoder.byte(0);
            let mut identity = Vec::with_capacity(9);
            site.encode_identity(&mut identity);
            encoder.bytes(&identity);
        }
        CallSite::Cleanup { edge, ordinal } => {
            encoder.byte(1);
            encoder.u64(edge.get());
            encoder.len(ordinal);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum KnownHandleScalar {
    Unknown,
    Integer(IntegerType, IntegerValue),
    Boolean(bool),
    Nonzero(IntegerType),
}

impl KnownHandleScalar {
    fn unsigned(&self) -> Option<u64> {
        match self {
            Self::Integer(_, IntegerValue::Unsigned(value)) => u64::try_from(*value).ok(),
            Self::Integer(_, IntegerValue::Signed(value)) => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    fn literal(&self) -> Option<ScalarTerm> {
        match *self {
            Self::Integer(scalar_type, value) => Some(ScalarTerm::Integer { scalar_type, value }),
            Self::Boolean(value) => Some(ScalarTerm::Boolean(value)),
            Self::Unknown | Self::Nonzero(_) => None,
        }
    }

    fn has_type(&self, expected: ScalarType) -> bool {
        match *self {
            Self::Unknown => true,
            Self::Integer(actual, value) => {
                expected == ScalarType::Integer(actual) && actual.admits(value)
            }
            Self::Nonzero(actual) => expected == ScalarType::Integer(actual),
            Self::Boolean(_) => expected == ScalarType::Boolean,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CapturedHandleScalar {
    word: HandleWordAliases,
    known: KnownHandleScalar,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CapturedCaseTag {
    carrier: StructuralTypeId,
    case: StructuralCaseId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CapturedMemory {
    words: CapturedHandleRegion,
    references: CapturedHandleReferences,
    cases: Vec<(
        super::windows_handle_aliases::RelativeHandleRange,
        CapturedCaseTag,
    )>,
    constants: Vec<(
        super::windows_handle_aliases::RelativeHandleRange,
        KnownHandleScalar,
    )>,
}

enum CapturedStore {
    Scalar(HandleScalarStorage, CapturedHandleScalar),
    Bytes(HandleStorageRange, CapturedMemory),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalHandleState {
    ledger: HandleLifetimes,
    aliases: HandleAliases,
    references: HandleReferences,
    cases: BTreeMap<HandleStorageRange, CapturedCaseTag>,
    constants: IdTable<ValueId, Option<KnownHandleScalar>>,
    stored_constants: BTreeMap<HandleStorageRange, KnownHandleScalar>,
    bindings: HandleStorageBindings,
}

/// These references are all prepared from the same reached machine. No
/// operation can substitute another graph's convenient storage or equations.
struct HandleMachineReplay<'a> {
    body: &'a terminal_psi::TerminalMachine,
    prepared: &'a PreparedHandleMachine,
    graph: &'a DescriptorMachine,
    calls: &'a HandleCallGraph<'a>,
    geometry: &'a HandleStorageGeometry<'a>,
}

struct HandleReplayInvocation {
    parent: usize,
    caller: Caller,
    callee: MachineId,
}

/// The suspended caller is retained in full, including retired integer
/// aliases. Returning from another sibling invocation cannot supply its state.
struct HandleReplayFrame {
    invocation: HandleReplayInvocation,
    activation: usize,
    caller: CanonicalHandleState,
    mutable_outputs: Vec<HandleCensusRoot>,
    normal_return: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HandleCensusRoot {
    storage: CapturedHandleStorage,
    carrier: StructuralTypeId,
}

enum CapturedReturnValue {
    Unit,
    Scalar(CapturedHandleScalar),
    Structural {
        carrier: StructuralTypeId,
        value: CapturedCallArgument,
    },
}

struct CapturedHandleReturn {
    activation: usize,
    machine: MachineId,
    edge: EdgeId,
    value: CapturedReturnValue,
}

impl CanonicalHandleState {
    fn capture_return(
        &mut self,
        activation: usize,
        position: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        inputs: &WindowsHandleInputs<'_>,
        activations: &HandleReplayActivations,
        context: &ReviewContext<'_>,
    ) -> Result<CapturedHandleReturn, String> {
        let scope = activations
            .rows
            .get(activation)
            .ok_or("handle return has no actual activation")?;
        let body = inputs
            .calls
            .machine(scope.machine)
            .ok_or("handle return has no original body")?;
        let block = body
            .block(&position.0)
            .filter(|block| block.operations.len() == position.1 as usize)
            .ok_or("handle return is not its actual terminating cut")?;
        let prepared = inputs
            .machines
            .get(body.id)
            .and_then(Option::as_ref)
            .ok_or("handle return has no matching prepared scalar roster")?;
        let bindings = self.bindings.clone();
        let geometry = HandleStorageGeometry::new(
            context.input.module(),
            body,
            &inputs.storage[prepared.storage_index],
            target::TargetProfile::WindowsX64,
            activation,
            &bindings,
        )?;
        let (edge, value) = match &block.terminator {
            terminal_psi::Terminator::Return { edge, value, .. } => {
                let captured = self.scalar(*value, position, &prepared.expressions)?;
                if body
                    .result
                    .scalar_ref()
                    .is_none_or(|result| result.scalar_type != captured.word.scalar_type())
                {
                    return Err("handle scalar return changes the body's actual result lane".into());
                }
                self.observe(
                    captured.word.dependencies().iter().copied(),
                    activations.scalar(activation, ScalarUseSite::Edge(*edge))?,
                )?;
                (*edge, CapturedReturnValue::Scalar(captured))
            }
            terminal_psi::Terminator::ReturnUnit { edge, .. } => {
                if body.result != terminal_psi::TerminalMachineResult::Unit {
                    return Err("handle Unit return changes the body's actual result lane".into());
                }
                (*edge, CapturedReturnValue::Unit)
            }
            terminal_psi::Terminator::ReturnStructural { edge, source, .. } => {
                let declaration = body
                    .result
                    .structural()
                    .ok_or("handle structural return changes its actual result lane")?;
                let location = DescriptorLocation::whole(*source);
                let (range, selected, _) = geometry.resolve(&location, &self.references)?;
                if geometry.carrier(&location)? != Some(declaration.structural_type) {
                    return Err(
                        "handle structural return changes its original declared carrier".into(),
                    );
                }
                self.observe(
                    self.aliases.stored_dependencies(&range),
                    activations.scalar(activation, ScalarUseSite::Edge(*edge))?,
                )?;
                let value = match selected.scalar {
                    Some(scalar) => CapturedCallArgument::Scalar(self.read_scalar(
                        &location,
                        scalar.logical_scalar,
                        &geometry,
                    )?),
                    None => CapturedCallArgument::Bytes(self.capture(&range)),
                };
                (
                    *edge,
                    CapturedReturnValue::Structural {
                        carrier: declaration.structural_type,
                        value,
                    },
                )
            }
            _ => return Err("handle normal return substitutes another original terminator".into()),
        };
        Ok(CapturedHandleReturn {
            activation,
            machine: body.id,
            edge,
            value,
        })
    }

    /// Only the execution owner calls this after the captured return's entire
    /// ordered cleanup roster has completed. Install the saved result in its
    /// actual caller before the output census, then retire callee-local bytes.
    fn resume_after_cleanup(
        mut self,
        frame: HandleReplayFrame,
        returned: CapturedHandleReturn,
        inputs: &WindowsHandleInputs<'_>,
        activations: &HandleReplayActivations,
        context: &ReviewContext<'_>,
    ) -> Result<Self, String> {
        if !frame.normal_return
            || returned.activation != frame.activation
            || returned.machine != frame.invocation.callee
        {
            return Err("handle return resumes a sibling invocation or another activation".into());
        }
        let caller = inputs
            .calls
            .machine(frame.invocation.caller.machine)
            .ok_or("handle continuation lost its exact suspended caller")?;
        let prepared = inputs
            .machines
            .get(caller.id)
            .and_then(Option::as_ref)
            .ok_or("handle continuation lost its prepared original caller")?;
        self.aliases = self.aliases.resume_caller(&frame.caller.aliases);
        self.constants = frame.caller.constants;
        self.bindings = frame.caller.bindings;
        let bindings = self.bindings.clone();
        let geometry = HandleStorageGeometry::new(
            context.input.module(),
            caller,
            &inputs.storage[prepared.storage_index],
            target::TargetProfile::WindowsX64,
            frame.invocation.parent,
            &bindings,
        )?;
        let result = match frame.invocation.caller.site {
            CallSite::Invocation(InvocationSite::Operation(operation)) => {
                &caller
                    .block(&frame.invocation.caller.block)
                    .and_then(|block| {
                        block
                            .operations
                            .get(frame.invocation.caller.operation_index)
                    })
                    .filter(|actual| actual.id == operation)
                    .ok_or("handle continuation changes its original receiving operation")?
                    .result
            }
            CallSite::Cleanup { .. } => &terminal_psi::OperationResult::Unit,
            CallSite::Invocation(_) => {
                return Err("handle continuation has no normal receiving operation".into());
            }
        };
        let mut outputs = frame.mutable_outputs;
        let mut scalar_outputs = BTreeSet::new();
        match (result, returned.value) {
            (terminal_psi::OperationResult::Unit, CapturedReturnValue::Unit) => {}
            (terminal_psi::OperationResult::Scalar(result), CapturedReturnValue::Scalar(value)) => {
                scalar_outputs.extend(value.word.dependencies());
                self.define(result.id, value, &prepared.expressions)?;
            }
            (
                terminal_psi::OperationResult::Structural(result),
                CapturedReturnValue::Structural { carrier, value },
            ) => {
                if result.structural_type != carrier {
                    return Err(
                        "handle return changes its caller's structural result carrier".into(),
                    );
                }
                let location = DescriptorLocation::whole(result.place);
                let (range, _, codec) = geometry.resolve(&location, &self.references)?;
                match value {
                    CapturedCallArgument::Scalar(value) => {
                        self.store_scalar(&location, &value, &geometry)?
                    }
                    CapturedCallArgument::Bytes(value) => self.install(&range, &value)?,
                }
                outputs.push(HandleCensusRoot {
                    storage: CapturedHandleStorage {
                        range,
                        stored_scalar: codec,
                    },
                    carrier,
                });
            }
            _ => return Err("handle return changes its actual receiving result lane".into()),
        }
        scalar_outputs.extend(self.output_census(outputs, &geometry, frame.activation)?);
        self.ledger = self
            .ledger
            .caller_continuation(
                &frame.caller.ledger,
                activations.scalar(frame.activation, ScalarUseSite::Edge(returned.edge))?,
                &scalar_outputs.into_iter().collect::<Vec<_>>(),
            )
            .map_err(|reason| {
                format!("handle return violates its complete output lifetime: {reason:?}")
            })?;
        self.aliases.retire_activation(frame.activation);
        self.references.retire_activation(frame.activation);
        let owner = HandleStorageOwner::Activation(frame.activation);
        self.cases.retain(|range, _| range.backing().owner != owner);
        self.stored_constants
            .retain(|range, _| range.backing().owner != owner);
        Ok(self)
    }

    /// Follow every resident pointer in the actual outgoing typed values.
    /// Readonly/dead local aliases are not outputs. Pointer targets are read
    /// after cleanup, so replacing a mutable output or retiring its resource
    /// cannot be hidden behind the return value's earlier snapshot.
    fn output_census(
        &self,
        roots: impl IntoIterator<Item = HandleCensusRoot>,
        geometry: &HandleStorageGeometry<'_>,
        retiring: usize,
    ) -> Result<BTreeSet<super::windows_handle_lifetimes::HandleInstance>, String> {
        fn enqueue(
            root: HandleCensusRoot,
            discovered: &mut BTreeSet<HandleCensusRoot>,
            pending: &mut Vec<HandleCensusRoot>,
        ) -> Result<(), String> {
            if discovered.contains(&root) {
                return Ok(());
            }
            if discovered.len() >= 65_536 {
                return Err("handle output pointer census exceeds its bounded inventory".into());
            }
            discovered.insert(root.clone());
            pending.push(root);
            Ok(())
        }
        let mut pending = Vec::new();
        let mut discovered = BTreeSet::new();
        for root in roots {
            enqueue(root, &mut discovered, &mut pending)?;
        }
        let mut dependencies = BTreeSet::new();
        while let Some(root) = pending.pop() {
            dependencies.extend(self.aliases.stored_dependencies(&root.storage.range));
            if root.storage.stored_scalar.is_some() {
                if geometry.primitive_scalar(root.carrier).is_none() {
                    return Err("handle output scalar codec hides a non-scalar carrier".into());
                }
                continue;
            }
            let slots = geometry.pointer_slots(
                root.carrier,
                &root.storage.range,
                |range, carrier, case| {
                    self.cases
                        .get(&range)
                        .filter(|tag| tag.carrier == carrier)
                        .map(|tag| tag.case == case)
                        .ok_or(
                            "handle output pointer slot has no current active-case evidence".into(),
                        )
                },
            )?;
            for slot in slots {
                match slot.kind {
                    HandlePointerKind::Reference(referent) => {
                        let captured = self
                            .references
                            .get(&slot.range)
                            .ok_or("handle output reference has no captured pointee")?;
                        if captured.referent != referent {
                            return Err(
                                "handle output reference changes its original referent carrier"
                                    .into(),
                            );
                        }
                        if captured.range.backing().owner
                            == HandleStorageOwner::Activation(retiring)
                        {
                            return Err("handle output reference escapes its returning activation's storage".into());
                        }
                        dependencies.extend(&captured.dependencies);
                        enqueue(
                            HandleCensusRoot {
                                storage: CapturedHandleStorage {
                                    range: captured.range,
                                    stored_scalar: captured.stored_scalar,
                                },
                                carrier: referent,
                            },
                            &mut discovered,
                            &mut pending,
                        )?;
                    }
                    HandlePointerKind::Sequence(element) => {
                        let captured = self
                            .references
                            .sequence(&slot.range)
                            .ok_or("handle output descriptor has no captured data window")?;
                        if captured.element != element {
                            return Err(
                                "handle output descriptor changes its original element carrier"
                                    .into(),
                            );
                        }
                        if captured.range.backing().owner
                            == HandleStorageOwner::Activation(retiring)
                        {
                            return Err("handle output descriptor escapes its returning activation's storage".into());
                        }
                        dependencies.extend(&captured.dependencies);
                        dependencies.extend(self.aliases.stored_dependencies(&captured.range));
                        let Some(element) = element else {
                            continue;
                        };
                        if !geometry.has_pointer_slots(element, captured.range.backing().place)? {
                            continue;
                        }
                        let length = captured.length.filter(|_| captured.precise).ok_or(
                            "handle output view has no complete resident pointer selection",
                        )?;
                        let length = usize::try_from(length)
                            .ok()
                            .filter(|count| *count <= 65_536 - discovered.len())
                            .ok_or("handle output view exceeds its bounded pointer inventory")?;
                        for ordinal in 0..length {
                            let range = geometry.sequence_element(captured, ordinal as u64)?;
                            enqueue(
                                HandleCensusRoot {
                                    storage: CapturedHandleStorage {
                                        range,
                                        stored_scalar: None,
                                    },
                                    carrier: element,
                                },
                                &mut discovered,
                                &mut pending,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(dependencies)
    }

    fn enter_call(
        mut self,
        invocation: HandleReplayInvocation,
        inputs: &WindowsHandleInputs<'_>,
        activations: &mut HandleReplayActivations,
        context: &ReviewContext<'_>,
    ) -> Result<(HandleReplayFrame, Self), String> {
        let (activation, call) = activations.enter(
            invocation.parent,
            invocation.caller,
            invocation.callee,
            &inputs.calls,
        )?;
        let caller = inputs
            .calls
            .machine(invocation.caller.machine)
            .ok_or("handle call has no actual containing body")?;
        let callee = inputs
            .calls
            .machine(invocation.callee)
            .ok_or("handle call has no actual selected body")?;
        let caller_prepared = inputs
            .machines
            .get(caller.id)
            .and_then(Option::as_ref)
            .ok_or("handle caller has no prepared operand roster")?;
        let callee_prepared = inputs
            .machines
            .get(callee.id)
            .and_then(Option::as_ref)
            .ok_or("handle callee has no prepared operand roster")?;
        let position = (
            invocation.caller.block,
            u32::try_from(invocation.caller.operation_index)
                .map_err(|_| "handle call ordinal exceeds its canonical position")?,
        );
        let site = match invocation.caller.site {
            CallSite::Invocation(InvocationSite::Operation(operation)) => {
                ScalarUseSite::Operation(operation)
            }
            CallSite::Cleanup { edge, .. } => ScalarUseSite::Edge(edge),
            CallSite::Invocation(
                InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge),
            ) => ScalarUseSite::Edge(edge),
        };
        let occurrence = activations.scalar(invocation.parent, site)?;
        let scalar_actuals = match call.arguments {
            super::windows_handle_calls::RuntimeArguments::Explicit { scalar, .. } => scalar,
            super::windows_handle_calls::RuntimeArguments::Cleanup(_) => &[],
        };
        if scalar_actuals.len() != callee.parameters.len() {
            return Err("handle call changes the selected body's scalar signature".into());
        }
        let scalar_actuals = callee
            .parameters
            .iter()
            .zip(scalar_actuals)
            .map(|(formal, actual)| {
                let captured = self.scalar(*actual, position, &caller_prepared.expressions)?;
                if captured.word.scalar_type() != formal.scalar_type {
                    return Err("handle call changes an evaluated scalar's formal carrier".into());
                }
                Ok((formal.id, captured))
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.observe(
            scalar_actuals
                .iter()
                .flat_map(|(_, value)| value.word.dependencies().iter().copied()),
            occurrence,
        )?;
        let bindings = self.bindings.clone();
        let geometry = HandleStorageGeometry::new(
            context.input.module(),
            caller,
            &inputs.storage[caller_prepared.storage_index],
            target::TargetProfile::WindowsX64,
            invocation.parent,
            &bindings,
        )?;
        let mut next = self.enter(&callee_prepared.expressions, callee)?;
        let mut owned = Vec::new();
        let mut mutable_outputs = Vec::new();
        for (ordinal, formal) in callee.structural_parameters.iter().enumerate() {
            let actual = call.storage_actual(callee, ordinal)?;
            let source = geometry.location(actual.place, actual.path)?;
            let (range, selected, codec) = geometry.resolve(&source, &self.references)?;
            match selected.scalar {
                Some(scalar)
                    if geometry.primitive_scalar(formal.structural_type)
                        != Some(scalar.logical_scalar) =>
                {
                    return Err("handle scalar actual changes its formal logical carrier".into());
                }
                None if geometry.carrier(&source)? != Some(formal.structural_type) => {
                    return Err(
                        "handle structural actual changes its formal declared carrier".into(),
                    );
                }
                _ => {}
            }
            if actual.access == terminal_psi::StructuralAccess::Owned {
                let value = if let Some(scalar) = selected.scalar {
                    CapturedCallArgument::Scalar(self.read_scalar(
                        &source,
                        scalar.logical_scalar,
                        &geometry,
                    )?)
                } else {
                    if geometry.carrier(&source)? != Some(formal.structural_type) {
                        return Err(
                            "handle owned actual changes its complete declared carrier".into()
                        );
                    }
                    CapturedCallArgument::Bytes(self.capture(&range))
                };
                self.observe(self.aliases.stored_dependencies(&range), occurrence)?;
                owned.push((formal.place, value));
            } else {
                let captured = CapturedHandleStorage {
                    range,
                    stored_scalar: codec,
                };
                next.bindings
                    .bind(formal.place, actual.access, captured.clone())?;
                if actual.access != terminal_psi::StructuralAccess::SharedBorrow {
                    mutable_outputs.push(HandleCensusRoot {
                        storage: captured,
                        carrier: formal.structural_type,
                    });
                }
            }
        }
        // Source arguments are all captured before any formal acquires its
        // new value. Borrowed roots keep their actual caller-owned backing.
        let next_bindings = next.bindings.clone();
        let callee_geometry = HandleStorageGeometry::new(
            context.input.module(),
            callee,
            &inputs.storage[callee_prepared.storage_index],
            target::TargetProfile::WindowsX64,
            activation,
            &next_bindings,
        )?;
        for (place, value) in owned {
            let location = DescriptorLocation::whole(place);
            match value {
                CapturedCallArgument::Scalar(value) => {
                    next.store_scalar(&location, &value, &callee_geometry)?
                }
                CapturedCallArgument::Bytes(value) => {
                    let (destination, _, _) =
                        callee_geometry.resolve(&location, &next.references)?;
                    next.install(&destination, &value)?;
                }
            }
        }
        for (formal, value) in scalar_actuals {
            next.define(formal, value, &callee_prepared.expressions)?;
        }
        // Argument observations belong to the history inherited by the callee,
        // rather than disappearing in the caller snapshot retained above.
        next.ledger = self.ledger.clone();
        Ok((
            HandleReplayFrame {
                invocation,
                activation,
                caller: self,
                mutable_outputs,
                normal_return: call.normal_return,
            },
            next,
        ))
    }

    /// Execute one original non-call operation. Every source is captured at
    /// its pre-operation cut before any destination is cleared. Calls have a
    /// separate matched-return owner and cannot enter this ordinary route.
    fn operation(
        mut self,
        position: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        machine: &HandleMachineReplay<'_>,
        occurrence: ScalarOccurrence,
    ) -> Result<Self, String> {
        let HandleMachineReplay {
            body,
            prepared,
            graph,
            calls,
            geometry,
        } = *machine;
        let operation = body
            .block(&position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("handle replay operation is absent from its original body")?;
        if prepared.expressions.identity != body.id
            || graph.id != body.id
            || occurrence.machine != body.id
            || occurrence.site != ScalarUseSite::Operation(operation.id)
        {
            return Err("handle operation substitutes its body, cut or source occurrence".into());
        }
        let caller = Caller {
            machine: body.id,
            block: position.0,
            operation_index: position.1 as usize,
            site: CallSite::Invocation(InvocationSite::Operation(operation.id)),
        };
        if calls.calls.contains_key(&caller) {
            return Err("handle call requires its matched execution and return".into());
        }
        if matches!(
            operation.kind,
            OperationKind::StorageAddress { .. }
                | OperationKind::EstablishAccessReference { .. }
                | OperationKind::ActivateTaskRegister { .. }
                | OperationKind::ReadTaskRegister { .. }
                | OperationKind::ReloadSegmentDescriptors { .. }
                | OperationKind::ReadSegmentSelector { .. }
                | OperationKind::GlobalDescriptorTable { .. }
                | OperationKind::PortWrite { .. }
                | OperationKind::RegisterAccess { .. }
                | OperationKind::AtomicAccess { .. }
                | OperationKind::AtomicFence { .. }
        ) {
            return Err(
                "handle replay has no preserving contract for this raw address or device operation"
                    .into(),
            );
        }
        let step = graph
            .flow
            .get(&position)
            .ok_or("handle operation has no complete borrowed-storage step")?;
        if step.may_escape || graph.unresolved_contents_writes.contains(&position) {
            return Err(format!(
                "handle operation {position:?} has unresolved storage effects"
            ));
        }
        let inputs =
            self.evaluated_operands(operation, position, &prepared.expressions, occurrence)?;
        let input_words = || inputs.iter().map(|(_, value)| value.word.clone());
        let input_dependencies = || {
            input_words()
                .flat_map(|word| word.dependencies().clone())
                .collect::<BTreeSet<_>>()
        };
        let operand = |id| {
            self.scalar(id, position, &prepared.expressions)
                .map(|value| value.known.unsigned())
        };
        let sequence = match &operation.kind {
            OperationKind::EstablishByteSequenceLiteral {
                destination, bytes, ..
            } => {
                let location = DescriptorLocation::whole(*destination);
                let (slot, _, _) = geometry.resolve(&location, &self.references)?;
                let length = u64::try_from(bytes.len())
                    .map_err(|_| "handle literal length exceeds its actual source extent")?;
                Some((
                    slot,
                    geometry.present_sequence(
                        CapturedHandleSequence {
                            range: HandleStorageRange::new(
                                HandleStorageBacking {
                                    owner: HandleStorageOwner::Literal(body.id),
                                    place: *destination,
                                },
                                0,
                                length,
                            )?,
                            element: None,
                            stride: 1,
                            length: Some(length),
                            precise: true,
                            dependencies: BTreeSet::new(),
                        },
                        &location,
                    )?,
                ))
            }
            terminal_psi::element_view_establish_pair_pattern! { destination, source, element, region } =>
            {
                let location = geometry.location(source.place, &source.path)?;
                let mut captured = geometry.sequence(&location, &self.references)?;
                let representation = match region {
                    terminal_psi::ElementViewRegion::WholeCollection => false,
                    terminal_psi::ElementViewRegion::WholeRepresentation {
                        source_length, ..
                    } => {
                        captured = captured.subslice(
                            Some(0),
                            operand(*source_length)?,
                            input_dependencies(),
                        )?;
                        true
                    }
                    terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. } => {
                        captured.element = None;
                        captured.stride = 1;
                        captured.length = captured.precise.then_some(captured.range.byte_length());
                        captured = captured.subslice(
                            operand(*start)?,
                            operand(*end)?,
                            input_dependencies(),
                        )?;
                        true
                    }
                };
                captured = geometry.as_elements(captured, *element, representation)?;
                captured.dependencies.extend(input_dependencies());
                let captured = geometry
                    .present_sequence(captured, &DescriptorLocation::whole(*destination))?;
                let (slot, _, _) =
                    geometry.resolve(&DescriptorLocation::whole(*destination), &self.references)?;
                Some((slot, captured))
            }
            OperationKind::ByteSequenceSubslice {
                source, start, end, ..
            }
            | OperationKind::ElementViewSubslice {
                source, start, end, ..
            } => {
                let location = geometry.location(source.place, &source.path)?;
                let captured = geometry.sequence(&location, &self.references)?.subslice(
                    operand(*start)?,
                    operand(*end)?,
                    input_dependencies(),
                )?;
                let result = operation
                    .result
                    .structural()
                    .ok_or("handle subslice has no actual descriptor result")?;
                let captured = geometry
                    .present_sequence(captured, &DescriptorLocation::whole(result.place))?;
                let (slot, _, _) =
                    geometry.resolve(&DescriptorLocation::whole(result.place), &self.references)?;
                Some((slot, captured))
            }
            _ => None,
        };
        let mut stores = Vec::new();
        if let Some(writes) = prepared.operands.writes.get(&position) {
            // An unresolved exact writer is still handled below by the
            // canonical bounded may-write roster. It cannot produce an exact
            // alias from a guessed element or preserve an overlapping old one.
            if let Ok(writes) = writes {
                for (destination, operand) in writes {
                    let ScalarTerm::Value { id, .. } = operand else {
                        return Err("canonical scalar writer is not its evaluated operand".into());
                    };
                    let value = self.scalar(*id, position, &prepared.expressions)?;
                    let selected =
                        geometry.scalar(destination, &self.references, value.word.scalar_type())?;
                    stores.push(CapturedStore::Scalar(selected, value));
                }
            }
        }
        if let Some(captured) = prepared.resident_transfers.get(&position) {
            let ResidentValueCapture::Complete(transfers) = captured else {
                return Err(format!(
                    "handle value transfer {position:?} has no exact source roster"
                ));
            };
            for transfer in transfers {
                let (source, source_type, _) =
                    geometry.resolve(&transfer.source, &self.references)?;
                let (destination, destination_type, _) =
                    geometry.resolve(&transfer.destination, &self.references)?;
                match (source_type.scalar, destination_type.scalar) {
                    (Some(source_scalar), Some(destination_scalar)) => {
                        if source_scalar.logical_scalar != destination_scalar.logical_scalar {
                            return Err(
                                "handle value copy changes its logical scalar carrier".into()
                            );
                        }
                        stores.push(CapturedStore::Scalar(
                            geometry.scalar(
                                &transfer.destination,
                                &self.references,
                                destination_scalar.logical_scalar,
                            )?,
                            self.read_scalar(
                                &transfer.source,
                                source_scalar.logical_scalar,
                                geometry,
                            )?,
                        ));
                    }
                    (None, None) => {
                        if geometry.carrier(&transfer.source)?
                            != geometry.carrier(&transfer.destination)?
                            || geometry.carrier(&transfer.source)?.is_none()
                            || source.byte_length() != destination.byte_length()
                        {
                            return Err(
                                "handle aggregate copy changes its complete representation".into(),
                            );
                        }
                        stores.push(CapturedStore::Bytes(destination, self.capture(&source)));
                    }
                    _ => {
                        return Err(
                            "handle copy substitutes aggregate bytes for a scalar codec".into()
                        );
                    }
                }
                self.observe(self.aliases.stored_dependencies(&source), occurrence)?;
            }
        }
        let scalar_result = if let Some(result) = operation.result.scalar_ref() {
            let reads = graph
                .scalar_payload_reads
                .get(&position)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let value = if let OperationKind::ByteSequenceLength { source }
            | OperationKind::ElementViewLength { source } = &operation.kind
            {
                let location = geometry.location(source.place, &source.path)?;
                let captured = geometry.sequence(&location, &self.references)?;
                let known = match (result.scalar_type, captured.length) {
                    (ScalarType::Integer(integer), Some(length))
                        if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                            && integer.bits() == 64 =>
                    {
                        KnownHandleScalar::Integer(
                            integer,
                            IntegerValue::Unsigned(u128::from(length)),
                        )
                    }
                    (_, None) => KnownHandleScalar::Unknown,
                    _ => {
                        return Err(
                            "handle descriptor length changes its actual count carrier".into()
                        );
                    }
                };
                CapturedHandleScalar {
                    word: HandleWordAliases::derived(
                        result.scalar_type,
                        [self.aliases.stored_word(
                            &geometry.resolve(&location, &self.references)?.0,
                            result.scalar_type,
                        )],
                    ),
                    known,
                }
            } else if let OperationKind::StructuralCaseMembership { source, path, case } =
                &operation.kind
            {
                let source = geometry.location(*source, path)?;
                let (range, _, _) = geometry.resolve(&source, &self.references)?;
                let carrier = geometry
                    .carrier(&source)?
                    .ok_or("handle case observation has no original sum carrier")?;
                if let Some(tag) = self.cases.get(&range).filter(|tag| tag.carrier == carrier) {
                    CapturedHandleScalar {
                        word: HandleWordAliases::untracked(result.scalar_type),
                        known: KnownHandleScalar::Boolean(tag.case == *case),
                    }
                } else {
                    CapturedHandleScalar {
                        word: HandleWordAliases::derived(
                            result.scalar_type,
                            [self.aliases.stored_word(&range, result.scalar_type)],
                        ),
                        known: KnownHandleScalar::Unknown,
                    }
                }
            } else if !reads.is_empty() {
                let [(source, ScalarTerm::Value { id, scalar_type })] = reads else {
                    return Err("handle scalar read has an ambiguous current storage roster".into());
                };
                if *id != result.id || *scalar_type != result.scalar_type {
                    return Err("handle scalar read changes its actual result binder".into());
                }
                self.read_scalar(source, result.scalar_type, geometry)?
            } else {
                let expression = prepared
                    .expressions
                    .definitions
                    .get(result.id)
                    .and_then(Option::as_ref)
                    .and_then(|(_, expression)| expression.as_ref());
                match expression {
                    Some(ScalarTerm::Value { id, .. }) => {
                        self.scalar(*id, position, &prepared.expressions)?
                    }
                    Some(expression) => CapturedHandleScalar {
                        word: HandleWordAliases::derived(result.scalar_type, input_words()),
                        known: self.equation(
                            expression,
                            position,
                            &prepared.expressions,
                            &mut 65_536,
                        )?,
                    },
                    None => {
                        if matches!(
                            operation.kind,
                            OperationKind::PrimitiveScalarRead { .. }
                                | OperationKind::ScalarStructuralField { .. }
                                | OperationKind::BooleanStructuralField { .. }
                                | OperationKind::ByteSequenceRead { .. }
                                | OperationKind::ElementViewRead { .. }
                                | OperationKind::StructuralByteSequenceFieldRead { .. }
                        ) {
                            return Err("handle scalar storage observation has no exact current read roster".into());
                        }
                        // Unsupported scalar observations retain every actual
                        // structural input's dependencies. They cannot become
                        // exact HANDLEs merely because no equation was emitted.
                        let mut words = input_words().collect::<Vec<_>>();
                        for source in &step.inputs {
                            let (range, _, _) = geometry.resolve(source, &self.references)?;
                            words.push(self.aliases.stored_word(&range, result.scalar_type));
                        }
                        CapturedHandleScalar {
                            word: HandleWordAliases::derived(result.scalar_type, words),
                            known: KnownHandleScalar::Unknown,
                        }
                    }
                }
            };
            self.observe(value.word.dependencies().iter().copied(), occurrence)?;
            Some((result.id, value))
        } else {
            None
        };
        let reference = if let OperationKind::EstablishReference { storage, .. } = &operation.kind {
            let offset = match storage {
                Some(storage) => match self
                    .scalar(storage.byte_offset, position, &prepared.expressions)?
                    .known
                {
                    KnownHandleScalar::Integer(_, IntegerValue::Unsigned(value)) => {
                        u64::try_from(value).ok()
                    }
                    KnownHandleScalar::Integer(_, IntegerValue::Signed(value)) => {
                        u64::try_from(value).ok()
                    }
                    _ => None,
                },
                None => None,
            };
            Some(
                geometry.capture_reference(
                    operation,
                    &self.references,
                    offset,
                    input_words()
                        .flat_map(|word| word.dependencies().clone())
                        .collect(),
                )?,
            )
        } else {
            None
        };
        let definitions = step
            .definitions
            .iter()
            .map(|location| {
                geometry
                    .resolve(location, &self.references)
                    .map(|(range, _, _)| range)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let may_writes = graph
            .payload_invalidations
            .get(&position)
            .into_iter()
            .flatten()
            .map(|query| {
                let query = query
                    .as_ref()
                    .ok_or("handle write has no bounded physical footprint")?;
                geometry.possible_write(query, &self.references)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let constructors = graph
            .case_constructors
            .get(&position)
            .into_iter()
            .flatten()
            .map(|predicate| {
                let (range, _, _) = geometry.resolve(&predicate.subject, &self.references)?;
                let carrier = geometry
                    .carrier(&predicate.subject)?
                    .ok_or("handle case constructor has no original sum carrier")?;
                Ok((
                    range,
                    CapturedCaseTag {
                        carrier,
                        case: predicate.case,
                    },
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // All reads precede destination mutation. This route consumes its
        // incoming path state: refusal cannot publish a partially updated one.
        let mut next = self;
        for destination in definitions {
            next.replace(&destination);
        }
        for destination in may_writes {
            next.aliases.write_region(
                destination,
                input_words()
                    .flat_map(|word| word.dependencies().clone())
                    .collect(),
            );
            next.references.retire(&destination);
            next.cases.retain(|slot, _| !slot.overlaps(&destination));
            next.stored_constants
                .retain(|slot, _| !slot.overlaps(&destination));
        }
        for store in stores {
            match store {
                CapturedStore::Scalar(destination, value) => {
                    next.store_selected_scalar(destination, &value)?
                }
                CapturedStore::Bytes(destination, value) => next.install(&destination, &value)?,
            }
        }
        if let Some((slot, captured)) = reference {
            next.aliases
                .write_region(slot, captured.dependencies.clone());
            next.references.replace(slot, captured);
        }
        if let Some((slot, captured)) = sequence {
            next.replace(&slot);
            next.aliases
                .write_region(slot, captured.dependencies.clone());
            next.references.replace_sequence(slot, captured);
        }
        next.cases.extend(constructors);
        if let Some((result, value)) = scalar_result {
            next.define(result, value, &prepared.expressions)?;
        }
        Ok(next)
    }

    fn enter(
        &self,
        expressions: &LocalScalarExpressions,
        body: &terminal_psi::TerminalMachine,
    ) -> Result<Self, String> {
        Ok(Self {
            ledger: self.ledger.clone(),
            aliases: self.aliases.enter_activation(
                expressions
                    .definitions
                    .iter()
                    .filter_map(|(id, row)| row.is_some().then_some(id)),
            )?,
            references: self.references.clone(),
            cases: self.cases.clone(),
            constants: expressions.definitions.map(|_, _| None),
            stored_constants: self.stored_constants.clone(),
            bindings: HandleStorageBindings::new(body)?,
        })
    }

    fn scalar(
        &self,
        value: ValueId,
        observation: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        expressions: &LocalScalarExpressions,
    ) -> Result<CapturedHandleScalar, String> {
        if !expressions.available_at(value, observation) {
            return Err(format!(
                "handle scalar {value:?} is not available at its actual use {observation:?}"
            ));
        }
        let &(expected, _) = expressions
            .definitions
            .get(value)
            .and_then(Option::as_ref)
            .ok_or("handle scalar use has no original binder declaration")?;
        let word = self.aliases.scalar(value)?.clone();
        let known = self
            .constants
            .get(value)
            .and_then(Option::as_ref)
            .ok_or("handle scalar use has no evaluated original snapshot")?
            .clone();
        if word.scalar_type() != expected || !known.has_type(expected) {
            return Err("handle scalar snapshot changes its original carrier".into());
        }
        Ok(CapturedHandleScalar { word, known })
    }

    fn define(
        &mut self,
        value: ValueId,
        captured: CapturedHandleScalar,
        expressions: &LocalScalarExpressions,
    ) -> Result<(), String> {
        let &(expected, _) = expressions
            .definitions
            .get(value)
            .and_then(Option::as_ref)
            .ok_or("handle scalar result has no original binder declaration")?;
        if captured.word.scalar_type() != expected
            || !captured.known.has_type(expected)
            || self.constants.get(value).is_none()
        {
            return Err(
                "handle scalar definition changes its declared carrier or namespace".into(),
            );
        }
        self.aliases.define_scalar(value, captured.word)?;
        *self
            .constants
            .get_mut(value)
            .expect("scalar coordinate was checked before commit") = Some(captured.known);
        Ok(())
    }

    fn evaluated_operands(
        &mut self,
        operation: &Operation,
        observation: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        expressions: &LocalScalarExpressions,
        occurrence: ScalarOccurrence,
    ) -> Result<Vec<(ValueId, CapturedHandleScalar)>, String> {
        let mut inputs = Vec::new();
        operation.kind.clone().map_scalar_uses(&mut |value| {
            inputs.push(value);
            value
        });
        inputs.sort_unstable();
        inputs.dedup();
        let values = inputs
            .into_iter()
            .map(|value| {
                self.scalar(value, observation, expressions)
                    .map(|captured| (value, captured))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.observe(
            values
                .iter()
                .flat_map(|(_, value)| value.word.dependencies().iter().copied()),
            occurrence,
        )?;
        Ok(values)
    }

    /// Interpret the producing equation against its operands now. Later
    /// guards read this result, without expanding it through renewed operands.
    fn equation(
        &self,
        term: &ScalarTerm,
        observation: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        expressions: &LocalScalarExpressions,
        remaining: &mut usize,
    ) -> Result<KnownHandleScalar, String> {
        *remaining = remaining
            .checked_sub(1)
            .ok_or("handle scalar equation exceeds its bounded operand work")?;
        if let ScalarTerm::Value { id, .. } = term {
            return Ok(self.scalar(*id, observation, expressions)?.known);
        }
        let values = term
            .operands()
            .iter()
            .map(|operand| self.equation(operand, observation, expressions, remaining))
            .collect::<Result<Vec<_>, _>>()?;
        if let ScalarTerm::IntegerEqual { left, right, .. } = term {
            let zero = |value: &KnownHandleScalar| {
                matches!(
                    value,
                    KnownHandleScalar::Integer(
                        _,
                        IntegerValue::Signed(0) | IntegerValue::Unsigned(0)
                    )
                )
            };
            if values.len() == 2
                && ((matches!(values[0], KnownHandleScalar::Nonzero(_)) && zero(&values[1]))
                    || (matches!(values[1], KnownHandleScalar::Nonzero(_)) && zero(&values[0])))
            {
                return Ok(KnownHandleScalar::Boolean(false));
            }
            // These instances were born only on successful CreateFileW or
            // same-object DuplicateHandle outcomes. A valid file/device object
            // handle excludes NULL and INVALID_HANDLE_VALUE; process pseudo
            // handles and arbitrary external integers never enter this set.
            // Native premise: CreateFileW's valid-handle result contract and
            // Application Verifier's INVALID_HANDLE / NULL_HANDLE checks.
            let invalid_file_word = |value: &KnownHandleScalar| match value {
                KnownHandleScalar::Integer(integer, IntegerValue::Signed(value)) => {
                    integer.bits() == 64 && (*value == 0 || *value == -1)
                }
                KnownHandleScalar::Integer(integer, IntegerValue::Unsigned(value)) => {
                    integer.bits() == 64 && (*value == 0 || *value == u64::MAX as u128)
                }
                _ => false,
            };
            let exact_file = |term: &ScalarTerm| -> Result<bool, String> {
                let ScalarTerm::Value { id, .. } = term else {
                    return Ok(false);
                };
                Ok(self.aliases.scalar(*id)?.exact_handle().is_ok())
            };
            if values.len() == 2
                && ((exact_file(left)? && invalid_file_word(&values[1]))
                    || (exact_file(right)? && invalid_file_word(&values[0])))
            {
                return Ok(KnownHandleScalar::Boolean(false));
            }
            if let (ScalarTerm::Value { id: left, .. }, ScalarTerm::Value { id: right, .. }) =
                (left.as_ref(), right.as_ref())
            {
                if let (Ok(left), Ok(right)) = (
                    self.aliases.scalar(*left)?.exact_handle(),
                    self.aliases.scalar(*right)?.exact_handle(),
                ) {
                    if left == right {
                        return Ok(KnownHandleScalar::Boolean(true));
                    }
                }
            }
        }
        let mut instantiated = term.clone();
        for (operand, value) in instantiated.operands_mut().into_iter().zip(values) {
            let Some(value) = value.literal() else {
                return Ok(KnownHandleScalar::Unknown);
            };
            *operand = value;
        }
        if let Some((scalar_type, value)) = instantiated.integer_value() {
            return Ok(KnownHandleScalar::Integer(scalar_type, value));
        }
        if let Some(value) = instantiated.boolean_value() {
            return Ok(KnownHandleScalar::Boolean(value));
        }
        Ok(KnownHandleScalar::Unknown)
    }

    fn capture(&self, region: &HandleStorageRange) -> CapturedMemory {
        CapturedMemory {
            words: self.aliases.capture_region(region),
            references: self.references.capture(region),
            cases: self
                .cases
                .iter()
                .filter_map(|(slot, value)| {
                    slot.relative_to(region)
                        .map(|relative| (relative, value.clone()))
                })
                .collect(),
            constants: self
                .stored_constants
                .iter()
                .filter_map(|(slot, value)| {
                    slot.relative_to(region)
                        .map(|relative| (relative, value.clone()))
                })
                .collect(),
        }
    }

    fn replace(&mut self, region: &HandleStorageRange) {
        self.aliases.replace_complete_region(region);
        self.references.retire(region);
        self.cases.retain(|slot, _| !slot.overlaps(region));
        self.stored_constants
            .retain(|slot, _| !slot.overlaps(region));
    }

    fn install(
        &mut self,
        region: &HandleStorageRange,
        captured: &CapturedMemory,
    ) -> Result<(), String> {
        // Prepare the tag destinations too, before any component can erase an
        // old source. Each captured component was bounded by the same source.
        let cases = captured
            .cases
            .iter()
            .map(|(relative, value)| Ok((relative.at(region)?, value.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        let constants = captured
            .constants
            .iter()
            .map(|(relative, value)| Ok((relative.at(region)?, value.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        let mut next = self.clone();
        next.aliases.install_region(region, &captured.words)?;
        next.references.install(region, &captured.references)?;
        next.cases.retain(|slot, _| !slot.overlaps(region));
        next.cases.extend(cases);
        next.stored_constants
            .retain(|slot, _| !slot.overlaps(region));
        next.stored_constants.extend(constants);
        *self = next;
        Ok(())
    }

    fn observe(
        &mut self,
        dependencies: impl IntoIterator<Item = super::windows_handle_lifetimes::HandleInstance>,
        occurrence: ScalarOccurrence,
    ) -> Result<(), String> {
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        self.ledger
            .observe_scalars(&dependencies, occurrence)
            .map_err(|reason| {
                format!("handle scalar observation is outside its live generation: {reason:?}")
            })
    }

    fn store_scalar(
        &mut self,
        location: &DescriptorLocation,
        value: &CapturedHandleScalar,
        geometry: &HandleStorageGeometry<'_>,
    ) -> Result<(), String> {
        let storage = geometry.scalar(location, &self.references, value.word.scalar_type())?;
        self.store_selected_scalar(storage, value)
    }

    fn store_selected_scalar(
        &mut self,
        storage: HandleScalarStorage,
        value: &CapturedHandleScalar,
    ) -> Result<(), String> {
        let bits = value.word.represented_as(storage.stored_scalar);
        // Complete physical width, independent of the logical source type.
        // Overlapping larger cells retain untouched dependencies in the kernel.
        self.aliases
            .write_complete_word(storage.range, storage.stored_scalar, bits)?;
        self.references.retire(&storage.range);
        self.cases.retain(|slot, _| !slot.overlaps(&storage.range));
        self.stored_constants
            .retain(|slot, _| !slot.overlaps(&storage.range));
        if value.known.has_type(storage.stored_scalar)
            && value.word.scalar_type() == storage.stored_scalar
            && value.known != KnownHandleScalar::Unknown
        {
            self.stored_constants
                .insert(storage.range, value.known.clone());
        }
        Ok(())
    }

    fn read_scalar(
        &self,
        location: &DescriptorLocation,
        scalar_type: ScalarType,
        geometry: &HandleStorageGeometry<'_>,
    ) -> Result<CapturedHandleScalar, String> {
        let storage = geometry.scalar(location, &self.references, scalar_type)?;
        // Numeric metadata follows the same complete stored codec as the word.
        // A different-width or noninteger interpretation cannot revive a saved
        // handle identity or reinterpret its old Boolean snapshot.
        let known = self
            .stored_constants
            .get(&storage.range)
            .filter(|value| storage.stored_scalar == scalar_type && value.has_type(scalar_type))
            .cloned()
            .unwrap_or(KnownHandleScalar::Unknown);
        Ok(CapturedHandleScalar {
            word: storage.read(&self.aliases),
            known,
        })
    }
}

enum CapturedCallArgument {
    Scalar(CapturedHandleScalar),
    Bytes(CapturedMemory),
}
