//! Elaborate authored mathematical declarations before executable checking.
//!
//! Preparation needs both public declaration records and a kernel-checked
//! signature. This proof owner produces them together; a pipeline stage does
//! not own their elaboration, and checked-fact assembly only transfers the
//! result. The admission kernel independently judges the mathematical terms.

mod declaration_records;
mod fixed_core_vocabulary;
mod mathematical_signature;

use declaration_records::build_checked_mathematical_declarations;
use fixed_core_vocabulary::{FixedCore, FixedCoreVocabulary};
use mathematical_signature::check_mathematical_signature;

/// Checked declaration records and their associated kernel signature.
/// The signature is absent when neither mathematical declarations nor source
/// evidence demand it. Consumers retain source evidence with its signature.
#[derive(Default)]
pub struct CheckedMathematics {
    pub declarations: Vec<checked_trees::CheckedMathematicalDeclaration>,
    pub signature: Option<checked_trees::CheckedMathematicalSignature>,
    pub applications: validation::mathematical_applications::PreparedMathematicalApplications,
}

/// Elaborate every mathematical declaration into its checked record and its
/// kernel term, then check the kernel signature once. A failure keeps the
/// declaration's own diagnostic and the original declaration ordering.
pub fn check_mathematical_declarations(
    program: &dyn validation::CheckingInput,
) -> Result<CheckedMathematics, Vec<diagnostics::Diagnostic>> {
    if program.mathematical_definitions().is_empty()
        && !mathematical_signature::has_source_evidence_demand(program)
    {
        return Ok(CheckedMathematics::default());
    }
    let declarations = build_checked_mathematical_declarations(program)?;
    let (signature, applications) = check_mathematical_signature(program)?.into_checked_evidence();
    Ok(CheckedMathematics {
        declarations,
        signature: Some(signature),
        applications,
    })
}

impl CheckedMathematics {
    pub fn prepared_view(
        &self,
    ) -> Option<validation::mathematical_applications::PreparedMathematicalView<'_>> {
        Some(
            validation::mathematical_applications::PreparedMathematicalView {
                arena: &self.signature.as_ref()?.arena,
                declarations: &self.signature.as_ref()?.declarations,
                applications: &self.applications,
            },
        )
    }
}
