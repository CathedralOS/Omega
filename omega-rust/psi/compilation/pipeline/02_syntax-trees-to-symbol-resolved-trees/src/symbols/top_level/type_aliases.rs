//! Assign alias-owned parameters and references without a nominal data owner.

use super::{
    MachineParameterArm, SignatureSymbolTables, next_child_of_kind, trait_parameter_tables,
};
use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::TypeParameterKind;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

pub(super) fn assign_type_alias_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let (_, trait_proposition_slots, trait_machine_identity_slots) =
        trait_parameter_tables(program);
    let constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let references = &mut declarations.child_type_references;
    program.roots.type_aliases.for_each_mut(|alias| {
        let mut children = symbols.child_handles(alias.symbol).into_iter().flatten();
        for parameter in parameters.span_mut_or_empty(alias.type_parameters) {
            let kind = match parameter.kind {
                TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
                _ => SymbolKind::TypeParameter,
            };
            parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        let locals = parameters.span_or_empty(alias.type_parameters).to_vec();
        SignatureSymbolTables {
            symbols,
            data_type_parameters: parameters,
            state_parameters,
            child_type_references: references,
            type_constraints: constraints,
        }
        .resolve_type_parameter_kind_symbols(
            alias.type_parameters,
            &locals,
            MachineParameterArm::ResolveWithAuthoredTraitArguments,
            true,
        );
        for bound in &mut alias.conformance_bounds {
            bound.binder = locals
                .iter()
                .find(|parameter| {
                    parameter.name == bound.binder_name
                        && parameter.kind == TypeParameterKind::Implementation
                })
                .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
        }
        super::assign_conformance_bound_symbols(
            symbols,
            references,
            constraints,
            &locals,
            &mut alias.conformance_bounds,
            &trait_proposition_slots,
            &trait_machine_identity_slots,
        );
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            references,
            constraints,
            &locals,
            &mut alias.expansion,
        );
    });
}
