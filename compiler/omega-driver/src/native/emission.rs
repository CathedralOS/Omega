use crate::native::plan::NativePlan;
use crate::native::target::ObjectFormat;
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub object_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub selected_instructions: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn build_emission_plan(native_plan: &NativePlan) -> EmissionPlan {
    let mut blockers = Arena::new();

    blockers.insert_many([
        blocker(
            "machine encoding",
            "selected native instructions are not encoded into target bytes yet",
        ),
        blocker(
            "relocation encoding",
            "planned relocation records are not serialized into target object format yet",
        ),
        blocker(
            "object writer",
            "ELF, Mach-O, and COFF bytes are planned but not serialized yet",
        ),
    ]);

    EmissionPlan {
        object_format: native_plan.target.object_format,
        entry_symbol: native_plan.object.entry_symbol.clone(),
        sections: native_plan.object.sections.len(),
        symbols: native_plan.object.symbols.len(),
        host_bindings: native_plan.host_abi.bindings.len(),
        host_calls: native_plan.host_calls.calls.len(),
        selected_instructions: native_plan.instructions.instructions.len(),
        relocations: native_plan.relocations.records.len(),
        blockers,
    }
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}
