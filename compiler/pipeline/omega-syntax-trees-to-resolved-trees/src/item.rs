use crate::data::lower_data_definition;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::platform::lower_platform;
use crate::program::Lowerer;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees as syntax;

pub(crate) fn lower_item(lowerer: &mut Lowerer, item: &syntax::item::Item) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::Data(data_definition) => {
            let data_definition = lower_data_definition(lowerer, data_definition)?;
            lowerer.program.data_definitions.push(data_definition);
        }
        syntax::item::Item::Invariant(invariant_definition) => {
            let invariant_definition = lower_invariant_definition(lowerer, invariant_definition)?;
            lowerer
                .program
                .invariant_definitions
                .push(invariant_definition);
        }
        syntax::item::Item::Machine(machine) => {
            let machine = lower_machine(lowerer, machine)?;
            lowerer.program.machines.push(machine);
        }
        syntax::item::Item::Platform(platform) => {
            let platform = lower_platform(lowerer, platform)?;
            lowerer.program.platforms.push(platform);
        }
        syntax::item::Item::Capability(_)
        | syntax::item::Item::Library(_)
        | syntax::item::Item::Target(_)
        | syntax::item::Item::TrustDefinition(_)
        | syntax::item::Item::Use(_) => {}
    }

    Ok(())
}
