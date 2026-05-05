use crate::diagnostics::Diagnostic;
use crate::driver::CompileOptions;
use crate::source::Resolver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub summary: String,
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    let mut resolver = Resolver::default();
    let root = resolver
        .load_root(&options.root_path)
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(CompileOutput {
        summary: format!(
            "loaded {} ({} bytes)",
            root.path.display(),
            root.source.len()
        ),
    })
}
