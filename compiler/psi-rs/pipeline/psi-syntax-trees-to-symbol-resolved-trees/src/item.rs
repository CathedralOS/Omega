use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::invariant::lower_invariant_definition;
use crate::lowerer::Lowerer;
use crate::machine::lower_machine_into;
use crate::measure::lower_measure_definition;
use crate::operator::lower_operator_definition;
use crate::trait_definition::lower_trait_definition;
use crate::type_reference::lower_child_type_references;
use crate::wire::lower_wire_schema;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_item(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    item: &syntax::item::Item,
) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::Data(data_definition) => {
            let lowered = lower_data_definition(lowerer, syntax_trees, data_definition)?;
            lowerer.symbol_resolved_trees.data_definitions.push(lowered);
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
            // A machine still carrying a target marker here was NOT selected:
            // the pre-resolution filter (pipeline/target_machines.rs) clears
            // the selected target's marker and validates the loud edges, so a
            // marked machine is inert. (Without the filter, EVERY target machine stays inert and
            // its call sites fail resolution loudly -- never a silent success.)
            if machine.target.is_none() {
                lower_machine_into(lowerer, syntax_trees, machine)?;
            }
        }
        syntax::item::Item::Trait(trait_definition) => {
            let trait_definition = lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
            lowerer.symbol_resolved_trees.traits.push(trait_definition);
        }
        syntax::item::Item::Conformance(conformance) => {
            let arguments =
                lower_child_type_references(lowerer, syntax_trees, conformance.trait_arguments)?;
            lowerer.symbol_resolved_trees.conformances.push(
                psi_symbol_resolved_trees::trait_definition::DataConformance {
                    symbol: psi_symbols::SymbolHandle::invalid(),
                    type_name: crate::name::lower_name(&conformance.type_name),
                    trait_name: crate::name::lower_name(&conformance.trait_name),
                    arguments,
                    alias: conformance.alias.as_ref().map(crate::name::lower_name),
                },
            );
        }
        syntax::item::Item::Measure(measure) => {
            let measure = lower_measure_definition(lowerer, syntax_trees, measure)?;
            lowerer.symbol_resolved_trees.measures.push(measure);
        }
        syntax::item::Item::Operator(operator) => {
            let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
            lowerer.symbol_resolved_trees.operators.push(operator);
        }
        // PROP-FAMILY-SURFACE source slice: parsing retains proposition
        // declarations exactly, but they must never disappear as inert items
        // or masquerade as executable machines. Resolution remains closed
        // until the dedicated proof-static binder telescope and proposition
        // symbol category are present.
        syntax::item::Item::Proposition(proposition) => {
            return Err(Diagnostic::error(format!(
                "proposition `{}` reached symbol resolution before the dedicated proposition-family representation is available",
                proposition.name.as_str()
            )));
        }
        syntax::item::Item::WireData(wire_data) => {
            let wire_schema = lower_wire_schema(lowerer, syntax_trees, wire_data)?;
            // Chapter 20: numbers are INERT schema facts -- a numbered data
            // is ALSO a plain program type (see
            // data_definition_from_wire_schema; the Message/Sample twin
            // corpus pattern was forced by this line's absence).
            let data_definition =
                crate::wire::data_definition_from_wire_schema(lowerer, &wire_schema);
            lowerer.symbol_resolved_trees.wire_schemas.push(wire_schema);
            lowerer
                .symbol_resolved_trees
                .data_definitions
                .push(data_definition);
        }
        // Consts exist only until symbol resolution: validated here, then
        // every `Type::NAME` use substitutes the initializer at expression
        // lowering (crate::constant) -- nothing is carried forward.
        syntax::item::Item::Const(definition) => {
            crate::constant::validate_const_definition(syntax_trees, definition)?;
        }
        syntax::item::Item::Capability(_)
        | syntax::item::Item::Module(_)
        | syntax::item::Item::Package(_)
        | syntax::item::Item::Provider(_)
        | syntax::item::Item::Export(_)
        | syntax::item::Item::Library(_)
        | syntax::item::Item::Target(_)
        | syntax::item::Item::Use(_) => {}
    }

    Ok(())
}
