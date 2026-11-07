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
    CapturedHandleRegion, HandleAliases, HandleStorageRange, HandleWordAliases,
};
use super::windows_handle_calls::{CallSite, CallTarget, Caller, HandleCallGraph, RuntimeCall};
use super::windows_handle_inputs::PreparedHandleMachine;
use super::windows_handle_lifetimes::{
    HandleLifetimes, NativeOccurrence, ScalarOccurrence, ScalarUseSite,
};
use super::windows_handle_storage::{
    CapturedHandleReferences, HandleReferences, HandleScalarStorage, HandleStorageBindings,
    HandleStorageGeometry,
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

impl CanonicalHandleState {
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
            let value = if let OperationKind::StructuralCaseMembership { source, path, case } =
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
