use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::top_level_symbol_for_source;
use crate::symbols::top_level::{
    SignatureSymbolTables, assign_machine_parameter_signature_symbols, next_child_of_kind,
    trait_parameter_tables,
};
use crate::symbols::type_references::{
    assign_trait_bound_slot_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
};

pub(super) fn assign_trait_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // A transparent refinement (`trait R = Base`) is a structural bound over
    // an existing base conformance, never a nominal conformance target: no
    // `satisfies R`, no `requires R`, no `:` parent. Evidence binders
    // (`B: L satisfies R`) are the legal consumer.
    let (refinement_symbols, trait_proposition_slots, trait_machine_identity_slots) =
        trait_parameter_tables(symbols, program);
    let type_constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let trait_requirements = &mut declarations.trait_requirements;
    let trait_machine_signatures = &mut declarations.trait_machine_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.traits.for_each_mut(|trait_definition| {
        let trait_symbol = trait_definition.symbol;
        let mut trait_children = symbols.child_handles(trait_symbol).into_iter().flatten();

        for type_parameter in
            data_type_parameters.span_mut_or_empty(trait_definition.type_parameters)
        {
            let kind = match type_parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Implementation => {
                    SymbolKind::ConformanceParameter
                }
                symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                    SymbolKind::PropositionParameter
                }
                symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            type_parameter.symbol = next_child_of_kind(&mut trait_children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(trait_definition.type_parameters)
            .to_vec();

        let trait_name = trait_definition.name.clone();
        if let Some(base) = &mut trait_definition.refines {
            base.symbol =
                top_level_symbol_for_source(symbols, SymbolKind::Trait, &base.name);
            if refinement_symbols.contains(&base.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "transparent refinement `{}` refines `{}`, which is itself a refinement; refine the nominal base directly",
                    trait_name.as_str(),
                    base.name.as_str(),
                )));
            }
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                base.arguments,
);
            assign_trait_bound_slot_argument_symbols(
                symbols, child_type_references, &local_type_parameters, base.symbol, base.arguments,
                &trait_proposition_slots, &trait_machine_identity_slots,
            );
        }

        for bound in &mut trait_definition.conformance_bounds {
            bound.binder = local_type_parameters.iter()
                .find(|parameter| parameter.name == bound.binder_name
                    && parameter.kind == symbol_resolved_trees::data::TypeParameterKind::Implementation)
                .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
        }

        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
        }
        .resolve_type_parameter_kind_symbols(
            trait_definition.type_parameters,
            &local_type_parameters,
            crate::symbols::top_level::MachineParameterArm::Skip,
            true,
        );

        crate::symbols::top_level::assign_conformance_bound_symbols(
            symbols,
            child_type_references,
            type_constraints,
            &local_type_parameters,
            &mut trait_definition.conformance_bounds,
            &trait_proposition_slots,
            &trait_machine_identity_slots,
        );

        for requirement in trait_requirements.span_mut_or_empty(trait_definition.requires) {
            requirement.symbol =
                top_level_symbol_for_source(symbols, SymbolKind::Trait, &requirement.name);
            if refinement_symbols.contains(&requirement.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "trait requirement `{}` is a transparent refinement — refinements bound evidence, they are not conformance targets",
                    requirement.name.as_str(),
                )));
            }
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                requirement.arguments,
            );
            assign_trait_bound_slot_argument_symbols(
                symbols,
                child_type_references,
                &local_type_parameters,
                requirement.symbol,
                requirement.arguments,
                &trait_proposition_slots,
                &trait_machine_identity_slots,
            );
        }

        for machine in trait_machine_signatures.span_mut_or_empty(trait_definition.machines) {
            machine.symbol = next_child_of_kind(&mut trait_children, symbols, SymbolKind::State);
            let machine_symbol = machine.symbol;
            assign_machine_parameter_signature_symbols(
                &mut SignatureSymbolTables {
                    symbols,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                },
                machine,
                machine_symbol,
                &local_type_parameters,
);
        }
    });
    diagnostics
}
