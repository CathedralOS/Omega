use std::path::{Path, PathBuf};

use crate::pipeline::phase_products::{
    own_token_stream, AssembledSyntax, BackendPlan, DiscoveredImports, EmittedProgram, LexedSource,
    LexedSources, LoadedSource, LoadedSources, ParsedSource, ParsedSources, ResolvedProgram,
    TypedProgram, ValidatedProgram,
};
use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use omega_syntax_trees::identifier::IdentifierPath;
use omega_syntax_trees::item::Item;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceId;

pub struct SourceLoader;
pub struct LexerPhase;
pub struct ParserPhase;
pub struct ImportDiscovery;
pub struct SyntaxAssembler;
pub struct Resolver;
pub struct Typechecker;
pub struct Validator;
pub struct BackendPlanner;
pub struct Emitter;
pub struct OutputWriter;

impl SourceLoader {
    pub fn load(
        &self,
        frontier: Vec<PathBuf>,
        first_source_id: usize,
    ) -> Result<LoadedSources, Vec<Diagnostic>> {
        let mut sources = Arena::new();
        let loaded = frontier
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                let source = std::fs::read_to_string(&path).map_err(|error| {
                    Diagnostic::error(format!("failed to read {}: {error}", path.display()))
                })?;

                Ok(LoadedSource {
                    source_id: SourceId(first_source_id + index),
                    path,
                    source: std::sync::Arc::from(source),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map_err(|diagnostic| vec![diagnostic])?;

        let batch = sources.insert_many(loaded);

        Ok(LoadedSources { sources, batch })
    }
}

impl LexerPhase {
    pub fn lex(&self, sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
        let mut lexed_sources = Arena::new();
        let batch = lexed_sources.insert_many(
            sources
                .sources
                .span_or_empty(sources.batch)
                .iter()
                .map(|loaded_source| {
                    let tokens = lexer::Lexer::new(loaded_source.source.as_ref())
                        .tokenize()
                        .map_err(|error| {
                            Diagnostic::error(format!(
                                "{}: {}",
                                loaded_source.path.display(),
                                error.message
                            ))
                        })?;

                    Ok(LexedSource {
                        source_id: loaded_source.source_id,
                        path: loaded_source.path.clone(),
                        source: loaded_source.source.clone(),
                        tokens: own_token_stream(&tokens),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()
                .map_err(|diagnostic| vec![diagnostic])?,
        );

        Ok(LexedSources {
            sources: lexed_sources,
            batch,
        })
    }
}

impl ParserPhase {
    pub fn parse(&self, lexed: LexedSources) -> Result<ParsedSources, Vec<Diagnostic>> {
        let mut parsed_sources = Arena::new();
        let batch = parsed_sources.insert_many(
            lexed
                .sources
                .span_or_empty(lexed.batch)
                .iter()
                .map(|lexed_source| {
                    let syntax_trees =
                        parser::parse_syntax_trees_with_id(
                            lexed_source.source_id,
                            &lexed_source.tokens,
                        )
                            .map_err(|error| {
                                Diagnostic::error(format!(
                                    "{}: {}",
                                    lexed_source.path.display(),
                                    error.message
                                ))
                            })?;

                    Ok(ParsedSource {
                        source_id: lexed_source.source_id,
                        path: lexed_source.path.clone(),
                        source: lexed_source.source.clone(),
                        syntax_trees,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()
                .map_err(|diagnostic| vec![diagnostic])?,
        );

        Ok(ParsedSources {
            sources: parsed_sources,
            batch,
        })
    }
}

impl ImportDiscovery {
    pub fn discover(
        &self,
        parsed: &ParsedSources,
        root_path: &Path,
        selected_target_name: Option<&str>,
    ) -> Result<DiscoveredImports, Vec<Diagnostic>> {
        let root_dir = root_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut imports = Vec::new();
        let mut selected_target_found = selected_target_name.is_none();

        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            for item in &parsed_source.syntax_trees.items {
                match item {
                    Item::Use(use_item) => {
                        imports.push(normalize_path(&resolve_source_path(
                            &root_dir,
                            &use_item.path,
                        ))?);
                    }
                    Item::Target(target) => {
                        let target_is_selected = selected_target_name
                            .is_none_or(|target_name| target.name.as_str() == target_name);

                        if target_is_selected {
                            selected_target_found = true;
                        } else {
                            continue;
                        }

                        if let Some(host) = &target.host {
                            if is_bundled_omega_path(&host.provider) {
                                imports.push(normalize_path(&resolve_source_path(
                                    &root_dir,
                                    &host.provider,
                                ))?);
                            }
                        }

                        for trust_policy in &target.trust_policies {
                            if is_bundled_omega_path(&trust_policy.path) {
                                imports.push(normalize_path(&resolve_source_path(
                                    &root_dir,
                                    &trust_policy.path,
                                ))?);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if !selected_target_found {
            return Err(vec![Diagnostic::error(format!(
                "target `{}` was not found in discovered source frontier",
                selected_target_name.expect("selected target should exist when missing")
            ))]);
        }

        Ok(imports)
    }
}

impl SyntaxAssembler {
    pub fn assemble(&self, _sources: &SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        todo!("assemble all parsed source files into a whole-program syntax product")
    }
}

impl Resolver {
    pub fn resolve(&self, _syntax: AssembledSyntax) -> Result<ResolvedProgram, Vec<Diagnostic>> {
        todo!("resolve symbols over assembled whole-program syntax")
    }
}

impl Typechecker {
    pub fn typecheck(&self, _resolved: ResolvedProgram) -> Result<TypedProgram, Vec<Diagnostic>> {
        todo!("typecheck resolved program")
    }
}

impl Validator {
    pub fn validate(&self, _typed: TypedProgram) -> Result<ValidatedProgram, Vec<Diagnostic>> {
        todo!("validate typed program")
    }
}

impl BackendPlanner {
    pub fn plan(&self, _validated: ValidatedProgram) -> Result<BackendPlan, Vec<Diagnostic>> {
        todo!("plan backend from validated program")
    }
}

impl Emitter {
    pub fn emit(&self, _plan: BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
        todo!("emit backend plan")
    }
}

impl OutputWriter {
    pub fn write(&self, _emitted: EmittedProgram) -> Result<(), Vec<Diagnostic>> {
        todo!("persist emitted output bytes")
    }
}

fn resolve_source_path(root_dir: &Path, source_path: &IdentifierPath) -> PathBuf {
    let mut segments = source_path.iter();
    let mut path = if is_bundled_omega_path(source_path) {
        segments.next();
        bundled_omega_root()
    } else {
        root_dir.to_path_buf()
    };

    for segment in segments {
        path.push(segment.as_str());
    }

    path.set_extension("omg");

    if path.exists() {
        return path;
    }

    path.set_extension("");
    path.join("mod.omg")
}

fn is_bundled_omega_path(path: &IdentifierPath) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str() == "omega")
}

fn bundled_omega_root() -> PathBuf {
    if let Some(path) = std::env::var_os("OMEGA_LIBRARY_ROOT") {
        return PathBuf::from(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .join("omega")
}

fn normalize_path(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to resolve {}: {error}",
            path.display()
        ))]
    })
}
