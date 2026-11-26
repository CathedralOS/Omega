//! Advisory uses collected across the staged compilations of one build.
use crate::ParsedStageSources;
use diagnostics::{Diagnostic, ResolvedSourceLocation};
use source_provenance::DependencyScope;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use syntax_trees::item::{Item, UseItem};

/// The same captured file may be parsed under distinct Build and Product owners.
/// Its path, contents and occurrence identify the authored import across those stages.
#[derive(Clone, Default)]
pub struct FileImportAdvisories(BTreeMap<(PathBuf, usize, Arc<str>), (String, bool)>);

impl FileImportAdvisories {
    pub fn observe_build(
        &mut self,
        stage: &ParsedStageSources,
        program: &crate::source_state::TypedTrees,
    ) {
        use crate::source_state::machine::MachineBodyStatus;
        let mut deferred: std::collections::HashSet<_> = program
            .machines()
            .iter()
            .filter(|machine| machine.body_status == MachineBodyStatus::Deferred)
            .filter_map(|machine| {
                program
                    .symbols
                    .symbol_provenance_source_span(machine.symbol)
            })
            .collect();
        // A generic declaration counts as used if any of its instantiations was compiled.
        if !deferred.is_empty() {
            for machine in program
                .machines()
                .iter()
                .filter(|machine| machine.body_status == MachineBodyStatus::Complete)
            {
                if let Some(span) = program
                    .symbols
                    .symbol_provenance_source_span(machine.symbol)
                {
                    deferred.remove(&span);
                }
            }
        }
        self.observe(stage, DependencyScope::Build, |owner| {
            !deferred.contains(&owner)
        });
    }

    pub fn observe_product(&mut self, stage: &ParsedStageSources) {
        self.observe(stage, DependencyScope::Product, |_| true);
    }

    fn observe(
        &mut self,
        stage: &ParsedStageSources,
        scope: DependencyScope,
        declaration_used: impl Fn(source_text::SourceSpan) -> bool,
    ) {
        let syntax = &stage.syntax.trees;
        for handle in syntax.root_item_handles() {
            let Item::Use(UseItem::File {
                span,
                alias,
                qualifiers,
                ..
            }) = syntax.root_item(*handle)
            else {
                continue;
            };
            let Some(file) = stage.files.get(span.source_id) else {
                continue;
            };
            if !stage.files.source_is_selected(span.source_id) || file.dependency_scope != scope {
                continue;
            }
            let identity = (file.path.clone(), span.span.start, Arc::clone(&file.source));
            let seen = qualifiers.iter().any(|usage| declaration_used(source_text::SourceSpan::new(span.source_id, usage.declaration)));
            self.0
                .entry(identity)
                .or_insert_with(|| (alias.as_str(syntax).to_owned(), false))
                .1 |= seen;
        }
    }

    /// A successfully evaluated Product locator uses the alias in its issuer's file.
    pub fn use_queried_file(&mut self, issuer: &source_admission::SourceFile, name: &str) {
        for ((path, _, source), (alias, used)) in &mut self.0 {
            if path == &issuer.path && alias == name && source == &issuer.source {
                *used = true;
            }
        }
    }

    pub fn extend(&mut self, other: &Self) {
        for (key, (alias, used)) in &other.0 {
            self.0
                .entry(key.clone())
                .or_insert_with(|| (alias.clone(), false))
                .1 |= used;
        }
    }

    pub fn warnings(&self) -> Vec<Diagnostic> {
        self.0
            .iter()
            .filter(|(_, (_, used))| !used)
            .map(|((path, at, source), (alias, _))| {
                let position = source_text::SourcePosition::of(source, *at);
                let mut warning = Diagnostic::warning(format!("unused file import `{alias}`"));
                warning.resolved_location = Some(ResolvedSourceLocation {
                    path: path.clone(),
                    package: None,
                    line: position.line as u32,
                    column: position.column as u32,
                });
                warning
            })
            .collect()
    }
}
