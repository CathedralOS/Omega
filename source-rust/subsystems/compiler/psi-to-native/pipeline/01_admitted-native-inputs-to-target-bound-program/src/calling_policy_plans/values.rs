//! Input and result shapes for an executed native calling policy.
use build_model::values::BuildTimeValue;
use semantic_vocabulary::StructuralTypeId;
use terminal_interpreter::{TerminalInstance, TerminalProgram, TerminalStructuralContents};

pub(super) fn signature_contents(
    program: &TerminalProgram<'_>,
    structural_type: StructuralTypeId,
    signature: &BuildTimeValue,
) -> Result<TerminalStructuralContents, String> {
    build_value_projection::input_contents(program, 0, structural_type, signature)
}

pub(super) fn plan_value(
    instance: &TerminalInstance<'_, '_>,
    structural_type: StructuralTypeId,
) -> Result<BuildTimeValue, String> {
    build_value_projection::result_value(instance, structural_type, &[])
}

#[cfg(test)]
#[path = "values/tests.rs"]
mod tests;
