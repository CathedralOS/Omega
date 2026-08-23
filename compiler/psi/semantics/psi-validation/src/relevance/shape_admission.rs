use psi_diagnostics::Diagnostic;
use psi_language_core::BindingRelevance;
use psi_language_semantics::DataSupplyMode;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn validate_supported_shapes(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for definition in program.data_definitions() {
        let erased = erased_fields(program, definition);
        if erased.is_empty() {
            continue;
        }
        let field_names = erased
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if definition.supply_mode == DataSupplyMode::BoundaryOpaque {
            diagnostics.push(unsupported(
                definition,
                &field_names,
                "boundary-opaque ABI data",
            ));
        }
        if program
            .placed_view_plans
            .iter()
            .any(|plan| plan.data_symbol == definition.symbol)
        {
            diagnostics.push(unsupported(definition, &field_names, "placed-view data"));
        }
        let has_attached_machines = program
            .machines()
            .iter()
            .any(|machine| machine.attached_data.as_ref() == Some(&definition.name));
        // Generic declarations are semantic templates, not runtime storage.
        // Their concrete synthesized uses are validated below as ordinary
        // closed definitions; unresolved uses are rejected by the dedicated
        // use-site pass. This keeps an unused schema legal without licensing a
        // generic runtime layout or method body.
        if has_attached_machines
            && program.data_type_parameters(definition).is_empty()
            && !supports_erased_attached_machine_record(program, definition)
        {
            diagnostics.push(unsupported(
                definition,
                &field_names,
                "data with attached machines",
            ));
        }
    }

    validate_unresolved_erased_generic_uses(program, diagnostics);
}

/// The attached-machine relevance slice is deliberately narrower than ordinary
/// erased data. A plain, closed record or case-bearing value can share the same
/// erased-stripped field sequence, case tag, and payload overlay between its
/// value layout and each checked attached machine. Generic templates have no
/// runtime storage and are checked at each synthesized closed use. Admitted and
/// boundary providers need additional representation or evidence rules and
/// therefore remain behind the existing fail-closed fence.
fn supports_erased_attached_machine_record(
    program: &TypedTrees,
    definition: &DataDefinition,
) -> bool {
    if definition.supply_mode != DataSupplyMode::CheckedShape
        || !program.data_type_parameters(definition).is_empty()
        || !matches!(
            DataDefinition::shape_kind_from_members(program.data_members(definition)),
            DataShapeKind::Record | DataShapeKind::Enum | DataShapeKind::Mixed
        )
        || program
            .plan_laid_layouts
            .iter()
            .any(|plan| plan.data_symbol == definition.symbol)
        || program
            .placed_view_plans
            .iter()
            .any(|plan| plan.data_symbol == definition.symbol)
        || program
            .wire_schemas()
            .iter()
            .any(|schema| schema.name == definition.name)
    {
        return false;
    }

    let attached = program
        .machines()
        .iter()
        .filter(|machine| machine.attached_data.as_ref() == Some(&definition.name));
    let mut found = false;
    for machine in attached {
        found = true;
        if !machine.supply_mode.is_checked_body()
            || !program.machine_type_parameters(machine).is_empty()
        {
            return false;
        }
    }
    found
}

fn validate_unresolved_erased_generic_uses(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for definition in program
        .data_definitions()
        .iter()
        .filter(|definition| definition.type_parameters.is_empty())
    {
        for member in program.data_members(definition) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant),
            };
            for field in fields {
                if let Some(base) = unresolved_erased_generic_base(program, field.type_reference) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` field `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        definition.name, field.name
                    )));
                }
            }
        }
    }

    for machine in program
        .machines()
        .iter()
        .filter(|machine| program.machine_type_parameters(machine).is_empty())
    {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if let Some(base) =
                    unresolved_erased_generic_base(program, parameter.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` parameter `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        machine.name, state.name, parameter.name
                    )));
                }
            }
            if let Some(base) = unresolved_erased_generic_base(program, state.return_type) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}::{}` result uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                    machine.name, state.name
                )));
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if let Some(base) = unresolved_erased_generic_base(program, local.type_reference) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` local `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        machine.name, state.name, local.name
                    )));
                }
            }
        }
    }
}

fn unsupported(definition: &DataDefinition, fields: &str, shape: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "data `{}` has erased field(s) `{fields}`, but erased-stripped runtime support for {shape} is not implemented yet",
        definition.name
    ))
}

pub(crate) fn erased_fields<'program>(
    program: &'program TypedTrees,
    definition: &'program DataDefinition,
) -> Vec<&'program DataField> {
    let mut fields = Vec::new();
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) if field.relevance == BindingRelevance::Erased => {
                fields.push(field)
            }
            DataMember::Variant(variant) => fields.extend(
                program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| field.relevance == BindingRelevance::Erased),
            ),
            DataMember::Field(_) => {}
        }
    }
    fields
}

fn unresolved_erased_generic_base(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<String> {
    if !handle.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            unresolved_erased_generic_base(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            unresolved_erased_generic_base(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            unresolved_erased_generic_base(program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let base = program.data_definitions().iter().find(|definition| {
                if base_symbol.is_valid() {
                    definition.symbol == *base_symbol
                } else {
                    definition.name == *base_name
                }
            });
            if base.is_some_and(|definition| {
                !definition.type_parameters.is_empty()
                    && !erased_fields(program, definition).is_empty()
            }) {
                return Some(base_name.as_str().to_owned());
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| unresolved_erased_generic_base(program, *argument))
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}
