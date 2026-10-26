//! Predicate expansion preserves the tested carrier as a formation obligation.
//! Diagnose a wrong domain subject before generated member copies can appear
//! to select conflicting declarations under that ill-formed substitution.

use diagnostics::Diagnostic;

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
            if !crate::declared_carriers_match(program, subject, target) {
                return Err(Diagnostic::error(format!(
                    "executable domain test `{}` expects subject carrier `{}`, got `{}`",
                    domain.name.as_str(),
                    program.display_type_reference_with_constraints(target),
                    program.display_type_reference_with_constraints(subject),
                ))
                .with_source_span(program.expression_table.source_span(expression)));
            }
        }
    }
    Ok(())
}
