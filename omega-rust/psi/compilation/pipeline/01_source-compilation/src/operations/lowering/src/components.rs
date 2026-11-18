/*
Call components: which machines lie on a call cycle, and whether the cycle is
one this crate lowers. A component is a strongly connected set of machines in
the call graph of the bodies read: one machine that calls itself, or several
that reach each other.

`wiki/spec/language/termination.md`, "Calls and productive loops": "Runtime
recursive call cycles are tail-position only and lower to constant-stack
iteration." and "A productive terminal-transfer cycle without a termination
promise may run forever and owes no rank". So a component is lowered as a
loop (`lower`: one head block per member, a jump for each call between
members) when every call between its members stands in terminal position,
where the call's result completes the caller. Source nesting plays no part:
`wiki/spec/terminal-psi/control_flow.md`, "Source nesting does not bound the
component or justify any edge." The lowered graph checks its invocation's
published guarantees at every normal return. A jump within the component
supplies no callee guarantee.

Everything else about a cycle that the loops lane has not yet served is
refused here by name, as `Unsupported`, before anything is lowered
(`wiki/drafts/designs/loops_recursion_termination_lane.md`, "Slices").
*/

use arena::IdTable;
use language_core::ReferenceAccess;
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::CrashRouteGuard;

use crate::lowering::body::{
    Body, Borrowed, CallTarget, CrashCeiling, Expression, ExpressionKind, Regions, Signature,
    Statement, Target, Termination, ValueType,
};
use crate::lowering::{Unserved, refuse};

#[path = "components_graph.rs"]
mod graph;
pub(crate) use graph::Components;
#[path = "components_rankings.rs"]
mod rankings;

/// All call occurrences in body order, grouped by caller without one allocation
/// per machine. Graph discovery and cycle admission borrow the same occurrences.
struct CallGraph<'body> {
    sites: Vec<CallSite<'body>>,
    offsets: Vec<usize>,
    /// Exact Build input occurrences currently require one emitted site.
    has_designated: Vec<bool>,
}

enum BodyWalk<'body, 'out> {
    Calls {
        sites: &'out mut Vec<CallSite<'body>>,
        has_designated: bool,
    },
    Direct {
        signature: &'out Signature,
        candidates: &'out [bool],
        observes_scalar_field: bool,
        borrows_from_inputs: bool,
    },
}

impl CallGraph<'_> {
    fn calls(&self, machine: usize) -> &[CallSite<'_>] {
        &self.sites[self.offsets[machine]..self.offsets[machine + 1]]
    }
}

/// One call in a body: its callee, and whether its result completes the
/// caller.
struct CallSite<'body> {
    callee: MachineId,
    terminal: bool,
    required_contract: Option<semantic_vocabulary::ContractId>,
    arguments: &'body [Expression],
}

pub(crate) fn index(machine: MachineId) -> usize {
    usize::try_from(machine.get() - 1).unwrap_or(usize::MAX)
}

/// The call components of the bodies read, before any cycle is judged, so a
/// test can settle outcomes (`outcome::outcomes`) over a cycle that lowering
/// would refuse. Compilation settles over the admitted `components` table.
#[cfg(test)]
pub(crate) fn discover(bodies: &IdTable<MachineId, Body>) -> Checked<Components> {
    Ok(call_graph(bodies)?.1)
}

fn call_graph(bodies: &IdTable<MachineId, Body>) -> Checked<(CallGraph<'_>, Components)> {
    let count = bodies.len();
    let mut calls = CallGraph {
        sites: Vec::with_capacity(count),
        offsets: Vec::with_capacity(count + 1),
        has_designated: Vec::with_capacity(count),
    };
    calls.offsets.push(0);
    for (machine, body) in bodies.iter() {
        let start = calls.sites.len();
        let mut walk = BodyWalk::Calls {
            sites: &mut calls.sites,
            has_designated: false,
        };
        statements(&body.statements, &mut walk);
        calls.has_designated.push(matches!(
            walk,
            BodyWalk::Calls {
                has_designated: true,
                ..
            }
        ));
        if calls.sites[start..]
            .iter()
            .any(|site| index(site.callee) >= count)
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Call("callee outside the module"),
            ));
        }
        calls.offsets.push(calls.sites.len());
    }
    let machines: Vec<_> = bodies.keys().collect();
    let table = Components::discover(&calls, &machines);
    Ok((calls, table))
}

pub(crate) fn components(
    bodies: &IdTable<MachineId, Body>,
    signatures: &mut IdTable<MachineId, Signature>,
) -> Checked<Components> {
    let (calls, mut table) = call_graph(bodies)?;
    rankings::carry(&calls, &table, signatures)?;
    let ranked = bodies
        .keys()
        .map(|machine| {
            terminal_check_tables::row(signatures, machine, machine, Construct::Machine)
                .map(|signature| signature.ranking.is_some())
        })
        .collect::<Checked<Vec<_>>>()?;
    table.rank(&calls, &ranked);
    for (machine, body) in bodies.iter() {
        let this = index(machine);
        let signature =
            terminal_check_tables::row(signatures, machine, machine, Construct::Machine)?;
        let members = table.members(machine)?;
        let unserved = |what| Err(refuse(machine, Construct::Machine, what));
        // A promise depends on every reachable cyclic member retaining its
        // own measure. The graph propagates missing witnesses once; lowering
        // and the control judgment still prove their actual edge comparisons.
        if signature.termination == Termination::Promised && table.reaches_unranked_cycle(this) {
            return unserved(Unserved::Contract("termination promise over a call cycle"));
        }
        if !members.is_empty() {
            // A frame-reusing jump cannot yet retain the public call's exact
            // invocation association. Keep that contract instead of erasing it.
            if calls.calls(this).iter().any(|site| {
                site.required_contract.is_some() && table.same_component(this, index(site.callee))
            }) {
                return unserved(Unserved::Call("selected public contract on a call cycle"));
            }
            if calls
                .calls(this)
                .iter()
                .any(|site| !site.terminal && table.same_component(this, index(site.callee)))
            {
                return unserved(Unserved::Call("recursion outside a terminal position"));
            }
            // A head's parameters carry the member's runtime formals.
            // An erased binding has no runtime lane to carry around the
            // loop, and a value of a type parameter is not one the heads
            // declare yet; each needs its own slice.
            if !signature.erased.is_empty()
                || body
                    .statements
                    .iter()
                    .any(|statement| matches!(statement, Statement::Witness { .. }))
            {
                return unserved(Unserved::Machine("erased binding on a call cycle"));
            }
            let rigid = |value_type: &ValueType| matches!(value_type, ValueType::Rigid(_));
            if signature.parameters.iter().any(rigid)
                || signature.result.as_ref().is_some_and(rigid)
                || body.slots.iter().any(rigid)
            {
                return unserved(Unserved::Machine(
                    "value of a type parameter on a call cycle",
                ));
            }
            if let CrashCeiling::Authored(routes) = &signature.crashes
                && routes.iter().any(|bucket| {
                    bucket
                        .alternatives
                        .iter()
                        .any(|guard| !matches!(guard, CrashRouteGuard::Truth))
                })
            {
                return unserved(Unserved::Contract("guarded crash route on a call cycle"));
            }
        }
    }
    let mut forwarding = Vec::new();
    for (machine, body) in bodies.iter() {
        if needs_separate_entry(machine, body, bodies, signatures, &calls, &table)? {
            table.separate_entry(machine);
        }
        if let Some(receiver) =
            forwarding_receiver(machine, body, bodies, signatures, &calls, &table)?
        {
            table.separate_entry(receiver);
            forwarding.push(machine);
        }
    }
    // Every receiver keeps its local loop. An ordinary forwarding invocation
    // cannot enter another forwarding frame, even when several inputs qualify.
    for machine in forwarding {
        table.forward_entry(machine);
    }
    Ok(table)
}

/// Keep the first reborrow under the invocation that supplies its exact
/// shared parent. The other copies of these bodies still use the cyclic heads.
fn forwarding_receiver(
    machine: MachineId,
    body: &Body,
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    calls: &CallGraph<'_>,
    components: &Components,
) -> Checked<Option<MachineId>> {
    let caller = index(machine);
    if components.members(machine)?.is_empty() || calls.has_designated[caller] {
        return Ok(None);
    }
    let signature = terminal_check_tables::row(signatures, machine, machine, Construct::Machine)?;
    // This entry leaves no owned structural storage in an outer frame while
    // the receiver runs its component. Its normal exit owes no extra result
    // or guarantee, and the canonical heads keep their ranking evidence.
    if signature.result.is_some()
        || !signature.ensures.is_empty()
        || !signature.membership_ensures.is_empty()
        || signature.ranking.is_some()
        || signature.termination != Termination::Silent
        || !signature
            .borrowed
            .iter()
            .any(|formal| formal.access == ReferenceAccess::Shared)
        || !has_no_owned_structural_slots(body, signature)
    {
        return Ok(None);
    }
    let internal = calls
        .calls(caller)
        .iter()
        .filter(|site| components.same_component(caller, index(site.callee)));
    let Some(first) = internal.clone().next() else {
        return Ok(None);
    };
    let receiver = first.callee;
    let receiving = terminal_check_tables::row(signatures, receiver, machine, Construct::Machine)?;
    if receiver == machine || receiving.result.is_some() || calls.has_designated[index(receiver)] {
        return Ok(None);
    }
    let caller_inputs = unchanged_inputs(body, signature);
    if !internal.clone().all(|site| {
        site.callee == receiver
            && site.arguments.iter().all(|argument| match &argument.kind {
                ExpressionKind::Borrow { place, .. } => static_field_slot(place)
                    .is_some_and(|slot| caller_inputs.get(slot) == Some(&true)),
                _ => true,
            })
    }) {
        return Ok(None);
    }
    // Component admission above already proved that these edges are terminal
    // and that they have no separately selected invocation contract.
    let receiver_body = terminal_check_tables::row(bodies, receiver, machine, Construct::Machine)?;
    let receiver_inputs = unchanged_inputs(receiver_body, receiving);
    let observations = calls.calls(index(receiver));
    for formal in &receiving.borrowed {
        let position = formal.position;
        if formal.access != ReferenceAccess::Shared || receiver_inputs.get(position) != Some(&true)
        {
            continue;
        }
        let Some(slot) = shared_argument_slot(first, position) else {
            continue;
        };
        let Some(input) = signature
            .borrowed(slot)
            .filter(|input| input.access == ReferenceAccess::Shared)
        else {
            continue;
        };
        if !internal
            .clone()
            .all(|site| shared_argument_slot(site, position) == Some(slot))
            || !cyclic_local_arrival(machine, input, bodies, signatures, calls, components)?
        {
            continue;
        }
        let reborrows = observations.iter().any(|site| {
            !site.terminal
                && site.required_contract.is_none()
                && (0..site.arguments.len())
                    .any(|argument| shared_argument_slot(site, argument) == Some(position))
        });
        if reborrows
            && observations
                .iter()
                .filter(|site| components.same_component(index(receiver), index(site.callee)))
                .all(|site| {
                    site.arguments.iter().all(|argument| {
                        independent_successor_argument(argument, &receiver_inputs, position)
                    })
                })
        {
            return Ok(Some(receiver));
        }
    }
    Ok(None)
}

fn shared_argument_slot(site: &CallSite<'_>, position: usize) -> Option<usize> {
    let ExpressionKind::Borrow {
        access: ReferenceAccess::Shared,
        place,
        ..
    } = &site.arguments.get(position)?.kind
    else {
        return None;
    };
    match place.kind {
        ExpressionKind::Slot(slot) => Some(slot),
        _ => None,
    }
}

fn unchanged_inputs(body: &Body, signature: &Signature) -> Vec<bool> {
    let mut unchanged = vec![true; signature.parameters.len()];
    for statement in &body.statements {
        let slot = match statement {
            Statement::Bind { slot, .. }
            | Statement::Declare { slot, .. }
            | Statement::CaptureReference { slot, .. } => *slot,
            _ => continue,
        };
        if let Some(input) = unchanged.get_mut(slot) {
            *input = false;
        }
    }
    unchanged
}

/// After the first use, a cyclic successor can carry unrelated input storage
/// or by-value data with no carried lifetime. A copied scalar can observe the
/// shared input without carrying its loan into the successor.
fn independent_successor_argument(argument: &Expression, inputs: &[bool], observed: usize) -> bool {
    let other_input = |slot| slot != observed && inputs.get(slot) == Some(&true);
    match &argument.kind {
        ExpressionKind::Borrow { place, .. } => static_field_slot(place).is_some_and(other_input),
        _ => match argument.result {
            Some(ValueType::Scalar(..) | ValueType::Record(_, _, Regions::NONE)) => true,
            _ => matches!(argument.kind, ExpressionKind::Slot(slot) if other_input(slot)),
        },
    }
}

/// Keep an incoming permission's first use apart from a cyclic arrival that
/// borrows owned local storage. The checker verifies both bodies with their
/// own parent occurrences.
fn needs_separate_entry(
    machine: MachineId,
    body: &Body,
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    calls: &CallGraph<'_>,
    components: &Components,
) -> Checked<bool> {
    if components.members(machine)?.is_empty() || calls.has_designated[index(machine)] {
        return Ok(false);
    }
    let signature = terminal_check_tables::row(signatures, machine, machine, Construct::Machine)?;
    if signature.borrowed.is_empty() {
        return Ok(false);
    }
    let mut candidates = vec![false; signature.parameters.len()];
    for borrowed in &signature.borrowed {
        if borrowed.access == ReferenceAccess::Shared
            && let Some(candidate) = candidates.get_mut(borrowed.position)
        {
            *candidate =
                cyclic_local_arrival(machine, borrowed, bodies, signatures, calls, components)?;
        }
    }
    if !candidates.contains(&true) {
        return Ok(false);
    }
    let mut direct_allowed = true;
    for statement in &body.statements {
        // Only an unchanged formal has the invocation's original permission.
        let slot = match statement {
            Statement::CaptureReference { slot, .. } => {
                direct_allowed = false;
                *slot
            }
            Statement::Bind { slot, .. } | Statement::Declare { slot, .. } => *slot,
            _ => continue,
        };
        if let Some(candidate) = candidates.get_mut(slot) {
            *candidate = false;
            direct_allowed = false;
        }
    }
    if !candidates.contains(&true) {
        return Ok(false);
    }
    let reborrowed = calls.calls(index(machine)).iter().any(|site| {
        !site.terminal
            && site.required_contract.is_none()
            && site.arguments.iter().any(|argument| {
                matches!(&argument.kind,
                    ExpressionKind::Borrow { access: ReferenceAccess::Shared, place, .. }
                    if matches!(place.kind, ExpressionKind::Slot(slot)
                        if candidates.get(slot) == Some(&true)))
            })
    });
    Ok(reborrowed || direct_allowed && direct_scalar_observation(body, signature, &candidates))
}

fn cyclic_local_arrival(
    machine: MachineId,
    borrowed: &Borrowed,
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    calls: &CallGraph<'_>,
    components: &Components,
) -> Checked<bool> {
    for &(caller_index, occurrence) in components.call_sites(index(machine)) {
        if !components.same_component(caller_index, index(machine)) {
            continue;
        }
        let site = &calls.sites[occurrence];
        if !site.terminal || site.required_contract.is_some() {
            continue;
        }
        let Some(Expression {
            kind: ExpressionKind::Borrow { access, place, .. },
            ..
        }) = site.arguments.get(borrowed.position)
        else {
            continue;
        };
        if *access != borrowed.access {
            continue;
        }
        let ExpressionKind::Slot(slot) = place.kind else {
            continue;
        };
        let caller = MachineId::new(caller_index as u64 + 1)
            .expect("a retained caller index has a machine identity");
        let caller_signature =
            terminal_check_tables::row(signatures, caller, machine, Construct::Machine)?;
        let caller_body = terminal_check_tables::row(bodies, caller, machine, Construct::Machine)?;
        if owned_local(caller_body, caller_signature, slot) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// In a direct projection body, only an unchanged input can lend storage.
/// Scalar locals carry no storage into a cyclic successor.
fn direct_scalar_observation(body: &Body, signature: &Signature, candidates: &[bool]) -> bool {
    if !has_no_owned_structural_slots(body, signature) {
        return false;
    }
    let mut walk = BodyWalk::Direct {
        signature,
        candidates,
        observes_scalar_field: false,
        borrows_from_inputs: true,
    };
    statements(&body.statements, &mut walk);
    matches!(
        walk,
        BodyWalk::Direct {
            observes_scalar_field: true,
            borrows_from_inputs: true,
            ..
        }
    )
}

fn has_no_owned_structural_slots(body: &Body, signature: &Signature) -> bool {
    body.slots
        .iter()
        .skip(signature.parameters.len())
        .all(|value| matches!(value, ValueType::Scalar(..)))
        && signature
            .parameters
            .iter()
            .enumerate()
            .all(|(slot, value)| {
                matches!(value, ValueType::Scalar(..)) || signature.borrowed(slot).is_some()
            })
}

/// A static inline field path has one exact root. Crossing a descriptor,
/// indexing, captured aliases and computed places need another supported slice.
fn static_field_slot(mut value: &Expression) -> Option<usize> {
    loop {
        match &value.kind {
            ExpressionKind::Slot(slot) => return Some(*slot),
            ExpressionKind::Field { base, .. } => value = base,
            _ => return None,
        }
    }
}

/// Declare and by-value Bind establish local storage. A reference capture
/// instead aliases another place and supplies no owned-local evidence.
fn owned_local(body: &Body, signature: &Signature, slot: usize) -> bool {
    slot >= signature.parameters.len()
        && matches!(body.slots.get(slot), Some(ValueType::Record(..)))
        && !body.statements.iter().any(|statement| {
            matches!(statement, Statement::CaptureReference { slot: captured, .. }
                if *captured == slot)
        })
        && body.statements.iter().any(|statement| {
            matches!(statement,
                Statement::Declare { slot: established, .. }
                | Statement::Bind { slot: established, .. }
                if *established == slot)
        })
}

fn statements<'body>(statements: &'body [Statement], sites: &mut BodyWalk<'body, '_>) {
    for statement in statements {
        match statement {
            Statement::Bind { value, .. }
            | Statement::Decompose { subject: value }
            | Statement::Evaluate {
                expression: value, ..
            } => expression(value, false, sites),
            Statement::Store {
                destination, value, ..
            } => {
                expression(destination, false, sites);
                expression(value, false, sites);
            }
            // Declarations, erased terms and reference captures call nothing.
            Statement::Declare { .. }
            | Statement::Witness { .. }
            | Statement::CaptureReference { .. } => {}
            Statement::Dispatch {
                subject,
                arms,
                default,
                ..
            } => {
                expression(subject, false, sites);
                for (selector, arm, _) in arms {
                    if let Some(condition) = selector.condition.as_deref() {
                        expression(condition, false, sites);
                    }
                    target(arm, sites);
                }
                if let Some((arm, _)) = default {
                    target(arm, sites);
                }
            }
            Statement::Arm {
                guard, target: arm, ..
            } => {
                if let Some(guard) = guard {
                    expression(guard, false, sites);
                }
                target(arm, sites);
            }
        }
    }
}

/// An arm's target is the one terminal position: a call there forwards its
/// result as the machine's (`wiki/spec/language/machines.md`: "A machine
/// call in a terminal `transition` arm forwards its normal result as the
/// current invocation's result, with no continuation in that invocation.").
fn target<'body>(target: &'body Target, sites: &mut BodyWalk<'body, '_>) {
    match target {
        Target::Unit | Target::Crash(_) => {}
        Target::Value(value) => expression(value, true, sites),
    }
}

fn call<'body>(
    callee: &'body CallTarget,
    arguments: &'body [Expression],
    required_contract: Option<semantic_vocabulary::ContractId>,
    terminal: bool,
    walk: &mut BodyWalk<'body, '_>,
) {
    if let Some(operand) = callee.operand() {
        expression(operand, false, walk);
    }
    for argument in arguments {
        expression(argument, false, walk);
    }
    // These components lower statically named tail transfers to local jumps.
    // A value call stays an invocation with its retained contract; selecting
    // code is not an edge, and no eligible-body list substitutes for it.
    if let BodyWalk::Calls { sites, .. } = walk
        && let CallTarget::Machine(callee) = callee
    {
        sites.push(CallSite {
            callee: *callee,
            terminal,
            required_contract,
            arguments,
        });
    }
}

fn expression<'body>(value: &'body Expression, terminal: bool, sites: &mut BodyWalk<'body, '_>) {
    match &value.kind {
        ExpressionKind::Atomic(operation) => {
            for operand in operation.inputs() {
                expression(operand, false, sites);
            }
        }
        ExpressionKind::SavedSubject(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::ScalarApplication { .. }
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::MachineValue { .. }
        | ExpressionKind::Zero
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Referent(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::CollectionView { base: operand, .. }
        | ExpressionKind::CollectionMeasure { base: operand, .. }
        | ExpressionKind::ByteSequencePredicate { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        } => expression(operand, false, sites),
        ExpressionKind::Field { base, .. } => {
            if let BodyWalk::Direct {
                candidates,
                observes_scalar_field,
                ..
            } = sites
                && !*observes_scalar_field
                && matches!(value.result, Some(ValueType::Scalar(..)))
            {
                *observes_scalar_field = static_field_slot(value)
                    .is_some_and(|slot| candidates.get(slot) == Some(&true));
            }
            expression(base, false, sites);
        }
        ExpressionKind::Borrow { place, .. } => {
            if let BodyWalk::Direct {
                signature,
                borrows_from_inputs,
                ..
            } = sites
                && *borrows_from_inputs
            {
                *borrows_from_inputs =
                    static_field_slot(place).is_some_and(|slot| signature.borrowed(slot).is_some());
            }
            expression(place, false, sites);
        }
        ExpressionKind::Array { elements } => {
            for element in elements {
                expression(element, false, sites);
            }
        }
        ExpressionKind::CollectionSubslice { base, start, end } => {
            for operand in std::iter::once(base.as_ref())
                .chain(start.as_deref())
                .chain(end.as_deref())
            {
                expression(operand, false, sites);
            }
        }
        ExpressionKind::Index { base, index } | ExpressionKind::ByteElement { base, index, .. } => {
            expression(base, false, sites);
            expression(index, false, sites);
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, operand) in fields {
                expression(operand, false, sites);
            }
        }
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
        } => {
            expression(subject, false, sites);
            for (selector, arm) in arms {
                if let Some(pattern) = selector.test() {
                    expression(pattern, false, sites);
                }
                expression(arm, terminal, sites);
            }
            if let Some(arm) = default {
                expression(arm, terminal, sites);
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression(left, false, sites);
            expression(right, false, sites);
        }
        ExpressionKind::Call {
            callee,
            required_contract,
            arguments,
            designated,
            ..
        } => {
            if let BodyWalk::Calls { has_designated, .. } = sites
                && designated.is_some()
            {
                // The source publication owner rejects duplicate emitted
                // Build input sites. Do not peel this body until it serves them.
                *has_designated = true;
            }
            call(callee, arguments, *required_contract, terminal, sites);
        }
    }
}
