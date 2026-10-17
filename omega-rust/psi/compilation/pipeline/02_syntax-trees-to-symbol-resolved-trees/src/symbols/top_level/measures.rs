use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolKind, SymbolTable};

use super::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;

pub(super) fn assign_measure_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let references = &mut program.tables.declarations.child_type_references;
    let constraints = &program.tables.types.constraints;
    program.roots.measures.for_each_mut(|measure| {
        if let Some(parameter) = &mut measure.parameter {
            let mut children = symbols.child_handles(measure.symbol).into_iter().flatten();
            parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                references,
                constraints,
                &[],
                &mut parameter.type_reference,
            );
        }
        if let Some(return_type) = &mut measure.return_type {
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                references,
                constraints,
                &[],
                return_type,
            );
        }
    });
}
