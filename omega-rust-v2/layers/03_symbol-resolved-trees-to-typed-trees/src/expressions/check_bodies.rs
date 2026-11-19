use crate::contracts::conformance;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::{TypeError, TypedStage};
use omega_parser::NodeKind;

pub(crate) fn check_bodies(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        if unit.file.nodes[0].kind == NodeKind::Fragment {
            declaration_bodies::check_fragment(unit, catalog, output)?;
        }
        for (node_index, node) in unit.file.nodes.iter().enumerate().skip(1) {
            use NodeKind::*;
            if node.kind == Data {
                crate::declarations::data_properties::check(unit, node_index, catalog, output)?;
            }
            if node.kind == MachineType {
                declaration_bodies::check_callable(unit, catalog, node_index, output)?;
            }
            if node.kind == Conformance {
                conformance::check(unit, catalog, node_index, output)?;
            }
            if node.kind == Const {
                let id = unit.declaration(node_index)?;
                let initializer = *node
                    .children
                    .last()
                    .ok_or_else(|| TypeError::unsupported(unit.file, node_index))?;
                output.constants.push(crate::TypedConstant {
                    declaration: id.clone(),
                    initializer: crate::StaticValueRequest::at(unit.file, initializer, &catalog.constants[id]),
                });
                if node.children.len() == 2 {
                    continue;
                }
            }
            if matches!(node.kind, Machine | AsmMachine | MathLet | Data | Domain | Const) {
                declaration_bodies::check_declaration(unit, catalog, node_index, output)?;
            }
        }
    }
    Ok(())
}
