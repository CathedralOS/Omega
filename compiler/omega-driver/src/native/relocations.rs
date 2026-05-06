use crate::native::abi::{HostBinding, HostBindingMechanism};
use crate::native::instructions::{FunctionInstructionPlan, SelectedInstructionKind};
use crate::native::plan::NativePlan;
use crate::native::target::NativeTarget;
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationPlan {
    pub target: NativeTarget,
    pub records: Arena<RelocationRecord>,
}

impl Default for RelocationPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            records: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationRecord {
    pub function_symbol: String,
    pub selected_instruction_index: u32,
    pub symbol: String,
    pub kind: RelocationKind,
}

impl Default for RelocationRecord {
    fn default() -> Self {
        Self {
            function_symbol: String::new(),
            selected_instruction_index: 0,
            symbol: String::new(),
            kind: RelocationKind::ExternalFunctionCall,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    ExternalFunctionCall,
}

pub fn build_relocation_plan(native_plan: &NativePlan) -> RelocationPlan {
    let mut relocation_plan = RelocationPlan {
        target: native_plan.target,
        records: Arena::new(),
    };

    for (_, function) in native_plan.instructions.functions.iter() {
        collect_function_relocations(native_plan, function, &mut relocation_plan);
    }

    relocation_plan
}

fn collect_function_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return;
    };

    for (offset, instruction) in instructions.iter().enumerate() {
        let SelectedInstructionKind::HostOperation {
            capability,
            operation,
            ..
        } = &instruction.kind
        else {
            continue;
        };

        let Some(binding) = find_host_binding(native_plan, capability, operation) else {
            continue;
        };

        let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism else {
            continue;
        };

        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        relocation_plan.records.insert(RelocationRecord {
            function_symbol: function.symbol.clone(),
            selected_instruction_index,
            symbol: symbol.clone(),
            kind: RelocationKind::ExternalFunctionCall,
        });
    }
}

fn find_host_binding<'plan>(
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBinding> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| binding)
}
