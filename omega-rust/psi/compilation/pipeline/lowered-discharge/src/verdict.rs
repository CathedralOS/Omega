/*
The route's verdict on one program: the one result a whole-compilation router
takes from the lowered-form route, and the only way it should take one.

`wiki/drafts/designs/check_on_lowered_form.md` ("Route one", Build): "An
obligation the prover cannot discharge is a compile error for the program,
never a slower guarded form." Omega 00 trusts every obligation of the module
it is given and fills evidence only where a seal arrives, so a module with an
undischarged obligation must never reach it: Omega 00 would accept that
obligation with no evidence. `RouteVerdict` therefore has no variant for a
partly proved closure. A closure is either accepted with every obligation
discharged by the kernel and every use of storage established, rejected with
one diagnostic per unproved obligation and per unestablished use, left
incomplete because the prover has no rule for some goal's
form, or handed back whole to Psi 05 and 06 because the route does not
serve some construct in it (`RefusalReason::Unsupported`; the design: "falling
back to Psi 05 and 06 on any `RefusalReason::Unsupported`").

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

`Unsupported` wins over `Unproved`: an unproved obligation in a closure the
route cannot judge whole is the current checker's to decide. That is not a
weaker check; Psi 05 and 06 still judge the closure. Any other refusal is a
defect of the route itself (an identity that does not resolve) and is
returned as `Err`; a router must not accept the program on it.

A use of storage that some path reaches with nothing established in it
(`RefusalReason::Unestablished`) is a compile error of the same standing as an
unproved obligation, but it is a structural judgment with no obligation row,
so a rejection carries the two lists side by side. One such use makes the
verdict `Rejected` even when every unproved obligation is merely incomplete:
the program fails a checking rule whatever the prover could or could not do.

A loan that may not be formed (`RefusalReason::Conflict`: two loans of one
call that may share storage with one of them exclusive, or an exclusive
reborrow of storage held shared) has the same standing, and is listed beside
the other two as a `ConflictingUse`.
*/

use diagnostics::Diagnostic;
use semantic_vocabulary::{MachineId, ObligationId};
use structure_elaboration::{Elaboration, SourceOrigin};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason, UnprovedCause};
use terminal_codec::ObligationEvidenceSeal;

use crate::{Discharge, describe, discharge, refusal};

/// What the route decided for one closure.
#[derive(Debug)]
pub enum RouteVerdict {
    /// Every obligation of every machine was discharged with a certificate
    /// the kernel accepted. `seals` are the operation obligations' evidence
    /// for Omega 00.
    Accepted(Box<AcceptedClosure>),
    /// The program is rejected. `unproved` has one entry per obligation the
    /// prover could not discharge, in machine and obligation order;
    /// `unestablished` one per use of storage that some path reaches with
    /// nothing established in it; `conflicts` one per loan that may not be
    /// formed. At least one of the three is not empty.
    Rejected {
        unproved: Vec<UnprovedObligation>,
        unestablished: Vec<UnestablishedUse>,
        /// One per loan that may not be formed where it is.
        conflicts: Vec<ConflictingUse>,
    },
    /// No obligation is unproved for want of a derivation, but the prover has
    /// no rule for the form of at least one goal, so the route can judge the
    /// program neither way. Every entry's cause is
    /// `UnprovedCause::NoRuleForGoalForm`. Never empty.
    Incomplete(Vec<UnprovedObligation>),
    /// The route does not serve some construct or obligation of the closure,
    /// so Psi 05 and 06 decide the whole closure. The first such refusal.
    FallBack(Refusal),
}

/// A closure the route accepted.
#[derive(Debug)]
pub struct AcceptedClosure {
    pub elaboration: Elaboration,
    pub discharge: Discharge,
    pub seals: Vec<ObligationEvidenceSeal>,
}

/// An accepted closure published as Terminal Psi: the canonical artifact of
/// the module the route checked, and the evidence seals bound to that
/// module's identity, which travel beside it in memory to Omega 00.
#[derive(Debug)]
pub struct RoutePublication {
    pub artifact: terminal_codec::CanonicalTerminalArtifact,
    pub seals: terminal_codec::EvidenceSeals,
}

/// Why an accepted closure could not be published.
#[derive(Debug)]
pub enum PublicationError {
    /// The module names a requirement that stands in for a body: a generic
    /// check form, which is never encoded (generics.md, "Generic bodies use
    /// the abstract requirement envelope").
    GenericCheckForm,
    /// A machine of the module has no declaration identity, for the reason
    /// given (`Elaboration::undeclared`). product.md: "Source-produced
    /// machine bodies retain a portable checked declaration identity".
    Undeclared(MachineId, &'static str),
    /// The module fails a structural check of Terminal semantics.
    Semantic(String),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    Seals(terminal_codec::CodecError),
}

impl AcceptedClosure {
    /// Publishes the module the route checked, with no Psi optimization: the
    /// artifact's execution record names the identity stage, so the module
    /// the kernel judged is byte for byte the module that is encoded, and the
    /// seals, bound to its identity, still name it. This is the same
    /// publication Psi 08 performs (`finalize_terminal_program`), from the
    /// route's module instead of Psi 06's.
    pub fn publish(&self) -> Result<RoutePublication, PublicationError> {
        if !self.elaboration.requirements.is_empty() {
            return Err(PublicationError::GenericCheckForm);
        }
        if let Some((machine, reason)) = self.elaboration.undeclared.first() {
            return Err(PublicationError::Undeclared(*machine, reason));
        }
        let module = self.elaboration.module.clone();
        terminal_semantics::activation_storage::validate_module(&module)
            .map_err(|reason| PublicationError::Semantic(reason.to_owned()))?;
        let identity =
            terminal_codec::terminal_psi_identity(&module).map_err(PublicationError::Seals)?;
        let record = terminal_codec::PsiOptimizationExecutionRecord::new(
            optimization::PsiOptimizationSelections::default(),
            identity,
            identity,
        )
        .map_err(|error| PublicationError::Semantic(format!("{error:?}")))?;
        let seals = terminal_codec::EvidenceSeals::for_module(&module, self.seals.clone())
            .map_err(PublicationError::Seals)?;
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None)
            .map_err(PublicationError::Artifact)?;
        Ok(RoutePublication { artifact, seals })
    }
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
    /// What was not proved, in the current checker's words for the same
    /// obligation (`describe`): for example "cannot prove requires contract
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
            // The statement is a sentence in the current checker's words and
            // closes with its period, as that checker's message does, so a
            // pinned fragment that ends the sentence matches either.
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
        match self.consumed.map_or(self.site, |consumed| consumed.site) {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
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
        Some(match self.site {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
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
        match self.site {
            Some(SourceOrigin::Authored(span)) => diagnostic.with_source_span(span),
            Some(SourceOrigin::Unpositioned(_)) | None => diagnostic,
        }
    }
}

/// One loan that may not be formed where it is: a call that lends storage
/// another of its arguments lends with an incompatible access, or an
/// exclusive reborrow of storage held shared. A compile error of the program
/// of the same standing as an unproved obligation; it is a resource judgment
/// with no certificate (`places::loans`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictingUse {
    pub machine: MachineId,
    /// The `Loan` row that may not be formed.
    pub loan: ObligationId,
    pub conflict: crate::LoanConflict,
    /// Where the call that forms it arises in the source.
    pub site: SourceOrigin,
    /// What the conflict is, in the current checker's words where it has
    /// them. Diagnostic text only.
    pub statement: String,
}

impl ConflictingUse {
    pub fn message(&self) -> String {
        format!("{} (conflicting loans)", self.statement)
    }

    /// The `Reject` diagnostic.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
        }
    }
}

/// The statement of one conflict, in the current checker's words: "state
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
    let callee = tables
        .invocations
        .get(conflict.invocation)
        .and_then(|row| match row.callee {
            terminal_check_tables::edges::Callee::Machine(callee) => Some(callee),
            terminal_check_tables::edges::Callee::Selected { .. } => None,
        })
        .and_then(|callee| elaboration.names.get(callee))
        .map(|names| names.machine.clone())
        .filter(|state| !state.is_empty());
    let lender = match site {
        SourceOrigin::Authored(span) => elaboration
            .names
            .get(machine)
            .and_then(|names| names.lender(span, conflict.position))
            .map(str::to_owned),
        SourceOrigin::Unpositioned(_) => None,
    };
    let how = match conflict.conflict {
        crate::LoanConflict::TwiceExclusive => "as mutable more than once",
        crate::LoanConflict::ExclusiveAndShared => "as both mutable and read-only",
        crate::LoanConflict::ExclusiveOfShared => {
            "as mutable through a shared reference, which grants no mutation"
        }
    };
    match (callee, lender) {
        (Some(state), Some(lender)) => format!("state `{state}` receives `{lender}` {how}"),
        (Some(state), None) => format!("state `{state}` receives one place {how}"),
        (None, Some(lender)) => format!("a call receives `{lender}` {how}"),
        (None, None) => format!("a call receives one place {how}"),
    }
}

/// The obligation kind as a diagnostic names it. Clause positions count from
/// one, as the source does.
fn describe_kind(kind: ObligationKind) -> String {
    match kind {
        ObligationKind::OperationSafety { .. } => String::from("operation safety"),
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
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refused) => return fall_back_or_defect(refused),
    };
    // A compilation's generic machines are judged as their own check forms
    // (`judge_compilation`). A closure that still lists them was judged
    // without them, so it decides nothing.
    if !elaboration.generic_templates.is_empty() {
        let first = MachineId::new(1).expect("1 is nonzero");
        return Ok(RouteVerdict::FallBack(refusal(
            RefusalReason::Unsupported,
            first,
            Construct::Machine,
            "machine generic parameters",
        )));
    }
    let mut affine_unestablished = Vec::new();
    for machine in &elaboration.module.machines {
        let plan = match terminal_semantics::plain_affine::verify(
            machine,
            &elaboration.module.structural_types,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let (reason, detail) = match error {
                    terminal_semantics::plain_affine::Error::Unsupported(detail) => {
                        (RefusalReason::Unsupported, detail)
                    }
                    terminal_semantics::plain_affine::Error::Malformed(detail) => {
                        (RefusalReason::ForeignIdentity, detail)
                    }
                };
                return fall_back_or_defect(refusal(
                    reason,
                    machine.id,
                    Construct::Machine,
                    detail,
                ));
            }
        };
        let sources = elaboration.sources.get(machine.id).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine.id,
                Construct::Machine,
                "affine machine has no source correspondence",
            )
        })?;
        for unavailable in plan.unavailable {
            let at = match unavailable.site {
                terminal_semantics::plain_affine::Site::Operation(operation) => {
                    Construct::Operation(operation)
                }
                terminal_semantics::plain_affine::Site::Edge(edge) => Construct::Edge(edge),
            };
            affine_unestablished.push(UnestablishedUse {
                machine: machine.id,
                at,
                site: Some(source_of(sources, machine.id, at)?),
                detail: "owned storage is used after its value was transferred or consumed"
                    .to_owned(),
                declared: None,
            });
        }
    }
    let discharged = match discharge(&mut elaboration) {
        Ok(discharged) => discharged,
        Err(refused) => return fall_back_or_defect(refused),
    };
    let mut unproved = Vec::new();
    let mut unestablished = affine_unestablished;
    let mut conflicts = Vec::new();
    for (machine, result) in discharged.machines.iter() {
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
                return fall_back_or_defect(refused.clone());
            };
            let concerns = Construct::Obligation(obligation);
            let row =
                terminal_check_tables::row(&tables.obligations, obligation, machine, concerns)?;
            let site =
                *terminal_check_tables::row(&sources.obligations, obligation, machine, concerns)?;
            unproved.push(UnprovedObligation {
                machine,
                obligation,
                kind: row.kind,
                site,
                cause,
                detail: refused.detail.clone(),
                statement: describe::unproved(&elaboration, machine, tables, row.kind, site),
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
                RefusalReason::Unsupported
                | RefusalReason::NotConserved
                | RefusalReason::Conflict
                | RefusalReason::ForeignIdentity => return fall_back_or_defect(refused.clone()),
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
                statement: describe::unproved(&elaboration, machine, tables, row.kind, site),
                consumed: Some(ConsumptionSite {
                    at: consumed.at,
                    site: source_of(sources, machine, consumed.at)?,
                }),
            });
        }
        for conflict in &result.conflicts {
            let concerns = Construct::Obligation(conflict.loan);
            let site = *terminal_check_tables::row(
                &sources.obligations,
                conflict.loan,
                machine,
                concerns,
            )?;
            conflicts.push(ConflictingUse {
                machine,
                loan: conflict.loan,
                conflict: conflict.conflict,
                site,
                statement: conflict_statement(&elaboration, machine, tables, site, conflict),
            });
        }
        for used in &result.uses {
            let Err(refused) = &used.verdict else {
                continue;
            };
            if refused.reason != RefusalReason::Unestablished {
                return fall_back_or_defect(refused.clone());
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
    if !unestablished.is_empty() || !conflicts.is_empty() {
        return Ok(RouteVerdict::Rejected {
            unproved: without_restated_clauses(unproved),
            unestablished,
            conflicts,
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
                    unproved,
                    unestablished: Vec::new(),
                    conflicts: Vec::new(),
                }
            },
        );
    }
    let seals = match discharged.operation_seals(&elaboration) {
        Ok(seals) => seals,
        Err(refused) => return fall_back_or_defect(refused),
    };
    Ok(RouteVerdict::Accepted(Box::new(AcceptedClosure {
        elaboration,
        discharge: discharged,
        seals,
    })))
}

/// The route's verdicts on everything `compilation` claims: the closure of
/// its roots first, then the check form of each generic machine its subject
/// declares, in declaration order, one per alternative for a finite family.
/// generics.md, "Static type equality": "An unspecialized body checks every
/// admitted alternative; one consumer cannot authorize an otherwise invalid
/// generic body." So each generic body is judged as written whether or not
/// the program applies it; the machines stage 04 generated for the
/// program's own applications are closed machines of the closure. The
/// compilation is decided only when every part is.
pub fn judge_compilation(
    typed: &typed_trees::TypedTrees,
    compilation: structure_elaboration::Compilation<'_>,
) -> Vec<Checked<RouteVerdict>> {
    let elaborated = structure_elaboration::elaborate(typed, compilation);
    let (closure, templates) = match elaborated {
        Ok(mut elaboration) => {
            let templates = std::mem::take(&mut elaboration.generic_templates);
            (Ok(elaboration), templates)
        }
        Err(refused) => (Err(refused), Vec::new()),
    };
    let mut verdicts = vec![judge(closure)];
    if !templates.is_empty() {
        match structure_elaboration::elaborate_generic_forms(typed, &templates) {
            Ok(forms) => verdicts.extend(forms.into_iter().map(|form| judge(Ok(form)))),
            Err(refused) => verdicts.push(judge(Err(refused))),
        }
    }
    verdicts
}

/// Where an operation, edge or block arises in the source, from
/// elaboration's own correspondence; a position is never guessed.
fn source_of(
    sources: &structure_elaboration::MachineSources,
    machine: MachineId,
    at: Construct,
) -> Checked<SourceOrigin> {
    let site = match at {
        Construct::Operation(operation) => sources.operations.get(operation),
        Construct::Edge(edge) => sources.edges.get(edge),
        Construct::Block(block) => sources.blocks.get(block),
        Construct::Machine
        | Construct::Value(_)
        | Construct::Place(_)
        | Construct::Obligation(_)
        | Construct::Invocation(_) => None,
    };
    site.copied().ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            at,
            "a use of storage has no source correspondence",
        )
    })
}

fn fall_back_or_defect(refused: Refusal) -> Checked<RouteVerdict> {
    match refused.reason {
        RefusalReason::Unsupported => Ok(RouteVerdict::FallBack(refused)),
        // Elaboration or discharge refused the closure outright because some
        // storage is used unestablished: a rejection of the program, with no
        // source correspondence to look its construct up in.
        RefusalReason::Unestablished => Ok(RouteVerdict::Rejected {
            unproved: Vec::new(),
            unestablished: vec![UnestablishedUse {
                machine: refused.machine,
                at: refused.concerns,
                site: None,
                detail: refused.detail,
                declared: None,
            }],
            conflicts: Vec::new(),
        }),
        RefusalReason::Unproved(_)
        | RefusalReason::NotConserved
        | RefusalReason::Conflict
        | RefusalReason::ForeignIdentity => Err(refused),
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
