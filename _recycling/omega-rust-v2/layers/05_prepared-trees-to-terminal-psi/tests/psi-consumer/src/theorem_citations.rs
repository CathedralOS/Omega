use omega_psi::{
    CallRequirement, CallSite, ContractClause, LogicalTheorem, LoweredMachine, LoweredPackage, MachineBody, OperationKind, OperationResult, PredicateProgress,
    ProofInput, ProofStep, ProofSubject, ScalarLiteral, ScalarType, TheoremArgument, TheoremBinder, TypeExpression, TypeExpressionStep, TypeShape, ValueId,
};

/// What a checker must do at one erased theorem citation, read from Psi alone.
#[derive(Debug)]
pub struct CitationObligations<'a> {
    pub theorem: &'a LogicalTheorem,
    /// The caller's values or closed literals substituted for the theorem's formals, in order.
    pub subjects: &'a [ProofSubject<ValueId>],
    /// The theorem `requires` clauses owed at this citation, in clause order.
    pub owed: Vec<&'a ContractClause>,
    /// For each hypothesis binder, the cited machine whose contract must refine it.
    pub evidence: Vec<(usize, &'a LoweredMachine)>,
    /// The theorem `ensures` clauses that hold after the citation.
    pub imported: &'a [ContractClause],
}

/// Every citation in `body`, in operation order. Returns `None` if any
/// citation is malformed: an unknown theorem, a binder or subject count that
/// disagrees with it, a subject value outside the body, a literal outside its
/// formal's carrier, evidence that names no lowered machine, a requirement of
/// another site, or a skipped or reordered `requires` clause.
pub fn theorem_citations<'a>(
    package: &'a LoweredPackage,
    body: &'a MachineBody,
) -> Option<Vec<CitationObligations<'a>>> {
    let mut citations = Vec::new();
    for op in &body.operations {
        let OperationKind::ApplyTheorem(application) = &op.kind else { continue };
        if op.result != OperationResult::Unit {
            return None;
        }
        let theorem = package
            .theorems
            .iter()
            .find(|t| t.declaration == application.theorem)?;
        if application.subjects.len() != theorem.parameters.len() || application.arguments.len() != theorem.binders.len() {
            return None;
        }
        for (subject, formal) in application.subjects.iter().zip(&theorem.parameters) {
            let fits = scalar(formal).is_some_and(|carrier| subject_fits(subject, carrier, |value| {
                match package.types.get(body.values.get(value.index())?.ty)? {
                    TypeShape::PrimitiveScalar(ty) => Some(*ty),
                    _ => None,
                }
            }));
            if !fits {
                return None;
            }
        }
        let mut owed = Vec::with_capacity(application.requirements.len());
        for (expected, &CallRequirement { site, clause }) in application.requirements.iter().enumerate() {
            if site != CallSite::Operation(op.id) || clause != expected {
                return None;
            }
            owed.push(theorem.requires.get(clause)?);
        }
        if owed.len() != theorem.requires.len() {
            return None;
        }
        let mut evidence = Vec::new();
        for (binder, argument) in theorem.binders.iter().zip(&application.arguments) {
            evidence.push(root_evidence(package, theorem, binder, argument)?);
        }
        citations.push(CitationObligations {
            theorem,
            subjects: &application.subjects,
            owed,
            evidence,
            imported: &theorem.ensures,
        });
    }
    Some(citations)
}

/// An application supplies only root hypotheses; nested premises bind within
/// the contract of their parent.
fn root_evidence<'a>(
    package: &'a LoweredPackage,
    theorem: &LogicalTheorem,
    binder: &TheoremBinder,
    argument: &TheoremArgument,
) -> Option<(usize, &'a LoweredMachine)> {
    let (TheoremBinder::Hypothesis(index), TheoremArgument::Citation(declaration)) = (binder, argument) else { return None };
    if theorem.hypotheses.get(*index)?.parent.is_some() {
        return None;
    }
    Some((*index, package.machines.iter().find(|m| &m.declaration == declaration)?))
}

/// What a checker must do at one machine citation inside a theorem proof.
#[derive(Debug)]
pub struct ProofCitation<'a> {
    pub step: usize,
    pub machine: &'a LoweredMachine,
    /// The cited machine's formals, each resolved through proof bindings to a
    /// theorem formal (by position) or a closed literal.
    pub subjects: Vec<ProofSubject<usize>>,
    /// The cited machine's `requires` clauses owed at those subjects.
    pub owed: &'a [ContractClause],
    /// The cited machine's `ensures` clauses that hold afterward.
    pub imported: &'a [ContractClause],
    pub progress: PredicateProgress,
}

/// Each machine citation in `theorem.proof`, in step order, with every
/// subject resolved to the theorem's own formals or literals. A binding
/// contributes no obligation. Returns `None` if a step is malformed: a local
/// that names no earlier binding, a formal outside the theorem, a cited
/// declaration with no lowered machine, a subject count or literal that
/// disagrees with the machine's scalar formals, or requirements other than
/// all of the machine's `requires` clauses in order.
pub fn theorem_proof<'a>(package: &'a LoweredPackage, theorem: &'a LogicalTheorem) -> Option<Vec<ProofCitation<'a>>> {
    proof_citations(package, theorem).map(|(machines, _)| machines)
}

pub struct NestedCitation<'a> {
    pub theorem: &'a LogicalTheorem,
    pub subjects: Vec<ProofSubject<usize>>,
    pub evidence: Vec<(usize, &'a LoweredMachine)>,
    pub owed: &'a [ContractClause],
    pub imported: &'a [ContractClause],
}

pub fn nested_theorem_citations<'a>(package: &'a LoweredPackage, theorem: &'a LogicalTheorem) -> Option<Vec<NestedCitation<'a>>> {
    proof_citations(package, theorem).map(|(_, theorems)| theorems)
}

fn proof_citations<'a>(
    package: &'a LoweredPackage,
    theorem: &'a LogicalTheorem,
) -> Option<(Vec<ProofCitation<'a>>, Vec<NestedCitation<'a>>)> {
    let mut bound: Vec<Option<ProofSubject<usize>>> = Vec::with_capacity(theorem.proof.len());
    let mut citations = Vec::new();
    let mut nested = Vec::new();
    let resolve = |subject: &ProofSubject<ProofInput>, bound: &[Option<ProofSubject<usize>>]| -> Option<ProofSubject<usize>> {
        let lookup = |input: ProofInput| match input {
            ProofInput::Formal(index) => (index < theorem.parameters.len()).then_some(ProofSubject::Input(index)),
            ProofInput::Local(step) => bound.get(step)?.clone(),
        };
        use omega_psi::ProofIntegerStep as Step;
        match subject {
            ProofSubject::Input(input) => lookup(*input),
            ProofSubject::Literal(literal) => Some(ProofSubject::Literal(literal.clone())),
            ProofSubject::IntegerTerm(steps) => {
                let mut result = Vec::new();
                for step in steps {
                    match step {
                        Step::Input(input) => match lookup(*input)? {
                            ProofSubject::Input(formal) => result.push(Step::Input(formal)),
                            ProofSubject::Literal(value) => result.push(Step::Literal(value)),
                            ProofSubject::IntegerTerm(inner) => result.extend(inner),
                        },
                        Step::Literal(value) => result.push(Step::Literal(value.clone())),
                        Step::Operation { operation, carrier, site } => result.push(Step::Operation {
                            operation: operation.clone(), carrier: *carrier, site: site.clone()
                        }),
                    }
                }
                Some(ProofSubject::IntegerTerm(result))
            }
        }
    };
    for (step, row) in theorem.proof.iter().enumerate() {
        match row {
            ProofStep::Bind { subject, .. } => {
                let resolved = resolve(subject, &bound)?;
                bound.push(Some(resolved));
            }
            ProofStep::ApplyTheorem { theorem: callee, arguments, subjects, requirements, .. } => {
                let callee = package.theorems.iter().find(|row| &row.declaration == callee)?;
                if *requirements != (0..callee.requires.len()) || subjects.len() != callee.parameters.len() || arguments.len() != callee.binders.len() {
                    return None;
                }
                let mut resolved = Vec::new();
                for (subject, ty) in subjects.iter().zip(&callee.parameters) {
                    let subject = resolve(subject, &bound)?;
                    if !subject_fits(&subject, scalar(ty)?, |index| scalar(theorem.parameters.get(index)?)) { return None; }
                    resolved.push(subject);
                }
                let mut evidence = Vec::new();
                for (binder, argument) in callee.binders.iter().zip(arguments) {
                    evidence.push(root_evidence(package, callee, binder, argument)?);
                }
                nested.push(NestedCitation { theorem: callee, subjects: resolved, evidence, owed: &callee.requires, imported: &callee.ensures });
                bound.push(None);
            }
            ProofStep::CiteMachine {
                machine,
                subjects,
                requirements,
                progress,
                ..
            } => {
                bound.push(None);
                let machine = package.machines.iter().find(|m| &m.declaration == machine)?;
                if subjects.len() != machine.parameter_types.len() || *requirements != (0..machine.contract.requires.len()) {
                    return None;
                }
                let mut resolved = Vec::with_capacity(subjects.len());
                for (subject, &ty) in subjects.iter().zip(&machine.parameter_types) {
                    let carrier = match package.types.get(ty)? {
                        TypeShape::PrimitiveScalar(carrier) => *carrier,
                        _ => return None,
                    };
                    let subject = resolve(subject, &bound)?;
                    let fits = subject_fits(&subject, carrier, |formal| scalar(theorem.parameters.get(formal)?));
                    if !fits {
                        return None;
                    }
                    resolved.push(subject);
                }
                citations.push(ProofCitation {
                    step,
                    machine,
                    subjects: resolved,
                    owed: &machine.contract.requires,
                    imported: &machine.contract.ensures,
                    progress: *progress,
                });
            }
        }
    }
    Some((citations, nested))
}

fn subject_fits<I: Copy>(subject: &ProofSubject<I>, carrier: ScalarType, input: impl Fn(I) -> Option<ScalarType>) -> bool {
    use omega_psi::ProofIntegerStep as Step;
    match subject {
        ProofSubject::Input(value) => input(*value) == Some(carrier),
        ProofSubject::Literal(value) => fits(value, carrier),
        ProofSubject::IntegerTerm(steps) => {
            let mut depth = 0;
            for step in steps {
                match step {
                    Step::Input(value) if input(*value) == Some(carrier) => depth += 1,
                    Step::Literal(value) if fits(value, carrier) => depth += 1,
                    Step::Operation { carrier: result, .. } if matches!(carrier, ScalarType::Integer { .. }) && *result == carrier && depth >= 2 => depth -= 1,
                    _ => return false,
                }
            }
            depth == 1 && matches!(steps.last(), Some(Step::Operation { .. }))
        }
    }
}

/// Discharge the exact-add shape of the unsigned byte example on one input.
/// A retained representability obligation rejects 255 + 1. A wrapping add in
/// the same carrier yields zero there.
pub fn byte_subject_addition<I: Eq>(subject: &ProofSubject<I>, expected: I, value: u8) -> Option<u8> {
    use omega_psi::{ArithmeticPolicy, IntegerObligation, IntegerPrimitive, ProofIntegerStep as Step};
    let ProofSubject::IntegerTerm(steps) = subject else { return None };
    let [Step::Input(input), Step::Literal(ScalarLiteral::Integer(addend)), Step::Operation { operation, carrier: ScalarType::Integer { signed: false, width: 8 }, .. }] = &steps[..] else { return None };
    if input != &expected || operation.primitive != IntegerPrimitive::Add { return None; }
    let addend = u8::try_from(addend).ok()?;
    match (operation.policy, operation.obligations.as_slice()) {
        (ArithmeticPolicy::Exact, [IntegerObligation::ResultFitsCarrier]) => value.checked_add(addend),
        (ArithmeticPolicy::Wrapping, []) => Some(value.wrapping_add(addend)),
        _ => None,
    }
}

fn scalar(ty: &TypeExpression) -> Option<ScalarType> {
    match ty.0.as_slice() {
        [TypeExpressionStep::Scalar(carrier)] => Some(*carrier),
        _ => None,
    }
}

fn fits(
    literal: &ScalarLiteral,
    carrier: ScalarType,
) -> bool {
    match (literal, carrier) {
        (ScalarLiteral::Boolean(_), ScalarType::Boolean) => true,
        // Widths above 64 bits are outside what this consumer checks.
        (ScalarLiteral::Integer(value), ScalarType::Integer { signed, width }) if (1..=64).contains(&width) => {
            let Ok(value) = i128::try_from(value) else { return false };
            let (low, high) = if signed {
                (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
            } else {
                (0, (1i128 << width) - 1)
            };
            (low..=high).contains(&value)
        }
        _ => false,
    }
}
