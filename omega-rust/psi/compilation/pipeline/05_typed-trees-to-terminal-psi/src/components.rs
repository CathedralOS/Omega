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

use semantic_vocabulary::{IdTable, MachineId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::CrashRouteGuard;

use crate::body::{
    Body, CallTarget, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target,
    Termination, ValueType,
};
use crate::{Unserved, refuse};

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
    };
    calls.offsets.push(0);
    for (machine, body) in bodies.iter() {
        let start = calls.sites.len();
        statements(&body.statements, &mut calls.sites);
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
    Ok(table)
}

fn statements<'body>(statements: &'body [Statement], sites: &mut Vec<CallSite<'body>>) {
    for statement in statements {
        match statement {
            Statement::Bind { value, .. }
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
                for (_, arm, _) in arms {
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
fn target<'body>(target: &'body Target, sites: &mut Vec<CallSite<'body>>) {
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
    sites: &mut Vec<CallSite<'body>>,
) {
    if let Some(operand) = callee.operand() {
        expression(operand, false, sites);
    }
    for argument in arguments {
        expression(argument, false, sites);
    }
    // These components lower statically named tail transfers to local jumps.
    // A value call stays an invocation with its retained contract; selecting
    // code is not an edge, and no eligible-body list substitutes for it.
    if let CallTarget::Machine(callee) = callee {
        sites.push(CallSite {
            callee: *callee,
            terminal,
            required_contract,
            arguments,
        });
    }
}

fn expression<'body>(value: &'body Expression, terminal: bool, sites: &mut Vec<CallSite<'body>>) {
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
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Referent(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::ArrayView { base: operand, .. }
        | ExpressionKind::CollectionMeasure { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        }
        | ExpressionKind::Borrow { place: operand, .. } => expression(operand, false, sites),
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
        ExpressionKind::Index { base, index } | ExpressionKind::ByteElement { base, index } => {
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
                if let crate::body::Selector::Value(pattern) = selector {
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
            ..
        } => {
            call(callee, arguments, *required_contract, terminal, sites);
        }
    }
}
