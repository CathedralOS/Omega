use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::{TypeError, TypedStage};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::HashMap;

pub(crate) fn infer<'a>(
    units: &[ResolvedSourceFile<'a>],
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut inferred = Vec::new();
    let mut indices = HashMap::new();
    for unit in units {
        for (n, id) in unit
            .declaration_nodes()
            .filter(|(_, id)| id.kind == NodeKind::Const)
        {
            if let [_, expr] = *unit.file.nodes[n].children.as_slice() {
                indices.insert(id, inferred.len());
                inferred.push((unit, id, expr));
            }
        }
    }
    let dependencies = inferred.iter().map(|&(unit, _, expr)| {
        (expr..unit.file.nodes[expr].end)
            .filter_map(|n| match unit.symbol(n)? {
                Symbol::Declaration(id) => indices.get(id).copied(),
                _ => None,
            })
            .collect()
    });
    super::dependency_order::check_in_dependency_order(
        dependencies,
        |i| {
            let (unit, id, expr) = inferred[i];
            match declaration_bodies::infer_constant(unit, catalog, expr, output) {
                Ok(value) => {
                    catalog.constants.insert(id, value);
                }
                Err(error) if error.waiting_for_static_value => catalog.types.indices.defer_constant(id),
                Err(error) => return Err(error),
            }
            Ok(())
        },
        |i| {
            let (unit, _, expr) = inferred[i];
            TypeError::at(unit.file, expr, "cannot infer a constant through a dependency cycle")
        },
    )
}
