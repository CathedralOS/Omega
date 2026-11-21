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
            let fits = match subject {
                ProofSubject::Input(value) => body.values.get(value.index()).is_some(),
                ProofSubject::Literal(literal) => scalar(formal).is_some_and(|carrier| fits(literal, carrier)),
            };
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
            let (TheoremBinder::Hypothesis(index), TheoremArgument::Citation(declaration)) = (binder, argument) else {
                return None;
            };
            evidence.push((
                *index,
                package
                    .machines
                    .iter()
                    .find(|m| &m.declaration == declaration)?,
            ));
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
pub fn theorem_proof<'a>(
    package: &'a LoweredPackage,
    theorem: &'a LogicalTheorem,
) -> Option<Vec<ProofCitation<'a>>> {
    let mut bound: Vec<Option<ProofSubject<usize>>> = Vec::with_capacity(theorem.proof.len());
    let mut citations = Vec::new();
    let resolve = |subject: &ProofSubject<ProofInput>, bound: &[Option<ProofSubject<usize>>]| -> Option<ProofSubject<usize>> {
        match subject {
            ProofSubject::Input(ProofInput::Formal(index)) => (*index < theorem.parameters.len()).then_some(ProofSubject::Input(*index)),
            ProofSubject::Input(ProofInput::Local(step)) => bound.get(*step)?.clone(),
            ProofSubject::Literal(literal) => Some(ProofSubject::Literal(literal.clone())),
        }
    };
    for (step, row) in theorem.proof.iter().enumerate() {
        match row {
            ProofStep::Bind { subject, .. } => {
                let resolved = resolve(subject, &bound)?;
                bound.push(Some(resolved));
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
                    let fits = match &subject {
                        ProofSubject::Input(formal) => scalar(&theorem.parameters[*formal]) == Some(carrier),
                        ProofSubject::Literal(literal) => fits(literal, carrier),
                    };
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
    Some(citations)
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
