//! Invocation-local syntax retained by source settlement. Package discovery
//! receives authored import/name summaries and opaque reusable parses.
use crate::PsiCompilationObserver;
use arena::{Arena, HandleSpan};
use declaration_contracts::source_identity::DependencyScope;
use diagnostics::Diagnostic;
use source_admission::{LoadedSource, LoadedSources, SourceOrigin};
use source_text::{SourceId, SourceSpan};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use syntax_trees::{SyntaxTrees, item::Item};

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct SourceSyntax {
    pub(crate) trees: SyntaxTrees,
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
    build_machine: bool,
    build_data: bool,
}
impl SourceItems {
    pub fn declares_build_machine(&self) -> bool {
        self.build_machine
    }
    pub fn declares_build_data(&self) -> bool {
        self.build_data
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImport {
    pub ordinal: usize,
    pub members: Vec<String>,
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
                            Item::Use(import) => Some(SourceImport {
                                ordinal: unit.import_root_range.start + offset,
                                members: syntax
                                    .items
                                    .identifier_path_members(import.path)
                                    .iter()
                                    .map(|name| name.as_str(syntax).to_owned())
                                    .collect(),
                            }),
                            _ => None,
                        })
                        .collect();
                    let mut root_items = SourceItems {
                        handles: unit.root_items.clone(),
                        ..Default::default()
                    };
                    for handle in &unit.root_items {
                        match syntax.root_item(*handle) {
                            Item::Machine(machine) if machine.name.as_str(syntax) == "build" => {
                                root_items.build_machine = true
                            }
                            Item::Data(data) if data.name.as_str(syntax) == "Build" => {
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
    unit: syntax_trees::ParsedUnit,
}

pub struct SourceDeclarations {
    pub module: Vec<String>,
    pub declares_operator_family: bool,
    pub declared: Vec<String>,
}

impl RetainedSourceParse {
    pub fn declarations(&self, path: &Path) -> Result<SourceDeclarations, Vec<Diagnostic>> {
        let syntax = &self.unit.syntax;
        let mut module = None;
        let mut declares_operator_family = false;
        let mut declared = Vec::new();
        for item in self.unit.root_items() {
            declared.extend(declared_names(syntax, syntax.root_item(*item)));
            if matches!(syntax.root_item(*item), Item::Machine(machine) if machine.spelling.is_some())
            {
                declares_operator_family = true;
            }
            if let Item::Module(declaration) = syntax.root_item(*item) {
                if module.is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "source {} declares more than one logical module",
                        path.display()
                    ))]);
                }
                module = Some(
                    syntax
                        .items
                        .identifier_path_members(declaration.path)
                        .iter()
                        .map(|name| name.as_str(syntax).to_owned())
                        .collect(),
                );
            }
        }
        Ok(SourceDeclarations {
            module: module.unwrap_or_default(),
            declares_operator_family,
            declared,
        })
    }
}

pub struct SourceInput(pub LoadedSources);
impl SourceInput {
    pub fn append(
        self,
        syntax: &mut SourceSyntax,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ParsedSourceBatch, Vec<Diagnostic>> {
        let parsed =
            crate::source_parsing::SourceInput(self.0).append(&mut syntax.trees, observer)?;
        Ok(ParsedSourceBatch::project(parsed, &syntax.trees))
    }
    pub fn into_parsed_units(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Vec<RetainedSourceParse>, Vec<Diagnostic>> {
        crate::source_parsing::SourceInput(self.0)
            .into_parsed_units(observer)
            .map(|units| {
                units
                    .into_iter()
                    .map(|unit| RetainedSourceParse { unit })
                    .collect()
            })
    }
    pub fn append_reusing<'unit>(
        self,
        syntax: &mut SourceSyntax,
        observer: &mut impl PsiCompilationObserver,
        mut retained: impl FnMut(&LoadedSource) -> Option<&'unit RetainedSourceParse>,
    ) -> Result<ParsedSourceBatch, Vec<Diagnostic>> {
        let parsed = crate::source_parsing::SourceInput(self.0).append_reusing(
            &mut syntax.trees,
            observer,
            |source| retained(source).map(|parse| &parse.unit),
        )?;
        Ok(ParsedSourceBatch::project(parsed, &syntax.trees))
    }
}

/// The names an import path can start with to select `item`: its own name,
/// and for a machine attached to a data type, that type.
fn declared_names(syntax: &SyntaxTrees, item: &Item) -> Vec<String> {
    let first = |name: &syntax_trees::identifier::Identifier| {
        let text = name.as_str(syntax);
        text.split("::").next().unwrap_or(text).to_owned()
    };
    let path = |members| {
        syntax
            .items
            .identifier_path_members(members)
            .first()
            .map(first)
    };
    match item {
        Item::Capability(definition) => vec![first(&definition.name)],
        Item::Conformance(conformance) => conformance.alias.iter().map(first).collect(),
        Item::Const(definition) => vec![first(&definition.name)],
        Item::Data(definition) => vec![first(&definition.name)],
        Item::Domain(definition) => vec![first(&definition.name)],
        Item::Measure(definition) => path(definition.name).into_iter().collect(),
        Item::Operator(definition) => path(definition.name).into_iter().collect(),
        Item::Proposition(definition) => vec![first(&definition.name)],
        Item::Machine(machine) => match &machine.attached_data {
            Some(owner) => vec![first(owner)],
            None => vec![first(&machine.name)],
        },
        Item::Trait(definition) => vec![first(&definition.name)],
        Item::TypeAlias(definition) => vec![first(&definition.name)],
        Item::Module(_) | Item::Package(_) | Item::Use(_) => Vec::new(),
    }
}
