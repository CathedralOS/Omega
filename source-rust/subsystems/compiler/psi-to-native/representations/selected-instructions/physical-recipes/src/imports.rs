//! Target import recipes and their resources.
use machine_code::import_code::ImportBodyResources;
use target::{NativeTarget, NormalizedImportLocator};
pub type PhysicalImportProgram<Origin> = crate::PhysicalRecipe<Origin, ImportInstructions>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInstructions {
    pub target: NativeTarget,
    pub imports: Vec<NormalizedImportLocator>,
    pub recipe: ImportRecipe,
    pub entry_resources: machine_code::import_code::ImportBodyResources,
    pub resolver_resources: Option<machine_code::import_code::ImportBodyResources>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRecipe {
    Empty,
    X86Indirect,
    Aarch64Indirect,
    X86Elf,
    Aarch64Elf,
}

impl<Origin> PhysicalImportProgram<Origin> {
    pub fn target(&self) -> NativeTarget {
        self.instructions.target
    }
    pub fn recipe(&self) -> ImportRecipe {
        self.instructions.recipe
    }
    pub fn entry_resources(&self) -> &ImportBodyResources {
        &self.instructions.entry_resources
    }
    pub fn resolver_resources(&self) -> Option<&ImportBodyResources> {
        self.instructions.resolver_resources.as_ref()
    }
    pub fn into_imports(self) -> Vec<NormalizedImportLocator> {
        self.instructions.imports
    }
}
