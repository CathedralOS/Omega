use crate::TypeError;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::toolchain_declarations::source_package::TOOLCHAIN_PACKAGE;
use omega_parser::NodeKind;
use omega_resolver::SymbolResolvedTrees;

pub(crate) fn index_resolved_sources<'a>(resolved: &'a SymbolResolvedTrees<'_>) -> Result<Vec<ResolvedSourceFile<'a>>, TypeError> {
    resolved
        .trees()
        .map(|tree| {
            let file = tree.file;
            if file.package == TOOLCHAIN_PACKAGE {
                return Err(TypeError::at(file, 0, "package identity reserved for the toolchain"));
            }
            let binding_annotations = file
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::Param)
                .filter_map(|p| Some((p.words().next()?.span, *p.children.first()?)))
                .collect();
            Ok(ResolvedSourceFile {
                tree,
                binding_annotations,
                parameters: Default::default(),
                hypotheses: Default::default(),
            })
        })
        .collect()
}
