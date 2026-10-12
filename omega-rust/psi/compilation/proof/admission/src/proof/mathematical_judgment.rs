//! Original kernel judgment retained by successful bounded admission.
//!
//! Declaration indices belong to this judgment's own signature. They must
//! never be interpreted as indices in an authored mathematical declaration
//! cohort. The accepted traversal moves its arena and certificate here;
//! consumers neither elaborate nor verify that same judgment again.

use std::sync::Arc;

use arena::Arena;

use super::MathematicalJudgmentReceipt;
use crate::mathematical_core::{MathematicalCertificate, TermArena};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AcceptedMathematicalAssumption {
    pub declaration_index: u32,
}

/// The exact judgment accepted by the kernel, beside its measured cost.
/// Cloning an acceptance shares the original immutable term owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedMathematicalJudgment {
    receipt: MathematicalJudgmentReceipt,
    content: Arc<MathematicalJudgmentContent>,
}

#[derive(Debug, PartialEq, Eq)]
struct MathematicalJudgmentContent {
    arena: TermArena,
    certificate: MathematicalCertificate,
    used_assumptions: Arena<AcceptedMathematicalAssumption>,
}

impl AcceptedMathematicalJudgment {
    pub(crate) fn from_accepted_denotation(
        receipt: MathematicalJudgmentReceipt,
        arena: TermArena,
        certificate: MathematicalCertificate,
        used_assumptions: Arena<AcceptedMathematicalAssumption>,
    ) -> Self {
        Self {
            receipt,
            content: Arc::new(MathematicalJudgmentContent {
                arena,
                certificate,
                used_assumptions,
            }),
        }
    }

    pub fn receipt(&self) -> MathematicalJudgmentReceipt {
        self.receipt
    }

    pub fn arena(&self) -> &TermArena {
        &self.content.arena
    }

    pub fn certificate(&self) -> &MathematicalCertificate {
        &self.content.certificate
    }

    /// Original signature indices selected by the conversion-independent
    /// closure of this accepted judgment, including context and claim types.
    pub fn used_assumptions(&self) -> &Arena<AcceptedMathematicalAssumption> {
        &self.content.used_assumptions
    }
}
