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
form, or refused whole because the route does not serve some construct in it
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
    /// formed; `formations` one per invalid bound calling shape. At least one
    /// list is not empty.
    Rejected {
        /// Calling-shape failures known before any body is elaborated.
        formations: Vec<crate::CallableFormationFailure>,
        unproved: Vec<UnprovedObligation>,
        unestablished: Vec<UnestablishedUse>,
        /// One per loan that may not be formed where it is.
        conflicts: Vec<ConflictingUse>,
        /// One per call that contributes services its caller's written
        /// `reaches` ceiling leaves out (`service_reach`).
        exceeded: Vec<crate::ExceededReach>,
    },
    /// No obligation is unproved for want of a derivation, but the prover has
    /// no rule for the form of at least one goal, so the route can judge the
    /// program neither way. Every entry's cause is
    /// `UnprovedCause::NoRuleForGoalForm`. Never empty.
    Incomplete(Vec<UnprovedObligation>),
    /// The route does not serve some construct or obligation of the closure,
    /// so the whole closure is refused as not supported yet. The first such
    /// refusal.
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
    pub artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub seals: std::sync::Arc<terminal_codec::EvidenceSeals>,
    pub machines: semantic_vocabulary::IdTable<MachineId, symbols::SymbolHandle>,
    pub boundaries:
        semantic_vocabulary::IdTable<semantic_vocabulary::BoundaryMachineId, symbols::SymbolHandle>,
    pub source_records: Vec<typed_trees::SourceRecordCorrespondence>,
    pub services: Vec<structure_elaboration::SourceServiceCorrespondence>,
    pub calls: Vec<structure_elaboration::SourceCallCorrespondence>,
}

impl RoutePublication {
    /// Uses the same nominal-entry receipt as every other Terminal producer.
    /// A same-spelled machine from another owner cannot satisfy this root.
    pub fn entry_receipt(
        &self,
        declaration: &terminal_psi::PreparedNominalRoot,
        source_signature_identity: [u8; 32],
    ) -> Result<
        terminal_psi::CheckedProgramEntryTerminalReceipt,
        terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError,
    > {
        terminal_codec::entry_receipt::prepare_program_entry_receipt(
            declaration,
            source_signature_identity,
            &self.artifact,
        )
    }
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
        let mut calls = Vec::new();
        for machine in &self.elaboration.module.machines {
            let tables = self
                .elaboration
                .tables
                .machines
                .get(machine.id)
                .ok_or_else(|| {
                    PublicationError::Semantic(format!(
                        "machine {} lost its checked invocation table before publication",
                        machine.id.get(),
                    ))
                })?;
            for invocation in tables.invocations.values() {
                let sources = self.elaboration.sources.get(machine.id).ok_or_else(|| {
                    PublicationError::Semantic(format!(
                        "machine {} lost its invocation source correspondence",
                        machine.id.get(),
                    ))
                })?;
                let origin = match invocation.site {
                    semantic_vocabulary::InvocationSite::Operation(operation) => {
                        sources.operations.get(operation)
                    }
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
                    | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                        sources.edges.get(edge)
                    }
                }
                .ok_or_else(|| {
                    PublicationError::Semantic(format!(
                        "machine {} invocation {:?} has no source correspondence",
                        machine.id.get(),
                        invocation.site,
                    ))
                })?;
                // Source-free calls have no typed occurrence. Their presence
                // grants no source-issued Build request or other companion.
                if let Some(source) = origin.node() {
                    calls.push(structure_elaboration::SourceCallCorrespondence {
                        machine: machine.id,
                        site: invocation.site,
                        owner: origin.owner().ok_or_else(|| {
                            PublicationError::Semantic(
                                "typed invocation lost its original source machine".to_owned(),
                            )
                        })?,
                        source,
                    });
                }
            }
        }
        calls.sort_unstable_by_key(|call| (call.machine, call.site));
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
        Ok(RoutePublication {
            artifact: std::sync::Arc::new(artifact),
            seals: std::sync::Arc::new(seals),
            machines: self.elaboration.machines.clone(),
            boundaries: self.elaboration.boundaries.clone(),
            source_records: self.elaboration.source_records.clone(),
            services: self.elaboration.services.clone(),
            calls,
        })
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
        match self.site.span() {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
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
            terminal_check_tables::edges::Callee::Selected { .. }
            | terminal_check_tables::edges::Callee::Boundary { .. } => None,
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
    judge_owing(elaborated, AppliedContracts::Undecided)
}

/// Who decides the contract rows of the generic machines a generic check
/// form applies (`Elaboration::applications`): `forward<T>` inside
/// `relay<T>` is there as its contract with no body, so the form proves
/// `forward`'s `requires` at the call and relies on its `ensures`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppliedContracts {
    /// Nobody this judgment knows of. Their rows keep the `Unsupported`
    /// verdict the discharge gives every machine with no body, so the form
    /// is undecided: a guarantee nothing proves is never read as held.
    Undecided,
    /// Each applied machine's own check form, judged in the same
    /// compilation with its parameters rigid, so for every type the caller
    /// can pass (`judge_generic_forms`). The rows are that form's
    /// obligations and are not judged a second time here.
    OwnCheckForms,
}

pub(crate) fn judge_owing(
    elaborated: Checked<Elaboration>,
    applied: AppliedContracts,
) -> Checked<RouteVerdict> {
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
    let discharged = match discharge(&mut elaboration, &mut Default::default()) {
        Ok(discharged) => discharged,
        Err(refused) => return fall_back_or_defect(refused),
    };
    let mut unproved = Vec::new();
    let mut unestablished = affine_unestablished;
    let mut conflicts = Vec::new();
    for (machine, result) in discharged.machines.iter() {
        if applied == AppliedContracts::OwnCheckForms && elaboration.applications.contains(&machine)
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
    // Service reach is judged on the module's own rows, whatever the proof
    // obligations came to.
    let exceeded = match crate::service_reach::judge(&elaboration) {
        Ok(exceeded) => exceeded,
        Err(refused) => return fall_back_or_defect(refused),
    };
    if !unestablished.is_empty() || !conflicts.is_empty() || !exceeded.is_empty() {
        return Ok(RouteVerdict::Rejected {
            formations: Vec::new(),
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
        return fall_back_or_defect(refused);
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

/// A compilation refused before a verdict. Binding failures carry their own
/// source identity because no Terminal machine exists at that point.
#[derive(Debug)]
pub enum CompilationRefusal {
    Elaboration(Refusal),
    Binding(crate::CallableFormationDefect),
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
) -> Vec<Result<RouteVerdict, CompilationRefusal>> {
    match crate::check_bound_callable_shapes(typed, compilation.bindings) {
        Ok(formations) if !formations.is_empty() => {
            return vec![Ok(RouteVerdict::Rejected {
                formations,
                unproved: Vec::new(),
                unestablished: Vec::new(),
                conflicts: Vec::new(),
                exceeded: Vec::new(),
            })];
        }
        Ok(_) => {}
        Err(defect) => return vec![Err(CompilationRefusal::Binding(defect))],
    }
    let elaborated = structure_elaboration::elaborate(typed, compilation);
    let (closure, templates) = match elaborated {
        Ok(mut elaboration) => {
            let templates = std::mem::take(&mut elaboration.generic_templates);
            (Ok(elaboration), templates)
        }
        Err(refused) => (Err(refused), Vec::new()),
    };
    let mut verdicts = vec![judge(closure).map_err(CompilationRefusal::Elaboration)];
    verdicts.extend(
        judge_generic_forms(typed, templates)
            .into_iter()
            .map(|verdict| verdict.map_err(CompilationRefusal::Elaboration)),
    );
    verdicts
}

/// The verdicts on the check forms of `templates`, the subject's generic
/// machines, and of every generic machine those forms apply, in the order
/// they are met.
///
/// A generic body may call another generic machine at its own type
/// parameters. That callee is in the caller's form as a contract with no
/// body, and what makes its `ensures` true is the callee's own check form:
/// generics.md, "Static type equality", "An unspecialized body checks every
/// admitted alternative", so one form judged with the callee's parameters
/// rigid proves the contract for every type a caller can name. The applied
/// machine is therefore judged here whichever package declares it
/// (validation_subjects.md: a passed check means "every declaration they
/// depend on is valid"), and the caller's form leaves the callee's rows to
/// it. When the callee's form is rejected or not served, that verdict is in
/// the list and the compilation is rejected or undecided with it.
///
/// A machine that reaches itself through such calls is the exception. Each
/// form on the cycle would take the next one's contract as given, and no
/// form holds the cycle's bodies together, so nothing would judge the cycle:
/// termination.md, "Calls and productive loops", asks that a runtime call
/// cycle stand in tail position and that a `terminates` promise over it be
/// ranked, and `structure_elaboration` judges both only for a cycle among
/// bodies it read. Such a template is refused by name as not supported yet,
/// whatever contract it states: one with no `ensures` has no applied row
/// left undecided, and would otherwise be accepted unjudged.
fn judge_generic_forms(
    typed: &typed_trees::TypedTrees,
    mut templates: Vec<symbols::SymbolHandle>,
) -> Vec<Checked<RouteVerdict>> {
    let mut forms = Vec::new();
    // For each template, the positions in `templates` of what its forms
    // apply; `None` when a form applies a machine with no symbol.
    let mut applies: Vec<Option<Vec<usize>>> = Vec::new();
    let mut next = 0;
    while next < templates.len() {
        let elaborated = structure_elaboration::elaborate_generic_forms(typed, &[templates[next]]);
        let mut applied = Some(Vec::new());
        for form in elaborated.iter().flatten() {
            for application in &form.applications {
                let Some(symbol) = form.machines.get(*application).copied() else {
                    applied = None;
                    continue;
                };
                let position = templates
                    .iter()
                    .position(|template| *template == symbol)
                    .unwrap_or_else(|| {
                        templates.push(symbol);
                        templates.len() - 1
                    });
                if let Some(applied) = &mut applied {
                    applied.push(position);
                }
            }
        }
        applies.push(applied);
        forms.push(elaborated);
        next += 1;
    }
    let mut verdicts = Vec::new();
    for (template, elaborated) in forms.into_iter().enumerate() {
        if reaches_itself(&applies, template) {
            let first = MachineId::new(1).expect("1 is nonzero");
            verdicts.push(Ok(RouteVerdict::FallBack(refusal(
                RefusalReason::Unsupported,
                first,
                Construct::Machine,
                &format!(
                    "generic machine `{}` on a call cycle",
                    typed.symbols.name(templates[template])
                ),
            ))));
            continue;
        }
        match elaborated {
            Ok(forms) => {
                verdicts.extend(
                    forms
                        .into_iter()
                        .map(|form| judge_owing(Ok(form), AppliedContracts::OwnCheckForms)),
                );
            }
            Err(refused) => verdicts.push(judge(Err(refused))),
        }
    }
    verdicts
}

/// Whether `template` applies itself, directly or through the machines it
/// applies. A template whose applications could not all be named counts as
/// doing so: what it reaches is then unknown.
fn reaches_itself(applies: &[Option<Vec<usize>>], template: usize) -> bool {
    let mut seen = vec![false; applies.len()];
    let mut pending = vec![template];
    while let Some(current) = pending.pop() {
        let Some(applied) = applies.get(current).and_then(Option::as_ref) else {
            return true;
        };
        for next in applied {
            if *next == template {
                return true;
            }
            if !std::mem::replace(&mut seen[*next], true) {
                pending.push(*next);
            }
        }
    }
    false
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
            formations: Vec::new(),
            unproved: Vec::new(),
            unestablished: vec![UnestablishedUse {
                machine: refused.machine,
                at: refused.concerns,
                site: None,
                detail: refused.detail,
                declared: None,
            }],
            conflicts: Vec::new(),
            exceeded: Vec::new(),
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
