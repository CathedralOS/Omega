//! Independent identity replay for compiler-derived plan-laid value types.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_plans(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let mut data_symbols = Vec::with_capacity(program.plan_laid_layouts.len());
    for plan in &program.plan_laid_layouts {
        if data_symbols.contains(&plan.data_symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` repeats its exact synthesized data identity",
                plan.data_name
            )));
            continue;
        }
        data_symbols.push(plan.data_symbol);

        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.data_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact synthesized data identity",
                plan.data_name
            )));
            continue;
        };
        let data_field_symbols = runtime_field_symbols(program, data);
        if data_field_symbols != plan.field_symbols
            || plan.offsets.len() != plan.field_symbols.len()
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact synthesized field identity inventory",
                plan.data_name
            )));
            continue;
        }

        let Some(schema) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.schema_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact source schema identity",
                plan.data_name
            )));
            continue;
        };
        if runtime_field_symbols(program, schema) != plan.schema_field_symbols
            || plan.schema_field_symbols.len() != plan.field_symbols.len()
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact source schema field identity inventory",
                plan.data_name
            )));
            continue;
        }

        let Some(policy) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.policy_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact nominal policy identity",
                plan.data_name
            )));
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.policy_plan_machine_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact policy plan machine",
                plan.data_name
            )));
            continue;
        };
        if machine
            .attached_data
            .as_ref()
            .is_none_or(|attached| attached.as_str() != policy.name.as_str())
            || machine.name.as_str() != format!("{}::plan", policy.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact nominal policy binding",
                plan.data_name
            )));
        }
    }
}

fn runtime_field_symbols(
    program: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
) -> Vec<psi_symbols::SymbolHandle> {
    program
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field.symbol)
            }
            psi_typed_trees::data::DataMember::Field(_)
            | psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect()
}
