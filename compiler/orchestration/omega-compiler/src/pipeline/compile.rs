use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::lexer::Lexer;
use crate::parser::{SyntaxFile, SyntaxKind, parse_syntax_file_with_id};
use crate::pipeline::import_queue::ImportQueue;
use crate::pipeline::options::CompileOptions;
use crate::pipeline::source_file::SourceFile;
use crate::source::{SourceMap, SourceSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    pub files: Vec<SourceFile>,
    pub sources: Arc<SourceMap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub files: Vec<SourceFile>,
    pub sources: Arc<SourceMap>,
}

pub fn check(options: CompileOptions) -> Result<CheckOutput, Vec<Diagnostic>> {
    Compiler::new(options).check()
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    Compiler::new(options).compile()
}

struct Compiler {
    options: CompileOptions,
    workers: WorkerPool,
    root_dir: PathBuf,
    imports: ImportQueue,
    sources: SourceMap,
    files: Vec<SourceFile>,
}

impl Compiler {
    fn new(options: CompileOptions) -> Self {
        let root_dir = options
            .root_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut imports = ImportQueue::default();
        imports.push(options.root_path.clone());

        Self {
            options,
            workers: WorkerPool::with_available_parallelism(),
            root_dir,
            imports,
            sources: SourceMap::default(),
            files: Vec::new(),
        }
    }

    fn check(mut self) -> Result<CheckOutput, Vec<Diagnostic>> {
        self.compile_frontend()?;
        Ok(CheckOutput {
            files: self.files,
            sources: Arc::new(self.sources),
        })
    }

    fn compile(mut self) -> Result<CompileOutput, Vec<Diagnostic>> {
        self.compile_frontend()?;
        Ok(CompileOutput {
            files: self.files,
            sources: Arc::new(self.sources),
        })
    }

    fn compile_frontend(&mut self) -> Result<(), Vec<Diagnostic>> {
        while let Some(frontier) = self.imports.pop_all() {
            let loaded = self
                .load_sources(frontier)?
                .into_iter()
                .map(|(path, source)| self.lex_and_parse(path, source))
                .collect::<Result<Vec<_>, _>>()?;

            loaded
                .iter()
                .flat_map(|file| self.discover_imports(file.syntax()))
                .try_for_each(|path| self.imports.push(self.normalize_path(&path)?));

            self.files.extend(loaded);
        }

        Ok(())
    }

    fn load_sources(&self, paths: Vec<PathBuf>) -> Result<Vec<(PathBuf, String)>, Vec<Diagnostic>> {
        let paths = Arc::new(paths);
        let loaded = self.workers.map_ordered(paths.len(), move |index| {
            let path = paths[index].clone();
            let source = std::fs::read_to_string(&path);
            (path, source)
        });

        loaded
            .into_iter()
            .map(|(path, source)| {
                source.map(|text| (path.clone(), text)).map_err(|error| {
                    vec![Diagnostic::error(format!("failed to read {}: {error}", path.display()))]
                })
            })
            .collect()
    }

    fn lex_and_parse(&mut self, path: PathBuf, source: String) -> Result<SourceFile, Vec<Diagnostic>> {
        let source_file = self.sources.add(path.clone(), source);
        let file_id = source_file.id;
        let leaked: &'static str = Box::leak(source_file.source.to_string().into_boxed_str());
        let tokens = Lexer::new(leaked).tokenize().map_err(|error| {
            vec![Diagnostic::error(self.format_span(file_id, path.as_path(), error.span, &error.message))]
        })?;
        let syntax = parse_syntax_file_with_id(file_id, &tokens).map_err(|error| {
            vec![Diagnostic::error(match error.span {
                Some(span) => self.format_span(file_id, path.as_path(), span, &error.message),
                None => format!("{}: {}", path.display(), error.message),
            })]
        })?;

        Ok(SourceFile {
            file_id,
            path,
            tokens,
            syntax,
        })
    }

    fn discover_imports<'a>(&'a self, syntax: &'a SyntaxFile) -> impl Iterator<Item = PathBuf> + 'a {
        let root = syntax.syntax.nodes.get(syntax.root);
        syntax
            .syntax
            .node_handles
            .span_or_empty(root.children)
            .iter()
            .filter(move |handle| matches!(syntax.syntax.nodes.get(**handle).kind, SyntaxKind::UseItem))
            .map(move |handle| self.join_import_path(syntax, *handle))
    }

    fn join_import_path(&self, syntax: &SyntaxFile, handle: omega_parser::SyntaxNodeHandle) -> PathBuf {
        let node = syntax.syntax.nodes.get(handle);
        let mut path = self.root_dir.clone();
        for token in syntax.syntax.tokens.span_or_empty(node.tokens).iter().skip(1) {
            match token.lexeme.as_str() {
                "::" | ";" => {}
                segment => path.push(segment),
            }
        }
        path.set_extension("omg");
        path
    }

    fn normalize_path(&self, path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
        path.canonicalize().map_err(|error| {
            vec![Diagnostic::error(format!("failed to resolve {}: {error}", path.display()))]
        })
    }

    fn format_span(&self, file_id: crate::source::FileId, path: &Path, span: omega_core::Span, message: &str) -> String {
        let file = self
            .sources
            .get(file_id)
            .expect("source file should exist for reported span");
        let span = SourceSpan::new(file.id, span);
        let start = file.position_at(span.span.start);
        let end = file.position_at(span.span.end);
        format!(
            "{}:{}:{}-{}:{}: {}",
            path.display(),
            start.line,
            start.column,
            end.line,
            end.column,
            message
        )
    }
}
