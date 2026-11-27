//! Analyses built once from one immutable module and shared by every check
//! that asks about it.
//!
//! Each descriptor closure used to rebuild its inputs: it re-sorted the
//! machine list, rebuilt the zeroed-case table and adapted every reached body
//! again, even when the previous closure had adapted the same bodies.
//! `ModuleAnalyses` builds the machine lookup and type index once, and adapts
//! each machine's descriptor row at most once per slot kind. Later closures
//! over the same module reuse those rows.
//!
//! The borrow `&'m TerminalModule` stands for the module revision. While a
//! bundle exists nothing can edit the module, so a shared row always describes
//! the current body. The bundle shares only data derived from the module,
//! never a verdict, under two rules:
//! - A closure that reaches a body with a dynamic call adapts each row against
//!   that closure's own rows, so the rows can differ from closure to closure.
//!   `descriptor_closure` returns those rows as `DescriptorRows::Owned` and
//!   never stores them.
//! - A row that fails keeps its own error. A closure that reaches it is
//!   refused, while closures that do not reach it still share the other rows.
//!
//! Rows shared through a family use the default inputs: no selected boundary
//! calls and no generic-call types. Rows for other inputs are adapted for the
//! one closure that asks, as before.
//!
//! The bundle also keeps the borrowed-storage replay summaries of its
//! machines (`ModuleSummaries`). An analysis the bundle builds over rows its
//! borrowed-storage family shares takes callee summaries other analyses of
//! the bundle completed, under the adoption rules of `Replay::adopted_summary`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use semantic_vocabulary::{MachineId, PlaceId, StructuralCaseId, StructuralTypeId};
use terminal_psi::{OperationKind, StructuralTypeDeclaration, TerminalMachine, TerminalModule};

use crate::descriptors::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, StructuralTypeIndex,
};
use crate::descriptors::descriptor_results::{
    DescriptorAnalysis, DescriptorCasePredicate, DescriptorGraphError, DescriptorInvocationTarget,
    DescriptorMachine, DescriptorSlotKind, GenericCallInputs, ModuleSummaries, terminal_flow,
};

#[cfg(test)]
mod tests;

type RowResult = Result<DescriptorMachine, DescriptorGraphError>;
type BoundaryCall = (semantic_vocabulary::BoundaryMachineId, MachineId);

const SLOT_KINDS: usize = 4;

fn kind_index(kind: DescriptorSlotKind) -> usize {
    match kind {
        DescriptorSlotKind::Views => 0,
        DescriptorSlotKind::References => 1,
        DescriptorSlotKind::BorrowedStorage => 2,
        DescriptorSlotKind::AccessOrigins => 3,
    }
}

fn body_calls_dynamically(body: &TerminalMachine) -> bool {
    body.operations()
        .any(|operation| matches!(operation.kind, OperationKind::CallDynamic { .. }))
}

fn push_callees(row: &DescriptorMachine, pending: &mut Vec<MachineId>) {
    for (_, call) in row.known_invocations() {
        match &call.target {
            DescriptorInvocationTarget::Machine(callee) => pending.push(*callee),
            DescriptorInvocationTarget::Public {
                effects_machine,
                realizations,
                ..
            } => {
                pending.extend(effects_machine);
                pending.extend(realizations);
            }
            DescriptorInvocationTarget::Provider { .. } => {}
        }
    }
}

/// Finds a machine's position in `module.machines` by its identity.
///
/// Saved modules can use sparse identities, so the lookup is a sorted list
/// rather than a table sized to the largest identity. A module that declares
/// one identity twice has no answer for it, so construction refuses it.
pub struct MachineSlots {
    /// Sorted by machine. The second field is the machine's position in
    /// `module.machines`.
    by_id: Box<[(MachineId, usize)]>,
}

impl MachineSlots {
    pub fn new(module: &TerminalModule) -> Result<Self, DescriptorGraphError> {
        let mut by_id: Box<[(MachineId, usize)]> = module
            .machines
            .iter()
            .enumerate()
            .map(|(slot, machine)| (machine.id, slot))
            .collect();
        by_id.sort_unstable();
        if let Some(pair) = by_id.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(DescriptorGraphError::MissingOrAmbiguousMachine(pair[1].0));
        }
        Ok(Self { by_id })
    }

    /// The machine's position in `module.machines`.
    pub fn slot(&self, machine: MachineId) -> Option<usize> {
        let index = self
            .by_id
            .binary_search_by_key(&machine, |(id, _)| *id)
            .ok()?;
        Some(self.by_id[index].1)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

type ZeroedCaseRows = Vec<(Vec<DescriptorPathSegment>, StructuralCaseId)>;

/// The case each zeroed sum starts in, resolved once per structural type.
///
/// The resolved paths do not depend on the root place, so the first request
/// for a type fills its row and every later root reuses it. Rows fill through
/// `OnceLock`, so threads that share the bundle may ask concurrently.
pub(crate) struct ZeroedCases<'m> {
    types: StructuralTypeIndex<'m>,
    by_position: Box<[OnceLock<ZeroedCaseRows>]>,
}

impl<'m> ZeroedCases<'m> {
    pub(crate) fn new(types: &'m [StructuralTypeDeclaration]) -> Self {
        let types = StructuralTypeIndex::new(types);
        let by_position = (0..types.len()).map(|_| OnceLock::new()).collect();
        Self { types, by_position }
    }

    pub(crate) fn types(&self) -> &StructuralTypeIndex<'m> {
        &self.types
    }

    pub(crate) fn predicates_for(
        &self,
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Vec<DescriptorCasePredicate> {
        let Some(row) = self
            .types
            .position(structural_type)
            .and_then(|position| self.by_position.get(position))
        else {
            return Vec::new();
        };
        row.get_or_init(|| self.selected_paths(root, structural_type))
            .iter()
            .map(|(path, case)| DescriptorCasePredicate {
                subject: DescriptorLocation {
                    root,
                    path: path.clone(),
                }
                .into(),
                case: *case,
            })
            .collect()
    }

    fn selected_paths(&self, root: PlaceId, structural_type: StructuralTypeId) -> ZeroedCaseRows {
        let Some(selections) = crate::structural::zeroed_structural::zeroed_sum_selections_indexed(
            &self.types,
            structural_type,
        ) else {
            return Vec::new();
        };
        let mut resolved = Vec::with_capacity(selections.len());
        for (path, case) in selections {
            let Some(location) = self
                .types
                .descriptor_location(root, structural_type, &path, None)
            else {
                return Vec::new();
            };
            resolved.push((location.path, case));
        }
        resolved
    }
}

/// Names one row family for the life of the process, so a replay summary
/// table can tell rows of its own family from rows built anywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowFamilyIdentity(u64);

impl RowFamilyIdentity {
    fn next() -> Self {
        static NEXT_FAMILY: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_FAMILY.fetch_add(1, Ordering::Relaxed))
    }
}

/// The descriptor rows of one slot kind, one per machine, each adapted on
/// first use with the default inputs.
pub struct DescriptorRowFamily {
    identity: RowFamilyIdentity,
    by_slot: Box<[OnceLock<RowResult>]>,
    /// Every machine's row adapted against the whole module's rows. Filled
    /// only for a module whose rows include a body with a dynamic call.
    module_rows: OnceLock<Box<[RowResult]>>,
}

impl DescriptorRowFamily {
    fn new(machines: usize) -> Self {
        Self {
            identity: RowFamilyIdentity::next(),
            by_slot: (0..machines).map(|_| OnceLock::new()).collect(),
            module_rows: OnceLock::new(),
        }
    }

    /// The row at `slot` when it has been adapted and succeeded.
    fn adapted(&self, slot: usize) -> Option<&DescriptorMachine> {
        self.by_slot.get(slot)?.get()?.as_ref().ok()
    }
}

/// The descriptor rows of one closure.
pub enum DescriptorRows<'a> {
    /// Members of a bundle's row family, in module order. Every member's row
    /// was adapted and succeeded before this value was built.
    Shared {
        family: &'a DescriptorRowFamily,
        machine_slots: &'a MachineSlots,
        members: Vec<usize>,
    },
    /// Rows built for this closure alone, such as a closure with a dynamic
    /// call.
    Owned(Vec<DescriptorMachine>),
    /// Rows a caller built without a bundle.
    Borrowed(&'a [DescriptorMachine]),
}

impl<'a> DescriptorRows<'a> {
    pub fn view(&self) -> DescriptorRowView<'_> {
        self.into()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DescriptorMachine> {
        self.view().iter()
    }

    /// The first row for `machine`, as `descriptor_machine` finds it.
    pub fn machine(&self, machine: MachineId) -> Option<&DescriptorMachine> {
        self.view().find(machine)
    }

    pub fn len(&self) -> usize {
        self.view().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<Vec<DescriptorMachine>> for DescriptorRows<'_> {
    fn from(rows: Vec<DescriptorMachine>) -> Self {
        Self::Owned(rows)
    }
}

impl<'a> From<&'a [DescriptorMachine]> for DescriptorRows<'a> {
    fn from(rows: &'a [DescriptorMachine]) -> Self {
        Self::Borrowed(rows)
    }
}

impl<'a> From<&'a Vec<DescriptorMachine>> for DescriptorRows<'a> {
    fn from(rows: &'a Vec<DescriptorMachine>) -> Self {
        Self::Borrowed(rows)
    }
}

/// A borrowed, copyable view of one closure's rows. Descriptor analyses read
/// their rows through it, whether a bundle shares them or a caller owns them.
#[derive(Clone, Copy)]
pub struct DescriptorRowView<'a> {
    rows: ViewRows<'a>,
}

#[derive(Clone, Copy)]
enum ViewRows<'a> {
    Slice(&'a [DescriptorMachine]),
    Shared {
        family: &'a DescriptorRowFamily,
        machine_slots: &'a MachineSlots,
        members: &'a [usize],
    },
}

impl<'a> DescriptorRowView<'a> {
    /// The rows in closure order.
    pub fn iter(self) -> impl Iterator<Item = &'a DescriptorMachine> + 'a {
        let (slice, shared) = match self.rows {
            ViewRows::Slice(rows) => (rows, None),
            ViewRows::Shared {
                family, members, ..
            } => (&[][..], Some((family, members))),
        };
        slice
            .iter()
            .chain(shared.into_iter().flat_map(|(family, members)| {
                members.iter().filter_map(move |slot| family.adapted(*slot))
            }))
    }

    pub fn len(self) -> usize {
        match self.rows {
            ViewRows::Slice(rows) => rows.len(),
            ViewRows::Shared { members, .. } => members.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// The caller's rows, when a caller passed them as a slice.
    pub(crate) fn as_slice(self) -> Option<&'a [DescriptorMachine]> {
        match self.rows {
            ViewRows::Slice(rows) => Some(rows),
            ViewRows::Shared { .. } => None,
        }
    }

    /// The bundle row family these rows come from, if a bundle shares them.
    pub(crate) fn family(self) -> Option<RowFamilyIdentity> {
        match self.rows {
            ViewRows::Slice(_) => None,
            ViewRows::Shared { family, .. } => Some(family.identity),
        }
    }

    /// The first row for `machine`, as `descriptor_machine` finds it.
    pub fn find(self, machine: MachineId) -> Option<&'a DescriptorMachine> {
        match self.rows {
            ViewRows::Slice(rows) => rows.iter().find(|row| row.id == machine),
            ViewRows::Shared { .. } => self.shared_member(machine),
        }
    }

    /// The row for `machine` when exactly one row has that identity.
    pub fn unique(self, machine: MachineId) -> Option<&'a DescriptorMachine> {
        match self.rows {
            ViewRows::Slice(rows) => {
                let mut matching = rows.iter().filter(|row| row.id == machine);
                let row = matching.next()?;
                matching.next().is_none().then_some(row)
            }
            ViewRows::Shared { .. } => self.shared_member(machine),
        }
    }

    /// Whether both views read the very same rows.
    pub fn same_rows(self, other: Self) -> bool {
        match (self.rows, other.rows) {
            (ViewRows::Slice(left), ViewRows::Slice(right)) => std::ptr::eq(left, right),
            (
                ViewRows::Shared {
                    family: left_family,
                    members: left_members,
                    ..
                },
                ViewRows::Shared {
                    family: right_family,
                    members: right_members,
                    ..
                },
            ) => {
                std::ptr::eq(left_family, right_family) && std::ptr::eq(left_members, right_members)
            }
            _ => false,
        }
    }

    fn shared_member(self, machine: MachineId) -> Option<&'a DescriptorMachine> {
        let ViewRows::Shared {
            family,
            machine_slots,
            members,
        } = self.rows
        else {
            return None;
        };
        let slot = machine_slots.slot(machine)?;
        members.binary_search(&slot).ok()?;
        family.adapted(slot)
    }
}

impl<'a> From<&'a [DescriptorMachine]> for DescriptorRowView<'a> {
    fn from(rows: &'a [DescriptorMachine]) -> Self {
        Self {
            rows: ViewRows::Slice(rows),
        }
    }
}

impl<'a> From<&'a Vec<DescriptorMachine>> for DescriptorRowView<'a> {
    fn from(rows: &'a Vec<DescriptorMachine>) -> Self {
        rows.as_slice().into()
    }
}

impl<'a, const N: usize> From<&'a [DescriptorMachine; N]> for DescriptorRowView<'a> {
    fn from(rows: &'a [DescriptorMachine; N]) -> Self {
        rows.as_slice().into()
    }
}

impl<'a> From<&'a DescriptorRows<'_>> for DescriptorRowView<'a> {
    fn from(rows: &'a DescriptorRows<'_>) -> Self {
        match rows {
            DescriptorRows::Shared {
                family,
                machine_slots,
                members,
            } => Self {
                rows: ViewRows::Shared {
                    family,
                    machine_slots,
                    members,
                },
            },
            DescriptorRows::Owned(rows) => rows.into(),
            DescriptorRows::Borrowed(rows) => (*rows).into(),
        }
    }
}

/// Machines one closure reached, as positions in `module.machines`, and the
/// call targets it named that the module does not declare.
struct ReachedMachines {
    /// Ascending, so in module order.
    slots: Vec<usize>,
    /// Ascending and without repeats.
    missing: Vec<MachineId>,
}

/// Everything derived from one immutable module that several checks read.
/// See the module documentation for what may be shared.
pub struct ModuleAnalyses<'m> {
    module: &'m TerminalModule,
    machine_slots: MachineSlots,
    /// The module's machines sorted by identity, as row adaptation reads them.
    roster: Box<[&'m TerminalMachine]>,
    zeroed_cases: ZeroedCases<'m>,
    /// Positions of bodies that contain a dynamic call, ascending.
    dynamic_slots: OnceLock<Box<[usize]>>,
    rows: [OnceLock<DescriptorRowFamily>; SLOT_KINDS],
    /// Borrowed-storage replay summaries, shared by the analyses this bundle
    /// builds over its borrowed-storage family. Built on the first such
    /// analysis.
    replays: OnceLock<ModuleSummaries<'m>>,
    /// Which calls may run while an owned value is out of borrowed storage,
    /// shared by every affine walk over the module. The tables fill on the
    /// first call inside an open window.
    pub(crate) fences: crate::plain_affine::CallFences<'m>,
    /// Whether the module's type permissions keep every nested ownership
    /// obligation, checked by the first affine walk that needs it.
    pub(crate) type_permissions: crate::plain_affine::TypePermissions,
}

impl<'m> ModuleAnalyses<'m> {
    /// Refuses a module that declares one machine identity twice.
    pub fn new(module: &'m TerminalModule) -> Result<Self, DescriptorGraphError> {
        let machine_slots = MachineSlots::new(module)?;
        let roster = machine_slots
            .by_id
            .iter()
            .map(|(_, slot)| &module.machines[*slot])
            .collect();
        Ok(Self {
            module,
            machine_slots,
            roster,
            zeroed_cases: ZeroedCases::new(&module.types.structural_types),
            dynamic_slots: OnceLock::new(),
            rows: Default::default(),
            replays: OnceLock::new(),
            fences: crate::plain_affine::CallFences::new(module),
            type_permissions: crate::plain_affine::TypePermissions::new(),
        })
    }

    pub fn module(&self) -> &'m TerminalModule {
        self.module
    }

    pub fn machine_slots(&self) -> &MachineSlots {
        &self.machine_slots
    }

    pub fn machine(&self, machine: MachineId) -> Option<&'m TerminalMachine> {
        let module = self.module;
        module.machines.get(self.machine_slots.slot(machine)?)
    }

    pub fn types(&self) -> &StructuralTypeIndex<'m> {
        self.zeroed_cases.types()
    }

    /// The affine walks of `machine`. They take the machine's borrowed-storage
    /// closure from this bundle's shared rows, unless the closure reaches a
    /// dynamic call, and share the bundle's call fences and type permissions.
    pub fn affine(
        &self,
        machine: MachineId,
    ) -> Result<crate::plain_affine::MachineAffine<'_>, crate::plain_affine::Error> {
        crate::plain_affine::MachineAffine::from_analyses(self, machine, None)
    }

    /// Whether the machine's body contains a dynamic call.
    pub fn calls_dynamically(&self, machine: MachineId) -> bool {
        self.machine_slots
            .slot(machine)
            .is_some_and(|slot| self.dynamic_slots().binary_search(&slot).is_ok())
    }

    /// The rows of every machine the roots reach, or the first failure among
    /// them in module order, then a call target the module does not declare.
    ///
    /// The rows are shared unless a reached body with a successful row calls
    /// dynamically. Then each row is adapted against this closure's rows and
    /// returned as `DescriptorRows::Owned`.
    pub fn descriptor_closure(
        &self,
        kind: DescriptorSlotKind,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Result<DescriptorRows<'_>, DescriptorGraphError> {
        let roots = roots.into_iter().collect::<Vec<_>>();
        let family = self.family(kind);
        let reached = self.reach(roots.iter().copied(), |slot, pending| {
            if let Ok(row) = self.shared_row(kind, family, slot) {
                push_callees(row, pending);
            }
        });
        let dynamic_slots = self.dynamic_slots();
        if reached.slots.iter().any(|slot| {
            family.adapted(*slot).is_some() && dynamic_slots.binary_search(slot).is_ok()
        }) {
            return self
                .closure_rows(kind, roots, &[], GenericCallInputs::default())
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .map(DescriptorRows::Owned);
        }
        if let Some(error) = reached
            .slots
            .iter()
            .find_map(|slot| self.shared_row(kind, family, *slot).as_ref().err())
        {
            return Err(*error);
        }
        if let Some(machine) = reached.missing.first() {
            return Err(DescriptorGraphError::MissingOrAmbiguousMachine(*machine));
        }
        Ok(DescriptorRows::Shared {
            family,
            machine_slots: &self.machine_slots,
            members: reached.slots,
        })
    }

    /// Every machine's row, each with its own failure.
    ///
    /// Rows come in module order. When a successful row's body calls
    /// dynamically, every successful row is adapted against the whole
    /// module's rows and the failed rows follow them, as
    /// `terminal_graph_rows` orders them.
    pub fn descriptor_rows(
        &self,
        kind: DescriptorSlotKind,
    ) -> impl Iterator<Item = Result<&DescriptorMachine, DescriptorGraphError>> + '_ {
        let family = self.family(kind);
        let machines = self.module.machines.len();
        let dynamic_slots = self.dynamic_slots();
        let dynamic = (0..machines).any(|slot| {
            self.shared_row(kind, family, slot).is_ok()
                && dynamic_slots.binary_search(&slot).is_ok()
        });
        let adapted = dynamic.then(|| {
            family.module_rows.get_or_init(|| {
                let rows = (0..machines)
                    .map(|slot| self.adapt(kind, slot, &[], GenericCallInputs::default(), None))
                    .collect();
                self.readapt_dynamic(kind, rows, &[], GenericCallInputs::default())
                    .into_boxed_slice()
            })
        });
        let shared = (!dynamic).then_some(0..machines);
        shared
            .into_iter()
            .flatten()
            .map(move |slot| {
                self.shared_row(kind, family, slot)
                    .as_ref()
                    .map_err(|error| *error)
            })
            .chain(
                adapted
                    .into_iter()
                    .flat_map(|rows| rows.iter().map(|row| row.as_ref().map_err(|error| *error))),
            )
    }

    /// A descriptor analysis of one closure's rows that reads this bundle's
    /// type index.
    ///
    /// A borrowed-storage analysis over rows this bundle's family shares
    /// takes callee summaries other such analyses completed and publishes
    /// its own. Rows the family does not share, such as the rows of a
    /// closure with a dynamic call, keep their summaries to themselves.
    pub fn analysis<'s>(
        &'s self,
        kind: DescriptorSlotKind,
        rows: &'s DescriptorRows<'_>,
    ) -> DescriptorAnalysis<'s> {
        let mut analysis = DescriptorAnalysis::with_type_index(
            &self.module.types.structural_types,
            self.types(),
            rows.view(),
            kind,
        );
        if kind == DescriptorSlotKind::BorrowedStorage {
            let replays = self.replays.get_or_init(|| {
                ModuleSummaries::for_family(self.module, self.family(kind).identity)
            });
            analysis.share_summaries(replays, &[]);
        }
        analysis
    }

    /// Rows of the closure of `roots` built for these inputs alone. Each
    /// failure stays in the list: reached rows come in module order, then a
    /// failure for each call target the module does not declare.
    pub(crate) fn closure_rows(
        &self,
        kind: DescriptorSlotKind,
        roots: impl IntoIterator<Item = MachineId>,
        selected_calls: &[BoundaryCall],
        generic_calls: GenericCallInputs<'_>,
    ) -> Vec<RowResult> {
        let mut adapted = Vec::new();
        let reached = self.reach(roots, |slot, pending| {
            let row = self.adapt(kind, slot, selected_calls, generic_calls, None);
            if let Ok(row) = &row {
                push_callees(row, pending);
            }
            adapted.push((slot, row));
        });
        adapted.sort_unstable_by_key(|(slot, _)| *slot);
        let rows = adapted
            .into_iter()
            .map(|(_, row)| row)
            .chain(
                reached
                    .missing
                    .into_iter()
                    .map(|machine| Err(DescriptorGraphError::MissingOrAmbiguousMachine(machine))),
            )
            .collect();
        self.readapt_dynamic(kind, rows, selected_calls, generic_calls)
    }

    /// Every machine's row for these inputs, or `None` when any row fails.
    pub(crate) fn module_rows(
        &self,
        kind: DescriptorSlotKind,
        selected_calls: &[BoundaryCall],
    ) -> Option<Vec<DescriptorMachine>> {
        let rows = (0..self.module.machines.len())
            .map(|slot| {
                self.adapt(
                    kind,
                    slot,
                    selected_calls,
                    GenericCallInputs::default(),
                    None,
                )
                .ok()
            })
            .collect::<Option<Vec<_>>>()?;
        self.readapt_dynamic(
            kind,
            rows.into_iter().map(Ok).collect(),
            selected_calls,
            GenericCallInputs::default(),
        )
        .into_iter()
        .map(Result::ok)
        .collect()
    }

    fn family(&self, kind: DescriptorSlotKind) -> &DescriptorRowFamily {
        self.rows[kind_index(kind)]
            .get_or_init(|| DescriptorRowFamily::new(self.module.machines.len()))
    }

    fn shared_row<'f>(
        &self,
        kind: DescriptorSlotKind,
        family: &'f DescriptorRowFamily,
        slot: usize,
    ) -> &'f RowResult {
        family.by_slot[slot]
            .get_or_init(|| self.adapt(kind, slot, &[], GenericCallInputs::default(), None))
    }

    fn dynamic_slots(&self) -> &[usize] {
        self.dynamic_slots.get_or_init(|| {
            self.module
                .machines
                .iter()
                .enumerate()
                .filter(|(_, body)| body_calls_dynamically(body))
                .map(|(slot, _)| slot)
                .collect()
        })
    }

    fn adapt(
        &self,
        kind: DescriptorSlotKind,
        slot: usize,
        selected_calls: &[BoundaryCall],
        generic_calls: GenericCallInputs<'_>,
        closure_rows: Option<&[DescriptorMachine]>,
    ) -> RowResult {
        terminal_flow::adapt(
            self.module,
            &self.roster,
            &self.module.machines[slot],
            kind,
            selected_calls,
            closure_rows,
            generic_calls,
            &self.zeroed_cases,
        )
    }

    /// Visits each declared machine the roots reach once, in call order.
    /// `visit` pushes the calls of the machine at that slot.
    fn reach(
        &self,
        roots: impl IntoIterator<Item = MachineId>,
        mut visit: impl FnMut(usize, &mut Vec<MachineId>),
    ) -> ReachedMachines {
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        let mut visited = vec![false; self.module.machines.len()];
        let mut reached = ReachedMachines {
            slots: Vec::new(),
            missing: Vec::new(),
        };
        while let Some(machine) = pending.pop() {
            let Some(slot) = self.machine_slots.slot(machine) else {
                reached.missing.push(machine);
                continue;
            };
            if std::mem::replace(&mut visited[slot], true) {
                continue;
            }
            reached.slots.push(slot);
            visit(slot, &mut pending);
        }
        reached.slots.sort_unstable();
        reached.missing.sort_unstable();
        reached.missing.dedup();
        reached
    }

    /// When a successful row's body calls dynamically, adapts every
    /// successful row again against those rows and lists the failures after
    /// them. Otherwise returns the rows unchanged.
    fn readapt_dynamic(
        &self,
        kind: DescriptorSlotKind,
        rows: Vec<RowResult>,
        selected_calls: &[BoundaryCall],
        generic_calls: GenericCallInputs<'_>,
    ) -> Vec<RowResult> {
        if !rows
            .iter()
            .filter_map(|row| row.as_ref().ok())
            .any(|row| self.machine(row.id).is_some_and(body_calls_dynamically))
        {
            return rows;
        }
        let mut initial = Vec::with_capacity(rows.len());
        let mut failures = Vec::new();
        for row in rows {
            match row {
                Ok(row) => initial.push(row),
                Err(error) => failures.push(Err(error)),
            }
        }
        let mut rows = Vec::with_capacity(initial.len() + failures.len());
        for row in &initial {
            let Some(slot) = self.machine_slots.slot(row.id) else {
                rows.push(Err(DescriptorGraphError::MissingOrAmbiguousMachine(row.id)));
                continue;
            };
            rows.push(self.adapt(kind, slot, selected_calls, generic_calls, Some(&initial)));
        }
        rows.extend(failures);
        rows
    }
}
