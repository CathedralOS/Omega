//! Retain the actual absence of ordinary source `via` producers, or refuse
//! their execution until the producing module and its accepted invocation
//! custody exist. A declaration roster is not an admitted executable closure.

use diagnostics::Diagnostic;
use provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable;
use target::TargetProfile;
use typed_trees::TypedTrees;

pub fn prepare_source_via_bindings(
    typed: &TypedTrees,
    scope: source::DependencyScope,
    selected_target: Option<TargetProfile>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    let machines = typed.tables.machines.span(typed.roots.machines).ok_or_else(|| vec![Diagnostic::error(
        "no product route yet: ordinary source via inventory has an unresolved machine roster",
    )])?;
    let mut diagnostics = Vec::new();
    for machine in machines.iter().filter(|machine| {
        typed
            .symbols
            .symbol_is_in_dependency_scope(machine.symbol, scope)
    }) {
        let conformances = typed.machine_trait_conformances.span(machine.satisfies).ok_or_else(|| vec![Diagnostic::error(
            "no product route yet: ordinary source via inventory has an unresolved conformance roster",
        )])?;
        for conformance in conformances {
            if conformance.via_expression.is_valid() {
                diagnostics.push(Diagnostic::error(
                    "no product route yet: ordinary source via producer has no accepted executable and invocation custody",
                ).with_source_span(typed.expression_table.source_span(conformance.via_expression)));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Every actual conformance in this scope was inspected. This is the
    // genuine no-producer case, not an unavailable evaluation called empty.
    Ok(EvaluatedViaBindingTable::without_bindings(
        scope,
        selected_target,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::{Machine, TraitConformance};

    fn source(with_via: bool) -> TypedTrees {
        let mut symbols = SymbolTableBuilder::new();
        let symbol = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("producer"));
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let via_expression = if with_via {
            typed.expression_table.insert(ExpressionNode::Boolean(true))
        } else {
            Default::default()
        };
        let satisfies = typed
            .machine_trait_conformances
            .insert_many([TraitConformance {
                via_expression,
                ..Default::default()
            }]);
        typed.roots.machines = typed.tables.machines.insert_many([Machine {
            symbol,
            satisfies,
            ..Default::default()
        }]);
        typed
    }

    #[test]
    fn only_an_actual_empty_via_roster_yields_an_empty_table() {
        let none =
            prepare_source_via_bindings(&source(false), source::DependencyScope::Product, None)
                .unwrap();
        assert!(none.rows().is_empty());
        let authored =
            prepare_source_via_bindings(&source(true), source::DependencyScope::Product, None)
                .unwrap_err();
        assert!(
            authored
                .iter()
                .any(|diagnostic| diagnostic.message.contains("no accepted executable"))
        );
    }

    #[test]
    fn missing_roster_storage_cannot_be_called_no_via() {
        let mut missing_conformances = source(true);
        missing_conformances.machine_trait_conformances = Default::default();
        assert!(
            prepare_source_via_bindings(
                &missing_conformances,
                source::DependencyScope::Product,
                None
            )
            .is_err()
        );
        let mut missing_machines = source(true);
        missing_machines.tables.machines = Default::default();
        assert!(
            prepare_source_via_bindings(&missing_machines, source::DependencyScope::Product, None)
                .is_err()
        );
    }
}
