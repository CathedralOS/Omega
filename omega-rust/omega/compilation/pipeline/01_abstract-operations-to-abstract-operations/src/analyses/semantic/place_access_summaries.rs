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
    AbstractAtomicEvent, AbstractOperation as O, AbstractStructuralBinding, StructuralCopyPath,
};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, MachineId, PlaceId, StructuralFieldId,
    StructuralPlaceKind,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, TerminalAffineCleanupAction,
    control_flow::ByteSequenceOperand,
};

use super::semantically_reachable_blocks;

/// One projection step inside a recorded access view. Runtime spellings keep
/// the operation's own `StructuralPathSegment`; verifier-keyed spellings —
/// canonical paths and the terminal `field` of a field operation — keep
/// `CanonicalStructuralPathSegment` so structural-field identity survives
/// without being flattened into a display string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PlaceAccessStep {
    Runtime(StructuralPathSegment),
    Canonical(CanonicalStructuralPathSegment),
    /// A returned-origin family, never an executable wildcard operand.
    Element,
}

impl PlaceAccessStep {
    fn is_referent(&self) -> bool {
        matches!(self, Self::Runtime(StructuralPathSegment::Referent))
    }
}

/// One may-access view: storage at `root`, a place declared by `owner`,
/// beneath `path`. An empty path is the whole root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PlaceAccessView {
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
pub(crate) struct ParameterAccessSurface {
    pub position: u32,
    pub place: PlaceId,
    pub declared: StructuralAccess,
    pub reads: Vec<Vec<PlaceAccessStep>>,
    pub writes: Vec<Vec<PlaceAccessStep>>,
}

/// One declared result reference leaf's resolved ingress, in the callee's
/// namespace: where the reference the result carries at `result_path` points,
/// resolved through this function's own view aliases and block bindings. The
/// verifier's `reference_sources` rows spell callee-local places; this
/// resolution is what a caller cannot read off the signature — whether the
/// source is a formal parameter lane it lent, or storage outside the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedResultSource {
    pub result_path: Vec<StructuralPathSegment>,
    pub origins: Vec<ResultSourceResolution>,
}

/// Where one resolved result-source view lives, in the callee's namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResultSourceResolution {
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
pub(crate) struct FunctionPlaceAccessSummary {
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
    /// carry or a boundary. Parameter surfaces
    /// stay exact — an unsummarized target's argument lane is bounded by
    /// each argument's declared `access` — but external rows are then a
    /// lower bound, not the complete reach.
    pub unsummarized_targets: bool,
    pub support: Vec<PsiProvenance>,
    pub revision: OptimizationUnitIdentity,
}

/// The place-access fact product for one unit revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceAccessSummaryAnalysis {
    pub functions: Vec<FunctionPlaceAccessSummary>,
}

/// The access a declared [`StructuralAccess`] authorizes: `(reads, writes)`.
/// This is the complete bound applied to argument lanes whose callee the
/// unit cannot observe. `Owned` counts as both — consumption observes the
/// content and vacates the caller's custody.
pub(crate) fn declared_access(access: StructuralAccess) -> (bool, bool) {
    match access {
        StructuralAccess::SharedBorrow => (true, false),
        terminal_psi::mutating_access_pair_pattern!() => (true, true),
        StructuralAccess::WriteOnlyBorrow => (false, true),
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
                    abstract_operations::element_view_or_byte_subslice_pattern!(
                        result,
                        source,
                        ..
                    )
                    | O::EstablishElementView { result, source, .. } => {
                        aliases.insert(
                            result.place,
                            AliasTarget::View {
                                target: source.place,
                                prefix: runtime_prefix(&source.path),
                            },
                        );
                    }
                    O::EstablishReference { result, source, .. }
                    | O::EstablishPlacedReference { result, source, .. } => {
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
    /// the argument lane — cleanup hooks.
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
    // Effect summaries include every possible selected edge, while execution
    // still owns precisely one roster. A hook's external reach must not vanish
    // merely because it precedes a successor rather than a return.
    for (_, actions) in operation.cleanup_rosters() {
        for action in actions {
            match action {
                TerminalAffineCleanupAction::RetireHome(_) => {}
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
                    pending.attribute(context, cleanup.place, Vec::new(), AccessKind::Write);
                    pending.push_reach(machines, cleanup.cleanup_machine);
                }
            }
        }
    }
    match operation {
        O::RegisterAccess { .. } => {
            // Register writes may change memory interpretation. They are not
            // ordinary accesses to one place that an alias summary can narrow.
            pending.unsummarized = true;
        }
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
        O::StructuralCase { source, path, .. } => {
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
        }
        O::StructuralLeafCopy { source, path, .. } => {
            let view = match path {
                StructuralCopyPath::Ordinary(path) => runtime_prefix(path),
                StructuralCopyPath::Case(path) => canonical_prefix(path),
            };
            pending.attribute(context, *source, view, AccessKind::Read);
        }
        O::ByteSequenceRead { source, .. } => {
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        O::ByteSequencePredicate { operand, .. } => match operand {
            ByteSequenceOperand::Whole { source, .. } => {
                pending.attribute_argument(context, source, true, false);
            }
            ByteSequenceOperand::Field { source, field, .. } => {
                let mut view = runtime_prefix(&source.path);
                with_field(&mut view, *field);
                pending.attribute(context, source.place, view, AccessKind::Read);
            }
        },
        O::ElementViewLength { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ElementViewSubslice { source, .. } => {
            pending.attribute(
                context,
                source.place,
                runtime_prefix(&source.path),
                AccessKind::Read,
            );
        }
        O::ElementViewRead { source, path, .. } => {
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Read);
        }
        O::ElementViewWrite { destination, .. } => {
            pending.attribute(
                context,
                destination.place,
                runtime_prefix(&destination.path),
                AccessKind::Write,
            );
        }
        abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
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
        } => {
            let mut view = runtime_prefix(path);
            with_field(&mut view, *field);
            pending.attribute(context, *destination, view, AccessKind::Write);
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        // A descriptor store without a field writes the whole slot.
        O::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => {
            let mut view = runtime_prefix(path);
            if let Some(field) = field {
                with_field(&mut view, *field);
            }
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
        O::EstablishByteSequenceConcatenation { left, right, .. }
        | O::ByteSequenceEqual { left, right, .. } => {
            for operand in [left, right] {
                match operand {
                    ByteSequenceOperand::Whole { source, .. } => {
                        pending.attribute_argument(context, source, true, false);
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
        abstract_operations::establish_record_or_case_pattern!(fields, ..) => {
            for field in fields {
                if let Some(argument) = field.value.as_structural() {
                    let (reads, writes) = declared_access(argument.access);
                    pending.attribute_argument(context, argument, reads, writes);
                }
            }
        }
        O::ReleaseReference { source, path, .. } => {
            // Release consumes only the selected descriptor cell.
            pending.attribute(context, *source, runtime_prefix(path), AccessKind::Write);
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
        O::ReturnStructural { source, .. } => {
            pending.attribute(context, *source, Vec::new(), AccessKind::Read);
        }
        abstract_operations::jump_or_conditional_pattern!()
        | abstract_operations::return_or_return_unit_pattern!() => {}
        O::CallStructural {
            callee,
            structural_arguments,
            ..
        } => {
            if let Some(machine) = callee.machine() {
                pending.push_call(context, machines, machine, structural_arguments);
            } else {
                // The slot grants each argument's access; it supplies no
                // representative body's narrower mutation summary.
                for argument in structural_arguments {
                    let (reads, writes) = declared_access(argument.access);
                    pending.attribute_argument(context, argument, reads, writes);
                }
                pending.unsummarized = true;
            }
        }
        O::Call { callee, .. } => {
            pending.push_call(context, machines, *callee, &[]);
        }
        O::BoundaryCall {
            structural_arguments,
            ..
        }
        | O::ExternalCompletion {
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
        abstract_operations::element_view_or_byte_subslice_pattern!(..)
        | O::EstablishElementView { .. }
        | O::EstablishReference { .. }
        | O::EstablishPlacedReference { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishPrimitiveLocal { .. }
        | abstract_operations::byte_literal_establish_pair_pattern!(..)
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::PortWrite { .. }
        | O::Crash { .. }
        | O::SelectImplementation { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        | O::FloatResultPolicy { .. }
        | O::BooleanConstant { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
        | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
        | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | abstract_operations::integer_dispatch_triple_pattern!(..) => {}
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
                                let terminal_psi::StructuralReferenceOrigin::Parameter(candidate) =
                                    candidate
                                else {
                                    return vec![ResolvedAccess::External {
                                        root: result.place,
                                        path: Vec::new(),
                                    }];
                                };
                                context.resolve(
                                    candidate.place,
                                    candidate
                                        .path
                                        .iter()
                                        .map(|projection| match projection {
                                            terminal_psi::ReferenceOriginProjection::Exact(
                                                segment,
                                            ) => PlaceAccessStep::Runtime(segment.clone()),
                                            terminal_psi::ReferenceOriginProjection::Element => {
                                                PlaceAccessStep::Element
                                            }
                                        })
                                        .collect(),
                                )
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
