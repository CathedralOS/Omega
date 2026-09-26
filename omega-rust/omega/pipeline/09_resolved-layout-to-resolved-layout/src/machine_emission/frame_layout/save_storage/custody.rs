use super::{
    NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStorageReceipt,
    non_authoritative_callee_save_storage_identity,
};

pub(super) fn seal(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageReceipt {
    NonAuthoritativeCalleeSaveStorageReceipt {
        identity: non_authoritative_callee_save_storage_identity(plan),
        callee_saved_requirements: plan.callee_saved_requirements,
        register_environment: plan.register_environment,
        physical_register_model: plan.physical_register_model,
        preservation_storage_catalog: plan.preservation_storage_catalog,
        target: plan.target,
        abi: plan.abi,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        modified_function_count: plan
            .functions
            .iter()
            .filter(|function| !function.slots.is_empty())
            .count(),
        slot_count: plan
            .functions
            .iter()
            .map(|function| function.slots.len())
            .sum(),
        modified_unit_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.slots)
            .map(|slot| slot.modified_units.len())
            .sum(),
        witness_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.slots)
            .flat_map(|slot| &slot.modified_units)
            .map(|requirement| requirement.witnesses.len())
            .sum(),
        max_abstract_area_bytes: plan
            .functions
            .iter()
            .map(|function| function.abstract_area_bytes)
            .max()
            .unwrap_or(0),
        max_abstract_area_alignment: plan
            .functions
            .iter()
            .map(|function| function.abstract_area_alignment)
            .max()
            .unwrap_or(1),
    }
}
