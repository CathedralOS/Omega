/*
How each machine may leave: whether some path completes normally, and the
crash routes it publishes. Both are read from structure, as a fixed point
over the call graph, once every body is read. Nothing here proves a route possible or impossible:
disproving a route is the checker's work, so a route that structure reaches
is kept.

Published routes follow `wiki/spec/language/effects.md`: an authored
`crashes` clause is the published ceiling; a public body without one
publishes none ("On a public checked body omitted `crashes` means no crash
route"); a private body without one publishes what its body may do ("Private
checked bodies may omit any of the three to request inference"); an authored
`crashes none` publishes none and refuses a body with a crash route ("promises
the empty value and rejects any body or selection that violates it"). Inference
here is structural: each cause the body may crash with, from a `crash`
statement, a Trapping operation or a callee's published route, is published
with the alternative `Truth`. That is the widest route of that cause, so it never
hides a crash a caller must cover; a narrower guard needs proof and belongs to
the checker.

Published service reach follows the same page: "published reach = authored
ceiling, when a clause is present; published reach = derived body reach,
otherwise", where "derived body reach = normalize(contributions from calls)"
and "Reach derivation computes conservative fixed points over recursive
checked-call components." A call of a machine contributes what that machine
publishes; a boundary call contributes "its requirement's explicit reach
ceiling", never the called trait's own identity. Whether a derived reach fits
a written ceiling is the checker's judgment, made on the published rows; a
row is only published here.

Published synchronous entries (`invokes`) are the same derivation on a second
axis (`Axis`): "the callee's published `invokes` joins the caller's, as its
reach does". A written clause is the published ceiling, an omitted one on a
checked body publishes what its calls contribute, and a boundary call
contributes its requirement's row, where omission is "No synchronous entry".
A call of a requirement is "not itself an entry", so nothing in the subset
this route reads originates an entry: a row is non-empty only where a clause
was written.
*/

use std::collections::BTreeSet;

use semantic_vocabulary::{CrashCause, IdTable, MachineId, ServiceId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard};

use crate::body::{
    Axis, Body, CallTarget, Ceiling, CrashCeiling, Expression, ExpressionKind, Signatures,
    Statement, Target,
};
use crate::canonical;
use crate::components::{Components, index};
use crate::{Unserved, refuse};
use terminal_check_tables::Refusal;

/// Call-visible outcomes in this compilation's fixed source unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Published suspension and blocking, independently bounded by the header.
    pub operational: [bool; 2],
    /// A normal return is possible under what callers may know. A local
    /// checked body can exclude it; an opaque contract cannot borrow the
    /// selected body's stronger outcome.
    pub completes_normally: bool,
    /// The published crash routes in canonical form, over the formals.
    pub crash_routes: Vec<CrashRouteBucket>,
    /// The service reach a caller may rely on, strictly increasing: the
    /// written ceiling, or what the body's calls contribute.
    pub service_reach: Vec<ServiceId>,
    /// Fixed reach when an invocation supplies part of the published row through a static binder.
    pub fixed_service_reach: Option<Vec<ServiceId>>,
    /// The synchronous entries a caller is charged, strictly increasing:
    /// the written `invokes` ceiling, or what the body's calls contribute.
    pub synchronous_entries: Vec<ServiceId>,
}

/// The outcomes of every machine, indexed by `MachineId`.
///
/// Machines may call each other in a cycle, so the table is the least fixed
/// point of one rule: every machine starts as never completing and reaching
/// no crash, each body is walked against the table as it stands, and a body
/// is walked again only when a callee's row changed. Both terms only grow (a
/// machine found to complete stays so, a cause found stays), so the walk
/// ends. A machine whose only paths call it again never completes normally,
/// which is what `wiki/spec/terminal-psi/control_flow.md` says of it: "An
/// infinite path has no normal return".
///
/// The walk follows `components`, the call components of these bodies, with
/// callees' components first, so a body outside a call cycle is walked once,
/// against its callees' final rows, whatever the declaration order. Only a
/// cycle iterates, as effects.md says: "Reach derivation computes
/// conservative fixed points over recursive checked-call components." The
/// component graph holds every call a body names, so it covers every row a
/// walk reads, including a call reached only once an earlier call is found
/// to complete. Its least fixed point is the one whole-program rounds reach.
///
/// When bodies refuse, the refusal of the lowest `MachineId` is returned, so
/// the diagnostic does not depend on the order in which rows settle.
///
/// A machine in `requirements` is a requirement's contract and has no body:
/// its member may return, and it may crash only as its authored ceiling says.
/// Source body summaries are restricted to `local_bodies`; all other rows
/// retain a possible normal return while their actual bodies are still walked
/// for service, crash and operational contributions.
pub(crate) fn outcomes(
    bodies: &IdTable<MachineId, Body>,
    signatures: Signatures<'_>,
    requirements: &[MachineId],
    callables: &crate::callables::CallableNamespace,
    local_bodies: Option<&[MachineId]>,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    components: &Components,
) -> Checked<IdTable<MachineId, Outcome>> {
    let mut walk = Walk::new(
        bodies,
        signatures,
        requirements,
        callables,
        local_bodies,
        inhabitation,
    )?;
    walk.settle(bodies, components)?;
    walk.table()
}

/// `outcomes`, with the number of body walks it took; `by_rounds` instead
/// walks every body in identity order each round until no row changes, the
/// whole-program fixed point the callee-first walk must agree with.
#[cfg(test)]
pub(crate) fn counted_outcomes(
    bodies: &IdTable<MachineId, Body>,
    signatures: Signatures<'_>,
    requirements: &[MachineId],
    local_bodies: Option<&[MachineId]>,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    by_rounds: bool,
) -> Checked<(IdTable<MachineId, Outcome>, usize)> {
    let callables = crate::callables::CallableNamespace::default();
    let mut walk = Walk::new(
        bodies,
        signatures,
        requirements,
        &callables,
        local_bodies,
        inhabitation,
    )?;
    if by_rounds {
        loop {
            let mut changed = false;
            for (machine, body) in bodies.iter() {
                changed |= walk.visit(machine, body)?;
            }
            if !changed {
                break;
            }
        }
    } else {
        walk.settle(bodies, &crate::components::discover(bodies)?)?;
    }
    Ok((walk.table()?, walk.visits))
}

/// What structure reaches in one body, against the table as it stands.
#[derive(Clone, Default, PartialEq)]
struct Reached {
    operational: [bool; 2],
    completes_normally: bool,
    /// Every cause a path of the body may crash with, from a `crash`
    /// statement, a Trapping operation or a callee that may.
    causes: BTreeSet<CrashCause>,
    /// What the body's calls contribute on each ceiling axis, by `Axis`.
    contributed: [Contributed; 2],
}

/// What a body's calls contribute on one ceiling axis.
#[derive(Clone, Default, PartialEq)]
struct Contributed {
    /// Every service a call contributes; strictly increasing once the
    /// body's walk ends.
    services: Vec<ServiceId>,
    /// Selected reach belongs to the callback dependency rather than the fixed part.
    selected: Option<Vec<ServiceId>>,
    /// Some call in the body is of a requirement that omits the clause and
    /// defers the row to its selected member, or of a machine that
    /// publishes such a call.
    open: bool,
}

struct Walk<'b> {
    signatures: Signatures<'b>,
    callables: &'b crate::callables::CallableNamespace,
    /// Whether a machine's normal-completion inference is its own body's.
    local: IdTable<MachineId, bool>,
    inhabitation: &'b terminal_semantics::inhabitation::Inhabitation,
    /// Machines that are a requirement's contract and have no body.
    requirements: &'b [MachineId],
    reached: IdTable<MachineId, Reached>,
    /// Every cause a path of the body being walked may crash with.
    causes: BTreeSet<CrashCause>,
    /// What the calls of the body being walked contribute on each axis, in
    /// the order met; sorted when the body's walk ends.
    contributed: [Contributed; 2],
    operational: [bool; 2],
    #[cfg(test)]
    visits: usize,
}

impl<'b> Walk<'b> {
    fn new(
        bodies: &IdTable<MachineId, Body>,
        signatures: Signatures<'b>,
        requirements: &'b [MachineId],
        callables: &'b crate::callables::CallableNamespace,
        local_bodies: Option<&[MachineId]>,
        inhabitation: &'b terminal_semantics::inhabitation::Inhabitation,
    ) -> Checked<Self> {
        // A closed-body input has no source-unit boundary. Source compilations
        // supply their exact local-body roster, independently of whether result
        // facts can be published as additional clauses of those bodies.
        let mut local = bodies.map(|_, _| local_bodies.is_none());
        for &machine in local_bodies.unwrap_or_default() {
            *local.get_mut(machine).ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Machine("local body summary outside the module"),
                )
            })? = true;
        }
        Ok(Walk {
            signatures,
            callables,
            local,
            requirements,
            inhabitation,
            reached: bodies.map(|_, _| Reached::default()),
            causes: BTreeSet::new(),
            contributed: Default::default(),
            operational: [false; 2],
            #[cfg(test)]
            visits: 0,
        })
    }

    /// Settles every row, one call component at a time with callees first.
    /// Inside a cycle a member is walked again only when a callee in the same
    /// component changed its row since the member was last walked.
    fn settle(
        &mut self,
        bodies: &IdTable<MachineId, Body>,
        components: &Components,
    ) -> Checked<()> {
        let mut stale = vec![false; bodies.len()];
        let mut refused = vec![false; bodies.len()];
        let mut refusal: Option<(MachineId, Refusal)> = None;
        for (members, cyclic) in components.callee_first() {
            let mut pending = members.len();
            for &member in members {
                stale[index(member)] = true;
            }
            while pending > 0 {
                for &machine in members {
                    let this = index(machine);
                    if !std::mem::replace(&mut stale[this], false) {
                        continue;
                    }
                    pending -= 1;
                    let body =
                        terminal_check_tables::row(bodies, machine, machine, Construct::Machine)?;
                    match self.visit(machine, body) {
                        Ok(changed) => {
                            if !(changed && cyclic) {
                                continue;
                            }
                            for caller in components.callers(this) {
                                if components.same_component(caller, this)
                                    && !refused[caller]
                                    && !std::mem::replace(&mut stale[caller], true)
                                {
                                    pending += 1;
                                }
                            }
                        }
                        // A refused body keeps the row it had; the program is
                        // refused, and the rest is walked only to choose the
                        // lowest refusing machine.
                        Err(error) => {
                            refused[this] = true;
                            if refusal.as_ref().is_none_or(|(first, _)| machine < *first) {
                                refusal = Some((machine, error));
                            }
                        }
                    }
                }
            }
        }
        match refusal {
            Some((_, error)) => Err(error),
            None => Ok(()),
        }
    }

    /// Walks `machine`'s body against the table as it stands; true when its
    /// row changed.
    fn visit(&mut self, machine: MachineId, body: &Body) -> Checked<bool> {
        #[cfg(test)]
        {
            self.visits += 1;
        }
        self.causes.clear();
        self.contributed = Default::default();
        self.operational = [false; 2];
        let declared_empty = crate::inhabitation::excludes_result(
            self.inhabitation,
            self.signatures.get(machine.into(), machine)?.result,
        );
        let completes_normally = if self.requirements.contains(&machine) {
            !declared_empty
        } else {
            // Always walk the body for its other effects. Only its
            // normal-completion inference is private to the unit.
            let completes = self.statements(machine, &body.statements)?;
            !declared_empty && (completes || self.local.get(machine) != Some(&true))
        };
        let found = Reached {
            operational: self.operational,
            completes_normally,
            causes: std::mem::take(&mut self.causes),
            contributed: std::mem::take(&mut self.contributed).map(|mut row| {
                row.services.sort_unstable();
                row.services.dedup();
                if let Some(selected) = &mut row.selected {
                    selected.sort_unstable();
                    selected.dedup();
                }
                row
            }),
        };
        let slot = self.reached.get_mut(machine).ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Call("callee outside the module"),
            )
        })?;
        Ok(if *slot != found {
            *slot = found;
            true
        } else {
            false
        })
    }

    /// The published table, once every row is settled.
    fn table(&self) -> Checked<IdTable<MachineId, Outcome>> {
        let mut table = IdTable::default();
        for (machine, reached) in self.reached.iter() {
            table.push(Outcome {
                operational: self
                    .signatures
                    .get(machine.into(), machine)?
                    .operational
                    .published(reached.operational),
                completes_normally: reached.completes_normally,
                crash_routes: self.published(machine, &reached.causes)?,
                service_reach: self.published_row(machine, Axis::Reach, reached, true)?,
                fixed_service_reach: self.fixed_reach(machine, reached)?,
                synchronous_entries: self.published_row(machine, Axis::Entries, reached, true)?,
            });
        }
        Ok(table)
    }

    fn fixed_reach(
        &self,
        machine: MachineId,
        reached: &Reached,
    ) -> Checked<Option<Vec<ServiceId>>> {
        let contributed = &reached.contributed[Axis::Reach as usize];
        if contributed.selected.is_none() {
            return Ok(None);
        }
        let signature = self.signatures.get(machine.into(), machine)?;
        Ok(Some(match signature.ceiling(Axis::Reach) {
            Ceiling::Authored(ceiling) => ceiling.clone(),
            Ceiling::Inferred => contributed.services.clone(),
        }))
    }

    /// The routes `machine` publishes, given the causes its body reaches.
    fn published(
        &self,
        machine: MachineId,
        causes: &BTreeSet<CrashCause>,
    ) -> Checked<Vec<CrashRouteBucket>> {
        let signature = self.signatures.get(machine.into(), machine)?;
        Ok(match &signature.crashes {
            CrashCeiling::Authored(routes) => routes.clone(),
            // A requirement that defers its crash routes to the selected
            // member publishes none in the generic form (`lower_generic`).
            CrashCeiling::Inferred if self.requirements.contains(&machine) => Vec::new(),
            CrashCeiling::Inferred => {
                canonical::routes(causes.iter().map(|cause| (*cause, CrashRouteGuard::Truth)))
            }
            // The author promised no crash route, so nothing is inferred and
            // none is published. Each crash site the body reaches then owes
            // coverage by no route, which holds only where the site is
            // unreachable: terminal-psi-to-terminal-psi proves that from what is known
            // there (`requires d != 0` against a `d == 0` guard) or rejects.
            CrashCeiling::DeclaredNone => Vec::new(),
        })
    }

    /// The row `machine` publishes on `axis`, given what its body's calls
    /// contribute: one rule for reach and for synchronous entries.
    ///
    /// A requirement that omits `reaches` is open: effects.md, "defer to the
    /// selected implementation's published reach". The generic form keeps
    /// no row dependency, so it publishes nothing for one, as it does for a
    /// deferred crash route. A written ceiling over such a call cannot be
    /// compared with a contribution nobody knows yet ("The template must
    /// remain valid for every admitted selection"), so that machine is
    /// refused when its row is published (`judged`).
    fn published_row(
        &self,
        machine: MachineId,
        axis: Axis,
        reached: &Reached,
        judged: bool,
    ) -> Checked<Vec<ServiceId>> {
        let signature = self.signatures.get(machine.into(), machine)?;
        let contributed = &reached.contributed[axis as usize];
        Ok(match signature.ceiling(axis) {
            Ceiling::Authored(_) if judged && contributed.open => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Machine(match axis {
                        Axis::Reach => "reach ceiling over a requirement that defers its reach",
                        Axis::Entries => {
                            "invokes ceiling over a requirement that defers its entries"
                        }
                    }),
                ));
            }
            Ceiling::Authored(ceiling) => ceiling.clone(),
            Ceiling::Inferred => {
                let mut services = contributed.services.clone();
                if let Some(selected) = &contributed.selected {
                    services.extend(selected);
                    services.sort_unstable();
                    services.dedup();
                }
                services
            }
        })
    }

    /// Whether the statements can fall through to a normal completion or
    /// complete normally in an arm. Statements after a no-successor exit are
    /// not reached, so they contribute nothing; lowering leaves them out
    /// (`lower`, `body`).
    fn statements(&mut self, machine: MachineId, statements: &[Statement]) -> Checked<bool> {
        let mut normal = false;
        for statement in statements {
            match statement {
                Statement::Bind { value, .. } | Statement::Decompose { subject: value } => {
                    if !self.expression(machine, value)? {
                        return Ok(normal);
                    }
                }
                Statement::Store {
                    destination, value, ..
                } => {
                    if !self.expression(machine, destination)? {
                        return Ok(normal);
                    }
                    if !self.expression(machine, value)? {
                        return Ok(normal);
                    }
                }
                Statement::Evaluate { expression, .. } => {
                    if !self.expression(machine, expression)? {
                        return Ok(normal);
                    }
                }
                // These bindings evaluate no value and reach no call.
                Statement::Declare { .. }
                | Statement::Witness { .. }
                | Statement::CaptureReference { .. } => {}
                // Exactly one arm runs and each completes the machine, so
                // the dispatch completes normally when some arm does and
                // nothing follows it.
                Statement::Dispatch {
                    subject,
                    arms,
                    default,
                    ..
                } => {
                    if !self.expression(machine, subject)? {
                        return Ok(normal);
                    }
                    for (_, target, _) in arms {
                        normal |= self.target(machine, target)?;
                    }
                    if let Some((target, _)) = default {
                        normal |= self.target(machine, target)?;
                    }
                    return Ok(normal);
                }
                Statement::Arm { guard, target, .. } => {
                    if let Some(guard) = guard
                        && !self.expression(machine, guard)?
                    {
                        return Ok(normal);
                    }
                    normal |= self.target(machine, target)?;
                    if guard.is_none() {
                        return Ok(normal);
                    }
                }
            }
        }
        // The last statement fell through: a Unit machine completes here.
        Ok(true)
    }

    fn target(&mut self, machine: MachineId, target: &Target) -> Checked<bool> {
        match target {
            Target::Value(expression) => self.expression(machine, expression),
            Target::Unit => Ok(true),
            Target::Crash(cause) => {
                self.causes.insert(*cause);
                Ok(false)
            }
        }
    }

    /// Records the crash causes of every call in `expression`; false when a
    /// call in it has no normal completion, so evaluation never finishes.
    fn expression(&mut self, machine: MachineId, expression: &Expression) -> Checked<bool> {
        match &expression.kind {
            ExpressionKind::Atomic(operation) => {
                for operand in operation.inputs() {
                    if !self.expression(machine, operand)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ExpressionKind::SavedSubject(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::ScalarApplication { .. }
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::MachineValue { .. }
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_) => Ok(true),
            ExpressionKind::Not(operand)
            | ExpressionKind::BitwiseNot(operand)
            | ExpressionKind::Cast(operand)
            | ExpressionKind::Requalify(operand)
            | ExpressionKind::Referent(operand)
            | ExpressionKind::Establish { operand, .. } => self.expression(machine, operand),
            ExpressionKind::Field { base, .. }
            | ExpressionKind::CollectionMeasure { base, .. }
            | ExpressionKind::ByteSequencePredicate { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::IsCase { subject: base, .. }
            | ExpressionKind::Borrow { place: base, .. } => self.expression(machine, base),
            // One arm runs: the match completes when its subject does and
            // some arm does, and every arm's causes are reachable.
            ExpressionKind::Match {
                subject,
                arms,
                default,
                ..
            } => {
                if !self.expression(machine, subject)? {
                    return Ok(false);
                }
                let mut completes = false;
                for (_, arm) in arms {
                    completes |= self.expression(machine, arm)?;
                }
                if let Some(arm) = default {
                    completes |= self.expression(machine, arm)?;
                }
                Ok(completes)
            }
            // Operands in authored order; the establishment itself cannot
            // crash or fail to complete.
            ExpressionKind::Array { elements } => {
                for element in elements {
                    if !self.expression(machine, element)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ExpressionKind::CollectionView { base, .. } => self.expression(machine, base),
            ExpressionKind::CollectionSubslice { base, start, end } => {
                for operand in std::iter::once(base.as_ref())
                    .chain(start.as_deref())
                    .chain(end.as_deref())
                {
                    if !self.expression(machine, operand)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ExpressionKind::Index { base, index } | ExpressionKind::ByteElement { base, index } => {
                Ok(self.expression(machine, base)? && self.expression(machine, index)?)
            }
            ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
                for (_, operand) in fields {
                    if !self.expression(machine, operand)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            // A Trapping operation is its own `Trap` site (numeric_values.md,
            // "Policy operators": it "contributes that route to its
            // machine's crash contract like any other Trapping operation").
            ExpressionKind::Strict {
                policy,
                left,
                right,
                ..
            } => {
                let completes =
                    self.expression(machine, left)? && self.expression(machine, right)?;
                if *policy == Some(crate::body::Policy::Trapping) {
                    self.causes.insert(CrashCause::Trap);
                }
                Ok(completes)
            }
            // The right operand may be skipped, so the expression completes
            // whenever its left operand does; the right operand's causes are
            // still reachable.
            ExpressionKind::ShortCircuit { left, right, .. } => {
                if !self.expression(machine, left)? {
                    return Ok(false);
                }
                self.expression(machine, right)?;
                Ok(true)
            }
            ExpressionKind::Call {
                callee,
                required_contract,
                static_reach_binding,
                arguments,
                ..
            } => {
                if let Some(operand) = callee.operand()
                    && !self.expression(machine, operand)?
                {
                    return Ok(false);
                }
                for argument in arguments {
                    if !self.expression(machine, argument)? {
                        return Ok(false);
                    }
                }
                match callee {
                    CallTarget::Selected { contract, .. } => {
                        let contract =
                            self.callables.contracts.get(*contract).ok_or_else(|| {
                                refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Call("selected call has no public contract"),
                                )
                            })?;
                        let signature = &contract.signature;
                        for (contributes, published) in self
                            .operational
                            .iter_mut()
                            .zip([contract.envelope.suspends, contract.envelope.blocks])
                        {
                            *contributes |= published;
                        }
                        match &signature.crashes {
                            CrashCeiling::Authored(routes) => {
                                self.causes.extend(routes.iter().map(|bucket| bucket.cause))
                            }
                            CrashCeiling::DeclaredNone => {}
                            CrashCeiling::Inferred => {
                                return Err(refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Call("selected call has an open crash ceiling"),
                                ));
                            }
                        }
                        for axis in Axis::ALL {
                            let Ceiling::Authored(row) = signature.ceiling(axis) else {
                                return Err(refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Call("selected call has an open operational ceiling"),
                                ));
                            };
                            self.contributed[axis as usize]
                                .services
                                .extend(row.iter().copied());
                        }
                        return Ok(true);
                    }
                    CallTarget::Boundary { boundary } => {
                        let signature = self.signatures.invocation(
                            semantic_vocabulary::TargetMachineCallable::Boundary(*boundary),
                            *required_contract,
                            machine,
                        )?;
                        for (contributes, published) in self
                            .operational
                            .iter_mut()
                            .zip(signature.operational.published([false; 2]))
                        {
                            *contributes |= published;
                        }
                        let routes = signature.declared_crash_routes().ok_or_else(|| {
                            refuse(
                                machine,
                                Construct::Machine,
                                Unserved::Call("boundary call has no published crash contract"),
                            )
                        })?;
                        self.causes.extend(routes.iter().map(|route| route.cause));
                        for axis in Axis::ALL {
                            let Ceiling::Authored(row) = signature.ceiling(axis) else {
                                return Err(refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Call("boundary call has an open operational ceiling"),
                                ));
                            };
                            self.contributed[axis as usize]
                                .services
                                .extend(row.iter().copied());
                        }
                        return Ok(required_contract.is_some()
                            || self.signatures.completion(*boundary, machine)?
                                == semantic_vocabulary::BoundaryCompletion::NormalReturn);
                    }
                    CallTarget::Machine(_) => {}
                }
                let CallTarget::Machine(callee) = callee else {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Call("call outcome lost its target classification"),
                    ));
                };
                // A caller sees the causes the callee publishes, as the
                // table stands: its ceiling when it authors one, what its
                // body reaches when the routes are inferred, none under
                // `crashes none` or a deferring requirement.
                let reached = self.reached.get(*callee).ok_or_else(|| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Call("callee outside the module"),
                    )
                })?;
                // A routed requirement advertises its own contract, never
                // the stronger private outcome of its chosen implementation.
                let causes = reached.causes.clone();
                let signature =
                    self.signatures
                        .invocation((*callee).into(), *required_contract, machine)?;
                let completes =
                    !crate::inhabitation::excludes_result(self.inhabitation, signature.result)
                        && !terminal_semantics::inhabitation::predicates_are_false(
                            &signature.ensures,
                        )
                        && (required_contract.is_some() || reached.completes_normally);
                let published = signature.operational.published(reached.operational);
                for (contributes, published) in self.operational.iter_mut().zip(published) {
                    *contributes |= published;
                }
                let mut joins = Vec::with_capacity(Axis::ALL.len());
                for axis in Axis::ALL {
                    let (row, open) = match signature.ceiling(axis) {
                        Ceiling::Authored(row) => (row.clone(), false),
                        Ceiling::Inferred => {
                            let physical = self.signatures.get((*callee).into(), machine)?;
                            let open = matches!(physical.ceiling(axis), Ceiling::Inferred)
                                && (reached.contributed[axis as usize].open
                                    || self.requirements.contains(callee));
                            (self.published_row(*callee, axis, reached, false)?, open)
                        }
                    };
                    joins.push((axis, row, open));
                }
                // A written public ceiling governs this call even if its
                // selected implementation promises less. Only an omitted
                // ceiling is closed from that exact body's outcome.
                let published = match signature.declared_crash_routes() {
                    Some(routes) => routes.to_vec(),
                    None => self.published(*callee, &causes)?,
                };
                self.causes
                    .extend(published.iter().map(|bucket| bucket.cause));
                for (axis, row, open) in joins {
                    let contributed = &mut self.contributed[axis as usize];
                    if axis == Axis::Reach && static_reach_binding.is_some() {
                        contributed
                            .selected
                            .get_or_insert_with(Vec::new)
                            .extend(row);
                    } else {
                        contributed.services.extend(row);
                    }
                    contributed.open |= open;
                }
                Ok(completes)
            }
        }
    }
}
