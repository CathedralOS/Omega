use psi_access_plans::FieldAccess;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RetainedFieldIdentity {
    Numbered(u64),
    Positional(String),
}

fn retained_field_identity(name: &str, identity: Option<u64>) -> RetainedFieldIdentity {
    match identity {
        Some(identity) => RetainedFieldIdentity::Numbered(identity),
        None => RetainedFieldIdentity::Positional(name.to_owned()),
    }
}

fn expected_accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
    match access {
        FieldAccess::Inaccessible | FieldAccess::Atomic { .. } => Vec::new(),
        FieldAccess::Stable { read, write, .. } => {
            let mut operations = Vec::new();
            if *read {
                operations.push("read");
            }
            if *write {
                operations.push("write");
            }
            operations
        }
        FieldAccess::External { read, write, .. } => {
            let mut operations = Vec::new();
            match read {
                psi_access_plans::ExternalRead::None => {}
                psi_access_plans::ExternalRead::Read => operations.push("read"),
                psi_access_plans::ExternalRead::Take => operations.push("take"),
            }
            if *write {
                operations.push("write");
            }
            operations
        }
    }
}

/// Independently replay the compiler-derived nominal and stable-member joins
/// before any placed accessor is accepted as an ordinary typed operation.
pub(crate) fn validate_plans(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for view in &program.placed_view_plans {
        let Some(view_data) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == view.data_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` no longer names its exact synthesized data identity",
                view.data_name
            )));
            continue;
        };
        let Some(schema) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == view.schema_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` no longer names its exact source schema identity",
                view.data_name
            )));
            continue;
        };
        let Some(policy) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == view.policy_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` no longer names its exact placement-policy identity",
                view.data_name
            )));
            continue;
        };
        let Some(policy_plan_machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == view.policy_plan_machine_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` no longer names its exact placement-policy plan machine",
                view.data_name
            )));
            continue;
        };
        if policy_plan_machine
            .attached_data
            .as_ref()
            .is_none_or(|attached| attached.as_str() != policy.name.as_str())
            || policy_plan_machine.name.as_str() != format!("{}::plan", policy.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` changed its exact placement-policy binding",
                view.data_name
            )));
            continue;
        }
        let accessible_field_count = view
            .placement
            .access()
            .plan()
            .entries()
            .iter()
            .filter(|entry| !matches!(entry.access(), FieldAccess::Inaccessible))
            .count();
        if view.fields.len() != accessible_field_count
            || program.data_members(view_data).len() != accessible_field_count
            || program
                .data_members(view_data)
                .iter()
                .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
        {
            diagnostics.push(Diagnostic::error(format!(
                "placed view `{}` changed its exact accessible field inventory",
                view.data_name
            )));
            continue;
        }
        let mut field_symbols = Vec::with_capacity(view.fields.len());
        let mut accessor_data_symbols = Vec::with_capacity(view.fields.len());
        for field in &view.fields {
            if field_symbols.contains(&field.field_symbol)
                || accessor_data_symbols.contains(&field.accessor_data_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` repeats field or accessor identity for `{}`",
                    view.data_name, field.field_name
                )));
                continue;
            }
            field_symbols.push(field.field_symbol);
            accessor_data_symbols.push(field.accessor_data_symbol);

            let Some(schema_field) = program
                .data_members(schema)
                .iter()
                .filter_map(|member| match member {
                    psi_typed_trees::data::DataMember::Field(field) => Some(field),
                    psi_typed_trees::data::DataMember::Variant(_) => None,
                })
                .find(|candidate| candidate.symbol == field.field_symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` no longer names its exact source field identity",
                    view.data_name, field.field_name
                )));
                continue;
            };
            if schema_field.identity != field.member_identity
                || (field.member_identity.is_none()
                    && schema_field.name.as_str() != field.field_name)
                || schema_field.type_reference != field.value_type
            {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` changed its exact source member binding",
                    view.data_name, field.field_name
                )));
                continue;
            }

            let field_identity = retained_field_identity(&field.field_name, field.member_identity);
            let exact_layout_entry = view.placement.layout().entries.iter().find(|entry| {
                retained_field_identity(&entry.field, entry.member_identity) == field_identity
            });
            if exact_layout_entry.is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` changed its retained layout member identity",
                    view.data_name, field.field_name
                )));
                continue;
            }

            let mut canonical_layout_identities = view
                .placement
                .layout()
                .entries
                .iter()
                .map(|entry| retained_field_identity(&entry.field, entry.member_identity))
                .collect::<Vec<_>>();
            canonical_layout_identities.sort();
            canonical_layout_identities.dedup();
            let exact_access = canonical_layout_identities
                .iter()
                .position(|identity| identity == &field_identity)
                .and_then(|slot| u32::try_from(slot).ok())
                .and_then(|slot| {
                    view.placement
                        .access()
                        .plan()
                        .entries()
                        .iter()
                        .find(|entry| entry.key().slot() == slot)
                });
            if !exact_access.is_some_and(|entry| entry.access() == &field.access) {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` changed its admitted access decision",
                    view.data_name, field.field_name
                )));
                continue;
            }

            let exact_view_field = program
                .data_members(view_data)
                .iter()
                .filter_map(|member| match member {
                    psi_typed_trees::data::DataMember::Field(field) => Some(field),
                    psi_typed_trees::data::DataMember::Variant(_) => None,
                })
                .find(|candidate| {
                    candidate.identity == field.member_identity
                        && (field.member_identity.is_some()
                            || candidate.name.as_str() == field.field_name)
                });
            if !exact_view_field
                .is_some_and(|candidate| candidate.type_reference == field.accessor_type)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` changed its exact synthesized accessor binding",
                    view.data_name, field.field_name
                )));
            }

            let has_accessor_data = program
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == field.accessor_data_symbol);
            if !has_accessor_data {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` no longer names its exact generated accessor data identity",
                    view.data_name, field.field_name
                )));
                continue;
            }
            if exact_view_field.is_some() {
                let type_symbol = program
                    .type_reference_table
                    .type_symbol(field.accessor_type);
                if (type_symbol.is_valid() && type_symbol != field.accessor_data_symbol)
                    || (!type_symbol.is_valid()
                        && !matches!(field.access, FieldAccess::Atomic { .. }))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "placed view `{}` field `{}` changed its exact generated accessor data binding",
                        view.data_name, field.field_name
                    )));
                    continue;
                }
            }

            let expected_operations = expected_accessor_operations(&field.access);
            if field
                .accessor_targets
                .iter()
                .map(|target| target.operation.as_str())
                .ne(expected_operations.iter().copied())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "placed view `{}` field `{}` changed its generated accessor operation set",
                    view.data_name, field.field_name
                )));
                continue;
            }
            for target in &field.accessor_targets {
                let Some(machine) = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == target.machine_symbol)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "placed view `{}` field `{}` operation `{}` no longer names its exact generated accessor machine",
                        view.data_name, field.field_name, target.operation
                    )));
                    continue;
                };
                let exact_machine = machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| attached.as_str() == field.accessor_name)
                    && machine
                        .name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .is_some_and(|operation| operation == target.operation);
                let states = program.machine_states(machine);
                if !exact_machine
                    || !matches!(states, [state] if state.symbol == target.state_symbol)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "placed view `{}` field `{}` operation `{}` changed its exact generated accessor target",
                        view.data_name, field.field_name, target.operation
                    )));
                }
            }
        }
    }
}
