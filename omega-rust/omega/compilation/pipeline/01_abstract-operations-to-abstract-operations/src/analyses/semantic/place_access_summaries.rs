//! Per-function place-access surfaces bound to one unit revision.
//!
//! [`super::EffectSummaryAnalysis`] answers "may this function have an
//! observable effect"; this analysis answers the finer question a call site
//! needs: which caller storage a callee may read or write, projected through
//! the call's own `structural_arguments`. A coarse structural-state flag is
//! not non-aliasing evidence — a `MutableBorrow` parameter the body never
//! stores through is observed read-only here, and a function whose external
//! rows are empty provably reaches no storage outside its argument lane.
//!
//! Every recorded path is a *may-access bound*: a row means the body may
//! touch some storage beneath that view, never that the view is the exact
//! extent. Bound semantics keep the transitive join finite — a recursive
//! callee's per-parameter sub-paths collapse to the lent argument view
//! rather than composing into an unbounded path grammar. `RuntimeIndex`
//! steps name caller- or callee-local scalars, so a callee path crossing a
//! call boundary truncates just before its first runtime segment: the
//! prefix is the bound every caller can express in its own operand identity.
//!
//! `Revision` is carried per row like the effect summary: a retained row
//! from an older unit revision is stale evidence, so the analysis manager
//! force-invalidates this product across every commit.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractAtomicEvent, AbstractDynamicDescriptorSource, AbstractDynamicDispatch,
    AbstractOperation as O, AbstractStructuralBinding, StructuralCopyPath,
};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, MachineId, PlaceId, StructuralFieldId,
    StructuralPlaceKind,
};
use terminal_psi::{
    ClosedConformanceApplication, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralPathSegment, TerminalAffineCleanupAction, control_flow::ByteSequenceOperand,
};

use super::semantically_reachable_blocks;

/// One projection step inside a recorded access view. Runtime spellings keep
/// the operation's own `StructuralPathSegment`; verifier-keyed spellings —
/// canonical paths and the terminal `field` of a field operation — keep
/// `CanonicalStructuralPathSegment` so structural-field identity survives
/// without being flattened into a display string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlaceAccessStep {
    Runtime(StructuralPathSegment),
    Canonical(CanonicalStructuralPathSegment),
}

impl PlaceAccessStep {
    #[cfg(test)]
    fn is_runtime_index(&self) -> bool {
        matches!(
            self,
            Self::Runtime(StructuralPathSegment::RuntimeIndex { .. })
        )
    }

    fn is_referent(&self) -> bool {
        matches!(self, Self::Runtime(StructuralPathSegment::Referent))
    }
}

/// One may-access view: storage at `root`, a place declared by `owner`,
/// beneath `path`. An empty path is the whole root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceAccessView {
    pub owner: MachineId,
    pub root: PlaceId,
    pub path: Vec<PlaceAccessStep>,
}

/// What one body provably does through one structural parameter. `declared`
/// is the signature's access bound; `reads` and `writes` hold the observed
/// bound paths beneath the parameter's root — both empty means the body
/// provably never reaches the parameter's storage, a fact the declared
/// signature alone cannot state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterAccessSurface {
    pub position: u32,
    pub place: PlaceId,
    pub declared: StructuralAccess,
    pub reads: Vec<Vec<PlaceAccessStep>>,
    pub writes: Vec<Vec<PlaceAccessStep>>,
}

#[cfg(test)]
impl ParameterAccessSurface {
    /// Whether the body may observe any storage beneath this parameter.
    pub fn is_read(&self) -> bool {
        !self.reads.is_empty()
    }

    /// Whether the body may mutate any storage beneath this parameter.
    pub fn is_written(&self) -> bool {
        !self.writes.is_empty()
    }
}

/// One declared result reference leaf's resolved ingress, in the callee's
/// namespace: where the reference the result carries at `result_path` points,
/// resolved through this function's own view aliases and block bindings. The
/// verifier's `reference_sources` rows spell callee-local places; this
/// resolution is what a caller cannot read off the signature — whether the
/// source is a formal parameter lane it lent, or storage outside the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedResultSource {
    pub result_path: Vec<StructuralPathSegment>,
    pub origins: Vec<ResultSourceResolution>,
}

/// Where one resolved result-source view lives, in the callee's namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultSourceResolution {
    /// Beneath formal parameter `position`, along `path` under its root.
    Parameter {
        position: u32,
        path: Vec<PlaceAccessStep>,
    },
    /// Outside every formal parameter: provider attachments, referent
    /// escapes no local alias resolves, or undeclared roots. `root` is the
    /// callee-local place the access named.
    External {
        root: PlaceId,
        path: Vec<PlaceAccessStep>,
    },
    /// Callee-owned storage — result root or local — that a returned
    /// reference cannot legitimately name. Reachable only under bare seeds:
    /// a verified unit proves each returned carrier's formal ingress.
    Internal,
}

/// One machine's transitive place surface. `external_*` rows are rooted at
/// places the unit cannot attribute to a formal parameter, an owned local,
/// or the result: provider attachments, referent escapes no local alias
/// resolves, undeclared roots, and rows propagated from callees (which keep
/// their declaring `owner`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionPlaceAccessSummary {
    pub machine: MachineId,
    /// One row per declared structural parameter, in signature order.
    pub parameters: Vec<ParameterAccessSurface>,
    pub external_reads: Vec<PlaceAccessView>,
    pub external_writes: Vec<PlaceAccessView>,
    /// Resolved ingress of each reference leaf the declared structural result
    /// carries, in `reference_sources` order. Empty for scalar and unit
    /// results and for structural results that carry no references.
    pub result_sources: Vec<ResolvedResultSource>,
    /// `true` when a reachable call targets a machine this unit does not
    /// carry, a dynamic-dispatch lane, or a boundary. Parameter surfaces
    /// stay exact — an unsummarized target's argument lane is bounded by
    /// each argument's declared `access` — but external rows are then a
    /// lower bound, not the complete reach.
    pub unsummarized_targets: bool,
    pub support: Vec<PsiProvenance>,
    pub revision: OptimizationUnitIdentity,
}

/// The place-access fact product for one unit revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAccessSummaryAnalysis {
    pub functions: Vec<FunctionPlaceAccessSummary>,
}

/// One structural argument's observed access projected into caller storage.
///
/// The whole call-site query surface below is staged for the proof-bound
/// inlining rewrite that consumes it (INTERPROCEDURAL-SUMMARIES): it is
/// exercised by this crate's analysis tests and stays `cfg(test)` until a
/// production proposal reads it.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedArgumentAccess {
    /// Index of the argument inside the call's `structural_arguments` row.
    pub position: u32,
    /// Caller-side views the call may read through this argument: the
    /// callee's observed parameter paths composed beneath the argument's own
    /// path. Empty means the callee provably does not observe the lane.
    pub reads: Vec<PlaceAccessView>,
    /// Caller-side views the call may write through this argument.
    pub writes: Vec<PlaceAccessView>,
    /// `false` when the callee signature declares no parameter at this
    /// position, leaving the argument's declared `access` as the only bound.
    pub signature_bound: bool,
}

/// A whole call's observed caller-visible access.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallAccessSummary {
    /// One row per `structural_arguments` entry, in argument order.
    pub arguments: Vec<ObservedArgumentAccess>,
    /// One row per `reference_sources` entry on the callee's declared
    /// structural result — where each returned reference leaf may point,
    /// projected into caller storage where it crosses the argument lane.
    /// Empty for scalar and unit results.
    pub result_references: Vec<CallResultReference>,
    /// Storage outside the argument lane the callee may read, in the
    /// callee's namespace (`owner` is the declaring machine).
    pub external_reads: Vec<PlaceAccessView>,
    /// Storage outside the argument lane the callee may write.
    pub external_writes: Vec<PlaceAccessView>,
    /// `true` when the callee's external surface is a lower bound — the call
    /// may touch storage the unit cannot enumerate.
    pub unsummarized_targets: bool,
}

/// One returned reference leaf's caller-projected ingress.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResultReference {
    /// Path inside the returned root where this reference leaf lands.
    pub result_path: Vec<StructuralPathSegment>,
    /// Every origin the callee resolved for this leaf, projected into the
    /// caller's namespace where the resolution crosses the argument lane.
    pub origins: Vec<CallResultOrigin>,
}

/// Where one caller-visible result-reference origin lives.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallResultOrigin {
    /// Caller storage lent through the call's argument row: the returned
    /// reference may alias this view.
    Argument(PlaceAccessView),
    /// Storage outside the argument lane, in the callee's namespace
    /// (`owner` is the callee machine).
    CalleeExternal(PlaceAccessView),
    /// A callee parameter lane the call's `structural_arguments` row does not
    /// cover — kept callee-spelled because no caller view exists for it.
    /// Reachable only where the call itself fails admission.
    UncoveredParameter {
        position: u32,
        path: Vec<PlaceAccessStep>,
    },
    /// Callee-owned storage the caller never observes through this call.
    CalleeInternal,
}

/// The access a declared [`StructuralAccess`] authorizes: `(reads, writes)`.
/// This is the complete bound applied to argument lanes whose callee the
/// unit cannot observe. `Owned` counts as both — consumption observes the
/// content and vacates the caller's custody.
pub fn declared_access(access: StructuralAccess) -> (bool, bool) {
    match access {
        StructuralAccess::SharedBorrow => (true, false),
        StructuralAccess::MutableBorrow => (true, true),
        StructuralAccess::WriteOnlyBorrow => (false, true),
        StructuralAccess::Owned => (true, true),
    }
}

#[cfg(test)]
impl PlaceAccessSummaryAnalysis {
    pub fn function(&self, machine: MachineId) -> Option<&FunctionPlaceAccessSummary> {
        self.functions
            .binary_search_by_key(&machine, |function| function.machine)
            .ok()
            .map(|index| &self.functions[index])
    }

    /// The observed caller-visible access of one static in-unit call,
    /// projected through its `structural_arguments` into `caller` storage.
    /// `None` means the unit carries no body for `callee`: each argument's
    /// declared `access` — see [`declared_access`] — is then the complete
    /// bound on its lane.
    pub fn call_accesses(
        &self,
        caller: MachineId,
        callee: MachineId,
        structural_arguments: &[StructuralArgument],
    ) -> Option<CallAccessSummary> {
        let summary = self.function(callee)?;
        let mut arguments = Vec::with_capacity(structural_arguments.len());
        for (index, argument) in structural_arguments.iter().enumerate() {
            let position = u32::try_from(index).expect("structural argument index is u32");
            let prefix = runtime_prefix(&argument.path);
            let Some(surface) = summary
                .parameters
                .iter()
                .find(|surface| surface.position == position)
            else {
                arguments.push(ObservedArgumentAccess {
                    position,
                    reads: Vec::new(),
                    writes: Vec::new(),
                    signature_bound: false,
                });
                continue;
            };
            let project = |paths: &[Vec<PlaceAccessStep>]| {
                paths
                    .iter()
                    .map(|path| PlaceAccessView {
                        owner: caller,
                        root: argument.place,
                        path: prefix
                            .iter()
                            .cloned()
                            .chain(portable_path(path).iter().cloned())
                            .collect(),
                    })
                    .collect()
            };
            arguments.push(ObservedArgumentAccess {
                position,
                reads: project(&surface.reads),
                writes: project(&surface.writes),
                signature_bound: true,
            });
        }
        let result_references = summary
            .result_sources
            .iter()
            .map(|source| CallResultReference {
                result_path: source.result_path.clone(),
                origins: source
                    .origins
                    .iter()
                    .map(|origin| match origin {
                        ResultSourceResolution::Parameter { position, path } => {
                            let index =
                                usize::try_from(*position).expect("parameter position is usize");
                            match structural_arguments.get(index) {
                                Some(argument) => CallResultOrigin::Argument(PlaceAccessView {
                                    owner: caller,
                                    root: argument.place,
                                    path: runtime_prefix(&argument.path)
                                        .iter()
                                        .cloned()
                                        .chain(portable_path(path).iter().cloned())
                                        .collect(),
                                }),
                                None => CallResultOrigin::UncoveredParameter {
                                    position: *position,
                                    path: path.clone(),
                                },
                            }
                        }
                        ResultSourceResolution::External { root, path } => {
                            CallResultOrigin::CalleeExternal(PlaceAccessView {
                                owner: callee,
                                root: *root,
                                path: path.clone(),
                            })
                        }
                        ResultSourceResolution::Internal => CallResultOrigin::CalleeInternal,
                    })
                    .collect(),
            })
            .collect();
        Some(CallAccessSummary {
            arguments,
            result_references,
            external_reads: summary.external_reads.clone(),
            external_writes: summary.external_writes.clone(),
            unsummarized_targets: summary.unsummarized_targets,
        })
    }
}

/// The callee-local prefix of a path that a caller can name. A
/// `RuntimeIndex` segment quotes a callee-side scalar value, so a path
/// carrying one truncates just before it — a bound the caller's own operand
/// identity can express.
#[cfg(test)]
fn portable_path(path: &[PlaceAccessStep]) -> &[PlaceAccessStep] {
    match path.iter().position(PlaceAccessStep::is_runtime_index) {
        Some(index) => &path[..index],
        None => path,
    }
}

fn runtime_prefix(path: &[StructuralPathSegment]) -> Vec<PlaceAccessStep> {
    path.iter().cloned().map(PlaceAccessStep::Runtime).collect()
}

fn canonical_prefix(path: &[CanonicalStructuralPathSegment]) -> Vec<PlaceAccessStep> {
    path.iter()
        .copied()
        .map(PlaceAccessStep::Canonical)
        .collect()
}

fn canonical_field(field: StructuralFieldId) -> PlaceAccessStep {
    PlaceAccessStep::Canonical(CanonicalStructuralPathSegment::Field(field))
}

fn with_field(path: &mut Vec<PlaceAccessStep>, field: StructuralFieldId) {
    path.push(canonical_field(field));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessKind {
    Read,
    Write,
}

/// Where one accessed view's storage actually lives relative to a function's
/// own boundaries.
enum ResolvedAccess {
    /// Beneath a formal parameter root; carries the parameter's position and
    /// the bound path under it.
    Parameter {
        position: u32,
        path: Vec<PlaceAccessStep>,
    },
    /// Outside every formal parameter, owned local, and the result place —
    /// caller-visible storage the summary must report. `root` is the
    /// function-local place the access named.
    External {
        root: PlaceId,
        path: Vec<PlaceAccessStep>,
    },
    /// Owned local or result storage the caller never observes as a call
    /// side effect.
    Internal,
}

/// A view-producing result's target. Subslice and element-view results are
/// prefixes of their source; a reference result's accesses cross the
/// `Referent` segment into the source's storage.
#[derive(Debug, Clone)]
enum AliasTarget {
    View {
        target: PlaceId,
        prefix: Vec<PlaceAccessStep>,
    },
    Reference {
        target: PlaceId,
        prefix: Vec<PlaceAccessStep>,
    },
}

/// One function's immutable resolution context: the declared root roster,
/// the view aliases its operations establish, and the structural bindings
/// every edge hands a block parameter.
struct FunctionContext<'u> {
    function: &'u PsiOptimizationFunction,
    roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>>,
    aliases: BTreeMap<PlaceId, AliasTarget>,
    bindings: BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>>,
    reachable: BTreeSet<BlockId>,
}

impl<'u> FunctionContext<'u> {
    fn build(function: &'u PsiOptimizationFunction) -> Self {
        let mut roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>> = function
            .declared_places
            .iter()
            .map(|place| (*place, None))
            .collect();
        for declaration in &function.structural_places {
            roots.insert(declaration.id, Some(declaration.kind));
        }
        // Bare seeds may omit the derived roster entries the verified unit
        // carries; the signature itself is the authority for parameters and
        // the result root.
        for parameter in &function.structural_parameters {
            roots.insert(
                parameter.place,
                Some(StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                }),
            );
        }
        for block in &function.blocks {
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                roots.insert(
                    parameter.place,
                    Some(StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: u32::try_from(position)
                            .expect("block structural parameter index is u32"),
                    }),
                );
            }
        }
        if let Some(result) = function.result.structural() {
            roots.insert(result.place, Some(StructuralPlaceKind::Result));
        }

        let mut aliases = BTreeMap::new();
        let mut bindings: BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>> =
            BTreeMap::new();
        for block in &function.blocks {
            for node in &block.nodes {
                match &node.operation {
                    O::ByteSequenceSubslice { result, source, .. }
                    | O::ElementViewSubslice { result, source, .. } => {
                        aliases.insert(
                            result.place,
                            AliasTarget::View {
                                target: *source,
                                prefix: Vec::new(),
                            },
                        );
                    }
                    O::EstablishElementView { result, source, .. } => {
                        aliases.insert(
                            result.place,
                            AliasTarget::View {
                                target: source.place,
                                prefix: runtime_prefix(&source.path),
                            },
                        );
                    }
                    O::EstablishReference { result, source, .. } => {
                        aliases.insert(
                            result.place,
                            AliasTarget::Reference {
                                target: source.place,
                                prefix: runtime_prefix(&source.path),
                            },
                        );
                    }
                    O::Jump {
                        structural_bindings,
                        ..
                    } => bind_arguments(&mut bindings, structural_bindings),
                    O::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        bind_arguments(&mut bindings, &when_true.structural_bindings);
                        bind_arguments(&mut bindings, &when_false.structural_bindings);
                    }
                    _ => {}
                }
            }
        }
        Self {
            function,
            roots,
            aliases,
            bindings,
            reachable: semantically_reachable_blocks(function),
        }
    }

    /// Chase one accessed view to the storage it names. Block parameters
    /// resolve through every edge binding — one place, several incoming
    /// views — and view results resolve through their source alias. A
    /// `visiting` cycle or a root outside the declared roster is external:
    /// only an unverified seed produces either, and refusing to attribute is
    /// the sound answer, not silence.
    fn resolve(&self, place: PlaceId, path: Vec<PlaceAccessStep>) -> Vec<ResolvedAccess> {
        let mut visiting = BTreeSet::new();
        self.resolve_inner(place, path, &mut visiting)
    }

    fn resolve_inner(
        &self,
        place: PlaceId,
        path: Vec<PlaceAccessStep>,
        visiting: &mut BTreeSet<PlaceId>,
    ) -> Vec<ResolvedAccess> {
        if !visiting.insert(place) {
            return vec![ResolvedAccess::External { root: place, path }];
        }
        let resolved = self.resolve_kind(place, path, visiting);
        visiting.remove(&place);
        resolved
    }

    fn resolve_kind(
        &self,
        place: PlaceId,
        path: Vec<PlaceAccessStep>,
        visiting: &mut BTreeSet<PlaceId>,
    ) -> Vec<ResolvedAccess> {
        match self.roots.get(&place).copied().flatten() {
            Some(StructuralPlaceKind::Parameter { position, .. }) => {
                vec![ResolvedAccess::Parameter { position, path }]
            }
            Some(StructuralPlaceKind::BlockParameter { .. }) => match self.bindings.get(&place) {
                Some(arguments) if !arguments.is_empty() => arguments
                    .iter()
                    .flat_map(|(argument_place, argument_path)| {
                        let mut composed = runtime_prefix(argument_path);
                        composed.extend(path.iter().cloned());
                        self.resolve_inner(*argument_place, composed, visiting)
                    })
                    .collect(),
                // A binding-less block parameter exists only where no edge
                // can reach it; conservatively report it as external.
                _ => vec![ResolvedAccess::External { root: place, path }],
            },
            Some(StructuralPlaceKind::OperationResult { .. })
            | Some(StructuralPlaceKind::ByteSequenceLiteral { .. })
            | Some(StructuralPlaceKind::TrivialAffineLocal { .. }) => {
                match self.aliases.get(&place) {
                    Some(AliasTarget::View { target, prefix }) => {
                        let mut composed = prefix.clone();
                        composed.extend(path.iter().cloned());
                        self.resolve_inner(*target, composed, visiting)
                    }
                    Some(AliasTarget::Reference { target, prefix }) => match path.first() {
                        Some(step) if step.is_referent() => {
                            // Dereferencing the carrier reaches the
                            // referent's storage beneath the retained source
                            // projection.
                            let mut composed = prefix.clone();
                            composed.extend(path[1..].iter().cloned());
                            self.resolve_inner(*target, composed, visiting)
                        }
                        // Any other path touches only the local carrier
                        // cell, never the referent.
                        _ => vec![ResolvedAccess::Internal],
                    },
                    None if path.iter().any(PlaceAccessStep::is_referent) => {
                        // A referent step inside an owned local's path leaves
                        // that root's storage for storage owned elsewhere —
                        // through a reference field no local alias tracks.
                        vec![ResolvedAccess::External { root: place, path }]
                    }
                    None => vec![ResolvedAccess::Internal],
                }
            }
            Some(StructuralPlaceKind::Result) => {
                if path.iter().any(PlaceAccessStep::is_referent) {
                    vec![ResolvedAccess::External { root: place, path }]
                } else {
                    vec![ResolvedAccess::Internal]
                }
            }
            // Provider attachments and undeclared roots are storage the
            // unit cannot place inside this function's own custody.
            Some(StructuralPlaceKind::ProviderAttachment { .. }) | None => {
                vec![ResolvedAccess::External { root: place, path }]
            }
        }
    }
}

fn bind_arguments(
    bindings: &mut BTreeMap<PlaceId, Vec<(PlaceId, Vec<StructuralPathSegment>)>>,
    structural_bindings: &[AbstractStructuralBinding],
) {
    for binding in structural_bindings {
        bindings
            .entry(binding.parameter)
            .or_default()
            .push((binding.argument.place, binding.argument.path.clone()));
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct PendingSurface {
    parameter_reads: BTreeMap<u32, BTreeSet<Vec<PlaceAccessStep>>>,
    parameter_writes: BTreeMap<u32, BTreeSet<Vec<PlaceAccessStep>>>,
    external_reads: BTreeSet<PlaceAccessView>,
    external_writes: BTreeSet<PlaceAccessView>,
    unsummarized: bool,
    support: BTreeSet<PsiProvenance>,
    contributions: Vec<PendingContribution>,
}

/// A call edge whose callee surface joins into the caller's pending rows on
/// every fixpoint round.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingContribution {
    /// A direct call to a unit machine: the callee's per-parameter surfaces
    /// project through this call's structural argument views.
    Call {
        callee: MachineId,
        arguments: Vec<StructuralArgument>,
    },
    /// A unit machine whose modeled caller-visible reach is storage outside
    /// the argument lane — cleanup hooks and known dynamic realizations.
    Reach { callee: MachineId },
}

impl PendingSurface {
    fn attribute(
        &mut self,
        context: &FunctionContext<'_>,
        place: PlaceId,
        path: Vec<PlaceAccessStep>,
        kind: AccessKind,
    ) {
        for resolved in context.resolve(place, path) {
            match (resolved, kind) {
                (ResolvedAccess::Parameter { position, path }, AccessKind::Read) => {
                    self.parameter_reads
                        .entry(position)
                        .or_default()
                        .insert(path);
                }
                (ResolvedAccess::Parameter { position, path }, AccessKind::Write) => {
                    self.parameter_writes
                        .entry(position)
                        .or_default()
                        .insert(path);
                }
                (ResolvedAccess::External { root, path }, AccessKind::Read) => {
                    self.external_reads.insert(PlaceAccessView {
                        owner: context.function.machine,
                        root,
                        path,
                    });
                }
                (ResolvedAccess::External { root, path }, AccessKind::Write) => {
                    self.external_writes.insert(PlaceAccessView {
                        owner: context.function.machine,
                        root,
                        path,
                    });
                }
                (ResolvedAccess::Internal, _) => {}
            }
        }
    }

    /// Attribute `reads`/`writes` on the argument's own lent view — the join
    /// shape for both declared bounds and projected callee surfaces. The
    /// bound inserts `argument.path` itself, never a callee path composed
    /// beneath it, which is what keeps recursive SCCs finite.
    fn attribute_argument(
        &mut self,
        context: &FunctionContext<'_>,
        argument: &StructuralArgument,
        reads: bool,
        writes: bool,
    ) {
        let path = runtime_prefix(&argument.path);
        if reads {
            self.attribute(context, argument.place, path.clone(), AccessKind::Read);
        }
        if writes {
            self.attribute(context, argument.place, path, AccessKind::Write);
        }
    }

    /// One direct call's contribution: known callees join through their
    /// parameter rows each fixpoint round; targets outside the unit apply
    /// each argument's declared access and mark the external surface a lower
    /// bound.
    fn push_call(
        &mut self,
        context: &FunctionContext<'_>,
        machines: &BTreeSet<MachineId>,
        callee: MachineId,
        structural_arguments: &[StructuralArgument],
    ) {
        if machines.contains(&callee) {
            self.contributions.push(PendingContribution::Call {
                callee,
                arguments: structural_arguments.to_vec(),
            });
        } else {
            for argument in structural_arguments {
                let (reads, writes) = declared_access(argument.access);
                self.attribute_argument(context, argument, reads, writes);
            }
            self.unsummarized = true;
        }
    }

    /// A reach-only edge: the unit machine's external surface and
    /// unsummarized flag propagate; its parameter lane cannot be projected
    /// without a structural argument row.
    fn push_reach(&mut self, machines: &BTreeSet<MachineId>, callee: MachineId) {
        if machines.contains(&callee) {
            self.contributions
                .push(PendingContribution::Reach { callee });
        } else {
            self.unsummarized = true;
        }
    }

    /// One application's callee set: every realization callable the closed
    /// conformance names may run behind the descriptor.
    fn push_application(
        &mut self,
        machines: &BTreeSet<MachineId>,
        application: &ClosedConformanceApplication,
    ) {
        for callable in &application.realization_callables {
            self.push_reach(machines, callable.machine);
        }
    }

    /// One argument of a dynamic-dispatch lane: the selected source view is
    /// touched up to its declared access, and every statically named
    /// realization joins by reach.
    fn push_dynamic_source(
        &mut self,
        context: &FunctionContext<'_>,
        machines: &BTreeSet<MachineId>,
        source: &AbstractDynamicDescriptorSource,
    ) {
        match source {
            AbstractDynamicDescriptorSource::Selection {
                selection,
                application,
            } => {
                let (reads, writes) = declared_access(selection.source.access);
                self.attribute_argument(context, &selection.source, reads, writes);
                self.push_application(machines, application);
            }
            AbstractDynamicDescriptorSource::Rebound {
                initial,
                rebound,
                initial_application,
                application,
                ..
            } => {
                let (reads, writes) = declared_access(initial.source.access);
                self.attribute_argument(context, &initial.source, reads, writes);
                let (reads, writes) = declared_access(rebound.source.access);
                self.attribute_argument(context, &rebound.source, reads, writes);
                self.push_application(machines, initial_application);
                self.push_application(machines, application);
            }
            AbstractDynamicDescriptorSource::Parameter(_) => {
                // A parameter-shaped descriptor names a formal lane already
                // bounded by the caller's own declared access; the dispatch
                // itself is what stays unsummarized.
            }
        }
    }
}

/// One node's local contribution. View-producing results were already
/// folded into the `FunctionContext` alias and binding maps; every remaining
/// operand contributes its exact accessed view here.
fn scan_operation(
    context: &FunctionContext<'_>,
    machines: &BTreeSet<MachineId>,
    pending: &mut PendingSurface,
    operation: &O,
) {
    match operation {
        O::PrimitiveScalarRead { source, path, .. } => {
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
        }
        O::StructuralScalarFieldRead {
            source,
            path,
            field,
            ..
        } => {
            let mut view = canonical_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, *source, view, AccessKind::Read);
        }
        O::StructuralCaseMembership { source, path, .. } => {
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
        }
        O::StructuralCase { source, cases, .. } => {
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            for case in cases {
                for place in &case.trivial_affine_discards {
                    pending.attribute(context, *place, Vec::new(), AccessKind::Write);
                }
            }
        }
        O::StructuralLeafCopy { source, path, .. } => {
            let view = match path {
                StructuralCopyPath::Ordinary(path) => runtime_prefix(path),
                StructuralCopyPath::Case(path) => canonical_prefix(path),
            };
            pending.attribute(context, *source, view, AccessKind::Read);
        }
        O::ByteSequenceRead { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::ElementViewLength { source, .. } => {
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        O::ByteSequencePredicate { operand, .. } => match operand {
            ByteSequenceOperand::Whole { source, .. } => {
                pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            }
            ByteSequenceOperand::Field { source, field, .. } => {
                let mut view = runtime_prefix(&source.path);
                with_field(&mut view, *field);
                pending.attribute(context, source.place, view, AccessKind::Read);
            }
        },
        O::ElementViewRead { source, path, .. } => {
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
        }
        O::ElementViewWrite { destination, .. } => {
            pending.attribute(context, *destination, Vec::new(), AccessKind::Write);
        }
        O::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        }
        | O::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
            ..
        } => {
            let mut view = runtime_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, *source, view, AccessKind::Read);
        }
        O::PrimitiveLocalStore { destination, .. } | O::ByteSequenceWrite { destination, .. } => {
            pending.attribute(context, *destination, Vec::new(), AccessKind::Write);
        }
        O::ByteSequenceStore {
            destination,
            source,
            ..
        } => {
            pending.attribute(context, *destination, Vec::new(), AccessKind::Write);
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        O::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            pending.attribute(
                context,
                destination.place(),
                runtime_prefix(path),
                AccessKind::Write,
            );
        }
        O::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => {
            let mut view = runtime_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, destination.place(), view, AccessKind::Write);
        }
        O::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        }
        | O::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => {
            let mut view = runtime_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, *destination, view, AccessKind::Write);
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        O::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => {
            let mut view = runtime_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, *destination, view, AccessKind::Write);
        }
        // A root window (`field: None`) writes the whole referent.
        O::MoveStructuralField {
            source,
            path,
            field,
            ..
        } => {
            let mut view = runtime_prefix(path);
            if let Some(field) = field {
                with_field(&mut view, *field);
            }
            pending.attribute(context, source.place(), view, AccessKind::Write);
        }
        O::StoreStructuralField {
            destination,
            path,
            field,
            value,
            ..
        } => {
            let mut view = runtime_prefix(path);
            if let Some(field) = field {
                with_field(&mut view, *field);
            }
            pending.attribute(context, destination.place(), view, AccessKind::Write);
            let (reads, writes) = declared_access(value.access);
            pending.attribute_argument(context, value, reads, writes);
        }
        O::StoreDynamicDescriptor { stored, .. } => {
            // The stored row captures the instance through the descriptor's
            // declared access; the aggregate field it lands in is outside
            // the structural place vocabulary.
            let (reads, writes) = declared_access(stored.selection.source.access);
            pending.attribute_argument(context, &stored.selection.source, reads, writes);
        }
        O::EstablishByteSequenceConcatenation { left, right, .. }
        | O::ByteSequenceEqual { left, right, .. } => {
            for operand in [left, right] {
                match operand {
                    ByteSequenceOperand::Whole { source, .. } => {
                        pending.attribute(context, *source, Vec::new(), AccessKind::Read);
                    }
                    ByteSequenceOperand::Field { source, field, .. } => {
                        let mut view = runtime_prefix(&source.path);
                        with_field(&mut view, *field);
                        pending.attribute(context, source.place, view, AccessKind::Read);
                    }
                }
            }
        }
        O::EstablishStructuralArray { elements, .. } => {
            for argument in elements {
                let (reads, writes) = declared_access(argument.access);
                pending.attribute_argument(context, argument, reads, writes);
            }
        }
        O::EstablishRecord { fields, .. } | O::EstablishCase { fields, .. } => {
            for field in fields {
                if let RecordFieldValue::Structural(argument) = &field.value {
                    let (reads, writes) = declared_access(argument.access);
                    pending.attribute_argument(context, argument, reads, writes);
                }
            }
        }
        O::ReleaseReference { source, .. } => {
            // Releasing the carrier vacates the caller-visible cell.
            pending.attribute(context, *source, Vec::new(), AccessKind::Write);
        }
        O::AtomicEvent { event, .. } => {
            if let Some(location) = event.location() {
                let mut view = runtime_prefix(&location.path);
                with_field(&mut view, location.field);
                let accesses: &[AccessKind] = match event {
                    AbstractAtomicEvent::Load { .. } => &[AccessKind::Read],
                    AbstractAtomicEvent::Store { .. } => &[AccessKind::Write],
                    AbstractAtomicEvent::Fence { .. } => &[],
                    _ => &[AccessKind::Read, AccessKind::Write],
                };
                for kind in accesses {
                    pending.attribute(context, location.root, view.clone(), *kind);
                }
            }
            if let AbstractAtomicEvent::CompareExchangeOnce { outcome, .. } = event {
                pending.attribute(context, outcome.place, Vec::new(), AccessKind::Write);
            }
        }
        O::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            for place in trivial_affine_discards {
                pending.attribute(context, *place, Vec::new(), AccessKind::Write);
            }
            for discard in residual_affine_discards {
                pending.attribute(
                    context,
                    discard.place,
                    runtime_prefix(&discard.path),
                    AccessKind::Write,
                );
            }
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for successor in [when_true, when_false] {
                for place in &successor.trivial_affine_discards {
                    pending.attribute(context, *place, Vec::new(), AccessKind::Write);
                }
            }
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for action in cleanup_actions {
                match action {
                    TerminalAffineCleanupAction::DiscardRoot(place) => {
                        pending.attribute(context, *place, Vec::new(), AccessKind::Write);
                    }
                    TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        pending.attribute(
                            context,
                            discard.place,
                            runtime_prefix(&discard.path),
                            AccessKind::Write,
                        );
                    }
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        // The hook consumes the passed cell; anything else it
                        // reaches joins through the reach edge.
                        pending.attribute(context, cleanup.place, Vec::new(), AccessKind::Write);
                        pending.push_reach(machines, cleanup.cleanup_machine);
                    }
                }
            }
        }
        O::ReturnStructural {
            source,
            trivial_affine_discards,
            ..
        } => {
            // The result handoff observes the returned storage; what the
            // result carries is declared in the signature, not here.
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
            for place in trivial_affine_discards {
                pending.attribute(context, *place, Vec::new(), AccessKind::Write);
            }
        }
        O::CallStructural {
            callee,
            structural_arguments,
            dynamic_arguments,
            ..
        } if !dynamic_arguments.is_empty() => {
            pending.push_call(context, machines, *callee, structural_arguments);
            for argument in dynamic_arguments {
                pending.push_dynamic_source(context, machines, &argument.source);
            }
            // The forwarded descriptor lane itself is not summarized.
            pending.unsummarized = true;
        }
        O::CallStructural {
            callee,
            structural_arguments,
            ..
        } => {
            pending.push_call(context, machines, *callee, structural_arguments);
        }
        O::Call { callee, .. } => {
            pending.push_call(context, machines, *callee, &[]);
        }
        O::CallDynamic { dispatch, .. } => {
            match dispatch {
                AbstractDynamicDispatch::Rebound(dispatch) => {
                    let (reads, writes) = declared_access(dispatch.initial.source.access);
                    pending.attribute_argument(context, &dispatch.initial.source, reads, writes);
                    let (reads, writes) = declared_access(dispatch.rebound.source.access);
                    pending.attribute_argument(context, &dispatch.rebound.source, reads, writes);
                    pending.push_application(machines, &dispatch.initial_application);
                    pending.push_application(machines, &dispatch.application);
                    pending.push_reach(machines, dispatch.dispatch.realization);
                }
                AbstractDynamicDispatch::Stored(dispatch) => {
                    let (reads, writes) = declared_access(dispatch.stored.selection.source.access);
                    pending.attribute_argument(
                        context,
                        &dispatch.stored.selection.source,
                        reads,
                        writes,
                    );
                    pending.push_application(machines, &dispatch.stored.application);
                    pending.push_reach(machines, dispatch.dispatch.realization);
                }
                AbstractDynamicDispatch::Parameter(_) => {}
            }
            pending.unsummarized = true;
        }
        O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                let (reads, writes) = declared_access(argument.access);
                pending.attribute_argument(context, argument, reads, writes);
            }
            pending.unsummarized = true;
        }
        // Alias establishments contribute through the `FunctionContext`
        // alias map, and pure scalar or internal establishment operations
        // touch no caller-visible place at all.
        O::ByteSequenceSubslice { .. }
        | O::ElementViewSubslice { .. }
        | O::EstablishElementView { .. }
        | O::EstablishReference { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::DynamicDescriptorParameter { .. }
        | O::PortWrite { .. }
        | O::Crash { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        | O::FloatResultPolicy { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::ScalarRepresentationView { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerShift { .. }
        | O::IntegerArithmetic { .. }
        | O::TrappingInteger { .. } => {}
    }
}

fn scan_function(
    context: &FunctionContext<'_>,
    machines: &BTreeSet<MachineId>,
    function: &PsiOptimizationFunction,
) -> PendingSurface {
    let mut pending = PendingSurface::default();
    for block in function
        .blocks
        .iter()
        .filter(|block| context.reachable.contains(&block.id))
    {
        for node in &block.nodes {
            pending.support.extend(node.provenance.iter().copied());
            pending.support.extend(
                node.successors
                    .iter()
                    .flat_map(|edge| edge.provenance.iter().copied()),
            );
            scan_operation(context, machines, &mut pending, &node.operation);
        }
    }
    pending
}

/// Compute every function's transitive place surface. The join converges
/// because every contribution inserts only the lent argument view beneath a
/// root — never a callee path composed onto it — so recursive components
/// cannot manufacture deeper rows.
pub(in crate::analyses) fn place_access_summaries(
    unit: &PsiOptimizationUnit,
) -> PlaceAccessSummaryAnalysis {
    let machines: BTreeSet<MachineId> = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    let mut contexts = BTreeMap::new();
    let mut pending = BTreeMap::<MachineId, PendingSurface>::new();
    for function in &unit.functions {
        contexts.insert(function.machine, FunctionContext::build(function));
    }
    for function in &unit.functions {
        pending.insert(
            function.machine,
            scan_function(&contexts[&function.machine], &machines, function),
        );
    }

    loop {
        let prior = pending.clone();
        let mut changed = false;
        for (machine, summary) in pending.iter_mut() {
            let context = &contexts[machine];
            for contribution in summary.contributions.clone() {
                match contribution {
                    PendingContribution::Call { callee, arguments } => {
                        let Some(callee_summary) = prior.get(&callee) else {
                            continue;
                        };
                        let touched: BTreeSet<u32> = callee_summary
                            .parameter_reads
                            .keys()
                            .chain(callee_summary.parameter_writes.keys())
                            .copied()
                            .collect();
                        for position in touched {
                            let index =
                                usize::try_from(position).expect("parameter position is usize");
                            let Some(argument) = arguments.get(index) else {
                                // A callee parameter no argument covers
                                // exists only past verified shapes — the
                                // call itself already fails admission.
                                continue;
                            };
                            let path = runtime_prefix(&argument.path);
                            if callee_summary
                                .parameter_reads
                                .get(&position)
                                .is_some_and(|reads| !reads.is_empty())
                            {
                                summary.attribute(
                                    context,
                                    argument.place,
                                    path.clone(),
                                    AccessKind::Read,
                                );
                            }
                            if callee_summary
                                .parameter_writes
                                .get(&position)
                                .is_some_and(|writes| !writes.is_empty())
                            {
                                summary.attribute(context, argument.place, path, AccessKind::Write);
                            }
                        }
                        // Arguments past the callee's signature keep only
                        // their declared bound.
                        let parameter_count = contexts
                            .get(&callee)
                            .map(|callee_context| {
                                callee_context.function.structural_parameters.len()
                            })
                            .unwrap_or_default();
                        for argument in arguments.iter().skip(parameter_count) {
                            let (reads, writes) = declared_access(argument.access);
                            summary.attribute_argument(context, argument, reads, writes);
                        }
                        summary
                            .external_reads
                            .extend(callee_summary.external_reads.iter().cloned());
                        summary
                            .external_writes
                            .extend(callee_summary.external_writes.iter().cloned());
                        summary.unsummarized |= callee_summary.unsummarized;
                    }
                    PendingContribution::Reach { callee } => {
                        let Some(callee_summary) = prior.get(&callee) else {
                            continue;
                        };
                        summary
                            .external_reads
                            .extend(callee_summary.external_reads.iter().cloned());
                        summary
                            .external_writes
                            .extend(callee_summary.external_writes.iter().cloned());
                        summary.unsummarized |= callee_summary.unsummarized;
                    }
                }
            }
            changed |= *summary != *prior.get(machine).expect("pending row persists");
        }
        if !changed {
            break;
        }
    }

    let mut functions = Vec::with_capacity(pending.len());
    for (machine, summary) in pending {
        let context = &contexts[&machine];
        let mut parameters = Vec::with_capacity(context.function.structural_parameters.len());
        for parameter in &context.function.structural_parameters {
            parameters.push(ParameterAccessSurface {
                position: parameter.position,
                place: parameter.place,
                declared: parameter.access,
                reads: summary
                    .parameter_reads
                    .get(&parameter.position)
                    .map(|paths| paths.iter().cloned().collect())
                    .unwrap_or_default(),
                writes: summary
                    .parameter_writes
                    .get(&parameter.position)
                    .map(|paths| paths.iter().cloned().collect())
                    .unwrap_or_default(),
            });
        }
        parameters.sort_by_key(|surface| surface.position);
        // The result's declared reference leaves resolve through this
        // function's own aliases and bindings once, at compute time: every
        // call site then projects the callee-namespace origins through its
        // own argument row instead of re-chasing the callee's locals.
        let result_sources = context
            .function
            .result
            .structural()
            .map(|result| {
                result
                    .reference_sources
                    .iter()
                    .map(|source| ResolvedResultSource {
                        result_path: source.path.clone(),
                        origins: source
                            .sources
                            .iter()
                            .flat_map(|candidate| {
                                context.resolve(candidate.place, runtime_prefix(&candidate.path))
                            })
                            .map(|resolved| match resolved {
                                ResolvedAccess::Parameter { position, path } => {
                                    ResultSourceResolution::Parameter { position, path }
                                }
                                ResolvedAccess::External { root, path } => {
                                    ResultSourceResolution::External { root, path }
                                }
                                ResolvedAccess::Internal => ResultSourceResolution::Internal,
                            })
                            .fold(Vec::new(), |mut origins, origin| {
                                if !origins.contains(&origin) {
                                    origins.push(origin);
                                }
                                origins
                            }),
                    })
                    .collect()
            })
            .unwrap_or_default();
        functions.push(FunctionPlaceAccessSummary {
            machine,
            parameters,
            external_reads: summary.external_reads.into_iter().collect(),
            external_writes: summary.external_writes.into_iter().collect(),
            result_sources,
            unsummarized_targets: summary.unsummarized,
            support: summary.support.into_iter().collect(),
            revision: unit.identity,
        });
    }
    functions.sort_by_key(|function| function.machine);
    PlaceAccessSummaryAnalysis { functions }
}
