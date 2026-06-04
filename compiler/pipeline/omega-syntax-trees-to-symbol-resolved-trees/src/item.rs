use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::invariant::lower_invariant_definition;
use crate::lowerer::Lowerer;
use crate::machine::lower_machine_into;
use crate::measure::lower_measure_definition;
use crate::operator::lower_operator_definition;
use crate::platform::lower_platform;
use crate::trait_definition::lower_trait_definition;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_item(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    item: &syntax::item::Item,
) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::Data(data_definition) => {
            let data_definition = lower_data_definition(lowerer, syntax_trees, data_definition)?;
            lowerer
                .symbol_resolved_trees
                .data_definitions
                .push(data_definition);
        }
        syntax::item::Item::Invariant(invariant_definition) => {
            let invariant_definition =
                lower_invariant_definition(lowerer, syntax_trees, invariant_definition)?;
            lowerer
                .symbol_resolved_trees
                .invariant_definitions
                .push(invariant_definition);
        }
        syntax::item::Item::Domain(domain_definition) => {
            let domain_definition =
                lower_domain_definition(lowerer, syntax_trees, domain_definition)?;
            lowerer
                .symbol_resolved_trees
                .domain_definitions
                .push(domain_definition);
        }
        syntax::item::Item::Machine(machine) => {
            lower_machine_into(lowerer, syntax_trees, machine)?;
        }
        syntax::item::Item::Platform(platform) => {
            let platform = lower_platform(lowerer, syntax_trees, platform)?;
            lowerer.symbol_resolved_trees.platforms.push(platform);
        }
        syntax::item::Item::Trait(trait_definition) => {
            let trait_definition = lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
            lowerer.symbol_resolved_trees.traits.push(trait_definition);
        }
        syntax::item::Item::Measure(measure) => {
            let measure = lower_measure_definition(lowerer, syntax_trees, measure)?;
            lowerer.symbol_resolved_trees.measures.push(measure);
        }
        syntax::item::Item::Operator(operator) => {
            let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
            lowerer.symbol_resolved_trees.operators.push(operator);
        }
        syntax::item::Item::Capability(_)
        | syntax::item::Item::Module(_)
        | syntax::item::Item::Package(_)
        | syntax::item::Item::Provider(_)
        | syntax::item::Item::HostProvider(_)
        | syntax::item::Item::Export(_)
        | syntax::item::Item::Library(_)
        | syntax::item::Item::Target(_)
        | syntax::item::Item::Use(_) => {}
    }

    Ok(())
}
