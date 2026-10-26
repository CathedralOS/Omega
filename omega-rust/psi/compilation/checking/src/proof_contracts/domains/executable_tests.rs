//! Predicate expansion preserves the tested carrier as a formation obligation.
//! Diagnose a wrong domain subject before generated member copies can appear
//! to select conflicting declarations under that ill-formed substitution.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::{TypedReferences, visit_type_reference};
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
            if (carrier_has_open_binders(program, subject)
                || carrier_has_open_binders(program, target))
                && awaits_declared_equations(program, owners, expression)
            {
                // Source equation preparation retains this nonexecutable
                // template until a closed explicit application discharges its
                // declared equations. This is a pending obligation, not an
                // equation inferred from the current consumers. Unconstrained
                // templates must form their tests independently of callers.
                continue;
            }
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

fn awaits_declared_equations(
    program: &dyn crate::CheckingInput,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    expression: ExpressionHandle,
) -> bool {
    let sites = owners.containing_statement_sites(expression);
    !sites.is_empty()
        && sites
            .into_iter()
            .all(|(machine, _, _)| program.machines()[machine].structural_type_equations_pending)
}

#[derive(Default)]
struct CarrierReferences {
    symbols: Vec<SymbolHandle>,
    types: Vec<TypeReferenceHandle>,
    expressions: Vec<ExpressionHandle>,
}

impl Extend<SymbolHandle> for CarrierReferences {
    fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: References) {
        self.symbols.extend(symbols);
    }
}

impl TypedReferences for CarrierReferences {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
}

fn carrier_has_open_binders(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    let mut references = CarrierReferences::default();
    visit_type_reference(program.typed_program(), &mut references, reference);
    // Elaboration can replace a generic root with its exact data instance.
    // Its retained application, rather than its rendered name, owns binders.
    let mut ordinal = 0;
    while ordinal < references.symbols.len() {
        let symbol = references.symbols[ordinal];
        ordinal += 1;
        if symbol.is_valid() && program.symbols.get(symbol).kind == SymbolKind::TypeParameter {
            return true;
        }
        if let Some(application) = program
            .data_definition_by_symbol(symbol)
            .and_then(|definition| definition.generic_instance)
        {
            visit_type_reference(program.typed_program(), &mut references, application);
        }
    }
    false
}
