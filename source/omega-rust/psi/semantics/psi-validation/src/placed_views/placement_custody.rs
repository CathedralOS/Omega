use std::path::Path;

use psi_diagnostics::Diagnostic;
use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataField, DataMember};
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Check the first closed `PlacementCustody` agreement slice. The agreement is
/// ordinary conformance evidence: this pass only replays one exact concrete
/// policy/schema plan, its direct erased fields, and one acyclic ordinary
/// record path from a represented outer field to erased leaves through at
/// most three additional represented records.
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
            let custody_field = custody_fields
                .iter()
                .copied()
                .find(|candidate| same_canonical_field(schema_field, candidate));
            if !schema_field.relevance.is_erased() {
                let Some(layout_entry) = exact_layout_entry(plan, schema_field) else {
                    continue;
                };
                let Some(nested_schema) =
                    direct_nested_custody_record(program, schema_field, schema.symbol)
                else {
                    if type_contains_erased_descendant(
                        program,
                        schema_field.type_reference,
                        &mut vec![schema.symbol],
                    ) {
                        diagnostics.push(unsupported_nested_custody_diagnostic(
                            program,
                            conformance,
                            schema,
                            schema_field,
                            &plan_name,
                            layout_entry,
                        ));
                    } else if custody_field.is_some() {
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
                };
                validate_nested_record(
                    program,
                    conformance,
                    schema,
                    custody,
                    schema_field,
                    nested_schema,
                    custody_field,
                    &plan_name,
                    layout_entry,
                    diagnostics,
                );
                continue;
            }
            let layout_entry = exact_layout_entry(plan, schema_field);
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
            if !schema_fields
                .iter()
                .any(|candidate| same_canonical_field(candidate, custody_field))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{}` path, but `{}` declares extra canonical field path `{}`",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(custody, custody_field),
                    custody.name,
                    canonical_path(custody, custody_field),
                )));
                continue;
            }
            // Every known direct field was checked in the schema walk above.
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_nested_record(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    custody: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    nested_schema: &psi_typed_trees::data::DataDefinition,
    custody_field: Option<&DataField>,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let nested_schema_fields = program
        .data_members(nested_schema)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    let Some(custody_field) = custody_field else {
        for nested_field in &nested_schema_fields {
            if nested_field.relevance.is_erased() {
                let path = nested_canonical_path(schema, outer_field, nested_field);
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    nested_field,
                    &path,
                    diagnostics,
                );
                continue;
            }
            if let Some(deep_schema) =
                second_nested_custody_record(program, nested_field, &[nested_schema.symbol])
            {
                let middle_path = nested_canonical_path(schema, outer_field, nested_field);
                push_missing_second_nested_paths(
                    program,
                    conformance,
                    custody,
                    deep_schema,
                    nested_schema.symbol,
                    &middle_path,
                    plan_name,
                    diagnostics,
                );
            }
        }
        return;
    };
    let Some(nested_custody) = plain_record_for_type(program, custody_field.type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{}` requires one authored ordinary projection record, but `{}` has type `{}`",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(schema, outer_field),
            canonical_path(custody, custody_field),
            program.normalized_type_identity(custody_field.type_reference),
        )));
        return;
    };
    let nested_custody_fields = program
        .data_members(nested_custody)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    if nested_custody_fields.len() != program.data_members(nested_custody).len() {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{}` must use an ordinary projection record without case members",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(schema, outer_field),
        )));
        return;
    }

    for nested_field in &nested_schema_fields {
        let custody_leaf = nested_custody_fields
            .iter()
            .copied()
            .find(|candidate| same_canonical_field(nested_field, candidate));
        let path = nested_canonical_path(schema, outer_field, nested_field);
        if !nested_field.relevance.is_erased() {
            if let Some(deep_schema) =
                second_nested_custody_record(program, nested_field, &[nested_schema.symbol])
            {
                validate_second_nested_record(
                    program,
                    conformance,
                    schema,
                    custody,
                    outer_field,
                    nested_field,
                    deep_schema,
                    custody_leaf,
                    plan_name,
                    outer_entry,
                    diagnostics,
                );
                continue;
            }
            if custody_leaf.is_some() {
                diagnostics.push(nested_represented_field_diagnostic(
                    program,
                    conformance,
                    schema,
                    outer_field,
                    nested_field,
                    custody,
                    plan_name,
                    outer_entry,
                ));
            }
            continue;
        }
        let Some(custody_leaf) = custody_leaf else {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{}` and multiplicity {:?}, but `{}` omits canonical field path `{path}`",
                program.symbols.display_path(conformance.symbol, "::"),
                program.normalized_type_identity(nested_field.type_reference),
                program.type_multiplicity(nested_field.type_reference),
                custody.name,
            )));
            continue;
        };
        let expected_multiplicity = program.type_multiplicity(nested_field.type_reference);
        let actual_multiplicity = program.type_multiplicity(custody_leaf.type_reference);
        if actual_multiplicity != expected_multiplicity {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
            continue;
        }
        let expected_type = program.normalized_type_identity(nested_field.type_reference);
        let actual_type = program.normalized_type_identity(custody_leaf.type_reference);
        if actual_type != expected_type {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
        }
    }

    for custody_leaf in nested_custody_fields {
        if !nested_schema_fields
            .iter()
            .any(|candidate| same_canonical_field(candidate, custody_leaf))
        {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{}.{}` path, but `{}` declares that extra canonical field path",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_path(schema, outer_field),
                canonical_segment(custody_leaf),
                custody.name,
            )));
            continue;
        }
        // Every known nested field was checked in the schema walk above.
    }
}

#[allow(clippy::too_many_arguments)]
fn push_missing_custody_path(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    custody: &psi_typed_trees::data::DataDefinition,
    plan_name: &str,
    field: &DataField,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(format!(
        "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{}` and multiplicity {:?}, but `{}` omits canonical field path `{path}`",
        program.symbols.display_path(conformance.symbol, "::"),
        program.normalized_type_identity(field.type_reference),
        program.type_multiplicity(field.type_reference),
        custody.name,
    )));
}

fn direct_nested_custody_record<'program>(
    program: &'program TypedTrees,
    schema_field: &DataField,
    owner_symbol: psi_symbols::SymbolHandle,
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    let record = plain_record_for_type(program, schema_field.type_reference)?;
    if record.symbol == owner_symbol {
        return None;
    }
    let mut has_custody = false;
    for field in program.data_members(record).iter().filter_map(field) {
        if field.relevance.is_erased() {
            has_custody = true;
            continue;
        }
        if !type_contains_erased_descendant(program, field.type_reference, &mut vec![record.symbol])
        {
            continue;
        }
        second_nested_custody_record(program, field, &[record.symbol])?;
        has_custody = true;
    }
    has_custody.then_some(record)
}

fn second_nested_custody_record<'program>(
    program: &'program TypedTrees,
    schema_field: &DataField,
    ancestors: &[psi_symbols::SymbolHandle],
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    let record = plain_record_for_type(program, schema_field.type_reference)?;
    if ancestors.contains(&record.symbol)
        || !fixed_type_width_bytes(program, schema_field.type_reference)
            .is_some_and(|width| width > 0)
    {
        return None;
    }
    let mut third_ancestors = ancestors.to_vec();
    third_ancestors.push(record.symbol);
    let mut has_custody = false;
    for field in program.data_members(record).iter().filter_map(field) {
        if field.relevance.is_erased() {
            has_custody = true;
            continue;
        }
        if !type_contains_erased_descendant(
            program,
            field.type_reference,
            &mut third_ancestors.clone(),
        ) {
            continue;
        }
        third_nested_custody_record(program, field, &third_ancestors)?;
        has_custody = true;
    }
    has_custody.then_some(record)
}

fn third_nested_custody_record<'program>(
    program: &'program TypedTrees,
    schema_field: &DataField,
    ancestors: &[psi_symbols::SymbolHandle],
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    let record = plain_record_for_type(program, schema_field.type_reference)?;
    if ancestors.contains(&record.symbol)
        || !fixed_type_width_bytes(program, schema_field.type_reference)
            .is_some_and(|width| width > 0)
    {
        return None;
    }
    let mut has_custody = false;
    let mut visiting = ancestors.to_vec();
    visiting.push(record.symbol);
    for field in program.data_members(record).iter().filter_map(field) {
        if field.relevance.is_erased() {
            has_custody = true;
            continue;
        }
        if !type_contains_erased_descendant(program, field.type_reference, &mut visiting.clone()) {
            continue;
        }
        fourth_nested_custody_record(program, field, &visiting)?;
        has_custody = true;
    }
    has_custody.then_some(record)
}

fn fourth_nested_custody_record<'program>(
    program: &'program TypedTrees,
    schema_field: &DataField,
    ancestors: &[psi_symbols::SymbolHandle],
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    let record = plain_record_for_type(program, schema_field.type_reference)?;
    if ancestors.contains(&record.symbol)
        || !fixed_type_width_bytes(program, schema_field.type_reference)
            .is_some_and(|width| width > 0)
    {
        return None;
    }
    let mut has_custody = false;
    let mut visiting = ancestors.to_vec();
    visiting.push(record.symbol);
    for field in program.data_members(record).iter().filter_map(field) {
        if field.relevance.is_erased() {
            has_custody = true;
            continue;
        }
        if type_contains_erased_descendant(program, field.type_reference, &mut visiting.clone()) {
            return None;
        }
    }
    has_custody.then_some(record)
}

fn type_contains_erased_descendant(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> bool {
    let mut memo = Vec::new();
    let mut visited_records = 0usize;
    type_contains_erased_descendant_bounded(
        program,
        type_reference,
        visiting,
        &mut memo,
        &mut visited_records,
        0,
    )
}

const MAX_CUSTODY_DESCENDANT_TYPE_DEPTH: usize = 64;
const MAX_CUSTODY_DESCENDANT_RECORDS: usize = 256;

fn type_contains_erased_descendant_bounded(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
    memo: &mut Vec<(psi_symbols::SymbolHandle, bool)>,
    visited_records: &mut usize,
    depth: usize,
) -> bool {
    if !type_reference.is_valid() {
        return true;
    }
    if depth > MAX_CUSTODY_DESCENDANT_TYPE_DEPTH {
        return true;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            let Some(record) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
            else {
                return false;
            };
            if visiting.contains(symbol) {
                return true;
            }
            if let Some((_, result)) = memo.iter().find(|(candidate, _)| candidate == symbol) {
                return *result;
            }
            *visited_records = match visited_records.checked_add(1) {
                Some(count) if count <= MAX_CUSTODY_DESCENDANT_RECORDS => count,
                _ => return true,
            };
            visiting.push(*symbol);
            let mut result = false;
            for member in program.data_members(record) {
                let fields = match member {
                    DataMember::Field(field) => std::slice::from_ref(field),
                    DataMember::Variant(variant) => program.data_payload_fields(variant),
                };
                for field in fields {
                    if field.relevance.is_erased()
                        || type_contains_erased_descendant_bounded(
                            program,
                            field.type_reference,
                            visiting,
                            memo,
                            visited_records,
                            depth + 1,
                        )
                    {
                        result = true;
                        break;
                    }
                }
                if result {
                    break;
                }
            }
            visiting.pop();
            memo.push((*symbol, result));
            result
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => type_contains_erased_descendant_bounded(
            program,
            *element_type,
            visiting,
            memo,
            visited_records,
            depth + 1,
        ),
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => type_contains_erased_descendant_bounded(
            program,
            *base_type,
            visiting,
            memo,
            visited_records,
            depth + 1,
        ),
        TypeReferenceNode::Generic { .. } => true,
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => true,
        TypeReferenceNode::Unit => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_missing_second_nested_paths(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    custody: &psi_typed_trees::data::DataDefinition,
    deep_schema: &psi_typed_trees::data::DataDefinition,
    parent_symbol: psi_symbols::SymbolHandle,
    middle_path: &str,
    plan_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for deep_field in program.data_members(deep_schema).iter().filter_map(field) {
        let path = format!("{middle_path}.{}", canonical_segment(deep_field));
        if deep_field.relevance.is_erased() {
            push_missing_custody_path(
                program,
                conformance,
                custody,
                plan_name,
                deep_field,
                &path,
                diagnostics,
            );
            continue;
        }
        let Some(third_schema) =
            third_nested_custody_record(program, deep_field, &[parent_symbol, deep_schema.symbol])
        else {
            continue;
        };
        for third_field in program.data_members(third_schema).iter().filter_map(field) {
            let third_path = format!("{path}.{}", canonical_segment(third_field));
            if third_field.relevance.is_erased() {
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    third_field,
                    &third_path,
                    diagnostics,
                );
                continue;
            }
            let Some(fourth_schema) = fourth_nested_custody_record(
                program,
                third_field,
                &[parent_symbol, deep_schema.symbol, third_schema.symbol],
            ) else {
                continue;
            };
            for fourth_field in program
                .data_members(fourth_schema)
                .iter()
                .filter_map(field)
                .filter(|field| field.relevance.is_erased())
            {
                let fourth_path = format!("{third_path}.{}", canonical_segment(fourth_field));
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    fourth_field,
                    &fourth_path,
                    diagnostics,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_second_nested_record(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    custody: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    middle_field: &DataField,
    deep_schema: &psi_typed_trees::data::DataDefinition,
    custody_field: Option<&DataField>,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let deep_schema_fields = program
        .data_members(deep_schema)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    let middle_path = nested_canonical_path(schema, outer_field, middle_field);
    let Some(custody_field) = custody_field else {
        push_missing_second_nested_paths(
            program,
            conformance,
            custody,
            deep_schema,
            nested_record_symbol(program, outer_field),
            &middle_path,
            plan_name,
            diagnostics,
        );
        return;
    };
    let Some(deep_custody) = plain_record_for_type(program, custody_field.type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{middle_path}` requires one authored ordinary projection record, but `{}` has type `{}`",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(custody, custody_field),
            program.normalized_type_identity(custody_field.type_reference),
        )));
        return;
    };
    let deep_custody_fields = program
        .data_members(deep_custody)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    if deep_custody_fields.len() != program.data_members(deep_custody).len() {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{middle_path}` must use an ordinary projection record without case members",
            program.symbols.display_path(conformance.symbol, "::"),
        )));
        return;
    }

    for deep_field in &deep_schema_fields {
        let custody_leaf = deep_custody_fields
            .iter()
            .copied()
            .find(|candidate| same_canonical_field(deep_field, candidate));
        let path = format!("{middle_path}.{}", canonical_segment(deep_field));
        if !deep_field.relevance.is_erased() {
            if let Some(third_schema) = third_nested_custody_record(
                program,
                deep_field,
                &[
                    nested_record_symbol(program, outer_field),
                    deep_schema.symbol,
                ],
            ) {
                validate_third_nested_record(
                    program,
                    conformance,
                    schema,
                    custody,
                    outer_field,
                    third_schema,
                    &[
                        nested_record_symbol(program, outer_field),
                        deep_schema.symbol,
                        third_schema.symbol,
                    ],
                    custody_leaf,
                    &path,
                    plan_name,
                    outer_entry,
                    diagnostics,
                );
                continue;
            }
            if custody_leaf.is_some() {
                diagnostics.push(nested_path_represented_field_diagnostic(
                    program,
                    conformance,
                    &path,
                    schema,
                    outer_field,
                    custody,
                    plan_name,
                    outer_entry,
                ));
            }
            continue;
        }
        let Some(custody_leaf) = custody_leaf else {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{}` and multiplicity {:?}, but `{}` omits canonical field path `{path}`",
                program.symbols.display_path(conformance.symbol, "::"),
                program.normalized_type_identity(deep_field.type_reference),
                program.type_multiplicity(deep_field.type_reference),
                custody.name,
            )));
            continue;
        };
        let expected_multiplicity = program.type_multiplicity(deep_field.type_reference);
        let actual_multiplicity = program.type_multiplicity(custody_leaf.type_reference);
        if actual_multiplicity != expected_multiplicity {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
            continue;
        }
        let expected_type = program.normalized_type_identity(deep_field.type_reference);
        let actual_type = program.normalized_type_identity(custody_leaf.type_reference);
        if actual_type != expected_type {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
        }
    }

    for custody_leaf in deep_custody_fields {
        if !deep_schema_fields
            .iter()
            .any(|candidate| same_canonical_field(candidate, custody_leaf))
        {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{middle_path}.{}` path, but `{}` declares that extra canonical field path",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_segment(custody_leaf),
                custody.name,
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_third_nested_record(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    custody: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    third_schema: &psi_typed_trees::data::DataDefinition,
    ancestors: &[psi_symbols::SymbolHandle],
    custody_field: Option<&DataField>,
    third_path: &str,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let third_schema_fields = program
        .data_members(third_schema)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    let Some(custody_field) = custody_field else {
        for leaf in &third_schema_fields {
            let path = format!("{third_path}.{}", canonical_segment(leaf));
            if leaf.relevance.is_erased() {
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    leaf,
                    &path,
                    diagnostics,
                );
                continue;
            }
            let Some(fourth_schema) = fourth_nested_custody_record(program, leaf, ancestors) else {
                continue;
            };
            for fourth_leaf in program
                .data_members(fourth_schema)
                .iter()
                .filter_map(field)
                .filter(|field| field.relevance.is_erased())
            {
                let fourth_path = format!("{path}.{}", canonical_segment(fourth_leaf));
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    fourth_leaf,
                    &fourth_path,
                    diagnostics,
                );
            }
        }
        return;
    };
    let Some(third_custody) = plain_record_for_type(program, custody_field.type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{third_path}` requires one authored ordinary projection record, but `{}` has type `{}`",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(custody, custody_field),
            program.normalized_type_identity(custody_field.type_reference),
        )));
        return;
    };
    let third_custody_fields = program
        .data_members(third_custody)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    if third_custody_fields.len() != program.data_members(third_custody).len() {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{third_path}` must use an ordinary projection record without case members",
            program.symbols.display_path(conformance.symbol, "::"),
        )));
        return;
    }

    for leaf in &third_schema_fields {
        let custody_leaf = third_custody_fields
            .iter()
            .copied()
            .find(|candidate| same_canonical_field(leaf, candidate));
        let path = format!("{third_path}.{}", canonical_segment(leaf));
        if !leaf.relevance.is_erased() {
            if let Some(fourth_schema) = fourth_nested_custody_record(program, leaf, ancestors) {
                validate_fourth_nested_record(
                    program,
                    conformance,
                    schema,
                    custody,
                    outer_field,
                    fourth_schema,
                    custody_leaf,
                    &path,
                    plan_name,
                    outer_entry,
                    diagnostics,
                );
                continue;
            }
            if custody_leaf.is_some() {
                diagnostics.push(nested_path_represented_field_diagnostic(
                    program,
                    conformance,
                    &path,
                    schema,
                    outer_field,
                    custody,
                    plan_name,
                    outer_entry,
                ));
            }
            continue;
        }
        let Some(custody_leaf) = custody_leaf else {
            push_missing_custody_path(
                program,
                conformance,
                custody,
                plan_name,
                leaf,
                &path,
                diagnostics,
            );
            continue;
        };
        let expected_multiplicity = program.type_multiplicity(leaf.type_reference);
        let actual_multiplicity = program.type_multiplicity(custody_leaf.type_reference);
        if actual_multiplicity != expected_multiplicity {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
            continue;
        }
        let expected_type = program.normalized_type_identity(leaf.type_reference);
        let actual_type = program.normalized_type_identity(custody_leaf.type_reference);
        if actual_type != expected_type {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
        }
    }

    for custody_leaf in third_custody_fields {
        if !third_schema_fields
            .iter()
            .any(|candidate| same_canonical_field(candidate, custody_leaf))
        {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{third_path}.{}` path, but `{}` declares that extra canonical field path",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_segment(custody_leaf),
                custody.name,
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_fourth_nested_record(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    custody: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    fourth_schema: &psi_typed_trees::data::DataDefinition,
    custody_field: Option<&DataField>,
    fourth_path: &str,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let schema_fields = program
        .data_members(fourth_schema)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    let Some(custody_field) = custody_field else {
        for leaf in schema_fields
            .iter()
            .filter(|field| field.relevance.is_erased())
        {
            let path = format!("{fourth_path}.{}", canonical_segment(leaf));
            push_missing_custody_path(
                program,
                conformance,
                custody,
                plan_name,
                leaf,
                &path,
                diagnostics,
            );
        }
        return;
    };
    let Some(fourth_custody) = plain_record_for_type(program, custody_field.type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{fourth_path}` requires one authored ordinary projection record, but `{}` has type `{}`",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(custody, custody_field),
            program.normalized_type_identity(custody_field.type_reference),
        )));
        return;
    };
    let custody_fields = program
        .data_members(fourth_custody)
        .iter()
        .filter_map(field)
        .collect::<Vec<_>>();
    if custody_fields.len() != program.data_members(fourth_custody).len() {
        diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{fourth_path}` must use an ordinary projection record without case members",
            program.symbols.display_path(conformance.symbol, "::"),
        )));
        return;
    }
    for leaf in &schema_fields {
        let custody_leaf = custody_fields
            .iter()
            .copied()
            .find(|candidate| same_canonical_field(leaf, candidate));
        let path = format!("{fourth_path}.{}", canonical_segment(leaf));
        if !leaf.relevance.is_erased() {
            if custody_leaf.is_some() {
                diagnostics.push(nested_path_represented_field_diagnostic(
                    program,
                    conformance,
                    &path,
                    schema,
                    outer_field,
                    custody,
                    plan_name,
                    outer_entry,
                ));
            }
            continue;
        }
        let Some(custody_leaf) = custody_leaf else {
            push_missing_custody_path(
                program,
                conformance,
                custody,
                plan_name,
                leaf,
                &path,
                diagnostics,
            );
            continue;
        };
        let expected_multiplicity = program.type_multiplicity(leaf.type_reference);
        let actual_multiplicity = program.type_multiplicity(custody_leaf.type_reference);
        if actual_multiplicity != expected_multiplicity {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
                program.symbols.display_path(conformance.symbol, "::"), custody.name,
            )));
            continue;
        }
        let expected_type = program.normalized_type_identity(leaf.type_reference);
        let actual_type = program.normalized_type_identity(custody_leaf.type_reference);
        if actual_type != expected_type {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
                program.symbols.display_path(conformance.symbol, "::"), custody.name,
            )));
        }
    }
    for custody_leaf in custody_fields {
        if !schema_fields
            .iter()
            .any(|candidate| same_canonical_field(candidate, custody_leaf))
        {
            diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{fourth_path}.{}` path, but `{}` declares that extra canonical field path",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_segment(custody_leaf), custody.name,
            )));
        }
    }
}

fn nested_record_symbol(program: &TypedTrees, field: &DataField) -> psi_symbols::SymbolHandle {
    plain_record_for_type(program, field.type_reference)
        .map_or(psi_symbols::SymbolHandle::invalid(), |record| record.symbol)
}

fn plain_record_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let record = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *symbol)?;
    (record.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
        && record.generic_instance.is_none()
        && record.type_parameters.is_empty()
        && record.lifetime_parameters.is_empty()
        && record.quotient.is_none()
        && program
            .data_members(record)
            .iter()
            .all(|member| matches!(member, DataMember::Field(_))))
    .then_some(record)
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
    format!("{}.{}", owner.name, canonical_segment(field))
}

fn canonical_segment(field: &DataField) -> String {
    match field.identity {
        Some(identity) => format!("#{identity}"),
        None => field.name.to_string(),
    }
}

fn nested_canonical_path(
    root: &psi_typed_trees::data::DataDefinition,
    outer: &DataField,
    nested: &DataField,
) -> String {
    format!(
        "{}.{}.{}",
        root.name,
        canonical_segment(outer),
        canonical_segment(nested)
    )
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

#[allow(clippy::too_many_arguments)]
fn nested_represented_field_diagnostic(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    nested_field: &DataField,
    custody: &psi_typed_trees::data::DataDefinition,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
) -> Diagnostic {
    let path = nested_canonical_path(schema, outer_field, nested_field);
    nested_path_represented_field_diagnostic(
        program,
        conformance,
        &path,
        schema,
        outer_field,
        custody,
        plan_name,
        outer_entry,
    )
}

#[allow(clippy::too_many_arguments)]
fn nested_path_represented_field_diagnostic(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    path: &str,
    schema: &psi_typed_trees::data::DataDefinition,
    outer_field: &DataField,
    custody: &psi_typed_trees::data::DataDefinition,
    plan_name: &str,
    outer_entry: &LayoutFieldEntryReport,
) -> Diagnostic {
    Diagnostic::error(format!(
        "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is contained in `{}`, which is {}, so represented field `{}` must be absent from `{}`",
        program.symbols.display_path(conformance.symbol, "::"),
        path,
        canonical_path(schema, outer_field),
        represented_decision(program, outer_field.type_reference, &outer_entry.placement),
        path,
        custody.name,
    ))
}

fn unsupported_nested_custody_diagnostic(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    schema: &psi_typed_trees::data::DataDefinition,
    field: &DataField,
    plan_name: &str,
    entry: &LayoutFieldEntryReport,
) -> Diagnostic {
    Diagnostic::error(format!(
        "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is {}, but its represented type contains non-runtime custody outside the exact four-record acyclic, non-generic, case-free projection spine",
        program.symbols.display_path(conformance.symbol, "::"),
        canonical_path(schema, field),
        represented_decision(program, field.type_reference, &entry.placement),
    ))
}

fn represented_decision(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    placement: &LayoutPlacementReport,
) -> String {
    match placement {
        LayoutPlacementReport::At { offset } => fixed_type_width_bytes(program, type_reference)
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

fn fixed_type_width_bytes(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    fixed_type_layout(program, type_reference, &mut Vec::new()).map(|(size, _)| size)
}

fn fixed_type_layout(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Option<(usize, usize)> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(primitive) = crate::recasts::exact_primitive_type(program, type_reference) {
                let size = primitive.scalar_byte_size()?;
                return Some((size, size));
            }
            if visiting.contains(symbol) {
                return None;
            }
            let record = plain_record_for_type(program, type_reference)?;
            visiting.push(*symbol);
            let mut size = 0usize;
            let mut aggregate_align = 1usize;
            for field in program.data_members(record).iter().filter_map(field) {
                if field.relevance.is_erased() {
                    continue;
                }
                let (field_size, field_align) =
                    fixed_type_layout(program, field.type_reference, visiting)?;
                size = align_up(size, field_align)?.checked_add(field_size)?;
                aggregate_align = aggregate_align.max(field_align);
            }
            visiting.pop();
            Some((align_up(size, aggregate_align)?, aggregate_align))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element_size, element_align) =
                fixed_type_layout(program, *element_type, visiting)?;
            Some((element_size.checked_mul(*length)?, element_align))
        }
        _ => None,
    }
}

fn align_up(value: usize, align: usize) -> Option<usize> {
    value
        .checked_add(align.checked_sub(1)?)
        .map(|value| value / align * align)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_core::BindingRelevance;
    use psi_typed_trees::data::{DataDefinition, DataField};
    use psi_typed_trees::name::Identifier;

    fn named_type(
        program: &mut TypedTrees,
        symbol: psi_symbols::SymbolHandle,
        name: &str,
    ) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(name),
            })
    }

    fn push_record(
        program: &mut TypedTrees,
        symbol: psi_symbols::SymbolHandle,
        field_type: TypeReferenceHandle,
        relevance: BindingRelevance,
    ) {
        let mut definition = DataDefinition {
            symbol,
            name: Identifier::generated("CustodyDepth"),
            ..DataDefinition::default()
        };
        program.push_data_member(
            &mut definition,
            DataMember::Field(DataField {
                relevance,
                type_reference: field_type,
                ..DataField::default()
            }),
        );
        program.push_data_definition(definition);
    }

    #[test]
    fn erased_descendant_walk_fails_closed_at_the_depth_cap_without_host_recursion() {
        let mut program = TypedTrees::default();
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let terminal_symbol = psi_symbols::SymbolHandle::from_arena_index(1_000);
        push_record(
            &mut program,
            terminal_symbol,
            unit,
            BindingRelevance::Erased,
        );
        let mut next = named_type(&mut program, terminal_symbol, "Terminal");
        for index in (0..=MAX_CUSTODY_DESCENDANT_TYPE_DEPTH).rev() {
            let symbol = psi_symbols::SymbolHandle::from_arena_index(
                u32::try_from(1_100 + index).expect("bounded test symbol index fits u32"),
            );
            push_record(&mut program, symbol, next, BindingRelevance::Relevant);
            next = named_type(&mut program, symbol, "Wrapper");
        }

        assert!(type_contains_erased_descendant(
            &program,
            next,
            &mut Vec::new(),
        ));
    }

    #[test]
    fn erased_descendant_walk_fails_closed_on_an_exact_symbol_cycle() {
        let mut program = TypedTrees::default();
        let symbol = psi_symbols::SymbolHandle::from_arena_index(2_000);
        let recursive = named_type(&mut program, symbol, "Recursive");
        push_record(&mut program, symbol, recursive, BindingRelevance::Relevant);

        assert!(type_contains_erased_descendant(
            &program,
            recursive,
            &mut Vec::new(),
        ));
    }
}
