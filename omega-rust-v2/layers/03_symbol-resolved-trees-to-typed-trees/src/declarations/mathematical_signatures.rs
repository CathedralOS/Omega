use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn index_mathematical(
        &self,
        node_index: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let source = self.file;
        let node = &source.nodes[node_index];
        let id = self.declaration(node_index)?;
        let mut parameters = Vec::new();
        let mut result = None;
        let equal = node
            .leaves
            .iter()
            .find(|l| l.text == "=")
            .map(|l| l.span.get_start_offset());
        for &part in &node.children {
            if equal.is_some_and(|offset| source.span(part).get_start_offset() > offset) {
                continue;
            }
            match source.nodes[part].kind {
                GenericParams => {}
                Params => {
                    for &p in &source.nodes[part].children {
                        let binder = source.nodes[p]
                            .words()
                            .next()
                            .ok_or_else(|| TypeError::unsupported(source, p))?;
                        parameters.push((self.binder_site(binder.span), self.annotation(self.single_child(p)?, types)?));
                    }
                }
                _ => result = Some(self.annotation(part, types)?),
            }
        }
        let ty = parameters
            .into_iter()
            .rev()
            .fold(result.ok_or_else(|| TypeError::unsupported(source, node_index))?, |out, (binder, p)| {
                ValueType::Function(Box::new(p), Box::new(out.bind_subject(&binder)))
            });
        types.mathematical.insert(id, ty);

        Ok(())
    }
}
