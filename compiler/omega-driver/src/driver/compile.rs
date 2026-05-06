use std::path::{Path, PathBuf};

use crate::ast::item::{Item, UseItem};
use crate::diagnostics::Diagnostic;
use crate::driver::CompileOptions;
use crate::driver::artifacts::ArtifactWriter;
use crate::ir::lowering::lower_program;
use crate::lexer::{Lexer, Span};
use crate::native::plan::build_native_plan;
use crate::native::target::NativeTarget;
use crate::parser::parser::parse_file;
use crate::semantic::validation::validate_program;
use crate::source::{Resolver, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub summary: String,
    pub artifacts_dir: PathBuf,
    pub executable_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    pub summary: String,
    pub artifacts_dir: PathBuf,
}

pub fn check(options: CompileOptions) -> Result<CheckOutput, Vec<Diagnostic>> {
    let artifacts = ArtifactWriter::new(&options.build_dir, &options.root_path)
        .map_err(|diagnostic| vec![diagnostic])?;
    let loaded_program = load_program_sources(&options)?;
    debug_assert!(loaded_program.file_ranges_are_valid());
    artifacts
        .write_sources(&loaded_program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_ast(&loaded_program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("03_resolve.txt", "Omega Resolve")
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("04_types.txt", "Omega Types")
        .map_err(|diagnostic| vec![diagnostic])?;
    let program = lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_ir(&program)
        .map_err(|diagnostic| vec![diagnostic])?;
    validate_program(&program)?;
    artifacts
        .write_validation(&program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("07_graph.txt", "Omega Graph")
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("08_proof.txt", "Omega Proof")
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("09_native_plan.txt", "Omega Native Plan")
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(CheckOutput {
        artifacts_dir: artifacts.root().to_path_buf(),
        summary: format!(
            "checked {}; artifacts {}",
            options.root_path.display(),
            artifacts.root().display()
        ),
    })
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    let artifacts = ArtifactWriter::new(&options.build_dir, &options.root_path)
        .map_err(|diagnostic| vec![diagnostic])?;
    let loaded_program = load_program_sources(&options)?;
    debug_assert!(loaded_program.file_ranges_are_valid());
    artifacts
        .write_sources(&loaded_program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_ast(&loaded_program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("03_resolve.txt", "Omega Resolve")
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("04_types.txt", "Omega Types")
        .map_err(|diagnostic| vec![diagnostic])?;
    let program = lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_ir(&program)
        .map_err(|diagnostic| vec![diagnostic])?;
    validate_program(&program)?;
    artifacts
        .write_validation(&program)
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("07_graph.txt", "Omega Graph")
        .map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_placeholder("08_proof.txt", "Omega Proof")
        .map_err(|diagnostic| vec![diagnostic])?;
    let native_plan =
        build_native_plan(&program, NativeTarget::host()).map_err(|diagnostic| vec![diagnostic])?;
    artifacts
        .write_native_plan(&native_plan)
        .map_err(|diagnostic| vec![diagnostic])?;

    Err(vec![Diagnostic::error(format!(
        "native object emission is not implemented yet; artifacts {}; planned {} data layout(s), {} machine layout(s), {} control-flow machine(s), {} object section(s), entry {}.{} as `{}`",
        artifacts.root().display(),
        native_plan.layouts.data_layouts.len(),
        native_plan.layouts.machine_layouts.len(),
        native_plan.control_flow.machines.len(),
        native_plan.object.sections.len(),
        native_plan.entry_machine,
        native_plan.entry_state,
        native_plan.object.entry_symbol
    ))])
}

#[derive(Debug)]
pub(crate) struct LoadedProgram {
    pub(crate) items: Vec<Item>,
    pub(crate) files: Vec<LoadedFile>,
}

#[derive(Debug)]
pub(crate) struct LoadedFile {
    pub(crate) path: PathBuf,
    pub(crate) first_item: usize,
    pub(crate) item_count: usize,
}

impl LoadedProgram {
    fn file_ranges_are_valid(&self) -> bool {
        self.files.iter().all(|file| {
            !file.path.as_os_str().is_empty()
                && file.first_item <= self.items.len()
                && file.first_item + file.item_count <= self.items.len()
        })
    }
}

fn load_program_sources(options: &CompileOptions) -> Result<LoadedProgram, Vec<Diagnostic>> {
    let mut resolver = Resolver::default();
    let root_dir = options
        .root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut seen = Vec::<PathBuf>::new();
    let mut pending = vec![options.root_path.clone()];
    let mut items = Vec::new();
    let mut loaded_files = Vec::new();

    while let Some(path) = pending.pop() {
        let normalized = normalize_path(&path)?;

        if seen.contains(&normalized) {
            continue;
        }

        seen.push(normalized.clone());

        let file = resolver
            .load_root(&normalized)
            .map_err(|diagnostic| vec![diagnostic])?;
        let tokens = Lexer::new(&file.source).tokenize().map_err(|error| {
            vec![Diagnostic::error(format_source_span(
                file,
                error.span,
                &error.message,
            ))]
        })?;
        let ast_file = parse_file(&tokens).map_err(|error| {
            vec![Diagnostic::error(match error.span {
                Some(span) => format_source_span(file, span, &error.message),
                None => format!("{}: {}", file.path.display(), error.message),
            })]
        })?;
        let first_item = items.len();
        let item_count = ast_file.items.len();

        for item in &ast_file.items {
            if let Item::Use(use_item) = item {
                pending.push(resolve_use(&root_dir, use_item));
            }
        }

        loaded_files.push(LoadedFile {
            path: file.path.clone(),
            first_item,
            item_count,
        });
        items.extend(ast_file.items);
    }

    Ok(LoadedProgram {
        items,
        files: loaded_files,
    })
}

fn resolve_use(root_dir: &Path, use_item: &UseItem) -> PathBuf {
    let mut path = root_dir.to_path_buf();

    for segment in &use_item.path {
        path.push(segment);
    }

    path.set_extension("omg");
    path
}

fn normalize_path(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to resolve {}: {error}",
            path.display()
        ))]
    })
}

fn format_source_span(file: &SourceFile, span: Span, message: &str) -> String {
    let start = file.position_at(span.start);
    let end = file.position_at(span.end);

    format!(
        "{}:{}:{}-{}:{}: {}",
        file.path.display(),
        start.line,
        start.column,
        end.line,
        end.column,
        message
    )
}
