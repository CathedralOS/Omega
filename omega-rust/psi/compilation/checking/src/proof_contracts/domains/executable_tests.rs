//! Predicate expansion preserves the tested carrier as a formation obligation.
//! Diagnose a wrong domain subject before generated member copies can appear
//! to select conflicting declarations under that ill-formed substitution.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
            if crate::value_custody::places::declared_carriers_match_with_substitutions(
                program,
                subject,
                target,
                &substitutions,
            ) {
                continue;
            }
            let subject = crate::value_custody::places::substituted_declared_carrier(
                program,
                subject,
                &substitutions,
            )
            .unwrap_or(subject);
            let target = crate::value_custody::places::substituted_declared_carrier(
                program,
                target,
                &substitutions,
            )
            .unwrap_or(target);
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

fn declared_type_equations(
    program: &dyn crate::CheckingInput,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    expression: ExpressionHandle,
) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
    let mut substitutions = Vec::new();
    let Some(mut owner) =
        crate::expression_owners::source_declaration_owner(program, expression, owners)
    else {
        return substitutions;
    };
    // Nested machines inherit the same exact static binder scope. No runtime
    // value or caller argument enters the declaration's assumptions.
    let mut visited = Vec::new();
    while owner.is_valid() && !visited.contains(&owner) {
        visited.push(owner);
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == owner)
        {
            for equation in program
                .declared_type_equations
                .span_or_empty(machine.declared_type_equations)
            {
                let TypeReferenceNode::Named { symbol, .. } = program
                    .type_reference_table
                    .type_reference(equation.parameter)
                else {
                    continue;
                };
                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                {
                    substitutions.push((*symbol, equation.definition));
                }
            }
        }
        owner = program.declaration_scope_parent(owner);
    }
    substitutions
}
