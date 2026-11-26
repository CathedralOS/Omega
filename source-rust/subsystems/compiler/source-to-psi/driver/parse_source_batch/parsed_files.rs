//! Invocation-local syntax retained by source settlement. Package discovery
//! receives authored import/name summaries and opaque reusable parses.
use arena::{Arena, HandleSpan};
use diagnostics::Diagnostic;
use source_files::{DependencyScope, SourceOrigin};
use source_text::{SourceId, SourceSpan};
use std::{path::PathBuf, sync::Arc};
use syntax_trees::{SyntaxTrees, item::Item};

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
#[record(format = b"OmegaSourceSyntax2")]
pub struct SourceSyntax {
    pub(crate) trees: SyntaxTrees,
    files: Vec<ParsedFile>,
}

/// File and item coordinates issued together by the parser.
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
struct ParsedFile {
    id: SourceId,
    path: PathBuf,
    source: Arc<str>,
    scope: DependencyScope,
    origin: Option<SourceOrigin>,
    items: SourceItems,
}

impl SourceSyntax {
    #[cfg(feature = "test-support")]
    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.trees.snapshot_json()
    }

    #[cfg(feature = "test-support")]
    pub fn root_item_count(&self) -> usize {
        self.trees.root_item_count()
    }
    pub fn new(source: SourceId) -> Self {
        Self {
            trees: SyntaxTrees::new(source),
            files: Vec::new(),
        }
    }
    pub fn primary_source(&self) -> SourceId {
        self.trees.source_id
    }
    pub fn set_primary_source(&mut self, source: SourceId) {
        self.trees.source_id = source;
    }
    pub fn share_pages(&mut self) {
        self.trees.share_pages();
    }
    pub(super) fn retain_batch(&mut self, parsed: syntax_trees::ParsedSources) -> ParsedSourceBatch {
        let batch = ParsedSourceBatch::project(parsed, &self.trees);
        self.files.extend(
            batch
                .sources
                .span_or_empty(batch.batch)
                .iter()
                .map(|unit| ParsedFile {
                    id: unit.source_id,
                    path: unit.path.clone(),
                    source: Arc::clone(&unit.source),
                    scope: unit.scope,
                    origin: unit.origin,
                    items: unit.root_items.clone(),
                }),
        );
        batch
    }

    pub(crate) fn take_selected_items(
        &mut self,
        files: &crate::source_inputs::SourceMap,
    ) -> Result<Vec<(SourceId, SourceItems)>, Vec<Diagnostic>> {
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::with_capacity(self.files.len());
        for unit in self.files.drain(..) {
            let selected = files.get(unit.id).filter(|file| {
                files.source_is_selected(unit.id)
                    && file.path == unit.path
                    && file.source == unit.source
                    && file.dependency_scope == unit.scope
                    && Some(file.origin) == unit.origin
            });
            if selected.is_none() || !seen.insert(unit.id) {
                return Err(vec![Diagnostic::error(format!(
                    "parsed file {} does not match its selected source occurrence",
                    unit.path.display()
                ))]);
            }
            items.push((unit.id, unit.items));
        }
        if seen.len() != files.selected_files().count() {
            return Err(vec![Diagnostic::error(
                "selected source is missing from the parsed stage",
            )]);
        }
        self.canonicalize_file_order(files, &mut items)?;
        Ok(items)
    }

    /// Put the files, and the root items resolution walks, in the order of
    /// each file's package and logical filename instead of the order the
    /// loader met them. Resolution, typing and lowering number declarations,
    /// record types and identities in the order they meet root items, so
    /// without this the same program captured in another order publishes
    /// other numbers (`wiki/spec/terminal-psi/encoding.md`, "Canonical
    /// form": unordered sets are sorted by stable identity). Items keep their
    /// written order within a file.
    fn canonicalize_file_order(
        &mut self,
        files: &crate::source_inputs::SourceMap,
        items: &mut [(SourceId, SourceItems)],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut keyed = Vec::with_capacity(items.len());
        for (position, (id, _)) in items.iter().enumerate() {
            let file = files.get(*id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "parsed file has no selected source occurrence",
                )]
            })?;
            // A source with no logical filename, such as the default Build,
            // is ordered by its path, which also separates two packages'
            // files of one name when neither package has an identity.
            let name = file.logical_filename().unwrap_or_default();
            keyed.push((file.package_identity, name, file.path.clone(), position));
        }
        keyed.sort_unstable();
        if keyed
            .iter()
            .enumerate()
            .all(|(rank, (_, _, _, position))| rank == *position)
        {
            return Ok(());
        }
        let mut sorted: Vec<_> = keyed
            .iter()
            .map(|(_, _, _, position)| items[*position].clone())
            .collect();
        // Each file's rank in the canonical order, looked up by its source,
        // for the root lists that record a span instead of their file.
        let mut ranks: Vec<(usize, usize)> = sorted
            .iter()
            .enumerate()
            .map(|(rank, (id, _))| (id.0, rank))
            .collect();
        ranks.sort_unstable();
        let rank_of = |source: SourceId| {
            ranks
                .binary_search_by_key(&source.0, |(id, _)| *id)
                .map_or(usize::MAX, |found| ranks[found].1)
        };

        // Each root item's file rank, looked up by its handle. An item no
        // parsed file lists keeps its place after the files' items.
        let mut item_ranks: Vec<((u32, u32), usize)> = sorted
            .iter()
            .enumerate()
            .flat_map(|(rank, (_, file))| {
                file.handles
                    .iter()
                    .map(move |handle| ((handle.generation(), handle.arena_index()), rank))
            })
            .collect();
        item_ranks.sort_unstable();
        let roots = &mut self.trees.roots;
        roots.items.sort_by_cached_key(|handle| {
            item_ranks
                .binary_search_by_key(&(handle.generation(), handle.arena_index()), |row| row.0)
                .map_or(usize::MAX, |found| item_ranks[found].1)
        });
        let tables = &self.trees.tables;
        roots.mathematical_definitions.sort_by_cached_key(|handle| {
            rank_of(
                tables
                    .items
                    .mathematical_definition(*handle)
                    .name
                    .source_span()
                    .source_id,
            )
        });
        roots
            .constant_extent_probes
            .sort_by_cached_key(|(expression, _)| {
                rank_of(tables.expressions.source_span(*expression).source_id)
            });
        items.swap_with_slice(&mut sorted);
        Ok(())
    }

    pub fn float_literal_spans(&self) -> impl Iterator<Item = SourceSpan> + '_ {
        self.trees
            .tables
            .expressions
            .iter_expressions()
            .filter(|(_, node)| matches!(node, syntax_trees::expression::ExpressionNode::Float(_)))
            .map(|(handle, _)| self.trees.tables.expressions.source_span(handle))
    }
}

/// Parser-issued roots. Acquisition can retain them, but cannot fabricate or
/// edit a declaration through them. Build vocabulary discovery uses summaries.
#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct SourceItems {
    pub(crate) handles: Vec<syntax_trees::item::ItemHandle>,
    build_machines: Vec<source_text::Span>,
    build_data: bool,
}
impl SourceItems {
    pub fn build_machines(&self) -> &[source_text::Span] {
        &self.build_machines
    }
    pub fn declares_build_data(&self) -> bool {
        self.build_data
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceImport {
    pub ordinal: usize,
    pub span: source_text::Span,
    pub target: SourceImportTarget,
    pub qualifiers: Vec<syntax_trees::item::FileQualifier>,
}

impl SourceImport {
    /// A symbolic operand of core field/parameter reflection may be a Product
    /// query. Only an ordinary Build reference makes a host binding mandatory.
    pub fn requires_build_binding(&self) -> bool {
        self.qualifiers
            .iter()
            .any(|usage| usage.role == syntax_trees::item::FileQualifierRole::Code)
    }

    pub fn used_in(&self, scope: source_provenance::DependencyScope) -> bool {
        use syntax_trees::item::FileQualifierRole;
        match scope {
            source_provenance::DependencyScope::Product => !self.qualifiers.is_empty(),
            source_provenance::DependencyScope::Build => {
                self.qualifiers.iter().any(|usage| match usage.role {
                    FileQualifierRole::Code => true,
                    FileQualifierRole::Product => false,
                    // A supplied Build package can own an ordinarily reflected declaration.
                    // A Product description alone does not import a local file into Build.
                    FileQualifierRole::Description => self.target.root.is_some(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
pub struct SourceImportTarget {
    pub root: Option<String>,
    /// The admitted logical filename always uses slash separators.
    pub path: String,
    pub alias: String,
}

impl std::fmt::Display for SourceImportTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(root) = &self.root {
            write!(f, "{{{root}}}/")?;
        }
        write!(f, "{} as {}", self.path, self.alias)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSourceSummary {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    pub root_items: SourceItems,
    pub imports: Vec<SourceImport>,
    pub scope: DependencyScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSourceBatch {
    pub sources: Arena<ParsedSourceSummary>,
    pub batch: HandleSpan<ParsedSourceSummary>,
}
impl ParsedSourceBatch {
    fn project(parsed: syntax_trees::ParsedSources, syntax: &SyntaxTrees) -> Self {
        let mut sources = Arena::default();
        let batch = sources.insert_many(
            parsed
                .sources
                .span_or_empty(parsed.batch)
                .iter()
                .map(|unit| {
                    let imports = unit.root_items[unit.import_root_range.clone()]
                        .iter()
                        .enumerate()
                        .filter_map(|(offset, handle)| match syntax.root_item(*handle) {
                            Item::Use(import) => {
                                let mut path = syntax
                                    .items
                                    .identifier_path_members(import.components)
                                    .iter()
                                    .map(|name| name.as_str(syntax))
                                    .collect::<Vec<_>>()
                                    .join("/");
                                path.push_str(".omg");
                                Some(SourceImport {
                                    ordinal: unit.import_root_range.start + offset,
                                    span: import.span.span,
                                    target: SourceImportTarget {
                                        root: import
                                            .root
                                            .as_ref()
                                            .map(|name| name.as_str(syntax).to_owned()),
                                        path,
                                        alias: import.alias.as_str(syntax).to_owned(),
                                    },
                                    qualifiers: import.qualifiers.clone(),
                                })
                            }
                            _ => None,
                        })
                        .collect();
                    let mut root_items = SourceItems {
                        handles: unit.root_items.clone(),
                        ..Default::default()
                    };
                    for handle in &unit.root_items {
                        match syntax.root_item(*handle) {
                            Item::Machine(machine)
                                if machine.attached_data.is_none()
                                    && machine.name.as_str(syntax) == "build" =>
                            {
                                root_items
                                    .build_machines
                                    .push(machine.name.source_span().span)
                            }
                            Item::Data(data) if data.name.as_str(syntax) == "BuildPlan" => {
                                root_items.build_data = true
                            }
                            _ => {}
                        }
                    }
                    ParsedSourceSummary {
                        source_id: unit.source_id,
                        path: unit.path.clone(),
                        source: Arc::clone(&unit.source),
                        origin: unit.origin,
                        root_items,
                        imports,
                        scope: unit.scope,
                    }
                })
                .collect::<Vec<_>>(),
        );
        Self { sources, batch }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct RetainedSourceParse {
    pub(super) unit: syntax_trees::ParsedUnit,
}

impl RetainedSourceParse {
    /// Name spans of free machines in this file, before source coordinates are assigned.
    pub fn free_machines<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = source_text::Span> + 'a {
        let syntax = &self.unit.syntax;
        self.unit
            .root_items()
            .iter()
            .filter_map(move |item| match syntax.root_item(*item) {
                Item::Machine(machine)
                    if machine.attached_data.is_none() && machine.name.as_str(syntax) == name =>
                {
                    Some(machine.name.source_span().span)
                }
                _ => None,
            })
    }

    pub fn declares_operator_family(&self) -> bool {
        let syntax = &self.unit.syntax;
        self.unit.root_items().iter().any(|item| {
            matches!(syntax.root_item(*item), Item::Machine(machine) if machine.spelling.is_some())
        })
    }
}
