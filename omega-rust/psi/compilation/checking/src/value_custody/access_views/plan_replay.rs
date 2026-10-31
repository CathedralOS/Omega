use access_plans::{AccessFieldEntry, FieldAccess};
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;

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

pub(super) fn exact_access_entry_for_field<'view>(
    view: &'view typed_trees::typed_trees::AccessViewPlan,
    field: &typed_trees::typed_trees::AccessFieldPlan,
) -> Option<&'view AccessFieldEntry> {
    let field_identity = retained_field_identity(&field.field_name, field.member_identity);
    let mut canonical_layout_identities = view
        .placement
        .layout()
        .entries
        .iter()
        .map(|entry| retained_field_identity(&entry.field, entry.member_identity))
        .collect::<Vec<_>>();
    canonical_layout_identities.sort();
    canonical_layout_identities.dedup();
    canonical_layout_identities
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
        })
}

fn expected_accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
    access.operation_names()
}

/// Independently replay the compiler-derived nominal and stable-member joins
/// before any accessor is accepted as an ordinary typed operation.
pub(crate) fn validate_plans(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for view in &program.access_view_plans {
        macro_rules! view {
            ($detail:literal) => {
                diagnostics.push(Diagnostic::error(format!(
                    concat!("access view `{}` ", $detail),
                    view.data_name
                )))
            };
        }
        let Some(view_data) =
            crate::declarations::data_queries::data_definition_by_symbol(program, view.data_symbol)
        else {
            view!("no longer names its exact synthesized data identity");
            continue;
        };
        let Some(schema) = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            view.schema_symbol,
        ) else {
            view!("no longer names its exact source schema identity");
            continue;
        };
        let Some(policy) = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            view.policy_symbol,
        ) else {
            view!("no longer names its exact placement-policy identity");
            continue;
        };
        let Some(policy_plan_machine) = program.machine(view.policy_plan_machine_symbol) else {
            view!("no longer names its exact placement-policy plan machine");
            continue;
        };
        if policy_plan_machine
            .attached_data
            .as_ref()
            .is_none_or(|attached| attached.as_str() != policy.name.as_str())
            || policy_plan_machine.name.as_str() != format!("{}::plan", policy.name.as_str())
        {
            view!("changed its exact placement-policy binding");
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
                .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            view!("changed its exact accessible field inventory");
            continue;
        }
        let mut field_symbols = Vec::with_capacity(view.fields.len());
        let mut accessor_data_symbols = Vec::with_capacity(view.fields.len());
        for field in &view.fields {
            macro_rules! field {
                ($detail:literal) => {
                    diagnostics.push(Diagnostic::error(format!(
                        concat!("access view `{}` field `{}` ", $detail),
                        view.data_name, field.field_name
                    )))
                };
            }
            if field_symbols.contains(&field.field_symbol)
                || accessor_data_symbols.contains(&field.accessor_data_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "access view `{}` repeats field or accessor identity for `{}`",
                    view.data_name, field.field_name
                )));
                continue;
            }
            field_symbols.push(field.field_symbol);
            accessor_data_symbols.push(field.accessor_data_symbol);
            if matches!(field.access, FieldAccess::Region { .. })
                && let Err(diagnostic) = field.scalar_projection_leaf(program.typed_program())
            {
                diagnostics.push(diagnostic);
                continue;
            }

            let Some(schema_field) = program
                .data_members(schema)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => Some(field),
                    typed_trees::data::DataMember::Variant(_) => None,
                })
                .find(|candidate| candidate.symbol == field.field_symbol)
            else {
                field!("no longer names its exact source field identity");
                continue;
            };
            if schema_field.identity != field.member_identity
                || (field.member_identity.is_none()
                    && schema_field.name.as_str() != field.field_name)
                || schema_field.type_reference != field.value_type
            {
                field!("changed its exact source member binding");
                continue;
            }

            let field_identity = retained_field_identity(&field.field_name, field.member_identity);
            let exact_layout_entry = view.placement.layout().entries.iter().find(|entry| {
                retained_field_identity(&entry.field, entry.member_identity) == field_identity
            });
            if exact_layout_entry.is_none() {
                field!("changed its retained layout member identity");
                continue;
            }

            let exact_access = exact_access_entry_for_field(view, field);
            if !exact_access.is_some_and(|entry| entry.access() == &field.access) {
                field!("changed its admitted access decision");
                continue;
            }

            let expected_atomic_resident = match &field.access {
                FieldAccess::Atomic {
                    transfer_width_bits,
                    operations,
                    ..
                } if operations.compare_exchange || operations.compare_exchange_once => {
                    let multiplicity = program.type_multiplicity(schema_field.type_reference);
                    if multiplicity != Multiplicity::Unrestricted {
                        field!(
                            "no longer has an unrestricted resident for observing compare-exchange"
                        );
                        continue;
                    }
                    let observing_results = super::access_atomic_observing_results(*operations);
                    Some(typed_trees::typed_trees::AccessAtomicResidentContract {
                        field_symbol: schema_field.symbol,
                        resident_type: schema_field.type_reference,
                        multiplicity,
                        transfer_width_bits: *transfer_width_bits,
                        compare_exchange: operations.compare_exchange,
                        compare_exchange_once: operations.compare_exchange_once,
                        observing_results,
                    })
                }
                _ => None,
            };
            if field.atomic_resident != expected_atomic_resident {
                field!("changed its checked Atomic resident/result contract");
                continue;
            }

            let exact_view_field = program
                .data_members(view_data)
                .iter()
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => Some(field),
                    typed_trees::data::DataMember::Variant(_) => None,
                })
                .find(|candidate| {
                    candidate.identity == field.member_identity
                        && (field.member_identity.is_some()
                            || candidate.name.as_str() == field.field_name)
                });
            if !exact_view_field
                .is_some_and(|candidate| candidate.type_reference == field.accessor_type)
            {
                field!("changed its exact synthesized accessor binding");
            }

            let has_accessor_data = crate::declarations::data_queries::data_definition_by_symbol(
                program,
                field.accessor_data_symbol,
            )
            .is_some();
            if !has_accessor_data {
                field!("no longer names its exact generated accessor data identity");
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
                    field!("changed its exact generated accessor data binding");
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
                field!("changed its generated accessor operation set");
                continue;
            }
            for target in &field.accessor_targets {
                let Some(machine) = program.machine(target.machine_symbol) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "access view `{}` field `{}` operation `{}` no longer names its exact generated accessor machine",
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
                        "access view `{}` field `{}` operation `{}` changed its exact generated accessor target",
                        view.data_name, field.field_name, target.operation
                    )));
                }
            }
        }
    }
}
