//! Partition the provider telescope by the exact contract being matched.
//!
//! `provider<T, U> satisfies Trait<Container<T>>::member` fixes T in that
//! trait application. U remains a fresh invocation binder and must match the
//! requirement's universal callable family. Only this exact edge captures
//! parameters: another edge mentioning U cannot erase that obligation here.
//! Inline named implementations additionally capture their own telescope.

use symbols::SymbolHandle;
use typed_trees::data::TypeParameter;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::references::{TypedReferences, visit_type_reference};
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn invocation_parameters(
    program: &dyn crate::CheckingInput,
    realization: &Machine,
    family: &Machine,
    parameters: &[TypeParameter],
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
) -> Vec<TypeParameter> {
    let mut captured = ApplicationReferences::default();
    for edge in program
        .machine_trait_conformances(family)
        .iter()
        .filter(|edge| edge.symbol == trait_symbol && edge.requirement_symbol == requirement_symbol)
    {
        for argument in program
            .type_reference_table
            .type_reference_handles(edge.arguments)
        {
            visit_type_reference(program, &mut captured, *argument);
        }
    }
    captured.symbols.extend(
        program
            .conformances()
            .iter()
            .filter(|conformance| {
                conformance.rows.iter().any(|row| {
                    (row.realization_machine == realization.symbol
                        || row.realization_machine == family.symbol)
                        && row.source.owns_realization()
                })
            })
            .flat_map(|conformance| program.conformance_type_parameters(conformance))
            .map(|parameter| parameter.symbol),
    );
    parameters
        .iter()
        .filter(|parameter| !captured.symbols.contains(&parameter.symbol))
        .cloned()
        .collect()
}

#[derive(Default)]
struct ApplicationReferences {
    symbols: Vec<SymbolHandle>,
    types: Vec<TypeReferenceHandle>,
    expressions: Vec<ExpressionHandle>,
}

impl Extend<SymbolHandle> for ApplicationReferences {
    fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, references: References) {
        self.symbols.extend(references);
    }
}

impl TypedReferences for ApplicationReferences {
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
