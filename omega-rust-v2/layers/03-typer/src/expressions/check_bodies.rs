use crate::contracts::conformance;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::{Multiplicity, Site, TypeError, TypedSignature, TypedStage, ValueType};
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
            if node.kind == Data && catalog.types.multiplicities.get(unit.declaration(node_index)?) == Some(&Multiplicity::Copy) {
                let id = unit.declaration(node_index)?;
                for shape in std::iter::once(id).chain(catalog.cases.get(id).into_iter().flatten().copied()) {
                    if let Some(fields) = catalog.fields.get(shape)
                        && let Some(name) = catalog.field_order[shape]
                            .iter()
                            .find(|name| catalog.types.multiplicity(&fields[*name]) != Multiplicity::Copy)
                    {
                        return Err(TypeError::at(&unit.file, node_index, format!("copy data contains non-copy field {name}")));
                    }
                }
            }
            if node.kind == Data
                && let Some(properties) = catalog.data_properties.get(unit.declaration(node_index)?)
            {
                output
                    .data_properties
                    .push((unit.declaration(node_index)?.clone(), properties.clone()));
            }
            if matches!(node.kind, Machine | AsmMachine) {
                let id = unit.declaration(node_index)?;
                let signature = &catalog.signatures[id];
                conformance::check_satisfaction(unit, catalog, node_index, output)?;
                for index in 0..catalog.types.arities[id] {
                    if let Some(ValueType::Callable(bound)) = catalog.types.bound(id, index) {
                        output.callable_bounds.push((id.clone(), index, *bound.clone()));
                    }
                }
                output.signatures.push(TypedSignature {
                    site: Site::at(&unit.file, node_index),
                    machine: id.clone(),
                    parameters: signature.params.clone(),
                    result: signature.result.clone(),
                });
            }
            if node.kind == MachineType {
                declaration_bodies::check_callable(unit, catalog, node_index, output)?;
            }
            if node.kind == Conformance {
                conformance::check(unit, catalog, node_index, output)?;
            }
            if node.kind == Const {
                let id = unit.declaration(node_index)?;
                output
                    .constants
                    .push((id.clone(), catalog.constants[id].clone()));
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
