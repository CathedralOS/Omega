use omega_register_model::RegisterConstraintKey;

use super::MachineSemanticKind;
use crate::SelectedConstraintKeys;

impl SelectedConstraintKeys {
    pub fn in_identity_order(self) -> Vec<RegisterConstraintKey> {
        self.structural_unit_call
            .into_iter()
            .chain([
                self.materialize_i64,
                self.copy_i64,
                self.add_i64,
                self.add_i64_immediate,
                self.subtract_i64,
                self.subtract_i64_immediate,
                self.compare_i64_zero,
                self.conditional_branch,
                self.return_i64,
                self.return_unit,
            ])
            .collect()
    }

    pub const fn for_semantic(self, semantic: MachineSemanticKind) -> RegisterConstraintKey {
        match semantic {
            MachineSemanticKind::CompareI64Zero => self.compare_i64_zero,
            MachineSemanticKind::MaterializeI64 => self.materialize_i64,
            MachineSemanticKind::CopyI64 => self.copy_i64,
            MachineSemanticKind::ExactAddI64 => self.add_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            MachineSemanticKind::ExactSubtractI64Immediate => self.subtract_i64_immediate,
            MachineSemanticKind::ConditionalBranchNonZero => self.conditional_branch,
            MachineSemanticKind::ReturnI64 => self.return_i64,
            MachineSemanticKind::ReturnUnit => self.return_unit,
        }
    }
}
