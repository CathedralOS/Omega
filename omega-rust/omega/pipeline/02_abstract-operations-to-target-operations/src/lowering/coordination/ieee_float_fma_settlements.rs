//! IEEE float fused-multiply-add settlement admission beside the lowering
//! coordinator: each admitted settlement names a real abstract FMA operation
//! and rejoins the exact slot, requirement identity, provider, and
//! compiler-intrinsic plan row before lowering accepts it.

use crate::LoweringError;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use semantic_vocabulary::IeeeFloatFormat;
use std::collections::{BTreeMap, BTreeSet};
use target::{Architecture, NativeTarget};

pub(super) fn validate_ieee_float_fma_settlements(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<(), LoweringError> {
    let abstract_fma = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                format,
                ..
            } => Some((*psi_operation, *format)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut settled_ieee_float_fma = BTreeSet::new();
    for settlement in settlements {
        if settled_ieee_float_fma.contains(&settlement.terminal_operation) {
            return Err(LoweringError::DuplicateIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        }
        let Some(format) = abstract_fma.get(&settlement.terminal_operation) else {
            return Err(LoweringError::UnknownIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        };
        let expected_slot = match format {
            IeeeFloatFormat::Binary32 => target::X86ScalarFmaSlot::Binary32,
            IeeeFloatFormat::Binary64 => target::X86ScalarFmaSlot::Binary64,
        };
        let expected_selected_requirement = expected_slot.selected_plan_requirement_identity();
        let provider = settlement.provider;
        let plan = settlement.provider_plan;
        if settlement.format != *format
            || settlement.slot != expected_slot
            || target.architecture != Architecture::X86_64
            || !provider.has_canonical_identity()
            || provider.profile().native_target() != target
            || !provider.admits(provider.requirement(), settlement.slot)
            || plan.target != provider.profile().target_name()
            || !matches!(plan.rows.as_slice(), [row]
                if row.requirement_identity == expected_selected_requirement
                    && matches!(row.binding,
                        effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }))
        {
            return Err(LoweringError::InvalidIeeeFloatFmaSettlement(
                settlement.terminal_operation,
            ));
        }
        settled_ieee_float_fma.insert(settlement.terminal_operation);
    }
    if let Some(missing) = abstract_fma
        .keys()
        .find(|operation| !settled_ieee_float_fma.contains(operation))
    {
        return Err(LoweringError::MissingIeeeFloatFmaSettlement(*missing));
    }
    Ok(())
}
