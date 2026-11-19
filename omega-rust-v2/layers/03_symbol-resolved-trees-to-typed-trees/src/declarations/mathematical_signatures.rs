use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::{MathBinding, NodeKind};
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;

pub(crate) fn index_interfaces<'a>(
    units: &[ResolvedSourceFile<'a>],
    types: &mut TypeCatalog<'a>,
) -> Result<(), TypeError> {
    let mut entries = Vec::new();
    let mut index = HashMap::new();
    for unit in units {
        for (n, id) in unit
            .declaration_nodes()
            .filter(|(_, id)| id.kind == NodeKind::MathLet)
        {
            index.insert(id, entries.len());
            entries.push((unit, n, id, unit.file.math_binding(n).unwrap()));
        }
    }
    let dependencies = entries.iter().map(|&(unit, n, _, binding)| {
        (n + 1..unit.file.nodes[binding.result].end)
            .filter_map(|child| {
                let node = &unit.file.nodes[child];
                if !matches!(node.kind, NodeKind::FamilyApplication | NodeKind::Call) {
                    return None;
                }
                let Symbol::Declaration(id) = unit.symbol(*node.children.first()?)? else {
                    return None;
                };
                index.get(id).copied()
            })
            .collect()
    });
    super::dependency_order::check_in_dependency_order(
        dependencies,
        |i| {
            let (unit, node, id, binding) = entries[i];
            unit.index_mathematical(id, node, binding, types)
        },
        |i| {
            let (unit, node, _, _) = entries[i];
            TypeError::at(unit.file, node, "cyclic mathematical family signatures")
        },
    )
}

impl<'a> ResolvedSourceFile<'a> {
    fn index_mathematical(
        &self,
        id: &'a DeclarationId,
        node: usize,
        binding: MathBinding,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let mut parameters = Vec::new();
        for (binder, annotation) in self.file.parameters(node) {
            parameters.push((self.binder_site(binder.span), self.annotation(annotation, types)?));
        }
        let result = self.annotation(binding.result, types)?;
        let ty = parameters
            .into_iter()
            .rev()
            .fold(result, |out, (binder, p)| ValueType::Function(Box::new(p), Box::new(out.bind_subject(&binder))));
        types.mathematical.insert(id, ty);

        Ok(())
    }
}
