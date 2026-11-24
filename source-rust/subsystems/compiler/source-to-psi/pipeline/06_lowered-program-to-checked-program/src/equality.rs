//! Rewrite a goal with the equality terms available at that program point.
//! Each visited expression carries a proof from the original left endpoint.

use proof_admission::{
    Budget, Context, CoreError, Term, TermArena, TermHandle, check_type, infer_type, shift,
    weak_head_normalize,
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SearchError {
    NoProof,
    Budget,
    Kernel(CoreError),
}

impl From<CoreError> for SearchError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::StepCeiling => Self::Budget,
            error => Self::Kernel(error),
        }
    }
}

#[derive(Clone, Copy)]
struct Equality {
    carrier: TermHandle,
    from: TermHandle,
    to: TermHandle,
    proof: TermHandle,
}

impl Equality {
    /// Eliminate this equality with a conclusion under two new binders:
    /// the destination and the proof that the source equals it.
    fn eliminate(
        self,
        arena: &mut TermArena,
        conclusion: TermHandle,
        base: TermHandle,
    ) -> TermHandle {
        let ty = shift(arena, self.carrier, 0, 1);
        let left = shift(arena, self.from, 0, 1);
        let right = arena.insert(Term::Variable(0));
        let domain = arena.insert(Term::Id { ty, left, right });
        let body = arena.insert(Term::Lambda {
            domain,
            body: conclusion,
        });
        let motive = arena.insert(Term::Lambda {
            domain: self.carrier,
            body,
        });
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: self.to,
            proof: self.proof,
        })
    }

    fn reversed(self, arena: &mut TermArena) -> Self {
        let ty = shift(arena, self.carrier, 0, 2);
        let left = arena.insert(Term::Variable(1));
        let right = shift(arena, self.from, 0, 2);
        let conclusion = arena.insert(Term::Id { ty, left, right });
        let base = arena.insert(Term::Refl {
            ty: self.carrier,
            value: self.from,
        });
        Self {
            from: self.to,
            to: self.from,
            proof: self.eliminate(arena, conclusion, base),
            ..self
        }
    }
}

/// A path through ordinary applications keeps exactly the surrounding
/// operands needed to rebuild a term around the rewritten subexpression.
#[derive(Clone, Copy)]
enum Surrounding {
    FunctionOf(TermHandle),
    ArgumentOf(TermHandle),
}

fn plug(
    arena: &mut TermArena,
    path: &[Surrounding],
    mut value: TermHandle,
    binders: u32,
) -> TermHandle {
    for frame in path.iter().rev() {
        let (function, argument) = match *frame {
            Surrounding::FunctionOf(argument) => (value, shift(arena, argument, 0, binders)),
            Surrounding::ArgumentOf(function) => (shift(arena, function, 0, binders), value),
        };
        value = arena.insert(Term::Apply { function, argument });
    }
    value
}

fn charge(remaining: &mut usize) -> Result<(), SearchError> {
    *remaining = remaining.checked_sub(1).ok_or(SearchError::Budget)?;
    Ok(())
}

/// Citations are instantiated terms from the checked signature. Inferring
/// their types keeps both endpoints tied to the actual evidence supplied.
pub(crate) fn prove(
    arena: &mut TermArena,
    context: &Context,
    goal: TermHandle,
    citations: &[TermHandle],
    mut search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let Term::Id { ty, left, right } = arena.get(goal) else {
        return Err(SearchError::NoProof);
    };
    let reflexivity = arena.insert(Term::Refl { ty, value: left });
    if arena.structurally_equal(left, right) {
        check_type(arena, context, reflexivity, goal, conversion)?;
        return Ok(reflexivity);
    }
    let mut rules = Vec::new();
    for &proof in citations {
        charge(&mut search_steps)?;
        let inferred = infer_type(arena, context, proof, conversion)?;
        let head = weak_head_normalize(arena, context.signature(), inferred, conversion)?;
        if let Term::Id {
            ty: carrier,
            left: from,
            right: to,
        } = arena.get(head)
        {
            let equality = Equality {
                carrier,
                from,
                to,
                proof,
            };
            rules.extend([equality, equality.reversed(arena)]);
        }
    }
    if rules.is_empty() {
        return Err(SearchError::NoProof);
    }
    let mut reached = vec![(left, reflexivity)];
    let mut next = 0;
    while let Some(&(current, base)) = reached.get(next) {
        next += 1;
        let mut pending = vec![(current, Vec::new())];
        while let Some((focus, path)) = pending.pop() {
            for &rule in &rules {
                charge(&mut search_steps)?;
                if !arena.structurally_equal(focus, rule.from) {
                    continue;
                }
                let rewritten = plug(arena, &path, rule.to, 0);
                let mut seen = false;
                for &(known, _) in &reached {
                    charge(&mut search_steps)?;
                    if arena.structurally_equal(known, rewritten) {
                        seen = true;
                        break;
                    }
                }
                if seen {
                    continue;
                }
                let destination = arena.insert(Term::Variable(1));
                let under_binders = plug(arena, &path, destination, 2);
                let carrier = shift(arena, ty, 0, 2);
                let start = shift(arena, left, 0, 2);
                let conclusion = arena.insert(Term::Id {
                    ty: carrier,
                    left: start,
                    right: under_binders,
                });
                let proof = rule.eliminate(arena, conclusion, base);
                if arena.structurally_equal(rewritten, right) {
                    check_type(arena, context, proof, goal, conversion)?;
                    return Ok(proof);
                }
                reached.push((rewritten, proof));
            }
            if let Term::Apply { function, argument } = arena.get(focus) {
                let mut function_path = path.clone();
                function_path.push(Surrounding::FunctionOf(argument));
                let mut argument_path = path;
                argument_path.push(Surrounding::ArgumentOf(function));
                pending.extend([(function, function_path), (argument, argument_path)]);
            }
        }
    }
    Err(SearchError::NoProof)
}

use super::{call_contracts, graph::Graph, hypotheses};
use proof_admission::{
    Declaration, Level, MathematicalCertificate, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, Sort, check_signature,
};
use semantic_vocabulary::{
    ClauseCarrier, ClauseDeclaration, ClauseIdentity, ClauseOperand, MachineId, ObligationId,
    PlaceId, Proposition, ScalarTerm, TargetMachineCallable,
};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, RefusalReason, UnprovedCause,
};
use terminal_check_tables::{edges::Callee, values::Justification};
use terminal_psi::{TerminalMachine, TerminalModule};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgment {
    pub goal: ClauseIdentity,
    pub citations: Vec<KnowledgeId>,
    pub decision: MathematicalCoreDecision,
    pub certificate: Vec<u8>,
}

#[derive(PartialEq, Eq)]
enum Entry {
    Carrier(ClauseCarrier),
    Input(PlaceId),
    Scalar(ScalarTerm),
    Operation(ClauseDeclaration),
    Law(MachineId, u32),
}

#[derive(Clone, Copy, PartialEq)]
enum LawInput {
    Structural(PlaceId),
    Scalar(semantic_vocabulary::ValueId),
}

#[derive(Default)]
struct Declarations {
    arena: TermArena,
    entries: Vec<(Entry, TermHandle)>,
    signature: Vec<Declaration>,
}

impl Declarations {
    fn entry(&mut self, key: Entry, ty: TermHandle) -> Result<TermHandle, &'static str> {
        if let Some(index) = self.entries.iter().position(|(known, _)| *known == key) {
            if !self.arena.structurally_equal(self.signature[index].ty, ty) {
                return Err("the same signature entry has two different types");
            }
            return Ok(self.entries[index].1);
        }
        let declaration =
            u32::try_from(self.signature.len()).map_err(|_| "signature exceeds its index width")?;
        let term = self.arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        });
        self.signature.push(Declaration::assumption(0, ty));
        self.entries.push((key, term));
        Ok(term)
    }

    fn carrier(&mut self, carrier: ClauseCarrier) -> Result<TermHandle, &'static str> {
        let universe = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        self.entry(Entry::Carrier(carrier), universe)
    }

    fn application(
        &mut self,
        declaration: ClauseDeclaration,
        result: TermHandle,
        domains: &[ClauseCarrier],
        arguments: &[TermHandle],
    ) -> Result<TermHandle, &'static str> {
        let mut function_type = result;
        for &carrier in domains.iter().rev() {
            let domain = self.carrier(carrier)?;
            function_type = self.arena.insert(Term::Pi {
                domain,
                codomain: function_type,
            });
        }
        let operation = self.entry(Entry::Operation(declaration), function_type)?;
        Ok(self.arena.apply_arguments(operation, arguments))
    }

    fn scalar(
        &mut self,
        term: &ScalarTerm,
        binders: &[LawInput],
    ) -> Result<TermHandle, &'static str> {
        let ty = self.carrier(ClauseCarrier::Scalar(term.scalar_type()))?;
        match term {
            ScalarTerm::DeclarationApplication {
                declaration,
                arguments,
                ..
            } => {
                let mut terms = Vec::with_capacity(arguments.len());
                let mut domains = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    domains.push(ClauseCarrier::Scalar(argument.scalar_type()));
                    terms.push(self.scalar(argument, binders)?);
                }
                self.application(*declaration, ty, &domains, &terms)
            }
            ScalarTerm::Value { id, .. } if !binders.is_empty() => {
                let position = binders
                    .iter()
                    .position(|binder| *binder == LawInput::Scalar(*id))
                    .ok_or("law scalar names a foreign input")?;
                Ok(self.arena.insert(Term::Variable(
                    u32::try_from(binders.len() - position - 1)
                        .map_err(|_| "too many law formals")?,
                )))
            }
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => self.entry(Entry::Scalar(term.clone()), ty),
            _ if binders.is_empty() => self.entry(Entry::Scalar(term.clone()), ty),
            _ => Err("no whole-value mathematical input for this scalar operand"),
        }
    }

    /// A closed law quantifies over its formals. An instantiated clause
    /// names the invocation inputs to which the storage analysis traced it.
    fn identity(
        &mut self,
        clause: &ClauseIdentity,
        binders: &[LawInput],
    ) -> Result<TermHandle, &'static str> {
        clause
            .validate()
            .map_err(|_| "malformed declaration Identity")?;
        let mut terms = Vec::<TermHandle>::with_capacity(clause.values.len());
        for value in &clause.values {
            let ty = self.carrier(value.carrier)?;
            let term = match &value.operand {
                ClauseOperand::Input(input) if input.path().is_empty() => {
                    if let Some(position) = binders
                        .iter()
                        .position(|known| *known == LawInput::Structural(input.root()))
                    {
                        self.arena.insert(Term::Variable(
                            u32::try_from(binders.len() - position - 1)
                                .map_err(|_| "too many law formals")?,
                        ))
                    } else if binders.is_empty() {
                        self.entry(Entry::Input(input.root()), ty)?
                    } else {
                        return Err("law names storage outside its telescope");
                    }
                }
                ClauseOperand::Scalar(term) => self.scalar(term, binders)?,
                ClauseOperand::Application { declaration, arguments } => {
                    let domains = arguments.iter()
                        .map(|&arg| clause.values[arg as usize].carrier)
                        .collect::<Vec<_>>();
                    let operands = arguments.iter()
                        .map(|&arg| terms[arg as usize])
                        .collect::<Vec<_>>();
                    self.application(*declaration, ty, &domains, &operands)?
                }
                _ => return Err("no whole-value mathematical input for this clause operand"),
            };
            terms.push(term);
        }
        let ty = self.carrier(clause.values[clause.left as usize].carrier)?;
        Ok(self.arena.insert(Term::Id {
            ty,
            left: terms[clause.left as usize],
            right: terms[clause.right as usize],
        }))
    }

    fn cite(
        &mut self,
        machine: MachineId,
        ordinal: u32,
        original: &ClauseIdentity,
        actual: &ClauseIdentity,
        contract: &call_contracts::PublicContract<'_>,
    ) -> Result<TermHandle, &'static str> {
        if original.left != actual.left
            || original.right != actual.right
            || original.values.len() != actual.values.len()
        {
            return Err("citation changed the law's endpoints or term table");
        }
        let call_contracts::ScalarParameters::Values(scalars) = contract.scalar_parameters else {
            return Err("law has no declaration identities for its scalar inputs");
        };
        let binders: Vec<_> = contract
            .structural_parameters
            .iter()
            .map(|formal| LawInput::Structural(formal.place))
            .chain(scalars.iter().map(|formal| LawInput::Scalar(formal.id)))
            .collect();
        let mut actuals = vec![None; binders.len()];
        for (formal, supplied) in original.values.iter().zip(&actual.values) {
            if formal.carrier != supplied.carrier {
                return Err("citation changed a carrier");
            }
            let (binder, operand) = match (&formal.operand, &supplied.operand) {
                (ClauseOperand::Input(input), ClauseOperand::Input(actual))
                    if input.path().is_empty() && actual.path().is_empty() =>
                {
                    (
                        LawInput::Structural(input.root()),
                        Entry::Input(actual.root()),
                    )
                }
                (
                    ClauseOperand::Scalar(ScalarTerm::Value { id, .. }),
                    ClauseOperand::Scalar(actual),
                ) => (LawInput::Scalar(*id), Entry::Scalar(actual.clone())),
                (left, right) if left == right => continue,
                _ => return Err("citation changed the selected law operation"),
            };
            let position = binders
                .iter()
                .position(|&known| known == binder)
                .ok_or("law has a foreign formal")?;
            let ty = self.carrier(formal.carrier)?;
            let operand = self.entry(operand, ty)?;
            if actuals[position]
                .replace((ty, operand))
                .is_some_and(|previous| previous != (ty, operand))
            {
                return Err("one formal received different values at a single citation");
            }
        }
        let mut statement = self.identity(original, &binders)?;
        let actuals: Vec<_> = actuals
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("law needs a supplied term for each formal")?;
        for &(domain, _) in actuals.iter().rev() {
            statement = self.arena.insert(Term::Pi {
                domain,
                codomain: statement,
            });
        }
        let law = self.entry(Entry::Law(machine, ordinal), statement)?;
        Ok(self.arena.apply_arguments(
            law,
            &actuals.iter().map(|&(_, term)| term).collect::<Vec<_>>(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn discharge(
    module: &TerminalModule,
    machine: &TerminalMachine,
    tables: &MachineTables,
    graph: &Graph,
    point: ProgramPoint,
    obligation: ObligationId,
    goal: &ClauseIdentity,
    law_instances: &[MachineId],
) -> Checked<Judgment> {
    let known = hypotheses(graph, tables, machine.id, obligation, point)?;
    let unavailable = |why: &str| {
        super::refusal(
            RefusalReason::Unsupported,
            machine.id,
            Construct::Obligation(obligation),
            why,
        )
    };
    let mut declarations = Declarations::default();
    let expected = declarations.identity(goal, &[]).map_err(unavailable)?;
    let mut proofs = Vec::new();
    let mut citations = Vec::new();
    let mut premises = Vec::new();
    for (&row, premise) in known.assumption_rows.iter().zip(&known.assumptions) {
        let Proposition::DeclarationIdentity(actual) = premise else {
            continue;
        };
        let justification = tables.knowledge.get(row).map(|row| &row.justification);
        if let Some(Justification::EntryRequirement { clause }) = justification {
            // Only the caller owes this hypothesis. Rejoin the row to this
            // machine's published input clause before binding its proof.
            if machine.requires.get(*clause as usize) != Some(premise) {
                return Err(unavailable(
                    "entry equality differs from its declared requirement",
                ));
            }
            premises.push(declarations.identity(actual, &[]).map_err(unavailable)?);
            citations.push(row);
            continue;
        }
        let Some(Justification::CalleeGuarantee { invocation, clause }) = justification else {
            continue;
        };
        let call = tables
            .invocations
            .get(*invocation)
            .ok_or_else(|| unavailable("citation lost its invocation"))?;
        let Callee::Declared(TargetMachineCallable::Machine(callee)) = call.callee else {
            continue;
        };
        if law_instances.binary_search(&callee).is_err() {
            continue;
        }
        let call = call_contracts::invocation(module, machine, tables, *invocation, call)?;
        if !call.contract.requires.is_empty() {
            return Err(unavailable("conditional law needs its proved premises"));
        }
        let Some(Proposition::DeclarationIdentity(original)) =
            call.contract.ensures.iter().nth(*clause as usize)
        else {
            return Err(unavailable("citation lost its declared equality"));
        };
        proofs.push(
            declarations
                .cite(callee, *clause, original, actual, &call.contract)
                .map_err(unavailable)?,
        );
        citations.push(row);
    }
    let mut budget = Budget::default();
    let kernel_error = |error| match error {
        SearchError::Budget => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::BudgetExhausted,
            "mathematical equality proof budget exhausted".into(),
        ),
        SearchError::NoProof => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::NoDerivation,
            "no available equality proves the stated Identity".into(),
        ),
        SearchError::Kernel(error) => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("mathematical kernel: {error:?}"),
        ),
    };
    let signature = check_signature(
        &mut declarations.arena,
        &declarations.signature,
        &mut budget,
    )
    .map_err(|error| kernel_error(error.into()))?;
    let mut context = Context::empty().with_signature(signature);
    for index in 0..premises.len() {
        let index =
            u32::try_from(index).map_err(|_| unavailable("too many equality hypotheses"))?;
        proofs.push(declarations.arena.insert(Term::Variable(index)));
    }
    for &premise in &premises {
        context = context.extend(premise);
    }
    let proof = prove(
        &mut declarations.arena,
        &context,
        expected,
        &proofs,
        65_536,
        &mut budget,
    )
    .map_err(kernel_error)?;
    let certificate =
        MathematicalCertificate::with_signature(declarations.signature, premises, proof, expected);
    let receipt =
        MathematicalJudgmentReceipt::verify(&mut declarations.arena, &certificate, &mut budget)
            .map_err(|error| kernel_error(error.into()))?;
    let bytes = terminal_codec::encode_mathematical_certificate(&declarations.arena, &certificate)
        .map_err(|_| unavailable("could not encode the judged mathematical term"))?;
    Ok(Judgment {
        goal: goal.clone(),
        citations,
        decision: MathematicalCoreDecision::Judged(receipt),
        certificate: bytes,
    })
}
