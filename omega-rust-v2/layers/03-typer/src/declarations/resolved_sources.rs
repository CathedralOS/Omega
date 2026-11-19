use crate::TypeError;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::toolchain_declarations::source_package::TOOLCHAIN_PACKAGE;
use omega_parser::NodeKind;
use omega_resolver::{IndexedSource, ResolvedStage, StageSource};
use std::collections::HashMap;

pub(crate) fn index_resolved_sources<'a>(
    sources: &[StageSource<'a>],
    resolved: &'a ResolvedStage,
) -> Result<Vec<ResolvedSourceFile<'a>>, TypeError> {
    let mut units = Vec::new();
    let mut by_path = HashMap::new();
    for src in sources {
        let f = IndexedSource::read(src).map_err(|e| TypeError {
            package: e.package,
            file: e.file,
            span: e.span,
            message: e.message,
        })?;
        if f.package == TOOLCHAIN_PACKAGE {
            return Err(TypeError::at(&f, 0, "package identity reserved for the toolchain"));
        }
        by_path.insert((f.package, f.path), units.len());
        let binding_annotations = f
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Param)
            .filter_map(|p| Some((p.words().next()?.span, *p.children.first()?)))
            .collect();
        units.push(ResolvedSourceFile {
            binding_annotations,
            file: f,
            names: HashMap::new(),
            parameters: HashMap::new(),
            definitions: HashMap::new(),
            hypotheses: HashMap::new(),
        });
    }
    for u in &resolved.uses {
        if let Some(&i) = by_path.get(&(u.package.as_str(), u.file.as_str())) {
            units[i].names.insert(u.span, &u.target);
        }
    }
    for (id, site) in resolved.declarations.iter().zip(&resolved.definition_sites) {
        if let Some(&i) = by_path.get(&(id.package.as_str(), site.file.as_str())) {
            units[i].definitions.insert(site.span, id);
        }
    }
    units.sort_by_key(|u| (u.file.package, u.file.path));
    Ok(units)
}
