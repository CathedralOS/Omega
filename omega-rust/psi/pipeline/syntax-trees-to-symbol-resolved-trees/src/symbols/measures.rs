use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolTable};

use super::expressions::assign_expression_span_symbols;
use super::scope::MachineScope;

pub(super) fn assign_measure_expression_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let attached_machines = super::scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    let expressions = &mut program.tables.bodies.expressions;
    for measure in &program.roots.measures {
        let Some(parameter) = &measure.parameter else {
            // Lexicographic field lists have no parameter binder. Their
            // projection semantics remain with the existing ranking owner.
            continue;
        };
        // Reuse expression traversal with the measure's lexical child scope,
        // but no machine receiver: `self` must not name this declaration.
        let scope = MachineScope {
            attached_machines: &attached_machines,
            symbol: SymbolHandle::invalid(),
            type_parameters: &[],
            attached_data: None,
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            data_members: &declarations.data_members,
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
