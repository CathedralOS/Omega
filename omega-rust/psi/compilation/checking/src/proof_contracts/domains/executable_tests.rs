//! Predicate expansion preserves the tested carrier as a formation obligation.
//! Diagnose a wrong domain subject before generated member copies can appear
//! to select conflicting declarations under that ill-formed substitution.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

pub fn validate_executable_domain_test_carriers(
    program: &dyn crate::CheckingInput,
    contracts: &arena::Arena<checked_trees::ContractProofFact>,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
) -> Result<(), Diagnostic> {
    for (expression, _) in program.expression_table.iter_expressions() {
        for test in program.expression_table.executable_domain_tests(expression) {
            let Some(domain) = program.domain_definition(test.domain) else {
                continue;
            };
            let Some(subject) =
                crate::expression_owners::checked_expression_type_reference_from_exact_owner(
                    program,
                    contracts,
                    expression,
                    test.subject,
                    Some(owners),
                )
                .and_then(|reference| crate::unwrapped_type_reference(program, reference))
            else {
                // Open applications are judged after their binders are closed.
                // Missing type information grants no carrier correspondence.
                continue;
            };
            let Some(target) = crate::unwrapped_type_reference(program, domain.target_type) else {
                continue;
            };
            if crate::declared_carriers_match(program, subject, target) {
                continue;
            }
            let substitutions = declared_type_equations(program, owners, expression);
            let normalized_subject = executable_domain_carrier(program, subject, &substitutions);
            let normalized_target = executable_domain_carrier(program, target, &substitutions);
            if normalized_subject
                .zip(normalized_target)
                .is_some_and(|(subject, target)| {
                    crate::value_custody::places::declared_carriers_match_with_substitutions(
                        program,
                        subject,
                        target,
                        &substitutions,
                    )
                })
            {
                continue;
            }
            let subject = normalized_subject.unwrap_or(subject);
            let target = normalized_target.unwrap_or(target);
            return Err(Diagnostic::error(format!(
                "executable domain test `{}` expects subject carrier `{}`, got `{}`",
                domain.name.as_str(),
                program.display_type_reference_with_constraints(target),
                program.display_type_reference_with_constraints(subject),
            ))
            .with_source_span(program.expression_table.source_span(expression)));
        }
    }
    Ok(())
}

/// Apply the same shell query to an open declaration as to its closed type.
/// An equation can reveal another reference or constrained shell, so the
/// substitution and shell queries must compose until neither exposes more.
/// This is only the domain's carrier judgment; ordinary predicate reads still
/// prove access to the original subject and never gain authority from a peel.
fn executable_domain_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return None;
        }
        visited.push(reference);
        let substituted = crate::value_custody::places::substituted_declared_carrier(
            program,
            reference,
            substitutions,
        )?;
        let unwrapped = crate::unwrapped_type_reference(program, substituted)?;
        if unwrapped == substituted {
            return Some(unwrapped);
        }
        reference = unwrapped;
    }
}

fn declared_type_equations(
    program: &dyn crate::CheckingInput,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    expression: ExpressionHandle,
) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
    let Some(owner) =
        crate::expression_owners::source_declaration_owner(program, expression, owners)
    else {
        return Vec::new();
    };
    crate::value_custody::places::declaration_type_equations(program, owner)
}
