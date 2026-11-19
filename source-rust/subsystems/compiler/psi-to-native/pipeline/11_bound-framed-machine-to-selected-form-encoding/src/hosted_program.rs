//! Encode the entry and its import stubs from the same selected physical plan.
use diagnostics::Diagnostic;
use machine_code::{hosted_entry::EncodedHostedEntry, import_code::EncodedImportProgram};
use physical_recipes::hosted_program::HostedProgramParts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedHostedProgramEntry<Context> {
    context: Context,
    program: EncodedHostedEntry,
    imports: EncodedImportProgram,
}

impl<Context> EncodedHostedProgramEntry<Context> {
    pub fn into_parts(self) -> (Context, EncodedHostedEntry, EncodedImportProgram) {
        (self.context, self.program, self.imports)
    }
    pub const fn context(&self) -> &Context {
        &self.context
    }
    pub const fn program(&self) -> &EncodedHostedEntry {
        &self.program
    }
    pub const fn imports(&self) -> &EncodedImportProgram {
        &self.imports
    }
}

pub fn encode_hosted_program_entry<Context>(
    plan: installation_binding::entries::BoundHostedEntry<Context>,
) -> Result<EncodedHostedProgramEntry<Context>, Diagnostic> {
    let HostedProgramParts {
        context,
        entry,
        imports,
    } = plan.into_parts();
    Ok(EncodedHostedProgramEntry {
        imports: crate::imports::encode_import_program(imports),
        program: crate::hosted_entry::encode_hosted_entry(entry).map_err(Diagnostic::error)?,
        context,
    })
}
