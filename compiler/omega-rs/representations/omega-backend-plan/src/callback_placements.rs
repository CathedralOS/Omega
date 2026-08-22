use omega_calling_conventions::BoundaryEntryPlan;
use omega_control_flow::StateKey;
use psi_checked_trees::NominalMachineUseSite;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

/// Target-owned callback recipe joined to one admitted nominal machine use.
///
/// The checked program owns the semantic admission and only retains the
/// evaluated plan fingerprint. This row carries the exact validated plan past
/// orchestration so thunk lowering never has to rediscover ABI placement from
/// names, types, or a convention oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundNominalCallbackPlacement {
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    pub boundary_calling_plan_fingerprint: u64,
    pub boundary_entry_plan: BoundaryEntryPlan,
}

/// One private inbound function that later target lowering must emit.
///
/// `placement_index` joins back to the exact validated placement row without
/// cloning it. The symbol is compiler-private planned object identity, never
/// an Omega value or a source-level address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackThunkPlan {
    pub placement_index: usize,
    pub entry_key: StateKey,
    pub private_symbol: Arc<str>,
}

/// Derive the one compiler-private object identity bound to an exact validated
/// callback placement. Planning and final emission both use this function so a
/// stored thunk symbol cannot drift from its source/selected identities or
/// evaluated boundary-plan fingerprint.
pub fn canonical_callback_private_symbol(placement: &BoundNominalCallbackPlacement) -> Arc<str> {
    let (site_kind, site_index, site_generation) = match placement.site {
        NominalMachineUseSite::Statement(handle) => {
            ('s', handle.arena_index(), handle.generation())
        }
        NominalMachineUseSite::Expression(handle) => {
            ('e', handle.arena_index(), handle.generation())
        }
    };
    Arc::from(format!(
        "__omega_callback_{site_kind}{site_index:08x}g{site_generation:08x}_a{:08x}_m{:08x}g{:08x}_e{:08x}g{:08x}_f{:016x}",
        placement.static_machine_ordinal,
        placement.selected_machine.arena_index(),
        placement.selected_machine.generation(),
        placement.selected_entry.arena_index(),
        placement.selected_entry.generation(),
        placement.boundary_calling_plan_fingerprint,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};

    fn placement() -> BoundNominalCallbackPlacement {
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty callback entry plan");
        BoundNominalCallbackPlacement {
            site: NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_parts(9, 1),
            ),
            registration_operation: SymbolHandle::from_arena_index(3),
            static_machine_ordinal: 7,
            selected_machine: SymbolHandle::from_parts(4, 2),
            selected_entry: SymbolHandle::from_parts(5, 3),
            satisfaction_trait: SymbolHandle::from_arena_index(6),
            satisfaction_requirement: SymbolHandle::from_arena_index(8),
            canonical_requirement_overload: "Handler::call".to_owned(),
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            boundary_entry_plan: validated.plan().clone(),
        }
    }

    #[test]
    fn canonical_private_symbol_binds_site_selected_handles_ordinal_and_plan() {
        let baseline = placement();
        let baseline_symbol = canonical_callback_private_symbol(&baseline);
        let mut drifts = Vec::new();

        let mut site = baseline.clone();
        site.site = NominalMachineUseSite::Expression(
            psi_checked_trees::expression::ExpressionHandle::from_parts(9, 4),
        );
        drifts.push(site);
        let mut ordinal = baseline.clone();
        ordinal.static_machine_ordinal += 1;
        drifts.push(ordinal);
        let mut machine = baseline.clone();
        machine.selected_machine = SymbolHandle::from_parts(4, 5);
        drifts.push(machine);
        let mut entry = baseline.clone();
        entry.selected_entry = SymbolHandle::from_parts(5, 6);
        drifts.push(entry);
        let mut fingerprint = baseline.clone();
        fingerprint.boundary_calling_plan_fingerprint ^= 1;
        drifts.push(fingerprint);

        assert!(baseline_symbol.starts_with("__omega_callback_e"));
        for drifted in drifts {
            assert_ne!(canonical_callback_private_symbol(&drifted), baseline_symbol);
        }
    }
}
