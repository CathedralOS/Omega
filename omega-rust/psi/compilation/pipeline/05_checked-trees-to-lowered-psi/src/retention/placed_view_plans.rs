//! Preserve normalized placement and exact field/callable relationships.
//! The normalizer-issued access key and source symbols are the only joins;
//! names remain normalized identity/export payloads, never late target selection.
use crate::lowering_error::LoweringError;
use checked_trees::CheckedTrees;
use terminal_psi::{
    TerminalIdentityTableBuilder, TerminalModule, TerminalPlacedAccessor,
    TerminalPlacedAccessorKind, TerminalPlacedFieldPlan, TerminalPlacedViewPlan,
};

pub(crate) fn retain(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let declared_type = |data: &typed_trees::data::DataDefinition| {
        let identity =
            if let Some(application) = data.generic_instance {
                checked.typed.type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(application),
                )
            } else {
                checked
                    .typed
                    .normalized_nominal_type_identity(data.symbol)
                    .unwrap_or_default()
            };
        checked.typed.type_identity_text(identity).and_then(|text| {
            module
                .structural_types
                .iter()
                .find(|declaration| {
                    module.identities.text(declaration.identity) == Some(text.as_str())
                })
                .map(|declaration| declaration.id)
        })
    };
    let mut plans = Vec::new();
    for view in &checked.typed.placed_view_plans {
        let data = checked
            .typed
            .data_definition_by_symbol(view.data_symbol)
            .ok_or(LoweringError::Unsupported(
                "placed plan lost exact view definition",
            ))?;
        let Some(carrier) = declared_type(data) else {
            continue;
        };
        let schema_data = checked
            .typed
            .data_definition_by_symbol(view.schema_symbol)
            .ok_or(LoweringError::Unsupported(
                "placed plan lost exact schema definition",
            ))?;
        let schema = declared_type(schema_data).ok_or(LoweringError::Unsupported(
            "retained placed plan lost schema carrier",
        ))?;
        let mut fields = Vec::new();
        for field in &view.fields {
            let view_member = checked
                .typed
                .data_members(data)
                .iter()
                .filter_map(|member| member.as_field())
                .find(|member| member.symbol == field.view_field_symbol)
                .ok_or(LoweringError::Unsupported(
                    "placed field lost exact view member",
                ))?;
            let schema_member = checked
                .typed
                .data_members(schema_data)
                .iter()
                .filter_map(|member| member.as_field())
                .find(|member| member.symbol == field.field_symbol)
                .ok_or(LoweringError::Unsupported(
                    "placed field lost exact schema member",
                ))?;
            let accessor_data = checked
                .typed
                .data_definition_by_symbol(field.accessor_data_symbol)
                .ok_or(LoweringError::Unsupported(
                    "placed field lost exact accessor carrier",
                ))?;
            let accessor = declared_type(accessor_data).ok_or(LoweringError::Unsupported(
                "retained placed field lost accessor carrier",
            ))?;
            let mut operations = Vec::new();
            for target in &field.accessor_targets {
                let identity = crate::machine_lowering::declarations::nominal(
                    identities,
                    checked,
                    sources,
                    target.state_symbol,
                )
                .map_err(LoweringError::Unsupported)?;
                let machine_identity = crate::machine_lowering::declarations::nominal(
                    identities,
                    checked,
                    sources,
                    target.machine_symbol,
                )
                .map_err(LoweringError::Unsupported)?;
                if let Some(boundary) = module.boundary_machines.iter().find(|boundary| {
                    boundary.owner == identity || boundary.owner == machine_identity
                }) {
                    let kind = match target.operation.as_str() {
                        "read" => TerminalPlacedAccessorKind::Read,
                        "take" => TerminalPlacedAccessorKind::Take,
                        "write" => TerminalPlacedAccessorKind::Write,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "placed field has unknown normalized operation",
                            ));
                        }
                    };
                    operations.push(TerminalPlacedAccessor {
                        boundary: boundary.id,
                        kind,
                    });
                }
            }
            operations.sort_by_key(|operation| operation.boundary);
            fields.push(TerminalPlacedFieldPlan {
                access_slot: field.access_key.slot(),
                view_field: identities.intern(
                    &checked_trees::CheckedMemberIdentity::of_member(
                        view_member.identity,
                        view_member.name.clone(),
                    )
                    .spelling(),
                ),
                schema_field: identities.intern(
                    &checked_trees::CheckedMemberIdentity::of_member(
                        schema_member.identity,
                        schema_member.name.clone(),
                    )
                    .spelling(),
                ),
                accessor,
                operations,
            });
        }
        fields.sort_by_key(|field| field.access_slot);
        let policy = checked
            .typed
            .normalized_hermetic_symbol_identity(view.policy_symbol)
            .map_err(|_| LoweringError::Unsupported("placed plan lost policy identity"))?;
        let schema_identity = checked
            .typed
            .normalized_hermetic_symbol_identity(view.schema_symbol)
            .map_err(|_| LoweringError::Unsupported("placed plan lost schema identity"))?;
        plans.push(TerminalPlacedViewPlan {
            carrier,
            schema,
            view_identity: identities.intern(&terminal_psi::canonical_placed_view_identity(
                &policy,
                &schema_identity,
            )),
            policy_identity: identities.intern(&policy),
            policy_plan_machine_identity: identities.intern(
                &checked
                    .typed
                    .normalized_hermetic_symbol_identity(view.policy_plan_machine_symbol)
                    .map_err(|_| {
                        LoweringError::Unsupported("placed plan lost policy machine identity")
                    })?,
            ),
            schema_identity: identities.intern(&schema_identity),
            placement: view.placement.clone(),
            fields,
        });
    }
    plans.sort_by_key(|plan| plan.carrier);
    module.placed_view_plans = plans;
    Ok(())
}
