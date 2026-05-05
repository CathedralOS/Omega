use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::item::{Item, UseItem};
use crate::backend;
use crate::diagnostics::Diagnostic;
use crate::driver::CompileOptions;
use crate::lexer::tokenize;
use crate::parser::parser::parse_file;
use crate::source::{Resolver, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub summary: String,
    pub executable_path: PathBuf,
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    let mut resolver = Resolver::default();
    let root_dir = options
        .root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let files = load_reachable_files(&mut resolver, &options.root_path, &root_dir)?;
    let mut items = Vec::new();

    for file in files {
        let tokens = tokenize(&file.source);
        let ast_file = parse_file(&tokens).map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {}",
                file.path.display(),
                error.message
            ))]
        })?;

        items.extend(ast_file.items);
    }

    let c_source = backend::c::emit_c(&items).map_err(|diagnostic| vec![diagnostic])?;
    let output_dir = PathBuf::from("target/omega");
    std::fs::create_dir_all(&output_dir).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to create target/omega: {error}"
        ))]
    })?;

    let stem = options
        .root_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("omega_program");
    let c_path = output_dir.join(format!("{stem}.c"));
    let executable_path = output_dir.join(stem);

    std::fs::write(&c_path, c_source).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to write {}: {error}",
            c_path.display()
        ))]
    })?;

    let status = Command::new("cc")
        .arg(&c_path)
        .arg("-o")
        .arg(&executable_path)
        .status()
        .map_err(|error| vec![Diagnostic::error(format!("failed to run cc: {error}"))])?;

    if !status.success() {
        return Err(vec![Diagnostic::error(format!(
            "cc failed while compiling {}",
            c_path.display()
        ))]);
    }

    Ok(CompileOutput {
        summary: format!("built {}", executable_path.display()),
        executable_path,
    })
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
        let tokens = tokenize(&file.source);
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
