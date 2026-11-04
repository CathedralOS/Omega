//! Explicit evidence namespaces shared by declaration and callable telescopes.

use crate::symbols::symbol_table::names::evidence_requirement_seed;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolTableAppender};

pub(super) fn insert_evidence_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    binder: SymbolHandle,
    bound: &symbol_resolved_trees::machine::GenericConformanceBound,
    sources: Option<&source::SourceMap>,
) {
    builder.insert_children(
        binder,
        crate::symbols::requirement_namespaces::visible_requirements(program, bound, sources)
            .into_iter()
            .map(|requirement| evidence_requirement_seed(&requirement.name)),
    );
}
