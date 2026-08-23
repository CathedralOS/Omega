//! Exact-identity relocations for compiler-private direct calls.

use super::context::InstructionRelocationContext;
use omega_control_flow::MachineFunctionIdentity;
use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord,
    SectionKind, object_function_symbol,
};
use omega_target::Architecture;
use omega_target_operations::SelectedInstructionKind;
use psi_diagnostics::Diagnostic;

pub(super) fn collect_internal_call_relocation(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> Result<bool, Diagnostic> {
    let SelectedInstructionKind::CallInternalFunction { target } = instruction else {
        return Ok(false);
    };
    let (offset, byte_width, kind, symbol_handle) = internal_call_relocation_spec(
        context.input.object,
        *target,
        context.input.target.architecture,
        context.selected_text_offset,
        context.selected_text_width,
    )?;
    context.relocation_plan.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: context.function_symbol_handle,
            selected_instruction_index: context.selected_instruction_index,
        },
        section: SectionKind::Text,
        offset,
        byte_width,
        symbol_handle,
        addend: 0,
        kind,
    });
    Ok(true)
}

fn internal_call_relocation_spec(
    object: &ObjectPlan,
    target: MachineFunctionIdentity,
    architecture: Architecture,
    instruction_offset: usize,
    instruction_width: usize,
) -> Result<(usize, usize, RelocationKind, ObjectSymbolHandle), Diagnostic> {
    let expected_width = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::internal_function_call_width(),
        Architecture::Aarch64 => omega_isa_aarch64::internal_function_call_width(),
    };
    if instruction_width != expected_width {
        return Err(Diagnostic::error(format!(
            "internal direct call retained {instruction_width} encoded byte(s), expected {expected_width} for {architecture:?}",
        )));
    }
    let (symbol_handle, _) = object_function_symbol(object, target).ok_or_else(|| {
        Diagnostic::error(format!(
            "internal direct call target {target:?} has no exact object text symbol",
        ))
    })?;
    Ok(match architecture {
        Architecture::X86_64 => (
            instruction_offset + 1,
            4,
            RelocationKind::X86_64Relative32,
            symbol_handle,
        ),
        Architecture::Aarch64 => (
            instruction_offset,
            4,
            RelocationKind::Aarch64Branch26,
            symbol_handle,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_control_flow::StateKey;
    use omega_object_file::{FunctionSymbolPlan, SymbolKind, SymbolPlan, SymbolSection};
    use omega_target::NativeTarget;
    use psi_symbols::SymbolHandle;

    fn source_identity(state: u32) -> MachineFunctionIdentity {
        MachineFunctionIdentity::source(StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        })
    }

    fn object_with_function(identity: MachineFunctionIdentity) -> (ObjectPlan, ObjectSymbolHandle) {
        let mut object = ObjectPlan::with_capacities(NativeTarget::linux_x64(), 0, 1, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "__omega_exact_callee".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 16,
            size: 4,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object
            .layout
            .function_symbols
            .insert(FunctionSymbolPlan { identity, symbol });
        (object, symbol)
    }

    #[test]
    fn internal_calls_retain_exact_target_and_architecture_specific_relocation_sites() {
        let identity = source_identity(2);
        let (object, symbol) = object_with_function(identity);

        assert_eq!(
            internal_call_relocation_spec(&object, identity, Architecture::X86_64, 7, 5)
                .expect("x86 direct-call relocation"),
            (8, 4, RelocationKind::X86_64Relative32, symbol)
        );
        assert_eq!(
            internal_call_relocation_spec(&object, identity, Architecture::Aarch64, 12, 4)
                .expect("AArch64 direct-call relocation"),
            (12, 4, RelocationKind::Aarch64Branch26, symbol)
        );
    }

    #[test]
    fn internal_call_relocation_rejects_missing_duplicate_and_wrong_width_targets() {
        let identity = source_identity(2);
        let other = source_identity(3);
        let (mut object, symbol) = object_with_function(identity);

        let missing = internal_call_relocation_spec(&object, other, Architecture::X86_64, 0, 5)
            .expect_err("a nearby source identity must not satisfy the call");
        assert!(missing.message.contains("no exact object text symbol"));

        object
            .layout
            .function_symbols
            .insert(FunctionSymbolPlan { identity, symbol });
        let duplicate =
            internal_call_relocation_spec(&object, identity, Architecture::X86_64, 0, 5)
                .expect_err("duplicate exact identity bindings must reject");
        assert!(duplicate.message.contains("no exact object text symbol"));

        let (object, _) = object_with_function(identity);
        let width = internal_call_relocation_spec(&object, identity, Architecture::Aarch64, 0, 5)
            .expect_err("wrong architecture width must reject");
        assert!(width.message.contains("expected 4"));
    }
}
