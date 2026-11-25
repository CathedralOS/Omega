/*
The route's verdict on one program: the one result a whole-compilation router
takes from the lowered-form route, and the only way it should take one.

`wiki/drafts/designs/check_on_lowered_form.md` ("Route one", Build): "An
obligation the prover cannot discharge is a compile error for the program,
never a slower guarded form." Omega 00 trusts every obligation of the module
it is given and fills evidence only where a seal arrives, so a module with an
undischarged obligation must never reach it: Omega 00 would accept that
obligation with no evidence. A complete closure is accepted only after the
kernel has discharged every obligation and established every use of storage.
When a Build helper's result depends on a package it prepares, the checker
can accept the executed prefix with the original result still owed at its
exact suspending call. Only the Build interpreter can receive that prefix.
It parks there until a checked successor supplies the suffix. A program can
also be rejected for an unproved obligation or unestablished use, left
incomplete because the prover has no rule for a goal's form, or refused whole because the route does not serve some construct in it
(`RefusalReason::Unsupported`). The variant is still named `FallBack`, but
nothing falls back since the owner's ruling of 2026-11-12: the router refuses
the compilation with an error naming the construct, and no other checker
judges it.

The specification says the same: `wiki/spec/packages/review.md`, "Unproved
source contracts reject in Psi checking"; `wiki/spec/build/configuration.md`,
an unproved obligation "grants no equality, inequality, implementation
selection or runtime fallback". A rejection names what was not proved and
why, and never claims the obligation is false
(`wiki/spec/build/compiler_request.md`, "Outcomes": "Failure to find a proof
is not proof of falsehood"; `wiki/spec/terminal-psi/fact_validity.md`: "The
false premise must be proved, not inferred from search failure"). The
diagnostic is an ordinary `Reject` error; no new outcome tag or exit code.

The two unproved outcomes, by the owner's ruling on
`unproved-goal-form-outcome`: a goal whose form the prover has rules for but
cannot establish from what the program states is `Reject`; a goal whose form
it has no rule for is `Incomplete`, the outcome table's "unsupported valid
case", with "no claim that the unproved conclusion is false"
(`wiki/spec/build/compiler_request.md`, "Outcomes"). Neither publishes an
artifact or a runtime fallback. `Reject` wins when both occur: the program
fails a checking rule whatever the other obligations hold. `Incomplete` has
no `Diagnostic` constructor here, because the closed `Incomplete` resource
table has no provision for a proof goal form yet, and a plain error would
publish it as `Reject`.

`Unsupported` wins over `Unproved`: the route has not judged a closure it
cannot lower whole, so it reports what it does not support rather than a
partial list of failures. Either way the program is not accepted. Any other
refusal is a defect of the route itself (an identity that does not resolve)
and is returned as `Err`; a router must not accept the program on it.

A use of storage that some path reaches with nothing established in it
(`RefusalReason::Unestablished`) is a compile error of the same standing as an
unproved obligation, but it is a structural judgment with no obligation row,
so a rejection carries the two lists side by side. One such use makes the
verdict `Rejected` even when every unproved obligation is merely incomplete:
the program fails a checking rule whatever the prover could or could not do.

A rule of the language that the source form breaks
(`RefusalReason::Violation`: a value whose type carries other domains than its
destination declares, a cast that adds a routed domain outside its routes) is
found by the reader of the typed trees, which refuses the closure with it.
`fall_back_or_defect` is the one place a refusal becomes a verdict, so that
refusal is a rejection with its own list, `violations`, and the same standing
as the others; only `RefusalReason::Unsupported` says "not supported yet".

A loan that may not be formed (`RefusalReason::Conflict`: two loans of one
call that may share storage with one of them exclusive, or an exclusive
reborrow of storage held shared) has the same standing, and is listed beside
the other two as a `ConflictingUse`.
*/

use diagnostics::Diagnostic;
use semantic_vocabulary::{MachineId, ObligationId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::program::{Branch, Elaboration, SourceOrigin};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason, UnprovedCause};

use crate::{Discharge, ImpossibleArm, describe, refusal};

/// What the route decided for one closure.
#[derive(Debug)]
pub enum RouteVerdict {
    /// Every obligation of every machine was discharged with a certificate
    /// the kernel accepted. `seals` are the operation obligations' evidence
    /// for Omega 00.
    Accepted(Box<AcceptedClosure>),
    /// Every operation up to the marked calls has passed ordinary checking.
    /// Completing one of these bodies requires an admitted successor.
    AcceptedPrefix(Box<AcceptedClosure>),
    /// The program is rejected. `unproved` has one entry per obligation the
    /// prover could not discharge, in machine and obligation order;
    /// `unestablished` one per use of storage that some path reaches with
    /// nothing established in it; `conflicts` one per loan that may not be
    /// formed; `exceeded` one per call outside its caller's service reach;
    /// `formations` one per invalid bound calling shape. At least one list
    /// is not empty.
    Rejected {
        /// Calling-shape failures known before any body is elaborated.
        formations: Vec<crate::CallableFormationFailure>,
        /// One per rule of the language the program breaks as written,
        /// found while its typed trees were read.
        violations: Vec<RuleViolation>,
        unproved: Vec<UnprovedObligation>,
        unestablished: Vec<UnestablishedUse>,
        /// One per loan that may not be formed where it is.
        conflicts: Vec<ConflictingUse>,
        /// One per call that reaches a service outside its caller's reach.
        exceeded: Vec<ExceededReach>,
    },
    /// Every remaining obligation needs a missing proof rule or more search
    /// budget. Each diagnostic records which condition stopped the prover.
    Incomplete(Vec<UnprovedObligation>),
    /// The route does not serve some construct or obligation of the closure,
    /// so the whole closure is refused as not supported yet. The first such
    /// refusal.
    FallBack(Refusal),
    /// The compilation's mathematical signature: the kernel checked every
    /// declaration once and judged every law. Reported after every closure.
    Laws(Box<crate::laws::SignatureJudgment>),
    /// Laws of that signature the checker could not prove, one entry per
    /// law. The compilation is refused.
    UnprovedLaws(Vec<crate::laws::UnprovedLaw>),
}

pub use super::publication::AcceptedClosure;
pub use checked_program::{CheckedPublicationModule, PublicationError, RoutePublication};
use checked_program_issuer::CHECKER;

impl AcceptedClosure {
    /// The advisory diagnostics of this accepted closure, in machine order.
    /// An advisory is information about a program the route accepted: it is
    /// no part of the verdict, and nothing is proved or pruned because of
    /// it (`wiki/spec/language/state_contracts.md`, "Unreachable branches":
    /// "This is an advisory diagnostic, not an inherent language error").
    ///
    /// An expression without a normal result causes one warning at the first
    /// following statement and one note at that expression. Calls and actual
    /// uninhabited values use the same continuation rule (state_contracts.md,
    /// "Published no-return").
    pub fn advisories(&self) -> Vec<Diagnostic> {
        AcceptedProgram {
            elaboration: &self.elaboration,
            discharge: &self.discharge,
        }
        .advisories()
    }
}

/// One accepted program with the discharge that accepted it. The root closure
/// handed to publication keeps its program in a `CheckedProgram`, so its
/// advisories are read through this view like any other closure's.
pub(crate) struct AcceptedProgram<'a> {
    pub(crate) elaboration: &'a Elaboration,
    pub(crate) discharge: &'a Discharge,
}

impl<'program> AcceptedProgram<'program> {
    pub(crate) fn advisories(&self) -> Vec<Diagnostic> {
        let impossible_arms = self.impossible_arms();
        let located = |diagnostic: Diagnostic, origin: SourceOrigin| match origin {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
        };
        let mut advisories = Vec::new();
        for (machine, unreachable) in self.elaboration.unreachable_statements() {
            let owner = describe::owner(self.elaboration.names.get(machine), unreachable.call);
            advisories.push(located(
                Diagnostic::warning(format!(
                    "unreachable statement in {owner}: the preceding expression cannot complete normally, so this statement and every statement after it never execute"
                )),
                unreachable.statement,
            ));
            advisories.push(located(
                Diagnostic::note("this expression cannot complete normally"),
                unreachable.call,
            ));
        }
        // Each authored arm no arrival can take is one warning at its
        // condition, naming the arm and the premises the condition
        // contradicts (state_contracts.md, "Unreachable branches": "The
        // diagnostic identifies the branch condition and the conflicting
        // premises").
        for (machine, branches, impossible) in impossible_arms {
            if impossible.is_empty() {
                continue;
            }
            let names = self.elaboration.names.get(machine);
            for (branch, impossible) in impossible_authored_arms(branches, impossible) {
                let arm = match (names, branch.arm) {
                    (Some(names), SourceOrigin::Authored(span)) => names
                        .arms
                        .iter()
                        .find(|(at, _)| *at == span)
                        .map(|(_, label)| label.clone()),
                    _ => None,
                }
                .unwrap_or_else(|| String::from("an arm"));
                let owner = describe::owner(names, branch.condition);
                let premises: Vec<String> = impossible
                    .premises
                    .iter()
                    .map(|premise| describe::premise(self.elaboration, machine, *premise))
                    .collect();
                let reason = if premises.is_empty() {
                    String::from("its condition can never hold")
                } else {
                    format!(
                        "its condition contradicts the facts in force here: {}",
                        premises.join(", ")
                    )
                };
                advisories.push(located(
                    Diagnostic::warning(format!(
                        "unreachable branch: {arm} in {owner} can never run; {reason}"
                    )),
                    branch.condition,
                ));
            }
        }
        // A `_` arm after arms naming every declared case of the subject is
        // an authored branch no arrival takes: lowering left it out, and it
        // is one warning at the arm (state_contracts.md, "Unreachable
        // branches").
        for (machine, arm) in self.elaboration.dead_wildcards() {
            let owner = describe::owner(self.elaboration.names.get(machine), arm);
            advisories.push(located(
                Diagnostic::warning(format!(
                    "unreachable branch: the `_` arm in {owner} can never run; an earlier arm names every declared case"
                )),
                arm,
            ));
        }
        for (_, names) in self.elaboration.names.iter() {
            for (arm, earlier) in &names.shadowed_patterns {
                advisories.push(Diagnostic::warning(
                    "unreachable branch: an earlier unconditional pattern already matches this arm"
                ).with_source_span(*arm));
                advisories.push(
                    Diagnostic::note("this earlier pattern already matches these values")
                        .with_source_span(*earlier),
                );
            }
        }
        advisories
    }

    /// The impossible arms of each authored machine with branches, in machine
    /// order. Each machine runs its own refutation searches over its own
    /// deferred inputs, and a large program has about a thousand of them, so
    /// the searches run on the invocation's workers. Outside a budget they
    /// stay on this thread: Rayon's global pool has small stacks.
    fn impossible_arms(&self) -> Vec<(MachineId, &'program [Branch], &'program [ImpossibleArm])> {
        let (elaboration, discharge): (&'program Elaboration, &'program Discharge) =
            (self.elaboration, self.discharge);
        let search = |(machine, branches): (MachineId, &'program [Branch])| {
            let discharge = discharge.machines.get(machine)?;
            let tables = elaboration.tables.machines.get(machine)?;
            let lowered = elaboration.module.machine(machine)?;
            // Advisory searches count toward no verdict, so their activity
            // is not reported.
            let mut activity = crate::ProofSearchActivity::default();
            let impossible =
                discharge
                    .branch_warnings
                    .materialize(lowered, tables, branches, &mut activity);
            Some((machine, branches, impossible))
        };
        let machines: Vec<_> = self.elaboration.authored_branches().collect();
        if worker_budget::current_workers() == 1 || machines.len() < 2 {
            return machines.into_iter().filter_map(search).collect();
        }
        use rayon::prelude::*;
        // Indexed collection keeps machine order; one machine per job keeps
        // the few expensive machines stealable.
        let searched: Vec<_> = machines
            .into_par_iter()
            .with_max_len(1)
            .map(search)
            .collect();
        searched.into_iter().flatten().collect()
    }
}

/// A separate invocation entry and a loop head can lower the same source arm
/// twice. Warn once for that arm only if neither occurrence can execute.
fn impossible_authored_arms<'a>(
    branches: &'a [Branch],
    impossible: &'a [ImpossibleArm],
) -> impl Iterator<Item = (&'a Branch, &'a ImpossibleArm)> {
    let mut by_block: Vec<_> = impossible.iter().collect();
    by_block.sort_unstable_by_key(|arm| arm.block);
    let refutation = move |block| {
        by_block
            .binary_search_by_key(&block, |arm| arm.block)
            .ok()
            .map(|position| by_block[position])
    };
    let mut excluded: std::collections::HashSet<_> = branches
        .iter()
        .filter(|branch| refutation(branch.block).is_none())
        .filter_map(|branch| branch.arm.span())
        .collect();
    branches.iter().filter_map(move |branch| {
        let impossible = refutation(branch.block)?;
        if branch.arm.span().is_some_and(|span| !excluded.insert(span)) {
            return None;
        }
        Some((branch, impossible))
    })
}

#[cfg(test)]
#[test]
fn a_source_arm_with_two_entries_requires_two_refutations_and_warns_once() {
    use semantic_vocabulary::BlockId;
    use source_text::{SourceId, SourceSpan, Span};

    let source = SourceOrigin::Authored(SourceSpan::new(SourceId(1), Span::new(4, 8)));
    let blocks = [BlockId::new(2).unwrap(), BlockId::new(7).unwrap()];
    let branches = blocks.map(|block| Branch {
        block,
        decided_in: BlockId::new(1).unwrap(),
        arm: source,
        condition: source,
    });
    let impossible = blocks.map(|block| ImpossibleArm {
        block,
        premises: Vec::new(),
    });
    assert_eq!(
        impossible_authored_arms(&branches, &impossible[..1]).count(),
        0
    );
    assert_eq!(
        impossible_authored_arms(&branches, &impossible[1..]).count(),
        0
    );
    assert_eq!(impossible_authored_arms(&branches, &impossible).count(), 1);
}

impl AcceptedClosure {
    pub fn prepare_publication(&self) -> Result<CheckedPublicationModule, PublicationError> {
        checked_program::prepare_accepted_publication(
            CHECKER,
            &self.elaboration,
            &self.seals,
            self.optimization.as_ref(),
        )
    }

    pub fn publish(&self) -> Result<RoutePublication, PublicationError> {
        self.prepare_publication()?.publish()
    }

    pub fn publish_invocation(
        &self,
    ) -> Result<RoutePublication<terminal_codec::InvocationTerminalCode>, PublicationError> {
        self.prepare_publication()?.publish_invocation()
    }
}

/// Reading a callable states its declaration but does not accept its
/// body. Publish that status only for the same discharged machine and
/// declaration this closure owns. This neither establishes its input
/// premises nor admits a later invocation under them.
fn accept_proof_callables(elaboration: &mut Elaboration, discharge: &Discharge) -> Checked<()> {
    use terminal_check_tables::{Construct, RefusalReason};
    use terminal_psi::{ProofCallableAssociation, ProofScopeDeclaration, TargetMachineCallable};

    let bodies_source = &elaboration.module.machines;
    let scope = &mut elaboration.module.scalars.scalar_applications.proof_scope;
    let invalid = |machine, detail: &str| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Machine,
            detail,
        )
    };
    let first = bodies_source
        .first()
        .map(|body| body.id)
        .unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
    let mut bodies = scope
        .callables
        .iter()
        .filter_map(|(_, association)| match association {
            ProofCallableAssociation::Callable {
                declaration,
                callable: TargetMachineCallable::Machine(machine),
            } => Some((*declaration, *machine)),
            _ => None,
        })
        .collect::<Vec<_>>();
    bodies.sort_unstable_by_key(|(declaration, _)| *declaration);
    if bodies.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err(invalid(
            first,
            "proof callable has competing accepted body associations",
        ));
    }
    let mut headers = scope
        .callable_headers
        .iter()
        .map(|(handle, header)| (header.declaration, handle))
        .collect::<Vec<_>>();
    headers.sort_unstable_by_key(|(declaration, _)| *declaration);
    if headers.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err(invalid(
            first,
            "proof callable has competing declaration headers",
        ));
    }
    for (declaration, _) in &bodies {
        if headers
            .binary_search_by_key(declaration, |(declaration, _)| *declaration)
            .is_err()
        {
            return Err(invalid(
                first,
                "accepted proof body has no exact declaration header",
            ));
        }
    }
    for (declaration, handle) in headers {
        let header = scope.callable_headers.get_mut(handle);
        let Ok(position) =
            bodies.binary_search_by_key(&declaration, |(declaration, _)| *declaration)
        else {
            if header.supplies_checked_body {
                return Err(invalid(
                    first,
                    "proof callable claims an accepted body without this closure's machine association",
                ));
            }
            continue;
        };
        let machine = bodies[position].1;
        let body = usize::try_from(machine.get() - 1)
            .ok()
            .and_then(|position| bodies_source.get(position))
            .filter(|body| body.id == machine)
            .ok_or_else(|| invalid(first, "proof callable lost its accepted source declaration"))?;
        let accepted = body
            .declaration
            .as_ref()
            .ok_or_else(|| invalid(first, "proof callable lost its accepted source declaration"))?;
        if discharge.machines.get(machine).is_none() {
            return Err(invalid(
                first,
                "proof callable has no discharged body in this closure",
            ));
        }
        let position = header
            .declaration
            .0
            .checked_sub(1)
            .and_then(|position| usize::try_from(position).ok())
            .ok_or_else(|| invalid(first, "proof callable has no declaration coordinate"))?;
        if !matches!(scope.declarations.storage_slice().get(position), Some(ProofScopeDeclaration::Callable(saved)) if saved == accepted)
            || header.callable.as_ref().ok() != Some(&accepted.callable)
        {
            return Err(invalid(
                first,
                "proof callable differs from this closure's accepted declaration",
            ));
        }
        // The invocation may execute only through this call. Its outstanding
        // result cannot supply a checked body for compiler evaluation.
        if body.blocks.iter().any(|block| {
            matches!(
                block.terminator,
                terminal_psi::Terminator::PendingNormalCompletion { .. }
            )
        }) {
            if header.supplies_checked_body {
                return Err(invalid(
                    machine,
                    "an unfinished Build suffix claims to supply a checked callable body",
                ));
            }
            continue;
        }
        header.supplies_checked_body = true;
        if let Ok(facts) = &mut header.compiler_evaluation {
            // The accepted ranking covers every reachable local cycle and
            // has discharged every rank comparison. Separate machine-call
            // cycles still need their exact invocation-edge proof.
            facts.local_terminates |= body.ranked_scc.is_some();
        }
    }
    Ok(())
}

/// One obligation the prover could not discharge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnprovedObligation {
    pub machine: MachineId,
    pub obligation: ObligationId,
    pub kind: ObligationKind,
    /// Where the obligation arises in the source, from elaboration's own
    /// source correspondence.
    pub site: SourceOrigin,
    pub cause: UnprovedCause,
    /// The prover's or kernel's own account. Diagnostic text only.
    pub detail: String,
    /// What was not proved, using the source correspondence (`describe`):
    /// for example "cannot prove requires contract
    /// for call halve from run: count > 1". Diagnostic text only.
    pub statement: String,
    /// For a data type's clause that was not proved again where an open
    /// invariant window is consumed: that consumption point. `site` is then
    /// the write that opened the window.
    /// `wiki/spec/language/dependent_values.md`: "An unclosed window rejects
    /// with both the opening write or storage cast and the failing
    /// consumption point."
    pub consumed: Option<ConsumptionSite>,
}

/// `statement` ending with a period.
fn closed_sentence(statement: &str) -> std::borrow::Cow<'_, str> {
    if statement.ends_with('.') {
        std::borrow::Cow::Borrowed(statement)
    } else {
        std::borrow::Cow::Owned(format!("{statement}."))
    }
}

/// Where an open invariant window is consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumptionSite {
    /// The operation, edge or block that consumes the value.
    pub at: Construct,
    /// Where that construct arises in the source.
    pub site: SourceOrigin,
}

impl UnprovedObligation {
    /// The message: what was not proved, then why, in parentheses: the
    /// cause and the prover's own note. For an unclosed window it says so,
    /// and that the clause was left unproved by a prior write, never that it
    /// is false.
    pub fn message(&self) -> String {
        match self.consumed {
            // Keep the unproved condition a complete sentence, with its
            // search cause separate from the authored obligation.
            None => format!(
                "{} ({}: {})",
                closed_sentence(&self.statement),
                self.cause.describe(),
                self.detail
            ),
            Some(_) => format!(
                "{} inside an open default-domain invariant window: a prior write left the clause \
                 unproved and this use consumes the value before it is proved again ({}: {})",
                self.statement,
                self.cause.describe(),
                self.detail
            ),
        }
    }

    /// The `Reject` diagnostic, for an entry of `RouteVerdict::Rejected`.
    /// An entry of `Incomplete` must not be published through it: see the
    /// module comment. For an unclosed window it is placed at the failing
    /// consumption point, and `opening_diagnostic` names the write.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self
            .consumed
            .map_or(self.site, |consumed| consumed.site)
            .span()
        {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }

    /// For an unclosed window, the second diagnostic of the rejection: the
    /// write that opened it, at that write.
    pub fn opening_diagnostic(&self) -> Option<Diagnostic> {
        self.consumed?;
        let diagnostic = Diagnostic::error(format!(
            "this write opens the invariant window: it is not shown to keep {}",
            describe_kind(self.kind)
        ));
        Some(match self.site.span() {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        })
    }
}

/// One use of storage that is not established on every path to it: the
/// zeroed storage of a gated type, a payload of a case the sum does not
/// hold, or a dispatch with no arm for a declared case.
/// `wiki/spec/language/dependent_values.md`: "Otherwise the type is gated:
/// construction or checked qualification must establish the domain before
/// observation."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnestablishedUse {
    pub machine: MachineId,
    /// The operation, edge or block that uses the storage.
    pub at: Construct,
    /// Where that construct arises in the source. `None` only when
    /// elaboration itself refused the closure, so no source correspondence
    /// exists to look the construct up in; a position is never guessed.
    pub site: Option<SourceOrigin>,
    /// The judgment's own account. Diagnostic text only.
    pub detail: String,
    /// When the use observes the zeroed storage of a gated type: where that
    /// storage was declared, the write that opened its establishment debt.
    pub declared: Option<SourceOrigin>,
}

impl UnestablishedUse {
    /// The message, opening with a stable part corpus expectations can match.
    pub fn message(&self) -> String {
        format!("unestablished storage: {}", self.detail)
    }

    /// The `Reject` diagnostic.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site.and_then(SourceOrigin::span) {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

/// One language rule the program breaks. The source reader or lowered-form
/// checker found it from its own checked inputs; this records the retained
/// source position and rule without a separate scalar proof obligation. `wiki/spec/build/compiler_request.md`, "Outcomes": "Diagnostics
/// distinguish failed source checking, invalid proof, unavailable evidence
/// and receiving-policy refusal"; this is failed source checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleViolation {
    pub machine: MachineId,
    /// Where the program breaks the rule; `None` when the authored node
    /// carries no span. A position is never guessed.
    pub site: Option<source_text::SourceSpan>,
    /// The rule and how the program breaks it. Diagnostic text only.
    pub statement: String,
}

impl RuleViolation {
    pub fn message(&self) -> String {
        self.statement.clone()
    }

    /// The `Reject` diagnostic.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

/// One call outside a ceiling its caller publishes: it reaches a service
/// outside the caller's service reach, or may enter a callback outside the
/// caller's `invokes`. A compile error of the program with no certificate:
/// effects.md, "A derived reach that exceeds its ceiling rejects at the call
/// that contributes the excess. The diagnostic names that call, the
/// identities its requirement declares, and the ceiling."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceededReach {
    pub axis: crate::ceilings::Axis,
    pub machine: MachineId,
    /// The call operation or edge.
    pub at: Construct,
    pub site: Option<SourceOrigin>,
    /// Diagnostic text only.
    pub statement: String,
}

impl ExceededReach {
    pub fn message(&self) -> String {
        match self.axis {
            crate::ceilings::Axis::Reach => format!("{} (service reach)", self.statement),
            crate::ceilings::Axis::Entries => {
                format!("{} (synchronous invocation)", self.statement)
            }
            crate::ceilings::Axis::Suspension => {
                format!("{} (suspension)", self.statement)
            }
            crate::ceilings::Axis::Blocking => format!("{} (blocking)", self.statement),
            crate::ceilings::Axis::Termination => {
                format!("{} (termination)", self.statement)
            }
        }
    }

    /// The `Reject` diagnostic.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site.and_then(SourceOrigin::span) {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

fn exceeded_reach(
    elaboration: &Elaboration,
    excess: crate::ceilings::Excess,
) -> Checked<ExceededReach> {
    let module = &elaboration.module;
    let permissions = |row: &[crate::ceilings::Permission]| {
        if row.is_empty() {
            return "none".to_owned();
        }
        row.iter()
            .map(|entry| match entry {
                crate::ceilings::Permission::Entry(
                    terminal_psi::TerminalCallableInvocation::Parameter(position),
                ) => {
                    format!("parameter {}", u64::from(*position) + 1)
                }
                crate::ceilings::Permission::Entry(
                    terminal_psi::TerminalCallableInvocation::Service(service),
                ) => module
                    .interfaces.services
                    .iter()
                    .find(|declaration| declaration.id == *service)
                    .and_then(|declaration| module.identities.text(declaration.identity))
                    .unwrap_or("an unnamed service")
                    .to_owned(),
                crate::ceilings::Permission::MaySuspend => "suspension".to_owned(),
                crate::ceilings::Permission::MayBlock => "blocking".to_owned(),
                crate::ceilings::Permission::MayDiverge => "nontermination".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" + ")
    };
    let machine_name = |machine: MachineId| {
        elaboration
            .names
            .get(machine)
            .map(|names| names.machine.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("a machine")
    };
    let callee = match excess.callee {
        terminal_check_tables::edges::Callee::Declared(
            semantic_vocabulary::TargetMachineCallable::Machine(callee),
        ) => machine_name(callee).to_owned(),
        terminal_check_tables::edges::Callee::Declared(
            semantic_vocabulary::TargetMachineCallable::Boundary(callee),
        ) => module
            .boundary_machine(callee)
            .and_then(|declaration| module.identities.text(declaration.identity))
            .unwrap_or("a boundary requirement")
            .to_owned(),
        terminal_check_tables::edges::Callee::Selected { callable } => {
            format!("evaluated callable value {}", callable.get())
        }
    };
    let ceiling_description = match excess.axis {
        crate::ceilings::Axis::Reach => {
            format!("reaches `{}`", permissions(&excess.ceiling))
        }
        crate::ceilings::Axis::Entries => {
            format!("invokes `{}`", permissions(&excess.ceiling))
        }
        crate::ceilings::Axis::Suspension => "permits no suspension".to_owned(),
        crate::ceilings::Axis::Blocking => "permits no blocking".to_owned(),
        crate::ceilings::Axis::Termination => "guarantees termination".to_owned(),
    };
    let site = match elaboration.sources.get(excess.machine) {
        Some(sources) => Some(source_of(sources, excess.machine, excess.at)?),
        None => None,
    };
    Ok(ExceededReach {
        axis: excess.axis,
        machine: excess.machine,
        at: excess.at,
        site,
        statement: format!(
            "machine `{}` {ceiling_description} but its call of `{callee}` contributes `{}`, which adds `{}`",
            machine_name(excess.machine),
            permissions(&excess.contributes),
            permissions(&excess.outside),
        ),
    })
}

/// One loan that may not be formed where it is: a call that lends storage
/// another of its arguments lends with an incompatible access, or an
/// exclusive reborrow of storage held shared. A compile error of the program
/// of the same standing as an unproved obligation; it is a resource judgment
/// with no certificate (`places::loans`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictingUse {
    pub machine: MachineId,
    /// The row of a conflicting formation. A successor forwards its existing loans.
    pub loan: Option<ObligationId>,
    pub conflict: crate::LoanConflict,
    /// Where the call that forms it arises in the source.
    pub site: SourceOrigin,
    /// What the conflict is, naming the callee and binding when retained.
    /// Diagnostic text only.
    pub statement: String,
}

impl ConflictingUse {
    pub fn message(&self) -> String {
        format!("{} (conflicting loans)", self.statement)
    }

    /// The `Reject` diagnostic.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site.span() {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

/// The statement of one conflict: "state
/// `two` receives `y` as mutable more than once", where `two` is the
/// callee as declared. The callee's name and the lent binding come from the
/// reader's names; without them the statement names no binding.
fn conflict_statement(
    elaboration: &Elaboration,
    machine: MachineId,
    tables: &terminal_check_tables::MachineTables,
    site: SourceOrigin,
    conflict: &crate::ConflictingLoan,
) -> String {
    let callee = conflict
        .invocation
        .and_then(|invocation| tables.invocations.get(invocation))
        .and_then(|row| match row.callee {
            terminal_check_tables::edges::Callee::Declared(
                semantic_vocabulary::TargetMachineCallable::Machine(callee),
            ) => Some(callee),
            terminal_check_tables::edges::Callee::Selected { .. }
            | terminal_check_tables::edges::Callee::Declared(
                semantic_vocabulary::TargetMachineCallable::Boundary(_),
            ) => None,
        })
        .and_then(|callee| elaboration.names.get(callee))
        .map(|names| names.machine.clone())
        .filter(|state| !state.is_empty());
    let lender = site.span().and_then(|span| {
        elaboration
            .names
            .get(machine)
            .and_then(|names| names.lender(span, conflict.position))
            .map(str::to_owned)
    });
    let how = match conflict.conflict {
        crate::LoanConflict::TwiceExclusive => "as mutable more than once",
        crate::LoanConflict::ExclusiveAndShared => "as both mutable and read-only",
        crate::LoanConflict::ExclusiveOfShared => {
            "as mutable through a shared reference, which grants no mutation"
        }
        crate::LoanConflict::ObserveWhileExclusive => {
            return "storage is observed outside its live exclusive reference".into();
        }
        crate::LoanConflict::ChangeWhileBorrowed => {
            return "storage is changed while a conflicting reference remains live".into();
        }
        crate::LoanConflict::ReleaseBeforeChildren => {
            return "reference release precedes its complete child-loan closure".into();
        }
        crate::LoanConflict::AccessExceedsParent => {
            return "reborrow exceeds the access of its actual source path".into();
        }
    };
    match (callee, lender) {
        (Some(state), Some(lender)) => format!("state `{state}` receives `{lender}` {how}"),
        (Some(state), None) => format!("state `{state}` receives one place {how}"),
        (None, Some(lender)) => format!("a borrow uses `{lender}` {how}"),
        (None, None) => format!("a borrow uses one place {how}"),
    }
}

/// The obligation kind as a diagnostic names it. Clause positions count from
/// one, as the source does.
fn describe_kind(kind: ObligationKind) -> String {
    match kind {
        ObligationKind::ExecutableMembership { .. } => String::from("retained domain membership"),
        ObligationKind::IntegerWidthPositive { .. } => String::from("positive integer width"),
        ObligationKind::OperationSafety { .. } => String::from("operation safety"),
        ObligationKind::TheoremRequirement { clause, .. } => {
            format!("premise {} of the supplied theorem", u64::from(clause) + 1)
        }
        ObligationKind::CallRequirement { clause, .. } => {
            format!(
                "requires clause {} of the called machine",
                u64::from(clause) + 1
            )
        }
        ObligationKind::NormalGuarantee { clause, .. } => {
            format!(
                "ensures clause {} on a normal return",
                u64::from(clause) + 1
            )
        }
        ObligationKind::PublishedGuarantee { clause } => {
            format!("ensures clause {}", u64::from(clause) + 1)
        }
        ObligationKind::DataDomain { clause, .. } => {
            format!("requires clause {} of a data type", u64::from(clause) + 1)
        }
        ObligationKind::ArrivalInvariant { .. } => String::from("block invariant"),
        ObligationKind::CrashCoverage { .. } => String::from("crash coverage"),
        ObligationKind::WellFoundedness { .. } | ObligationKind::RankComparison { .. } => {
            String::from("termination")
        }
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. } => String::from("conserved obligation"),
    }
}

/// Discharges an elaborated closure and returns the route's verdict on it.
/// It takes elaboration's result as is, so an elaboration refused as
/// `Unsupported` falls back like an undecided obligation does.
pub fn judge(elaborated: Checked<Elaboration>) -> Checked<RouteVerdict> {
    judge_owing(elaborated, AppliedContracts::Undecided)
}

/// Who decides bodyless requirement rows. Applied generic helpers have
/// actual bodies and always receive the ordinary verdict in this form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppliedContracts {
    /// No independent implementation witness is available. A missing body
    /// keeps its ordinary refusal; no unchecked guarantee is assumed.
    Undecided,
    /// Exact implementation witnesses in this compilation decide abstract
    /// requirement bodies. Their callers still prove the complete contract.
    OwnCheckForms,
}

pub(crate) fn judge_owing(
    elaborated: Checked<Elaboration>,
    applied: AppliedContracts,
) -> Checked<RouteVerdict> {
    judge_owing_with_activity(
        elaborated,
        applied,
        &mut crate::ProofSearchActivity::default(),
    )
}

pub(super) fn judge_owing_with_activity(
    elaborated: Checked<Elaboration>,
    applied: AppliedContracts,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<RouteVerdict> {
    let mut further = Vec::new();
    match examine(elaborated, &[], None, &[], activity, &mut further) {
        Ok(examined) => conclude(examined, applied, true, &mut further),
        Err(decided) => decided,
    }
}

/// A closure lowered and discharged, before its verdict is drawn.
struct Examined {
    elaboration: Elaboration,
    opaque_storage: terminal_psi::OpaqueStorageShapes,
    discharged: Discharge,
    /// Uses of owned storage after its value was transferred or consumed,
    /// found by the affine check before anything is discharged.
    affine_unestablished: Vec<UnestablishedUse>,
}

/// Runs the affine check and the discharge on one closure. `Err` holds the
/// verdict when one is reached before every obligation has a verdict: an
/// elaboration refused, a closure that still lists generic machines, or a
/// discharge refused whole. `carried` are the machines of the closure stage
/// 04 generated from a generic machine whose check form is already
/// discharged (`discharge_carrying`).
///
/// `further` receives every machine refused as not supported after the one
/// the verdict names, in machine order.
fn examine(
    elaborated: Checked<Elaboration>,
    carried: &[crate::Carried<'_>],
    donors: Option<&crate::reuse::Donors<'_>>,
    laws: &[crate::laws::LawJudgment],
    activity: &mut crate::ProofSearchActivity,
    further: &mut Vec<Refusal>,
) -> Result<Examined, Checked<RouteVerdict>> {
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refused) => return Err(fall_back_or_defect(refused)),
    };
    let opaque_storage = elaboration
        .module
        .take_opaque_storage_shapes()
        .map_err(|reason| {
            Err(refusal(
                RefusalReason::ForeignIdentity,
                MachineId::new(1).expect("1 is nonzero"),
                Construct::Machine,
                reason,
            ))
        })?;
    // A compilation's generic machines are judged as their own check forms
    // (`judge_compilation`). A closure that still lists them was judged
    // without them, so it decides nothing.
    if !elaboration.generic_templates.is_empty() {
        let first = MachineId::new(1).expect("1 is nonzero");
        return Err(Ok(RouteVerdict::FallBack(refusal(
            RefusalReason::Unsupported,
            first,
            Construct::Machine,
            "machine generic machines not judged as their own check forms",
        ))));
    }
    // Likewise the rows of the subject's implementations, each judged as
    // its own refinement witness.
    if !elaboration.conformance_witnesses.is_empty() {
        let first = MachineId::new(1).expect("1 is nonzero");
        return Err(Ok(RouteVerdict::FallBack(refusal(
            RefusalReason::Unsupported,
            first,
            Construct::Machine,
            "implementation rows without their refinement witnesses",
        ))));
    }
    crate::generic_storage::declared(&elaboration).map_err(fall_back_or_defect)?;
    crate::nominal_calls::validate(&elaboration).map_err(fall_back_or_defect)?;
    crate::static_reach::validate(&elaboration).map_err(fall_back_or_defect)?;
    crate::owned_cleanup::materialize(&mut elaboration).map_err(fall_back_or_defect)?;
    let mut affine_unestablished = Vec::new();
    let mut verified_borrow_ends = Vec::new();
    // A machine the affine check cannot judge yet is one refusal; the check
    // goes on with the next machine, and the first such refusal becomes the
    // verdict with the rest beside it.
    let mut unsupported = Unsupported::default();
    // The candidate search and every machine's walks replay the same
    // callees; the module replays each of them once for all of them.
    let summaries = terminal_semantics::plain_affine::AffineSummaries::new(&elaboration.module);
    let reference_ends =
        terminal_semantics::reference_ends::candidates_sharing(&elaboration.module, &summaries)
            .map_err(|error| {
            fall_back_or_defect(
                Roster::of(&elaboration).locate(refusal(
                    RefusalReason::Unsupported,
                    error.machine,
                    error
                        .operation
                        .map(Construct::Operation)
                        .unwrap_or(Construct::Machine),
                    &error.detail,
                )),
            )
        })?;
    // Plans are computed on the workers, then read in machine order: the
    // first deciding refusal and the noted machines are those one pass meets.
    let plans = plan_affine_machines(&elaboration.module, &summaries, &reference_ends);
    drop(summaries);
    for (machine, planned) in plans {
        let plan = match planned {
            Some(Ok(plan)) => plan,
            Some(Err(AffineRefusal::Unsupported(refused))) => {
                unsupported.note(machine, refused);
                continue;
            }
            Some(Err(AffineRefusal::Refused(refused))) => {
                return Err(fall_back_or_defect(
                    Roster::of(&elaboration).locate(refused),
                ));
            }
            Some(Err(AffineRefusal::Decided(verdict))) => return Err(verdict),
            // A machine is skipped only after an earlier one decided the
            // verdict, so reading reaches no skipped machine.
            None => {
                return Err(Err(refusal(
                    RefusalReason::ForeignIdentity,
                    machine,
                    Construct::Machine,
                    "the affine check skipped a machine without an earlier refusal",
                )));
            }
        };
        let sources = elaboration.sources.get(machine).ok_or_else(|| {
            Err(refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "affine machine has no source correspondence",
            ))
        })?;
        if plan.unavailable.is_empty() && !plan.borrowed_header_ends.is_empty() {
            verified_borrow_ends.push((machine, plan.borrowed_header_ends));
        }
        for unavailable in plan.unavailable {
            let at = match unavailable.site {
                terminal_semantics::plain_affine::Site::Operation(operation) => {
                    Construct::Operation(operation)
                }
                terminal_semantics::plain_affine::Site::Edge(edge) => Construct::Edge(edge),
            };
            affine_unestablished.push(UnestablishedUse {
                machine,
                at,
                site: Some(source_of(sources, machine, at).map_err(Err)?),
                detail: unavailable.message().to_owned(),
                declared: None,
            });
        }
    }
    if let Some(first) = unsupported.verdict(Roster::of(&elaboration), further) {
        return Err(Ok(RouteVerdict::FallBack(first)));
    }
    // The established ownership frontier belongs to this elaboration.
    // Replay adds logical call requirements and guarantees without changing
    // the structural operations, selected edges, or their cleanup schedules.
    verified_borrow_ends.sort_unstable_by_key(|(machine, _)| *machine);
    match crate::discharge_inner(
        &mut elaboration,
        carried,
        laws,
        Some(&verified_borrow_ends),
        donors,
        activity,
    ) {
        Ok(discharged) => Ok(Examined {
            elaboration,
            opaque_storage,
            discharged,
            affine_unestablished,
        }),
        Err(refused) => Err(fall_back_or_defect(
            Roster::of(&elaboration).locate(refused),
        )),
    }
}

/// Why one machine has no affine plan.
enum AffineRefusal {
    /// The check cannot judge this machine yet. Later machines are still
    /// checked, and the first such machine becomes the verdict.
    Unsupported(Refusal),
    /// The machine is refused at a construct the closure's roster locates.
    Refused(Refusal),
    /// The verdict as the check reports it, without locating it.
    Decided(Checked<RouteVerdict>),
}

impl AffineRefusal {
    fn of(refused: Refusal) -> Self {
        if refused.reason == RefusalReason::Unsupported {
            Self::Unsupported(refused)
        } else {
            Self::Refused(refused)
        }
    }

    /// Whether this refusal decides the verdict, so that no later machine's
    /// plan can be read.
    fn decides(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }
}

/// The affine plan of every machine of `module`, in machine order. Each plan
/// reads only the shared module and that machine's reference-end candidates,
/// so the machines are planned on the invocation's workers. A machine after
/// one whose refusal decides the verdict may be skipped (`None`); reading in
/// machine order returns at that refusal before it reaches a skipped machine.
fn plan_affine_machines(
    module: &terminal_psi::TerminalModule,
    summaries: &terminal_semantics::plain_affine::AffineSummaries<'_>,
    reference_ends: &arena::IdTable<MachineId, terminal_semantics::reference_ends::Candidates>,
) -> Vec<(
    MachineId,
    Option<Result<terminal_semantics::plain_affine::Plan, AffineRefusal>>,
)> {
    let plan =
        |machine: MachineId| affine_plan(module, summaries, machine, reference_ends.get(machine));
    let machines = &module.machines;
    if worker_budget::current_workers() == 1 || machines.len() < 2 {
        let mut plans = Vec::with_capacity(machines.len());
        for machine in machines {
            let planned = plan(machine.id);
            let decided = planned.as_ref().is_err_and(AffineRefusal::decides);
            plans.push((machine.id, Some(planned)));
            if decided {
                break;
            }
        }
        return plans;
    }
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let first_decided = AtomicUsize::new(usize::MAX);
    machines
        .par_iter()
        .enumerate()
        .with_max_len(1)
        .map(|(position, machine)| {
            if position > first_decided.load(Ordering::Relaxed) {
                return (machine.id, None);
            }
            let planned = plan(machine.id);
            if planned.as_ref().is_err_and(AffineRefusal::decides) {
                first_decided.fetch_min(position, Ordering::Relaxed);
            }
            (machine.id, Some(planned))
        })
        .collect()
}

/// The affine plan of `machine`: its dead stored references each have an
/// explicit last-use disposition, and its owned values are used affinely.
/// Both walks read one descriptor graph built from `machine`.
fn affine_plan(
    module: &terminal_psi::TerminalModule,
    summaries: &terminal_semantics::plain_affine::AffineSummaries<'_>,
    machine: MachineId,
    candidates: Option<&terminal_semantics::reference_ends::Candidates>,
) -> Result<terminal_semantics::plain_affine::Plan, AffineRefusal> {
    let candidates = candidates.ok_or_else(|| {
        AffineRefusal::Decided(Err(refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Machine,
            "reference disposition lost its machine",
        )))
    })?;
    let walks =
        terminal_semantics::plain_affine::MachineAffine::new(module, machine, Some(summaries))
            .map_err(|error| AffineRefusal::of(affine_refusal(machine, error)))?;
    let ends = walks
        .plan_reference_ends(candidates)
        .map_err(|error| AffineRefusal::of(affine_refusal(machine, error)))?;
    if !ends.reference_ends.is_empty() {
        return Err(AffineRefusal::Decided(fall_back_or_defect(refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Machine,
            "a dead stored reference has no explicit last-use disposition",
        ))));
    }
    walks
        .verify()
        .map_err(|error| AffineRefusal::of(affine_refusal(machine, error)))
}

/// The refusal of `machine` that an affine-check error states.
fn affine_refusal(machine: MachineId, error: terminal_semantics::plain_affine::Error) -> Refusal {
    let concerns = match error.site() {
        Some(terminal_semantics::plain_affine::Site::Operation(at)) => Construct::Operation(at),
        Some(terminal_semantics::plain_affine::Site::Edge(at)) => Construct::Edge(at),
        None => Construct::Machine,
    };
    let (reason, detail) = match error {
        terminal_semantics::plain_affine::Error::Unavailable(unavailable) => {
            (RefusalReason::Unestablished, unavailable.message())
        }
        terminal_semantics::plain_affine::Error::Unconsumed { at_join, .. } => (
            RefusalReason::Violation,
            terminal_semantics::plain_affine::Error::unconsumed_message(at_join),
        ),
        terminal_semantics::plain_affine::Error::Unsupported { detail, .. } => {
            (RefusalReason::Unsupported, detail)
        }
        terminal_semantics::plain_affine::Error::Malformed { detail, .. } => {
            (RefusalReason::ForeignIdentity, detail)
        }
    };
    refusal(reason, machine, concerns, detail)
}

/// The verdict on an examined closure. A machine with an obligation the
/// checker cannot judge yet falls back; each later such machine is put in
/// `further` (`Unsupported`).
fn conclude(
    examined: Examined,
    applied: AppliedContracts,
    seal_operations: bool,
    further: &mut Vec<Refusal>,
) -> Checked<RouteVerdict> {
    let Examined {
        mut elaboration,
        opaque_storage,
        mut discharged,
        affine_unestablished,
    } = examined;
    let mut unsupported = Unsupported::default();
    let mut unproved = Vec::new();
    let mut unestablished = affine_unestablished;
    let mut conflicts = Vec::new();
    for (machine, result) in discharged.machines.iter() {
        if applied == AppliedContracts::OwnCheckForms && elaboration.requirements.contains(&machine)
        {
            continue;
        }
        let tables = terminal_check_tables::row(
            &elaboration.tables.machines,
            machine,
            machine,
            Construct::Machine,
        )?;
        let sources = elaboration.sources.get(machine).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "machine has no source correspondence",
            )
        })?;
        if result.verdicts.len() != tables.obligations.len() {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "verdicts do not cover the machine's obligations one to one",
            ));
        }
        for (obligation, verdict) in result.verdicts.iter() {
            let Err(refused) = verdict else {
                continue;
            };
            // A loan's conflict is listed with what it conflicts with, below.
            if refused.reason == RefusalReason::Conflict
                && result
                    .conflicts
                    .iter()
                    .any(|conflict| conflict.loan == obligation)
            {
                continue;
            }
            let RefusalReason::Unproved(cause) = refused.reason else {
                if refused.reason == RefusalReason::Unsupported {
                    unsupported.note(machine, refused.clone());
                    continue;
                }
                return fall_back_or_defect(Roster::of(&elaboration).locate(refused.clone()));
            };
            let concerns = Construct::Obligation(obligation);
            let row =
                terminal_check_tables::row(&tables.obligations, obligation, machine, concerns)?;
            let site =
                *terminal_check_tables::row(&sources.obligations, obligation, machine, concerns)?;
            // The return edge identifies the failing path. The authored
            // equality identifies the exact claim to underline on that path.
            let claim_site = match row.kind {
                ObligationKind::NormalGuarantee { clause, .. }
                    if elaboration.module.machine(machine)
                        .and_then(|body| body.ensures.get(clause as usize))
                        .is_some_and(|row| matches!(row.proposition, semantic_vocabulary::Proposition::DeclarationIdentity(_))) => {
                    elaboration.names.get(machine).and_then(|names| names.ensures.get(clause as usize))
                        .and_then(|text| text.source).map(SourceOrigin::Authored)
                }
                _ => None,
            };
            unproved.push(UnprovedObligation {
                machine,
                obligation,
                kind: row.kind,
                site: claim_site.unwrap_or(site),
                cause,
                detail: refused.detail.clone(),
                statement: describe::unproved(
                    &elaboration,
                    machine,
                    tables,
                    obligation,
                    row.kind,
                    site,
                ),
                consumed: None,
            });
        }
        // An open window that a consumption point does not close, reported
        // once, at the first consumption point in program order where the
        // clause is not proved again: the specification's "the failing
        // consumption point". Every later one has its own verdict in
        // `MachineDischarge::windows` all the same. One a zero
        // establishment opened is reported with its use, below.
        let mut reported: Vec<ObligationId> = Vec::new();
        for consumed in &result.windows {
            let Err(refused) = &consumed.verdict else {
                continue;
            };
            let cause = match refused.reason {
                RefusalReason::Unproved(_) if reported.contains(&consumed.opened_by) => continue,
                RefusalReason::Unproved(cause) => {
                    reported.push(consumed.opened_by);
                    cause
                }
                RefusalReason::Unestablished => continue,
                RefusalReason::Unsupported => {
                    unsupported.note(machine, refused.clone());
                    continue;
                }
                RefusalReason::NotConserved
                | RefusalReason::Conflict
                | RefusalReason::ConflictingLoan(_)
                | RefusalReason::Violation
                | RefusalReason::ForeignIdentity => {
                    return fall_back_or_defect(Roster::of(&elaboration).locate(refused.clone()));
                }
            };
            let concerns = Construct::Obligation(consumed.opened_by);
            let row = terminal_check_tables::row(
                &tables.obligations,
                consumed.opened_by,
                machine,
                concerns,
            )?;
            let site = *terminal_check_tables::row(
                &sources.obligations,
                consumed.opened_by,
                machine,
                concerns,
            )?;
            unproved.push(UnprovedObligation {
                machine,
                obligation: consumed.opened_by,
                kind: row.kind,
                site,
                cause,
                detail: refused.detail.clone(),
                statement: describe::unproved(
                    &elaboration,
                    machine,
                    tables,
                    consumed.opened_by,
                    row.kind,
                    site,
                ),
                consumed: Some(ConsumptionSite {
                    at: consumed.at,
                    site: source_of(sources, machine, consumed.at)?,
                }),
            });
        }
        for conflict in &result.conflicts {
            let site = match conflict.at {
                Construct::Obligation(_) => *terminal_check_tables::row(
                    &sources.obligations,
                    conflict.loan,
                    machine,
                    conflict.at,
                )?,
                at => source_of(sources, machine, at)?,
            };
            conflicts.push(ConflictingUse {
                machine,
                loan: Some(conflict.loan),
                conflict: conflict.conflict,
                site,
                statement: conflict_statement(&elaboration, machine, tables, site, conflict),
            });
        }
        for used in &result.uses {
            let Err(refused) = &used.verdict else {
                continue;
            };
            if refused.reason == RefusalReason::Unsupported {
                unsupported.note(machine, refused.clone());
                continue;
            }
            if refused.reason != RefusalReason::Unestablished {
                return fall_back_or_defect(Roster::of(&elaboration).locate(refused.clone()));
            }
            let mut declared = None;
            for consumed in &result.windows {
                if consumed.at == used.at
                    && matches!(&consumed.verdict, Err(open) if open.reason == RefusalReason::Unestablished)
                {
                    declared = Some(*terminal_check_tables::row(
                        &sources.obligations,
                        consumed.opened_by,
                        machine,
                        Construct::Obligation(consumed.opened_by),
                    )?);
                    break;
                }
            }
            unestablished.push(UnestablishedUse {
                machine,
                at: used.at,
                site: Some(source_of(sources, machine, used.at)?),
                detail: refused.detail.clone(),
                declared,
            });
        }
    }
    if let Some(first) = unsupported.verdict(Roster::of(&elaboration), further) {
        return Ok(RouteVerdict::FallBack(first));
    }
    let exceeded = match crate::ceilings::judge(&elaboration) {
        Ok(found) => found
            .into_iter()
            .map(|excess| exceeded_reach(&elaboration, excess))
            .collect::<Checked<Vec<_>>>()?,
        Err(refused) => return fall_back_or_defect(Roster::of(&elaboration).locate(refused)),
    };
    if !unestablished.is_empty() || !conflicts.is_empty() || !exceeded.is_empty() {
        return Ok(RouteVerdict::Rejected {
            formations: Vec::new(),
            violations: Vec::new(),
            unproved: without_restated_clauses(unproved),
            unestablished,
            conflicts,
            exceeded,
        });
    }
    if !unproved.is_empty() {
        // Classified after restated clauses are dropped: a published clause
        // fails as `ReturnDoesNotEstablish` even when its return's goal had
        // no rule, and must not turn an `Incomplete` program into `Reject`.
        let unproved = without_restated_clauses(unproved);
        return Ok(
            if unproved.iter().all(|entry| entry.cause.is_incomplete()) {
                RouteVerdict::Incomplete(unproved)
            } else {
                RouteVerdict::Rejected {
                    formations: Vec::new(),
                    violations: Vec::new(),
                    unproved,
                    unestablished: Vec::new(),
                    conflicts: Vec::new(),
                    exceeded: Vec::new(),
                }
            },
        );
    }
    if let Err(refused) = crate::external_providers::publish(&mut elaboration) {
        return fall_back_or_defect(refused);
    }
    if let Err(refused) = crate::static_contracts::publish(&mut elaboration) {
        return fall_back_or_defect(Roster::of(&elaboration).locate(refused));
    }
    let applications = &mut elaboration.module.scalars.scalar_applications;
    let mut pending = Vec::new();
    for (machine, result) in discharged.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            let Ok(crate::Discharged::TargetCondition(condition)) = verdict else {
                continue;
            };
            let owner = match condition.as_ref() {
                crate::TargetCondition::Predicate(condition) => {
                    let terminal_psi::ScalarConditionSubject::Declaration(owner) =
                        condition.subject
                    else {
                        return fall_back_or_defect(refusal(
                            RefusalReason::ForeignIdentity,
                            machine,
                            Construct::Obligation(obligation),
                            "ground predicate lost its declaration owner",
                        ));
                    };
                    applications.conditions.insert_predicate(condition.clone());
                    owner
                }
                crate::TargetCondition::Range(range) => {
                    if let Err(detail) = range.retain(&mut applications.conditions) {
                        return fall_back_or_defect(refusal(
                            RefusalReason::ForeignIdentity,
                            machine,
                            Construct::Obligation(obligation),
                            detail,
                        ));
                    }
                    range.owner
                }
            };
            pending.push(owner);
        }
    }
    pending.sort_unstable();
    pending.dedup();
    applications
        .proof_scope
        .pending_evaluation_conditions
        .insert_many(pending);
    if let Err(refused) = accept_proof_callables(&mut elaboration, &discharged) {
        return fall_back_or_defect(Roster::of(&elaboration).locate(refused));
    }
    // Only a published closure carries operation evidence seals. Generic check
    // forms and conformance witnesses are never published (`publication.rs`
    // retains only their elaboration and discharge for advisories).
    let seals = if seal_operations && elaboration.generic_binders.is_empty() {
        match discharged.operation_seals(&elaboration) {
            Ok(seals) => seals,
            Err(refused) => return fall_back_or_defect(Roster::of(&elaboration).locate(refused)),
        }
    } else {
        Vec::new()
    };
    opaque_storage
        .restore(&mut elaboration.module)
        .map_err(|reason| {
            refusal(
                RefusalReason::ForeignIdentity,
                MachineId::new(1).expect("1 is nonzero"),
                Construct::Machine,
                reason,
            )
        })?;
    if elaboration.module.identities.invocation_scope().is_some() {
        let inferred = std::mem::take(&mut discharged.inferred_results);
        let results = crate::result_summaries::checked_results(
            inferred,
            &elaboration,
            &discharged,
        )?;
        elaboration
            .module
            .identities
            .retain_checked_private_results(results)
            .map_err(|detail| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    MachineId::new(1).expect("one is nonzero"),
                    Construct::Machine,
                    detail,
                )
            })?;
        let inferred = std::mem::take(&mut discharged.inferred_calls);
        let receipts = inferred.checked(&elaboration, &discharged)?;
        elaboration
            .module
            .identities
            .retain_checked_private_inputs(receipts)
            .map_err(|detail| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    MachineId::new(1).expect("one is nonzero"),
                    Construct::Machine,
                    detail,
                )
            })?;
    }
    let pending = elaboration
        .module
        .pending_normal_completions()
        .next()
        .is_some();
    let checked = Box::new(AcceptedClosure {
        elaboration,
        discharge: discharged,
        seals,
        optimization: None,
    });
    Ok(if pending {
        RouteVerdict::AcceptedPrefix(checked)
    } else {
        RouteVerdict::Accepted(checked)
    })
}

/// A compilation refused before a verdict. Binding failures carry their own
/// source identity because no Terminal machine exists at that point.
#[derive(Debug)]
pub enum CompilationRefusal {
    Elaboration(Refusal),
    #[cfg_attr(not(any(test, feature = "test-support")), allow(dead_code))]
    Binding(crate::CallableFormationDefect),
}

/// Judges the complete prepared Terminal compilation without consulting typed
/// source or invoking a preceding stage. Forms are discharged before reuse by
/// their concrete instances; every demanded conformance still gets a verdict.
pub fn judge_compilation(
    input: terminal_check_tables::compilation::TerminalCompilation,
) -> Vec<Result<RouteVerdict, CompilationRefusal>> {
    judge_compilation_with_activity(input, &mut crate::ProofSearchActivity::default())
}

pub(crate) fn judge_compilation_with_activity(
    input: terminal_check_tables::compilation::TerminalCompilation,
    activity: &mut crate::ProofSearchActivity,
) -> Vec<Result<RouteVerdict, CompilationRefusal>> {
    use terminal_check_tables::compilation::TerminalCompilation;
    let mut prepared = match input {
        TerminalCompilation::FormationFailures(formations) => {
            return vec![Ok(RouteVerdict::Rejected {
                formations,
                violations: Vec::new(),
                unproved: Vec::new(),
                unestablished: Vec::new(),
                conflicts: Vec::new(),
                exceeded: Vec::new(),
            })];
        }
        TerminalCompilation::FormationDefect(defect) => {
            return vec![Err(CompilationRefusal::Binding(defect))];
        }
        TerminalCompilation::Prepared(prepared) => prepared,
    };
    match crate::callable_arrivals::check(prepared.arrivals, activity) {
        Ok(unproved) if unproved.is_empty() => {}
        Ok(unproved) => {
            let verdict = if unproved
                .iter()
                .all(|failure| failure.cause.is_incomplete())
            {
                RouteVerdict::Incomplete(unproved)
            } else {
                RouteVerdict::Rejected {
                    formations: Vec::new(),
                    violations: Vec::new(),
                    unproved,
                    unestablished: Vec::new(),
                    conflicts: Vec::new(),
                    exceeded: Vec::new(),
                }
            };
            return vec![Ok(verdict)];
        }
        Err(refused) => {
            return vec![
                judge_owing_with_activity(Err(refused), AppliedContracts::Undecided, activity)
                    .map_err(CompilationRefusal::Elaboration),
            ];
        }
    }
    let mathematics = std::mem::take(&mut prepared.mathematics);
    // The signature is one judgment for the whole compilation, independent
    // of every closure below: no machine's obligations are among its inputs.
    // It goes first because a theorem over indexed data has its guarantees
    // proved there, and a machine that cites the theorem imports them
    // (`Discharged::Law`).
    let signature = (!mathematics.is_empty()).then(|| crate::laws::judge(mathematics));
    let laws = signature
        .as_ref()
        .map_or(&[][..], |judged| judged.theorems.as_slice());
    let mut further = Vec::new();
    // A selection borrows the same discharge that later decides its witness's
    // verdict. Several selections may name one witness; none owns a second
    // copy to prove independently.
    //
    // Witnesses repeat helper bodies: each holds every machine its
    // implementation calls. They are checked in two waves so that a later
    // copy of a helper can take the proofs an earlier copy found
    // (`crate::reuse::Donors`). `crate::reuse::first_wave` picks the first
    // wave from the witnesses' machines; the second wave borrows from it, and
    // the closure then borrows from every witness. Which copy donates is
    // fixed by witness order, never by which worker finished first.
    let held: Vec<Vec<symbols::SymbolHandle>> = prepared
        .witnesses
        .iter()
        .map(|witness| match witness {
            Ok(elaboration) => elaboration
                .machines
                .iter()
                .map(|(_, symbol)| *symbol)
                .collect(),
            Err(_) => Vec::new(),
        })
        .collect();
    let first_wave = crate::reuse::first_wave(&held);
    let (first, second): (Vec<_>, Vec<_>) = prepared
        .witnesses
        .into_iter()
        .enumerate()
        .partition(|(witness, _)| first_wave[*witness]);
    // Generic check forms and the first wave of conformance witnesses share
    // only immutable `laws`, so they run concurrently when workers are
    // available and merge their refusals and activity in deterministic order.
    let (forms, first) =
        if worker_budget::current_workers() == 1 || prepared.forms.is_empty() || first.is_empty() {
            let forms = examine_generic_forms(prepared.forms, laws, activity, &mut further);
            let first = examine_witnesses(first, None, laws, activity);
            (forms, first)
        } else {
            let mut forms_further = Vec::new();
            let mut forms_activity = crate::ProofSearchActivity::default();
            let mut first_activity = crate::ProofSearchActivity::default();
            let (forms, first) = rayon::join(
                || {
                    examine_generic_forms(
                        prepared.forms,
                        laws,
                        &mut forms_activity,
                        &mut forms_further,
                    )
                },
                || examine_witnesses(first, None, laws, &mut first_activity),
            );
            further.extend(forms_further);
            activity.merge(forms_activity);
            activity.merge(first_activity);
            (forms, first)
        };
    let second = {
        let donors = crate::reuse::Donors::new(first.iter().filter_map(|(_, examined, _)| {
            examined
                .as_ref()
                .ok()
                .map(|examined| (&examined.elaboration, &examined.discharged))
        }));
        examine_witnesses(second, Some(&donors), laws, activity)
    };
    // Back in witness order, for the verdicts and the refusals beside them.
    let mut ordered: Vec<_> = first.into_iter().chain(second).collect();
    ordered.sort_by_key(|(witness, _, _)| *witness);
    let mut examined_witnesses = Vec::with_capacity(ordered.len());
    for (_, examined, local_further) in ordered {
        further.extend(local_further);
        examined_witnesses.push(examined);
    }
    let closure_verdict = match prepared.closure {
        Ok(closure) => {
            let witnesses = witnesses(
                &closure,
                &forms,
                &prepared.specializations,
                &prepared.selection_witnesses,
                &examined_witnesses,
            );
            let carried = carried(&closure, &forms, &witnesses, &prepared.specializations);
            let donors = crate::reuse::Donors::new(examined_witnesses.iter().filter_map(
                |examined| {
                    examined
                        .as_ref()
                        .ok()
                        .map(|examined| (&examined.elaboration, &examined.discharged))
                },
            ));
            match examine(Ok(closure), &carried, Some(&donors), laws, activity, &mut further) {
                Ok(examined) => conclude(examined, AppliedContracts::Undecided, true, &mut further),
                Err(decided) => decided,
            }
        }
        Err(refused) => {
            judge_owing_with_activity(Err(refused), AppliedContracts::Undecided, activity)
        }
    };
    let mut verdicts = Vec::with_capacity(1 + forms.len() + examined_witnesses.len());
    verdicts.push(closure_verdict.map_err(CompilationRefusal::Elaboration));
    let conclude_form = |form: GenericForm| {
        let mut local_further = Vec::new();
        let verdict = match form {
            GenericForm::Decided(verdict) => verdict,
            GenericForm::Examined { examined, .. } => conclude(
                examined,
                AppliedContracts::OwnCheckForms,
                false,
                &mut local_further,
            ),
        }
        .map_err(CompilationRefusal::Elaboration);
        (verdict, local_further)
    };
    let conclude_witness = |witness: Result<Examined, Checked<RouteVerdict>>| {
        let mut local_further = Vec::new();
        let verdict = match witness {
            Ok(examined) => {
                conclude(examined, AppliedContracts::Undecided, false, &mut local_further)
            }
            Err(decided) => decided,
        }
        .map_err(CompilationRefusal::Elaboration);
        (verdict, local_further)
    };
    let (form_verdicts, witness_verdicts) =
        if worker_budget::current_workers() == 1 || forms.len() + examined_witnesses.len() < 2 {
            (
                forms.into_iter().map(conclude_form).collect::<Vec<_>>(),
                examined_witnesses
                    .into_iter()
                    .map(conclude_witness)
                    .collect::<Vec<_>>(),
            )
        } else {
            use rayon::prelude::*;
            rayon::join(
                || {
                    forms
                        .into_par_iter()
                        .with_max_len(1)
                        .map(conclude_form)
                        .collect::<Vec<_>>()
                },
                || {
                    examined_witnesses
                        .into_par_iter()
                        .with_max_len(1)
                        .map(conclude_witness)
                        .collect::<Vec<_>>()
                },
            )
        };
    for (verdict, local_further) in form_verdicts {
        further.extend(local_further);
        verdicts.push(verdict);
    }
    for (verdict, local_further) in witness_verdicts {
        further.extend(local_further);
        verdicts.push(verdict);
    }
    // The closure's further refusals come last, in the order reading met
    // them. Each is judged as the closure's own refusal is, so an
    // unsupported construct falls back and a rejection rejects.
    verdicts.extend(
        prepared
            .further
            .into_iter()
            .map(|refused| judge(Err(refused)).map_err(CompilationRefusal::Elaboration)),
    );
    // And every machine judging refused as not supported after the first of
    // its part.
    verdicts.extend(
        further
            .into_iter()
            .map(|refused| Ok(RouteVerdict::FallBack(refused))),
    );
    if let Some(signature) = signature {
        verdicts.push(Ok(match signature.verdict {
            Ok(judged) => RouteVerdict::Laws(Box::new(judged)),
            Err(unproved) => RouteVerdict::UnprovedLaws(unproved),
        }));
    }
    verdicts
}

/// One examined witness, by its position among the compilation's witnesses,
/// with the refusals its examination put beside its verdict.
type ExaminedWitness = (usize, Result<Examined, Checked<RouteVerdict>>, Vec<Refusal>);

/// Examines `witnesses`, each with its position, on the workers when there
/// are several; each may borrow proofs from `donors`. The result is in the
/// order given.
fn examine_witnesses(
    witnesses: Vec<(usize, Checked<Elaboration>)>,
    donors: Option<&crate::reuse::Donors<'_>>,
    laws: &[crate::laws::LawJudgment],
    activity: &mut crate::ProofSearchActivity,
) -> Vec<ExaminedWitness> {
    if worker_budget::current_workers() == 1 || witnesses.len() < 2 {
        return witnesses
            .into_iter()
            .map(|(position, witness)| {
                let mut further = Vec::new();
                let examined = examine(witness, &[], donors, laws, activity, &mut further);
                (position, examined, further)
            })
            .collect();
    }
    use rayon::prelude::*;
    let results: Vec<_> = witnesses
        .into_par_iter()
        .with_max_len(1)
        .map(|(position, witness)| {
            let mut local_activity = crate::ProofSearchActivity::default();
            let mut further = Vec::new();
            let examined = examine(
                witness,
                &[],
                donors,
                laws,
                &mut local_activity,
                &mut further,
            );
            ((position, examined, further), local_activity)
        })
        .collect();
    results
        .into_iter()
        .map(|(examined, local_activity)| {
            activity.merge(local_activity);
            examined
        })
        .collect()
}

/// The machines one judgment refuses as not supported yet, one refusal for
/// each: the first that machine meets, in machine order. The first becomes
/// the verdict and the rest are reported beside it.
#[derive(Default)]
struct Unsupported(Vec<(MachineId, Refusal)>);

impl Unsupported {
    fn note(&mut self, machine: MachineId, refused: Refusal) {
        if self.0.iter().all(|(noted, _)| *noted != machine) {
            self.0.push((machine, refused));
        }
    }

    /// The first refusal, with the rest put in `further`, each named and
    /// placed by `roster`.
    fn verdict(self, roster: Roster<'_>, further: &mut Vec<Refusal>) -> Option<Refusal> {
        let mut refused = self
            .0
            .into_iter()
            .map(|(_, refused)| roster.locate(refused));
        let first = refused.next()?;
        further.extend(refused);
        Some(first)
    }
}

/// One check form of a generic machine, in the order its verdict is listed.
enum GenericForm {
    /// Discharged, its verdict not yet drawn. `template` is the generic
    /// machine it is the form of.
    Examined {
        template: symbols::SymbolHandle,
        examined: Examined,
    },
    /// Its verdict, reached before a discharge: preparation or checking refused.
    Decided(Checked<RouteVerdict>),
}

/// The selection of the closure at `selection`, with its conformance
/// witness and the witness's discharge: the proof that the selected
/// implementation refines the requirement (`crate::lowering::
/// elaborate_witness`).
struct Witnessed<'a> {
    selection: usize,
    examined: &'a Examined,
}

/// The conformance witness of each selection a machine of `closure` makes
/// when that machine was generated from a generic machine with a discharged
/// form. A selection whose witness the route does not lower has none, and
/// the obligations that would cite it are decided as in any closed machine.
fn witnesses<'a>(
    closure: &Elaboration,
    forms: &[GenericForm],
    specializations: &arena::IdTable<MachineId, Option<symbols::SymbolHandle>>,
    selections: &[(usize, usize)],
    examined: &'a [Result<Examined, Checked<RouteVerdict>>],
) -> Vec<Witnessed<'a>> {
    let mut witnessed = Vec::with_capacity(selections.len());
    for &(selection, witness) in selections {
        let Some(selected) = closure.selections.get(selection) else {
            continue;
        };
        if !specializations
            .get(selected.caller)
            .copied()
            .flatten()
            .is_some_and(|template| has_form(forms, template))
        {
            continue;
        }
        let Some(Ok(examined)) = examined.get(witness) else {
            continue;
        };
        witnessed.push(Witnessed {
            selection,
            examined,
        });
    }
    witnessed
}

fn has_form(forms: &[GenericForm], template: symbols::SymbolHandle) -> bool {
    forms.iter().any(|form| {
        matches!(form, GenericForm::Examined { template: declared, .. } if *declared == template)
    })
}

/// Each machine of `closure` stage 04 generated from a generic machine with
/// a discharged form, with the forms it may take its certificates from and,
/// for each form, the selection made for each machine parameter that form
/// names.
fn carried<'a>(
    closure: &Elaboration,
    forms: &'a [GenericForm],
    witnesses: &'a [Witnessed<'a>],
    specializations: &arena::IdTable<MachineId, Option<symbols::SymbolHandle>>,
) -> Vec<crate::Carried<'a>> {
    // Prepare the actual record-binding family once for each immutable check form.
    // Borrow profiles; only the instance membership roster owns copied IDs.
    let forms: Vec<_> = forms
        .iter()
        .filter_map(|form| {
            let GenericForm::Examined { template, examined } = form else {
                return None;
            };
            let mut bindings: Vec<_> = examined
                .elaboration
                .static_schemas
                .iter()
                .flat_map(|schema| &schema.bindings)
                .collect();
            bindings.sort_unstable_by_key(|binding| {
                (
                    binding.parameter.arena_index(),
                    binding.parameter.generation(),
                )
            });
            let mut record_instances: Vec<_> = bindings
                .iter()
                .flat_map(|binding| &binding.members)
                .flat_map(|member| &member.instances)
                .map(|instance| instance.machine)
                .collect();
            record_instances.sort_unstable();
            record_instances.dedup();
            Some((template, examined, bindings, record_instances))
        })
        .collect();
    let mut carried = Vec::new();
    for (specialization, _) in closure.machines.iter() {
        let Some(Some(template)) = specializations.get(specialization) else {
            continue;
        };
        let mut candidates = Vec::new();
        for (declared, examined, bindings, record_instances) in &forms {
            if *declared != template {
                continue;
            }
            let generic = &examined.elaboration;
            let mut selections = Vec::new();
            for witnessed in witnesses {
                let Some(selected) = closure.selections.get(witnessed.selection) else {
                    continue;
                };
                if selected.caller != specialization {
                    continue;
                }
                let record = selected
                    .source_operand
                    .as_ref()
                    .and_then(|operand| operand.static_record.as_ref());
                let requirement = if let Some(record) = record {
                    let parameter = (
                        record.parameter.arena_index(),
                        record.parameter.generation(),
                    );
                    let start = bindings.partition_point(|binding| {
                        (
                            binding.parameter.arena_index(),
                            binding.parameter.generation(),
                        ) < parameter
                    });
                    let end = bindings.partition_point(|binding| {
                        (
                            binding.parameter.arena_index(),
                            binding.parameter.generation(),
                        ) <= parameter
                    });
                    let mut instances = bindings[start..end].iter().filter_map(|binding| {
                        binding.instance(
                            record.parameter,
                            &record.application,
                            selected.declaring_trait,
                            selected.requirement,
                            &selected.value_arguments,
                        )
                    });
                    match (instances.next(), instances.next()) {
                        (Some(instance), None) => Some(instance),
                        _ => None,
                    }
                } else {
                    generic
                        .machines
                        .iter()
                        .find(|(machine, symbol)| {
                            **symbol == selected.requirement
                                && generic.requirements.contains(machine)
                                && generic.value_arguments.get(*machine)
                                    == Some(&selected.value_arguments)
                                && record_instances.binary_search(machine).is_err()
                        })
                        .map(|(machine, _)| machine)
                };
                if let Some(requirement) = requirement {
                    selections.push(crate::Selection {
                        requirement,
                        witness: &witnessed.examined.elaboration,
                        refinement: &witnessed.examined.discharged,
                    });
                }
            }
            candidates.push(crate::CarriedForm {
                generic,
                found: &examined.discharged,
                selections,
            });
        }
        if !candidates.is_empty() {
            carried.push(crate::Carried {
                specialization,
                forms: candidates,
            });
        }
    }
    carried
}

/// The check forms of `templates`, the subject's generic machines, and of
/// every generic machine those forms apply, in the order they are met, each
/// discharged and awaiting its verdict.
///
/// Applied generic helpers retain their actual bodies in the same form.
/// Ordinary component lowering checks every call, tail transfer and ranking
/// edge, and the ordinary verdict checks those bodies. Independent forms still
/// check each declaration universally. Only actual abstract requirements leave
/// their body obligations to exact implementation witnesses.
fn examine_generic_forms(
    forms: Vec<terminal_check_tables::compilation::GenericForms>,
    laws: &[crate::laws::LawJudgment],
    activity: &mut crate::ProofSearchActivity,
    further: &mut Vec<Refusal>,
) -> Vec<GenericForm> {
    let count = forms.len();
    let mut examined = Vec::new();
    for form in forms {
        let template = form.template;
        if form
            .applies
            .as_ref()
            .is_none_or(|applied| applied.iter().any(|position| *position >= count))
        {
            let first = MachineId::new(1).expect("1 is nonzero");
            examined.push(GenericForm::Decided(Err(refusal(
                RefusalReason::ForeignIdentity,
                first,
                Construct::Machine,
                &format!(
                    "generic machine `{}` has an incomplete dependency roster",
                    form.name
                ),
            ))));
            continue;
        }
        match form.forms {
            Ok(forms) => examined.extend(forms.into_iter().map(|form| {
                match examine(Ok(form), &[], None, laws, activity, further) {
                    Ok(examined) => GenericForm::Examined { template, examined },
                    Err(decided) => GenericForm::Decided(decided),
                }
            })),
            Err(refused) => examined.push(GenericForm::Decided(judge_owing_with_activity(
                Err(refused),
                AppliedContracts::Undecided,
                activity,
            ))),
        }
    }
    examined
}

/// Where an operation, edge or block arises in the source, from
/// elaboration's own correspondence; a position is never guessed.
/// Where construct `at` arises in the source, when the correspondence keeps
/// a row for it.
fn site_of(
    sources: &terminal_check_tables::program::MachineSources,
    at: Construct,
) -> Option<SourceOrigin> {
    match at {
        Construct::Operation(operation) => sources.operations.get(operation),
        Construct::Edge(edge) => sources.edges.get(edge),
        Construct::Block(block) => sources.blocks.get(block),
        Construct::Obligation(obligation) => sources.obligations.get(obligation),
        Construct::Machine
        | Construct::Value(_)
        | Construct::Place(_)
        | Construct::Invocation(_) => None,
    }
    .copied()
}

/// The names and source correspondence of one judged compilation part, used
/// to say where a refusal is. A refusal leaves the checker as a diagnostic
/// that a person must be able to find, even inside a dependency such as
/// `std`: `locate` names its machine and, when the refusal has no span of
/// its own, places it at its construct or else at the machine's declaration.
/// Machine identities are local to one part, so a roster serves only the
/// refusals of the elaboration it was taken from.
#[derive(Clone, Copy)]
struct Roster<'a> {
    names: &'a arena::IdTable<MachineId, terminal_check_tables::names::MachineNames>,
    sources: &'a arena::IdTable<MachineId, terminal_check_tables::program::MachineSources>,
}

impl<'a> Roster<'a> {
    fn of(elaboration: &'a Elaboration) -> Self {
        Self {
            names: &elaboration.names,
            sources: &elaboration.sources,
        }
    }

    fn locate(self, mut refused: Refusal) -> Refusal {
        let terminal_check_tables::RefusalSite::Machine(machine, concerns) = refused.site else {
            return refused;
        };
        let names = self.names.get(machine);
        if refused.source_span.is_none() {
            refused.source_span = self
                .sources
                .get(machine)
                .and_then(|sources| site_of(sources, concerns))
                .and_then(SourceOrigin::authored_span)
                .or_else(|| names.and_then(|names| names.declared));
        }
        if let Some(name) = names
            .map(|names| names.machine.as_str())
            .filter(|name| !name.is_empty())
        {
            refused.detail.push_str(&format!(" in machine `{name}`"));
        }
        refused
    }
}

fn source_of(
    sources: &terminal_check_tables::program::MachineSources,
    machine: MachineId,
    at: Construct,
) -> Checked<SourceOrigin> {
    site_of(sources, at).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            at,
            "a use of storage has no source correspondence",
        )
    })
}

fn fall_back_or_defect(refused: Refusal) -> Checked<RouteVerdict> {
    let (reason, site) = (refused.reason, refused.site);
    match (reason, site) {
        (RefusalReason::Unsupported, _) => Ok(RouteVerdict::FallBack(refused)),
        (
            RefusalReason::ConflictingLoan(conflict),
            terminal_check_tables::RefusalSite::Machine(machine, _),
        ) => {
            let site = refused.source_span.map(SourceOrigin::Authored).unwrap_or(
                SourceOrigin::Unpositioned(
                    terminal_check_tables::program::Unpositioned::SpanAbsent,
                ),
            );
            Ok(RouteVerdict::Rejected {
                formations: Vec::new(),
                violations: Vec::new(),
                unproved: Vec::new(),
                unestablished: Vec::new(),
                conflicts: vec![ConflictingUse {
                    machine,
                    loan: None,
                    conflict,
                    site,
                    statement: refused.detail,
                }],
                exceeded: Vec::new(),
            })
        }
        // Elaboration or discharge refused the closure outright because some
        // storage is used unestablished: a rejection of the program, with no
        // source correspondence to look its construct up in.
        (
            RefusalReason::Unestablished,
            terminal_check_tables::RefusalSite::Machine(machine, at),
        ) => Ok(RouteVerdict::Rejected {
            formations: Vec::new(),
            violations: Vec::new(),
            unproved: Vec::new(),
            unestablished: vec![UnestablishedUse {
                machine,
                at,
                site: refused.source_span.map(SourceOrigin::Authored),
                detail: refused.detail,
                declared: None,
            }],
            conflicts: Vec::new(),
            exceeded: Vec::new(),
        }),
        // A language rule failed at the source position retained by the
        // reader or the lowered-form checker.
        (RefusalReason::Violation, terminal_check_tables::RefusalSite::Machine(machine, _)) => {
            Ok(RouteVerdict::Rejected {
                formations: Vec::new(),
                violations: vec![RuleViolation {
                    machine,
                    site: refused.source_span,
                    statement: refused.detail,
                }],
                unproved: Vec::new(),
                unestablished: Vec::new(),
                conflicts: Vec::new(),
                exceeded: Vec::new(),
            })
        }
        _ => Err(refused),
    }
}

/// A published `ensures` clause fails because a return's own guarantee
/// failed, and that return is already reported with its own site. The
/// clause is kept only when no return of it is reported.
fn without_restated_clauses(unproved: Vec<UnprovedObligation>) -> Vec<UnprovedObligation> {
    let restated = |candidate: &UnprovedObligation| {
        matches!(candidate.kind, ObligationKind::PublishedGuarantee { .. })
            && unproved.iter().any(|exit| {
                exit.machine == candidate.machine
                    && matches!(
                        exit.kind,
                        ObligationKind::NormalGuarantee { published, .. }
                            if published == candidate.obligation
                    )
            })
    };
    let keep: Vec<bool> = unproved.iter().map(|entry| !restated(entry)).collect();
    unproved
        .into_iter()
        .zip(keep)
        .filter_map(|(entry, keep)| keep.then_some(entry))
        .collect()
}

#[cfg(test)]
mod unsupported_tests {
    use super::*;

    fn refused(machine: u64, detail: &str) -> Refusal {
        refusal(
            RefusalReason::Unsupported,
            MachineId::new(machine).unwrap(),
            Construct::Machine,
            detail,
        )
    }

    /// One refusal per machine, the first that machine meets, in the order
    /// the machines were judged; the first is the verdict, the rest go on.
    #[test]
    fn each_machine_is_refused_once_and_the_first_is_the_verdict() {
        let mut unsupported = Unsupported::default();
        for (machine, detail) in [(2, "a"), (2, "b"), (5, "c"), (3, "d"), (5, "e")] {
            unsupported.note(MachineId::new(machine).unwrap(), refused(machine, detail));
        }
        let mut further = Vec::new();
        let (names, sources) = (arena::IdTable::default(), arena::IdTable::default());
        let roster = Roster {
            names: &names,
            sources: &sources,
        };
        let first = unsupported.verdict(roster, &mut further).unwrap();
        assert_eq!(first.detail, "a");
        let rest: Vec<_> = further
            .iter()
            .map(|refused| refused.detail.as_str())
            .collect();
        assert_eq!(rest, ["c", "d"]);
        assert!(
            Unsupported::default()
                .verdict(roster, &mut further)
                .is_none()
        );
    }
}
