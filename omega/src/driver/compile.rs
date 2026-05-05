use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::item::{Item, UseItem};
use crate::diagnostics::Diagnostic;
use crate::driver::CompileOptions;
use crate::ir::lowering::lower_program;
use crate::lexer::Lexer;
use crate::native::plan::build_native_plan;
use crate::native::target::NativeTarget;
use crate::parser::parser::parse_file;
use crate::semantic::validation::validate_program;
use crate::source::{Resolver, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub summary: String,
    pub executable_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    pub summary: String,
}

pub fn check(options: CompileOptions) -> Result<CheckOutput, Vec<Diagnostic>> {
    let items = load_items(&options)?;
    let program = lower_program(&items).map_err(|diagnostic| vec![diagnostic])?;
    validate_program(&program)?;

    Ok(CheckOutput {
        summary: format!("checked {}", options.root_path.display()),
    })
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    let items = load_items(&options)?;
    let program = lower_program(&items).map_err(|diagnostic| vec![diagnostic])?;
    validate_program(&program)?;
    let native_plan =
        build_native_plan(&program, NativeTarget::host()).map_err(|diagnostic| vec![diagnostic])?;

    Err(vec![Diagnostic::error(format!(
        "native object emission is not implemented yet; planned {} data layout(s), {} machine layout(s), {} control-flow machine(s), {} object section(s), entry {}.{} as `{}`",
        native_plan.layouts.data_layouts.len(),
        native_plan.layouts.machine_layouts.len(),
        native_plan.control_flow.machines.len(),
        native_plan.object.sections.len(),
        native_plan.entry_machine,
        native_plan.entry_state,
        native_plan.object.entry_symbol
    ))])
}

fn load_items(options: &CompileOptions) -> Result<Vec<Item>, Vec<Diagnostic>> {
    let mut resolver = Resolver::default();
    let root_dir = options
        .root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let files = load_reachable_files(&mut resolver, &options.root_path, &root_dir)?;
    let mut items = Vec::new();

    for file in files {
        let tokens = Lexer::new(&file.source).tokenize().map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {} at {}..{}",
                file.path.display(),
                error.message,
                error.span.start,
                error.span.end
            ))]
        })?;
        let ast_file = parse_file(&tokens).map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {}",
                file.path.display(),
                error.message
            ))]
        })?;

        items.extend(ast_file.items);
    }

    Ok(items)
}

fn load_reachable_files(
    resolver: &mut Resolver,
    root_path: &Path,
    root_dir: &Path,
) -> Result<Vec<SourceFile>, Vec<Diagnostic>> {
    let mut seen = HashSet::<PathBuf>::new();
    let mut pending = vec![root_path.to_path_buf()];
    let mut files = Vec::new();

    while let Some(path) = pending.pop() {
        let normalized = path.clone();

        if !seen.insert(normalized.clone()) {
            continue;
        }

        let file = resolver
            .load_root(&normalized)
            .map_err(|diagnostic| vec![diagnostic])?;
        let tokens = Lexer::new(&file.source).tokenize().map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {} at {}..{}",
                file.path.display(),
                error.message,
                error.span.start,
                error.span.end
            ))]
        })?;
        let ast_file = parse_file(&tokens).map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {}",
                file.path.display(),
                error.message
            ))]
        })?;

        for item in &ast_file.items {
            if let Item::Use(use_item) = item {
                pending.push(resolve_use(root_dir, use_item));
            }
        }

        files.push(file);
    }

    Ok(files)
}

fn resolve_use(root_dir: &Path, use_item: &UseItem) -> PathBuf {
    let mut path = root_dir.to_path_buf();

    for segment in &use_item.path {
        path.push(segment);
    }

    path.set_extension("omg");
    path
}
