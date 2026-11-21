use omega_psi::{
    CallRequirement, CallSite, ContractClause, LogicalTheorem, LoweredMachine, LoweredPackage, MachineBody, OperationKind, OperationResult, TheoremArgument,
    TheoremBinder, ValueId,
};

/// What a checker must do at one erased theorem citation, read from Psi alone.
#[derive(Debug)]
pub struct CitationObligations<'a> {
    pub theorem: &'a LogicalTheorem,
    /// The caller's values substituted for the theorem's formals, in order.
    pub subjects: &'a [ValueId],
    /// The theorem `requires` clauses owed at this citation, in clause order.
    pub owed: Vec<&'a ContractClause>,
    /// For each hypothesis binder, the cited machine whose contract must refine it.
    pub evidence: Vec<(usize, &'a LoweredMachine)>,
    /// The theorem `ensures` clauses that hold after the citation.
    pub imported: &'a [ContractClause],
}

/// Every citation in `body`, in operation order. Returns `None` if any
/// citation is malformed: an unknown theorem, a binder or subject count that
/// disagrees with it, evidence that names no lowered machine, a requirement of
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
        if application.subjects.len() != theorem.parameters.len()
            || application.arguments.len() != theorem.binders.len()
            || application
                .subjects
                .iter()
                .any(|v| body.values.get(v.index()).is_none())
        {
            return None;
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
