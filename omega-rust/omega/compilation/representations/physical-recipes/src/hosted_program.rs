//! A hosted entry's chosen instructions, import stubs and matching custody.

use crate::{PhysicalRecipe, hosted_entry::HostedEntryProgram, imports::PhysicalImportProgram};

pub type HostedProgram<Origin, Context> =
    PhysicalRecipe<Origin, HostedProgramParts<Origin, Context>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedProgramParts<Origin, Context> {
    pub context: Context,
    pub entry: HostedEntryProgram<Origin>,
    pub imports: PhysicalImportProgram<Origin>,
}

impl<Origin, Context> HostedProgram<Origin, Context> {
    pub fn into_parts(self) -> HostedProgramParts<Origin, Context> {
        self.instructions
    }
}
