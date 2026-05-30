use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::operator::lower_operator_definition;
use crate::platform::lower_platform;
use crate::trait_definition::lower_trait_definition;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees::TypedTrees;

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
    };

    for invariant_definition in &symbol_resolved_trees.invariant_definitions {
        let invariant_definition = lower_invariant_definition(&mut lowerer, invariant_definition)?;
        lowerer
            .typed_trees
            .push_invariant_definition(invariant_definition);
    }

    for data_definition in &symbol_resolved_trees.data_definitions {
        let data_definition = lower_data_definition(&mut lowerer, data_definition)?;
        lowerer.typed_trees.push_data_definition(data_definition);
    }

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lower_domain_definition(&mut lowerer, domain_definition)?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.typed_trees.push_machine(machine);
    }

    for measure in &symbol_resolved_trees.measures {
        let measure = crate::measure::lower_measure_definition(&mut lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lower_operator_definition(&mut lowerer, operator)?;
        lowerer.typed_trees.push_operator(operator);
    }

    for platform in &symbol_resolved_trees.platforms {
        let platform = lower_platform(&mut lowerer, platform)?;
        lowerer.typed_trees.push_platform(platform);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        let trait_definition = lower_trait_definition(&mut lowerer, trait_definition)?;
        lowerer.typed_trees.push_trait_definition(trait_definition);
    }

    lowerer.finish()
}

pub fn lower_symbol_resolved_trees_owned(
    symbol_resolved_trees: SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut typed_trees = lower_symbol_resolved_trees(&symbol_resolved_trees)?;
    typed_trees.symbols = symbol_resolved_trees.symbols;
    Ok(typed_trees)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
}

impl Lowerer<'_> {
    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        let TypedTrees {
            roots,
            tables,
            symbols,
        } = self.typed_trees;

        Ok(TypedTrees::with_roots(roots, tables, symbols))
    }
}

#[cfg(test)]
mod tests;
