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
included, is a use that keeps it live. A root with no producing result (a
byte literal's home, for one) counts as present from entry, which can only
add a row.

What the plan does not resolve yet: the loan lineage of a live value that
holds a reference or a view, and of a borrowed parameter. Those rows say
`Unresolved`. The checker judges loans across the call by its own loan
analysis, which a suspension does not interrupt (ownership.md: "Suspension
does not end a loan, restore its parent's access or transfer ownership of its
referent"); a native realization that needs the lineage has nothing to read
until it is published.
*/

use std::collections::BTreeSet;

use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, StructuralTypeId, SuspensionCrossingId, TargetMachineCallable,
    ValueId,
};
use terminal_psi::{
    Block, Operation, OperationResult, StructuralAccess, StructuralArgument, StructuralPathSegment,
    TerminalMachine, TerminalModule, TerminalSuspensionCallPlan, TerminalSuspensionCallSite,
    TerminalSuspensionCallTarget, TerminalSuspensionLiveValue, TerminalSuspensionPlace,
    TerminalSuspensionStorage, TerminalSuspensionValueLoans, TerminalSuspensionValueType,
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
    for projection in operation.kind.structural_projections() {
        uses.insert(Name::Root(projection.root));
        path_values(projection.path, &mut uses);
    }
    for argument in operation.kind.structural_arguments() {
        uses.insert(Name::Root(argument.place));
        path_values(&argument.path, &mut uses);
    }
    uses
}

fn operation_definition(operation: &Operation) -> Option<Name> {
    match &operation.result {
        OperationResult::Unit => None,
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

fn loans_of(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
) -> TerminalSuspensionValueLoans {
    if access == StructuralAccess::Owned
        && !terminal_psi::structural_type_contains_reference(
            &module.structural_types,
            structural_type,
        )
    {
        TerminalSuspensionValueLoans::ViewFree
    } else {
        TerminalSuspensionValueLoans::Unresolved
    }
}

/// The plan of the possibly suspending call `operation` of `machine`: its
/// target and its live frontier, in the canonical row order.
pub fn call_plan(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    crossing: SuspensionCrossingId,
) -> Result<TerminalSuspensionCallPlan, &'static str> {
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
    let entering = live_in(machine);
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
                let (scalar, storage) =
                    scalar_type(value).ok_or("a live scalar has no declaration in its machine")?;
                rows.push(TerminalSuspensionLiveValue {
                    place: TerminalSuspensionPlace::Scalar(value),
                    value_type: TerminalSuspensionValueType::Scalar(scalar),
                    storage,
                    claim_count: 0,
                    claims: Vec::new(),
                    effective: CarryPolicy::PERMISSIVE,
                    loans: TerminalSuspensionValueLoans::ViewFree,
                });
            }
            Name::Root(root) => {
                let structural_type = machine
                    .structural_place_type(root)
                    .ok_or("a live structural root has no declared type")?;
                let parameter = machine
                    .structural_parameters
                    .iter()
                    .chain(
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .find(|parameter| parameter.place == root);
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
                    effective: CarryPolicy::PERMISSIVE,
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
            module.structural_types.iter(),
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
            effective: CarryPolicy::PERMISSIVE,
            loans: loans_of(module, structural_type, argument.access),
        });
    }
    rows.sort_by(|left, right| {
        (&left.place, left.storage, left.value_type, left.effective).cmp(&(
            &right.place,
            right.storage,
            right.value_type,
            right.effective,
        ))
    });
    rows.dedup();
    Ok(TerminalSuspensionCallPlan {
        operation,
        crossing,
        target,
        effective: CarryPolicy::PERMISSIVE,
        live_value_count: u32::try_from(rows.len())
            .map_err(|_| "a suspension frontier holds more rows than a plan can count")?,
        live_values: rows,
        loans: Default::default(),
    })
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
