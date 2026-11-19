use crate::TypeError;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::toolchain_declarations::source_package::TOOLCHAIN_PACKAGE;
use omega_resolver::SymbolResolvedTrees;

pub(crate) fn index_resolved_sources<'a>(resolved: &'a SymbolResolvedTrees<'_>) -> Result<Vec<ResolvedSourceFile<'a>>, TypeError> {
    resolved
        .trees()
        .map(|tree| {
            let file = tree.file;
            if file.package == TOOLCHAIN_PACKAGE {
                return Err(TypeError::at(file, 0, "package identity reserved for the toolchain"));
            }
            Ok(ResolvedSourceFile {
                tree,
                parameters: Default::default(),
                hypotheses: Default::default(),
            })
        })
        .collect()
}
