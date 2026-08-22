use crate::RelocationPlanningInput;
use crate::instruction_records::collect_instruction_relocations;
use crate::lookups::SelectedInstructionTextLayouts;
use omega_object_file::{RelocationPlan, object_function_symbol};
use omega_target_operations::FunctionInstructionPlan;
use psi_diagnostics::Diagnostic;

pub fn build_relocation_plan(
    input: RelocationPlanningInput<'_>,
) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan::with_record_capacity(
        input.target,
        input.instructions.code.instructions.len(),
    );
    let selected_instruction_text_layouts = SelectedInstructionTextLayouts::collect(input);

    for (_, function) in input.instructions.code.functions.iter() {
        collect_function_relocations(
            input,
            function,
            &selected_instruction_text_layouts,
            &mut relocation_plan,
        )?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    input: RelocationPlanningInput<'_>,
    function: &FunctionInstructionPlan,
    selected_instruction_text_layouts: &SelectedInstructionTextLayouts,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let Some(instructions) = input
        .instructions
        .code
        .instructions
        .span(function.instructions)
    else {
        return Ok(());
    };
    let function_symbol_handle = exact_function_symbol(input.object, function)?;

    for (offset, instruction) in instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        let selected_text_offset =
            selected_instruction_text_layouts.offset(function, selected_instruction_index)?;
        let selected_text_width =
            selected_instruction_text_layouts.width(selected_instruction_index);

        collect_instruction_relocations(
            input,
            function_symbol_handle,
            selected_instruction_index,
            selected_text_offset,
            selected_text_width,
            instruction,
            relocation_plan,
        )?;
    }

    Ok(())
}

fn exact_function_symbol(
    object: &omega_object_file::ObjectPlan,
    function: &FunctionInstructionPlan,
) -> Result<omega_object_file::ObjectSymbolHandle, Diagnostic> {
    object_function_symbol(object, function.identity)
        .map(|(handle, _)| handle)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "cannot plan relocations for `{}`: function identity {:?} has no exact object text symbol",
                function.symbol, function.identity
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::exact_function_symbol;
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_object_file::{
        FunctionSymbolPlan, ObjectPlan, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::FunctionInstructionPlan;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn source_key(state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    #[test]
    fn relocation_origin_resolves_only_the_exact_function_identity() {
        let source_identity = MachineFunctionIdentity::source(source_key(2));
        let wrong_identity = MachineFunctionIdentity::source(source_key(3));
        let mut object = ObjectPlan::with_capacities(NativeTarget::host(), 0, 1, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::private_function_symbol_name(source_identity)
                .expect("canonical private source name"),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 8,
            size: 4,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: source_identity,
            symbol,
        });
        let function = FunctionInstructionPlan {
            // Deliberately equal to an unrelated external spelling: relocation
            // origin must not recover linkage by this name.
            symbol: Arc::from("_write"),
            identity: source_identity,
            instructions: Default::default(),
        };

        assert_eq!(
            exact_function_symbol(&object, &function).expect("exact identity linkage"),
            symbol
        );

        let redirected = FunctionInstructionPlan {
            identity: wrong_identity,
            ..function.clone()
        };
        let error = exact_function_symbol(&object, &redirected)
            .expect_err("matching source spelling cannot redirect function identity");
        assert!(error.message.contains("has no exact object text symbol"));

        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: source_identity,
            symbol,
        });
        let error = exact_function_symbol(&object, &function)
            .expect_err("duplicate identity linkage must fail closed");
        assert!(error.message.contains("has no exact object text symbol"));
    }
}
