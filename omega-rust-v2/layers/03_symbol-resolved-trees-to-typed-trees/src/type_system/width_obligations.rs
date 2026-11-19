use super::type_catalog::TypeCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{ObligationKind, Site, TypeError, TypedStage, TypingObligation, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

pub(crate) fn collect_width_obligations(
    units: &[ResolvedSourceFile<'_>],
    types: &mut TypeCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        for (node_index, node) in unit.file.nodes.iter().enumerate() {
            if node.kind == NodeKind::PathType
                && matches!(unit.symbol(node_index),Some(Symbol::Builtin(s)) if matches!(s.as_str(),"u"|"i"))
                && let Some(section) = unit.file.child(node_index, NodeKind::TypeArgs)
            {
                let arg = unit.single_child(section)?;
                let width = match unit.symbol(arg) {
                    Some(Symbol::Binding {
                        span,
                        kind: NodeKind::ConstParam | NodeKind::KindedParam,
                        ..
                    }) => unit.parameters.get(span).cloned(),
                    _ => unit.open_index(types, arg, &ValueType::integer("u64").unwrap())?,
                };
                let Some(width) = width else {
                    continue;
                };
                types.positive_widths.insert(width.clone());
                output.obligations.push(TypingObligation {
                    site: Site::at(unit.file, node_index),
                    kind: ObligationKind::PositiveWidth(width.clone()),
                });
            }
        }
    }
    Ok(())
}
