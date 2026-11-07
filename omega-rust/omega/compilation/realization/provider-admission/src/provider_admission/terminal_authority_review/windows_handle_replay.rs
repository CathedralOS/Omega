//! Replay current values, physical aliases and exact matched activations.
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
use terminal_semantics::descriptor_lifetimes::DescriptorLocation;

use crate::digest_encoder::DomainDigestEncoder;

use super::context::ReviewContext;
use super::windows_handle_aliases::{
    CapturedHandleRegion, HandleAliases, HandleStorageRange, HandleWordAliases,
};
use super::windows_handle_calls::{CallSite, CallTarget, Caller, HandleCallGraph, RuntimeCall};
use super::windows_handle_lifetimes::{
    HandleLifetimes, NativeOccurrence, ScalarOccurrence, ScalarUseSite,
};
use super::windows_handle_storage::{
    CapturedHandleReferences, HandleReferences, HandleStorageGeometry,
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
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalHandleState {
    ledger: HandleLifetimes,
    aliases: HandleAliases,
    references: HandleReferences,
    cases: BTreeMap<HandleStorageRange, CapturedCaseTag>,
    constants: IdTable<ValueId, Option<KnownHandleScalar>>,
}

impl CanonicalHandleState {
    fn enter(&self, expressions: &LocalScalarExpressions) -> Result<Self, String> {
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
        })
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
        }
    }

    fn replace(&mut self, region: &HandleStorageRange) {
        self.aliases.replace_complete_region(region);
        self.references.retire(region);
        self.cases.retain(|slot, _| !slot.overlaps(region));
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
        let mut next = self.clone();
        next.aliases.install_region(region, &captured.words)?;
        next.references.install(region, &captured.references)?;
        next.cases.retain(|slot, _| !slot.overlaps(region));
        next.cases.extend(cases);
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
        value: &HandleWordAliases,
        geometry: &HandleStorageGeometry<'_>,
    ) -> Result<(), String> {
        let storage = geometry.scalar(location, &self.references, value.scalar_type())?;
        let bits = value.represented_as(storage.stored_scalar);
        // Complete physical width, independent of the logical source type.
        // Overlapping larger cells retain untouched dependencies in the kernel.
        self.aliases
            .write_complete_word(storage.range, storage.stored_scalar, bits)?;
        self.references.retire(&storage.range);
        self.cases.retain(|slot, _| !slot.overlaps(&storage.range));
        Ok(())
    }
}
