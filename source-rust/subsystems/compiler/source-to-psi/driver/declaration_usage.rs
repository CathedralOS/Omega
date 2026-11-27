//! Advisory unused-declaration tracking merged across the Build and Product stages of one build.

use crate::compile_time_invocations::from_supplied_roots;
use crate::source_state::TypedTrees;
use crate::source_state::machine::MachinePurpose;
use crate::source_state::references::{ComputationDependencyRows, collect_program_declarations};
use crate::source_state::source_subjects;
use crate::source_state::trait_definition::Supply;
use crate::source_state::visibility::declaration_visibility;
use diagnostics::{Diagnostic, ResolvedSourceLocation};
use semantic_vocabulary::PackageKeyIdentity;
use source_provenance::DependencyScope;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use symbols::{SymbolHandle, SymbolKind};

/// The same captured file may be checked under Build and Product scopes or across
/// multiple targets. Its path, start byte and source contents identify the authored
/// declaration across those stages, matching `FileImportAdvisories`.
#[derive(Clone, Default)]
pub struct DeclarationUsageAdvisories(
    BTreeMap<(PathBuf, usize, Arc<str>), (&'static str, String, bool)>,
);

impl DeclarationUsageAdvisories {
    /// Observe every package-private top-level declaration (`machine`, `const`, `data`,
    /// `domain`) in `subjects` for `purpose`, marking each one used if it is reachable
    /// from `roots`, `supplies`, any `public` declaration in `subjects`, or any
    /// conformance in `subjects`.
    pub(crate) fn observe(
        &mut self,
        program: &TypedTrees,
        subjects: &[Option<PackageKeyIdentity>],
        purpose: DependencyScope,
        roots: &[SymbolHandle],
        supplies: &[Supply],
    ) -> Result<(), String> {
        if subjects.is_empty() {
            return Ok(());
        }
        let declarations = collect_program_declarations(program);
        let mut all_roots = roots.to_vec();
        for &symbol in &declarations {
            if !program.symbols.symbols().nodes().is_valid(symbol)
                || program.symbols.authored_origin(symbol) != symbol
            {
                continue;
            }
            if !source_subjects::contains(program, symbol, subjects, purpose)
                .map_err(str::to_owned)?
            {
                continue;
            }
            let kind = program.symbols.get(symbol).kind;
            let is_public = declaration_visibility(program, symbol)
                .is_some_and(|visibility| visibility.is_public());
            if is_public || kind == SymbolKind::Conformance {
                all_roots.push(symbol);
            }
        }
        let dependencies = ComputationDependencyRows::of(program);
        let used = from_supplied_roots(program, &dependencies, all_roots, supplies.iter().copied())
            .map_err(str::to_owned)?;
        for symbol in declarations {
            if !program.symbols.symbols().nodes().is_valid(symbol)
                || program.symbols.authored_origin(symbol) != symbol
            {
                continue;
            }
            let parent = program.symbols.get(symbol).parent;
            let is_top_level = !parent.is_valid()
                || matches!(
                    program.symbols.get(parent).kind,
                    SymbolKind::Root | SymbolKind::Module
                );
            if !is_top_level {
                continue;
            }
            let kind_label = match program.symbols.get(symbol).kind {
                SymbolKind::Machine => {
                    if program
                        .machine_by_symbol(symbol)
                        .is_some_and(|m| m.purpose == MachinePurpose::ProviderDefaults)
                    {
                        continue;
                    }
                    "machine"
                }
                SymbolKind::Const => "const",
                SymbolKind::Data => "data",
                SymbolKind::Domain => "domain",
                _ => continue,
            };
            if !source_subjects::contains(program, symbol, subjects, purpose)
                .map_err(str::to_owned)?
            {
                continue;
            }
            if !declaration_visibility(program, symbol)
                .is_some_and(|visibility| !visibility.is_public())
            {
                continue;
            }
            let Some(span) = program.symbols.symbol_provenance_source_span(symbol) else {
                continue;
            };
            let Some(file) = program.symbols.source_file(span) else {
                continue;
            };
            let identity = (file.path.clone(), span.span.start, Arc::clone(&file.source));
            let name = program.symbols.name(symbol).to_owned();
            let seen = used.contains(symbol);
            self.0
                .entry(identity)
                .or_insert_with(|| (kind_label, name, false))
                .2 |= seen;
        }
        Ok(())
    }

    pub fn extend(&mut self, other: &Self) {
        for (key, (kind, name, used)) in &other.0 {
            self.0
                .entry(key.clone())
                .or_insert_with(|| (*kind, name.clone(), false))
                .2 |= used;
        }
    }

    pub fn warnings(&self) -> Vec<Diagnostic> {
        self.0
            .iter()
            .filter(|(_, (_, _, used))| !used)
            .map(|((path, at, source), (kind, name, _))| {
                let position = source_text::SourcePosition::of(source, *at);
                let mut warning = Diagnostic::warning(format!("unused {kind} `{name}`"));
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
