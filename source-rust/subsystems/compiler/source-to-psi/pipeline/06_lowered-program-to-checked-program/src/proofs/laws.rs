//! Judges the compilation's mathematical signature.
//!
//! The kernel checks every declaration once. Each law is then proved against
//! the declarations before its own statement, so a law is never among its own
//! evidence. A law whose body dispatches on a formal is an induction: each
//! case is an equality over an arbitrary value of that case, proved by
//! rewriting with the laws the case cites and with the law itself at a field
//! of the case, and the cases are assembled with the data's induction
//! principle. The kernel checks that term once, as the certificate the
//! judgment reports.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level,
    MathematicalCertificate, MathematicalCoreDecision, MathematicalJudgmentReceipt, Signature,
    Sort, Term, TermArena, TermHandle, check_signature_with_attribution, convertible, instantiate,
    shift,
};
use terminal_check_tables::UnprovedCause;
use terminal_check_tables::mathematics::{
    Citation, Evidence, FamilyLawCase, IntegerPositions, Law, LawCase, LawProof,
    MathematicalSignature, OperandLaw, OperandLicense, SignatureEntry, SignatureEntryKind,
    SplitSubject,
};

use super::equality::{
    BudgetLimit, Definitions, Distribution, License, Licenses, SearchError, abstracted, normal,
    prove_licensed,
};

/// How many rewrite attempts one case of one law may make.
const SEARCH_STEPS: usize = 65_536;
/// What one search for a proof may spend computing with the kernel's
/// reduction: normalizing the terms it reaches. This bounds the search. It is
/// no part of what the finished proof costs the kernel to check, which
/// [`DEFAULT_CONVERSION_STEPS`] bounds.
const SEARCH_COMPUTING: u32 = 1 << 20;

/// One law the kernel judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawJudgment {
    /// The machine that states the law, as authored.
    pub machine: String,
    pub clause: u32,
    pub decision: MathematicalCoreDecision,
    /// The proof, its statement and the declarations they depend on.
    pub certificate: Vec<u8>,
    /// Kernel conversion steps spent finding and checking the proof.
    pub conversion_steps: u32,
    /// The machine whose `ensures` clause `clause` this law proves for every
    /// value of the machine's formals, when the law is a theorem over data
    /// indexed by values (`prove_family`). A running machine that cites the
    /// theorem calls that machine, and its guarantee is this proof.
    pub theorem: Option<symbols::SymbolHandle>,
}

/// The signature after the kernel checked its declarations and every law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureJudgment {
    pub declarations: usize,
    /// One row per data declaration, in signature order, for the Psi
    /// mathematical carrier table.
    pub carriers: Vec<DataCarrier>,
    pub laws: Vec<LawJudgment>,
}

/// A data declaration's kernel type, under the source identity Psi names it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCarrier {
    /// The data's source type identity text.
    pub type_identity: String,
    /// The carrier key the encoding specifies: a closed certificate whose term
    /// and expected type are both the data's type. Its signature is the
    /// declarations up to and including the data's own, the prefix the type
    /// depends on, so the key does not change when the program adds a later
    /// declaration.
    pub carrier_key: Vec<u8>,
}

/// The carrier of every data declaration the kernel has checked, a record
/// of mathematical data and a quotient of it included.
fn carriers(
    arena: &mut TermArena,
    declarations: &[Declaration],
    entries: &[SignatureEntry],
) -> Result<Vec<DataCarrier>, terminal_codec::CodecError> {
    let mut carriers = Vec::new();
    for (position, entry) in entries.iter().enumerate() {
        let (SignatureEntryKind::Data { type_identity, .. }
        | SignatureEntryKind::Record { type_identity, .. }
        | SignatureEntryKind::Quotient { type_identity, .. }) = &entry.kind
        else {
            continue;
        };
        let ty = constant(arena, position as u32);
        let prefix_end = match declarations
            .get(position)
            .map(|declaration| &declaration.kind)
        {
            Some(proof_admission::DeclarationKind::Inductive {
                num_mutual,
                mutual_index,
                num_constructors,
                ..
            }) => {
                let base = position - *mutual_index as usize;
                base + 2 * (*num_mutual as usize) + (*num_constructors as usize)
            }
            _ => position + 1,
        };
        let certificate = MathematicalCertificate::with_signature(
            declarations[..prefix_end].to_vec(),
            Vec::new(),
            ty,
            ty,
        );
        carriers.push(DataCarrier {
            type_identity: type_identity.clone(),
            carrier_key: terminal_codec::mathematical_carrier_key(arena, &certificate)?,
        });
    }
    Ok(carriers)
}

/// One law, or one case of it, the checker could not prove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnprovedLaw {
    pub machine: String,
    /// The case arm that was left unproved, else the law's `ensures` clause.
    pub site: Option<source_text::SourceSpan>,
    pub cause: UnprovedCause,
    /// What was not proved, in the program's own names. Diagnostic text only.
    pub statement: String,
    pub detail: String,
}

impl UnprovedLaw {
    pub fn message(&self) -> String {
        format!(
            "{}. ({}: {})",
            self.statement,
            self.cause.describe(),
            self.detail
        )
    }

    /// An exhausted budget says nothing about the law, so its diagnostic
    /// states that the program is refused without a claim that it is wrong.
    pub fn reject_diagnostic(&self) -> diagnostics::Diagnostic {
        let message = if self.cause.is_incomplete() {
            format!(
                "not supported yet: {}; the checker ran out of proof budget, so the program is refused, with no claim that it is wrong",
                self.message()
            )
        } else {
            self.message()
        };
        let diagnostic = diagnostics::Diagnostic::error(message);
        match self.site {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

struct Failure {
    case: Option<usize>,
    cause: UnprovedCause,
    detail: String,
    /// Where the body wrote the recursive call whose descent is unproved,
    /// when that is what failed: the refusal names that call.
    call: Option<source_text::SourceSpan>,
}

fn failure(case: Option<usize>, error: SearchError) -> Failure {
    let (cause, detail) = match error {
        SearchError::Budget(limit) => (UnprovedCause::BudgetExhausted, limit.describe()),
        SearchError::NoProof => (
            UnprovedCause::NoDerivation,
            "no available equality proves the stated Identity".to_owned(),
        ),
        SearchError::Kernel(error) => (
            UnprovedCause::KernelRefused,
            format!("mathematical kernel: {error:?}"),
        ),
    };
    Failure {
        case,
        cause,
        detail,
        call: None,
    }
}

/// Why a case's citations gave no evidence.
enum Uncited {
    Search(SearchError),
    /// A cited law requires an equality the case cannot prove at the cited
    /// arguments.
    Premise,
    /// That equality is the descent of a recursive call the body wrote, at
    /// `call`, and its search ran out of budget when `spent` says so.
    Descent {
        call: source_text::SourceSpan,
        spent: Option<BudgetLimit>,
    },
}

impl Uncited {
    fn failure(self, case: Option<usize>) -> Failure {
        match self {
            Self::Search(error) => failure(case, error),
            Self::Premise => Failure {
                case,
                cause: UnprovedCause::NoDerivation,
                detail: "a cited law's premise is not established at its citation".to_owned(),
                call: None,
            },
            Self::Descent { call, spent } => {
                let (cause, detail) = match spent {
                    Some(limit) => (UnprovedCause::BudgetExhausted, limit.describe()),
                    None => (
                        UnprovedCause::NoDerivation,
                        "no fact in scope at the call shows its ranked argument strictly below the law's"
                            .to_owned(),
                    ),
                };
                Failure {
                    case,
                    cause,
                    detail,
                    call: Some(call),
                }
            }
        }
    }
}

fn kernel(error: CoreError) -> Failure {
    failure(None, error.into())
}

/// The signature's verdict, and the theorems over indexed data it proved.
pub(crate) struct Judged {
    pub(crate) verdict: Result<SignatureJudgment, Vec<UnprovedLaw>>,
    /// Every proved law that is a theorem's guarantee (`LawJudgment::theorem`),
    /// also when another law is unproved. Each is checked by the kernel on
    /// its own and rests on no other law, so a machine that states it keeps
    /// its proof, and the refusal names the law that is unproved.
    pub(crate) theorems: Vec<LawJudgment>,
}

pub(crate) fn judge_with_policy(
    signature: MathematicalSignature,
    policy: crate::ProofWorkPolicy,
) -> Judged {
    let theorems = |laws: &[LawJudgment]| {
        laws.iter()
            .filter(|law| law.theorem.is_some())
            .cloned()
            .collect::<Vec<_>>()
    };
    match judge_laws(signature, policy) {
        Ok(judged) => Judged {
            theorems: theorems(&judged.laws),
            verdict: Ok(judged),
        },
        Err((unproved, proved)) => Judged {
            theorems: theorems(&proved),
            verdict: Err(unproved),
        },
    }
}

/// `Err` holds what is unproved and every law that was proved beside it.
fn judge_laws(
    signature: MathematicalSignature,
    policy: crate::ProofWorkPolicy,
) -> Result<SignatureJudgment, (Vec<UnprovedLaw>, Vec<LawJudgment>)> {
    let MathematicalSignature {
        mut arena,
        declarations,
        entries,
        laws,
        licenses,
        integers,
        families,
        // Each generic check form's tables are checked with its module.
        identities: _,
    } = signature;
    let extras = Extras {
        integers,
        families: &families,
    };
    let steps = DEFAULT_CONVERSION_STEPS.saturating_mul(declarations.len() as u32 + 1);
    let checked_signature = match check_signature_with_attribution(
        &mut arena,
        &declarations,
        &mut Budget::new(steps),
    ) {
        Ok(checked) => checked,
        Err(refused) => {
            let name = entries
                .get(refused.declaration)
                .map_or("?", |entry| entry.name.as_str());
            let Failure { cause, detail, .. } = kernel(refused.error);
            return Err((
                vec![UnprovedLaw {
                    machine: name.to_owned(),
                    site: None,
                    cause,
                    statement: format!("the mathematical declaration `{name}` is not well formed"),
                    detail,
                }],
                Vec::new(),
            ));
        }
    };
    let carriers = carriers(&mut arena, &declarations, &entries).map_err(|_| {
        (
            vec![UnprovedLaw {
                machine: String::new(),
                site: None,
                cause: UnprovedCause::KernelRefused,
                statement: "the mathematical declarations cannot be published".to_owned(),
                detail: "could not encode a data declaration's carrier key".to_owned(),
            }],
            Vec::new(),
        )
    })?;
    let mut judged = Vec::with_capacity(laws.len());
    let mut unproved = Vec::new();
    for law in &laws {
        let proved = match &law.proof {
            LawProof::Family {
                family,
                invariant,
                cases,
            } => prove_family(
                &mut arena,
                &checked_signature,
                &declarations,
                &entries,
                &extras,
                law,
                *family,
                *invariant,
                cases,
            ),
            LawProof::Integers => prove_integers(&mut arena, &declarations, &extras, law),
            _ => prove_law(
                &mut arena,
                &checked_signature,
                &declarations,
                &entries,
                &laws,
                &licenses,
                policy,
                law,
            ),
        };
        match proved {
            Ok(judgment) => judged.push(judgment),
            Err(Failure {
                cause,
                detail,
                call: Some(call),
                ..
            }) => unproved.push(UnprovedLaw {
                machine: law.machine_name.clone(),
                site: Some(call),
                cause,
                statement: format!(
                    "cannot prove ensures contract for `{0}`: the descent of `{0}` at its recursive call",
                    law.machine_name
                ),
                detail,
            }),
            Err(Failure {
                case,
                cause,
                detail,
                call: None,
            }) => unproved.push(UnprovedLaw {
                machine: law.machine_name.clone(),
                site: case
                    .and_then(|case| match &law.proof {
                        LawProof::Family { cases, .. } => {
                            cases.get(case).and_then(|case| case.source)
                        }
                        _ => script(law, case).and_then(|script| script.source),
                    })
                    .or(law.source),
                cause,
                statement: statement(&mut arena, &entries, law, case),
                detail,
            }),
        }
    }
    if unproved.is_empty() {
        Ok(SignatureJudgment {
            declarations: declarations.len(),
            carriers,
            laws: judged,
        })
    } else {
        Err((unproved, judged))
    }
}

fn script(law: &Law, case: usize) -> Option<&LawCase> {
    match &law.proof {
        LawProof::Direct(_)
        | LawProof::Fuel { .. }
        | LawProof::Family { .. }
        | LawProof::Integers => None,
        LawProof::Cases { cases, .. } => cases.get(case),
    }
}

fn variable(arena: &mut TermArena, index: usize) -> TermHandle {
    arena.insert(Term::Variable(index as u32))
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

/// The operations whose operands this law's proof may regroup or reorder,
/// each with the laws that say so, and the distributivity laws it may expand
/// products under: the licenses stated before the law. That keeps a law out
/// of its own proof and keeps two laws from resting on each other. The search
/// puts each law it uses into the proof term, so a wrong license fails the
/// kernel's check and the certificate names every law the proof relied on.
fn licenses(
    arena: &mut TermArena,
    declarations: &[Declaration],
    granted: &[OperandLicense],
    law: &Law,
) -> (Vec<License>, Vec<Distribution>) {
    let mut found: Vec<(u32, License)> = Vec::new();
    let mut distributions: Vec<(u32, u32, bool, Distribution)> = Vec::new();
    for grant in granted
        .iter()
        .filter(|grant| grant.statement < law.statement)
    {
        let Term::Pi {
            domain: carrier, ..
        } = arena.get(declarations[grant.statement as usize].ty)
        else {
            continue;
        };
        let proved = constant(arena, grant.statement);
        if let OperandLaw::Distributes {
            over,
            from_left,
            product_first,
        } = grant.law
        {
            // The first law stated for each pair of operations and side.
            if !distributions
                .iter()
                .any(|known| (known.0, known.1, known.2) == (grant.operation, over, from_left))
            {
                let outer = constant(arena, grant.operation);
                let inner = constant(arena, over);
                distributions.push((
                    grant.operation,
                    over,
                    from_left,
                    Distribution {
                        outer,
                        inner,
                        carrier,
                        law: proved,
                        from_left,
                        product_first,
                    },
                ));
            }
            continue;
        }
        let at = match found
            .iter()
            .position(|(operation, _)| *operation == grant.operation)
        {
            Some(at) => at,
            None => {
                let operation = constant(arena, grant.operation);
                found.push((
                    grant.operation,
                    License {
                        operation,
                        carrier,
                        associates: None,
                        commutes: None,
                    },
                ));
                found.len() - 1
            }
        };
        let license = &mut found[at].1;
        match grant.law {
            OperandLaw::Commutes => {
                license.commutes.get_or_insert(proved);
            }
            OperandLaw::Associates { left_nested_first } => {
                license
                    .associates
                    .get_or_insert((proved, left_nested_first));
            }
            OperandLaw::Distributes { .. } => {}
        }
    }
    (
        found.into_iter().map(|(_, license)| license).collect(),
        distributions.into_iter().map(|(.., law)| law).collect(),
    )
}

/// What a case of a law may cite, as kernel terms of that case's context.
struct Cited<'c> {
    arena: &'c mut TermArena,
    context: &'c Context,
    definitions: &'c Definitions,
    licenses: Licenses<'c>,
    laws: &'c [Law],
    law: &'c Law,
    /// Each formal of the law and then each field of the enclosing cases, as
    /// the context names them: what a citation's arguments are written over.
    names: &'c [TermHandle],
    /// The proofs the case already has: the law's premises and what the
    /// enclosing cases cited.
    held: Vec<TermHandle>,
    /// The induction hypothesis for a field, when the case has one.
    hypothesis: &'c dyn Fn(u32) -> Option<TermHandle>,
    /// The machine whose law each field's hypothesis is, when it is not
    /// `law` (`Frame::companions`).
    companions: &'c [Option<symbols::SymbolHandle>],
}

impl Cited<'_> {
    /// The equalities the case has: the law's premises as its hypotheses,
    /// then each citation. A cited law's premises are proved here, at the
    /// cited arguments, from the case's other evidence, in whatever order
    /// the body wrote its citations. A citation whose premise cannot be
    /// proved refuses the case: its law was cited where it does not apply.
    fn evidence(
        self,
        citations: &[terminal_check_tables::mathematics::Citation],
    ) -> Result<Vec<TermHandle>, Uncited> {
        let mut evidence = self.held;
        let mut waiting = Vec::new();
        for citation in citations {
            let arguments: Vec<_> = citation
                .arguments
                .iter()
                .map(|&argument| instantiate(self.arena, argument, self.names))
                .collect();
            let (head, premises, actuals) = match citation.evidence {
                Evidence::Statement(cited) => {
                    let premises = self
                        .laws
                        .iter()
                        .find(|law| law.statement == cited)
                        .map_or(&[][..], |law| law.premises.as_slice());
                    (constant(self.arena, cited), premises, arguments.clone())
                }
                Evidence::Hypothesis { field, at } => {
                    // The law at a field: the field in the dispatched
                    // formal's place, the citation's arguments in the others'.
                    // At a field holding a function to the data, the law at
                    // the child `at` names: the field applied there. In a
                    // mutual block's induction it is the law of the member
                    // the field holds.
                    let cited = match self.companions.get(field as usize).copied().flatten() {
                        Some(machine) => self
                            .laws
                            .iter()
                            .find(|law| law.machine == machine)
                            .ok_or(Uncited::Search(SearchError::NoProof))?,
                        None => self.law,
                    };
                    let LawProof::Cases { formal, .. } = &cited.proof else {
                        return Err(Uncited::Search(SearchError::NoProof));
                    };
                    let count = self.law.formals.len();
                    let at = at.map(|at| instantiate(self.arena, at, self.names));
                    let child =
                        self.names
                            .get(count + field as usize)
                            .copied()
                            .map(|value| match at {
                                Some(argument) => self.arena.insert(Term::Apply {
                                    function: value,
                                    argument,
                                }),
                                None => value,
                            });
                    let mut others = arguments.iter().copied();
                    let actuals: Vec<_> = (0..cited.formals.len())
                        .map(|position| {
                            if position == *formal as usize {
                                child
                            } else {
                                others.next()
                            }
                        })
                        .collect::<Option<_>>()
                        .ok_or(Uncited::Search(SearchError::NoProof))?;
                    let mut hypothesis =
                        (self.hypothesis)(field).unwrap_or_else(|| self.arena.insert(Term::Dummy));
                    if let Some(argument) = at {
                        hypothesis = self.arena.insert(Term::Apply {
                            function: hypothesis,
                            argument,
                        });
                    }
                    (hypothesis, cited.premises.as_slice(), actuals)
                }
                // The law with one unit of fuel less, at any arguments. The
                // case with no fuel left has no such hypothesis: the goal
                // there must follow without it.
                Evidence::Fuel => match (self.hypothesis)(0) {
                    Some(hypothesis) => {
                        (hypothesis, self.law.premises.as_slice(), arguments.clone())
                    }
                    None => continue,
                },
            };
            let term = self.arena.apply_arguments(head, &arguments);
            let goals: Vec<_> = premises
                .iter()
                .map(|&premise| instantiate(self.arena, premise, &actuals))
                .collect();
            if goals.is_empty() {
                evidence.push(term);
            } else {
                waiting.push((term, goals, citation.descent));
            }
        }
        // Each round admits every waiting citation whose premises the
        // evidence so far proves, until none waits or a round admits none.
        while !waiting.is_empty() {
            let mut refused = Vec::new();
            let before = evidence.len();
            let mut exhausted: Option<BudgetLimit> = None;
            for (mut term, goals, descent) in waiting {
                let mut proofs = Vec::with_capacity(goals.len());
                let mut spent = None;
                for &goal in &goals {
                    // A premise that waits on a later citation may run out
                    // of budget before it runs out of rewrites. Each search
                    // has a budget of its own, so that one cannot starve the
                    // premises after it.
                    match prove_licensed(
                        self.arena,
                        self.context,
                        self.definitions,
                        goal,
                        &evidence,
                        self.licenses,
                        SEARCH_STEPS,
                        &mut Budget::new(SEARCH_COMPUTING),
                    ) {
                        Ok(proof) => proofs.push(proof),
                        Err(SearchError::NoProof) => break,
                        Err(SearchError::Budget(limit)) => {
                            exhausted.get_or_insert(limit);
                            spent = Some(limit);
                            break;
                        }
                        Err(error) => return Err(Uncited::Search(error)),
                    }
                }
                if proofs.len() < goals.len() {
                    refused.push((term, goals, descent.map(|call| (call, spent))));
                    continue;
                }
                for proof in proofs {
                    term = self.arena.insert(Term::Apply {
                        function: term,
                        argument: proof,
                    });
                }
                evidence.push(term);
            }
            if evidence.len() == before {
                // A refused descent is reported at its call, ahead of any
                // other citation it leaves waiting.
                if let Some((call, spent)) = refused.iter().find_map(|(.., descent)| *descent) {
                    return Err(Uncited::Descent { call, spent });
                }
                return Err(match exhausted {
                    Some(limit) => Uncited::Search(SearchError::Budget(limit)),
                    None => Uncited::Premise,
                });
            }
            waiting = refused
                .into_iter()
                .map(|(term, goals, descent)| (term, goals, descent.map(|(call, _)| call)))
                .collect();
        }
        Ok(evidence)
    }
}

/// Where one step of a law's proof stands: the variables it is under and what
/// it has to work with. Every term here is stated under `telescope`.
struct Frame {
    /// The type of each variable, outermost first, each stated under the
    /// ones before it: the law's formals and a proof of each premise, and
    /// inside a case that case's fields and hypotheses before them.
    telescope: Vec<TermHandle>,
    goal: TermHandle,
    /// What each formal of the law stands for, then each field of the
    /// enclosing cases.
    names: Vec<TermHandle>,
    /// The proofs in hand: the premises and what enclosing cases cited.
    evidence: Vec<TermHandle>,
    /// The law at each field of the induction's case that holds the data
    /// itself, by field; `None` at a payload field and empty outside one.
    hypotheses: Vec<Option<TermHandle>>,
    /// In a mutual block's induction, the machine whose law each field's
    /// hypothesis is: the law of the member the field holds. Empty when
    /// every hypothesis is the law's own.
    companions: Vec<Option<symbols::SymbolHandle>>,
}

/// What a split is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Split {
    /// A case analysis inside a case.
    Cases,
    /// The law's induction on one of its formals.
    Induction,
    /// The induction on fuel of a law about a machine defined on fuel. Its
    /// cases bind units of fuel, which the body's script never names. A
    /// unit of fuel is guarded by `ranked ∸ rest`, the truncated
    /// subtraction at entry `order`, so its case splits on that first.
    Fuel { order: u32, ranked: u32 },
    /// The split on a unit of fuel's guard: a case analysis whose field
    /// the body's script never names either.
    Guard,
}

struct Proving<'p> {
    arena: &'p mut TermArena,
    closed: &'p Context,
    definitions: &'p Definitions,
    licenses: Licenses<'p>,
    entries: &'p [SignatureEntry],
    laws: &'p [Law],
    law: &'p Law,
}

impl Proving<'_> {
    /// The proof of the frame's goal by one script: its citations, then
    /// either a search with them or the script's own cases. `reported` is
    /// the case of the law's first split a failure is reported under.
    fn case(
        &mut self,
        frame: Frame,
        script: &LawCase,
        reported: Option<usize>,
    ) -> Result<TermHandle, Failure> {
        let mut context = self.closed.clone();
        for &ty in &frame.telescope {
            context = context.extend(ty);
        }
        let hypotheses = &frame.hypotheses;
        let evidence = Cited {
            arena: self.arena,
            context: &context,
            definitions: self.definitions,
            licenses: self.licenses,
            laws: self.laws,
            law: self.law,
            names: &frame.names,
            held: frame.evidence.clone(),
            hypothesis: &|field| hypotheses.get(field as usize).copied().flatten(),
            companions: &frame.companions,
        }
        .evidence(&script.citations)
        .map_err(|error| error.failure(reported))?;
        let mut evidence = evidence;
        if script.split.is_none() {
            evidence.extend(self.lift_points(&context, frame.goal));
        }
        match &script.split {
            None => match prove_licensed(
                self.arena,
                &context,
                self.definitions,
                frame.goal,
                &evidence,
                self.licenses,
                SEARCH_STEPS,
                &mut Budget::new(SEARCH_COMPUTING),
            ) {
                // Plain data is a set (Hedberg's theorem, no UIP), two classes
                // of a quotient are equal when their representatives are
                // related (soundness), and equal classes imply related
                // representatives (effectivity, using the selected
                // `MereEquivalence` laws).
                Err(SearchError::NoProof) => self
                    .is_mere(&context, frame.goal)
                    .and_then(|proof| match proof {
                        Some(proof) => Ok(Some(proof)),
                        None => self.sound(&context, frame.goal, &evidence),
                    })
                    .and_then(|proof| match proof {
                        Some(proof) => Ok(Some(proof)),
                        None => self.effective(&context, frame.goal, &evidence),
                    })
                    .and_then(|proof| proof.ok_or(SearchError::NoProof))
                    .map_err(|error| failure(reported, error)),
                proved => proved.map_err(|error| failure(reported, error)),
            },
            Some(split) => {
                let frame = Frame { evidence, ..frame };
                match split.subject {
                    // A value an enclosing case already took apart is no
                    // longer a variable and has no cases left.
                    SplitSubject::Name(name) => {
                        let count = frame.telescope.len();
                        let place = match frame
                            .names
                            .get(name as usize)
                            .map(|&name| self.arena.get(name))
                        {
                            Some(Term::Variable(index)) => count.checked_sub(index as usize + 1),
                            _ => None,
                        }
                        .ok_or_else(|| failure(reported, SearchError::NoProof))?;
                        self.split(
                            frame,
                            place,
                            split.data,
                            &split.cases,
                            Split::Cases,
                            reported,
                        )
                    }
                    SplitSubject::Value(value) => {
                        let value = instantiate(self.arena, value, &frame.names);
                        self.split_value(
                            frame,
                            &context,
                            value,
                            split.data,
                            &split.cases,
                            Split::Cases,
                            reported,
                        )
                    }
                }
            }
        }
    }

    /// The point computation of each operation lifted to a quotient that
    /// the goal applies to a class: `lift(.., f, ..)` at the class of `a`
    /// is `f a` (`QUOTIENT_LIFT_BETA`). A lifted operation is defined by
    /// the kernel's `lift`, which does not compute, so these are the only
    /// equations a proof can rewrite it with. Only applications outside any
    /// binder of the goal are collected: each proof is stated in the case's
    /// context. A signature with no quotient has none, and a goal the
    /// search cannot compute gives none: the search then fails on it as
    /// before.
    fn lift_points(&mut self, context: &Context, goal: TermHandle) -> Vec<TermHandle> {
        let mut points = Vec::new();
        if !self
            .entries
            .iter()
            .any(|entry| matches!(entry.kind, SignatureEntryKind::Quotient { .. }))
        {
            return points;
        }
        let Ok(goal) = normal(
            self.arena,
            context,
            self.definitions,
            goal,
            &mut Budget::new(SEARCH_COMPUTING),
        ) else {
            return points;
        };
        let mut pending = vec![goal];
        while let Some(term) = pending.pop() {
            let (head, arguments) = super::equality::spine(self.arena, term);
            if let (
                Term::Constant {
                    declaration: proof_admission::QUOTIENT_LIFT,
                    levels,
                },
                [carrier, relation, result, sets, operation, respect, class],
            ) = (self.arena.get(head), arguments.as_slice())
            {
                let (projected, projection) = super::equality::spine(self.arena, *class);
                if let (
                    Term::Constant {
                        declaration: proof_admission::QUOTIENT_PROJECT,
                        ..
                    },
                    [over, by, representative],
                ) = (self.arena.get(projected), projection.as_slice())
                    && self.arena.structurally_equal(*over, *carrier)
                    && self.arena.structurally_equal(*by, *relation)
                {
                    let point = self.arena.insert(Term::Constant {
                        declaration: proof_admission::QUOTIENT_LIFT_BETA,
                        levels,
                    });
                    points.push(self.arena.apply_arguments(
                        point,
                        &[
                            *carrier,
                            *relation,
                            *result,
                            *sets,
                            *operation,
                            *respect,
                            *representative,
                        ],
                    ));
                }
            }
            pending.extend(
                self.arena
                    .get(term)
                    .bound_children()
                    .filter(|&(_, bound)| bound == 0)
                    .map(|(child, _)| child),
            );
        }
        points
    }

    /// A proof that two classes of a quotient are equal, by the quotient's
    /// soundness law: `goal` equates the projections of two representatives,
    /// and the evidence proves that the relation holds between them. `None`
    /// when the goal is not of that form.
    fn sound(
        &mut self,
        context: &Context,
        goal: TermHandle,
        evidence: &[TermHandle],
    ) -> Result<Option<TermHandle>, SearchError> {
        let Term::Id { ty, left, right } = self.arena.get(goal) else {
            return Ok(None);
        };
        let Term::Constant { declaration, .. } = self.arena.get(ty) else {
            return Ok(None);
        };
        let Some(&SignatureEntryKind::Quotient {
            carrier, relation, ..
        }) = self
            .entries
            .get(declaration as usize)
            .map(|entry| &entry.kind)
        else {
            return Ok(None);
        };
        let family = proof_admission::QuotientFamily {
            levels: [Level::Constant(0), Level::Constant(0)],
            carrier: constant(self.arena, carrier),
            relation: constant(self.arena, relation),
        };
        // The representative a side is the class of: the side is the
        // kernel's projection of it at this carrier and relation, as stage
        // 05 wrote it. The side is compared as written, because computing
        // it would replace the relation's name with its formula.
        let representative = |proving: &mut Self, side: TermHandle| {
            let Term::Apply { argument, .. } = proving.arena.get(side) else {
                return None;
            };
            let projected = family.project(proving.arena, argument);
            proving
                .arena
                .structurally_equal(projected, side)
                .then_some(argument)
        };
        let (Some(left), Some(right)) = (representative(self, left), representative(self, right))
        else {
            return Ok(None);
        };
        // The relation at the two representatives is the formula it is
        // defined as, which the search proves like any equality.
        let related = self.arena.apply_arguments(family.relation, &[left, right]);
        let related = normal(
            self.arena,
            context,
            self.definitions,
            related,
            &mut Budget::new(SEARCH_COMPUTING),
        )?;
        let held = prove_licensed(
            self.arena,
            context,
            self.definitions,
            related,
            evidence,
            self.licenses,
            SEARCH_STEPS,
            &mut Budget::new(SEARCH_COMPUTING),
        )?;
        Ok(Some(family.sound(self.arena, left, right, held)))
    }

    /// A derived proof of `IsMere(Id D x y)` (`Π (p q: Id D x y). Id (Id D x y) p q`)
    /// when `D` is plain inductive data, using `PlainData::is_set` (Hedberg's
    /// theorem, zero assumptions, no UIP).
    fn is_mere(
        &mut self,
        context: &Context,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, SearchError> {
        let Term::Pi {
            domain: prop,
            codomain: inner,
        } = self.arena.get(goal)
        else {
            return Ok(None);
        };
        let Term::Pi {
            domain: prop_1,
            codomain: eq,
        } = self.arena.get(inner)
        else {
            return Ok(None);
        };
        let Term::Id {
            ty: prop_2,
            left: p,
            right: q,
        } = self.arena.get(eq)
        else {
            return Ok(None);
        };
        if !matches!(self.arena.get(p), Term::Variable(1))
            || !matches!(self.arena.get(q), Term::Variable(0))
        {
            return Ok(None);
        }
        let expected_1 = proof_admission::shift(self.arena, prop, 0, 1);
        let expected_2 = proof_admission::shift(self.arena, prop, 0, 2);
        if !self.arena.structurally_equal(prop_1, expected_1)
            || !self.arena.structurally_equal(prop_2, expected_2)
        {
            return Ok(None);
        }
        let prop = normal(
            self.arena,
            context,
            self.definitions,
            prop,
            &mut Budget::new(SEARCH_COMPUTING),
        )?;
        let Term::Id { ty, left, right } = self.arena.get(prop) else {
            return Ok(None);
        };
        let Term::Constant { declaration, .. } = self.arena.get(ty) else {
            return Ok(None);
        };
        let Some(SignatureEntryKind::Data { shape, .. }) = self
            .entries
            .get(declaration as usize)
            .map(|entry| &entry.kind)
        else {
            return Ok(None);
        };
        // Data with a payload has no derived set proof (`PlainData::is_set`).
        let Some(set_proof) = shape.is_set(self.arena, declaration) else {
            return Ok(None);
        };
        Ok(Some(self.arena.apply_arguments(set_proof, &[left, right])))
    }

    /// A proof of `R(a, b)` from evidence of `Id Q (project a) (project b)`,
    /// using the quotient's effectivity assumption and its four selected
    /// `MereEquivalence` laws (`[propositional, reflexive, symmetric, transitive]`).
    fn effective(
        &mut self,
        context: &Context,
        goal: TermHandle,
        evidence: &[TermHandle],
    ) -> Result<Option<TermHandle>, SearchError> {
        let mut budget = Budget::new(SEARCH_COMPUTING);
        let goal_normal = normal(self.arena, context, self.definitions, goal, &mut budget)?;
        for &item in evidence {
            let Ok(item_ty) = proof_admission::infer_type(self.arena, context, item, &mut budget)
            else {
                continue;
            };
            let Term::Id { ty, left, right } = self.arena.get(item_ty) else {
                continue;
            };
            let Term::Constant { declaration, .. } = self.arena.get(ty) else {
                continue;
            };
            let Some(&SignatureEntryKind::Quotient {
                carrier,
                relation,
                laws: [propositional, reflexive, symmetric, transitive],
                ..
            }) = self
                .entries
                .get(declaration as usize)
                .map(|entry| &entry.kind)
            else {
                continue;
            };
            let family = proof_admission::QuotientFamily {
                levels: [Level::Constant(0), Level::Constant(0)],
                carrier: constant(self.arena, carrier),
                relation: constant(self.arena, relation),
            };
            let representative = |proving: &mut Self, side: TermHandle| {
                let Term::Apply { argument, .. } = proving.arena.get(side) else {
                    return None;
                };
                let projected = family.project(proving.arena, argument);
                proving
                    .arena
                    .structurally_equal(projected, side)
                    .then_some(argument)
            };
            let (Some(a), Some(b)) = (representative(self, left), representative(self, right))
            else {
                continue;
            };
            let related = self.arena.apply_arguments(family.relation, &[a, b]);
            let related_normal =
                normal(self.arena, context, self.definitions, related, &mut budget)?;
            if self.arena.structurally_equal(related_normal, goal_normal) {
                let relation_laws = proof_admission::EquivalenceProofs {
                    propositional: constant(self.arena, propositional),
                    reflexive: constant(self.arena, reflexive),
                    symmetric: constant(self.arena, symmetric),
                    transitive: constant(self.arena, transitive),
                };
                return Ok(Some(family.effective(
                    self.arena,
                    &relation_laws,
                    a,
                    b,
                    item,
                )));
            }
        }
        Ok(None)
    }

    /// The proof of the frame's goal by the cases of a computed value.
    ///
    /// The goal is first stated for any value `s` in the computed one's
    /// place, together with a proof that the computed value equals `s`.
    /// Splitting on `s` then puts each case where the computed value stood
    /// in the goal, so a definition that chooses by that value computes,
    /// and gives the case the equality as evidence for whatever else names
    /// the computed value. The whole is applied to the computed value and
    /// the proof that it equals itself. `value` is stated under the
    /// frame's telescope, and `how` is `Split::Cases` or `Split::Guard`.
    #[allow(clippy::too_many_arguments)]
    fn split_value(
        &mut self,
        frame: Frame,
        context: &Context,
        value: TermHandle,
        data: u32,
        cases: &[LawCase],
        how: Split,
        reported: Option<usize>,
    ) -> Result<TermHandle, Failure> {
        let unproved = |error| failure(reported, error);
        let mut computing = Budget::new(SEARCH_COMPUTING);
        let value = normal(self.arena, context, self.definitions, value, &mut computing)
            .map_err(unproved)?;
        let Term::Id { ty, left, right } = self.arena.get(frame.goal) else {
            return Err(unproved(SearchError::NoProof));
        };
        let left = normal(self.arena, context, self.definitions, left, &mut computing)
            .map_err(unproved)?;
        let right = normal(self.arena, context, self.definitions, right, &mut computing)
            .map_err(unproved)?;
        let goal = self.arena.insert(Term::Id { ty, left, right });
        let subject = constant(self.arena, data);
        // Under `s`: the goal with `s` for the value, and `value = s`.
        let stated = abstracted(self.arena, goal, value);
        let lifted = shift(self.arena, value, 0, 1);
        let s = variable(self.arena, 0);
        let equation = self.arena.insert(Term::Id {
            ty: subject,
            left: lifted,
            right: s,
        });
        let count = frame.telescope.len();
        let mut telescope = frame.telescope;
        telescope.extend([subject, equation]);
        let under = |arena: &mut TermArena, terms: &[TermHandle]| -> Vec<TermHandle> {
            terms.iter().map(|&term| shift(arena, term, 0, 2)).collect()
        };
        let mut evidence = under(self.arena, &frame.evidence);
        evidence.push(variable(self.arena, 0));
        let inner = Frame {
            goal: shift(self.arena, stated, 0, 1),
            names: under(self.arena, &frame.names),
            evidence,
            hypotheses: frame
                .hypotheses
                .iter()
                .map(|held| held.map(|held| shift(self.arena, held, 0, 2)))
                .collect(),
            companions: frame.companions,
            telescope,
        };
        let mut proof = self.split(inner, count, data, cases, how, reported)?;
        proof = self.arena.insert(Term::Lambda {
            domain: equation,
            body: proof,
        });
        proof = self.arena.insert(Term::Lambda {
            domain: subject,
            body: proof,
        });
        let reflexivity = self.arena.insert(Term::Refl { ty: subject, value });
        Ok(self.arena.apply_arguments(proof, &[value, reflexivity]))
    }

    /// The proof of the frame's goal in a case of fuel with a unit `rest`
    /// left, by the cases of its guard `ranked ∸ rest`: at zero the unit is
    /// spent doing nothing, so the law at the rest of the fuel and the same
    /// arguments is the goal; otherwise the definition runs its body, which
    /// `script` argues.
    #[allow(clippy::too_many_arguments)]
    fn guard(
        &mut self,
        frame: Frame,
        order: u32,
        ranked: u32,
        rest: TermHandle,
        data: u32,
        script: &LawCase,
        reported: Option<usize>,
    ) -> Result<TermHandle, Failure> {
        let Some(SignatureEntryKind::Data { shape, .. }) =
            self.entries.get(data as usize).map(|entry| &entry.kind)
        else {
            return Err(failure(reported, SearchError::NoProof));
        };
        let Some(&formal) = frame.names.get(ranked as usize) else {
            return Err(failure(reported, SearchError::NoProof));
        };
        let count = self.law.formals.len();
        let same = LawCase {
            citations: vec![Citation {
                evidence: Evidence::Fuel,
                arguments: (0..count)
                    .map(|formal| variable(self.arena, frame.names.len() - 1 - formal))
                    .collect(),
                descent: None,
            }],
            ..LawCase::default()
        };
        let cases: Vec<LawCase> = (0..shape.cases())
            .map(|case| {
                if shape.arity(case) == 0 {
                    same.clone()
                } else {
                    script.clone()
                }
            })
            .collect();
        let monus = constant(self.arena, order);
        let value = self.arena.apply_arguments(monus, &[formal, rest]);
        let mut context = self.closed.clone();
        for &ty in &frame.telescope {
            context = context.extend(ty);
        }
        self.split_value(frame, &context, value, data, &cases, Split::Guard, reported)
    }

    /// The proof of the frame's goal by the cases of the variable at `place`
    /// in its telescope.
    ///
    /// What the variable must satisfy
    /// is the goal for the variables bound after it, whose types may name
    /// it, so the motive binds those again after it, in their order. Each
    /// case is then proved for arbitrary fields with those variables bound
    /// anew at the case's value, and the whole is applied to the frame's own
    /// variables.
    ///
    /// An induction (`Split::Induction`, `Split::Fuel`) gives the law at each
    /// field of the case, which is what a self-citation names. Its motive
    /// binds the variables before the subject again too, so the hypothesis
    /// holds for every value of the other formals. A split inside a case
    /// leaves them bound where they are, which keeps the enclosing case's
    /// hypotheses out of its motive.
    fn split(
        &mut self,
        frame: Frame,
        place: usize,
        data: u32,
        cases: &[LawCase],
        how: Split,
        reported: Option<usize>,
    ) -> Result<TermHandle, Failure> {
        let induction = matches!(how, Split::Induction | Split::Fuel { .. });
        let unproved = || failure(reported, SearchError::NoProof);
        let Some(SignatureEntryKind::Data { shape, .. }) =
            self.entries.get(data as usize).map(|entry| &entry.kind)
        else {
            return Err(unproved());
        };
        // A law's induction over a member of a mutual block takes every
        // member's law; a case analysis is the block's recursor with motives
        // into `Two` at the other members (`PlainData::analysis_motives`).
        if shape.is_mutual() && how == Split::Induction {
            return self.mutual_induction(frame, place, data, shape, cases, reported);
        }
        if shape.is_mutual() && matches!(how, Split::Fuel { .. }) {
            return Err(unproved());
        }
        let count = frame.telescope.len();
        let Some(index) = count.checked_sub(place + 1) else {
            return Err(unproved());
        };
        if cases.len() != shape.cases() {
            return Err(unproved());
        }
        let kept = if induction { 0 } else { place };
        let others: Vec<usize> = (kept..count).filter(|&entry| entry != place).collect();
        // The variables of the frame as the new binders name them (`rename`).
        let renamed = |arena: &mut TermArena,
                       upto: usize,
                       between: usize,
                       value: &dyn Fn(&mut TermArena, u32) -> TermHandle|
         -> Vec<TermHandle> {
            rename(arena, &others, count, place, upto, between, value)
        };
        let bound = |arena: &mut TermArena, inner: u32| variable(arena, inner as usize);
        let actuals = renamed(self.arena, count, 1, &bound);
        let mut landing = instantiate(self.arena, frame.goal, &actuals);
        for &other in others.iter().rev() {
            let actuals = renamed(self.arena, other, 1, &bound);
            let domain = instantiate(self.arena, frame.telescope[other], &actuals);
            landing = self.arena.insert(Term::Pi {
                domain,
                codomain: landing,
            });
        }
        // The subject is a formal or a field: its type is a declared
        // constant, stated under nothing.
        let motive = self.arena.insert(Term::Lambda {
            domain: frame.telescope[place],
            body: landing,
        });
        let rest = others.len() as u32;
        let mut branches = Vec::with_capacity(cases.len());
        for (case, script) in cases.iter().enumerate() {
            let value =
                |arena: &mut TermArena, inner: u32| shape.case_value(arena, data, case, inner);
            // An induction's branch stands alone; a case split's keeps the
            // frame it is in.
            let binders = shape.case_binders(self.arena, data, motive, case);
            let between = binders.len();
            let mut telescope = if induction {
                Vec::new()
            } else {
                frame.telescope.clone()
            };
            telescope.extend(binders);
            let fields = telescope.len();
            for &other in &others {
                let actuals = renamed(self.arena, other, between, &value);
                telescope.push(instantiate(self.arena, frame.telescope[other], &actuals));
            }
            let actuals = renamed(self.arena, count, between, &value);
            let mut names: Vec<_> = frame
                .names
                .iter()
                .map(|&name| instantiate(self.arena, name, &actuals))
                .collect();
            // A unit of fuel, or what its guard computed, is no name of the
            // script's.
            if how == Split::Cases || how == Split::Induction {
                for field in 0..shape.arity(case) {
                    names.push(shape.field(self.arena, case, field, rest));
                }
            }
            let hypotheses = if induction {
                (0..shape.arity(case))
                    .map(|field| shape.hypothesis(self.arena, case, field, rest))
                    .collect()
            } else {
                frame
                    .hypotheses
                    .iter()
                    .map(|held| held.map(|held| instantiate(self.arena, held, &actuals)))
                    .collect()
            };
            let inner = Frame {
                goal: instantiate(self.arena, frame.goal, &actuals),
                names,
                evidence: frame
                    .evidence
                    .iter()
                    .map(|&held| instantiate(self.arena, held, &actuals))
                    .collect(),
                hypotheses,
                companions: if induction {
                    Vec::new()
                } else {
                    frame.companions.clone()
                },
                telescope,
            };
            let bound_again = inner.telescope[fields..].to_vec();
            let reported = reported.or(Some(case));
            let mut branch = match how {
                Split::Fuel { order, ranked } if shape.arity(case) == 1 => {
                    let rest = shape.field(self.arena, case, 0, rest);
                    self.guard(inner, order, ranked, rest, data, script, reported)?
                }
                _ => self.case(inner, script, reported)?,
            };
            for &domain in bound_again.iter().rev() {
                branch = self.arena.insert(Term::Lambda {
                    domain,
                    body: branch,
                });
            }
            branches.push(shape.branch(self.arena, data, motive, case, branch));
        }
        let tree = variable(self.arena, index);
        let cases = shape.induct(self.arena, data, motive, &branches, tree);
        let arguments: Vec<_> = others
            .iter()
            .map(|&other| variable(self.arena, count - 1 - other))
            .collect();
        Ok(self.arena.apply_arguments(cases, &arguments))
    }

    /// The proof of a law over a member of a mutual block of data by the
    /// block's induction. Its recursor takes a motive per member: the goal
    /// of the law of the group about that member (`LawProof::Cases`'s
    /// `mutual`), or, for a member no law is about, `Two`, whose branches
    /// ignore their fields. As in `split`, a law's motive binds its other
    /// formals and a proof of each premise again, so the hypothesis at a
    /// field holds for every value of them. Every case of every member is a
    /// branch, proved by its law's own script, in which the hypothesis at a
    /// field of a member is that member's law there (`Frame::companions`).
    fn mutual_induction(
        &mut self,
        frame: Frame,
        place: usize,
        data: u32,
        shape: &mathematical_kernel::PlainData,
        cases: &[LawCase],
        reported: Option<usize>,
    ) -> Result<TermHandle, Failure> {
        struct Framed {
            frame: Frame,
            place: usize,
            others: Vec<usize>,
        }
        let unproved = || failure(reported, SearchError::NoProof);
        let LawProof::Cases { mutual, .. } = &self.law.proof else {
            return Err(unproved());
        };
        let own = shape.member();
        if mutual.len() != shape.members() as usize
            || mutual[own as usize] != Some(self.law.machine)
        {
            return Err(unproved());
        }
        let laws: Vec<Option<&Law>> = mutual
            .iter()
            .enumerate()
            .map(|(member, machine)| match machine {
                None => Ok(None),
                Some(_) if member as u32 == own => Ok(Some(self.law)),
                Some(machine) => self
                    .laws
                    .iter()
                    .find(|law| law.machine == *machine)
                    .map(Some)
                    .ok_or_else(unproved),
            })
            .collect::<Result<_, _>>()?;
        // Each law's frame, the formal it dispatches on and its others.
        let mut own_frame = Some((frame, place));
        let mut framed = Vec::with_capacity(laws.len());
        for (member, law) in laws.iter().enumerate() {
            let Some(law) = law else {
                framed.push(None);
                continue;
            };
            let (frame, place) = if member as u32 == own {
                own_frame.take().ok_or_else(unproved)?
            } else {
                let LawProof::Cases { formal, .. } = &law.proof else {
                    return Err(unproved());
                };
                (law_frame(self.arena, law), *formal as usize)
            };
            let count = frame.telescope.len();
            if place >= count {
                return Err(unproved());
            }
            let others = (0..count).filter(|&entry| entry != place).collect();
            framed.push(Some(Framed {
                frame,
                place,
                others,
            }));
        }
        let first = shape.member_entry(data, 0);
        let two = self.arena.insert(Term::Two);
        let bound = |arena: &mut TermArena, inner: u32| variable(arena, inner as usize);
        let mut motives = Vec::with_capacity(framed.len());
        for (member, framed) in framed.iter().enumerate() {
            let Some(Framed {
                frame,
                place,
                others,
            }) = framed
            else {
                let domain = constant(self.arena, first + member as u32);
                motives.push(self.arena.insert(Term::Lambda { domain, body: two }));
                continue;
            };
            let count = frame.telescope.len();
            let actuals = rename(self.arena, others, count, *place, count, 1, &bound);
            let mut landing = instantiate(self.arena, frame.goal, &actuals);
            for &other in others.iter().rev() {
                let actuals = rename(self.arena, others, count, *place, other, 1, &bound);
                let domain = instantiate(self.arena, frame.telescope[other], &actuals);
                landing = self.arena.insert(Term::Pi {
                    domain,
                    codomain: landing,
                });
            }
            motives.push(self.arena.insert(Term::Lambda {
                domain: frame.telescope[*place],
                body: landing,
            }));
        }
        let mut branches = Vec::new();
        for (member, law) in laws.iter().enumerate() {
            let at = shape
                .at_member(member as u32)
                .expect("a member of the block");
            let entry = first + member as u32;
            let (
                Some(law),
                Some(Framed {
                    frame,
                    place,
                    others,
                }),
            ) = (law, &framed[member])
            else {
                for case in 0..at.cases() {
                    let ignored = self.arena.insert(Term::TwoZero);
                    branches.push(at.branch_in(self.arena, entry, &motives, case, ignored));
                }
                continue;
            };
            let LawProof::Cases { cases: scripts, .. } = &law.proof else {
                return Err(unproved());
            };
            let scripts = if member as u32 == own { cases } else { scripts };
            if scripts.len() != at.cases() {
                return Err(unproved());
            }
            let count = frame.telescope.len();
            let rest = others.len() as u32;
            for (case, script) in scripts.iter().enumerate() {
                let value =
                    |arena: &mut TermArena, inner: u32| at.case_value(arena, entry, case, inner);
                let mut telescope = at.case_binders_in(self.arena, entry, &motives, case);
                let between = telescope.len();
                let fields = telescope.len();
                for &other in others {
                    let actuals = rename(self.arena, others, count, *place, other, between, &value);
                    telescope.push(instantiate(self.arena, frame.telescope[other], &actuals));
                }
                let actuals = rename(self.arena, others, count, *place, count, between, &value);
                let mut names: Vec<_> = frame
                    .names
                    .iter()
                    .map(|&name| instantiate(self.arena, name, &actuals))
                    .collect();
                for field in 0..at.arity(case) {
                    names.push(at.field(self.arena, case, field, rest));
                }
                let hypotheses = (0..at.arity(case))
                    .map(|field| at.hypothesis(self.arena, case, field, rest))
                    .collect();
                let companions = (0..at.arity(case))
                    .map(|field| {
                        at.field_member(case, field)
                            .and_then(|of| mutual.get(of as usize).copied().flatten())
                    })
                    .collect();
                let inner = Frame {
                    goal: instantiate(self.arena, frame.goal, &actuals),
                    names,
                    evidence: frame
                        .evidence
                        .iter()
                        .map(|&held| instantiate(self.arena, held, &actuals))
                        .collect(),
                    hypotheses,
                    companions,
                    telescope,
                };
                let bound_again = inner.telescope[fields..].to_vec();
                // A case of another member is reported as its own law's.
                let reported = if member as u32 == own {
                    reported.or(Some(case))
                } else {
                    reported
                };
                let mut proving = Proving {
                    arena: &mut *self.arena,
                    closed: self.closed,
                    definitions: self.definitions,
                    licenses: self.licenses,
                    entries: self.entries,
                    laws: self.laws,
                    law,
                };
                let mut body = proving.case(inner, script, reported)?;
                for &domain in bound_again.iter().rev() {
                    body = self.arena.insert(Term::Lambda { domain, body });
                }
                branches.push(at.branch_in(self.arena, entry, &motives, case, body));
            }
        }
        let Some(Some(Framed {
            frame,
            place,
            others,
        })) = framed.get(own as usize)
        else {
            return Err(unproved());
        };
        let count = frame.telescope.len();
        let subject = variable(self.arena, count - 1 - place);
        let induction = shape.induct_in(
            self.arena,
            data,
            Level::Constant(0),
            &motives,
            &branches,
            subject,
        );
        let arguments: Vec<_> = others
            .iter()
            .map(|&other| variable(self.arena, count - 1 - other))
            .collect();
        Ok(self.arena.apply_arguments(induction, &arguments))
    }
}

/// Proves a theorem over data indexed by values by induction over the
/// value.
///
/// Each case of the data is a goal about integers: for arbitrary indices
/// and payload values of the case, from what the data's declaration
/// requires of the case and the theorem at each child the body cites, the
/// theorem at this index. The integer search finds a proof of each, the
/// kernel's builder assembles them into the induction, and the kernel
/// checks the whole once.
///
/// The search states a goal about named unknowns, so each case declares its
/// indices and payload values as unknown integers, proves the goal about
/// them, and then turns them into the binders of the case's lemma.
#[allow(clippy::too_many_arguments)]
fn prove_family(
    arena: &mut TermArena,
    checked_signature: &Signature,
    declarations: &[Declaration],
    entries: &[SignatureEntry],
    extras: &Extras<'_>,
    law: &Law,
    family: u32,
    invariant: TermHandle,
    cases: &[FamilyLawCase],
) -> Result<LawJudgment, Failure> {
    let refused = |case: Option<usize>, detail: String| Failure {
        case,
        cause: UnprovedCause::KernelRefused,
        detail,
        call: None,
    };
    let Some(declared) = extras.families.get(family as usize) else {
        return Err(refused(
            None,
            "theorem over indexed data without its declarations".to_owned(),
        ));
    };
    let positions = extras.integers.map(source_positions);
    let mut local = declarations[..law.statement as usize].to_vec();
    let statement = declarations[law.statement as usize].ty;
    let indices = law.formals.len();
    let instance = declared
        .at(arena, &[])
        .map_err(|error| refused(None, format!("indexed data: {error}")))?;
    // A family whose values are all integers is proved with the integer
    // search. One with a value of data (a derivation indexed by a context)
    // is proved by rewriting: its clauses are Identities over data.
    let integer = positions.map(|positions| constant(arena, positions.vocabulary.carrier));
    let integral = integer.is_some_and(|integer| {
        law.formals
            .iter()
            .chain(cases.iter().flat_map(|case| &case.payload_domains))
            .all(|&domain| arena.structurally_equal(domain, integer))
    });
    let checked = checked_signature.prefix(law.statement as usize);
    let definitions = Definitions::of_signature(&entries[..law.statement as usize]);
    let mut lemmas = Vec::with_capacity(cases.len());
    for (position, case) in cases.iter().enumerate() {
        let here = Some(position);
        let searched = |error: proof_admission::BoundedDenotationError| {
            refused(here, format!("integer search: {error:?}"))
        };
        // The laws a search appends belong to one case: the unknowns below
        // are removed again, which renumbers what follows them.
        let mut theory = match positions {
            Some(positions) if integral => Some(
                proof_admission::SourceIntegerTheory::resume(arena, &mut local, positions)
                    .map_err(searched)?,
            ),
            _ => None,
        };
        let first = local.len() as u32;
        let total = indices + case.payload as usize;
        if case.payload_domains.len() != case.payload as usize {
            return Err(refused(
                here,
                "case of indexed data without the type of each payload value".to_owned(),
            ));
        }
        let mut unknowns = Vec::with_capacity(total);
        for (unknown, &domain) in law.formals.iter().chain(&case.payload_domains).enumerate() {
            local.push(Declaration::assumption(0, domain));
            unknowns.push(constant(arena, first + unknown as u32));
        }
        // One case's proof of `goal` from `hypotheses`, as a function of
        // them.
        let mut derive = |arena: &mut TermArena,
                          local: &mut Vec<Declaration>,
                          goal: TermHandle,
                          hypotheses: &[TermHandle]|
         -> Result<Option<TermHandle>, Failure> {
            match theory.as_mut() {
                Some(theory) => theory
                    .predicate_from_assumptions(arena, local, goal, hypotheses, 0)
                    .map_err(searched),
                None => rewritten(arena, &checked, &definitions, local, goal, hypotheses)
                    .map_err(|detail| refused(here, detail)),
            }
        };
        let constraints: Vec<_> = case
            .constraints
            .iter()
            .map(|&constraint| instantiate(arena, constraint, &unknowns))
            .collect();
        for &owed in &case.owed {
            let goal = instantiate(arena, owed, &unknowns);
            if derive(arena, &mut local, goal, &constraints)?.is_none() {
                return Err(Failure {
                    case: here,
                    cause: UnprovedCause::NoDerivation,
                    detail: "what the step of this case requires does not follow from what the \
                             data requires of the case"
                        .to_owned(),
                    call: None,
                });
            }
        }
        // The theorem's rank is smaller at each child the body cites. An
        // index or payload value of an unsigned type is not negative, which
        // the family's integers do not say; it is a hypothesis here only.
        let mut counted = constraints.clone();
        counted.extend(
            case.natural
                .iter()
                .map(|&natural| instantiate(arena, natural, &unknowns)),
        );
        for &descent in &case.descent {
            let goal = instantiate(arena, descent, &unknowns);
            if derive(arena, &mut local, goal, &counted)?.is_none() {
                return Err(Failure {
                    case: here,
                    cause: UnprovedCause::NoDerivation,
                    detail: "the index the theorem is ranked on is not shown to be smaller at a \
                             child this case cites the theorem at"
                        .to_owned(),
                    call: None,
                });
            }
        }
        // The theorem at each child, and as a hypothesis where it is cited.
        let mut hypotheses = constraints.clone();
        let mut held = Vec::with_capacity(case.children.len());
        for (at, &cited) in case.children.iter().zip(&case.cited) {
            let at: Vec<_> = at
                .iter()
                .map(|&component| instantiate(arena, component, &unknowns))
                .collect();
            let theorem = instantiate(arena, law.goal, &at);
            held.push(theorem);
            if cited {
                hypotheses.push(theorem);
            }
        }
        let goal = instantiate(arena, law.goal, &unknowns[..indices]);
        let Some(proof) = derive(arena, &mut local, goal, &hypotheses)? else {
            return Err(Failure {
                case: here,
                cause: UnprovedCause::NoDerivation,
                detail: "no proof of this case from what the data requires of it and the theorem \
                         at the children the body cites"
                    .to_owned(),
                call: None,
            });
        };
        // The case's lemma for these unknowns: given one proof of all the
        // constraints and the theorem at every child, the goal.
        let children = held.len();
        let mut arguments = Vec::with_capacity(hypotheses.len());
        let all = variable(arena, children);
        for constraint in 0..constraints.len() {
            arguments.push(arena.tuple_component(all, constraint, constraints.len()));
        }
        for (child, &cited) in case.cited.iter().enumerate() {
            if cited {
                arguments.push(variable(arena, children - 1 - child));
            }
        }
        let mut lemma = arena.apply_arguments(proof, &arguments);
        let mut lemma_type = goal;
        for &theorem in held.iter().rev() {
            lemma = arena.insert(Term::Lambda {
                domain: theorem,
                body: lemma,
            });
            lemma_type = arena.insert(Term::Pi {
                domain: theorem,
                codomain: lemma_type,
            });
        }
        let Some((&last, before)) = constraints.split_last() else {
            return Err(refused(
                here,
                "case of indexed data with no constraint".to_owned(),
            ));
        };
        let mut all = last;
        for &constraint in before.iter().rev() {
            all = arena.insert(Term::Sigma {
                domain: constraint,
                codomain: all,
            });
        }
        lemma = arena.insert(Term::Lambda {
            domain: all,
            body: lemma,
        });
        lemma_type = arena.insert(Term::Pi {
            domain: all,
            codomain: lemma_type,
        });
        let certificate =
            MathematicalCertificate::with_signature(local, Vec::new(), lemma, lemma_type);
        let selected: Vec<u32> = (first..first + total as u32).collect();
        let abstracted = mathematical_kernel::abstract_certificate_assumptions(
            arena,
            &certificate,
            &selected,
            &mut Budget::default(),
        )
        .map_err(|error| refused(here, format!("case lemma: {error:?}")))?;
        local = abstracted.certificate.signature;
        lemmas.push(abstracted.certificate.term);
    }
    let induction = instance
        .induction(arena, &local, invariant, &lemmas)
        .map_err(|error| refused(None, format!("indexed induction: {error}")))?;
    let mut check = Budget::default();
    let certificate =
        MathematicalCertificate::with_signature(local, Vec::new(), induction.theorem, statement);
    let receipt =
        MathematicalJudgmentReceipt::verify(arena, &certificate, &mut check).map_err(kernel)?;
    judged(arena, law, &certificate, receipt, &check, Some(law.machine))
}

/// A proof of `goal` from `hypotheses` by rewriting with them, as a function
/// of them: each is assumed after `local`, cited, and abstracted again.
/// `None` when the search finds none.
fn rewritten(
    arena: &mut TermArena,
    checked: &Signature,
    definitions: &Definitions,
    local: &[Declaration],
    goal: TermHandle,
    hypotheses: &[TermHandle],
) -> Result<Option<TermHandle>, String> {
    let mut added = local[checked.len()..].to_vec();
    let first = local.len() as u32;
    added.extend(
        hypotheses
            .iter()
            .map(|&hypothesis| Declaration::assumption(0, hypothesis)),
    );
    let signature = proof_admission::extend_signature(
        arena,
        checked,
        &added,
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    )
    .map_err(|refused| format!("case hypotheses: {:?}", refused.error))?;
    let context = Context::empty().with_signature(signature);
    let held: Vec<_> = (0..hypotheses.len() as u32)
        .map(|hypothesis| constant(arena, first + hypothesis))
        .collect();
    let proof = match prove_licensed(
        arena,
        &context,
        definitions,
        goal,
        &held,
        Licenses {
            operations: &[],
            distributions: &[],
            index_expansion_terms: 0,
        },
        SEARCH_STEPS,
        &mut Budget::new(SEARCH_COMPUTING),
    ) {
        Ok(proof) => proof,
        Err(SearchError::NoProof | SearchError::Budget(_)) => return Ok(None),
        Err(error) => return Err(format!("rewriting: {error:?}")),
    };
    // Innermost the last hypothesis, as the caller applies them in order.
    let mut proof = proof;
    for (&hypothesis, &named) in hypotheses.iter().zip(&held).rev() {
        let body = abstracted(arena, proof, named);
        proof = arena.insert(Term::Lambda {
            domain: hypothesis,
            body,
        });
    }
    Ok(Some(proof))
}

/// The judgment for `law` once the kernel accepted `certificate`: its
/// encoded bytes and the conversion steps `check` spent finding and checking
/// it.
fn judged(
    arena: &TermArena,
    law: &Law,
    certificate: &MathematicalCertificate,
    receipt: MathematicalJudgmentReceipt,
    check: &Budget,
    theorem: Option<symbols::SymbolHandle>,
) -> Result<LawJudgment, Failure> {
    let bytes =
        terminal_codec::encode_mathematical_certificate(arena, certificate).map_err(|_| {
            Failure {
                case: None,
                cause: UnprovedCause::KernelRefused,
                detail: "could not encode the judged mathematical term".to_owned(),
                call: None,
            }
        })?;
    Ok(LawJudgment {
        machine: law.machine_name.clone(),
        clause: law.clause,
        decision: MathematicalCoreDecision::Judged(receipt),
        certificate: bytes,
        conversion_steps: DEFAULT_CONVERSION_STEPS - check.remaining(),
        theorem,
    })
}

/// Proves a goal about integers from the law's premises with the integer
/// search: what a case of indexed data requires of a value a machine builds,
/// from what that machine requires of its value parameters.
fn prove_integers(
    arena: &mut TermArena,
    declarations: &[Declaration],
    extras: &Extras<'_>,
    law: &Law,
) -> Result<LawJudgment, Failure> {
    let refused = |detail: String| Failure {
        case: None,
        cause: UnprovedCause::KernelRefused,
        detail,
        call: None,
    };
    let Some(positions) = extras.integers else {
        return Err(refused(
            "goal about integers without the integer vocabulary".to_owned(),
        ));
    };
    let positions = source_positions(positions);
    let mut local = declarations[..law.statement as usize].to_vec();
    let statement = declarations[law.statement as usize].ty;
    let searched = |error: proof_admission::BoundedDenotationError| {
        refused(format!("integer search: {error:?}"))
    };
    let mut theory = proof_admission::SourceIntegerTheory::resume(arena, &mut local, positions)
        .map_err(searched)?;
    let integer = constant(arena, positions.vocabulary.carrier);
    let first = local.len() as u32;
    let total = law.formals.len();
    let mut unknowns = Vec::with_capacity(total);
    for unknown in 0..total {
        local.push(Declaration::assumption(0, integer));
        unknowns.push(constant(arena, first + unknown as u32));
    }
    let hypotheses: Vec<_> = law
        .premises
        .iter()
        .map(|&premise| instantiate(arena, premise, &unknowns))
        .collect();
    let goal = instantiate(arena, law.goal, &unknowns);
    let Some(proof) = theory
        .predicate_from_assumptions(arena, &mut local, goal, &hypotheses, 0)
        .map_err(searched)?
    else {
        return Err(Failure {
            case: None,
            cause: UnprovedCause::NoDerivation,
            detail: "no proof from what the machine requires of its value parameters".to_owned(),
            call: None,
        });
    };
    let mut proved = goal;
    for &hypothesis in hypotheses.iter().rev() {
        proved = arena.insert(Term::Pi {
            domain: hypothesis,
            codomain: proved,
        });
    }
    let certificate = MathematicalCertificate::with_signature(local, Vec::new(), proof, proved);
    let selected: Vec<u32> = (first..first + total as u32).collect();
    let abstracted = mathematical_kernel::abstract_certificate_assumptions(
        arena,
        &certificate,
        &selected,
        &mut Budget::default(),
    )
    .map_err(|error| refused(format!("integer goal: {error:?}")))?;
    let mut check = Budget::default();
    let certificate = MathematicalCertificate::with_signature(
        abstracted.certificate.signature,
        Vec::new(),
        abstracted.certificate.term,
        statement,
    );
    let receipt =
        MathematicalJudgmentReceipt::verify(arena, &certificate, &mut check).map_err(kernel)?;
    judged(arena, law, &certificate, receipt, &check, None)
}

fn source_positions(positions: IntegerPositions) -> proof_admission::SourceIntegerPositions {
    proof_admission::SourceIntegerPositions {
        vocabulary: proof_admission::SourceIntegerVocabulary {
            carrier: positions.carrier,
            less_than: positions.less_than,
            less_or_equal: positions.less_or_equal,
            add: positions.add,
            subtract: positions.subtract,
            multiply: positions.multiply,
        },
        numeral_zero: positions.numeral_zero,
        numeral_double: positions.numeral_double,
        numeral_odd: positions.numeral_odd,
        numeral_negate: positions.numeral_negate,
    }
}

/// What a law over indexed data needs beside the declarations: where the
/// integer vocabulary is, and each indexed family as it was declared.
struct Extras<'s> {
    integers: Option<IntegerPositions>,
    families: &'s [mathematical_kernel::DeclaredIndexedData],
}

#[allow(clippy::too_many_arguments)]
/// The first `upto` variables of a frame of `count` as an induction's new
/// binders name them. `between` binders stand between the frame and the
/// variables bound again (`others`): the motive's one, or a case's fields
/// and hypotheses. The subject, at `place`, is `value`, which is told how
/// many of the variables bound again are beneath it; each of those is its
/// own new binder; a variable kept is itself, further out.
fn rename(
    arena: &mut TermArena,
    others: &[usize],
    count: usize,
    place: usize,
    upto: usize,
    between: usize,
    value: &dyn Fn(&mut TermArena, u32) -> TermHandle,
) -> Vec<TermHandle> {
    let inner = others.iter().filter(|&&other| other < upto).count();
    (0..upto)
        .map(
            |entry| match others.iter().position(|&other| other == entry) {
                Some(position) => variable(arena, inner - 1 - position),
                None if entry == place => value(arena, inner as u32),
                None => variable(arena, inner + between + count - 1 - entry),
            },
        )
        .collect()
}

/// Where a law's proof starts: under its formals and a proof of each
/// premise, bound after the formals, innermost last.
fn law_frame(arena: &mut TermArena, law: &Law) -> Frame {
    let count = law.formals.len();
    let held = law.premises.len() as u32;
    let mut telescope = law.formals.clone();
    for (position, &premise) in law.premises.iter().enumerate() {
        telescope.push(shift(arena, premise, 0, position as u32));
    }
    Frame {
        goal: shift(arena, law.goal, 0, held),
        names: (0..count)
            .map(|formal| variable(arena, count - 1 - formal + held as usize))
            .collect(),
        evidence: (0..held)
            .map(|premise| variable(arena, (held - 1 - premise) as usize))
            .collect(),
        hypotheses: Vec::new(),
        companions: Vec::new(),
        telescope,
    }
}

fn prove_law(
    arena: &mut TermArena,
    checked_signature: &Signature,
    declarations: &[Declaration],
    entries: &[SignatureEntry],
    laws: &[Law],
    granted: &[OperandLicense],
    policy: crate::ProofWorkPolicy,
    law: &Law,
) -> Result<LawJudgment, Failure> {
    let (operations, distributions) = licenses(arena, declarations, granted, law);
    let licenses = Licenses {
        operations: &operations,
        distributions: &distributions,
        index_expansion_terms: policy.index_expansion_terms,
    };
    let definitions = Definitions::of_signature(&entries[..law.statement as usize]);
    let prefix = declarations[..law.statement as usize].to_vec();
    let closed = Context::empty().with_signature(checked_signature.prefix(law.statement as usize));
    let statement = declarations[law.statement as usize].ty;
    let count = law.formals.len();
    let held = law.premises.len() as u32;
    let frame = law_frame(arena, law);
    let mut proving = Proving {
        arena,
        closed: &closed,
        definitions: &definitions,
        licenses,
        entries,
        laws,
        law,
    };
    let body = match &law.proof {
        // `judge` sends a theorem over indexed data to `prove_family` and a
        // goal about integers to `prove_integers`.
        LawProof::Family { .. } | LawProof::Integers => {
            return Err(Failure {
                case: None,
                cause: UnprovedCause::KernelRefused,
                detail: "theorem over indexed data read as a plain law".to_owned(),
                call: None,
            });
        }
        LawProof::Direct(direct) => proving.case(frame, direct, None)?,
        LawProof::Cases {
            formal,
            data,
            cases,
            ..
        } => proving.split(
            frame,
            *formal as usize,
            *data,
            cases,
            Split::Induction,
            None,
        )?,
        // The law for every amount of fuel, then at the machine's own: its
        // ranked formal. The fuel is one more variable, bound first; every
        // case of it runs the same script, a unit of fuel after the split
        // on its guard.
        LawProof::Fuel {
            data,
            formal,
            order,
            goal,
            script,
        } => {
            let Some(SignatureEntryKind::Data { shape, .. }) =
                entries.get(*data as usize).map(|entry| &entry.kind)
            else {
                return Err(failure(None, SearchError::NoProof));
            };
            let ranked = *formal as usize;
            if ranked >= count {
                return Err(failure(None, SearchError::NoProof));
            }
            let fuel = constant(proving.arena, *data);
            let mut telescope = vec![fuel];
            telescope.extend(frame.telescope.iter().copied());
            let over_fuel = Frame {
                goal: shift(proving.arena, *goal, 0, held),
                telescope,
                ..frame
            };
            let scripts = vec![script.clone(); shape.cases()];
            let how = Split::Fuel {
                order: *order,
                ranked: *formal,
            };
            let general = proving.split(over_fuel, 0, *data, &scripts, how, None)?;
            // `general` is stated under the fuel, the formals and the
            // premises. Here the fuel is the ranked formal.
            let mut actuals = vec![variable(proving.arena, count - 1 - ranked + held as usize)];
            actuals.extend(
                (0..count + held as usize)
                    .map(|entry| variable(proving.arena, count + held as usize - 1 - entry)),
            );
            instantiate(proving.arena, general, &actuals)
        }
    };
    let mut proof = body;
    for (position, &premise) in law.premises.iter().enumerate().rev() {
        let domain = shift(arena, premise, 0, position as u32);
        proof = arena.insert(Term::Lambda {
            domain,
            body: proof,
        });
    }
    for &formal in law.formals.iter().rev() {
        proof = arena.insert(Term::Lambda {
            domain: formal,
            body: proof,
        });
    }
    // The kernel checks the proof once, as the certificate it will publish.
    // That check has the whole of a law's budget. Each search for a proof
    // or a premise had one of its own, so what a search computed on the way
    // is no part of what the law costs to check.
    let mut check = Budget::default();
    let certificate = MathematicalCertificate::with_signature(prefix, Vec::new(), proof, statement);
    let receipt =
        MathematicalJudgmentReceipt::verify(arena, &certificate, &mut check).map_err(kernel)?;
    if let Some(declared) = &law.declared {
        // Something other than the machine decides what it must prove: the
        // requirement it satisfies, or the declaration that selects it. The
        // statement it proved must be that one, up to the kernel's own
        // computation: the same equality from the same premises, in any
        // order.
        let mut under = closed.clone();
        for &formal in &law.formals {
            under = under.extend(formal);
        }
        let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let differs = |cause| Failure {
            case: None,
            cause,
            detail: declared.refusal.clone(),
            call: None,
        };
        let mut same = |arena: &mut TermArena, proved, required| match convertible(
            arena, &under, proved, required, universe, &mut check,
        ) {
            Ok(same) => Ok(same),
            Err(CoreError::StepCeiling) => Err(differs(UnprovedCause::BudgetExhausted)),
            Err(_) => Ok(false),
        };
        let mut unmatched = law.premises.clone();
        let mut agrees =
            unmatched.len() == declared.premises.len() && same(arena, law.goal, declared.goal)?;
        for &required in &declared.premises {
            if !agrees {
                break;
            }
            let mut found = None;
            for (position, &proved) in unmatched.iter().enumerate() {
                if same(arena, proved, required)? {
                    found = Some(position);
                    break;
                }
            }
            match found {
                Some(position) => {
                    unmatched.swap_remove(position);
                }
                None => agrees = false,
            }
        }
        if !agrees {
            return Err(differs(UnprovedCause::NoDerivation));
        }
    }
    judged(arena, law, &certificate, receipt, &check, None)
}

/// What was left unproved, with the dispatched formal replaced by the case
/// that failed.
fn statement(
    arena: &mut TermArena,
    entries: &[SignatureEntry],
    law: &Law,
    case: Option<usize>,
) -> String {
    // What a literal of indexed data owes is named by its case.
    if matches!(law.proof, LawProof::Integers) {
        return format!("cannot prove {} in `{}`", law.text, law.machine_name);
    }
    // A theorem over indexed data states an order of integers, which the
    // clause as written says best.
    if let LawProof::Family { cases, .. } = &law.proof {
        let within = case
            .and_then(|case| cases.get(case))
            .map(|case| format!(" in the case `{}`", case.name))
            .unwrap_or_default();
        return format!(
            "cannot prove ensures contract for `{}`{within}: {}",
            law.machine_name, law.text
        );
    }
    let count = law.formals.len();
    let mut names = law.formal_names.clone();
    names.resize(count, "?".to_owned());
    let mut within = String::new();
    let mut goal = law.goal;
    if let (
        Some(case),
        LawProof::Cases {
            formal,
            data,
            cases,
            ..
        },
    ) = (case, &law.proof)
        && let Some(script) = cases.get(case)
    {
        let arity = script.field_names.len();
        let constructor = match entries.get(*data as usize).map(|entry| &entry.kind) {
            Some(SignatureEntryKind::Data { shape, .. }) => shape.constructor_entry(*data, case),
            _ => *data + 1 + case as u32,
        };
        let head = constant(arena, constructor);
        let fields: Vec<_> = (0..arity)
            .map(|field| variable(arena, arity - 1 - field))
            .collect();
        let value = arena.apply_arguments(head, &fields);
        let actuals: Vec<_> = (0..count)
            .map(|position| {
                if position == *formal as usize {
                    value
                } else {
                    variable(arena, count - 1 - position + arity)
                }
            })
            .collect();
        goal = instantiate(arena, law.goal, &actuals);
        names.extend(script.field_names.iter().cloned());
        within = format!(
            " in the case `{}`",
            entries
                .get(constructor as usize)
                .map_or("?", |entry| entry.name.as_str())
        );
    }
    let mut equality = render(arena, entries, goal, &names);
    // A field of a record is a projection of a tuple, which carries no name
    // to print. The clause as written says it, where there is one to quote.
    if equality.contains('?') && case.is_none() && !law.text.is_empty() {
        equality.clone_from(&law.text);
    }
    if law.text.is_empty() {
        format!(
            "cannot prove the law `{}` inherits from the requirement it satisfies{within}: {equality}",
            law.machine_name
        )
    } else {
        format!(
            "cannot prove ensures contract for `{}`{within}: {equality}",
            law.machine_name
        )
    }
}

/// A term of the signature in source spelling: a formal or field by name, a
/// declaration by name and its arguments, an equality as `core\Identity`.
fn render(
    arena: &TermArena,
    entries: &[SignatureEntry],
    term: TermHandle,
    names: &[String],
) -> String {
    match arena.get(term) {
        Term::Variable(index) => names
            .len()
            .checked_sub(index as usize + 1)
            .and_then(|position| names.get(position))
            .cloned()
            .unwrap_or_else(|| "?".to_owned()),
        Term::Constant { declaration, .. } => entries
            .get(declaration as usize)
            .map_or_else(|| "?".to_owned(), |entry| entry.name.clone()),
        Term::Apply { .. } => {
            let mut head = term;
            let mut arguments = Vec::new();
            while let Term::Apply { function, argument } = arena.get(head) {
                arguments.push(render(arena, entries, argument, names));
                head = function;
            }
            arguments.reverse();
            format!(
                "{}({})",
                render(arena, entries, head, names),
                arguments.join(", ")
            )
        }
        Term::Id { ty, left, right } => format!(
            "core\\Identity::<{}>({}, {})",
            render(arena, entries, ty, names),
            render(arena, entries, left, names),
            render(arena, entries, right, names)
        ),
        _ => "?".to_owned(),
    }
}
