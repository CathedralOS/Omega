use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolTable};

use super::expressions::assign_expression_span_symbols;
use super::scopes::MachineScope;

pub(super) fn assign_measure_expression_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let attached_machines = super::scopes::attached_machines(program);
    let binder_roles = super::scopes::BinderRoles::of(program, symbols);
    let declarations = &mut program.tables.declarations;
    let expressions = &mut program.tables.bodies.expressions;
    for measure in &program.roots.measures {
        let Some(parameter) = &measure.parameter else {
            continue;
        };
        let scope = MachineScope {
            attached_machines: &attached_machines,
            binder_roles: &binder_roles,
            resolution_indexes: &program.resolution_indexes,
            symbol: SymbolHandle::invalid(),
            lexical_requirements: super::scopes::LexicalRequirements::none(),
            type_parameters: &[],
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            contract_result_data: symbols::SymbolHandle::invalid(),
            data_definitions: &program.roots.data_definitions,
            conformances: &program.roots.conformances,
            traits: &program.roots.traits,
            operand_owners: &program.tables.types.operand_lifetime_owners,
            data_members: &declarations.data_members,
            data_type_parameters: &declarations.data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        assign_expression_span_symbols(
            symbols,
            &scope,
            std::slice::from_ref(parameter),
            measure.symbol,
            expressions,
            &mut declarations.child_type_references,
            measure.body,
        );
    }
}
