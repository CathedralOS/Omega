//! A placeholder selected plan for unit tests whose records must point at a
//! plan but never read its rows. `marker` fills the Psi fingerprint, so tests
//! can build two plans that compare unequal; no compilation produces one.
use std::sync::Arc;

use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{SelectedFunctions, SelectedInstructionPlan};

pub fn placeholder_plan(marker: u8) -> Arc<SelectedInstructionPlan> {
    Arc::new(SelectedInstructionPlan {
        contracts: Default::default(),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([marker; 32]),
        },
        work_schedule: WorkScheduleIdentity::new(1).expect("nonzero schedule marker"),
        target: NativeTarget::linux_x64(),
        entry: MachineId::new(1).expect("nonzero machine"),
        functions: SelectedFunctions::default(),
    })
}
