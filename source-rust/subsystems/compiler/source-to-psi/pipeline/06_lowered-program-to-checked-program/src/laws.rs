//! Judges the compilation's mathematical signature.
//!
//! The kernel checks every declaration once. Each law is then proved against
//! the declarations before its own statement, so a law is never among its own
//! evidence. A law whose body dispatches on a formal is an induction: each
//! case is an equality over an arbitrary value of that case, proved by
//! rewriting with the laws the case cites and with the law itself at a field
//! of the case, and the cases are assembled into one `indW` term. The kernel
//! checks that term against the law's statement, and the encoded certificate
//! is what the judgment reports.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, MathematicalCertificate,
    MathematicalCoreDecision, MathematicalJudgmentReceipt, Signature, Term, TermArena, TermHandle,
    check_signature_with_attribution, check_type, instantiate,
};
use terminal_check_tables::UnprovedCause;
use terminal_check_tables::mathematics::{
    Evidence, Law, LawCase, LawProof, MathematicalSignature, OperandLaw, OperandLicense,
    SignatureEntry, SignatureEntryKind,
};

use super::equality::{License, SearchError, prove_under};

/// How many rewrite attempts one case of one law may make.
const SEARCH_STEPS: usize = 65_536;

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

/// The carrier of every data declaration the kernel has checked.
fn carriers(
    arena: &mut TermArena,
    declarations: &[Declaration],
    entries: &[SignatureEntry],
) -> Result<Vec<DataCarrier>, terminal_codec::CodecError> {
    let mut carriers = Vec::new();
    for (position, entry) in entries.iter().enumerate() {
        let SignatureEntryKind::Data { type_identity, .. } = &entry.kind else {
            continue;
        };
        let ty = constant(arena, position as u32);
        let certificate = MathematicalCertificate::with_signature(
            declarations[..=position].to_vec(),
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
}

fn failure(case: Option<usize>, error: SearchError) -> Failure {
    let (cause, detail) = match error {
        SearchError::Budget => (
            UnprovedCause::BudgetExhausted,
            "mathematical equality proof budget exhausted".to_owned(),
        ),
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
    }
}

fn kernel(error: CoreError) -> Failure {
    failure(None, error.into())
}

pub(crate) fn judge(
    signature: MathematicalSignature,
) -> Result<SignatureJudgment, Vec<UnprovedLaw>> {
    let MathematicalSignature {
        mut arena,
        declarations,
        entries,
        laws,
        licenses,
    } = signature;
    let steps = DEFAULT_CONVERSION_STEPS.saturating_mul(declarations.len() as u32 + 1);
    if let Err(refused) =
        check_signature_with_attribution(&mut arena, &declarations, &mut Budget::new(steps))
    {
        let name = entries
            .get(refused.declaration)
            .map_or("?", |entry| entry.name.as_str());
        let Failure { cause, detail, .. } = kernel(refused.error);
        return Err(vec![UnprovedLaw {
            machine: name.to_owned(),
            site: None,
            cause,
            statement: format!("the mathematical declaration `{name}` is not well formed"),
            detail,
        }]);
    }
    let carriers = carriers(&mut arena, &declarations, &entries).map_err(|_| {
        vec![UnprovedLaw {
            machine: String::new(),
            site: None,
            cause: UnprovedCause::KernelRefused,
            statement: "the mathematical declarations cannot be published".to_owned(),
            detail: "could not encode a data declaration's carrier key".to_owned(),
        }]
    })?;
    let mut judged = Vec::with_capacity(laws.len());
    let mut unproved = Vec::new();
    for law in &laws {
        match prove_law(&mut arena, &declarations, &entries, &licenses, law) {
            Ok(judgment) => judged.push(judgment),
            Err(Failure {
                case,
                cause,
                detail,
            }) => unproved.push(UnprovedLaw {
                machine: law.machine_name.clone(),
                site: case
                    .and_then(|case| script(law, case))
                    .and_then(|script| script.source)
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
        Err(unproved)
    }
}

fn script(law: &Law, case: usize) -> Option<&LawCase> {
    match &law.proof {
        LawProof::Direct(_) => None,
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
/// each with the laws that say so: the licenses stated before the law. That
/// keeps a law out of its own proof and keeps two laws from resting on each
/// other. The search puts each law it uses into the proof term, so a wrong
/// license fails the kernel's check and the certificate names every law the
/// proof relied on.
fn licenses(
    arena: &mut TermArena,
    declarations: &[Declaration],
    granted: &[OperandLicense],
    law: &Law,
) -> Vec<License> {
    let mut found: Vec<(u32, License)> = Vec::new();
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
        }
    }
    found.into_iter().map(|(_, license)| license).collect()
}

fn prove_law(
    arena: &mut TermArena,
    declarations: &[Declaration],
    entries: &[SignatureEntry],
    granted: &[OperandLicense],
    law: &Law,
) -> Result<LawJudgment, Failure> {
    let licenses = licenses(arena, declarations, granted, law);
    let prefix = declarations[..law.statement as usize].to_vec();
    let closed = Context::empty().with_signature(Signature::from_declarations(prefix.clone()));
    let statement = declarations[law.statement as usize].ty;
    let count = law.formals.len();
    let mut budget = Budget::default();
    let body = match &law.proof {
        LawProof::Direct(direct) => {
            let mut context = closed.clone();
            for &formal in &law.formals {
                context = context.extend(formal);
            }
            let mut evidence = Vec::with_capacity(direct.citations.len());
            for citation in &direct.citations {
                let Evidence::Statement(cited) = citation.evidence else {
                    return Err(failure(None, SearchError::NoProof));
                };
                let head = constant(arena, cited);
                evidence.push(arena.apply_arguments(head, &citation.arguments));
            }
            prove_under(
                arena,
                &context,
                law.goal,
                &evidence,
                &licenses,
                SEARCH_STEPS,
                &mut budget,
            )
            .map_err(|error| failure(None, error))?
        }
        LawProof::Cases {
            formal,
            data,
            cases,
        } => {
            let subject = *formal as usize;
            let Some(SignatureEntryKind::Data { shape, .. }) =
                entries.get(*data as usize).map(|entry| &entry.kind)
            else {
                return Err(failure(None, SearchError::NoProof));
            };
            let others: Vec<usize> = (0..count).filter(|formal| *formal != subject).collect();
            let rest = others.len();
            // Each formal as a branch names it: the subject is replaced, the
            // others are bound again inside the motive and inside each branch.
            let outer = |arena: &mut TermArena, subject_term: TermHandle| -> Vec<TermHandle> {
                (0..count)
                    .map(
                        |formal| match others.iter().position(|other| *other == formal) {
                            Some(position) => variable(arena, rest - 1 - position),
                            None => subject_term,
                        },
                    )
                    .collect()
            };
            let tree = variable(arena, rest);
            let actuals = outer(arena, tree);
            let mut landing = instantiate(arena, law.goal, &actuals);
            for &other in others.iter().rev() {
                landing = arena.insert(Term::Pi {
                    domain: law.formals[other],
                    codomain: landing,
                });
            }
            let motive = arena.insert(Term::Lambda {
                domain: law.formals[subject],
                body: landing,
            });
            let mut branches = Vec::with_capacity(cases.len());
            for (case, script) in cases.iter().enumerate() {
                let (function, hypothesis) = shape.case_binders(arena, motive, case);
                let mut context = closed.extend(function).extend(hypothesis);
                for &other in &others {
                    context = context.extend(law.formals[other]);
                }
                let node = shape.node(arena, case, rest as u32);
                let mut actuals = outer(arena, node);
                let goal = instantiate(arena, law.goal, &actuals);
                for field in 0..shape.arity(case) {
                    actuals.push(shape.field(arena, case, field, rest as u32));
                }
                let mut evidence = Vec::with_capacity(script.citations.len());
                for citation in &script.citations {
                    let head = match citation.evidence {
                        Evidence::Statement(cited) => constant(arena, cited),
                        Evidence::Hypothesis { field } => {
                            shape.hypothesis(arena, case, field, rest as u32)
                        }
                    };
                    let arguments: Vec<_> = citation
                        .arguments
                        .iter()
                        .map(|&argument| instantiate(arena, argument, &actuals))
                        .collect();
                    evidence.push(arena.apply_arguments(head, &arguments));
                }
                let mut branch = prove_under(
                    arena,
                    &context,
                    goal,
                    &evidence,
                    &licenses,
                    SEARCH_STEPS,
                    &mut budget,
                )
                .map_err(|error| failure(Some(case), error))?;
                for &other in others.iter().rev() {
                    branch = arena.insert(Term::Lambda {
                        domain: law.formals[other],
                        body: branch,
                    });
                }
                branches.push(branch);
            }
            let step = shape.step(arena, motive, &branches);
            let tree = variable(arena, count - 1 - subject);
            let induction = arena.insert(Term::IndW { motive, step, tree });
            let arguments: Vec<_> = others
                .iter()
                .map(|&other| variable(arena, count - 1 - other))
                .collect();
            arena.apply_arguments(induction, &arguments)
        }
    };
    let mut proof = body;
    for &formal in law.formals.iter().rev() {
        proof = arena.insert(Term::Lambda {
            domain: formal,
            body: proof,
        });
    }
    check_type(arena, &closed, proof, statement, &mut budget).map_err(kernel)?;
    if let Some(declared) = law.declared {
        // The machine satisfies a requirement: what it proved must be the
        // law that requirement declares, up to the kernel's own computation.
        let mut required = declared;
        for &formal in law.formals.iter().rev() {
            required = arena.insert(Term::Pi {
                domain: formal,
                codomain: required,
            });
        }
        check_type(arena, &closed, proof, required, &mut budget).map_err(|error| Failure {
            case: None,
            cause: match error {
                CoreError::StepCeiling => UnprovedCause::BudgetExhausted,
                _ => UnprovedCause::NoDerivation,
            },
            detail: "the proved equality is not the law its requirement declares".to_owned(),
        })?;
    }
    let certificate = MathematicalCertificate::with_signature(prefix, Vec::new(), proof, statement);
    let receipt =
        MathematicalJudgmentReceipt::verify(arena, &certificate, &mut budget).map_err(kernel)?;
    let bytes =
        terminal_codec::encode_mathematical_certificate(arena, &certificate).map_err(|_| {
            Failure {
                case: None,
                cause: UnprovedCause::KernelRefused,
                detail: "could not encode the judged mathematical term".to_owned(),
            }
        })?;
    Ok(LawJudgment {
        machine: law.machine_name.clone(),
        clause: law.clause,
        decision: MathematicalCoreDecision::Judged(receipt),
        certificate: bytes,
        conversion_steps: DEFAULT_CONVERSION_STEPS - budget.remaining(),
    })
}

/// What was left unproved, with the dispatched formal replaced by the case
/// that failed.
fn statement(
    arena: &mut TermArena,
    entries: &[SignatureEntry],
    law: &Law,
    case: Option<usize>,
) -> String {
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
        },
    ) = (case, &law.proof)
        && let Some(script) = cases.get(case)
    {
        let arity = script.field_names.len();
        let constructor = *data + 1 + case as u32;
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
    let equality = render(arena, entries, goal, &names);
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
