//! The parser output, capture and import bindings supplied to one Build or Product.

use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use crate::driver::resolve_and_type::ResolveAndTypeRequest;
use crate::{SourceInput, SourceSyntax, CompileTimings};
use diagnostics::Diagnostic;
use source_files::{LoadedSource, LoadedSources};
use source_text::SourceId;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct ParsedStageSources {
    pub syntax: SourceSyntax,
    pub files: Arc<SourceMap>,
    pub roots: Vec<SourceId>,
    pub imports: Vec<SourceScopedTopLevelBinding>,
}

impl ParsedStageSources {
    /// Parse an independently acquired capture through the numbered source stages.
    pub fn parse(
        files: Arc<SourceMap>,
        roots: Vec<SourceId>,
        imports: Vec<SourceScopedTopLevelBinding>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let primary = roots
            .first()
            .copied()
            .ok_or_else(|| vec![Diagnostic::error("stage has no root source")])?;
        let loaded: LoadedSources = files
            .selected_files()
            .map(|file| LoadedSource {
                source_id: file.source_id,
                path: file.path.clone(),
                source: Arc::clone(&file.source),
                origin: Some(file.origin),
                scope: file.dependency_scope,
            })
            .collect();
        let mut syntax = SourceSyntax::new(primary);
        SourceInput(loaded).append(&mut syntax, &mut CompileTimings::default())?;
        Ok(Self {
            syntax,
            files,
            roots,
            imports,
        })
    }

    pub(crate) fn into_request(
        mut self,
        host: &'static str,
        purpose: source_provenance::DependencyScope,
        primary: SourceId,
    ) -> Result<ResolveAndTypeRequest, Vec<Diagnostic>> {
        if !self.files.source_is_selected(primary)
            || self
                .roots
                .iter()
                .any(|id| !self.files.source_is_selected(*id))
        {
            return Err(vec![Diagnostic::error(
                "stage root is outside its selected sources",
            )]);
        }
        self.syntax.set_primary_source(primary);
        let source_items = self.syntax.take_selected_items(&self.files)?;
        Ok(ResolveAndTypeRequest {
            syntax: self.syntax,
            source_items,
            root_sources: self.roots,
            sources: self.files,
            top_level_bindings: self.imports,
            build_helper_identity: host,
            purpose,
            selection_authority: None,
            frontends: Default::default(),
        })
    }
}
