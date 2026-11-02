//! Admit a selected nominal cleanup entry and root its ordinary Unit closure.
//! Signature contexts and return-action finalization are shared with members.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, LoweringError, lower_unit_closure,
};
use crate::producer_result::SourceMappedLowered;
use crate::unit::attached_unit::UnitClosureRequest;
use crate::unit::attached_unit::bodies::UnitPlans;

pub(super) fn lower_ordered_nominal_affine_unit_cleanup_machine(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<SourceMappedLowered, LoweringError> {
    // The admitted source body and shared normal-exit rows already own the
    // hook closure. Entry routing adds no shape gate or disposal reconstruction.
    let plans = UnitPlans::published(&checked.facts.flow);
    lower_unit_closure(
        identities,
        checked,
        &UnitClosureRequest {
            plans,
            entry: nominal.machine,
            unit_roots: &[nominal.machine],
            program_roots,
            exact_toolchain_sources,
            scalar_entry: false,
        },
    )
}
