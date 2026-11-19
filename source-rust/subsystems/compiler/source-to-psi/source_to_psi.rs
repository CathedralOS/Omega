//! Run the source pipeline once over the complete file set for one stage.

#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
use source_admission::{LoadedSource, LoadedSources};

#[path = "pipeline/modules.rs"]
mod stages;
pub use stages::*;

use product_source::{ProductSourceInputs, prepare_lower_and_check};
use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use source_text::SourceId;
use std::sync::Arc;

pub type PublishedPsi = Arc<terminal_codec::CanonicalTerminalArtifact>;

/// Captured files include generated source from earlier stages.
/// Bindings belong to this compilation's selected packages and source identities.
pub struct StageSource<'a> {
    pub files: Arc<SourceMap>,
    pub roots: Vec<SourceId>,
    pub imports: Vec<SourceScopedTopLevelBinding>,
    pub selections: ProductSourceInputs<'a>,
    pub declaration_authority:
        Option<&'a dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>,
    pub build_host: target::TargetProfile,
}

pub fn compile(source: StageSource<'_>) -> Result<PublishedPsi, Vec<Diagnostic>> {
    let mut observer = UnmeasuredProduction;
    let parsed = lex_and_parse(&source, &mut observer)?;
    let PreparedSource { typed, mut accounting } = parsed.resolve_and_type(&mut observer)?;
    let optimizations = source.selections.optimizations.clone();
    let checked = prepare_lower_and_check(
        typed,
        source.selections,
        &Default::default(),
        &mut accounting,
        None,
        None,
        &mut observer,
    )?;
    let psi = checked.terminal_publication.publish(
        &optimizations, source.declaration_authority, &mut observer,
    )?;
    Ok(Arc::clone(psi.artifact()))
}

fn lex_and_parse(
    source: &StageSource<'_>,
    observer: &mut impl PsiCompilationObserver,
) -> Result<ResolveAndTypeRequest, Vec<Diagnostic>> {
    let Some(&primary) = source.roots.first() else {
        return Err(vec![Diagnostic::error("stage has no root source")]);
    };
    if source.roots.iter().any(|id| !source.files.source_is_selected(*id)) {
        return Err(vec![Diagnostic::error(
            "stage root is outside its selected sources",
        )]);
    }
    let files: LoadedSources = source.files.files()
        .filter(|file| source.files.source_is_selected(file.source_id))
        .map(|file| LoadedSource {
            source_id: file.source_id,
            path: file.path.clone(),
            source: Arc::clone(&file.source),
            origin: Some(file.origin),
            scope: file.dependency_scope,
        })
        .collect();
    let mut syntax = SourceSyntax::new(primary);
    let parsed = SourceInput(files).append(&mut syntax, observer)?;
    let source_items = parsed.sources.span_or_empty(parsed.batch)
        .iter()
        .map(|unit| (unit.source_id, unit.root_items.clone()))
        .collect();
    Ok(ResolveAndTypeRequest {
        syntax,
        source_items,
        root_sources: source.roots.clone(),
        sources: Arc::clone(&source.files),
        top_level_bindings: source.imports.clone(),
        selection_authority: source.selections.layout_selection_authority.clone(),
        build_helper_identity: source.build_host.target_name(),
        build_sources: Default::default(),
        build_stage: None,
        frontends: Default::default(),
    })
}
