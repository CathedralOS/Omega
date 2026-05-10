use crate::pipeline::phase_products::ParsedSources;
use crate::pipeline::source_file::SourceFile;
use crate::source::SourceMap;
use omega_core::diagnostics::Diagnostic;

#[derive(Default)]
pub struct SourceStorage {
    pub files: Vec<SourceFile>,
    pub sources: SourceMap,
}

impl SourceStorage {
    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        let _ = parsed;
        todo!("append parsed source files into compiler-owned storage")
    }
}
