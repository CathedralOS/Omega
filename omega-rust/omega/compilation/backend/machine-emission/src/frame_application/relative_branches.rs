//! Reuse the retained successor proof at the function's final byte coordinates.

use super::{
    FunctionFragmentFrameApplicationError as Error, StagedFunctionFragmentFrameApplication,
};
use machine_code::{FunctionFragmentBranchEvidence as Branch, FunctionFragmentInstructionSpan};
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::MachineId;

impl StagedFunctionFragmentFrameApplication {
    /// Check the complete relative-branch row roster for one retained function.
    /// Copied candidates are observations: success grants no executable backing
    /// or entry authority. Source successors and final block offsets come from
    /// this stage's immutable input and output, never from the candidate rows.
    pub fn validate_relative_branch_rows<'a>(
        &self,
        machine: MachineId,
        candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
    ) -> Result<Vec<SelectedInstructionId>, Error> {
        let source = self
            .source()
            .fragments()
            .function(machine)
            .ok_or(Error::MissingFunction(machine))?;
        let current = self
            .fragments()
            .function(machine)
            .ok_or(Error::MissingFunction(machine))?;
        let mut offsets: Vec<_> = current
            .blocks
            .iter()
            .map(|block| (block.block, block.offset))
            .collect();
        offsets.sort_unstable_by_key(|row| row.0);
        let mut source_rows = source.instructions().filter(|row| row.branch.is_some());
        let mut candidates = candidates.into_iter();
        let mut checked = Vec::new();
        for retained in current.instructions().filter(|row| row.branch.is_some()) {
            let candidate = candidates
                .next()
                .ok_or(Error::SourceShapeMismatch(machine))?;
            if candidate.instruction != retained.instruction
                || candidate.alternative != retained.alternative
                || candidate.offset != retained.offset
                || candidate.bytes.len() != retained.bytes.len()
                || candidate.provenance != retained.provenance
                || candidate.control != retained.control
                || candidate.internal_machine_fixup != retained.internal_machine_fixup
                || candidate.normalized_foreign_call_fixup != retained.normalized_foreign_call_fixup
                || candidate.image_address_fixup != retained.image_address_fixup
            {
                return Err(Error::ArtifactMismatch);
            }
            let source_row = source_rows
                .next()
                .ok_or(Error::SourceShapeMismatch(machine))?;
            if source_row.instruction != retained.instruction {
                return Err(Error::SourceShapeMismatch(machine));
            }
            let branch = candidate.branch.as_deref().ok_or(Error::ArtifactMismatch)?;
            let inside = |offset| offset < current.byte_count;
            let targets_inside = match branch {
                Branch::Jump(row) => inside(row.target_offset),
                Branch::Conditional(row) => {
                    inside(row.when_taken_offset) && inside(row.when_fallthrough_offset)
                }
            };
            if !targets_inside {
                return Err(Error::ArtifactMismatch);
            }
            super::insertion::validation_branch::validate(
                source_row,
                candidate,
                self.fragments().target.architecture,
                self.source().source().register_environment().physical(),
                |block| {
                    offsets
                        .binary_search_by_key(&block, |row| row.0)
                        .ok()
                        .map(|index| offsets[index].1)
                },
            )
            .map_err(Error::from)?;
            checked.push(candidate.instruction);
        }
        if candidates.next().is_some() || source_rows.next().is_some() {
            return Err(Error::SourceShapeMismatch(machine));
        }
        checked.sort_unstable();
        Ok(checked)
    }
}
