use std::path::Path;

use psi_diagnostics::Diagnostic;
use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataField, DataMember};
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Check the first closed `PlacementCustody` agreement slice. The agreement is
/// ordinary conformance evidence: this pass only replays one exact concrete
/// policy/schema plan and its direct erased schema fields.
pub(super) fn validate_agreements(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for conformance in program.conformances() {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == conformance.trait_symbol)
        else {
            continue;
        };
        if !is_core_placement_custody(program, trait_definition) {
            continue;
        }

        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [policy_argument, schema_argument] = arguments else {
            continue;
        };
        let (Some(policy_symbol), Some(schema_symbol)) = (
            concrete_named_symbol(program, *policy_argument),
            concrete_named_symbol(program, *schema_argument),
        ) else {
            continue;
        };
        let Some(plan) = program.placed_view_plans.iter().find(|plan| {
            plan.policy_symbol == policy_symbol && plan.schema_symbol == schema_symbol
        }) else {
            // The bounded checker cannot manufacture a plan for a vocabulary-
            // only conformance. Its agreement becomes checkable once a real
            // `Placed<P, T>` producer has evaluated this exact P/T pair.
            continue;
        };
        let Some(schema) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == schema_symbol)
        else {
            continue;
        };
        let Some(custody) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == conformance.carrier_symbol)
        else {
            continue;
        };
        let plan_name = program
            .symbols
            .display_path(plan.policy_plan_machine_symbol, "::");
        let schema_fields = program
            .data_members(schema)
            .iter()
            .filter_map(field)
            .collect::<Vec<_>>();
        let custody_fields = program
            .data_members(custody)
            .iter()
            .filter_map(field)
            .collect::<Vec<_>>();

        if custody_fields.len() != program.data_members(custody).len() {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: `{}` must be one ordinary record, but it declares case members",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
            continue;
        }

        for schema_field in &schema_fields {
            if !schema_field.relevance.is_erased() {
                continue;
            }
            let layout_entry = exact_layout_entry(plan, schema_field);
            let custody_field = custody_fields
                .iter()
                .copied()
                .find(|candidate| same_canonical_field(schema_field, candidate));
            if let Some(layout_entry) = layout_entry {
                if custody_field.is_some() {
                    diagnostics.push(represented_field_diagnostic(
                        program,
                        conformance,
                        schema,
                        custody,
                        schema_field,
                        &plan_name,
                        layout_entry,
                    ));
                }
                continue;
            }
            let Some(custody_field) = custody_field else {
                diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is custody-carried with exact type `{}` and multiplicity {:?}, but `{}` omits canonical field path `{}`",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(schema, schema_field),
                    program.normalized_type_identity(schema_field.type_reference),
                    program.type_multiplicity(schema_field.type_reference),
                    custody.name,
                    canonical_path(schema, schema_field),
                )));
                continue;
            };
            let expected_multiplicity = program.type_multiplicity(schema_field.type_reference);
            let actual_multiplicity = program.type_multiplicity(custody_field.type_reference);
            if actual_multiplicity != expected_multiplicity {
                diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(schema, schema_field),
                    custody.name,
                )));
                continue;
            }
            let expected_type = program.normalized_type_identity(schema_field.type_reference);
            let actual_type = program.normalized_type_identity(custody_field.type_reference);
            if actual_type != expected_type {
                diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(schema, schema_field),
                    custody.name,
                )));
            }
        }

        for custody_field in custody_fields {
            let Some(schema_field) = schema_fields
                .iter()
                .copied()
                .find(|candidate| same_canonical_field(candidate, custody_field))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{}` path, but `{}` declares extra canonical field path `{}`",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(custody, custody_field),
                    custody.name,
                    canonical_path(custody, custody_field),
                )));
                continue;
            };
            if !schema_field.relevance.is_erased()
                && let Some(layout_entry) = exact_layout_entry(plan, schema_field)
            {
                diagnostics.push(represented_field_diagnostic(
                    program,
                    conformance,
                    schema,
                    custody,
                    schema_field,
                    &plan_name,
                    layout_entry,
                ));
            }
        }
    }
}

fn is_core_placement_custody(program: &TypedTrees, definition: &TraitDefinition) -> bool {
    definition.name.as_str() == "PlacementCustody"
        && !definition.is_boundary
        && program
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == psi_source::SourceOrigin::Toolchain
                    && source.path.ends_with(Path::new("core/layout.omg"))
            })
}

fn concrete_named_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

fn field(member: &DataMember) -> Option<&DataField> {
    match member {
        DataMember::Field(field) => Some(field),
        DataMember::Variant(_) => None,
    }
}

fn same_canonical_field(left: &DataField, right: &DataField) -> bool {
    match (left.identity, right.identity) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left.name.as_str() == right.name.as_str(),
        _ => false,
    }
}

fn exact_layout_entry<'plan>(
    plan: &'plan psi_typed_trees::typed_trees::PlacedViewPlan,
    field: &DataField,
) -> Option<&'plan LayoutFieldEntryReport> {
    plan.placement.layout().entries.iter().find(|entry| {
        match (field.identity, entry.member_identity) {
            (Some(field), Some(entry)) => field == entry,
            (None, None) => field.name.as_str() == entry.field,
            _ => false,
        }
    })
}

fn canonical_path(owner: &psi_typed_trees::data::DataDefinition, field: &DataField) -> String {
    match field.identity {
        Some(identity) => format!("{}.#{}", owner.name, identity),
        None => format!("{}.{}", owner.name, field.name),
    }
}

fn represented_field_diagnostic(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    custody: &psi_typed_trees::data::DataDefinition,
    schema_field: &DataField,
    plan_name: &str,
    entry: &LayoutFieldEntryReport,
) -> Diagnostic {
    Diagnostic::error(format!(
        "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is {}, so represented field `{}` must be absent from `{}`",
        program.symbols.display_path(conformance.symbol, "::"),
        canonical_path(schema, schema_field),
        represented_decision(program, schema_field.type_reference, &entry.placement),
        canonical_path(schema, schema_field),
        custody.name,
    ))
}

fn represented_decision(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    placement: &LayoutPlacementReport,
) -> String {
    match placement {
        LayoutPlacementReport::At { offset } => primitive_width_bytes(program, type_reference)
            .map_or_else(
                || format!("represented at offset {offset} with its exact semantic width"),
                |width| format!("represented at offset {offset} with width {width}"),
            ),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            interpretation,
        } => format!(
            "represented at offset {offset} with stored width {} ({interpretation:?})",
            stored_width / 8
        ),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => format!(
            "represented in container {container} (width {container_width}) at destination bit {destination_lsb} from source bit {source_lsb} with width {width} bits"
        ),
    }
}

fn primitive_width_bytes(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    PrimitiveType::from_name(name.as_str())?.scalar_byte_size()
}
