#![forbid(unsafe_code)]

// Crate-wide aliases for sibling stages and the shared test fixtures.
#[cfg(test)]
use prepared_typed_trees_to_lowered_program as lowering;
#[cfg(test)]
use source_admission::LoadedSource;
#[cfg(test)]
use source_declarations::declaration_selection;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
#[cfg(test)]
use source_symbol_table::{source_inputs, source_symbols};
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees as typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
#[cfg(test)]
use typed_trees::source_state;
#[cfg(test)]
use typed_trees_to_prepared_typed_trees as preparation;
#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[cfg(test)]
fn prepare_fixture(typed: source_state::TypedTrees) -> preparation::PreparedTypedTrees {
    preparation::prepare_typed_trees(typed, &[], &[]).expect("prepare checker fixture")
}

/*
Discharge on the lowered form: settle every obligation that elaboration
minted. The proof kernel accepts certificates for proof goals. A ground
layout call requirement retains its exact predicate for the selected target
closure; that target must establish it before admitting execution.
(`wiki/drafts/designs/check_on_lowered_form.md`, "Route one";
`wiki/drafts/designs/stage_trust.md`, rule 2.)

Per machine it does five things:

- Checks that the rows it is about to decide are the ones the lowered form
  owes, each at the point of what owes it (`minted`). The lowered form names
  every obligation itself, so a table with a row missing, restated over
  another subject or moved to another point is refused before any proof.
- Fills the `knowledge` table from the lowered form alone: each entry
  `requires` clause at the entry block, each operation's result equation on
  its normal successor (Terminal semantics states it), each field read's
  equation with the one value its place holds on every path to it (`places`,
  the same walk a copy's leaves are resolved by), each `requires` clause of
  a data type that a read of established storage can state on its value,
  and each callee `ensures` clause, under the call's actuals, after the call.
  The checker derives this knowledge from the lowered program it judges.
- Judges initialization: every operation and terminator that reads storage
  gets one verdict, `Unestablished` when some path to it supplies no
  established value for a scalar leaf it reads (`places`). It is a
  structural judgment over the write log and the graph, not an obligation,
  so it has no certificate.
- Follows each invariant window: a store or zero establishment that was not
  shown to keep a data type's clause opens one on its place, the one
  reaching walk finds every consumption point it reaches open, and the
  clause is proved again there by the same prover (`places`,
  `Storage::consumptions`). Zeroed storage of a gated type observed while
  its window is open is `Unestablished`.
- States each obligation's goal over Terminal values: an operation's safety
  goal is Terminal semantics' own canonical goal, a call requirement is the
  callee's `requires` clause under the call's actuals, a normal guarantee is
  this machine's `ensures` clause.
  A data type's `requires` clause is stated over the values its place holds
  once the owing operation completes (`places`, `Storage::domain_goal`).
  Every clause, whoever owns it, is stated over this machine's values by the
  one substitution (`substitute`).
- Passes the knowledge whose program point dominates the obligation's to one
  prover (`prove`), which emits a certificate, and asks the kernel to decide
  it.

A loop head's invariant, the `requires` of the machine lowered beneath it, is
proved on every arrival (`ArrivalInvariant`) and known from the head's entry
on (`arrivals`). Kinds outside those five are refused as `Unsupported`.
*/

// Read the stage from verdict; publication only receives accepted results.
mod publication;
mod verdict;

mod calls {
    pub(crate) mod call_contracts;
    pub(crate) mod callable_arrivals;
    pub(crate) mod entry_inputs;
    pub(crate) mod external_providers;
    pub(crate) mod nominal_calls;
    pub(crate) mod refinement;
    pub(crate) mod result_summaries;
    pub(crate) mod static_contracts;
}
mod effects {
    pub(crate) mod ceilings;
    pub(crate) mod crashes;
    pub(crate) mod operational;
    pub(crate) mod static_reach;
}
mod control_flow {
    pub(crate) mod arrivals;
    pub(crate) mod graph;
    pub(crate) mod joins;
    pub(crate) mod rankings;
}
mod proofs {
    pub(crate) mod equality;
    pub(crate) mod known;
    pub(crate) mod laws;
    pub(crate) mod membership_tests;
    pub(crate) mod prove;
    pub(crate) mod reuse;
    pub(crate) mod theorems;
}
mod storage {
    pub(crate) mod generic_storage;
    pub(crate) mod owned_cleanup;
    pub(crate) mod places;
}
mod obligations {
    pub(crate) mod formation;
    pub(crate) mod machine_batch;
    pub(crate) mod machine_inputs;
    pub(crate) mod minted;
}
mod reports {
    pub(crate) mod branch_warnings;
    pub(crate) mod describe;
}

use calls::{
    call_contracts, callable_arrivals, entry_inputs, external_providers, nominal_calls, refinement,
    result_summaries, static_contracts,
};
use control_flow::{arrivals, graph, joins, rankings};
use effects::{ceilings, crashes, operational, static_reach};
use obligations::{formation, machine_batch, machine_inputs, minted};
use proofs::{equality, known, laws, membership_tests, prove, reuse, theorems};
use reports::{branch_warnings, describe};
use storage::{generic_storage, owned_cleanup, places};

/// Default summand limit per attempted automatic compatibility-proof product
/// expansion (`wiki/spec/build/configuration.md#proof-work-policy`).
pub const DEFAULT_PROOF_INDEX_EXPANSION_TERMS: usize = equality::MAX_SUMMANDS;

/// Proof-work policy admitted by the root Build for checking one Product
/// (`wiki/spec/build/configuration.md#proof-work-policy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofWorkPolicy {
    pub index_expansion_terms: usize,
}

impl ProofWorkPolicy {
    pub fn with_index_expansion_terms(limit: u64) -> Self {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX).max(1);
        Self {
            index_expansion_terms: limit,
        }
    }
}

impl Default for ProofWorkPolicy {
    fn default() -> Self {
        Self {
            index_expansion_terms: DEFAULT_PROOF_INDEX_EXPANSION_TERMS,
        }
    }
}

pub use publication::{
    AcceptedAdvisories, CheckedPublication, judge_publication, judge_publication_with_policy,
};
pub use verdict::judge_compilation_with_policy;

#[cfg(any(test, feature = "test-support"))]
pub use ceilings::Axis as CeilingAxis;
pub use formation::{ReplayScope, prepare_replay};
pub use places::{ConflictingLoan, LoanConflict};
pub use prove::ProofSearchActivity;
pub use rankings::FiniteIntegerRanking;
#[cfg(not(any(test, feature = "test-support")))]
pub use refinement::finalize_callable_refinements;
#[cfg(any(test, feature = "test-support"))]
pub use refinement::{finalize_callable_refinements, prove_callable_refinement};
#[cfg(not(any(test, feature = "test-support")))]
pub use reuse::{Carried, CarriedForm, Selection};
#[cfg(any(test, feature = "test-support"))]
pub use reuse::{
    Carried, CarriedForm, Selection, reverify, reverify_at_call, reverify_specialization,
};
#[cfg(any(test, feature = "test-support"))]
pub use terminal_check_tables::compilation::{
    CallableFormationCause, CallableFormationDefect, CallableFormationFailure,
};
#[cfg(not(any(test, feature = "test-support")))]
pub use terminal_check_tables::compilation::{CallableFormationDefect, CallableFormationFailure};
#[cfg(any(test, feature = "test-support"))]
pub use verdict::{
    AcceptedClosure, CheckedPublicationModule, CompilationRefusal, ConflictingUse, ConsumptionSite,
    ExceededReach, PublicationError, RoutePublication, RouteVerdict, RuleViolation,
    UnestablishedUse, UnprovedObligation, judge, judge_compilation,
};
#[cfg(not(any(test, feature = "test-support")))]
pub use verdict::{
    AcceptedClosure, CheckedPublicationModule, CompilationRefusal, RouteVerdict, UnprovedObligation,
};

#[cfg(test)]
pub(crate) use prove::has_rules_for;

#[cfg(test)]
mod tests;

use arena::IdTable;
use proof_admission::{
    AcceptedFact, CertificateEnvelope, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, EvidenceIdentity, IntegerMathTerm,
    InvocationSite, MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, Destination};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::values::{Justification, KnowledgeRow};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, Refusal, RefusalReason,
    UnprovedCause,
};
use terminal_codec::ObligationEvidenceSeal;
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use graph::Graph;

type VerifiedBorrowEnds = [(MachineId, Vec<(EdgeId, PlaceId)>)];

/// What the kernel accepted for one obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The goal the kernel decided, over this machine's values.
    pub goal: Proposition,
    /// The knowledge rows passed as the certificate's assumptions, by index.
    pub assumptions: Vec<KnowledgeId>,
    /// The knowledge rows passed as its semantic axioms (operation result
    /// equations), by index.
    pub axioms: Vec<KnowledgeId>,
    pub certificate: CertificateEnvelope,
    /// The kernel's acceptance, including whether the mathematical core
    /// judged the certificate or refused a rule family it does not cover.
    pub fact: AcceptedFact,
}

/// A check against scalar applications in the selected target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetCondition {
    Predicate(terminal_psi::ScalarPredicateCondition),
    Range(terminal_semantics::TargetScalarRange),
}

/// What discharged one obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discharged {
    /// Dataflow across all reaching edges established this nominal membership.
    RetainedMembership,
    /// A typed source Identity checked against its actual mathematical term.
    Identity(equality::Judgment),
    /// The result conserves the declared projection of every listed owned entry claim on all normal returns.
    ConservedContent(Vec<semantic_vocabulary::ClaimId>),
    /// A certificate the kernel accepted.
    Certificate(Evidence),
    /// A `NormalGuarantee` of a theorem over data indexed by values. The
    /// theorem's body is an induction over erased evidence, which no machine
    /// runs; the mathematical signature proves the clause for every value of
    /// the machine's formals and the kernel checks that proof
    /// (`laws::prove_family`). A running machine that cites the theorem
    /// calls it and imports this clause at the citation:
    /// `wiki/spec/proofs/contracts.md`, "Citation and induction": "An
    /// ordinary statement call cites a checked theorem. Establish its
    /// `requires` under exact operand substitution before importing its
    /// `ensures`".
    Law(Box<laws::LawJudgment>),
    /// A ground prerequisite still owed in its actual target closure.
    TargetCondition(Box<TargetCondition>),
    /// A `PublishedGuarantee`: the `NormalGuarantee` of every normal return
    /// edge for this clause was discharged by a certificate, and these are
    /// they. Empty when the machine has no normal return, where the clause
    /// holds vacuously.
    EveryExit(Vec<ObligationId>),
    /// Only the existing return edges have been proved. The stated result
    /// guarantee still applies to an invocation prefix's uncompiled suffix.
    PendingNormalGuarantee(Vec<ObligationId>),
    /// A `DataDomain` clause that a store or a zero establishment was not
    /// shown to keep, with why: the write opens an invariant window.
    /// `wiki/spec/language/dependent_values.md`, "Invariant windows": "A
    /// write known to preserve the default domain keeps it established.
    /// Otherwise the write opens a window on the place, whose actual new
    /// contents remain available to ordinary flow reasoning. The domain must
    /// be proved again at the next consumption point". Nothing is owed at
    /// the write, so this is not a refusal; it is not a discharge either.
    /// The clause is owed at every consumption point the window reaches
    /// open, and each of those has its own verdict in
    /// `MachineDischarge::windows`, under this obligation's identity.
    ///
    /// A zero establishment (`let x: T;`) whose zero value does not satisfy
    /// the clause opens the same debt on storage that holds no valid value
    /// yet: "Zeroing storage is permitted independently of establishing a
    /// value." and "If zero satisfies the default domain, the value is
    /// zero-constructible. Otherwise the type is gated: construction or
    /// checked qualification must establish the domain before observation."
    Window(Box<Refusal>),
    /// A `DataDomain` clause of a store that writes no storage the clause
    /// reads, where the clause was not proved over the place's values: the
    /// clause is as true after the store as before it, so the store opens
    /// no window and closes none. dependent_values.md: "A write known to
    /// preserve the default domain keeps it established." It is a judgment
    /// over the clause's names and the stored place (`Storage::untouched`),
    /// with no certificate.
    Preserved,
    /// A `Loan` that may be formed where it is: no other loan of the same
    /// call overlaps it incompatibly, and a reborrow asks no more access
    /// than its parent holds (`places::loans`). loans.md: "Loan descent,
    /// access attenuation, temporal containment and restoration are
    /// resource judgments. Logical facts cannot create, amplify, transfer,
    /// extend, return, consume or duplicate authority." It is a structural
    /// judgment with no certificate, and the loan ends when its call
    /// returns, so it owes nothing after it.
    Lent,
    /// The canonical finite integer relation and its complete authenticated
    /// private bindings. Every internal edge comparison has a separate kernel
    /// certificate; no Truth proposition stands in for well-foundedness.
    FiniteIntegerRanking(FiniteIntegerRanking),
}

impl Discharged {
    pub const fn certificate(&self) -> Option<&Evidence> {
        match self {
            Self::Certificate(evidence) => Some(evidence),
            Self::Identity(_)
            | Self::Law(_)
            | Self::ConservedContent(_)
            | Self::TargetCondition(_)
            | Self::PendingNormalGuarantee(_)
            | Self::EveryExit(_)
            | Self::Window(_)
            | Self::Preserved
            | Self::Lent
            | Self::RetainedMembership
            | Self::FiniteIntegerRanking(_) => None,
        }
    }
}

/// One consumption point an open invariant window reaches, and whether the
/// window's clause was proved again there. dependent_values.md: "The domain
/// must be proved again at the next consumption point" and "An unclosed
/// window rejects with both the opening write or storage cast and the
/// failing consumption point."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowUse {
    /// The `DataDomain` obligation the opening write left unproved
    /// (`Discharged::Window`): the window's identity, and the identity the
    /// certificate is issued under.
    pub opened_by: ObligationId,
    /// The consumption point: the operation, or for a terminator its edge
    /// or block.
    pub at: Construct,
    /// The kernel's acceptance of the clause over what the place holds at
    /// the consumption point. `Err` with reason `Unproved` when a store
    /// opened the window, and with reason `Unestablished` when a zero
    /// establishment did, as the storage then never held a valid value:
    /// either is a compile error of the program.
    pub verdict: Checked<Evidence>,
}

/// One operation or terminator that reads storage, and whether what it
/// reads is established there. The initialization judgment is structural: it
/// has no obligation row, so its verdicts stand beside the obligations'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageUse {
    /// The operation, or for a terminator its edge.
    pub at: Construct,
    /// `Err` with reason `Unestablished` when some path to the use supplies
    /// no established value for a scalar leaf it reads: a compile error of
    /// the program.
    pub verdict: Checked<()>,
}

/// One machine's result.
#[derive(Debug)]
pub struct MachineDischarge {
    /// One verdict per obligation row, indexed by the same `ObligationId`.
    pub verdicts: IdTable<ObligationId, Checked<Discharged>>,
    /// One verdict per use of storage, in block and operation order, a
    /// block's terminator last.
    pub uses: Vec<StorageUse>,
    /// One verdict per consumption point each invariant window reaches
    /// open, in block and operation order.
    pub windows: Vec<WindowUse>,
    /// Joins whose inferred predicate some arrival could not establish. Each
    /// loses its inferred facts; its original declared predicate, if any,
    /// remains mandatory (`joins`).
    pub weakened_joins: Vec<BlockId>,
    /// Each loan that may not be formed, with what it conflicts with. Its
    /// row's verdict is the `Conflict` refusal.
    pub conflicts: Vec<places::ConflictingLoan>,
    /// Prepared warning inputs are consumed only by accepted diagnostics.
    /// They never decide a mandatory obligation or storage/loan judgment.
    branch_warnings: branch_warnings::Deferred,
}

/// An authored transition arm whose condition contradicts the facts in force
/// before its branch, while those facts are consistent:
/// `wiki/spec/language/state_contracts.md`, "Unreachable branches": "A
/// checked contradiction between valid premises can establish that a
/// particular arrival is impossible", and "issue an unreachable-branch
/// warning when an authored branch contradicts its machine's entry contract
/// or established source facts". `premises` names the facts cited by the
/// accepted refutation, in authored order with the incoming condition itself
/// left out. It need not be a minimal explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpossibleArm {
    pub block: BlockId,
    pub premises: Vec<Justification>,
}

impl MachineDischarge {
    /// The uses of storage that are not established: each is a rejection of
    /// the program by this route, as an `Unproved` obligation is.
    pub fn unestablished(&self) -> impl Iterator<Item = &Refusal> {
        self.uses
            .iter()
            .filter_map(|used| used.verdict.as_ref().err())
    }
}

#[derive(Debug)]
pub struct Discharge {
    inferred_calls: entry_inputs::InferredCalls,
    inferred_results: Vec<(MachineId, u32, terminal_psi::ContractClause)>,
    pub machines: IdTable<MachineId, MachineDischarge>,
}

impl Discharge {
    /// One seal per discharged `OperationSafety` obligation, at an instruction
    /// or a pre-edge terminator read. Publication saves the seals, with
    /// their complete certificates, in the artifact's operation evidence
    /// (`ObligationEvidenceSeal`).
    pub fn operation_seals(
        &self,
        elaboration: &Elaboration,
    ) -> Checked<Vec<ObligationEvidenceSeal>> {
        let mut seals = Vec::new();
        for (machine, result) in self.machines.iter() {
            let tables = terminal_check_tables::row(
                &elaboration.tables.machines,
                machine,
                machine,
                Construct::Machine,
            )?;
            for (obligation, verdict) in result.verdicts.iter() {
                let Ok(Discharged::Certificate(evidence)) = verdict else {
                    continue;
                };
                let row = terminal_check_tables::row(
                    &tables.obligations,
                    obligation,
                    machine,
                    Construct::Obligation(obligation),
                )?;
                if !matches!(row.kind, ObligationKind::OperationSafety { .. }) {
                    continue;
                }
                let resolve = |rows: &[KnowledgeId]| {
                    rows.iter()
                        .map(|row| {
                            knowledge_proposition(tables, *row).cloned().ok_or_else(|| {
                                refusal(
                                    RefusalReason::ForeignIdentity,
                                    machine,
                                    Construct::Obligation(obligation),
                                    "evidence cites a knowledge row with no proposition",
                                )
                            })
                        })
                        .collect::<Checked<Vec<_>>>()
                };
                let seal = ObligationEvidenceSeal::seal(
                    machine,
                    obligation,
                    evidence.goal.clone(),
                    resolve(&evidence.assumptions)?,
                    resolve(&evidence.axioms)?,
                    evidence.certificate.clone(),
                )
                .map_err(|error| Refusal {
                    reason: RefusalReason::Unsupported,
                    site: terminal_check_tables::RefusalSite::Machine(
                        machine,
                        Construct::Obligation(obligation),
                    ),
                    source_span: None,
                    detail: format!("evidence encoding {error:?}"),
                })?;
                seals.push(seal);
            }
        }
        Ok(seals)
    }
}

/// Decides every obligation of every machine and fills each machine's
/// `knowledge` table. A refusal here concerns the module itself (an identity
/// that does not resolve, knowledge already present); a refusal of one
/// obligation is its verdict.
/// Branch conditions, dispatch bindings and value relations are ordinary
/// premises. Proof replay uses the same preparation as source acceptance.
pub fn discharge(
    elaboration: &mut Elaboration,
    activity: &mut ProofSearchActivity,
) -> Checked<Discharge> {
    discharge_inner(elaboration, &[], &[], None, activity)
}

/// `discharge`, where each machine of `carried` is one stage 04 generated
/// from a generic machine whose check form was proved once. Such a machine
/// takes each certificate from that form, moved into it, and the kernel
/// decides it against the machine's own goal and hypotheses: no search runs
/// for it (generics.md, "Static type equality": the unspecialized body is
/// checked for every admitted use). An obligation for which no carried
/// proof is accepted (a kind the carrier does not move, or a machine that
/// is not the form's body) is decided as in any closed machine, so carrying
/// never accepts what the closed check would not, and never rejects what it
/// accepts.
#[cfg(any(test, feature = "test-support"))]
pub fn discharge_carrying(
    elaboration: &mut Elaboration,
    carried: &[Carried<'_>],
    activity: &mut ProofSearchActivity,
) -> Checked<Discharge> {
    discharge_inner(elaboration, carried, &[], None, activity)
}

/// Read-only fields of `Elaboration` needed while checking machines and
/// holding `AffineSummaries<'_>` against `module` alongside `&mut CheckTables`.
pub(crate) struct ElaborationView<'a> {
    pub(crate) module: &'a TerminalModule,
    pub(crate) sources: &'a IdTable<MachineId, terminal_check_tables::program::MachineSources>,
    pub(crate) requirements: &'a [MachineId],
    pub(crate) nominal_calls: &'a [terminal_psi::generic_calls::NominalCallArgument],
    pub(crate) schemas: &'a [terminal_check_tables::contracts::StaticSchemaQuestion],
    pub(crate) domain_sources: &'a IdTable<
        semantic_vocabulary::DomainSemanticId,
        terminal_check_tables::program::DomainSource,
    >,
}

pub(crate) struct PreparedDischarge<'l> {
    inhabitation: terminal_semantics::inhabitation::Inhabitation,
    unreached: Unreached,
    law_instances: Vec<MachineId>,
    /// The Identity table of a generic check form with const record
    /// parameters, checked once; `None` for any other module.
    identities: Option<equality::IdentityTables>,
    theorems: Vec<(MachineId, &'l laws::LawJudgment)>,
    inferred_calls: entry_inputs::InferredCalls,
    inferred_results: Vec<(MachineId, u32, terminal_psi::ContractClause)>,
}

pub(crate) fn prepare_discharge<'l>(
    elaboration: &mut Elaboration,
    laws: &'l [laws::LawJudgment],
    identities: Option<&terminal_check_tables::mathematics::IdentityTable>,
    activity: &mut ProofSearchActivity,
) -> Checked<PreparedDischarge<'l>> {
    for boundary in &elaboration.module.interfaces.boundary_machines {
        terminal_semantics::boundary_result_lifetimes(
            &elaboration.module.types.structural_types,
            boundary,
        )
        .and_then(|_| {
            terminal_semantics::references::validate_declared_boundary_reference_outputs(
                &elaboration.module.types.structural_types,
                boundary,
            )
        })
        .map_err(|detail| {
            refusal(
                RefusalReason::Unsupported,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                detail,
            )
        })?;
    }
    terminal_semantics::references::validate_provider_reference_outputs(&elaboration.module)
        .map_err(|detail| {
            refusal(
                RefusalReason::Unsupported,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                detail,
            )
        })?;
    let mut scope = prepare_replay(elaboration, activity)?;
    let elaboration = scope.source();
    minted::width_questions_are_minted(elaboration)?;
    operational::judge(elaboration)?;
    terminal_semantics::nominal_cleanup::validate_normal_returns(&elaboration.module).map_err(
        |detail| {
            refusal(
                RefusalReason::Unsupported,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is nonzero")),
                Construct::Machine,
                detail,
            )
        },
    )?;
    // A caller may retain old contents only outside the callee's actual
    // writes. Check every published frame before summaries or knowledge can
    // use it. Low-level proof reconstruction still states a conditional
    // judgment; this whole-module route owns admission of those premises.
    terminal_semantics::borrowed_storage::validate_write_frames(&elaboration.module).map_err(
        |error| {
            Refusal::at(
                RefusalReason::ForeignIdentity,
                error
                    .machine
                    .map_or(terminal_check_tables::RefusalSite::Program, Into::into),
                error.detail,
            )
        },
    )?;
    // Results before inputs: what a helper returns is a fact its caller
    // may pass on to the next private helper, whose inferred input
    // condition is described from the caller's facts.
    // And once more after inputs: a helper that omits `requires` was
    // summarized under no input, and what it returns because of the input
    // its callers establish is a further clause. A summary already in the
    // contract is not added twice.
    let diagnostic = elaboration
        .module
        .machines
        .first()
        .map(|machine| machine.id)
        .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity"));
    let inhabitation = prepare_inhabitation(&elaboration.module, diagnostic)?;
    let mut unreached = Unreached::new(elaboration);
    scope.infer_results(&mut unreached, &inhabitation, activity)?;
    scope.infer_inputs(&mut unreached, &inhabitation, activity)?;
    scope.infer_results(&mut unreached, &inhabitation, activity)?;
    scope.infer_crash_routes(&mut unreached, &inhabitation, activity)?;
    scope.finalize_callable_refinements(activity)?;
    let mut law_instances = scope
        .source()
        .static_schemas
        .iter()
        .flat_map(|schema| &schema.bindings)
        .flat_map(|binding| &binding.members)
        .flat_map(|member| member.instances.iter().map(|instance| instance.machine))
        .collect::<Vec<_>>();
    law_instances.sort_unstable();
    if !law_instances.is_empty() {
        static_contracts::admit(scope.source())?;
    }
    // Each machine that is the theorem of a proved law, with that law, in
    // machine order.
    let theorems: Vec<(MachineId, &laws::LawJudgment)> = scope
        .source()
        .machines
        .iter()
        .flat_map(|(id, symbol)| {
            laws.iter()
                .filter(move |law| law.theorem == Some(*symbol))
                .map(move |law| (id, law))
        })
        .collect();
    let identities =
        identities.map(|table| equality::check_identity_table(&scope.source().module, table));
    let (inferred_calls, inferred_results) = scope.into_inferred();
    Ok(PreparedDischarge {
        inhabitation,
        unreached,
        law_instances,
        identities,
        theorems,
        inferred_calls,
        inferred_results,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_discharge<'m>(
    view: ElaborationView<'m>,
    checking: &mut terminal_check_tables::CheckTables,
    prepared: PreparedDischarge<'_>,
    carried: &[Carried<'_>],
    verified_borrow_ends: Option<&VerifiedBorrowEnds>,
    affine: &'m terminal_semantics::plain_affine::AffineSummaries<'m>,
    activity: &mut ProofSearchActivity,
) -> Checked<Discharge> {
    let PreparedDischarge {
        inhabitation,
        unreached,
        law_instances,
        identities,
        theorems,
        inferred_calls,
        inferred_results,
    } = prepared;
    let module = view.module;
    let formed = formation::FormedModule::new(
        module,
        view.nominal_calls,
        view.schemas,
        view.domain_sources,
    );
    let inputs = machine_inputs::PreparedMachineInputs::new(
        module,
        view.sources,
        &checking.machines,
        view.requirements,
        carried,
    );
    // Observe the same immutable type family used by inference above. The
    // existing module-scoped exclusion graph serves every machine; only this
    // borrowed declaration view is needed beside it for projected paths.
    let empty_contracts = terminal_semantics::inhabitation::TerminalContracts::new(
        module.types.structural_types.iter(),
    );
    // Each machine's borrowed-storage replay reaches the same callees; the
    // module replays each of them once for all its machines.
    let summaries = ModuleReplays {
        borrowed_storage: terminal_semantics::descriptor_results::ModuleSummaries::borrowed_storage(
            module,
            formed.nominal_calls(),
        ),
        affine,
    };
    let machines =
        machine_batch::check(&mut checking.machines, activity, |id, tables, activity| {
            // A requirement has a contract and no body, so nothing about it is
            // decided here. Its guarantees are each selected implementation's to
            // prove; leaving them without a verdict would read as vacuously held.
            let inputs = inputs.for_machine(id)?;
            if inputs.requirement {
                return Ok(MachineDischarge {
                    verdicts: tables.obligations.map(|obligation, _| {
                        Err(refusal(
                            RefusalReason::Unsupported,
                            id,
                            Construct::Obligation(obligation),
                            "a requirement's guarantee is proved by each implementation, not here",
                        ))
                    }),
                    uses: Vec::new(),
                    windows: Vec::new(),
                    weakened_joins: Vec::new(),
                    conflicts: Vec::new(),
                    branch_warnings: branch_warnings::Deferred::default(),
                });
            }
            let lowered = terminal_machine(&module.machines, id, id)?;
            let beyond = unreached.values(lowered, tables);
            let proved = &theorems[theorems.partition_point(|(machine, _)| *machine < id)
                ..theorems.partition_point(|(machine, _)| *machine <= id)];
            let borrow_ends = verified_borrow_ends.map(|ends| {
                ends.binary_search_by_key(&id, |(machine, _)| *machine)
                    .map_or(&[][..], |position| ends[position].1.as_slice())
            });
            machine(
                formed,
                &inhabitation,
                id,
                tables,
                &beyond,
                inputs.carried.map(|position| &carried[position]),
                inputs.branches,
                equality::LawScope {
                    instances: &law_instances,
                    identities: identities.as_ref(),
                },
                proved,
                &empty_contracts,
                &summaries,
                borrow_ends,
                activity,
            )
        })?;
    Ok(Discharge {
        machines,
        inferred_calls,
        inferred_results,
    })
}

pub(crate) fn publish_rankings(
    module: &mut TerminalModule,
    discharges: &IdTable<MachineId, MachineDischarge>,
) -> Checked<()> {
    for machine in &mut module.machines {
        if machine.ranked_scc.is_none() {
            continue;
        }
        let discharge =
            terminal_check_tables::row(discharges, machine.id, machine.id, Construct::Machine)?;
        rankings::publish_comparisons(machine, discharge)?;
    }
    Ok(())
}

/// `laws` are the laws the mathematical signature proved. A machine of the
/// elaboration that one of them is the theorem of takes that law as the
/// proof of the clause it states (`Discharged::Law`).
pub(crate) fn discharge_inner(
    elaboration: &mut Elaboration,
    carried: &[Carried<'_>],
    laws: &[laws::LawJudgment],
    verified_borrow_ends: Option<&VerifiedBorrowEnds>,
    activity: &mut ProofSearchActivity,
) -> Checked<Discharge> {
    let prepared = prepare_discharge(elaboration, laws, None, activity)?;
    let summaries = terminal_semantics::plain_affine::AffineSummaries::new(&elaboration.module);
    let discharged = finish_discharge(
        ElaborationView {
            module: &elaboration.module,
            sources: &elaboration.sources,
            requirements: &elaboration.requirements,
            nominal_calls: &elaboration.nominal_calls,
            schemas: &elaboration.static_schemas,
            domain_sources: &elaboration.domain_sources,
        },
        &mut elaboration.tables,
        prepared,
        carried,
        verified_borrow_ends,
        &summaries,
        activity,
    )?;
    drop(summaries);
    publish_rankings(&mut elaboration.module, &discharged.machines)?;
    Ok(discharged)
}

/// Values that have no defining operation in the machine that names them and
/// still stand for one the integer rules do not reach (`prove::reach`): a
/// formal whose input condition was inferred from a caller's unreached
/// actual, and the result of a call to a private helper whose body returns
/// an unreached value. A goal that depends on one is `Incomplete` when it is
/// not proved, as it would be were the operation in the same machine.
///
/// Only inferred conditions carry this across a call. A written contract is
/// the program's own claim: an arrival or a return that cannot establish it
/// is judged where the operation is, and a caller that needs more than the
/// contract states is rejected.
pub(crate) struct Unreached {
    formals: IdTable<MachineId, Vec<ValueId>>,
    results: IdTable<MachineId, bool>,
}

impl Unreached {
    fn new(elaboration: &Elaboration) -> Self {
        Self {
            formals: elaboration.tables.machines.map(|_, _| Vec::new()),
            results: elaboration.tables.machines.map(|_, _| false),
        }
    }

    /// The unreached values of the machine `lowered`: its marked formals and
    /// the results of its calls of marked helpers.
    pub(crate) fn values(&self, lowered: &TerminalMachine, tables: &MachineTables) -> Vec<ValueId> {
        let mut values = self.formals.get(lowered.id).cloned().unwrap_or_default();
        for call in tables.invocations.values() {
            if let Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(callee)) =
                call.callee
                && self.results.get(callee) == Some(&true)
                && let InvocationSite::Operation(operation) = call.site
                && let Some(result) = operation_result(lowered, operation)
            {
                values.push(result);
            }
        }
        values
    }

    pub(crate) fn mark_formal(&mut self, machine: MachineId, formal: ValueId) {
        if let Some(formals) = self.formals.get_mut(machine)
            && !formals.contains(&formal)
        {
            formals.push(formal);
        }
    }

    pub(crate) fn mark_result(&mut self, machine: MachineId) {
        if let Some(result) = self.results.get_mut(machine) {
            *result = true;
        }
    }
}

fn refusal(
    reason: RefusalReason,
    machine: MachineId,
    concerns: Construct,
    detail: &str,
) -> Refusal {
    Refusal {
        reason,
        site: terminal_check_tables::RefusalSite::Machine(machine, concerns),
        source_span: None,
        detail: detail.to_owned(),
    }
}

fn terminal_machine(
    machines: &[TerminalMachine],
    id: MachineId,
    from: MachineId,
) -> Checked<&TerminalMachine> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| machines.get(position))
        .filter(|machine| machine.id == id)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                from,
                Construct::Machine,
                "machine is not at its position in the module",
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn machine(
    formed: formation::FormedModule<'_>,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    id: MachineId,
    tables: &mut MachineTables,
    unreached: &[ValueId],
    carried: Option<&Carried<'_>>,
    branches: &[terminal_check_tables::program::Branch],
    law_instances: equality::LawScope<'_>,
    proved: &[(MachineId, &laws::LawJudgment)],
    empty_contracts: &terminal_semantics::inhabitation::TerminalContracts<'_>,
    summaries: &ModuleReplays<'_>,
    borrow_ends: Option<&[(EdgeId, PlaceId)]>,
    activity: &mut ProofSearchActivity,
) -> Checked<MachineDischarge> {
    let module = formed.source();
    terminal_machine(&module.machines, id, id)?;
    if !tables.knowledge.is_empty() {
        return Err(refusal(
            RefusalReason::Unsupported,
            id,
            Construct::Machine,
            "knowledge rows already present",
        ));
    }
    let (prepared, mut discharge) = with_prepared_machine(
        formed,
        id,
        tables,
        inhabitation,
        Some(summaries),
        |prepared, tables| {
            check_prepared_machine(
                prepared,
                inhabitation,
                tables,
                unreached,
                carried,
                law_instances,
                proved,
                empty_contracts,
                borrow_ends,
                activity,
            )
        },
    )?;
    if !branches.is_empty() {
        discharge.branch_warnings =
            branch_warnings::Deferred::new(prepared.graph, prepared.context);
    }
    Ok(discharge)
}

#[allow(clippy::too_many_arguments)]
fn check_prepared_machine(
    mut prepared: PreparedMachine<'_, '_>,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    tables: &mut MachineTables,
    unreached: &[ValueId],
    carried: Option<&Carried<'_>>,
    law_instances: equality::LawScope<'_>,
    proved: &[(MachineId, &laws::LawJudgment)],
    empty_contracts: &terminal_semantics::inhabitation::TerminalContracts<'_>,
    borrow_ends: Option<&[(EdgeId, PlaceId)]>,
    activity: &mut ProofSearchActivity,
) -> Checked<MachineDischarge> {
    let mut decided: Vec<Option<Checked<Discharged>>> =
        (0..tables.obligations.len()).map(|_| None).collect();
    let (module, id) = (prepared.module, prepared.id);
    for (index, verdict) in membership_tests::judge(module, id, prepared.graph, tables)? {
        decided[index] = Some(verdict);
    }
    // Loans are judged by structure, not proved (`places::loans`).
    let (lent, conflicts) =
        prepared
            .storage
            .loans(prepared.references, module, tables, borrow_ends)?;
    for (obligation, verdict) in lent {
        let slot = decided
            .get_mut(usize::try_from(obligation.get() - 1).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Obligation(obligation),
                    "a loan row past the end of this machine",
                )
            })?;
        *slot = Some(verdict.map(|()| Discharged::Lent));
    }
    // Declared block invariants (a loop head's, the `requires` of the
    // machine lowered beneath it) and inferred join predicates are one kind
    // of row: each is known from its block's entry on and is proved on every
    // arrival before it may justify anything downstream (fact_validity.md:
    // "Every arrival must check before the destination predicate can justify
    // downstream obligations."). A declared one holds because every arrival
    // proves it, and the closure is accepted only when every obligation is
    // discharged (`judge`).
    let weakened_joins =
        prepared.check_joins(tables, &mut decided, unreached, law_instances, activity)?;
    let PreparedMachine {
        lowered,
        types,
        graph,
        context,
        ranking_bindings,
        storage,
        ref mut references,
        ..
    } = prepared;
    // Certificates next; a published clause is then decided from its exits.
    // A machine stage 04 generated takes each one its generic form proved,
    // and searches only where none is carried (`discharge_carrying`).
    let carrier = carried.and_then(|carried| carried.carrier(tables, lowered, module));
    let mut layout_conditions = terminal_semantics::layout_call_conditions(module, lowered)
        .map_err(|detail| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                detail,
            )
        })?
        .into_iter()
        .peekable();
    let mut target_ranges = terminal_semantics::target_scalar_ranges(module, lowered, types)
        .map_err(|detail| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                detail,
            )
        })?
        .into_iter()
        .peekable();
    let content_returns = terminal_semantics::content_transformations::forwarding::checked_returns(
        module, lowered,
    )
    .map_err(|detail| refusal(RefusalReason::Unestablished, id, Construct::Machine, detail))?;
    let mut knowledge = KnowledgeIndex::new(graph, tables, id)?;
    for (index, (obligation, row)) in tables.obligations.iter().enumerate() {
        if decided[index].is_some() {
            continue;
        }
        if let ObligationKind::PublishedGuarantee { .. } = row.kind {
            continue;
        }
        if let ObligationKind::WellFoundedness { component } = row.kind {
            decided[index] = Some(
                rankings::well_founded(lowered, ranking_bindings, obligation, component)
                    .map(Discharged::FiniteIntegerRanking),
            );
            continue;
        }
        if layout_conditions
            .peek()
            .is_some_and(|(target, _)| *target == obligation)
            && let Some((_, condition)) = layout_conditions.next()
        {
            if !matches!(row.kind, ObligationKind::CallRequirement { .. }) {
                return Err(refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Obligation(obligation),
                    "layout condition names a different obligation",
                ));
            }
            decided[index] = Some(Ok(Discharged::TargetCondition(Box::new(
                TargetCondition::Predicate(condition),
            ))));
            continue;
        }
        if target_ranges
            .peek()
            .is_some_and(|range| range.obligation == obligation)
            && let Some(range) = target_ranges.next()
        {
            if !matches!(row.kind, ObligationKind::OperationSafety { .. }) {
                return Err(refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Obligation(obligation),
                    "target arithmetic range names a different obligation",
                ));
            }
            decided[index] = Some(Ok(Discharged::TargetCondition(Box::new(
                TargetCondition::Range(range),
            ))));
            continue;
        }
        if let ObligationKind::NormalGuarantee { clause, .. } = row.kind
            && let Ok(index_in_returns) =
                content_returns.binary_search_by_key(&(clause as usize), |&(ordinal, _)| ordinal)
        {
            decided[index] = Some(Ok(Discharged::ConservedContent(
                content_returns[index_in_returns].1.clone(),
            )));
            continue;
        }
        // A theorem over indexed data: the signature proved this clause.
        if let ObligationKind::NormalGuarantee { clause, .. } = row.kind
            && let Some((_, law)) = proved.iter().find(|(_, law)| law.clause == clause)
        {
            decided[index] = Some(Ok(Discharged::Law(Box::new((*law).clone()))));
            continue;
        }
        let verdict = goal(
            module,
            lowered,
            (storage, references),
            id,
            obligation,
            &row.kind,
            tables,
            types,
            ranking_bindings,
        )
        .and_then(|goal| {
            if let Proposition::DeclarationIdentity(clause) = &goal
                && !clause.is_float_meaning()
            {
                return match row.kind {
                    ObligationKind::NormalGuarantee { exit, .. } => {
                        let mut at_exit = storage.at_return(
                            references,
                            tables,
                            exit,
                            Construct::Obligation(obligation),
                        )?;
                        equality::discharge(
                            module,
                            lowered,
                            tables,
                            graph,
                            storage,
                            row.created_at,
                            obligation,
                            clause,
                            law_instances.instances,
                            law_instances.identities,
                            &mut at_exit,
                        )
                        .map(Discharged::Identity)
                    }
                    _ => equality::discharge(
                        module,
                        lowered,
                        tables,
                        graph,
                        storage,
                        row.created_at,
                        obligation,
                        clause,
                        law_instances.instances,
                        law_instances.identities,
                        &mut storage.at_entry(references, tables),
                    )
                    .map(Discharged::Identity),
                };
            }
            if let Some(carry) = &carrier
                && let Ok(evidence) = carry.certify(
                    context,
                    graph,
                    obligation,
                    &row.kind,
                    row.created_at,
                    goal.clone(),
                )
            {
                return Ok(Discharged::Certificate(evidence));
            }
            decide_indexed(
                context,
                graph,
                tables,
                &mut knowledge,
                id,
                obligation,
                row.created_at,
                goal,
                unreached,
                activity,
            )
            .map(Discharged::Certificate)
        });
        decided[index] = Some(verdict);
    }
    rankings::settle_comparisons(
        lowered,
        ranking_bindings,
        &mut decided,
        |obligation, edge, goal| {
            decide_indexed(
                context,
                graph,
                tables,
                &mut knowledge,
                id,
                obligation,
                ProgramPoint::Edge(edge),
                goal,
                unreached,
                activity,
            )
        },
    )?;
    if layout_conditions.next().is_some() || target_ranges.next().is_some() {
        return Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            "target condition lost its checked obligation",
        ));
    }
    // Published guarantees read normal-return verdicts, so finish that last
    // dependency pass before moving any certificate out of this table. Once
    // it is complete, settlement owns each result; no duplicate proof tree
    // or intermediate verdict table is needed.
    let pending_result = !lowered.ensures.is_empty()
        && lowered.blocks.iter().any(|block| {
            matches!(
                block.terminator,
                terminal_psi::Terminator::PendingNormalCompletion { .. }
            )
        });
    for (index, (obligation, row)) in tables.obligations.iter().enumerate() {
        if decided[index].is_none()
            && let ObligationKind::PublishedGuarantee { clause } = row.kind
        {
            let checked = published(tables, id, obligation, clause, &decided);
            decided[index] = Some(match checked {
                Ok(Discharged::EveryExit(exits)) if pending_result => {
                    Ok(Discharged::PendingNormalGuarantee(exits))
                }
                other => other,
            });
        }
    }
    // A store or a zero establishment that was not shown to keep a clause
    // does not fail: it opens an invariant window, and the clause is owed
    // at the consumption points the window reaches (dependent_values.md,
    // "Invariant windows"). A construction makes a value and must prove its
    // clauses where it stands, so its unproved clause stays a refusal.
    let mut windows: Vec<places::Window> = Vec::new();
    let mut restoring: Vec<(places::Owner, OperationId)> = Vec::new();
    let mut settled = IdTable::default();
    for ((obligation, row), verdict) in tables.obligations.iter().zip(decided) {
        let verdict = verdict.unwrap_or_else(|| {
            Err(refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Obligation(obligation),
                "obligation left undecided",
            ))
        });
        let ObligationKind::DataDomain { operation, clause } = row.kind else {
            settled.push(verdict);
            continue;
        };
        let concerns = Construct::Obligation(obligation);
        let stores = storage.store(operation)?;
        let zeroes = storage.zero_establishment(operation)?;
        settled.push(match verdict {
            // data_and_literals.md: "Construction and every restoring write
            // prove the owning default-domain clauses."
            Ok(Discharged::Certificate(evidence)) if stores => {
                let owner = storage.owner(operation, clause, concerns)?;
                restoring.push((
                    storage
                        .capture_owner(references, operation, owner, concerns)?
                        .owner,
                    operation,
                ));
                Ok(Discharged::Certificate(evidence))
            }
            Err(unproved)
                if matches!(unproved.reason, RefusalReason::Unproved(_)) && (stores || zeroes) =>
            {
                let owner = storage.owner(operation, clause, concerns)?;
                if stores && storage.untouched(references, operation, &owner, concerns)? {
                    Ok(Discharged::Preserved)
                } else {
                    let captured = storage.capture_owner(references, operation, owner, concerns)?;
                    windows.push(places::Window {
                        obligation,
                        opened_by: operation,
                        owner: captured.owner,
                        backing: captured.backing,
                        initial: zeroes,
                        restored: Vec::new(),
                    });
                    Ok(Discharged::Window(Box::new(unproved)))
                }
            }
            verdict => verdict,
        });
    }
    for window in &mut windows {
        window.restored = restoring
            .iter()
            .filter(|(owner, _)| *owner == window.owner)
            .map(|(_, operation)| *operation)
            .collect();
    }
    // A store not shown to keep a clause while that clause's window on the
    // same place is already open opens no second window: the window stays
    // open and the store only changes the contents it holds
    // (dependent_values.md: "whose actual new contents remain available to
    // ordinary flow reasoning"). Its consumption points are the first
    // window's, which names the write that opened it. Zeroed storage is the
    // exception: a store into it makes the leaf hold a written value, so a
    // store that does not keep the clause keeps its own window.
    let mut opened: Vec<places::Window> = Vec::with_capacity(windows.len());
    for window in windows {
        let mut within = false;
        for earlier in &opened {
            if !earlier.initial
                && earlier.owner == window.owner
                && storage.open_before(references, tables, earlier, window.opened_by)?
            {
                within = true;
                break;
            }
        }
        if !within {
            opened.push(window);
        }
    }
    let windows = opened;
    let mut uses = storage.uses(references, tables, empty_contracts, inhabitation)?;
    // Byte-domain establishments are judged from their actual source at the
    // producer, including qualified transfers whose result is never consumed.
    for judged in storage.byte_predicate_establishments(module, references)? {
        match uses.iter_mut().find(|used| used.at == judged.at) {
            Some(used) if used.verdict.is_ok() => used.verdict = judged.verdict,
            Some(_) => {}
            None => uses.push(judged),
        }
    }
    let mut closures = Vec::new();
    for consumption in storage.consumptions(references, tables, &windows)? {
        let window = windows.get(consumption.window).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                consumption.at,
                "a consumption names no window",
            )
        })?;
        let proved = consumption.goal.and_then(|goal| {
            decide_indexed(
                context,
                graph,
                tables,
                &mut knowledge,
                id,
                window.obligation,
                consumption.point,
                goal,
                unreached,
                activity,
            )
        });
        let verdict = match proved {
            // Zeroed storage of a gated type that is observed before its
            // domain holds was never an established value: the use itself
            // is unestablished, which is the initialization judgment's
            // verdict on it.
            Err(unproved)
                if window.initial && matches!(unproved.reason, RefusalReason::Unproved(_)) =>
            {
                let unestablished = Refusal {
                    reason: RefusalReason::Unestablished,
                    site: terminal_check_tables::RefusalSite::Machine(id, consumption.at),
                    source_span: None,
                    detail: format!(
                        "zeroed storage of a gated type is used before its requires holds: {}",
                        unproved.detail
                    ),
                };
                let used = uses
                    .iter_mut()
                    .find(|used| used.at == consumption.at)
                    .ok_or_else(|| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            id,
                            consumption.at,
                            "a consumption point is no use of storage",
                        )
                    })?;
                if used.verdict.is_ok() {
                    used.verdict = Err(unestablished.clone());
                }
                Err(unestablished)
            }
            proved => proved,
        };
        closures.push(WindowUse {
            opened_by: window.obligation,
            at: consumption.at,
            verdict,
        });
    }
    Ok(MachineDischarge {
        verdicts: settled,
        uses,
        windows: closures,
        weakened_joins,
        conflicts,
        branch_warnings: branch_warnings::Deferred::default(),
    })
}

/// Fills one machine's knowledge table from its lowered form: entry
/// `requires`, result equations and field-read equations, branch
/// conditions, dispatch bindings and callee guarantees.
fn fill_knowledge<'a>(
    module: &TerminalModule,
    id: MachineId,
    lowered: &TerminalMachine,
    tables: &mut MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    (storage, references): (
        &places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
) -> Checked<()> {
    let edge_entries = owned_cleanup::edge_entry_points(lowered, tables)?;
    theorems::fill(module, id, tables, (storage, &mut *references))?;
    for (clause, proposition) in lowered.contract.requires.iter().enumerate() {
        let mut premises =
            entry_snapshot_conjuncts(proposition, &mut storage.at_entry(references, tables))
                .map_err(|unbound| unbound.refusal(id, Construct::Machine))?;
        // A clause over a shared input's cell is also stated over the input
        // itself, so a call that selects the cell with an equal value of
        // another identity reads the same term (`at_entry_over_shared_inputs`).
        // Both forms state the one clause at entry; nothing new is assumed.
        let over_inputs = entry_snapshot_conjuncts(
            proposition,
            &mut storage.at_entry_over_shared_inputs(references, tables),
        )
        .map_err(|unbound| unbound.refusal(id, Construct::Machine))?;
        for stated in over_inputs {
            if !premises.contains(&stated) {
                premises.push(stated);
            }
        }
        for stated in premises {
            push_knowledge(
                tables,
                stated,
                ProgramPoint::BlockEntry(lowered.entry),
                Justification::EntryRequirement {
                    clause: position(clause, id)?,
                },
            );
        }
    }
    // A sum holds exactly one case at each boundary its case is named at.
    for (point, after, premise) in storage.case_name_premises(tables)? {
        push_knowledge(
            tables,
            premise,
            point,
            match after {
                None => Justification::FormalType,
                Some(call) => Justification::OperationSemantics(call),
            },
        );
    }
    // Each readable record formal of the machine or of a loop head arrived
    // as a member of its type's domain, so the clauses of that type hold of
    // its entry contents, including a clause that relates several fields.
    for (block, premise) in storage.entry_type_premises(references, tables)? {
        push_knowledge(
            tables,
            premise,
            ProgramPoint::BlockEntry(block),
            Justification::FormalType,
        );
    }
    for block in &lowered.blocks {
        for operation in &block.operations {
            if let Some(equation) = result_equation(module, lowered, operation, types)? {
                push_knowledge(
                    tables,
                    equation,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
            // A field read's result is a fresh value; what it equals comes
            // from the write log, not from the operation alone.
            if let Some(equation) = storage.read_equation(references, tables, operation)? {
                push_knowledge(
                    tables,
                    equation,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
            // Storage operations establish constraints on the specific
            // versions they observe or produce.
            for premise in storage
                .read_premises(references, tables, operation)?
                .into_iter()
                .chain(storage.returned_type_premises(references, tables, operation)?)
                .chain(storage.subslice_predicate_premises(references, operation)?)
            {
                push_knowledge(
                    tables,
                    premise,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
        }
        // A conditional's condition holds on its true edge and fails on its
        // false edge. Only a block that edge alone enters is dominated by it.
        if let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &block.terminator
        {
            for (edge, holds) in [(when_true.edge, true), (when_false.edge, false)] {
                push_knowledge(
                    tables,
                    Proposition::Equal(
                        ScalarTerm::value(*condition, ScalarType::Boolean),
                        ScalarTerm::Boolean(holds),
                    ),
                    edge_entries.get(edge).copied().ok_or_else(|| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            id,
                            Construct::Edge(edge),
                            "branch has no exact source edge point",
                        )
                    })?,
                    Justification::BranchCondition { edge, holds },
                );
            }
        }
    }
    // What each edge of a dispatch binds: its payload parameters, each
    // equal to what the payload leaf holds, and each under the clauses the
    // type of that storage states of it, as a payload read would be. The
    // rows hold because that edge was taken and are available on it.
    for block in &lowered.blocks {
        for (edge, selected) in storage.excluded_dispatch_cases(references, tables, block)? {
            push_knowledge(
                tables,
                Proposition::Falsehood,
                ProgramPoint::Edge(edge),
                Justification::ExcludedDispatchCase { edge, selected },
            );
        }
        let equations = storage
            .payload_equations(references, tables, block)?
            .into_iter()
            .chain(storage.dispatch_case_names(references, tables, block)?);
        let premises = storage.payload_premises(references, tables, block)?;
        for (edge, proposition) in equations.chain(
            premises
                .into_iter()
                .map(|(edge, _, premise)| (edge, premise)),
        ) {
            push_knowledge(
                tables,
                proposition,
                ProgramPoint::Edge(edge),
                Justification::DispatchBinding { edge },
            );
        }
    }
    for invocation in tables.invocations.keys().collect::<Vec<_>>() {
        let row = terminal_check_tables::row(
            &tables.invocations,
            invocation,
            id,
            Construct::Invocation(invocation),
        )?;
        if let InvocationSite::Cleanup {
            edge,
            action_ordinal,
        } = row.site
        {
            let call = call_contracts::invocation(module, lowered, tables, invocation, row)?;
            let formals = call.structural_inputs();
            let ensures = call.contract.ensures.scalar_clauses();
            let mut inputs = storage.at_cleanup_call(
                references,
                tables,
                &row.actuals,
                formals,
                edge,
                action_ordinal,
                Construct::Invocation(invocation),
            )?;
            // Cleanup is Unit and its actual is Owned. As for an ordinary
            // owned input, guarantees denote the captured invocation inputs.
            let propositions = ensures
                .map(|(position, clause)| {
                    substitute(clause, &mut inputs)
                        .map(|proposition| (position, proposition))
                        .map_err(|unbound| unbound.refusal(id, Construct::Invocation(invocation)))
                })
                .collect::<Checked<Vec<_>>>()?;
            for (clause, proposition) in propositions {
                push_knowledge(
                    tables,
                    proposition,
                    ProgramPoint::AfterCleanup {
                        edge,
                        action_ordinal,
                    },
                    Justification::CalleeGuarantee {
                        invocation,
                        clause: position(clause, id)?,
                    },
                );
            }
            continue;
        }
        // A call with no normal completion is a terminator. Its callee's
        // guarantees "apply only on normal return; external exit neither
        // establishes them nor necessarily violates them"
        // (calls_and_outcomes.md), and it has no return, so it adds no
        // knowledge.
        let operation = match row.site {
            InvocationSite::Operation(operation) => operation,
            InvocationSite::InvokeNoNormal(_)
            | InvocationSite::ExternalCompletion(_)
            | InvocationSite::Cleanup { .. } => continue,
        };
        let call = call_contracts::invocation(module, lowered, tables, invocation, row)?;
        let contract = &call.contract;
        let erased = erased_actuals(contract, call.inputs, id, invocation)?;
        let mut ensures = contract.ensures.scalar_clauses().peekable();
        if ensures.peek().is_none() {
            continue;
        }
        let mut returned = storage.after_call(
            references,
            tables,
            &row.actuals,
            &call,
            operation,
            Construct::Invocation(invocation),
        )?;
        // Resolve each guarantee against this invocation's retained inputs and
        // returned storage before mutating the caller's knowledge table.
        let propositions = ensures
            .map(|(position, ensures)| {
                substitute(
                    ensures,
                    &mut ErasedTerms {
                        terms: &erased,
                        runtime: &mut returned,
                    },
                )
                .map(|proposition| (position, proposition))
                .map_err(|unbound| unbound.refusal(id, Construct::Invocation(invocation)))
            })
            .collect::<Checked<Vec<_>>>()?;
        for (clause, proposition) in propositions {
            push_knowledge(
                tables,
                proposition,
                ProgramPoint::AfterOperation(operation),
                Justification::CalleeGuarantee {
                    invocation,
                    clause: position(clause, id)?,
                },
            );
        }
    }

    Ok(())
}

/// The callee summaries one module's machine checks share, so each callee
/// is replayed once per module rather than once per checked caller. The two
/// tables serve graphs built differently, and a summary from one does not
/// describe the other's rows.
pub(crate) struct ModuleReplays<'m> {
    /// For each machine check's own borrowed-storage graph, which includes
    /// the module's nominal calls.
    borrowed_storage: terminal_semantics::descriptor_results::ModuleSummaries<'m>,
    /// For the affine claim walk of each machine (`minted`), whose graph
    /// has no nominal calls.
    affine: &'m terminal_semantics::plain_affine::AffineSummaries<'m>,
}

/// What deciding one machine's obligations needs: value types, dominance,
/// and the proposition context; filling the knowledge table on the way.
// Checking reads the graph and context; test support also reads the rest.
#[cfg_attr(not(any(test, feature = "test-support")), allow(dead_code))]
pub(crate) struct Prepared {
    pub(crate) types: IdTable<ValueId, ScalarType>,
    pub(crate) graph: Graph,
    pub(crate) context: PropositionContext,
    reference_graph: Vec<terminal_semantics::descriptor_results::DescriptorMachine>,
    ranking_bindings: rankings::Bindings,
}

/// The checking continuation borrows the same descriptor and storage analysis
/// that admitted rankings and filled knowledge. It cannot outlive preparation.
struct PreparedMachine<'scope, 'analysis> {
    module: &'analysis TerminalModule,
    lowered: &'analysis TerminalMachine,
    id: MachineId,
    types: &'scope IdTable<ValueId, ScalarType>,
    graph: &'scope Graph,
    context: &'scope PropositionContext,
    ranking_bindings: &'scope rankings::Bindings,
    storage: &'scope places::Storage<'analysis>,
    references: &'scope mut terminal_semantics::descriptor_results::DescriptorAnalysis<'analysis>,
}

impl PreparedMachine<'_, '_> {
    /// Decides every arrival of this machine's joins into `decided`
    /// (`joins::check`) and returns the weakened joins.
    fn check_joins(
        &mut self,
        tables: &mut MachineTables,
        decided: &mut [Option<Checked<Discharged>>],
        unreached: &[ValueId],
        law_instances: equality::LawScope<'_>,
        activity: &mut ProofSearchActivity,
    ) -> Checked<Vec<BlockId>> {
        joins::check(
            &joins::Machine {
                module: self.module,
                lowered: self.lowered,
                storage: self.storage,
                id: self.id,
                types: self.types,
                graph: self.graph,
                context: self.context,
                ranking_bindings: self.ranking_bindings,
                unreached,
                law_instances,
            },
            tables,
            decided,
            activity,
            self.references,
        )
    }

    /// A replay on a copy of the machine's tables needs the knowledge its
    /// joins admit, not their verdicts. Returns the values the integer rules
    /// do not reach in that copy (`Unreached`).
    fn replay_joins(
        &mut self,
        tables: &mut MachineTables,
        unreached: &Unreached,
        activity: &mut ProofSearchActivity,
    ) -> Checked<Vec<ValueId>> {
        let beyond = unreached.values(self.lowered, tables);
        let mut decided = (0..tables.obligations.len())
            .map(|_| None)
            .collect::<Vec<_>>();
        self.check_joins(
            tables,
            &mut decided,
            &beyond,
            equality::LawScope {
                instances: &[],
                identities: None,
            },
            activity,
        )?;
        Ok(beyond)
    }
}

impl Prepared {
    #[cfg_attr(not(any(test, feature = "test-support")), allow(dead_code))]
    fn references<'a>(
        &'a self,
        types: &'a [terminal_psi::StructuralTypeDeclaration],
    ) -> terminal_semantics::descriptor_results::DescriptorAnalysis<'a> {
        terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
            types,
            &self.reference_graph,
        )
    }
}

/// Fills `tables.knowledge` from the lowered form and builds what deciding
/// needs. Knowledge already present is kept: it is this machine's own.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn prepare(
    formed: formation::FormedModule<'_>,
    id: MachineId,
    tables: &mut MachineTables,
) -> Checked<Prepared> {
    let module = formed.source();
    let inhabitation = prepare_inhabitation(module, id)?;
    with_prepared_machine(formed, id, tables, &inhabitation, None, |_, _| Ok(()))
        .map(|(prepared, ())| prepared)
}

fn prepare_inhabitation(
    module: &TerminalModule,
    id: MachineId,
) -> Checked<terminal_semantics::inhabitation::Inhabitation> {
    terminal_semantics::inhabitation::Inhabitation::prepare(
        &terminal_semantics::inhabitation::TerminalContracts::new(&module.types.structural_types),
    )
    .map_err(|_| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            "structural type contracts do not form one inhabitation family",
        )
    })
}

/// Keep analysis within one invocation while a caller checks the prepared
/// machine. The returned values own no borrow of this local working state.
fn with_prepared_machine<Outcome>(
    formed: formation::FormedModule<'_>,
    id: MachineId,
    tables: &mut MachineTables,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    summaries: Option<&ModuleReplays<'_>>,
    check: impl FnOnce(PreparedMachine<'_, '_>, &mut MachineTables) -> Checked<Outcome>,
) -> Checked<(Prepared, Outcome)> {
    let module = formed.source();
    let machines = &module.machines;
    let lowered = terminal_machine(machines, id, id)?;
    let fill = tables.knowledge.is_empty();
    let types = tables.values.map(|_, row| row.scalar_type);
    // Before anything is decided: the rows must be the ones the lowered form
    // owes, at its points, or deciding every row proves nothing (`minted`).
    // A consumer re-checking a saved proof decides rows too, so it checks
    // them the same way.
    minted::follow_the_lowered_form(
        id,
        module,
        lowered,
        tables,
        &types,
        inhabitation,
        summaries.map(|summaries| summaries.affine),
    )?;
    let graph = Graph::new(id, lowered, tables)?;
    theorems::validate(module, lowered, tables, &graph)?;
    minted::check_pending_operands(lowered, tables, &graph)?;
    let reference_graph = places::reference_graph_with_generic_calls(
        module,
        id,
        formed.nominal_calls(),
        formed.schemas,
    )?;
    let (context, ranking_bindings, result) = {
        let mut references =
            terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &module.types.structural_types,
                &reference_graph,
            );
        // The graph is this module's closure with its nominal calls, which
        // is what the module's shared summaries were replayed from.
        if let Some(summaries) = summaries {
            references.share_summaries(&summaries.borrowed_storage, formed.nominal_calls());
        }
        let storage =
            places::Storage::new(id, lowered, module, &graph, tables, &types, &mut references)?;
        // domains.md: "Mutation invalidates subject-bound facts unless
        // preserved or re-established." A routed domain is re-established
        // only by a machine its declaration names, so any other machine
        // that changes part of such a value cannot keep the domain on it.
        if let Some((operation, domain)) = storage.unauthorized_routed_write() {
            return Err(refusal(
                RefusalReason::Violation,
                id,
                Construct::Operation(operation),
                &format!(
                    "this statement changes, or lends for change, part of a value that carries \
                     {} in a machine that is not one of the domain's establishment routes: only \
                     a machine the domain's declaration names may change such a value and keep \
                     the domain on it",
                    formed.domain_label(domain),
                ),
            ));
        }
        let ranking_bindings = rankings::admit(lowered, tables, &graph, &storage, &mut references)?;
        if fill {
            storage.owed_are_minted(tables)?;
            fill_knowledge(
                module,
                id,
                lowered,
                tables,
                &types,
                (&storage, &mut references),
            )?;
        }

        // A value of a type parameter has no type the kernel could hold it at,
        // so the context does not list it (`concrete`). The machine's own places
        // are listed so a goal may name one: a byte predicate of an input view
        // (`Storage::byte_source`) stays a term over that input's place.
        let context = PropositionContext::from_value_types_and_places(
            types
                .iter()
                .filter(|(value, _)| {
                    tables
                        .values
                        .get(*value)
                        .is_none_or(|row| row.rigid.is_none())
                })
                .map(|(value, scalar_type)| (value, *scalar_type)),
            lowered
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(|_| {
            refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Machine,
                "value types do not form a proposition context",
            )
        })?;
        let result = check(
            PreparedMachine {
                module,
                lowered,
                id,
                types: &types,
                graph: &graph,
                context: &context,
                ranking_bindings: &ranking_bindings,
                storage: &storage,
                references: &mut references,
            },
            tables,
        )?;
        (context, ranking_bindings, result)
    };
    Ok((
        Prepared {
            types,
            graph,
            context,
            reference_graph,
            ranking_bindings,
        },
        result,
    ))
}

/// A published `ensures` clause holds when every `Destination::Return` edge
/// has a certificate-discharged `NormalGuarantee` for it naming this
/// published identity; vacuously when there is no return edge.
/// control_flow.md: "All normal returns, including returns after a cycle,
/// must establish the published guarantees."
fn published(
    tables: &MachineTables,
    id: MachineId,
    published: ObligationId,
    clause: u32,
    decided: &[Option<Checked<Discharged>>],
) -> Checked<Discharged> {
    let unproved = |detail: &str| {
        refusal(
            RefusalReason::Unproved(UnprovedCause::ReturnDoesNotEstablish),
            id,
            Construct::Obligation(published),
            detail,
        )
    };
    let mut exits = Vec::new();
    for (edge, row) in tables.edges.iter() {
        if row.destination != Destination::Return {
            continue;
        }
        let guarantee = tables.obligations.iter().find(|(_, candidate)| {
            candidate.kind
                == ObligationKind::NormalGuarantee {
                    exit: edge,
                    clause,
                    published,
                }
        });
        let Some((obligation, _)) = guarantee else {
            return Err(unproved(
                "a normal return owes no guarantee for this clause",
            ));
        };
        let position = usize::try_from(obligation.get() - 1).unwrap_or(usize::MAX);
        match decided.get(position) {
            Some(Some(Ok(
                Discharged::Certificate(_)
                | Discharged::Identity(_)
                | Discharged::Law(_)
                | Discharged::ConservedContent(_),
            ))) => exits.push(obligation),
            _ => return Err(unproved("a normal return does not establish this clause")),
        }
    }
    Ok(Discharged::EveryExit(exits))
}

fn push_knowledge(
    tables: &mut MachineTables,
    proposition: Proposition,
    available_from: ProgramPoint,
    justification: Justification,
) {
    let proposition = tables.propositions.push(proposition);
    tables.knowledge.push(KnowledgeRow {
        proposition,
        available_from,
        justification,
        premises: Vec::new(),
    });
}

/// The proposition a knowledge row states.
fn knowledge_proposition(tables: &MachineTables, row: KnowledgeId) -> Option<&Proposition> {
    let row = tables.knowledge.get(row)?;
    tables.propositions.get(row.proposition)
}

fn position(index: usize, machine: MachineId) -> Checked<u32> {
    u32::try_from(index).map_err(|_| {
        refusal(
            RefusalReason::Unsupported,
            machine,
            Construct::Machine,
            "clause position exceeds u32",
        )
    })
}

fn semantics_refusal(
    machine: MachineId,
    operation: semantic_vocabulary::OperationId,
) -> impl Fn(terminal_semantics::OperationSemanticError) -> Refusal {
    move |error| Refusal {
        reason: RefusalReason::Unsupported,
        site: terminal_check_tables::RefusalSite::Machine(machine, Construct::Operation(operation)),
        source_span: None,
        detail: format!("operation semantics {error:?}"),
    }
}

/// The equation an operation establishes on its normal successor, as
/// Terminal semantics states it, or `None` for an operation with none (a
/// call).
fn result_equation(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    operation: &terminal_psi::Operation,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<Option<Proposition>> {
    let machine = lowered.id;
    let malformed = |reason| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Operation(operation.id),
            reason,
        )
    };
    if let Some(fact) = terminal_semantics::storage_alignment_fact(
        &module.types.structural_types,
        lowered,
        operation,
    )
    .map_err(malformed)?
    {
        return Ok(Some(fact));
    }
    if let Some(equation) = terminal_semantics::scalar_application_read_equation(
        &module.scalars.scalar_applications,
        operation,
    )
    .map_err(malformed)?
    {
        return Ok(Some(equation));
    }
    let refuse = semantics_refusal(machine, operation.id);
    if let Some(semantics) =
        terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, types)
            .map_err(&refuse)?
    {
        return Ok(Some(semantics.result_equation().clone()));
    }
    // A Trapping operation's normal successor is reached only where it did
    // not trap, and there it computed the Exact primitive's result.
    if let Some(semantics) =
        terminal_semantics::trapping_integer_semantics(operation, types).map_err(&refuse)?
    {
        return Ok(Some(semantics.result_equation().clone()));
    }
    Ok(
        terminal_semantics::goal_free_scalar_leaf_semantics(operation, types)
            .map_err(&refuse)?
            .map(|semantics| semantics.result_equation().clone()),
    )
}

/// A call's scalar lane: the actuals a callee clause's formals stand for.
/// Elaboration numbers a machine's scalar formals apart from its record
/// formals (the `k`-th scalar formal is its value `k + 1`), so a clause term
/// is replaced by position in this lane. A record actual is a place and has
/// no value identity: a clause names a field of it, which stands for what
/// that field of the actual holds at the call (`places`, `AtCall`).
pub(crate) fn scalar_actuals(actuals: &[Actual]) -> Vec<ValueId> {
    let mut scalars = Vec::with_capacity(actuals.len());
    for actual in actuals {
        match actual {
            Actual::Value(value) => scalars.push(*value),
            Actual::Place(_) => {}
        }
    }
    scalars
}

/// Why a clause could not be stated over this machine's values.
pub(crate) enum Unbound {
    /// The clause has a form or names something this route does not serve.
    Unsupported(&'static str),
    /// A name of the clause stands for nothing with a value identity where
    /// the clause is used, so the clause cannot be proved there.
    Unproved(&'static str),
    /// Resolving a name refused.
    Refused(Refusal),
}

impl Unbound {
    fn refusal(self, machine: MachineId, concerns: Construct) -> Refusal {
        match self {
            Self::Unsupported(detail) => {
                refusal(RefusalReason::Unsupported, machine, concerns, detail)
            }
            // The clause cannot be stated over values here, so no derivation
            // of it exists from what this point knows.
            Self::Unproved(detail) => refusal(
                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                machine,
                concerns,
                detail,
            ),
            Self::Refused(refusal) => refusal,
        }
    }
}

use terminal_semantics::substitution::clauses::substitute_sufficient_clause;
pub(crate) use terminal_semantics::substitution::clauses::{
    CaseAnswer, ClauseBindings as Bindings, substitute_clause as substitute,
    substitute_clause_term as bound,
};

/// Express a machine input contract in the same vocabulary when assuming it
/// at entry and when proving it is forwarded intact. An observable field has
/// an SSA identity. An unread field still names its original entry snapshot.
fn entry_snapshot_conjuncts(
    clause: &Proposition,
    bindings: &mut dyn Bindings<Error = Unbound>,
) -> Result<Vec<Proposition>, Unbound> {
    storage_conjuncts(clause, bindings, |unread| Some(unread.clone()))
}

/// States one of this machine's own entry clauses as a goal over its entry
/// storage. An atomic part the entry binding cannot state
/// (`Unbound::Unproved`: an element no operation reads at entry) keeps its
/// form over the machine's own formals, the form `entry_snapshot_conjuncts`
/// gives the same part of a `requires` premise and a call gives a shared
/// input it reads. The goal then names the same unread entry observation as
/// the premises about it, and nothing else proves it.
pub(crate) fn entry_goal(
    clause: &Proposition,
    bindings: &mut dyn Bindings<Error = Unbound>,
) -> Result<Proposition, Unbound> {
    let parts = |parts: &[Proposition], bindings: &mut dyn Bindings<Error = Unbound>| {
        parts
            .iter()
            .map(|part| entry_goal(part, bindings))
            .collect::<Result<Vec<_>, _>>()
    };
    match clause {
        Proposition::Conjunction(conjuncts) => {
            Ok(Proposition::Conjunction(parts(conjuncts, bindings)?))
        }
        Proposition::Disjunction(disjuncts) => {
            Ok(Proposition::Disjunction(parts(disjuncts, bindings)?))
        }
        part => match substitute_goal(part, bindings) {
            Err(Unbound::Unproved(_)) => Ok(part.clone()),
            stated => stated,
        },
    }
}

/// A conjunction admits independent projections. A disjunction must remain
/// whole even if only one of its alternatives mentions an unread field.
fn storage_conjuncts(
    clause: &Proposition,
    bindings: &mut dyn Bindings<Error = Unbound>,
    unobserved: fn(&Proposition) -> Option<Proposition>,
) -> Result<Vec<Proposition>, Unbound> {
    let mut pending = vec![clause];
    let mut stated = Vec::new();
    while let Some(part) = pending.pop() {
        if let Proposition::Conjunction(parts) = part {
            pending.extend(parts);
            continue;
        }
        match substitute(part, bindings) {
            Ok(part) => stated.push(part),
            Err(Unbound::Unproved(_)) => stated.extend(unobserved(part)),
            Err(error) => return Err(error),
        }
    }
    Ok(stated)
}

/// States a clause the checker must prove: a goal, never a premise.
///
/// A disjunct this point cannot state over its values counts as false, and
/// the rest of the disjunction must hold. Proving the remaining disjuncts
/// proves the whole clause, because each disjunct only adds ways for it to
/// hold. An inferred input condition (`entry_inputs`) needs this: it joins
/// what each caller establishes, so one caller's disjunct can name a field
/// that another caller never reads and so has no value for.
///
/// Only `Unbound::Unproved` parts are dropped. A clause form this route does
/// not serve, or a refused name, still refuses. Premises keep `substitute`:
/// dropping a part there would assume more than the clause says.
pub(crate) fn substitute_goal(
    clause: &Proposition,
    bindings: &mut dyn Bindings<Error = Unbound>,
) -> Result<Proposition, Unbound> {
    // A single reaching alternative may decide the whole disjunction. Keep
    // that correlation before trying the branches this arrival can observe.
    match substitute_sufficient_clause(clause, bindings) {
        Err(Unbound::Unproved(_))
            if matches!(
                clause,
                Proposition::Conjunction(_) | Proposition::Disjunction(_)
            ) =>
        {
            goal_from_observed_parts(clause, bindings)
        }
        // An erased actual can make a previously scalar equality mention
        // a declaration application. Use the same typed form as authored
        // clause applications, and keep the ordinary scalar route otherwise.
        Ok(Proposition::Equal(left, right))
            if [&left, &right]
                .into_iter()
                .any(|term| matches!(term, ScalarTerm::DeclarationApplication { .. })) =>
        {
            terminal_codec::contract_forms::equal(left, right)
                .map_err(|_| Unbound::Unsupported("an erased equality has no canonical form"))
        }
        result => result,
    }
}

fn goal_from_observed_parts(
    clause: &Proposition,
    bindings: &mut dyn Bindings<Error = Unbound>,
) -> Result<Proposition, Unbound> {
    let (parts, conjunction) = match clause {
        Proposition::Conjunction(parts) => (parts, true),
        Proposition::Disjunction(parts) => (parts, false),
        _ => return substitute_sufficient_clause(clause, bindings),
    };
    let (deciding, unit) = if conjunction {
        (Proposition::Falsehood, Proposition::Truth)
    } else {
        (Proposition::Truth, Proposition::Falsehood)
    };
    let mut kept = Vec::with_capacity(parts.len());
    let mut unstated = None;
    for part in parts {
        match goal_from_observed_parts(part, bindings) {
            Ok(part) if part == deciding => return Ok(deciding),
            Ok(part) if part == unit => {}
            Ok(part) => kept.push(part),
            // A conjunct is owed whatever the others say.
            Err(Unbound::Unproved(detail)) => {
                unstated = unstated.or(Some(Unbound::Unproved(detail)))
            }
            Err(unbound) => return Err(unbound),
        }
    }
    if let Some(unbound) = unstated
        && (conjunction || kept.is_empty())
    {
        return Err(unbound);
    }
    Ok(match kept.len() {
        0 => unit,
        1 => kept.remove(0),
        _ if conjunction => Proposition::Conjunction(kept),
        _ => Proposition::Disjunction(kept),
    })
}

impl From<&'static str> for Unbound {
    fn from(detail: &'static str) -> Self {
        Self::Unsupported(detail)
    }
}

/// A call's erased lane: each `[erased]` scalar formal of the callee with the
/// term the call supplies for it. `wiki/spec/proofs/contracts.md`, "Explicit
/// erased bindings": "Construction supplies the erased term". The check
/// tables hold no row for such an actual, because it is a term and not a
/// value or a place, so it is read from the call in the lowered form, where
/// Terminal Psi keeps it (`erased_arguments`). A term names values of the
/// caller or literals; any other form is refused, as in `substitute`.
fn erased_actuals(
    contract: &call_contracts::PublicContract<'_>,
    inputs: call_contracts::Inputs<'_>,
    id: MachineId,
    invocation: terminal_check_tables::InvocationId,
) -> Checked<Vec<(ValueId, ScalarTerm)>> {
    let refuse = |reason, detail| refusal(reason, id, Construct::Invocation(invocation), detail);
    if contract.erased_proof_formals.len() != inputs.erased_proof_arguments.len()
        || contract.erased_proof_formals.iter().any(|formal| {
            !matches!(
                formal.source,
                terminal_psi::ProofFormalSource::GenericParameter(_)
            )
        })
        || inputs.erased_proof_arguments.iter().any(|actual| {
            !matches!(
                actual,
                semantic_vocabulary::ProofTerm::Mathematical { .. }
                    | semantic_vocabulary::ProofTerm::InvocationFormal { .. }
            )
        })
    {
        return Err(refuse(
            RefusalReason::Unsupported,
            "callee lost its exact checked theorem supply",
        ));
    }
    let supplied = inputs.erased_arguments;
    if supplied.len() != contract.erased_scalar_formals.len() {
        return Err(refuse(
            RefusalReason::ForeignIdentity,
            "call supplies another number of erased terms than its callee declares",
        ));
    }
    contract
        .erased_scalar_formals
        .iter()
        .zip(supplied)
        .map(|(formal, term)| match term {
            ScalarTerm::Value { .. }
            | ScalarTerm::Integer { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::DeclarationApplication { .. } => Ok((formal.id, term.clone())),
            _ => Err(refuse(
                RefusalReason::Unsupported,
                "erased actual outside values, literals, and declaration applications",
            )),
        })
        .collect()
}

/// A call's erased terms standing for the callee's `[erased]` scalar
/// formals, over the bindings of its runtime formals. An erased formal's
/// identity follows the callee's result pseudo-value
/// (`Signature::formal_values`), so it never collides with a runtime formal;
/// every other name is the runtime bindings' to resolve.
pub(crate) struct ErasedTerms<'a> {
    pub(crate) terms: &'a [(ValueId, ScalarTerm)],
    pub(crate) runtime: &'a mut dyn Bindings<Error = Unbound>,
}

impl Bindings for ErasedTerms<'_> {
    type Error = Unbound;
    fn whole_input(&mut self, root: PlaceId) -> Result<PlaceId, Unbound> {
        self.runtime.whole_input(root)
    }
    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Self::Error> {
        self.runtime.validate_float_meaning_source(source)
    }

    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound> {
        let Some((_, supplied)) = self.terms.iter().find(|(formal, _)| *formal == id) else {
            return self.runtime.value(id, scalar_type);
        };
        if supplied.scalar_type() != scalar_type {
            return Err(Unbound::Unsupported(
                "erased actual type differs from the formal's",
            ));
        }
        Ok(supplied.clone())
    }

    fn value_alternatives(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<Vec<ScalarTerm>, Unbound> {
        if self.terms.iter().any(|(formal, _)| *formal == id) {
            self.value(id, scalar_type).map(|value| vec![value])
        } else {
            self.runtime.value_alternatives(id, scalar_type)
        }
    }

    fn field(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.field(version, root, path, scalar_type)
    }

    fn structural_read(
        &mut self,
        root: PlaceId,
        path: &[semantic_vocabulary::ScalarReadProjection],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.structural_read(root, path, scalar_type)
    }

    fn field_alternatives(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<Vec<ScalarTerm>, Unbound> {
        self.runtime
            .field_alternatives(version, root, path, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.extent(root, path, scalar_type)
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.byte_predicate(predicate, root, path)
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<crate::CaseAnswer, Unbound> {
        self.runtime.case(root, path, case)
    }
}

/// The caller's scalar actual a callee's scalar formal stands for.
pub(crate) fn scalar_actual(
    actuals: &[ValueId],
    tables: &MachineTables,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<ScalarTerm, Unbound> {
    let actual = usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| actuals.get(position))
        .ok_or(Unbound::Unsupported(
            "clause names a value that is not a formal",
        ))?;
    let row = tables.values.get(*actual).ok_or(Unbound::Unsupported(
        "actual is past the end of the caller's values",
    ))?;
    if row.scalar_type != scalar_type {
        return Err(Unbound::Unsupported(
            "actual type differs from the formal's",
        ));
    }
    Ok(ScalarTerm::value(*actual, scalar_type))
}

/// States one of a callee's entry clauses (a `requires` or a published crash
/// route) over one call with `state`. A scalar formal is the scalar actual in
/// its lane, an `[erased]` formal is the term the call supplies, and a field
/// or element of a record formal is what that storage of the record actual
/// holds when the call is made (calls_and_outcomes.md: "Declared
/// requirements and published crash routes describe invocation entry").
pub(crate) fn at_call_actuals<'a>(
    (storage, references): (
        &places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
    tables: &MachineTables,
    (invocation, row, call): (
        terminal_check_tables::InvocationId,
        &terminal_check_tables::edges::InvocationRow,
        &call_contracts::Invocation<'_>,
    ),
    id: MachineId,
    concerns: Construct,
    state: impl FnOnce(&mut dyn Bindings<Error = Unbound>) -> Result<Proposition, Unbound>,
) -> Checked<Proposition> {
    let erased = erased_actuals(&call.contract, call.inputs, id, invocation)?;
    let formals = call.structural_inputs();
    let over_actuals = |runtime: &mut dyn Bindings<Error = Unbound>| {
        state(&mut ErasedTerms {
            terms: &erased,
            runtime,
        })
    };
    match row.site {
        InvocationSite::Cleanup {
            edge,
            action_ordinal,
        } => over_actuals(&mut storage.at_cleanup_call(
            references,
            tables,
            &row.actuals,
            formals,
            edge,
            action_ordinal,
            concerns,
        )?),
        site => over_actuals(&mut storage.at_call(
            references,
            tables,
            &row.actuals,
            formals,
            site,
            concerns,
        )?),
    }
    .map_err(|unbound| unbound.refusal(id, concerns))
}

/// The goal of one obligation, over this machine's values.
#[allow(clippy::too_many_arguments)]
fn goal<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    (storage, references): (
        &places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
    id: MachineId,
    obligation: ObligationId,
    kind: &ObligationKind,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    ranking_bindings: &rankings::Bindings,
) -> Checked<Proposition> {
    let goal = stated_goal(
        module,
        lowered,
        (storage, references),
        id,
        obligation,
        kind,
        tables,
        types,
        ranking_bindings,
    )?;
    concrete(tables, id, Construct::Obligation(obligation), &goal)?;
    Ok(goal)
}

/// Refuses a proposition that names a value of a type parameter.
///
/// A generic check form declares such a value with a stand-in type
/// (`RigidType::DECLARED_AS`), because the lowered form has no other way to
/// declare one. Nothing may be proved about it from that declaration, and no
/// proof rule is yet marked as holding at every type, so a goal or a
/// hypothesis over such a value is refused as outside the route: every goal
/// passes here from `goal`, and every hypothesis from `hypotheses`.
fn concrete(
    tables: &MachineTables,
    id: MachineId,
    concerns: Construct,
    proposition: &Proposition,
) -> Checked<()> {
    let rigid = proposition.any_value_id(|value| {
        tables
            .values
            .get(value)
            .is_some_and(|row| row.rigid.is_some())
    });
    if rigid {
        return Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "a proposition names a value of a type parameter",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stated_goal<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    (storage, references): (
        &places::Storage<'a>,
        &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    ),
    id: MachineId,
    obligation: ObligationId,
    kind: &ObligationKind,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    ranking_bindings: &rankings::Bindings,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    match *kind {
        ObligationKind::IntegerWidthPositive { parameter } => {
            let integer = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .expect("u64 is a supported carrier");
            Ok(Proposition::LessThan(
                ScalarTerm::integer(integer, semantic_vocabulary::IntegerValue::Unsigned(0))
                    .expect("zero is a u64 value"),
                ScalarTerm::value(parameter, ScalarType::Integer(integer)),
            ))
        }
        ObligationKind::OperationSafety { site } => storage.safety_goal(site, obligation, types),
        ObligationKind::TheoremRequirement { edge, clause } => theorems::goal(
            module,
            id,
            (storage, &mut *references),
            tables,
            edge,
            clause,
        ),
        ObligationKind::CallRequirement { invocation, clause } => {
            let row = terminal_check_tables::row(
                &tables.invocations,
                invocation,
                id,
                Construct::Invocation(invocation),
            )?;
            let call = call_contracts::invocation(module, lowered, tables, invocation, row)?;
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| call.contract.requires.get(clause))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "callee has no such requires clause",
                    )
                })?;
            at_call_actuals(
                (storage, references),
                tables,
                (invocation, row, &call),
                id,
                concerns,
                |bindings| substitute_goal(clause, bindings),
            )
        }
        ObligationKind::DataDomain { operation, clause } => {
            storage.domain_goal(references, tables, operation, clause, concerns)
        }
        ObligationKind::NormalGuarantee { exit, clause, .. } => {
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| lowered.contract.ensures.get(clause))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "machine has no such ensures clause",
                    )
                })?;
            substitute_goal(
                &clause.proposition,
                &mut storage.at_return(references, tables, exit, concerns)?,
            )
            .map_err(|unbound| unbound.refusal(id, concerns))
        }
        ObligationKind::PublishedGuarantee { .. } => Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "a published clause is decided from its exits, not proved",
        )),
        ObligationKind::ArrivalInvariant { edge } => joins::arrival_goal(
            lowered,
            id,
            obligation,
            edge,
            tables,
            storage,
            &module.proofs.scalar_block_invariants,
            references,
        ),
        ObligationKind::CrashCoverage { site, cause } => crashes::coverage_goal(
            module,
            lowered,
            (storage, references),
            tables,
            types,
            id,
            obligation,
            site,
            cause,
        ),
        ObligationKind::RankComparison { component, edge } => rankings::goal(
            lowered,
            ranking_bindings,
            obligation,
            component,
            edge,
            rankings::Comparison::Claimed,
        ),
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. }
        | ObligationKind::ExecutableMembership { .. }
        | ObligationKind::WellFoundedness { .. } => Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "obligation kind outside this route",
        )),
    }
}

/// Proves `goal` from the knowledge available at `point` and has the kernel
/// decide the certificate.
#[allow(clippy::too_many_arguments)]
fn decide(
    context: &PropositionContext,
    graph: &Graph,
    tables: &MachineTables,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
    goal: Proposition,
    unreached: &[ValueId],
    activity: &mut ProofSearchActivity,
) -> Checked<Evidence> {
    let hypotheses = hypotheses(graph, tables, id, obligation, point)?;
    // Search and the kernel first see only the facts connected to the goal
    // (`connected`); the full set is tried only when that search fails, so
    // the narrowing never refuses an obligation the full set proves.
    let narrowed = connected(&goal, &hypotheses);
    if let Some(narrowed) = narrowed
        && let Ok(certificate) = search_certificate(
            context,
            &tables.values,
            id,
            obligation,
            &goal,
            &narrowed.assumptions,
            &narrowed.axioms,
            unreached,
            activity,
        )
    {
        return verify(context, id, obligation, goal, narrowed, certificate);
    }
    let certificate = search_certificate(
        context,
        &tables.values,
        id,
        obligation,
        &goal,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        unreached,
        activity,
    )?;
    verify(context, id, obligation, goal, hypotheses, certificate)
}

#[allow(clippy::too_many_arguments)]
fn decide_indexed(
    context: &PropositionContext,
    graph: &Graph,
    tables: &MachineTables,
    knowledge: &mut KnowledgeIndex<'_>,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
    goal: Proposition,
    unreached: &[ValueId],
    activity: &mut ProofSearchActivity,
) -> Checked<Evidence> {
    match knowledge.narrowed_or_full(graph, tables, id, obligation, point, &goal)? {
        NarrowedKnowledge::Narrowed(narrowed) => {
            if let Ok(certificate) = search_certificate(
                context,
                &tables.values,
                id,
                obligation,
                &goal,
                &narrowed.assumptions,
                &narrowed.axioms,
                unreached,
                activity,
            ) {
                return verify(context, id, obligation, goal, narrowed, certificate);
            }
            let full = knowledge.full(graph, tables, id, obligation, point)?;
            let certificate = search_certificate(
                context,
                &tables.values,
                id,
                obligation,
                &goal,
                &full.assumptions,
                &full.axioms,
                unreached,
                activity,
            )?;
            verify(context, id, obligation, goal, full, certificate)
        }
        NarrowedKnowledge::All(hypotheses) => {
            let certificate = search_certificate(
                context,
                &tables.values,
                id,
                obligation,
                &goal,
                &hypotheses.assumptions,
                &hypotheses.axioms,
                unreached,
                activity,
            )?;
            verify(context, id, obligation, goal, hypotheses, certificate)
        }
    }
}

enum NarrowedKnowledge {
    Narrowed(Hypotheses),
    All(Hypotheses),
}

#[derive(Clone, Copy)]
struct BlockKnowledgeRow {
    pos: usize,
    row_idx: u32,
}

#[derive(Clone, Copy)]
enum RowLocation {
    Block {
        block: semantic_vocabulary::BlockId,
        pos: usize,
    },
    EdgeDominated {
        target: semantic_vocabulary::BlockId,
    },
    ExactEdge,
    Cleanup,
}

#[derive(Clone, Copy)]
struct ValueRowRef {
    traversal_start: usize,
    pos: usize,
    row_idx: u32,
}

struct KnowledgeIndex<'a> {
    row_ids: Vec<KnowledgeId>,
    row_points: Vec<ProgramPoint>,
    row_is_axiom: Vec<bool>,
    row_props: Vec<Option<&'a Proposition>>,
    row_locations: Vec<RowLocation>,
    bad_rows: Vec<(KnowledgeId, ProgramPoint)>,
    block_rows: Vec<Vec<BlockKnowledgeRow>>,
    edge_target_rows: Vec<Vec<u32>>,
    exact_edge_rows: Vec<(semantic_vocabulary::EdgeId, u32)>,
    cleanup_rows: Vec<u32>,
    strict_dom_block_count: Vec<usize>,
    strict_dom_edge_count: Vec<usize>,
    has_edge_or_cleanup_rows: bool,
    distinct_values: Vec<ValueId>,
    val_dense: Vec<u32>,
    val_row_ranges: Vec<(u32, u32)>,
    val_rows: Vec<ValueRowRef>,
    row_val_ranges: Vec<(u32, u32)>,
    row_vals: Vec<u32>,
    always_kept: Vec<u32>,
    visited_val: Vec<u32>,
    visited_row: Vec<u32>,
    stamp: u32,
    queue: Vec<u32>,
    kept_rows: Vec<u32>,
    goal_vals: Vec<ValueId>,
}

impl<'a> KnowledgeIndex<'a> {
    fn new(graph: &Graph, tables: &'a MachineTables, id: MachineId) -> Checked<Self> {
        let block_count = graph.block_count();
        let row_count = tables.knowledge.len();
        let mut row_ids = Vec::with_capacity(row_count);
        let mut row_points = Vec::with_capacity(row_count);
        let mut row_is_axiom = Vec::with_capacity(row_count);
        let mut row_props = Vec::with_capacity(row_count);
        let mut row_locations = Vec::with_capacity(row_count);
        let mut bad_rows = Vec::new();
        let mut block_rows = vec![Vec::new(); block_count];
        let mut edge_target_rows = vec![Vec::new(); block_count];
        let mut exact_edge_rows = Vec::new();
        let mut cleanup_rows = Vec::new();
        let mut always_kept = Vec::new();
        let mut raw_row_vals: Vec<(ValueId, usize, usize, u32)> = Vec::new();
        let mut temp_vals = Vec::new();
        let mut distinct_values = Vec::new();
        let mut row_complete = Vec::with_capacity(row_count);
        let mut row_val_slices = Vec::with_capacity(row_count);
        let mut flat_raw_vals = Vec::new();

        for (row_pos, (row, knowledge_row)) in tables.knowledge.iter().enumerate() {
            let row_idx = u32::try_from(row_pos).unwrap_or(u32::MAX);
            row_ids.push(row);
            row_points.push(knowledge_row.available_from);
            row_is_axiom.push(matches!(
                knowledge_row.justification,
                Justification::OperationSemantics(_)
            ));
            let valid_prop = match knowledge_proposition(tables, row) {
                Some(proposition)
                    if concrete(tables, id, Construct::Machine, proposition).is_ok() =>
                {
                    Some(proposition)
                }
                _ => {
                    bad_rows.push((row, knowledge_row.available_from));
                    None
                }
            };
            row_props.push(valid_prop);
            let (location, sort_start, sort_pos) = match knowledge_row.available_from {
                ProgramPoint::BlockEntry(_) | ProgramPoint::AfterOperation(_) => {
                    let (block, pos, dom) =
                        graph.locate_with_dominance_info(knowledge_row.available_from)?;
                    block_rows[usize::try_from(block.get() - 1).unwrap_or(usize::MAX)]
                        .push(BlockKnowledgeRow { pos, row_idx });
                    (
                        RowLocation::Block { block, pos },
                        dom.traversal_start(),
                        pos,
                    )
                }
                ProgramPoint::Edge(edge) => {
                    exact_edge_rows.push((edge, row_idx));
                    if let Some(target) = graph.edge_dominator(edge)? {
                        edge_target_rows[usize::try_from(target.get() - 1).unwrap_or(usize::MAX)]
                            .push(row_idx);
                        (RowLocation::EdgeDominated { target }, 0, 0)
                    } else {
                        (RowLocation::ExactEdge, 0, 0)
                    }
                }
                ProgramPoint::BeforeCleanup { .. } | ProgramPoint::AfterCleanup { .. } => {
                    cleanup_rows.push(row_idx);
                    (RowLocation::Cleanup, 0, 0)
                }
            };
            row_locations.push(location);
            let start = flat_raw_vals.len() as u32;
            if let Some(proposition) = valid_prop {
                temp_vals.clear();
                let complete = proposition.visit_value_ids(|value| temp_vals.push(value));
                if !complete || temp_vals.is_empty() {
                    always_kept.push(row_idx);
                    row_complete.push(false);
                } else {
                    temp_vals.sort_unstable();
                    temp_vals.dedup();
                    for &val in &temp_vals {
                        distinct_values.push(val);
                        flat_raw_vals.push(val);
                        raw_row_vals.push((val, sort_start, sort_pos, row_idx));
                    }
                    row_complete.push(true);
                }
            } else {
                row_complete.push(false);
            }
            row_val_slices.push((start, flat_raw_vals.len() as u32));
        }

        for rows in &mut block_rows {
            rows.sort_unstable_by_key(|row| (row.pos, row.row_idx));
        }
        exact_edge_rows.sort_unstable();

        let mut by_preorder = (0..block_count).collect::<Vec<_>>();
        by_preorder.sort_unstable_by_key(|&slot| {
            let block = BlockId::new(slot as u64 + 1).expect("dense block identity");
            graph
                .dominance_of(block)
                .map_or(0, |dom| dom.traversal_start())
        });
        let mut strict_dom_block_count = vec![0usize; block_count];
        let mut strict_dom_edge_count = vec![0usize; block_count];
        for slot in by_preorder {
            let block = BlockId::new(slot as u64 + 1).expect("dense block identity");
            if let Some(parent) = graph.immediate_dominator(block)? {
                let p_slot = usize::try_from(parent.get() - 1).unwrap_or(usize::MAX);
                strict_dom_block_count[slot] =
                    strict_dom_block_count[p_slot] + block_rows[p_slot].len();
                strict_dom_edge_count[slot] =
                    strict_dom_edge_count[p_slot] + edge_target_rows[p_slot].len();
            }
        }
        let has_edge_or_cleanup_rows = !cleanup_rows.is_empty()
            || !exact_edge_rows.is_empty()
            || edge_target_rows.iter().any(|rows| !rows.is_empty());

        distinct_values.sort_unstable();
        distinct_values.dedup();
        let max_val_slot = distinct_values
            .last()
            .and_then(|val| usize::try_from(val.get() - 1).ok())
            .unwrap_or(0);
        let mut val_dense =
            if !distinct_values.is_empty() && max_val_slot <= distinct_values.len() * 4 + 64 {
                let mut map = vec![u32::MAX; max_val_slot + 1];
                for (dense, &val) in distinct_values.iter().enumerate() {
                    map[usize::try_from(val.get() - 1).expect("bounded value id")] = dense as u32;
                }
                map
            } else {
                Vec::new()
            };
        let lookup_val = |val: ValueId| -> Option<u32> {
            if !val_dense.is_empty() {
                let slot = usize::try_from(val.get() - 1).ok()?;
                let mapped = *val_dense.get(slot)?;
                (mapped != u32::MAX).then_some(mapped)
            } else {
                distinct_values
                    .binary_search(&val)
                    .ok()
                    .map(|idx| idx as u32)
            }
        };

        let mut val_row_ranges = vec![(0u32, 0u32); distinct_values.len()];
        let mut counts = vec![0u32; distinct_values.len()];
        for &(val, _, _, _) in &raw_row_vals {
            if let Some(dense) = lookup_val(val) {
                counts[dense as usize] += 1;
            }
        }
        let mut offset = 0u32;
        for (dense, &count) in counts.iter().enumerate() {
            val_row_ranges[dense] = (offset, offset + count);
            offset += count;
        }
        let mut write_pos = val_row_ranges
            .iter()
            .map(|(start, _)| *start)
            .collect::<Vec<_>>();
        let mut val_rows = vec![
            ValueRowRef {
                traversal_start: 0,
                pos: 0,
                row_idx: 0,
            };
            offset as usize
        ];
        for (val, traversal_start, pos, row_idx) in raw_row_vals {
            if let Some(dense) = lookup_val(val) {
                let slot = write_pos[dense as usize] as usize;
                write_pos[dense as usize] += 1;
                val_rows[slot] = ValueRowRef {
                    traversal_start,
                    pos,
                    row_idx,
                };
            }
        }
        for &(start, end) in &val_row_ranges {
            val_rows[start as usize..end as usize]
                .sort_unstable_by_key(|entry| (entry.traversal_start, entry.pos, entry.row_idx));
        }

        let mut row_val_ranges = Vec::with_capacity(row_count);
        let mut row_vals = Vec::with_capacity(flat_raw_vals.len());
        for row_idx in 0..row_count {
            let (raw_start, raw_end) = row_val_slices[row_idx];
            let out_start = row_vals.len() as u32;
            if row_complete[row_idx] {
                for &val in &flat_raw_vals[raw_start as usize..raw_end as usize] {
                    if let Some(dense) = lookup_val(val) {
                        row_vals.push(dense);
                    }
                }
            }
            row_val_ranges.push((out_start, row_vals.len() as u32));
        }
        let _ = &mut val_dense;
        let distinct_count = distinct_values.len();

        Ok(Self {
            row_ids,
            row_points,
            row_is_axiom,
            row_props,
            row_locations,
            bad_rows,
            block_rows,
            edge_target_rows,
            exact_edge_rows,
            cleanup_rows,
            strict_dom_block_count,
            strict_dom_edge_count,
            has_edge_or_cleanup_rows,
            distinct_values,
            val_dense,
            val_row_ranges,
            val_rows,
            row_val_ranges,
            row_vals,
            always_kept,
            visited_val: vec![0u32; distinct_count],
            visited_row: vec![0u32; row_count],
            stamp: 0,
            queue: Vec::new(),
            kept_rows: Vec::new(),
            goal_vals: Vec::new(),
        })
    }

    fn lookup_val(&self, val: ValueId) -> Option<u32> {
        if !self.val_dense.is_empty() {
            let slot = usize::try_from(val.get() - 1).ok()?;
            let mapped = *self.val_dense.get(slot)?;
            (mapped != u32::MAX).then_some(mapped)
        } else {
            self.distinct_values
                .binary_search(&val)
                .ok()
                .map(|idx| idx as u32)
        }
    }

    fn check_bad_rows(
        &self,
        graph: &Graph,
        tables: &MachineTables,
        id: MachineId,
        obligation: ObligationId,
        point: ProgramPoint,
    ) -> Checked<()> {
        if self.bad_rows.is_empty() {
            return Ok(());
        }
        for &(row, available_from) in &self.bad_rows {
            if !graph.dominates(available_from, point)? {
                continue;
            }
            let proposition = knowledge_proposition(tables, row).ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Obligation(obligation),
                    "knowledge row has no proposition",
                )
            })?;
            concrete(tables, id, Construct::Obligation(obligation), proposition)?;
        }
        Ok(())
    }

    fn dominates_row(
        &self,
        graph: &Graph,
        row_idx: usize,
        point: ProgramPoint,
        used_block: semantic_vocabulary::BlockId,
        used_pos: usize,
        used_dom: &terminal_semantics::control_graph::DominanceInterval,
    ) -> Checked<bool> {
        match self.row_locations[row_idx] {
            RowLocation::Block { block, pos } => {
                if block == used_block {
                    Ok(pos <= used_pos)
                } else {
                    Ok(graph.dominance_of(block)?.dominates(used_dom))
                }
            }
            RowLocation::EdgeDominated { .. } | RowLocation::ExactEdge | RowLocation::Cleanup => {
                graph.dominates(self.row_points[row_idx], point)
            }
        }
    }

    fn count_dominating(
        &self,
        graph: &Graph,
        point: ProgramPoint,
        used_block: semantic_vocabulary::BlockId,
        used_pos: usize,
        used_dom: &terminal_semantics::control_graph::DominanceInterval,
    ) -> Checked<usize> {
        let used_slot = usize::try_from(used_block.get() - 1).unwrap_or(usize::MAX);
        let mut total = self.strict_dom_block_count[used_slot]
            + self.block_rows[used_slot].partition_point(|row| row.pos <= used_pos);
        if !self.has_edge_or_cleanup_rows {
            return Ok(total);
        }
        let is_cleanup_or_edge = matches!(
            point,
            ProgramPoint::BeforeCleanup { .. }
                | ProgramPoint::AfterCleanup { .. }
                | ProgramPoint::Edge(_)
        );
        if !is_cleanup_or_edge {
            total += self.strict_dom_edge_count[used_slot] + self.edge_target_rows[used_slot].len();
        } else {
            let mut cur = Some(used_block);
            while let Some(block) = cur {
                let slot = usize::try_from(block.get() - 1).unwrap_or(usize::MAX);
                for &row_idx in &self.edge_target_rows[slot] {
                    if graph.dominates(self.row_points[row_idx as usize], point)? {
                        total += 1;
                    }
                }
                cur = graph.immediate_dominator(block)?;
            }
            if let ProgramPoint::Edge(used_edge) = point {
                let start = self
                    .exact_edge_rows
                    .partition_point(|&(edge, _)| edge < used_edge);
                for &(edge, row_idx) in &self.exact_edge_rows[start..] {
                    if edge != used_edge {
                        break;
                    }
                    let already_counted = match self.row_locations[row_idx as usize] {
                        RowLocation::EdgeDominated { target } => {
                            graph.dominance_of(target)?.dominates(used_dom)
                        }
                        _ => false,
                    };
                    if !already_counted {
                        total += 1;
                    }
                }
            }
        }
        for &row_idx in &self.cleanup_rows {
            if graph.dominates(self.row_points[row_idx as usize], point)? {
                total += 1;
            }
        }
        Ok(total)
    }

    fn materialize(&self, rows: &[u32]) -> Hypotheses {
        let mut assumption_rows = Vec::new();
        let mut axiom_rows = Vec::new();
        let mut assumptions = Vec::new();
        let mut axioms = Vec::new();
        for &row_idx in rows {
            let idx = row_idx as usize;
            let row = self.row_ids[idx];
            let proposition = self.row_props[idx].expect("validated proposition");
            if self.row_is_axiom[idx] {
                axiom_rows.push(row);
                axioms.push(proposition.clone());
            } else {
                assumption_rows.push(row);
                assumptions.push(proposition.clone());
            }
        }
        Hypotheses {
            assumption_rows,
            axiom_rows,
            assumptions,
            axioms,
        }
    }

    fn full(
        &mut self,
        graph: &Graph,
        tables: &MachineTables,
        id: MachineId,
        obligation: ObligationId,
        point: ProgramPoint,
    ) -> Checked<Hypotheses> {
        self.check_bad_rows(graph, tables, id, obligation, point)?;
        let (used_block, used_pos, used_dom) = graph.locate_with_dominance_info(point)?;
        self.kept_rows.clear();
        let is_cleanup = matches!(
            point,
            ProgramPoint::BeforeCleanup { .. } | ProgramPoint::AfterCleanup { .. }
        );
        let mut cur = Some(used_block);
        while let Some(block) = cur {
            let slot = usize::try_from(block.get() - 1).unwrap_or(usize::MAX);
            let rows = &self.block_rows[slot];
            let limit = if block == used_block {
                rows.partition_point(|row| row.pos <= used_pos)
            } else {
                rows.len()
            };
            for row in &rows[..limit] {
                self.kept_rows.push(row.row_idx);
            }
            for &row_idx in &self.edge_target_rows[slot] {
                if !is_cleanup || graph.dominates(self.row_points[row_idx as usize], point)? {
                    self.kept_rows.push(row_idx);
                }
            }
            cur = graph.immediate_dominator(block)?;
        }
        if let ProgramPoint::Edge(used_edge) = point {
            let start = self
                .exact_edge_rows
                .partition_point(|&(edge, _)| edge < used_edge);
            for &(edge, row_idx) in &self.exact_edge_rows[start..] {
                if edge != used_edge {
                    break;
                }
                let already_counted = match self.row_locations[row_idx as usize] {
                    RowLocation::EdgeDominated { target } => {
                        graph.dominance_of(target)?.dominates(used_dom)
                    }
                    _ => false,
                };
                if !already_counted {
                    self.kept_rows.push(row_idx);
                }
            }
        }
        for &row_idx in &self.cleanup_rows {
            if graph.dominates(self.row_points[row_idx as usize], point)? {
                self.kept_rows.push(row_idx);
            }
        }
        self.kept_rows.sort_unstable();
        Ok(self.materialize(&self.kept_rows))
    }

    fn narrowed_or_full(
        &mut self,
        graph: &Graph,
        tables: &MachineTables,
        id: MachineId,
        obligation: ObligationId,
        point: ProgramPoint,
        goal: &Proposition,
    ) -> Checked<NarrowedKnowledge> {
        self.check_bad_rows(graph, tables, id, obligation, point)?;
        let (used_block, used_pos, used_dom) = graph.locate_with_dominance_info(point)?;
        let used_start = used_dom.traversal_start();
        self.goal_vals.clear();
        if !goal.visit_value_ids(|value| self.goal_vals.push(value)) {
            return Ok(NarrowedKnowledge::All(
                self.full(graph, tables, id, obligation, point)?,
            ));
        }
        if self.stamp == u32::MAX {
            self.visited_val.fill(0);
            self.visited_row.fill(0);
            self.stamp = 1;
        } else {
            self.stamp += 1;
        }
        let stamp = self.stamp;
        self.queue.clear();
        self.kept_rows.clear();
        for &val in &self.goal_vals {
            if let Some(dense) = self.lookup_val(val)
                && self.visited_val[dense as usize] != stamp
            {
                self.visited_val[dense as usize] = stamp;
                self.queue.push(dense);
            }
        }
        while let Some(dense) = self.queue.pop() {
            let (start, end) = self.val_row_ranges[dense as usize];
            let entries = &self.val_rows[start as usize..end as usize];
            let limit = entries.partition_point(|entry| {
                (entry.traversal_start, entry.pos) <= (used_start, used_pos)
            });
            for entry in &entries[..limit] {
                let row_idx = entry.row_idx as usize;
                if self.visited_row[row_idx] == stamp {
                    continue;
                }
                self.visited_row[row_idx] = stamp;
                if !self.dominates_row(graph, row_idx, point, used_block, used_pos, used_dom)? {
                    continue;
                }
                self.kept_rows.push(entry.row_idx);
                let (val_start, val_end) = self.row_val_ranges[row_idx];
                for &other_val in &self.row_vals[val_start as usize..val_end as usize] {
                    if self.visited_val[other_val as usize] != stamp {
                        self.visited_val[other_val as usize] = stamp;
                        self.queue.push(other_val);
                    }
                }
            }
        }
        for &row_idx in &self.always_kept {
            let idx = row_idx as usize;
            if self.visited_row[idx] != stamp
                && self.dominates_row(graph, idx, point, used_block, used_pos, used_dom)?
            {
                self.visited_row[idx] = stamp;
                self.kept_rows.push(row_idx);
            }
        }
        self.kept_rows.sort_unstable();
        let total = self.count_dominating(graph, point, used_block, used_pos, used_dom)?;
        let hypotheses = self.materialize(&self.kept_rows);
        if self.kept_rows.len() < total {
            Ok(NarrowedKnowledge::Narrowed(hypotheses))
        } else {
            Ok(NarrowedKnowledge::All(hypotheses))
        }
    }
}

/// The hypotheses that share a value with `goal`, directly or through a
/// chain of other hypotheses, plus every hypothesis that names no value.
/// `None` when that is all of them.
///
/// A long machine accumulates thousands of available facts (a layout plan
/// that fills a 64-entry record array has over 5,000), and the kernel
/// re-checks every certificate in the context of every premise it is given,
/// so passing all of them made each obligation cost time linear in the
/// machine's size. A fact whose values are disjoint from the goal's
/// component can only matter by contradicting the others; the caller keeps
/// that case by retrying with the full set. Dropping premises cannot make
/// the kernel accept more, and the evidence records exactly the rows used.
fn connected(goal: &Proposition, hypotheses: &Hypotheses) -> Option<Hypotheses> {
    let premises = || hypotheses.assumptions.iter().chain(&hypotheses.axioms);
    // Dense indices for every value named anywhere, then one union-find
    // pass joining each premise's values.
    let mut values = Vec::new();
    let mut complete = Vec::with_capacity(hypotheses.assumptions.len() + hypotheses.axioms.len());
    for premise in premises() {
        complete.push(premise.visit_value_ids(|value| values.push(value)));
    }
    if !goal.visit_value_ids(|value| values.push(value)) {
        return None;
    }
    values.sort_unstable();
    values.dedup();
    let index = |value: ValueId| values.binary_search(&value).ok();
    let mut parent = (0..values.len()).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    for premise in premises() {
        let mut first = None;
        premise.visit_value_ids(|value| {
            let Some(node) = index(value) else { return };
            let node = root(&mut parent, node);
            match first {
                None => first = Some(node),
                Some(other) => parent[node] = root(&mut parent, other),
            }
        });
    }
    let mut goal_roots = Vec::new();
    goal.visit_value_ids(|value| {
        if let Some(node) = index(value) {
            goal_roots.push(root(&mut parent, node));
        }
    });
    goal_roots.sort_unstable();
    goal_roots.dedup();
    // A premise is kept when it names no value, could not be enumerated, or
    // names a value of the goal's component.
    let mut keep = |premise: &Proposition, complete: bool| {
        let mut named = false;
        let mut joined = false;
        premise.visit_value_ids(|value| {
            named = true;
            if let Some(node) = index(value) {
                joined |= goal_roots.binary_search(&root(&mut parent, node)).is_ok();
            }
        });
        !complete || !named || joined
    };
    let mut narrowed = Hypotheses {
        assumption_rows: Vec::new(),
        axiom_rows: Vec::new(),
        assumptions: Vec::new(),
        axioms: Vec::new(),
    };
    let (assumption_complete, axiom_complete) = complete.split_at(hypotheses.assumptions.len());
    for ((premise, row), complete) in hypotheses
        .assumptions
        .iter()
        .zip(&hypotheses.assumption_rows)
        .zip(assumption_complete)
    {
        if keep(premise, *complete) {
            narrowed.assumptions.push(premise.clone());
            narrowed.assumption_rows.push(*row);
        }
    }
    for ((premise, row), complete) in hypotheses
        .axioms
        .iter()
        .zip(&hypotheses.axiom_rows)
        .zip(axiom_complete)
    {
        if keep(premise, *complete) {
            narrowed.axioms.push(premise.clone());
            narrowed.axiom_rows.push(*row);
        }
    }
    (narrowed.assumptions.len() + narrowed.axioms.len()
        < hypotheses.assumptions.len() + hypotheses.axioms.len())
    .then_some(narrowed)
}

/// Declaration formation and executable obligations search the same proof
/// rules. Their callers provide the exact typed inputs and independent facts;
/// the resulting certificate still needs the kernel's acceptance.
#[allow(clippy::too_many_arguments)]
fn search_certificate(
    context: &PropositionContext,
    values: &dyn known::ProofValues,
    id: MachineId,
    obligation: ObligationId,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    unreached: &[ValueId],
    activity: &mut ProofSearchActivity,
) -> Checked<CertificateEnvelope> {
    let mut prover = prove::Prover::new(context, assumptions, axioms, values, activity);
    let proof = prover.prove(goal).map_err(|reason| {
        // Classified by the goal's form, not by which search path failed
        // last, so the outcome does not depend on search order: the form as
        // written, and the form of the operations that define the values it
        // names (`prove::reach`).
        let cause = if prover.has_rules_for(goal) && !prover.beyond_rules(goal, unreached) {
            UnprovedCause::NoDerivation
        } else {
            UnprovedCause::NoRuleForGoalForm
        };
        unproved(id, obligation, cause, reason.to_owned())
    })?;
    let identity = EvidenceIdentity::new(obligation.get()).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Obligation(obligation),
            "obligation identity is not an evidence identity",
        )
    })?;
    Ok(CertificateEnvelope {
        identity,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    })
}

fn unproved(
    id: MachineId,
    obligation: ObligationId,
    cause: UnprovedCause,
    detail: String,
) -> Refusal {
    Refusal {
        reason: RefusalReason::Unproved(cause),
        site: terminal_check_tables::RefusalSite::Machine(id, Construct::Obligation(obligation)),
        source_span: None,
        detail,
    }
}

/// The knowledge available where an obligation sits, split as the kernel
/// takes it: assumptions and semantic axioms, with their rows.
pub(crate) struct Hypotheses {
    pub(crate) assumption_rows: Vec<KnowledgeId>,
    pub(crate) axiom_rows: Vec<KnowledgeId>,
    pub(crate) assumptions: Vec<Proposition>,
    pub(crate) axioms: Vec<Proposition>,
}

pub(crate) fn hypotheses(
    graph: &Graph,
    tables: &MachineTables,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
) -> Checked<Hypotheses> {
    let mut assumption_rows = Vec::new();
    let mut axiom_rows = Vec::new();
    let mut assumptions = Vec::new();
    let mut axioms = Vec::new();
    for (row, knowledge_row) in tables.knowledge.iter() {
        if !graph.dominates(knowledge_row.available_from, point)? {
            continue;
        }
        let proposition = knowledge_proposition(tables, row).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Obligation(obligation),
                "knowledge row has no proposition",
            )
        })?;
        concrete(tables, id, Construct::Obligation(obligation), proposition)?;
        match knowledge_row.justification {
            Justification::OperationSemantics(_) => {
                axiom_rows.push(row);
                axioms.push(proposition.clone());
            }
            Justification::EntryRequirement { .. }
            | Justification::FormalType
            | Justification::TheoremGuarantee { .. }
            | Justification::CalleeGuarantee { .. }
            | Justification::BranchCondition { .. }
            | Justification::ExcludedDispatchCase { .. }
            | Justification::DispatchBinding { .. }
            | Justification::BlockInvariant { .. }
            | Justification::Derived { .. } => {
                assumption_rows.push(row);
                assumptions.push(proposition.clone());
            }
        }
    }
    Ok(Hypotheses {
        assumption_rows,
        axiom_rows,
        assumptions,
        axioms,
    })
}

/// Keep the actual program-point dependencies beside the kernel's accepted
/// certificate so a later consumer can reconstruct the same judgment.
pub(crate) fn verify(
    context: &PropositionContext,
    id: MachineId,
    obligation: ObligationId,
    goal: Proposition,
    hypotheses: Hypotheses,
    certificate: CertificateEnvelope,
) -> Checked<Evidence> {
    let fact = verify_certificate(
        context,
        id,
        obligation,
        &goal,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        &certificate,
    )?;
    Ok(Evidence {
        goal,
        assumptions: hypotheses.assumption_rows,
        axioms: hypotheses.axiom_rows,
        certificate,
        fact,
    })
}

/// The kernel checks the exact goal and independent premises before this
/// certificate can justify a declaration or an executable obligation.
fn verify_certificate(
    context: &PropositionContext,
    id: MachineId,
    obligation: ObligationId,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    certificate: &CertificateEnvelope,
) -> Checked<AcceptedFact> {
    verify_obligation(
        context,
        &Obligation {
            id: obligation,
            proposition: goal.clone(),
            class: ObligationClass::Derivable,
        },
        assumptions,
        axioms,
        EvidenceRoute::CertificateDerived(certificate.as_ref()),
    )
    .map_err(|error| {
        unproved(
            id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("kernel {error:?}"),
        )
    })
}

/// The scalar value an operation defines, if any: a call's result value.
pub(crate) fn operation_result(
    lowered: &TerminalMachine,
    operation: OperationId,
) -> Option<ValueId> {
    lowered
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation)
        .and_then(|found| match &found.result {
            terminal_psi::OperationResult::Scalar(declaration) => Some(declaration.id),
            _ => None,
        })
}

/// `clause` with the result pseudo-value `result` replaced by `value`, in
/// the comparison and connective forms a clause over scalars or mathematical
/// integers has. Any other form is left as it is: it then still names the pseudo-value, which no
/// knowledge row describes, so it is unproved and never wrongly accepted.
fn returned_for_result(clause: &Proposition, result: ValueId, value: ValueId) -> Proposition {
    let term = |term: &ScalarTerm| match term {
        ScalarTerm::Value { id, scalar_type } if *id == result => {
            ScalarTerm::value(value, *scalar_type)
        }
        other => other.clone(),
    };
    let parts = |parts: &[Proposition]| {
        parts
            .iter()
            .map(|part| returned_for_result(part, result, value))
            .collect()
    };
    // An embedded `result` is the returned value embedded: the embedding
    // keeps the carrier, and the two have the same one.
    fn mathematical(term: &IntegerMathTerm, result: ValueId, value: ValueId) -> IntegerMathTerm {
        let mut returned = term.clone();
        match &mut returned {
            IntegerMathTerm::MathValue { value: id, .. } if *id == result => *id = value,
            other => {
                for operand in other.operands_mut() {
                    *operand = mathematical(operand, result, value);
                }
            }
        }
        returned
    }
    let math = |term: &IntegerMathTerm| mathematical(term, result, value);
    match clause {
        Proposition::Equal(left, right) => Proposition::Equal(term(left), term(right)),
        Proposition::LessThan(left, right) => Proposition::LessThan(term(left), term(right)),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(term(left), term(right)),
        Proposition::IntegerMathEqual(left, right) => {
            Proposition::IntegerMathEqual(math(left), math(right))
        }
        Proposition::IntegerMathLessThan(left, right) => {
            Proposition::IntegerMathLessThan(math(left), math(right))
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            Proposition::IntegerMathLessOrEqual(math(left), math(right))
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(parts(conjuncts)),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(parts(disjuncts)),
        other => other.clone(),
    }
}

/// Build and replay a certificate for a closed contract implication. Callers
/// provide declarations, never hypotheses from source checking. This is the
/// same prover and kernel used for obligations at executable program points.
pub fn derive_contract_obligation(
    site: terminal_check_tables::RefusalSite,
    context: &PropositionContext,
    obligation: ObligationId,
    goal: &Proposition,
    declarations: &[(ValueId, ScalarType)],
    activity: &mut ProofSearchActivity,
) -> Checked<CertificateEnvelope> {
    let malformed = |detail| Refusal::at(RefusalReason::ForeignIdentity, site, detail);
    context
        .validate(goal)
        .map_err(|_| malformed("contract obligation escapes its formal namespace"))?;
    let mut declarations = declarations.to_vec();
    declarations.sort_unstable_by_key(|(id, _)| *id);
    let values = IdTable::from_rows(
        declarations
            .iter()
            .enumerate()
            .map(|(position, (id, scalar_type))| {
                let position =
                    u32::try_from(position).map_err(|_| "contract formal count exceeds u32")?;
                Ok((
                    *id,
                    terminal_check_tables::values::ValueRow {
                        scalar_type: *scalar_type,
                        origin: terminal_check_tables::values::ValueOrigin::MachineParameter {
                            position,
                        },
                        rigid: None,
                    },
                ))
            })
            .collect::<Result<Vec<_>, &'static str>>()
            .map_err(malformed)?,
    )
    .map_err(|_| malformed("contract formal identities are not a complete dense namespace"))?;
    let mut prover = prove::Prover::new(context, &[], &[], &values, activity);
    let proof = prover.prove(goal).map_err(|detail| {
        let cause = if prover.has_rules_for(goal) {
            UnprovedCause::NoDerivation
        } else {
            UnprovedCause::NoRuleForGoalForm
        };
        Refusal::at(RefusalReason::Unproved(cause), site, detail)
    })?;
    let certificate = CertificateEnvelope {
        identity: EvidenceIdentity::new(obligation.get())
            .ok_or_else(|| malformed("contract obligation identity is absent"))?,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    };
    verify_obligation(
        context,
        &Obligation {
            id: obligation,
            proposition: goal.clone(),
            class: ObligationClass::Derivable,
        },
        &[],
        &[],
        EvidenceRoute::CertificateDerived(certificate.as_ref()),
    )
    .map_err(|_| {
        Refusal::at(
            RefusalReason::Unproved(UnprovedCause::KernelRefused),
            site,
            "contract implication certificate was refused by the proof kernel",
        )
    })?;
    Ok(certificate)
}
