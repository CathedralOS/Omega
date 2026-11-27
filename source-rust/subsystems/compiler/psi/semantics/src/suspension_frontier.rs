/*
What a call that may suspend leaves live in its caller, reconstructed from
the lowered machine itself.

calls_and_outcomes.md, "Suspension": "A suspension site binds the exact call,
checked crossing, callee, and commitment to the complete ordered live
frontier. Its plan retains each value/place, type, storage role, structural
access, exact live claims, and carry requirements". While the call is parked
"the operation is incomplete and its result is not established" and "later
operations have not executed", so the frontier is what the caller will still
use after the call returns, plus what it lent the call.

Lowering publishes one site and one plan per possibly suspending call, and
the checker reconstructs both with these same functions and compares: a row
is never taken on a producer's word. Both read the module, so a plan cannot
describe anything the canonical operations do not.

Liveness is the ordinary backward dataflow over the machine's blocks. A
scalar is an SSA value. A structural root is treated the same way: it has one
producer (its operation's result, a block parameter or a machine parameter),
and every later operation that names it, a store into it or a cleanup of it
included, is a use that keeps it live. A byte literal or trivial affine
local is defined by its establishing operation's destination even though
that operation has a Unit result.

What the plan does not resolve yet: the loan lineage of a live value that
holds a reference or a view after descriptor transport. A direct formation
and a shared entry borrow of view-free storage retain their known origin.
Rows awaiting descriptor-flow evidence say
`Unresolved`. The checker judges loans across the call by its own loan
analysis, which a suspension does not interrupt (ownership.md: "Suspension
does not end a loan, restore its parent's access or transfer ownership of its
referent"); a native realization that needs the lineage has nothing to read
until it is published.
*/

use std::collections::BTreeSet;

use crate::descriptor_results::DescriptorSlotKind;
use crate::descriptors::row_source::DescriptorRowSource;
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, StructuralTypeId, SuspensionCrossingId, TargetMachineCallable,
    ValueId,
};
use terminal_psi::{
    Block, Operation, OperationResult, StructuralAccess, StructuralArgument, StructuralPathSegment,
    TerminalMachine, TerminalModule, TerminalSuspensionCallPlan, TerminalSuspensionCallSite,
    TerminalSuspensionCallTarget, TerminalSuspensionLiveValue, TerminalSuspensionLoan,
    TerminalSuspensionLoanBacking, TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource,
    TerminalSuspensionLoans, TerminalSuspensionPlace, TerminalSuspensionStorage,
    TerminalSuspensionValueLoans, TerminalSuspensionValueType,
};

/// Whether the call `operation` makes may suspend, by the contract its
/// callee publishes: a selected call's public contract, a machine's own, or
/// a boundary requirement's. `None` for an operation that is no declared
/// call, or whose callee the module does not hold.
pub fn call_may_suspend(module: &TerminalModule, operation: &Operation) -> Option<bool> {
    let inputs = operation.declared_invocation()?;
    match (inputs.callee, inputs.required_contract) {
        (TargetMachineCallable::Machine(body), Some(required)) => module
            .invocation_machine_contract(body, Some(required))
            .map(|contract| contract.published_may_suspend),
        (TargetMachineCallable::Machine(callee), None) => module
            .machine(callee)
            .map(|machine| machine.published_may_suspend),
        (TargetMachineCallable::Boundary(boundary), _) => module
            .boundary_machine(boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
            .map(|declaration| declaration.source.published_may_suspend),
    }
}

/// One thing liveness follows: a scalar value or a whole structural root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Name {
    Value(ValueId),
    Root(PlaceId),
}

fn path_values(path: &[StructuralPathSegment], uses: &mut BTreeSet<Name>) {
    for segment in path {
        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
            uses.insert(Name::Value(*index));
        }
    }
}

fn operation_uses(operation: &Operation) -> BTreeSet<Name> {
    let mut uses = BTreeSet::new();
    let mut kind = operation.kind.clone();
    kind.map_scalar_uses(&mut |value| {
        uses.insert(Name::Value(value));
        value
    });
    for (storage, _) in operation.kind.storage_uses() {
        use terminal_psi::StructuralStorageUse;
        let root = match storage {
            StructuralStorageUse::Place(root)
            | StructuralStorageUse::Canonical { root, .. }
            | StructuralStorageUse::ObservedField { root, .. } => root,
            StructuralStorageUse::Projection(projection) => {
                path_values(projection.path, &mut uses);
                projection.root
            }
            StructuralStorageUse::Element { descriptor, path } => {
                path_values(descriptor.path, &mut uses);
                path_values(path, &mut uses);
                descriptor.root
            }
            StructuralStorageUse::Type(_) => continue,
        };
        uses.insert(Name::Root(root));
    }
    for argument in operation.kind.structural_arguments() {
        uses.insert(Name::Root(argument.place));
        path_values(&argument.path, &mut uses);
    }
    uses
}

fn operation_definition(operation: &Operation) -> Option<Name> {
    match &operation.result {
        OperationResult::Unit => match &operation.kind {
            terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. }
            | terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } => {
                Some(Name::Root(*destination))
            }
            _ => None,
        },
        OperationResult::Scalar(value) => Some(Name::Value(value.id)),
        OperationResult::Structural(result) => Some(Name::Root(result.place)),
    }
}

fn terminator_uses(block: &Block) -> BTreeSet<Name> {
    let mut uses = BTreeSet::new();
    let mut terminator = block.terminator.clone();
    terminator.map_scalar_uses(&mut |value| {
        uses.insert(Name::Value(value));
        value
    });
    for projection in block.terminator.storage_uses() {
        uses.insert(Name::Root(projection.root));
        path_values(projection.path, &mut uses);
    }
    for arguments in block.terminator.successor_block_arguments() {
        for argument in arguments {
            uses.insert(Name::Root(argument.place));
            path_values(&argument.path, &mut uses);
        }
    }
    uses
}

/// What `block` defines on entry: its own parameters.
fn entry_definitions(block: &Block) -> impl Iterator<Item = Name> + '_ {
    block
        .parameters
        .iter()
        .map(|parameter| Name::Value(parameter.id))
        .chain(
            block
                .structural_parameters
                .iter()
                .map(|parameter| Name::Root(parameter.place)),
        )
}

/// The names live after the operation at `position` of `block`, given what
/// is live when the block ends.
fn live_after(block: &Block, position: usize, live_out: &BTreeSet<Name>) -> BTreeSet<Name> {
    let mut live = live_out.clone();
    live.extend(terminator_uses(block));
    for operation in block.operations.iter().skip(position + 1).rev() {
        if let Some(defined) = operation_definition(operation) {
            live.remove(&defined);
        }
        live.extend(operation_uses(operation));
    }
    live
}

/// What is live on entry to each block, in the order of `machine.blocks`.
fn live_in(machine: &TerminalMachine) -> Vec<BTreeSet<Name>> {
    let index = |id: BlockId| machine.blocks.iter().position(|block| block.id == id);
    let mut entering = vec![BTreeSet::new(); machine.blocks.len()];
    loop {
        let mut changed = false;
        for (position, block) in machine.blocks.iter().enumerate().rev() {
            let mut live_out = BTreeSet::new();
            for successor in block.terminator.successor_targets() {
                if let Some(successor) = index(successor) {
                    live_out.extend(entering[successor].iter().copied());
                }
            }
            let mut live = live_out;
            live.extend(terminator_uses(block));
            for operation in block.operations.iter().rev() {
                if let Some(defined) = operation_definition(operation) {
                    live.remove(&defined);
                }
                live.extend(operation_uses(operation));
            }
            for defined in entry_definitions(block) {
                live.remove(&defined);
            }
            if live != entering[position] {
                entering[position] = live;
                changed = true;
            }
        }
        if !changed {
            return entering;
        }
    }
}

fn contains_borrows(module: &TerminalModule, ty: StructuralTypeId) -> bool {
    terminal_psi::structural_type_has_borrowed_storage_by(|id| module.structural_type(id), ty)
}

fn loans_of(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
) -> TerminalSuspensionValueLoans {
    if access == StructuralAccess::Owned && !contains_borrows(module, structural_type) {
        TerminalSuspensionValueLoans::ViewFree
    } else {
        TerminalSuspensionValueLoans::Unresolved
    }
}

struct LoanFormation<'a> {
    operation: OperationId,
    carrier: PlaceId,
    source: &'a StructuralArgument,
    lifetime: crate::descriptor_lifetimes::DescriptorLifetime,
    direct: bool,
    delegated_from: Vec<(BlockId, u32)>,
}

fn direct_storage(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> bool {
    machine
        .structural_parameter(source.place)
        .is_none_or(|param| param.access == StructuralAccess::Owned)
        && machine
            .structural_place_type(source.place)
            .is_some_and(|root| !contains_borrows(module, root))
        && source.path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
            )
        })
}

/// A later reborrow cannot change an earlier crossing. Record the first
/// possible delegation in each block and propagate it through CFG successors;
/// a backedge can therefore invalidate a crossing on a later iteration.
fn delegation_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    lifetime: &crate::descriptor_lifetimes::DescriptorLifetime,
) -> Vec<(BlockId, u32)> {
    use terminal_psi::OperationKind;
    let mut origins = Vec::new();
    let mut successors = Vec::new();
    for block in &machine.blocks {
        let first = block.operations.iter().enumerate().find_map(|(index, op)| {
            let at = index as u32;
            let locations = lifetime.surviving.get(&(block.id, at))?;
            match &op.kind {
                OperationKind::MoveStructuralValue { .. }
                | OperationKind::StoreStructuralField { .. }
                | OperationKind::PrimitiveScalarRead { .. }
                | OperationKind::WriteOnlyPrimitiveStore { .. }
                | OperationKind::ScalarStructuralField { .. }
                | OperationKind::BooleanStructuralField { .. }
                | OperationKind::StructuralScalarFieldStore { .. }
                | OperationKind::ReleaseReference { .. } => return None,
                // Reading a view-free referent cannot copy or delegate the descriptor.
                OperationKind::StructuralLeafCopy { path, .. }
                    if matches!(path.first(), Some(StructuralPathSegment::Referent))
                        && op.result.as_structural().is_some_and(|value| {
                            module.structural_type(value.structural_type).is_some()
                                && !contains_borrows(module, value.structural_type)
                        }) => return None,
                _ => {}
            }
            let uses = operation_uses(op);
            locations.iter().any(|location| uses.contains(&Name::Root(location.root))).then_some(at)
        });
        let end = block.operations.len() as u32;
        let edge_delegates = || lifetime.surviving.get(&(block.id, end)).is_some_and(|locations| {
            block.terminator.successor_block_arguments().into_iter().flatten().any(|arg| {
                arg.access != StructuralAccess::Owned
                    && locations.iter().any(|location| location.root == arg.place)
            })
        });
        if let Some(at) = first.or_else(|| edge_delegates().then_some(end)) {
            origins.push((block.id, at));
            successors.extend(block.terminator.successor_targets());
        }
    }
    origins.sort_unstable_by_key(|(block, _)| *block);
    if successors.is_empty() {
        return origins;
    }
    let mut blocks = machine
        .blocks
        .iter()
        .map(|block| (block, None))
        .collect::<Vec<_>>();
    blocks.sort_unstable_by_key(|(block, _)| block.id);
    for (id, at) in origins.drain(..) {
        let row = blocks
            .binary_search_by_key(&id, |(block, _)| block.id)
            .expect("block from this machine");
        blocks[row].1 = Some(at);
    }
    while let Some(target) = successors.pop() {
        let Ok(row) = blocks.binary_search_by_key(&target, |(block, _)| block.id) else {
            continue;
        };
        let (block, first) = &mut blocks[row];
        if first.replace(0) != Some(0) {
            successors.extend(block.terminator.successor_targets());
        }
    }
    origins.extend(
        blocks
            .into_iter()
            .filter_map(|(block, first)| Some((block.id, first?))),
    );
    origins
}

/// Crossings in one caller share its block liveness and the transport of its
/// reference descriptors. A machine without a live reference needs no descriptor graph.
pub struct MachineFrontiers<'a, 'r> {
    module: &'a TerminalModule,
    machine: &'a TerminalMachine,
    rows: DescriptorRowSource<'r, 'a>,
    entering: Vec<BTreeSet<Name>>,
    formations: std::cell::OnceCell<Vec<LoanFormation<'a>>>,
}

impl<'a, 'r> MachineFrontiers<'a, 'r> {
    pub fn new(module: &'a TerminalModule, machine: &'a TerminalMachine) -> Self {
        Self::with_rows(DescriptorRowSource::new(module), machine)
    }

    /// `new`, reading the machine's reference rows from the module's shared
    /// analyses, so the frontiers of several callers adapt each body once.
    pub fn new_in(analyses: &'r crate::ModuleAnalyses<'a>, machine: &'a TerminalMachine) -> Self {
        Self::with_rows(DescriptorRowSource::of(analyses), machine)
    }

    fn with_rows(rows: DescriptorRowSource<'r, 'a>, machine: &'a TerminalMachine) -> Self {
        Self {
            module: rows.module(),
            machine,
            rows,
            entering: live_in(machine),
            formations: Default::default(),
        }
    }

    fn formations(&self) -> Vec<LoanFormation<'a>> {
        use crate::descriptor_lifetimes::{DescriptorLocation, descriptor_location_lifetime};
        use terminal_psi::OperationKind;
        let Ok(graphs) = self
            .rows
            .closure(DescriptorSlotKind::References, [self.machine.id])
        else {
            return Vec::new();
        };
        let Some(graph) = graphs.machine(self.machine.id) else {
            return Vec::new();
        };
        let mut formations = Vec::new();
        for block in &self.machine.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let OperationKind::EstablishReference {
                    source,
                    storage: None,
                } = &operation.kind
                else {
                    continue;
                };
                let Some(result) = operation.result.as_structural() else {
                    continue;
                };
                let at = (block.id, index as u32);
                let lifetime = descriptor_location_lifetime(
                    &graph.flow,
                    at,
                    &DescriptorLocation::whole(result.place),
                );
                let direct = direct_storage(self.module, self.machine, source);
                let delegated_from = if direct {
                    delegation_frontier(self.module, self.machine, &lifetime)
                } else {
                    Vec::new()
                };
                formations.push(LoanFormation {
                    operation: operation.id,
                    carrier: result.place,
                    source,
                    lifetime,
                    direct,
                    delegated_from,
                });
            }
        }
        formations
    }

    fn direct_loans(
        &self,
        at: crate::descriptor_lifetimes::DescriptorPosition,
        call: &Operation,
        live: &BTreeSet<Name>,
    ) -> Vec<(&LoanFormation<'a>, PlaceId)> {
        use crate::descriptor_lifetimes::DescriptorLocation;
        let mut loans = Vec::new();
        for name in live {
            let Name::Root(carrier) = *name else { continue };
            let Some(ty) = self
                .machine
                .structural_place_type(carrier)
                .and_then(|id| self.module.structural_type(id))
            else {
                continue;
            };
            if !matches!(
                ty.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            ) || call
                .kind
                .structural_arguments()
                .iter()
                .any(|arg| arg.place == carrier)
            {
                continue;
            }
            let location = DescriptorLocation::whole(carrier);
            let formations = self.formations.get_or_init(|| self.formations());
            let mut possible = formations.iter().filter(|formation| {
                formation
                    .lifetime
                    .surviving
                    .get(&at)
                    .is_some_and(|locations| locations.contains(&location))
            });
            let Some(formation) = possible.next() else {
                continue;
            };
            if possible.next().is_some()
                || !formation
                    .lifetime
                    .authorized
                    .get(&at)
                    .is_some_and(|locations| locations.contains(&location))
            {
                continue;
            }
            if !formation.direct || formation.delegated_from.binary_search_by_key(&at.0, |(block, _)| *block).is_ok_and(|row| formation.delegated_from[row].1 <= at.1) {
                continue;
            }
            loans.push((formation, carrier));
        }
        loans
    }
    pub fn call_plan(
        &self,
        operation: OperationId,
        crossing: SuspensionCrossingId,
    ) -> Result<TerminalSuspensionCallPlan, &'static str> {
        let module = self.module;
        let machine = self.machine;
        let (block_position, position) = machine
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_position, block)| {
                block
                    .operations
                    .iter()
                    .position(|candidate| candidate.id == operation)
                    .map(|position| (block_position, position))
            })
            .ok_or("a suspension crossing names no operation of its machine")?;
        let block = &machine.blocks[block_position];
        let call = &block.operations[position];
        let inputs = call
            .declared_invocation()
            .ok_or("a suspension crossing is on an operation that is no declared call")?;
        let target = match inputs.callee {
            TargetMachineCallable::Machine(callee) => TerminalSuspensionCallTarget::Machine(callee),
            TargetMachineCallable::Boundary(boundary) => {
                TerminalSuspensionCallTarget::Boundary(boundary)
            }
        };
        let entering = &self.entering;
        let mut live_out = BTreeSet::new();
        for successor in block.terminator.successor_targets() {
            let successor = machine
                .blocks
                .iter()
                .position(|candidate| candidate.id == successor)
                .ok_or("a suspension crossing's block leaves for an absent block")?;
            live_out.extend(entering[successor].iter().copied());
        }
        let mut live = live_after(block, position, &live_out);
        // The parked call has not completed, so its result is not established.
        if let Some(defined) = operation_definition(call) {
            live.remove(&defined);
        }

        let direct_loans = self.direct_loans((block.id, position as u32), call, &live);
        for (formation, _) in &direct_loans {
            live.insert(Name::Root(formation.source.place));
        }

        let scalar_type = |value: ValueId| {
            machine
                .parameters
                .iter()
                .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                .find(|declared| declared.id == value)
                .map(|declared| (declared.scalar_type, TerminalSuspensionStorage::Parameter))
                .or_else(|| {
                    machine.operations().find_map(|candidate| {
                        candidate
                            .result
                            .scalar_ref()
                            .filter(|result| result.id == value)
                            .map(|result| (result.scalar_type, TerminalSuspensionStorage::Local))
                    })
                })
        };
        let mut rows = Vec::new();
        for name in &live {
            match *name {
                Name::Value(value) => {
                    let (scalar, storage) = scalar_type(value)
                        .ok_or("a live scalar has no declaration in its machine")?;
                    rows.push(TerminalSuspensionLiveValue {
                        place: TerminalSuspensionPlace::Scalar(value),
                        value_type: TerminalSuspensionValueType::Scalar(scalar),
                        storage,
                        claim_count: 0,
                        claims: Vec::new(),
                        loans: TerminalSuspensionValueLoans::ViewFree,
                    });
                }
                Name::Root(root) => {
                    let structural_type = machine
                        .structural_place_type(root)
                        .ok_or("a live structural root has no declared type")?;
                    let parameter = machine.structural_parameter(root);
                    let (access, storage) = match parameter {
                        Some(parameter) => (parameter.access, TerminalSuspensionStorage::Parameter),
                        None => (StructuralAccess::Owned, TerminalSuspensionStorage::Local),
                    };
                    rows.push(TerminalSuspensionLiveValue {
                        place: TerminalSuspensionPlace::Structural(StructuralArgument {
                            place: root,
                            path: Vec::new(),
                            access,
                        }),
                        value_type: TerminalSuspensionValueType::Structural(structural_type),
                        storage,
                        claim_count: 0,
                        claims: Vec::new(),
                        loans: loans_of(module, structural_type, access),
                    });
                }
            }
        }
        // What the call was lent or given stays with the incomplete callee.
        for argument in inputs.structural_arguments {
            let root = machine
                .structural_place_type(argument.place)
                .ok_or("a call argument names a root with no declared type")?;
            let structural_type = crate::runtime_structural_path_tip(
                module.types.structural_types.iter(),
                root,
                &argument.path,
            )
            .ok_or("a call argument's path selects no declared type")?
            .id;
            rows.push(TerminalSuspensionLiveValue {
                place: TerminalSuspensionPlace::Structural(argument.clone()),
                value_type: TerminalSuspensionValueType::Structural(structural_type),
                storage: TerminalSuspensionStorage::CallArgument,
                claim_count: 0,
                claims: Vec::new(),
                loans: loans_of(module, structural_type, argument.access),
            });
        }
        rows.sort_by(|left, right| {
            (&left.place, left.storage, left.value_type).cmp(&(&right.place, right.storage, right.value_type))
        });
        let live_value_count = u32::try_from(rows.len())
            .map_err(|_| "a suspension frontier holds more rows than a plan can count")?;
        let mut loans = TerminalSuspensionLoans::default();
        for (i, row) in rows.iter_mut().enumerate() {
            let (
                TerminalSuspensionPlace::Structural(argument),
                TerminalSuspensionValueType::Structural(ty),
            ) = (&row.place, row.value_type)
            else {
                continue;
            };
            if row.storage != TerminalSuspensionStorage::Parameter
                || argument.access != StructuralAccess::SharedBorrow
                || machine
                    .signature_structural_parameter(argument.place)
                    .is_none()
                || contains_borrows(module, ty)
            {
                continue;
            }
            let occurrence = loans.occurrences.insert(TerminalSuspensionLoan {
                capture: TerminalSuspensionLoanCapture::RetainedValue {
                    live_value: i as u32,
                    path: Default::default(),
                },
                backing: TerminalSuspensionLoanBacking::IncomingReferent {
                    argument: argument.clone(),
                },
                resource: TerminalSuspensionLoanResource::IncomingReference,
                access: argument.access,
                active_at_call: true,
                ..Default::default()
            });
            row.loans = TerminalSuspensionValueLoans::Retained {
                links: loans.value_links.insert_many([occurrence]),
            };
        }
        // Rows are ordered by place, then storage role. Publishing the backing needs
        // the owned row even when the call also has an argument at the same root.
        let locate = |rows: &[TerminalSuspensionLiveValue], root| {
            let start = rows.partition_point(|row| match &row.place {
                TerminalSuspensionPlace::Scalar(_) => true,
                TerminalSuspensionPlace::Structural(arg) => arg.place < root,
            });
            rows[start..].iter().take_while(|row| {
                matches!(&row.place, TerminalSuspensionPlace::Structural(arg) if arg.place == root && arg.path.is_empty())
            }).position(|row| row.storage != TerminalSuspensionStorage::CallArgument).map(|offset| start + offset)
        };
        let mut direct_loans = direct_loans;
        direct_loans.sort_by_key(|(formation, carrier)| (formation.operation, *carrier));
        let mut last_loan = None;
        for (formation, carrier) in direct_loans {
            let source = formation.source;
            let backing = locate(&rows, source.place)
                .ok_or("a suspended reference lost its backing storage")?;
            let descriptor =
                locate(&rows, carrier).ok_or("a suspended reference lost its descriptor")?;
            let occurrence = match last_loan {
                Some((id, occurrence)) if id == formation.operation => occurrence,
                _ => {
                    let source_path = loans.paths.insert_many(source.path.iter().cloned());
                    let occurrence = loans.occurrences.insert(TerminalSuspensionLoan {
                        capture: TerminalSuspensionLoanCapture::Reference {
                            formation: formation.operation,
                            carrier: formation.carrier,
                            source: source.place,
                            source_access: StructuralAccess::Owned,
                            source_path,
                        },
                        backing: TerminalSuspensionLoanBacking::RetainedOwner {
                            live_value: backing as u32,
                        },
                        resource: TerminalSuspensionLoanResource::DirectRoot,
                        access: source.access,
                        active_at_call: true,
                        ..Default::default()
                    });
                    last_loan = Some((formation.operation, occurrence));
                    occurrence
                }
            };
            rows[descriptor].loans = TerminalSuspensionValueLoans::Retained {
                links: loans.value_links.insert_many([occurrence]),
            };
        }
        Ok(TerminalSuspensionCallPlan {
            operation,
            crossing,
            target,
            live_value_count,
            live_values: rows,
            loans,
        })
    }
}

/// The site of `plan`: the same call and crossing, with the commitment to
/// its frontier.
pub fn call_site(
    module: &TerminalModule,
    plan: &TerminalSuspensionCallPlan,
) -> TerminalSuspensionCallSite {
    TerminalSuspensionCallSite {
        operation: plan.operation,
        crossing: plan.crossing,
        target: plan.target,
        frontier_commitment: terminal_psi::suspension_frontier_commitment(&module.identities, plan),
    }
}
