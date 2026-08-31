//! Projection of the authoritative conventional pure-sum runtime layout.
//!
//! This is a report of the fixed tag-prefixed overlay selected by this crate,
//! not a back door for programmable `Layout` policies to author case/tag
//! placement.

use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_layout_plans::{
    ConventionalDepthFiveRecordSumOccurrenceLayoutReport,
    ConventionalDepthFiveRecordSumPathsLayoutReport,
    ConventionalDepthFourRecordSumOccurrenceLayoutReport,
    ConventionalDepthFourRecordSumPathsLayoutReport,
    ConventionalDepthSixRecordSumOccurrenceLayoutReport,
    ConventionalDepthSixRecordSumPathsLayoutReport,
    ConventionalDepthThreeRecordSumOccurrenceLayoutReport,
    ConventionalDepthThreeRecordSumPathLayoutReport,
    ConventionalDepthThreeRecordSumPathsLayoutReport,
    ConventionalDepthTwoRecordSumOccurrenceLayoutReport,
    ConventionalDepthTwoRecordSumPathLayoutReport, ConventionalDepthTwoRecordSumPathsLayoutReport,
    ConventionalNestedRecordSumOccurrenceLayoutReport, ConventionalNestedRecordSumPathLayoutReport,
    ConventionalNestedRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumCaseLayoutReport, ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceNode};

use crate::{DataShape, ENUM_TAG_BYTES, LayoutPlan, TypeLayoutDescriptor};

/// Project the bounded nested-sum materialization set from the exact target
/// runtime layout: one closed `[copy]` record with one or more direct,
/// runtime-relevant conventional pure-sum fields.
///
/// The outer report contains only whole-field `At` placements. The nested
/// reports remain compiler-owned tag/payload overlays; this function does not
/// expose programmable tag or case placement. Every nested report is paired
/// with its outer field name and stable member identity in authored runtime
/// field order, so repeated uses of the same sum type remain distinguishable.
/// Arrays of sums, recursively nested sums, and mixed data shapes reject.
pub fn project_conventional_record_with_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<(LayoutPlanReport, Vec<ConventionalSumFieldLayoutReport>), Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "nested-sum record")?;
    if definition.supply_mode != DataSupplyMode::CheckedShape
        || definition.properties.multiplicity != Multiplicity::Unrestricted
        || !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || DataDefinition::shape_kind_from_members(program.data_members(definition))
            != DataShapeKind::Record
    {
        return Err(Diagnostic::error(format!(
            "nested-sum materialization owner `{}` must be one closed non-generic `[copy]` record",
            definition.name
        )));
    }

    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for nested-sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for nested-sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut nested_sums = Vec::new();
    let mut entries = Vec::with_capacity(declared_fields.len());
    let mut offsets = Vec::with_capacity(declared_fields.len());
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "nested-sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }
        if let Some(named) = exact_named_data(program, declared.type_reference)? {
            match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                DataShapeKind::Enum => {
                    let TypeLayoutDescriptor::Named {
                        symbol: laid_symbol,
                        name: laid_name,
                    } = &laid.type_descriptor
                    else {
                        return Err(Diagnostic::error(format!(
                            "target runtime layout field `{}` is not the exact declared nested sum",
                            declared.name
                        )));
                    };
                    if laid.type_symbol != named.symbol
                        || *laid_symbol != named.symbol
                        || laid_name.as_str() != named.name.as_str()
                    {
                        return Err(Diagnostic::error(format!(
                            "target runtime layout field `{}` substitutes its nested sum type",
                            declared.name
                        )));
                    }
                    let nested_layout = project_conventional_sum_materialization_layout(
                        program,
                        plan,
                        named.symbol,
                    )?;
                    if laid.layout.size as u64 != nested_layout.size
                        || laid.layout.alignment as u64 != nested_layout.align
                    {
                        return Err(Diagnostic::error(format!(
                            "target runtime layout field `{}` does not retain the exact conventional sum extent/alignment",
                            declared.name
                        )));
                    }
                    nested_sums.push(ConventionalSumFieldLayoutReport {
                        field: declared.name.to_string(),
                        member_identity: declared.identity,
                        layout: nested_layout,
                    });
                }
                DataShapeKind::Mixed => {
                    return Err(Diagnostic::error(format!(
                        "nested-sum layout field `{}` uses a mixed common-field/case shape",
                        declared.name
                    )));
                }
                DataShapeKind::Empty | DataShapeKind::Record => {}
            }
        }
        let offset = laid.offset as u64;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if nested_sums.is_empty() {
        return Err(Diagnostic::error(
            "nested-sum layout projection requires at least one direct runtime-relevant pure-sum field",
        ));
    }

    Ok((
        LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(data_layout.layout.size as u64),
            align: data_layout.layout.alignment as u64,
        },
        nested_sums,
    ))
}

/// Project the bounded one-record path to direct conventional sum fields.
///
/// The outer record must contain exactly one relevant direct field whose exact
/// closed `[copy]` record type contains a nonempty direct pure-sum set. Both
/// record layouts and all child rows come from `plan`; no path flattening or
/// independently supplied nested plan is accepted.
pub fn project_conventional_record_with_nested_sum_record_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalNestedRecordSumPathLayoutReport, Diagnostic> {
    let plural = project_conventional_record_with_nested_sum_records_materialization_layout(
        program,
        plan,
        data_symbol,
    )?;
    if plural.paths.len() != 1 {
        return Err(Diagnostic::error(format!(
            "singular nested-record sum projection requires exactly one qualifying direct inner-record field; found {}",
            plural.paths.len()
        )));
    }
    let path = plural.paths.into_iter().next().expect("exactly one path");
    Ok(ConventionalNestedRecordSumPathLayoutReport {
        outer_layout: plural.outer_layout,
        outer_field: path.outer_field,
        outer_member_identity: path.outer_member_identity,
        inner_layout: path.inner_layout,
        child_sum_layouts: path.child_sum_layouts,
    })
}

/// Project the complete nonempty authored-order set of bounded one-record
/// paths to direct conventional sum fields. The outer layout is retained once;
/// each occurrence owns one exact inner layout and complete child rows.
pub fn project_conventional_record_with_nested_sum_records_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalNestedRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_nested_sum_records_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

fn project_conventional_record_with_nested_sum_records_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalNestedRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "nested-record sum owner")?;
    validate_closed_copy_record(program, definition, "nested-record sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for nested-record sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for nested-record sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("nested-record sum outer layout report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("nested-record sum outer offset report exceeds compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("nested-record sum path report exceeds compiler resources")
        })?;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "nested-record sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if matches!(
            program
                .type_reference_table
                .type_reference(declared.type_reference),
            TypeReferenceNode::FixedArray { .. }
        ) && reachability.type_contains_sum(declared.type_reference)?
        {
            return Err(Diagnostic::error(format!(
                "nested-record sum outer field `{}` uses an array or deeper aggregate containing sums",
                declared.name
            )));
        }
        if let Some(named) = exact_named_data(program, declared.type_reference)? {
            match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                DataShapeKind::Enum => {
                    return Err(Diagnostic::error(format!(
                        "nested-record sum materialization does not admit direct outer sum field `{}`",
                        declared.name
                    )));
                }
                DataShapeKind::Mixed => {
                    return Err(Diagnostic::error(format!(
                        "nested-record sum outer field `{}` uses a mixed common-field/case shape",
                        declared.name
                    )));
                }
                DataShapeKind::Record => {
                    let profile = record_sum_profile(program, named, reachability)?;
                    if profile.direct {
                        if profile.array || profile.deeper {
                            return Err(Diagnostic::error(format!(
                                "nested-record sum field `{}` combines direct sums with an array or deeper sum path",
                                declared.name
                            )));
                        }
                        validate_closed_copy_record(program, named, "nested-record sum inner")?;
                        let TypeLayoutDescriptor::Named {
                            symbol: laid_symbol,
                            name: laid_name,
                        } = &laid.type_descriptor
                        else {
                            return Err(Diagnostic::error(format!(
                                "target runtime layout field `{}` is not the exact declared inner record",
                                declared.name
                            )));
                        };
                        if laid.type_symbol != named.symbol
                            || *laid_symbol != named.symbol
                            || laid_name.as_str() != named.name.as_str()
                        {
                            return Err(Diagnostic::error(format!(
                                "target runtime layout field `{}` substitutes its inner record type",
                                declared.name
                            )));
                        }
                        let (inner_layout, child_sum_layouts) =
                            project_conventional_record_with_sum_materialization_layout(
                                program,
                                plan,
                                named.symbol,
                            )?;
                        if usize_to_u64(laid.layout.size, "inner record extent")?
                            != inner_layout
                                .size
                                .expect("inner projection has fixed extent")
                            || usize_to_u64(laid.layout.alignment, "inner record alignment")?
                                != inner_layout.align
                        {
                            return Err(Diagnostic::error(format!(
                                "target runtime layout field `{}` does not retain the exact inner record extent/alignment",
                                declared.name
                            )));
                        }
                        paths.push(ConventionalNestedRecordSumOccurrenceLayoutReport {
                            outer_field: declared.name.to_string(),
                            outer_member_identity: declared.identity,
                            inner_layout,
                            child_sum_layouts,
                        });
                    } else if profile.array || profile.deeper {
                        return Err(Diagnostic::error(format!(
                            "nested-record sum field `{}` reaches sums beyond the admitted direct child path",
                            declared.name
                        )));
                    }
                }
                DataShapeKind::Empty => {}
            }
        }

        let offset = usize_to_u64(laid.offset, "outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "nested-record sum projection requires a nonempty qualifying direct inner-record field set",
        ));
    }
    let outer_layout = LayoutPlanReport {
        schema_report_fingerprint: psi_typed_trees::identity::normalized_schema_report_fingerprint(
            program, definition,
        ),
        entries,
        offsets: Some(offsets),
        size: Some(usize_to_u64(
            data_layout.layout.size,
            "outer record extent",
        )?),
        align: usize_to_u64(data_layout.layout.alignment, "outer record alignment")?,
    };
    Ok(ConventionalNestedRecordSumPathsLayoutReport {
        outer_layout,
        paths,
    })
}

/// Project one exact fixed-depth record chain:
/// `Outer -> Middle -> Leaf -> direct conventional sums`.
///
/// Exactly one runtime-relevant outer field may reach any sum, and the middle
/// record must itself satisfy the existing singular one-level path judgment.
/// All three whole-record layouts and the leaf sum rows come from `plan`.
pub fn project_conventional_record_with_depth_two_nested_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthTwoRecordSumPathLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "depth-two sum owner")?;
    validate_closed_copy_record(program, definition, "depth-two sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for depth-two sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for depth-two sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("depth-two sum outer report exceeds compiler resources"))?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("depth-two sum outer offsets exceed compiler resources"))?;
    let mut selected = None;
    let mut reachability = SumReachability::new(program);
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "depth-two sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "depth-two sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "depth-two sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "depth-two sum outer field `{}` does not name the required middle record",
                    declared.name
                )));
            }
            if selected.is_some() {
                return Err(Diagnostic::error(
                    "depth-two sum projection requires exactly one sum-reachable outer record field",
                ));
            }
            let middle_path =
                project_conventional_record_with_nested_sum_record_materialization_layout(
                    program,
                    plan,
                    named.symbol,
                )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared middle record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its middle record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "middle record extent")?
                != middle_path
                    .outer_layout
                    .size
                    .expect("middle projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "middle record alignment")?
                    != middle_path.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact middle record extent/alignment",
                    declared.name
                )));
            }
            selected = Some((declared.name.to_string(), declared.identity, middle_path));
        }

        let offset = usize_to_u64(laid.offset, "depth-two outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    let Some((outer_field, outer_member_identity, middle_path)) = selected else {
        return Err(Diagnostic::error(
            "depth-two sum projection requires exactly one qualifying record chain",
        ));
    };
    Ok(ConventionalDepthTwoRecordSumPathLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "depth-two outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "depth-two outer record alignment",
            )?,
        },
        outer_field,
        outer_member_identity,
        middle_path,
    })
}

/// Project one exact fixed-depth record chain:
/// `Outer -> First -> Middle -> Leaf -> direct conventional sums`.
///
/// Exactly one runtime-relevant outer field may reach any sum, and that field's
/// exact closed record type must satisfy the existing singular depth-two path
/// judgment. All four whole-record layouts and the leaf sum rows come from the
/// same target plan.
pub fn project_conventional_record_with_depth_three_nested_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthThreeRecordSumPathLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "depth-three sum owner")?;
    validate_closed_copy_record(program, definition, "depth-three sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for depth-three sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for depth-three sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("depth-three sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("depth-three sum outer offsets exceed compiler resources")
        })?;
    let mut selected = None;
    let mut reachability = SumReachability::new(program);
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "depth-three sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "depth-three sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "depth-three sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "depth-three sum outer field `{}` does not name the required enclosing record",
                    declared.name
                )));
            }
            if selected.is_some() {
                return Err(Diagnostic::error(
                    "depth-three sum projection requires exactly one sum-reachable outer record field",
                ));
            }
            let depth_two_path =
                project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                    program,
                    plan,
                    named.symbol,
                )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared enclosing record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its enclosing record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-two record extent")?
                != depth_two_path
                    .outer_layout
                    .size
                    .expect("depth-two projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "depth-two record alignment")?
                    != depth_two_path.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact depth-two record extent/alignment",
                    declared.name
                )));
            }
            selected = Some((declared.name.to_string(), declared.identity, depth_two_path));
        }

        let offset = usize_to_u64(laid.offset, "depth-three outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    let Some((outer_field, outer_member_identity, depth_two_path)) = selected else {
        return Err(Diagnostic::error(
            "depth-three sum projection requires exactly one qualifying record chain",
        ));
    };
    Ok(ConventionalDepthThreeRecordSumPathLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "depth-three outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "depth-three outer record alignment",
            )?,
        },
        outer_field,
        outer_member_identity,
        depth_two_path,
    })
}

/// Project the complete nonempty authored-order set of exact depth-three
/// record chains: `Outer -> First -> Middle -> Leaf -> direct sums`.
///
/// The outer layout is retained once. Every sum-reachable outer field owns the
/// unchanged plural depth-two report for its exact first record. The shared
/// reachability walk and global leaf-path ceiling prevent repeated nominal
/// subgraphs from amplifying traversal or retained custody.
pub fn project_conventional_record_with_depth_three_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthThreeRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_three_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

fn project_conventional_record_with_depth_three_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthThreeRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "plural depth-three sum owner")?;
    validate_closed_copy_record(program, definition, "plural depth-three sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for plural depth-three sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for plural depth-three sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-three sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-three sum outer offsets exceed compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-three sum path report exceeds compiler resources")
        })?;
    let mut total_leaf_paths = 0usize;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "plural depth-three sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "plural depth-three sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural depth-three sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural depth-three sum outer field `{}` does not name the required first record",
                    declared.name
                )));
            }
            let depth_two_paths = project_conventional_record_with_depth_two_nested_sums_materialization_layout_with_reachability(
                program,
                plan,
                named.symbol,
                reachability,
            )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared first record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its first record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-three first-record extent")?
                != depth_two_paths
                    .outer_layout
                    .size
                    .expect("plural depth-two projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "depth-three first-record alignment")?
                    != depth_two_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact first-record extent/alignment",
                    declared.name
                )));
            }
            for occurrence in &depth_two_paths.paths {
                total_leaf_paths = total_leaf_paths
                    .checked_add(occurrence.middle_paths.paths.len())
                    .ok_or_else(|| {
                        Diagnostic::error("plural depth-three leaf-path count overflows")
                    })?;
            }
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural depth-three paths exceed bounded total leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthThreeRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                depth_two_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural depth-three outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural depth-three sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthThreeRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural depth-three outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural depth-three outer record alignment",
            )?,
        },
        paths,
    })
}

/// Project the complete nonempty authored-order set of exact depth-four
/// record chains: `Outer -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-three
/// report for its exact second record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_four_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthFourRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_four_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

fn project_conventional_record_with_depth_four_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthFourRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "plural depth-four sum owner")?;
    validate_closed_copy_record(program, definition, "plural depth-four sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for plural depth-four sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for plural depth-four sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-four sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-four sum outer offsets exceed compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-four sum path report exceeds compiler resources")
        })?;
    let mut total_leaf_paths = 0usize;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "plural depth-four sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "plural depth-four sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural depth-four sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural depth-four sum outer field `{}` does not name the required second record",
                    declared.name
                )));
            }
            let depth_three_paths = project_conventional_record_with_depth_three_nested_sums_materialization_layout_with_reachability(
                program,
                plan,
                named.symbol,
                reachability,
            )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared second record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its second record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-four second-record extent")?
                != depth_three_paths
                    .outer_layout
                    .size
                    .expect("plural depth-three projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "depth-four second-record alignment")?
                    != depth_three_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact second-record extent/alignment",
                    declared.name
                )));
            }
            for second_occurrence in &depth_three_paths.paths {
                for first_occurrence in &second_occurrence.depth_two_paths.paths {
                    total_leaf_paths = total_leaf_paths
                        .checked_add(first_occurrence.middle_paths.paths.len())
                        .ok_or_else(|| {
                            Diagnostic::error("plural depth-four leaf-path count overflows")
                        })?;
                }
            }
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural depth-four paths exceed bounded total leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthFourRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                depth_three_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural depth-four outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural depth-four sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthFourRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural depth-four outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural depth-four outer record alignment",
            )?,
        },
        paths,
    })
}

/// Project the complete nonempty authored-order set of exact depth-five
/// record chains:
/// `Outer -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-four
/// report for its exact third record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_five_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthFiveRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_five_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

fn project_conventional_record_with_depth_five_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthFiveRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "plural depth-five sum owner")?;
    validate_closed_copy_record(program, definition, "plural depth-five sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for plural depth-five sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for plural depth-five sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-five sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-five sum outer offsets exceed compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-five sum path report exceeds compiler resources")
        })?;
    let mut total_leaf_paths = 0usize;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "plural depth-five sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "plural depth-five sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural depth-five sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural depth-five sum outer field `{}` does not name the required third record",
                    declared.name
                )));
            }
            let depth_four_paths = project_conventional_record_with_depth_four_nested_sums_materialization_layout_with_reachability(
                program,
                plan,
                named.symbol,
                reachability,
            )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared third record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its third record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-five third-record extent")?
                != depth_four_paths
                    .outer_layout
                    .size
                    .expect("plural depth-four projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "depth-five third-record alignment")?
                    != depth_four_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact third-record extent/alignment",
                    declared.name
                )));
            }
            for third_occurrence in &depth_four_paths.paths {
                for second_occurrence in &third_occurrence.depth_three_paths.paths {
                    for first_occurrence in &second_occurrence.depth_two_paths.paths {
                        total_leaf_paths = total_leaf_paths
                            .checked_add(first_occurrence.middle_paths.paths.len())
                            .ok_or_else(|| {
                                Diagnostic::error("plural depth-five leaf-path count overflows")
                            })?;
                    }
                }
            }
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural depth-five paths exceed bounded total leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthFiveRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                depth_four_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural depth-five outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural depth-five sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthFiveRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural depth-five outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural depth-five outer record alignment",
            )?,
        },
        paths,
    })
}

/// Project the complete nonempty authored-order set of exact depth-six
/// record chains:
/// `Outer -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-five
/// report for its exact fourth record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_six_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthSixRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "plural depth-six sum owner")?;
    validate_closed_copy_record(program, definition, "plural depth-six sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for plural depth-six sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for plural depth-six sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-six sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-six sum outer offsets exceed compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-six sum path report exceeds compiler resources")
        })?;
    let mut reachability = SumReachability::new(program);
    let mut total_leaf_paths = 0usize;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "plural depth-six sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "plural depth-six sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural depth-six sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural depth-six sum outer field `{}` does not name the required fourth record",
                    declared.name
                )));
            }
            let depth_five_paths = project_conventional_record_with_depth_five_nested_sums_materialization_layout_with_reachability(
                program,
                plan,
                named.symbol,
                &mut reachability,
            )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared fourth record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its fourth record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-six fourth-record extent")?
                != depth_five_paths
                    .outer_layout
                    .size
                    .expect("plural depth-five projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "depth-six fourth-record alignment")?
                    != depth_five_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact fourth-record extent/alignment",
                    declared.name
                )));
            }
            for fourth_occurrence in &depth_five_paths.paths {
                for third_occurrence in &fourth_occurrence.depth_four_paths.paths {
                    for second_occurrence in &third_occurrence.depth_three_paths.paths {
                        for first_occurrence in &second_occurrence.depth_two_paths.paths {
                            total_leaf_paths = total_leaf_paths
                                .checked_add(first_occurrence.middle_paths.paths.len())
                                .ok_or_else(|| {
                                    Diagnostic::error("plural depth-six leaf-path count overflows")
                                })?;
                        }
                    }
                }
            }
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural depth-six paths exceed bounded total leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthSixRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                depth_five_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural depth-six outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural depth-six sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthSixRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural depth-six outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural depth-six outer record alignment",
            )?,
        },
        paths,
    })
}

/// Project the complete nonempty authored-order set of exact fixed-depth
/// record chains: `Outer -> Middle -> Leaf -> direct conventional sums`.
///
/// Every sum-reachable outer field must name one exact middle record satisfying
/// the existing plural one-level judgment. The outer layout is retained once,
/// while each occurrence owns its middle layout and complete authored-order
/// leaf-path set without flattening child rows across either record boundary.
pub fn project_conventional_record_with_depth_two_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthTwoRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_two_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

fn project_conventional_record_with_depth_two_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthTwoRecordSumPathsLayoutReport, Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "depth-two sum owner")?;
    validate_closed_copy_record(program, definition, "depth-two sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for depth-two sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for depth-two sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("depth-two sum outer report exceeds compiler resources"))?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("depth-two sum outer offsets exceed compiler resources"))?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("depth-two sum path report exceeds compiler resources"))?;
    let mut total_middle_paths = 0usize;
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "depth-two sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }

        if reachability.type_contains_sum(declared.type_reference)? {
            if matches!(
                program
                    .type_reference_table
                    .type_reference(declared.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                return Err(Diagnostic::error(format!(
                    "depth-two sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "depth-two sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "depth-two sum outer field `{}` does not name the required middle record",
                    declared.name
                )));
            }
            let middle_paths = project_conventional_record_with_nested_sum_records_materialization_layout_with_reachability(
                program,
                plan,
                named.symbol,
                reachability,
            )?;
            let TypeLayoutDescriptor::Named {
                symbol: laid_symbol,
                name: laid_name,
            } = &laid.type_descriptor
            else {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` is not the exact declared middle record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its middle record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "middle record extent")?
                != middle_paths
                    .outer_layout
                    .size
                    .expect("middle projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "middle record alignment")?
                    != middle_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact middle record extent/alignment",
                    declared.name
                )));
            }
            total_middle_paths = total_middle_paths
                .checked_add(middle_paths.paths.len())
                .ok_or_else(|| {
                    Diagnostic::error("depth-two sum path occurrence count overflows")
                })?;
            if total_middle_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "depth-two sum paths exceed bounded total middle-to-leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthTwoRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                middle_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "depth-two outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "depth-two sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthTwoRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "depth-two outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "depth-two outer record alignment",
            )?,
        },
        paths,
    })
}

#[derive(Default)]
struct RecordSumProfile {
    direct: bool,
    array: bool,
    deeper: bool,
}

fn record_sum_profile(
    program: &CheckedTrees,
    definition: &DataDefinition,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordSumProfile, Diagnostic> {
    let mut profile = RecordSumProfile::default();
    for field in relevant_record_fields(program, definition) {
        match program
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named { .. } => {
                let Some(named) = exact_named_data(program, field.type_reference)? else {
                    continue;
                };
                match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                    DataShapeKind::Enum => profile.direct = true,
                    DataShapeKind::Record => {
                        if reachability.type_contains_sum(field.type_reference)? {
                            profile.deeper = true;
                        }
                    }
                    DataShapeKind::Mixed => profile.deeper = true,
                    DataShapeKind::Empty => {}
                }
            }
            TypeReferenceNode::FixedArray { .. } => {
                if reachability.type_contains_sum(field.type_reference)? {
                    profile.array = true;
                }
            }
            _ => {}
        }
    }
    Ok(profile)
}

#[derive(Clone, Copy)]
enum ReachabilityState {
    Visiting,
    Done(bool),
}

struct ReachabilityFrame<'a> {
    data: &'a DataDefinition,
    next_member: usize,
    found: bool,
}

struct SumReachability<'a> {
    program: &'a CheckedTrees,
    states: std::collections::HashMap<(u32, u32), ReachabilityState>,
    traversed_edges: usize,
}

impl<'a> SumReachability<'a> {
    const MAX_RECORDS: usize = 4096;
    const MAX_EDGES: usize = 16384;

    fn new(program: &'a CheckedTrees) -> Self {
        Self {
            program,
            states: std::collections::HashMap::new(),
            traversed_edges: 0,
        }
    }

    fn type_contains_sum(
        &mut self,
        mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Result<bool, Diagnostic> {
        let mut array_depth = 0usize;
        while let TypeReferenceNode::FixedArray { element_type, .. } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            array_depth += 1;
            if array_depth > 64 {
                return Err(Diagnostic::error(
                    "nested-record sum path exceeds bounded fixed-array depth",
                ));
            }
            type_reference = *element_type;
        }
        let Some(data) = exact_named_data(self.program, type_reference)? else {
            return Ok(false);
        };
        match DataDefinition::shape_kind_from_members(self.program.data_members(data)) {
            DataShapeKind::Enum | DataShapeKind::Mixed => Ok(true),
            DataShapeKind::Empty => Ok(false),
            DataShapeKind::Record => self.record_contains_sum(data),
        }
    }

    fn record_contains_sum(&mut self, root: &'a DataDefinition) -> Result<bool, Diagnostic> {
        let root_identity = symbol_identity(root.symbol)?;
        if let Some(state) = self.states.get(&root_identity) {
            return match state {
                ReachabilityState::Done(found) => Ok(*found),
                ReachabilityState::Visiting => Err(Diagnostic::error(format!(
                    "nested-record sum path is recursive through `{}`",
                    root.name
                ))),
            };
        }
        self.insert_state(root_identity, ReachabilityState::Visiting)?;
        let mut stack = Vec::new();
        stack.try_reserve(1).map_err(|_| {
            Diagnostic::error("nested-record sum traversal stack exceeds compiler resources")
        })?;
        stack.push(ReachabilityFrame {
            data: root,
            next_member: 0,
            found: false,
        });

        loop {
            let Some(frame) = stack.last_mut() else {
                unreachable!("root reachability frame returns when completed")
            };
            let members = self.program.data_members(frame.data);
            // `found` is the eventual answer, not permission to skip later
            // fields: a later branch can still expose a cycle, malformed
            // nominal identity, or resource-bound failure.
            if frame.next_member == members.len() {
                let completed = stack.pop().expect("active reachability frame");
                let identity = symbol_identity(completed.data.symbol)?;
                self.states
                    .insert(identity, ReachabilityState::Done(completed.found));
                if let Some(parent) = stack.last_mut() {
                    parent.found |= completed.found;
                    continue;
                }
                return Ok(completed.found);
            }
            let member = &members[frame.next_member];
            frame.next_member += 1;
            let DataMember::Field(field) = member else {
                frame.found = true;
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            self.traversed_edges = self.traversed_edges.checked_add(1).ok_or_else(|| {
                Diagnostic::error("nested-record sum traversal edge count overflows")
            })?;
            if self.traversed_edges > Self::MAX_EDGES {
                return Err(Diagnostic::error(
                    "nested-record sum path exceeds bounded schema traversal edges",
                ));
            }
            let mut child_type = field.type_reference;
            let mut array_depth = 0usize;
            while let TypeReferenceNode::FixedArray { element_type, .. } =
                self.program.type_reference_table.type_reference(child_type)
            {
                array_depth += 1;
                if array_depth > 64 {
                    return Err(Diagnostic::error(
                        "nested-record sum path exceeds bounded fixed-array depth",
                    ));
                }
                child_type = *element_type;
            }
            let Some(child) = exact_named_data(self.program, child_type)? else {
                continue;
            };
            match DataDefinition::shape_kind_from_members(self.program.data_members(child)) {
                DataShapeKind::Enum | DataShapeKind::Mixed => frame.found = true,
                DataShapeKind::Empty => {}
                DataShapeKind::Record => {
                    let identity = symbol_identity(child.symbol)?;
                    match self.states.get(&identity).copied() {
                        Some(ReachabilityState::Done(found)) => frame.found |= found,
                        Some(ReachabilityState::Visiting) => {
                            return Err(Diagnostic::error(format!(
                                "nested-record sum path is recursive through `{}`",
                                child.name
                            )));
                        }
                        None => {
                            self.insert_state(identity, ReachabilityState::Visiting)?;
                            stack.try_reserve(1).map_err(|_| {
                                Diagnostic::error(
                                    "nested-record sum traversal stack exceeds compiler resources",
                                )
                            })?;
                            stack.push(ReachabilityFrame {
                                data: child,
                                next_member: 0,
                                found: false,
                            });
                        }
                    }
                }
            }
        }
    }

    fn insert_state(
        &mut self,
        identity: (u32, u32),
        state: ReachabilityState,
    ) -> Result<(), Diagnostic> {
        if self.states.len() >= Self::MAX_RECORDS {
            return Err(Diagnostic::error(
                "nested-record sum path exceeds bounded schema traversal records",
            ));
        }
        self.states.try_reserve(1).map_err(|_| {
            Diagnostic::error("nested-record sum visited map exceeds compiler resources")
        })?;
        self.states.insert(identity, state);
        Ok(())
    }
}

fn symbol_identity(symbol: SymbolHandle) -> Result<(u32, u32), Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "nested-record sum path encountered an invalid nominal identity",
        ));
    }
    Ok((symbol.arena_index(), symbol.generation()))
}

fn validate_closed_copy_record(
    program: &CheckedTrees,
    definition: &DataDefinition,
    role: &str,
) -> Result<(), Diagnostic> {
    if !definition.symbol.is_valid()
        || definition.supply_mode != DataSupplyMode::CheckedShape
        || definition.properties.multiplicity != Multiplicity::Unrestricted
        || !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || DataDefinition::shape_kind_from_members(program.data_members(definition))
            != DataShapeKind::Record
    {
        return Err(Diagnostic::error(format!(
            "{role} `{}` must be one closed non-generic `[copy]` record",
            definition.name
        )));
    }
    Ok(())
}

fn relevant_record_fields<'a>(
    program: &'a CheckedTrees,
    definition: &'a DataDefinition,
) -> Vec<&'a psi_typed_trees::data::DataField> {
    program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect()
}

fn usize_to_u64(value: usize, role: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| Diagnostic::error(format!("{role} exceeds report width")))
}

/// Project the first compact fixed-array-of-conventional-sums rung.
///
/// The owner is the same exact closed `[copy]` record as the direct-field
/// projection, but it must contain exactly one runtime-relevant direct field
/// of type `[S; N]` where `N > 0` is literal and `S` is a conventional pure
/// sum. The complete sum layout is retained once with exact count/stride;
/// value-sensitive materialization retains the selected case separately for
/// each literal index.
pub fn project_conventional_record_with_sum_array_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<(LayoutPlanReport, ConventionalSumArrayFieldLayoutReport), Diagnostic> {
    let (outer, mut rows) = project_conventional_record_with_sum_arrays_materialization_layout(
        program,
        plan,
        data_symbol,
    )?;
    if rows.len() != 1 {
        return Err(Diagnostic::error(format!(
            "singular nested-sum array materialization requires exactly one direct field; found {}",
            rows.len()
        )));
    }
    Ok((outer, rows.pop().expect("exactly one array row")))
}

/// Project the complete authored-order set of direct nonzero literal
/// fixed-array-of-conventional-sum fields while retaining each complete sum
/// layout only once per outer field occurrence.
pub fn project_conventional_record_with_sum_arrays_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<(LayoutPlanReport, Vec<ConventionalSumArrayFieldLayoutReport>), Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "nested-sum array record")?;
    if definition.supply_mode != DataSupplyMode::CheckedShape
        || definition.properties.multiplicity != Multiplicity::Unrestricted
        || !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || DataDefinition::shape_kind_from_members(program.data_members(definition))
            != DataShapeKind::Record
    {
        return Err(Diagnostic::error(format!(
            "nested-sum array materialization owner `{}` must be one closed non-generic `[copy]` record",
            definition.name
        )));
    }

    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for nested-sum array owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for nested-sum array owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut array_reports = Vec::new();
    let mut entries = Vec::new();
    let mut offsets = Vec::new();
    array_reports
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error("nested-sum array report set exceeds compiler resources"))?;
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("nested-sum array outer entries exceed compiler resources")
        })?;
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("nested-sum array outer offsets exceed compiler resources")
        })?;
    let mut reachability = SumReachability::new(program);
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "nested-sum array outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }
        let mut selected_direct_array = false;
        match program
            .type_reference_table
            .type_reference(declared.type_reference)
        {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                if let Some(named) = exact_named_data(program, *element_type)? {
                    match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                        DataShapeKind::Enum => {
                            if *length == 0 {
                                return Err(Diagnostic::error(format!(
                                    "nested-sum array field `{}` must have nonzero literal length",
                                    declared.name
                                )));
                            }
                            let element_layout = project_conventional_sum_materialization_layout(
                                program,
                                plan,
                                named.symbol,
                            )?;
                            let TypeLayoutDescriptor::FixedArray {
                                element_type: laid_element,
                                length: laid_length,
                            } = &laid.type_descriptor
                            else {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` is not the exact declared fixed array",
                                    declared.name
                                )));
                            };
                            let TypeLayoutDescriptor::Named {
                                symbol: laid_symbol,
                                name: laid_name,
                            } = laid_element.as_ref()
                            else {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` substitutes its sum-array element type",
                                    declared.name
                                )));
                            };
                            if *laid_length != *length
                                || *laid_symbol != named.symbol
                                || laid.type_symbol != named.symbol
                                || laid_name.as_str() != named.name.as_str()
                            {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` substitutes its sum-array element/count",
                                    declared.name
                                )));
                            }
                            let stride = usize::try_from(element_layout.size).map_err(|_| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` element stride exceeds the compiler host",
                                    declared.name
                                ))
                            })?;
                            let element_count = u64::try_from(*length).map_err(|_| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` count exceeds canonical report width",
                                    declared.name
                                ))
                            })?;
                            let expected_size = stride.checked_mul(*length).ok_or_else(|| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` extent exceeds the compiler host",
                                    declared.name
                                ))
                            })?;
                            if laid.layout.size != expected_size
                                || laid.layout.alignment as u64 != element_layout.align
                            {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` does not retain the exact repeated conventional sum extent/alignment",
                                    declared.name
                                )));
                            }
                            array_reports.push(ConventionalSumArrayFieldLayoutReport {
                                field: declared.name.to_string(),
                                member_identity: declared.identity,
                                element_count,
                                element_stride: element_layout.size,
                                element_layout,
                            });
                            selected_direct_array = true;
                        }
                        DataShapeKind::Mixed => {
                            return Err(Diagnostic::error(format!(
                                "nested-sum array field `{}` uses mixed common-field/case elements",
                                declared.name
                            )));
                        }
                        DataShapeKind::Empty | DataShapeKind::Record => {}
                    }
                }
            }
            _ => {
                if let Some(named) = exact_named_data(program, declared.type_reference)?
                    && DataDefinition::shape_kind_from_members(program.data_members(named))
                        == DataShapeKind::Enum
                {
                    return Err(Diagnostic::error(
                        "nested-sum array materialization does not combine direct sum fields with the array occurrence",
                    ));
                }
            }
        }
        if !selected_direct_array && reachability.type_contains_sum(declared.type_reference)? {
            return Err(Diagnostic::error(format!(
                "nested-sum array outer field `{}` reaches a sum through a nested array or record",
                declared.name
            )));
        }
        let offset = laid.offset as u64;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if array_reports.is_empty() {
        return Err(Diagnostic::error(
            "nested-sum array layout projection requires a nonempty direct nonzero literal fixed-array-of-sums field set",
        ));
    }

    Ok((
        LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(data_layout.layout.size as u64),
            align: data_layout.layout.alignment as u64,
        },
        array_reports,
    ))
}

fn unique_data_definition<'a>(
    program: &'a CheckedTrees,
    data_symbol: SymbolHandle,
    role: &str,
) -> Result<&'a DataDefinition, Diagnostic> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == data_symbol);
    let definition = definitions.next().ok_or_else(|| {
        Diagnostic::error(format!("{role} names no exact checked data definition"))
    })?;
    if definitions.next().is_some() {
        return Err(Diagnostic::error(format!(
            "{role} data identity is ambiguous"
        )));
    }
    Ok(definition)
}

fn unique_data_layout<'a>(
    plan: &'a LayoutPlan,
    data_symbol: SymbolHandle,
    name: &str,
) -> Result<&'a crate::DataLayout, Diagnostic> {
    let mut layouts = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .filter(|layout| layout.symbol == data_symbol);
    let layout = layouts.next().ok_or_else(|| {
        Diagnostic::error(format!(
            "target runtime layout has no exact data row for `{name}`"
        ))
    })?;
    if layouts.next().is_some() {
        return Err(Diagnostic::error(format!(
            "target runtime layout has duplicate data rows for `{name}`"
        )));
    }
    Ok(layout)
}

fn exact_named_data<'a>(
    program: &'a CheckedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<Option<&'a DataDefinition>, Diagnostic> {
    if program.primitive_type_reference(type_reference).is_some() {
        return Ok(None);
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return Ok(None);
    };
    if !symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "nested-sum field type `{name}` has no exact nominal identity"
        )));
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == *symbol);
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() || definition.name.as_str() != name.as_str() {
        return Err(Diagnostic::error(format!(
            "nested-sum field type `{name}` has ambiguous or mismatched nominal identity"
        )));
    }
    Ok(Some(definition))
}

/// Project one exact closed pure sum from the already-built runtime layout.
/// Common-field/case mixed shapes reject and remain a separate materialization
/// rung.
pub fn project_conventional_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalSumLayoutReport, Diagnostic> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == data_symbol);
    let definition = definitions.next().ok_or_else(|| {
        Diagnostic::error("conventional sum layout names no exact checked data definition")
    })?;
    if definitions.next().is_some() {
        return Err(Diagnostic::error(
            "conventional sum layout data identity is ambiguous",
        ));
    }
    let members = program.data_members(definition);
    if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
        return Err(Diagnostic::error(format!(
            "conventional sum materialization requires a pure sum; `{}` is empty, a record, or a mixed common-field/case shape",
            definition.name
        )));
    }

    let mut layouts = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .filter(|layout| layout.symbol == data_symbol);
    let data_layout = layouts.next().ok_or_else(|| {
        Diagnostic::error(format!(
            "runtime layout has no exact data row for pure sum `{}`",
            definition.name
        ))
    })?;
    if layouts.next().is_some() {
        return Err(Diagnostic::error(format!(
            "runtime layout has duplicate data rows for pure sum `{}`",
            definition.name
        )));
    }
    let DataShape::Enum {
        common_fields,
        variants,
    } = &data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "runtime layout row for pure sum `{}` is not case-bearing",
            definition.name
        )));
    };
    if !plan.fields.span_or_empty(*common_fields).is_empty() {
        return Err(Diagnostic::error(format!(
            "pure sum `{}` unexpectedly retains common runtime fields",
            definition.name
        )));
    }

    let declared_cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_cases = plan.variants.span_or_empty(*variants);
    if declared_cases.len() != laid_cases.len() {
        return Err(Diagnostic::error(format!(
            "pure sum `{}` runtime layout has {} cases; checked schema has {}",
            definition.name,
            laid_cases.len(),
            declared_cases.len()
        )));
    }

    let cases = declared_cases
        .into_iter()
        .zip(laid_cases)
        .enumerate()
        .map(|(ordinal, (declared, laid))| {
            if declared.symbol != laid.symbol || declared.name != laid.name {
                return Err(Diagnostic::error(format!(
                    "pure sum `{}` runtime case order or identity drifted at ordinal {ordinal}",
                    definition.name
                )));
            }
            let declared_payload = program
                .data_payload_fields(declared)
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            let laid_payload = plan.fields.span_or_empty(laid.fields);
            if declared_payload.len() != laid_payload.len() {
                return Err(Diagnostic::error(format!(
                    "pure sum `{}` case `{}` runtime payload has {} fields; checked schema has {} relevant fields",
                    definition.name,
                    declared.name,
                    laid_payload.len(),
                    declared_payload.len()
                )));
            }
            let payload_fields = declared_payload
                .into_iter()
                .zip(laid_payload)
                .map(|(declared_field, laid_field)| {
                    if declared_field.symbol != laid_field.symbol
                        || declared_field.name != laid_field.name
                    {
                        return Err(Diagnostic::error(format!(
                            "pure sum `{}` case `{}` runtime payload field identity or order drifted",
                            definition.name, declared.name
                        )));
                    }
                    Ok(ConventionalSumPayloadFieldLayoutReport {
                        field: declared_field.name.to_string(),
                        member_identity: declared_field.identity,
                        offset: laid_field.offset as u64,
                        size: laid_field.layout.size as u64,
                        align: laid_field.layout.alignment as u64,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(ConventionalSumCaseLayoutReport {
                case: declared.name.to_string(),
                member_identity: declared.identity,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "pure sum `{}` case ordinal exceeds u32",
                        definition.name
                    ))
                })?,
                payload_fields,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(ConventionalSumLayoutReport {
        schema_report_fingerprint: psi_typed_trees::identity::normalized_schema_report_fingerprint(
            program, definition,
        ),
        tag_offset: 0,
        tag_size: ENUM_TAG_BYTES as u64,
        tag_align: ENUM_TAG_BYTES as u64,
        cases,
        size: data_layout.layout.size as u64,
        align: data_layout.layout.alignment as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::NativeTarget;
    use psi_build_time_evaluation::{
        BuildTimeValue, validate_const_materializable_conventional_sum,
        validate_const_materializable_record_with_conventional_sum,
        validate_const_materializable_record_with_conventional_sum_array,
        validate_const_materializable_record_with_conventional_sum_arrays,
        validate_const_materializable_record_with_depth_five_nested_sums,
        validate_const_materializable_record_with_depth_four_nested_sums,
        validate_const_materializable_record_with_depth_six_nested_sums,
        validate_const_materializable_record_with_depth_three_nested_sum,
        validate_const_materializable_record_with_depth_three_nested_sums,
        validate_const_materializable_record_with_depth_two_nested_sum,
        validate_const_materializable_record_with_depth_two_nested_sums,
        validate_const_materializable_record_with_nested_sum_record,
        validate_const_materializable_record_with_nested_sum_records,
    };
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_layout_plans::{ByteOrder, normalized_conventional_sum_layout_report_fingerprint};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        CheckedTrees::with_roots(typed, CheckFacts::default())
    }

    #[test]
    fn projects_exact_authored_case_order_and_overlay_geometry() {
        let checked = checked(
            r#"
            data Choice [copy] {
                case Empty;
                case Number(value: u8, proof [erased]: u64);
                case Pair(left: u16, right: u32);
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Choice")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let report =
            project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
                .unwrap();

        assert_eq!(report.tag_offset, 0);
        assert_eq!(report.tag_size, 4);
        assert_eq!(report.tag_align, 4);
        assert_eq!(report.size, 12);
        assert_eq!(report.align, 4);
        assert_eq!(
            report
                .cases
                .iter()
                .map(|case| (case.case.as_str(), case.ordinal))
                .collect::<Vec<_>>(),
            [("Empty", 0), ("Number", 1), ("Pair", 2)]
        );
        assert!(report.cases[0].payload_fields.is_empty());
        assert_eq!(report.cases[1].payload_fields[0].offset, 4);
        assert_eq!(
            report.cases[2]
                .payload_fields
                .iter()
                .map(|field| (field.field.as_str(), field.offset, field.size))
                .collect::<Vec<_>>(),
            [("left", 4, 2), ("right", 8, 4)]
        );
        assert_ne!(
            normalized_conventional_sum_layout_report_fingerprint(&report),
            0
        );

        let value = BuildTimeValue::Case {
            variant: "Pair".into(),
            payload: vec![
                ("left".into(), BuildTimeValue::Int(0x1122)),
                ("right".into(), BuildTimeValue::Int(0x3344_5566)),
            ],
        };
        let materialized = validate_const_materializable_conventional_sum(
            &checked,
            "Choice",
            &report,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("authoritative runtime report should materialize its active case");
        assert_eq!(
            materialized.bytes(),
            &[2, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33]
        );
    }

    #[test]
    fn mixed_common_field_shape_is_not_projected_as_a_pure_sum() {
        let checked = checked(
            r#"
            data Event [copy] {
                sequence: u8;
                case Ready(value: u16);
                case Waiting;
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Event")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let error =
            project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
                .unwrap_err();
        assert!(error.message.contains("pure sum"));
    }

    #[test]
    fn target_path_projects_and_replays_one_inner_record_with_complete_direct_sums() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u8); }
            data Inner [copy] { #1 first: Choice; #2 marker: u16; #3 second: Choice; }
            data Outer [copy] { #1 prefix: u8; #2 inner: Inner; #3 suffix: u16; }
            "#,
        );
        let outer = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .unwrap();
        let inner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Inner")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let path = project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("one exact target path should project");
        assert_eq!(path.outer_field, "inner");
        assert_eq!(path.outer_member_identity, Some(2));
        assert_eq!(path.outer_layout.offsets.as_deref(), Some(&[0, 4, 24][..]));
        assert_eq!(path.inner_layout.offsets.as_deref(), Some(&[0, 8, 12][..]));
        assert_eq!(
            path.child_sum_layouts
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );

        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(0xaa)),
                (
                    "inner".into(),
                    BuildTimeValue::Struct {
                        type_name: "Inner".into(),
                        fields: vec![
                            (
                                "first".into(),
                                BuildTimeValue::Case {
                                    variant: "Empty".into(),
                                    payload: Vec::new(),
                                },
                            ),
                            ("marker".into(), BuildTimeValue::Int(0x1122)),
                            (
                                "second".into(),
                                BuildTimeValue::Case {
                                    variant: "Number".into(),
                                    payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                                },
                            ),
                        ],
                    },
                ),
                ("suffix".into(), BuildTimeValue::Int(0x3344)),
            ],
        };
        let carrier = validate_const_materializable_record_with_nested_sum_record(
            &checked,
            "Outer",
            &path,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("target path should feed exact nested-record materialization");
        assert_eq!(carrier.inner().nested_sums().len(), 2);
        assert_eq!(
            carrier.inner().nested_sums()[0]
                .nested_sum()
                .selected_case_ordinal(),
            0
        );
        assert_eq!(
            carrier.inner().nested_sums()[1]
                .nested_sum()
                .selected_case_ordinal(),
            1
        );
        assert_eq!(
            carrier.bytes(),
            &[
                0xaa, 0, 0, 0, // outer prefix padding
                0, 0, 0, 0, 0, 0, 0, 0, // first Empty
                0x22, 0x11, 0, 0, // marker plus inner padding
                1, 0, 0, 0, 0x5c, 0, 0, 0, // second Number
                0x44, 0x33, 0, 0, // outer suffix and padding
            ]
        );
        carrier
            .replay_against(&checked, "Outer", &path, &value, ByteOrder::LittleEndian)
            .expect("the complete path should replay exactly");

        let mut renamed_reports = path.clone();
        renamed_reports.outer_field = "renamed_inner".into();
        renamed_reports.outer_layout.entries[1].field = "renamed_inner".into();
        renamed_reports.inner_layout.entries[0].field = "renamed_first".into();
        renamed_reports.child_sum_layouts[0].field = "renamed_first".into();
        carrier
            .replay_against(
                &checked,
                "Outer",
                &renamed_reports,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("stable-numbered report names are presentation-only");

        let mut wrong_outer_identity = path.clone();
        wrong_outer_identity.outer_member_identity = path.outer_layout.entries[0].member_identity;
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &wrong_outer_identity,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut wrong_inner_layout = path.clone();
        wrong_inner_layout.inner_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 10 };
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &wrong_inner_layout,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut missing_child = path.clone();
        missing_child.child_sum_layouts.pop();
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &missing_child,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut extra_child = path.clone();
        extra_child
            .child_sum_layouts
            .push(path.child_sum_layouts[0].clone());
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &extra_child,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut duplicate_child = path.clone();
        duplicate_child.child_sum_layouts[1] = path.child_sum_layouts[0].clone();
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &duplicate_child,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut wrong_child_identity = path.clone();
        wrong_child_identity.child_sum_layouts[0].member_identity =
            path.child_sum_layouts[1].member_identity;
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &wrong_child_identity,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut reordered_children = path.clone();
        reordered_children.child_sum_layouts.swap(0, 1);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &reordered_children,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut wrong_child_geometry = path.clone();
        wrong_child_geometry.child_sum_layouts[1].layout.cases[1].payload_fields[0].offset += 1;
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &wrong_child_geometry,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        let mut short = [0xa5; 27];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0xa5; 27]);

        let inner_layout = unique_data_layout(&plan, inner.symbol, "Inner").unwrap();
        let DataShape::Record {
            fields: inner_fields,
        } = inner_layout.shape
        else {
            unreachable!("fixture inner is a record")
        };
        let mut substituted_child_plan = plan.clone();
        substituted_child_plan
            .fields
            .span_mut_or_empty(inner_fields)[0]
            .type_descriptor = TypeLayoutDescriptor::Unit;
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &substituted_child_plan,
                outer.symbol,
            )
            .is_err(),
            "a child sum descriptor substitution must reject"
        );

        let outer_data_layout = unique_data_layout(&plan, outer.symbol, "Outer").unwrap();
        let DataShape::Record {
            fields: outer_fields,
        } = outer_data_layout.shape
        else {
            unreachable!("fixture outer is a record")
        };
        let mut substituted_outer_plan = plan.clone();
        substituted_outer_plan
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Unit;
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &substituted_outer_plan,
                outer.symbol,
            )
            .is_err(),
            "the exact outer-to-inner descriptor must rejoin"
        );

        let outer_field_symbol = plan.fields.span_or_empty(outer_fields)[0].symbol;
        let mut repeated_outer_plan = plan.clone();
        repeated_outer_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: outer_field_symbol,
                element_stride: 1,
            });
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &repeated_outer_plan,
                outer.symbol,
            )
            .is_err(),
            "target-dependent placement on any outer field must reject"
        );
        let inner_field_symbol = plan.fields.span_or_empty(inner_fields)[1].symbol;
        let mut repeated_inner_plan = plan.clone();
        repeated_inner_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: inner_field_symbol,
                element_stride: 2,
            });
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &repeated_inner_plan,
                outer.symbol,
            )
            .is_err(),
            "target-dependent placement on any inner field must reject"
        );
    }

    #[test]
    fn nested_record_path_projection_fences_competing_and_deeper_sum_shapes() {
        let checked = checked(
            r#"
            data Choice [copy] { case Empty; case Number(value: u8); }
            data Inner [copy] { choice: Choice; }
            data Deep [copy] { inner: Inner; }
            data ArrayInner [copy] { choices: [Choice; 2]; }
            data DirectOuter [copy] { inner: Inner; direct: Choice; }
            data TwoInner [copy] { first: Inner; second: Inner; }
            data ArrayChild [copy] { inner: Inner; array: [Choice; 1]; }
            data DeeperChild [copy] { inner: Inner; deeper: Deep; }
            data OuterArraySibling [copy] { inner: Inner; sibling: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        for name in [
            "DirectOuter",
            "TwoInner",
            "ArrayChild",
            "DeeperChild",
            "OuterArraySibling",
        ] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_nested_sum_record_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the singular one-level path cohort"
            );
        }

        let outer = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ArrayInner")
            .unwrap();
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err()
        );
    }

    #[test]
    fn plural_nested_record_paths_retain_complete_ordered_occurrences_and_replay_atomically() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u8); }
            data Inner [copy] {
                #1 choice: Choice;
                #2 marker: u16;
                #3 backup: Choice;
            }
            data Outer [copy] {
                #1 prefix: u8;
                #2 first: Inner;
                #3 between: u16;
                #4 second: Inner;
                #5 suffix: u8;
            }
            "#,
        );
        let outer = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let paths = project_conventional_record_with_nested_sum_records_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete authored-order path set should project");
        assert_eq!(
            paths.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 24, 28, 48][..])
        );
        assert_eq!(paths.outer_layout.size, Some(52));
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(paths.paths[0].outer_field, "first");
        assert_eq!(paths.paths[1].outer_field, "second");
        assert_eq!(paths.paths[0].outer_member_identity, Some(2));
        assert_eq!(paths.paths[1].outer_member_identity, Some(4));
        assert_eq!(paths.paths[0].inner_layout, paths.paths[1].inner_layout);
        assert_eq!(paths.paths[0].child_sum_layouts.len(), 2);
        assert_eq!(paths.paths[1].child_sum_layouts.len(), 2);
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the singular compatibility projection must fail closed on two occurrences"
        );

        let empty = || BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let number = |value| BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(0xaa)),
                (
                    "first".into(),
                    BuildTimeValue::Struct {
                        type_name: "Inner".into(),
                        fields: vec![
                            ("choice".into(), empty()),
                            ("marker".into(), BuildTimeValue::Int(0x1122)),
                            ("backup".into(), number(0x3a)),
                        ],
                    },
                ),
                ("between".into(), BuildTimeValue::Int(0x3344)),
                (
                    "second".into(),
                    BuildTimeValue::Struct {
                        type_name: "Inner".into(),
                        fields: vec![
                            ("choice".into(), number(0x5c)),
                            ("marker".into(), BuildTimeValue::Int(0x5566)),
                            ("backup".into(), empty()),
                        ],
                    },
                ),
                ("suffix".into(), BuildTimeValue::Int(0x77)),
            ],
        };
        let singular_first = ConventionalNestedRecordSumPathLayoutReport {
            outer_layout: paths.outer_layout.clone(),
            outer_field: paths.paths[0].outer_field.clone(),
            outer_member_identity: paths.paths[0].outer_member_identity,
            inner_layout: paths.paths[0].inner_layout.clone(),
            child_sum_layouts: paths.paths[0].child_sum_layouts.clone(),
        };
        assert!(
            validate_const_materializable_record_with_nested_sum_record(
                &checked,
                "Outer",
                &singular_first,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "the singular consumer must not discard the second qualifying occurrence"
        );
        let carrier = validate_const_materializable_record_with_nested_sum_records(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("two same-type occurrences should retain independent selected sums");
        assert_eq!(carrier.inner_records().len(), 2);
        assert_eq!(
            carrier.inner_records()[0].inner().nested_sums()[0]
                .nested_sum()
                .selected_case_ordinal(),
            0
        );
        assert_eq!(
            carrier.inner_records()[1].inner().nested_sums()[0]
                .nested_sum()
                .selected_case_ordinal(),
            1
        );
        assert_eq!(
            carrier.bytes(),
            &[
                0xaa, 0, 0, 0, // prefix padding
                0, 0, 0, 0, 0, 0, 0, 0, // first.choice Empty
                0x22, 0x11, 0, 0, // first.marker + padding
                1, 0, 0, 0, 0x3a, 0, 0, 0, // first.backup Number
                0x44, 0x33, 0, 0, // between + outer padding
                1, 0, 0, 0, 0x5c, 0, 0, 0, // second.choice Number
                0x66, 0x55, 0, 0, // second.marker + padding
                0, 0, 0, 0, 0, 0, 0, 0, // second.backup Empty
                0x77, 0, 0, 0, // suffix + tail padding
            ]
        );
        carrier
            .replay_against(&checked, "Outer", &paths, &value, ByteOrder::LittleEndian)
            .expect("the complete plural path report should replay");
        let mut destination = [0xa5; 56];
        carrier
            .apply(&checked, &mut destination)
            .expect("all occurrences replay before one outer copy");
        assert_eq!(&destination[..52], carrier.bytes());
        assert_eq!(&destination[52..], &[0xa5; 4]);
        let mut short = [0x5a; 51];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x5a; 51]);

        let mut renamed = paths.clone();
        renamed.outer_layout.entries[1].field = "renamed_first".into();
        renamed.outer_layout.entries[3].field = "renamed_second".into();
        renamed.paths[0].outer_field = "renamed_first".into();
        renamed.paths[1].outer_field = "renamed_second".into();
        renamed.paths[0].inner_layout.entries[0].field = "renamed_choice".into();
        renamed.paths[0].child_sum_layouts[0].field = "renamed_choice".into();
        carrier
            .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
            .expect("stable-numbered outer and child names remain presentation-only");

        let rejects = |mutated: &psi_layout_plans::ConventionalNestedRecordSumPathsLayoutReport| {
            assert!(
                carrier
                    .replay_against(&checked, "Outer", mutated, &value, ByteOrder::LittleEndian,)
                    .is_err()
            );
        };
        let mut missing_path = paths.clone();
        missing_path.paths.pop();
        rejects(&missing_path);
        let mut extra_path = paths.clone();
        extra_path.paths.push(paths.paths[0].clone());
        rejects(&extra_path);
        let mut reordered_paths = paths.clone();
        reordered_paths.paths.swap(0, 1);
        rejects(&reordered_paths);
        let mut duplicate_path = paths.clone();
        duplicate_path.paths[1] = paths.paths[0].clone();
        rejects(&duplicate_path);
        let mut wrong_path_identity = paths.clone();
        wrong_path_identity.paths[0].outer_member_identity = paths.paths[1].outer_member_identity;
        rejects(&wrong_path_identity);
        let mut missing_child = paths.clone();
        missing_child.paths[0].child_sum_layouts.pop();
        rejects(&missing_child);
        let mut extra_child = paths.clone();
        extra_child.paths[0]
            .child_sum_layouts
            .push(paths.paths[0].child_sum_layouts[0].clone());
        rejects(&extra_child);
        let mut reordered_children = paths.clone();
        reordered_children.paths[0].child_sum_layouts.swap(0, 1);
        rejects(&reordered_children);
        let mut duplicate_child = paths.clone();
        duplicate_child.paths[0].child_sum_layouts[1] = paths.paths[0].child_sum_layouts[0].clone();
        rejects(&duplicate_child);
        let mut wrong_child_identity = paths.clone();
        wrong_child_identity.paths[0].child_sum_layouts[0].member_identity =
            paths.paths[0].child_sum_layouts[1].member_identity;
        rejects(&wrong_child_identity);
        let mut wrong_outer_layout = paths.clone();
        wrong_outer_layout.outer_layout.entries[2].placement =
            LayoutPlacementReport::At { offset: 26 };
        rejects(&wrong_outer_layout);
        let mut wrong_inner_layout = paths.clone();
        wrong_inner_layout.paths[1].inner_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 10 };
        rejects(&wrong_inner_layout);
        let mut wrong_child_geometry = paths.clone();
        wrong_child_geometry.paths[1].child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_child_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian,)
                .is_err()
        );
        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is outer record")
        };
        let BuildTimeValue::Struct { fields, .. } = &mut fields[3].1 else {
            unreachable!("second occurrence is inner record")
        };
        fields[0].1 = empty();
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &paths,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
    }

    #[test]
    fn depth_two_record_chain_projects_replays_and_fails_closed_on_every_adjacent_shape() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] {
                #1 lead: u8;
                #2 first: Choice;
                #3 marker: u16;
                #4 second: Choice;
                #5 tail: u8;
            }
            data Middle [copy] { #1 lead: u8; #2 leaf: Leaf; #3 tail: u16; }
            data Outer [copy] { #1 lead: u8; #2 middle: Middle; #3 tail: u16; }

            data OuterTwo [copy] { first: Middle; second: Middle; }
            data MiddleTwo [copy] { first: Leaf; second: Leaf; }
            data OuterMiddleTwo [copy] { middle: MiddleTwo; }
            data OuterShallow [copy] { leaf: Leaf; }
            data Deep [copy] { middle: Middle; }
            data OuterDeep [copy] { deep: Deep; }
            data OuterDirect [copy] { middle: Middle; direct: Choice; }
            data MiddleDirect [copy] { leaf: Leaf; direct: Choice; }
            data OuterMiddleDirect [copy] { middle: MiddleDirect; }
            data OuterArray [copy] { middle: Middle; choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let path = project_conventional_record_with_depth_two_nested_sum_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("one exact depth-two chain should project compositionally");
        assert_eq!(path.outer_layout.offsets.as_deref(), Some(&[0, 4, 40][..]));
        assert_eq!(path.outer_layout.size, Some(44));
        assert_eq!(path.outer_field, "middle");
        assert_eq!(path.outer_member_identity, Some(2));
        assert_eq!(
            path.middle_path.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 32][..])
        );
        assert_eq!(path.middle_path.outer_layout.size, Some(36));
        assert_eq!(path.middle_path.outer_field, "leaf");
        assert_eq!(path.middle_path.outer_member_identity, Some(2));
        assert_eq!(
            path.middle_path.inner_layout.offsets.as_deref(),
            Some(&[0, 4, 12, 16, 24][..])
        );
        assert_eq!(path.middle_path.inner_layout.size, Some(28));
        assert_eq!(
            path.middle_path
                .child_sum_layouts
                .iter()
                .map(|row| (row.field.as_str(), row.member_identity))
                .collect::<Vec<_>>(),
            [("first", Some(2)), ("second", Some(4))]
        );

        let empty = || BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let number = |value| BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xdd)),
                (
                    "middle".into(),
                    BuildTimeValue::Struct {
                        type_name: "Middle".into(),
                        fields: vec![
                            ("lead".into(), BuildTimeValue::Int(0xcc)),
                            (
                                "leaf".into(),
                                BuildTimeValue::Struct {
                                    type_name: "Leaf".into(),
                                    fields: vec![
                                        ("lead".into(), BuildTimeValue::Int(0xaa)),
                                        ("first".into(), empty()),
                                        ("marker".into(), BuildTimeValue::Int(0x1122)),
                                        ("second".into(), number(0x3344)),
                                        ("tail".into(), BuildTimeValue::Int(0xbb)),
                                    ],
                                },
                            ),
                            ("tail".into(), BuildTimeValue::Int(0x5566)),
                        ],
                    },
                ),
                ("tail".into(), BuildTimeValue::Int(0x7788)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_two_nested_sum(
            &checked,
            "Outer",
            &path,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced depth-two report should rejoin value custody");
        assert_eq!(
            carrier.bytes(),
            &[
                0xdd, 0, 0, 0, 0xcc, 0, 0, 0, 0xaa, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x22, 0x11, 0,
                0, 1, 0, 0, 0, 0x44, 0x33, 0, 0, 0xbb, 0, 0, 0, 0x66, 0x55, 0, 0, 0x88, 0x77, 0, 0,
            ]
        );
        assert_eq!(carrier.middle().inner().nested_sums().len(), 2);

        let mut destination = [0x5a; 48];
        carrier
            .apply(&checked, &mut destination)
            .expect("the complete outer image should copy atomically");
        assert_eq!(&destination[..44], carrier.bytes());
        assert_eq!(&destination[44..], &[0x5a; 4]);
        let mut short = [0x6b; 43];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 43]);

        let mut renamed = path.clone();
        renamed.outer_layout.entries[1].field = "renamed_middle".into();
        renamed.outer_field = "renamed_middle".into();
        renamed.middle_path.outer_layout.entries[1].field = "renamed_leaf".into();
        renamed.middle_path.outer_field = "renamed_leaf".into();
        renamed.middle_path.inner_layout.entries[1].field = "renamed_first".into();
        renamed.middle_path.inner_layout.entries[3].field = "renamed_second".into();
        renamed.middle_path.child_sum_layouts[0].field = "renamed_first".into();
        renamed.middle_path.child_sum_layouts[1].field = "renamed_second".into();
        carrier
            .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
            .expect("stable-numbered names at all three layers are presentation-only");

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthTwoRecordSumPathLayoutReport| {
                assert!(
                carrier
                    .replay_against(
                        &checked,
                        "Outer",
                        mutated,
                        &value,
                        ByteOrder::LittleEndian,
                    )
                    .is_err()
            );
            };
        let mut wrong_outer_identity = path.clone();
        wrong_outer_identity.outer_member_identity = Some(3);
        rejects(&wrong_outer_identity);
        let mut wrong_middle_identity = path.clone();
        wrong_middle_identity.middle_path.outer_member_identity = Some(3);
        rejects(&wrong_middle_identity);
        let mut wrong_leaf_identity = path.clone();
        wrong_leaf_identity.middle_path.child_sum_layouts[0].member_identity = Some(4);
        rejects(&wrong_leaf_identity);
        let mut wrong_outer_geometry = path.clone();
        wrong_outer_geometry.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_outer_geometry);
        let mut wrong_middle_geometry = path.clone();
        wrong_middle_geometry.middle_path.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_middle_geometry);
        let mut wrong_leaf_geometry = path.clone();
        wrong_leaf_geometry.middle_path.inner_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_leaf_geometry);
        let mut missing_child = path.clone();
        missing_child.middle_path.child_sum_layouts.pop();
        rejects(&missing_child);
        let mut reordered_children = path.clone();
        reordered_children.middle_path.child_sum_layouts.swap(0, 1);
        rejects(&reordered_children);
        let mut wrong_child_geometry = path.clone();
        wrong_child_geometry.middle_path.child_sum_layouts[1]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_child_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &path, &value, ByteOrder::BigEndian)
                .is_err()
        );

        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &plan,
            definition("Middle").symbol,
        )
        .expect("the preexisting one-level singular producer remains unchanged");
        for name in [
            "OuterTwo",
            "OuterMiddleTwo",
            "OuterShallow",
            "OuterDeep",
            "OuterDirect",
            "OuterMiddleDirect",
            "OuterArray",
        ] {
            assert!(
                project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} must remain outside the exact-one depth-two chain"
            );
        }

        let outer_layout = unique_data_layout(&plan, outer.symbol, "Outer").unwrap();
        let DataShape::Record {
            fields: outer_fields,
        } = outer_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let middle_layout =
            unique_data_layout(&plan, definition("Middle").symbol, "Middle").unwrap();
        let DataShape::Record {
            fields: middle_fields,
        } = middle_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let leaf_layout = unique_data_layout(&plan, definition("Leaf").symbol, "Leaf").unwrap();
        let DataShape::Record {
            fields: leaf_fields,
        } = leaf_layout.shape
        else {
            unreachable!("fixture is a record")
        };

        let middle = definition("Middle");
        let leaf = definition("Leaf");
        let mut wrong_top_type_symbol = plan.clone();
        wrong_top_type_symbol.fields.span_mut_or_empty(outer_fields)[1].type_symbol = leaf.symbol;
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &wrong_top_type_symbol,
                outer.symbol,
            )
            .is_err(),
            "the top edge must rejoin the exact laid type symbol"
        );
        let mut wrong_top_descriptor_symbol = plan.clone();
        wrong_top_descriptor_symbol
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: leaf.symbol,
            name: middle.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &wrong_top_descriptor_symbol,
                outer.symbol,
            )
            .is_err(),
            "the top edge must rejoin the exact descriptor symbol"
        );
        let mut wrong_top_descriptor_name = plan.clone();
        wrong_top_descriptor_name
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: middle.symbol,
            name: leaf.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &wrong_top_descriptor_name,
                outer.symbol,
            )
            .is_err(),
            "the top edge must rejoin the exact descriptor spelling"
        );

        let recursive_type = checked
            .data_members(middle)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "tail" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: middle.symbol,
                    name: middle.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "a sum-reachable middle record cycle must fail through bounded Visiting-state detection"
        );

        for field_symbol in [
            plan.fields.span_or_empty(outer_fields)[1].symbol,
            plan.fields.span_or_empty(middle_fields)[1].symbol,
            plan.fields.span_or_empty(leaf_fields)[0].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every record layer must reject"
            );
        }
    }

    #[test]
    fn depth_three_record_chain_composes_existing_custody_and_fails_closed() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] {
                #1 lead: u8;
                #2 first: Choice;
                #3 marker: u16;
                #4 second: Choice;
                #5 tail: u8;
            }
            data Middle [copy] { #1 lead: u8; #2 leaf: Leaf; #3 tail: u16; }
            data First [copy] { #1 lead: u8; #2 middle: Middle; #3 tail: u16; }
            data Outer [copy] { #1 lead: u8; #2 first: First; #3 tail: u16; }

            data OuterTwo [copy] { first: First; second: First; }
            data FirstTwo [copy] { first: Middle; second: Middle; }
            data OuterFirstTwo [copy] { first: FirstTwo; }
            data MiddleTwo [copy] { first: Leaf; second: Leaf; }
            data FirstMiddleTwo [copy] { middle: MiddleTwo; }
            data OuterMiddleTwo [copy] { first: FirstMiddleTwo; }
            data OuterShallow [copy] { middle: Middle; }
            data Deep [copy] { first: First; }
            data OuterDeep [copy] { deep: Deep; }
            data OuterDirect [copy] { first: First; direct: Choice; }
            data OuterArray [copy] { first: First; choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let first = definition("First");
        let middle = definition("Middle");
        let leaf = definition("Leaf");
        let path = project_conventional_record_with_depth_three_nested_sum_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("one exact depth-three chain should project compositionally");
        assert_eq!(path.outer_layout.offsets.as_deref(), Some(&[0, 4, 48][..]));
        assert_eq!(path.outer_layout.size, Some(52));
        assert_eq!(path.outer_field, "first");
        assert_eq!(path.outer_member_identity, Some(2));
        assert_eq!(
            path.depth_two_path.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 40][..])
        );
        assert_eq!(path.depth_two_path.outer_layout.size, Some(44));
        assert_eq!(path.depth_two_path.outer_field, "middle");
        assert_eq!(
            path.depth_two_path
                .middle_path
                .outer_layout
                .offsets
                .as_deref(),
            Some(&[0, 4, 32][..])
        );
        assert_eq!(
            path.depth_two_path
                .middle_path
                .inner_layout
                .offsets
                .as_deref(),
            Some(&[0, 4, 12, 16, 24][..])
        );
        assert_eq!(path.depth_two_path.middle_path.child_sum_layouts.len(), 2);

        let empty = || BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let number = |value| BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xee)),
                (
                    "first".into(),
                    BuildTimeValue::Struct {
                        type_name: "First".into(),
                        fields: vec![
                            ("lead".into(), BuildTimeValue::Int(0xdd)),
                            (
                                "middle".into(),
                                BuildTimeValue::Struct {
                                    type_name: "Middle".into(),
                                    fields: vec![
                                        ("lead".into(), BuildTimeValue::Int(0xcc)),
                                        (
                                            "leaf".into(),
                                            BuildTimeValue::Struct {
                                                type_name: "Leaf".into(),
                                                fields: vec![
                                                    ("lead".into(), BuildTimeValue::Int(0xaa)),
                                                    ("first".into(), empty()),
                                                    ("marker".into(), BuildTimeValue::Int(0x1122)),
                                                    ("second".into(), number(0x3344)),
                                                    ("tail".into(), BuildTimeValue::Int(0xbb)),
                                                ],
                                            },
                                        ),
                                        ("tail".into(), BuildTimeValue::Int(0x5566)),
                                    ],
                                },
                            ),
                            ("tail".into(), BuildTimeValue::Int(0x7788)),
                        ],
                    },
                ),
                ("tail".into(), BuildTimeValue::Int(0x99aa)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_three_nested_sum(
            &checked,
            "Outer",
            &path,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced depth-three report should rejoin value custody");
        let mut expected = vec![0; 52];
        expected[0] = 0xee;
        expected[4] = 0xdd;
        expected[8] = 0xcc;
        expected[12] = 0xaa;
        expected[24..26].copy_from_slice(&0x1122_u16.to_le_bytes());
        expected[28..32].copy_from_slice(&1_u32.to_le_bytes());
        expected[32..34].copy_from_slice(&0x3344_u16.to_le_bytes());
        expected[36] = 0xbb;
        expected[40..42].copy_from_slice(&0x5566_u16.to_le_bytes());
        expected[44..46].copy_from_slice(&0x7788_u16.to_le_bytes());
        expected[48..50].copy_from_slice(&0x99aa_u16.to_le_bytes());
        assert_eq!(carrier.bytes(), expected);
        assert_eq!(carrier.inner().middle().inner().nested_sums().len(), 2);

        let big_endian = validate_const_materializable_record_with_depth_three_nested_sum(
            &checked,
            "Outer",
            &path,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("the new outer staging must retain target byte order");
        let mut expected_big = expected.clone();
        expected_big[24..26].copy_from_slice(&0x1122_u16.to_be_bytes());
        expected_big[28..32].copy_from_slice(&1_u32.to_be_bytes());
        expected_big[32..34].copy_from_slice(&0x3344_u16.to_be_bytes());
        expected_big[40..42].copy_from_slice(&0x5566_u16.to_be_bytes());
        expected_big[44..46].copy_from_slice(&0x7788_u16.to_be_bytes());
        expected_big[48..50].copy_from_slice(&0x99aa_u16.to_be_bytes());
        assert_eq!(big_endian.bytes(), expected_big);

        let mut destination = [0x5a; 56];
        carrier
            .apply(&checked, &mut destination)
            .expect("the complete depth-three image should copy atomically");
        assert_eq!(&destination[..52], carrier.bytes());
        assert_eq!(&destination[52..], &[0x5a; 4]);
        let mut short = [0x6b; 51];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 51]);

        let mut renamed = path.clone();
        renamed.outer_layout.entries[1].field = "renamed_first".into();
        renamed.outer_field = "renamed_first".into();
        renamed.depth_two_path.outer_layout.entries[1].field = "renamed_middle".into();
        renamed.depth_two_path.outer_field = "renamed_middle".into();
        renamed.depth_two_path.middle_path.outer_layout.entries[1].field = "renamed_leaf".into();
        renamed.depth_two_path.middle_path.outer_field = "renamed_leaf".into();
        renamed.depth_two_path.middle_path.inner_layout.entries[1].field = "renamed_sum".into();
        renamed.depth_two_path.middle_path.child_sum_layouts[0].field = "renamed_sum".into();
        carrier
            .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
            .expect("stable-numbered names at every record edge are presentation-only");

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthThreeRecordSumPathLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "Outer",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut wrong_outer_identity = path.clone();
        wrong_outer_identity.outer_member_identity = Some(3);
        rejects(&wrong_outer_identity);
        let mut wrong_first_identity = path.clone();
        wrong_first_identity.depth_two_path.outer_member_identity = Some(3);
        rejects(&wrong_first_identity);
        let mut wrong_middle_identity = path.clone();
        wrong_middle_identity
            .depth_two_path
            .middle_path
            .outer_member_identity = Some(3);
        rejects(&wrong_middle_identity);
        let mut wrong_leaf_identity = path.clone();
        wrong_leaf_identity
            .depth_two_path
            .middle_path
            .child_sum_layouts[0]
            .member_identity = Some(4);
        rejects(&wrong_leaf_identity);
        let mut wrong_outer_geometry = path.clone();
        wrong_outer_geometry.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_outer_geometry);
        let mut wrong_first_geometry = path.clone();
        wrong_first_geometry.depth_two_path.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_first_geometry);
        let mut wrong_middle_geometry = path.clone();
        wrong_middle_geometry
            .depth_two_path
            .middle_path
            .outer_layout
            .entries[1]
            .placement = LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_middle_geometry);
        let mut wrong_leaf_geometry = path.clone();
        wrong_leaf_geometry
            .depth_two_path
            .middle_path
            .inner_layout
            .entries[1]
            .placement = LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_leaf_geometry);
        let mut wrong_child_geometry = path.clone();
        wrong_child_geometry
            .depth_two_path
            .middle_path
            .child_sum_layouts[1]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_child_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &path, &value, ByteOrder::BigEndian)
                .is_err()
        );
        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is an outer record")
        };
        fields[2].1 = BuildTimeValue::Int(0x99ab);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &path,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &plan,
                first.symbol,
            )
            .is_ok(),
            "the existing depth-two API must retain its exact prior cohort"
        );
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the existing depth-two API must not widen to the new depth-three root"
        );
        for name in [
            "OuterTwo",
            "OuterFirstTwo",
            "OuterMiddleTwo",
            "OuterShallow",
            "OuterDeep",
            "OuterDirect",
            "OuterArray",
        ] {
            assert!(
                project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} must remain outside the exact-one depth-three cohort"
            );
        }

        let outer_layout = unique_data_layout(&plan, outer.symbol, "Outer").unwrap();
        let DataShape::Record {
            fields: outer_fields,
        } = outer_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let first_layout = unique_data_layout(&plan, first.symbol, "First").unwrap();
        let DataShape::Record {
            fields: first_fields,
        } = first_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let middle_layout = unique_data_layout(&plan, middle.symbol, "Middle").unwrap();
        let DataShape::Record {
            fields: middle_fields,
        } = middle_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let leaf_layout = unique_data_layout(&plan, leaf.symbol, "Leaf").unwrap();
        let DataShape::Record {
            fields: leaf_fields,
        } = leaf_layout.shape
        else {
            unreachable!("fixture is a record")
        };

        let mut wrong_top_type_symbol = plan.clone();
        wrong_top_type_symbol.fields.span_mut_or_empty(outer_fields)[1].type_symbol = middle.symbol;
        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &checked,
                &wrong_top_type_symbol,
                outer.symbol,
            )
            .is_err()
        );
        let mut wrong_top_descriptor_symbol = plan.clone();
        wrong_top_descriptor_symbol
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: middle.symbol,
            name: first.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &checked,
                &wrong_top_descriptor_symbol,
                outer.symbol,
            )
            .is_err()
        );
        let mut wrong_top_descriptor_name = plan.clone();
        wrong_top_descriptor_name
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: first.symbol,
            name: middle.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &checked,
                &wrong_top_descriptor_name,
                outer.symbol,
            )
            .is_err()
        );

        for field_symbol in [
            plan.fields.span_or_empty(outer_fields)[1].symbol,
            plan.fields.span_or_empty(first_fields)[1].symbol,
            plan.fields.span_or_empty(middle_fields)[1].symbol,
            plan.fields.span_or_empty(leaf_fields)[1].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every record layer must reject"
            );
        }

        let recursive_type = checked
            .data_members(first)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "tail" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: first.symbol,
                    name: first.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "recursion after the admitted middle path must fail through bounded reachability"
        );
        assert!(
            validate_const_materializable_record_with_depth_three_nested_sum(
                &recursive_checked,
                "Outer",
                &path,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "consumer replay must also scan recursion after the admitted middle path"
        );
    }

    #[test]
    fn plural_depth_three_paths_compose_existing_plural_custody_and_fail_closed() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] { #1 choice: Choice; }
            data Middle [copy] { #1 leaf: Leaf; }
            data First [copy] { #1 middle: Middle; #2 tail: u8; }
            data FirstTwo [copy] { #1 left: Middle; #2 right: Middle; }
            data OuterPlural [copy] {
                #1 lead: u8;
                #2 first: First;
                #3 pair: FirstTwo;
                #4 tail: u16;
            }

            data OuterSingular [copy] { #1 first: First; }
            data OuterShallow [copy] { #1 middle: Middle; }
            data Deeper [copy] { #1 first: First; }
            data OuterDeep [copy] { #1 deeper: Deeper; }
            data OuterDirect [copy] { #1 first: First; #2 direct: Choice; }
            data OuterArray [copy] { #1 first: First; #2 choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("OuterPlural");
        let first = definition("First");
        let first_two = definition("FirstTwo");
        let middle = definition("Middle");
        let leaf = definition("Leaf");
        let paths =
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .expect("the complete plural depth-three path set should project");
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(paths.paths[0].outer_field, "first");
        assert_eq!(paths.paths[0].outer_member_identity, Some(2));
        assert_eq!(paths.paths[0].depth_two_paths.paths.len(), 1);
        assert_eq!(paths.paths[1].outer_field, "pair");
        assert_eq!(paths.paths[1].outer_member_identity, Some(3));
        assert_eq!(paths.paths[1].depth_two_paths.paths.len(), 2);
        assert_eq!(
            paths.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 16, 32][..])
        );
        assert_eq!(paths.outer_layout.size, Some(36));

        let empty = || BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let number = |value| BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let leaf_value = |choice| BuildTimeValue::Struct {
            type_name: "Leaf".into(),
            fields: vec![("choice".into(), choice)],
        };
        let middle_value = |choice| BuildTimeValue::Struct {
            type_name: "Middle".into(),
            fields: vec![("leaf".into(), leaf_value(choice))],
        };
        let value = BuildTimeValue::Struct {
            type_name: "OuterPlural".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xaa)),
                (
                    "first".into(),
                    BuildTimeValue::Struct {
                        type_name: "First".into(),
                        fields: vec![
                            ("middle".into(), middle_value(number(0x1122))),
                            ("tail".into(), BuildTimeValue::Int(0xbb)),
                        ],
                    },
                ),
                (
                    "pair".into(),
                    BuildTimeValue::Struct {
                        type_name: "FirstTwo".into(),
                        fields: vec![
                            ("left".into(), middle_value(empty())),
                            ("right".into(), middle_value(number(0x3344))),
                        ],
                    },
                ),
                ("tail".into(), BuildTimeValue::Int(0x5566)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_three_nested_sums(
            &checked,
            "OuterPlural",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced plural report should rejoin complete value custody");
        assert_eq!(carrier.occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
        assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 2);
        let mut expected = vec![0; 36];
        expected[0] = 0xaa;
        expected[4..8].copy_from_slice(&1_u32.to_le_bytes());
        expected[8..10].copy_from_slice(&0x1122_u16.to_le_bytes());
        expected[12] = 0xbb;
        expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
        expected[28..30].copy_from_slice(&0x3344_u16.to_le_bytes());
        expected[32..34].copy_from_slice(&0x5566_u16.to_le_bytes());
        assert_eq!(carrier.bytes(), expected);

        let big_endian = validate_const_materializable_record_with_depth_three_nested_sums(
            &checked,
            "OuterPlural",
            &paths,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("plural nested staging retains target byte order");
        let mut expected_big = expected.clone();
        expected_big[4..8].copy_from_slice(&1_u32.to_be_bytes());
        expected_big[8..10].copy_from_slice(&0x1122_u16.to_be_bytes());
        expected_big[24..28].copy_from_slice(&1_u32.to_be_bytes());
        expected_big[28..30].copy_from_slice(&0x3344_u16.to_be_bytes());
        expected_big[32..34].copy_from_slice(&0x5566_u16.to_be_bytes());
        assert_eq!(big_endian.bytes(), expected_big);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "OuterPlural",
                    &paths,
                    &value,
                    ByteOrder::BigEndian,
                )
                .is_err()
        );

        let mut destination = [0x5a; 40];
        carrier
            .apply(&checked, &mut destination)
            .expect("plural custody should copy one complete outer image");
        assert_eq!(&destination[..36], carrier.bytes());
        assert_eq!(&destination[36..], &[0x5a; 4]);
        let mut short = [0x6b; 35];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 35]);

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthThreeRecordSumPathsLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "OuterPlural",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut missing = paths.clone();
        missing.paths.pop();
        rejects(&missing);
        let mut extra = paths.clone();
        extra.paths.push(paths.paths[0].clone());
        rejects(&extra);
        let mut reordered = paths.clone();
        reordered.paths.swap(0, 1);
        rejects(&reordered);
        let mut duplicate = paths.clone();
        duplicate.paths[1] = duplicate.paths[0].clone();
        rejects(&duplicate);
        let mut missing_inner = paths.clone();
        missing_inner.paths[1].depth_two_paths.paths.pop();
        rejects(&missing_inner);
        let mut wrong_outer_identity = paths.clone();
        wrong_outer_identity.paths[1].outer_member_identity = Some(2);
        rejects(&wrong_outer_identity);
        let mut wrong_first_identity = paths.clone();
        wrong_first_identity.paths[1].depth_two_paths.paths[1].outer_member_identity = Some(1);
        rejects(&wrong_first_identity);
        let mut wrong_middle_identity = paths.clone();
        wrong_middle_identity.paths[1].depth_two_paths.paths[1]
            .middle_paths
            .paths[0]
            .outer_member_identity = Some(2);
        rejects(&wrong_middle_identity);
        let mut wrong_leaf_identity = paths.clone();
        wrong_leaf_identity.paths[1].depth_two_paths.paths[1]
            .middle_paths
            .paths[0]
            .child_sum_layouts[0]
            .member_identity = Some(2);
        rejects(&wrong_leaf_identity);
        let mut wrong_outer_geometry = paths.clone();
        wrong_outer_geometry.outer_layout.entries[2].placement =
            LayoutPlacementReport::At { offset: 20 };
        rejects(&wrong_outer_geometry);
        let mut wrong_first_geometry = paths.clone();
        wrong_first_geometry.paths[1]
            .depth_two_paths
            .outer_layout
            .entries[1]
            .placement = LayoutPlacementReport::At { offset: 12 };
        rejects(&wrong_first_geometry);
        let mut wrong_middle_geometry = paths.clone();
        wrong_middle_geometry.paths[1].depth_two_paths.paths[1]
            .middle_paths
            .outer_layout
            .entries[0]
            .placement = LayoutPlacementReport::At { offset: 4 };
        rejects(&wrong_middle_geometry);
        let mut wrong_leaf_geometry = paths.clone();
        wrong_leaf_geometry.paths[1].depth_two_paths.paths[1]
            .middle_paths
            .paths[0]
            .inner_layout
            .entries[0]
            .placement = LayoutPlacementReport::At { offset: 4 };
        rejects(&wrong_leaf_geometry);
        let mut wrong_sum_geometry = paths.clone();
        wrong_sum_geometry.paths[1].depth_two_paths.paths[1]
            .middle_paths
            .paths[0]
            .child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_sum_geometry);

        let mut renamed = paths.clone();
        renamed.outer_layout.entries[1].field = "renamed_first".into();
        renamed.paths[0].outer_field = "renamed_first".into();
        renamed.outer_layout.entries[2].field = "renamed_pair".into();
        renamed.paths[1].outer_field = "renamed_pair".into();
        renamed.paths[1].depth_two_paths.outer_layout.entries[1].field = "renamed_right".into();
        renamed.paths[1].depth_two_paths.paths[1].outer_field = "renamed_right".into();
        carrier
            .replay_against(
                &checked,
                "OuterPlural",
                &renamed,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("stable-numbered names remain presentation-only at plural boundaries");

        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is an outer record")
        };
        fields[3].1 = BuildTimeValue::Int(0x5567);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "OuterPlural",
                    &paths,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the singular depth-three API must remain exact-one"
        );
        assert!(
            project_conventional_record_with_depth_three_nested_sum_materialization_layout(
                &checked,
                &plan,
                definition("OuterSingular").symbol,
            )
            .is_ok(),
            "the preexisting singular depth-three cohort remains unchanged"
        );
        for name in ["OuterShallow", "OuterDeep", "OuterDirect", "OuterArray"] {
            assert!(
                project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} remains outside the exact plural depth-three cohort"
            );
        }

        let outer_layout = unique_data_layout(&plan, outer.symbol, "OuterPlural").unwrap();
        let DataShape::Record {
            fields: outer_fields,
        } = outer_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let first_two_layout = unique_data_layout(&plan, first_two.symbol, "FirstTwo").unwrap();
        let DataShape::Record {
            fields: first_two_fields,
        } = first_two_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let middle_layout = unique_data_layout(&plan, middle.symbol, "Middle").unwrap();
        let DataShape::Record {
            fields: middle_fields,
        } = middle_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let leaf_layout = unique_data_layout(&plan, leaf.symbol, "Leaf").unwrap();
        let DataShape::Record {
            fields: leaf_fields,
        } = leaf_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let mut wrong_top_type_symbol = plan.clone();
        wrong_top_type_symbol.fields.span_mut_or_empty(outer_fields)[2].type_symbol = first.symbol;
        assert!(
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &checked,
                &wrong_top_type_symbol,
                outer.symbol,
            )
            .is_err(),
            "the second plural edge must rejoin the exact laid type symbol"
        );
        let mut wrong_top_descriptor = plan.clone();
        wrong_top_descriptor.fields.span_mut_or_empty(outer_fields)[2].type_descriptor =
            TypeLayoutDescriptor::Named {
                symbol: first_two.symbol,
                name: first.name.clone(),
            };
        assert!(
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &checked,
                &wrong_top_descriptor,
                outer.symbol,
            )
            .is_err(),
            "the second plural edge must rejoin its exact descriptor spelling"
        );
        for field_symbol in [
            plan.fields.span_or_empty(outer_fields)[2].symbol,
            plan.fields.span_or_empty(first_two_fields)[1].symbol,
            plan.fields.span_or_empty(middle_fields)[0].symbol,
            plan.fields.span_or_empty(leaf_fields)[0].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every layer remains fenced"
            );
        }

        let recursive_type = checked
            .data_members(first)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "tail" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: first.symbol,
                    name: first.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "a later recursive sibling must reject through shared bounded reachability"
        );
        assert!(
            validate_const_materializable_record_with_depth_three_nested_sums(
                &recursive_checked,
                "OuterPlural",
                &paths,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "consumer replay must independently retain the recursion fence"
        );
    }

    #[test]
    fn plural_depth_four_paths_compose_depth_three_custody_and_retain_fences() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] { #1 choice: Choice; }
            data Middle [copy] { #1 leaf: Leaf; }
            data First [copy] { #1 middle: Middle; }
            data Second [copy] { #1 first: First; #2 alternate: First; #3 tail: u8; }
            data Outer [copy] {
                #1 lead: u8;
                #2 left: Second;
                #3 right: Second;
                #4 tail: u16;
            }

            data OuterShallow [copy] { #1 first: First; }
            data Third [copy] { #1 second: Second; }
            data OuterTooDeep [copy] { #1 third: Third; }
            data OuterDirect [copy] { #1 second: Second; #2 direct: Choice; }
            data OuterArray [copy] { #1 second: Second; #2 choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let second = definition("Second");
        let paths = project_conventional_record_with_depth_four_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete depth-four occurrence cohort should project");
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(paths.paths[0].outer_field, "left");
        assert_eq!(paths.paths[0].outer_member_identity, Some(2));
        assert_eq!(paths.paths[1].outer_field, "right");
        assert_eq!(paths.paths[1].outer_member_identity, Some(3));
        assert_eq!(paths.paths[0].depth_three_paths.paths.len(), 2);
        assert_eq!(paths.paths[1].depth_three_paths.paths.len(), 2);
        assert_eq!(
            paths.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 24, 44][..])
        );
        assert_eq!(paths.outer_layout.size, Some(48));

        let choice = |number: Option<u16>| match number {
            Some(value) => BuildTimeValue::Case {
                variant: "Number".into(),
                payload: vec![("value".into(), BuildTimeValue::Int(i64::from(value)))],
            },
            None => BuildTimeValue::Case {
                variant: "Empty".into(),
                payload: Vec::new(),
            },
        };
        let first_value = |selected| BuildTimeValue::Struct {
            type_name: "First".into(),
            fields: vec![(
                "middle".into(),
                BuildTimeValue::Struct {
                    type_name: "Middle".into(),
                    fields: vec![(
                        "leaf".into(),
                        BuildTimeValue::Struct {
                            type_name: "Leaf".into(),
                            fields: vec![("choice".into(), selected)],
                        },
                    )],
                },
            )],
        };
        let second_value = |first_choice, alternate_choice, tail| BuildTimeValue::Struct {
            type_name: "Second".into(),
            fields: vec![
                ("first".into(), first_value(first_choice)),
                ("alternate".into(), first_value(alternate_choice)),
                ("tail".into(), BuildTimeValue::Int(tail)),
            ],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xaa)),
                (
                    "left".into(),
                    second_value(choice(None), choice(Some(0x1122)), 0xbb),
                ),
                (
                    "right".into(),
                    second_value(choice(Some(0x3344)), choice(None), 0xcc),
                ),
                ("tail".into(), BuildTimeValue::Int(0x5566)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_four_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("depth-four value custody should compose the depth-three carriers");
        assert_eq!(carrier.occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 2);
        let mut expected = vec![0; 48];
        expected[0] = 0xaa;
        expected[12..16].copy_from_slice(&1_u32.to_le_bytes());
        expected[16..18].copy_from_slice(&0x1122_u16.to_le_bytes());
        expected[20] = 0xbb;
        expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
        expected[28..30].copy_from_slice(&0x3344_u16.to_le_bytes());
        expected[40] = 0xcc;
        expected[44..46].copy_from_slice(&0x5566_u16.to_le_bytes());
        assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

        let mut destination = [0x5a; 52];
        carrier
            .apply(&checked, &mut destination)
            .expect("complete replay should permit one atomic copy");
        assert_eq!(&destination[..48], carrier.bytes());
        assert_eq!(&destination[48..], &[0x5a; 4]);
        let mut short = [0x6b; 47];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 47]);

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthFourRecordSumPathsLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "Outer",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut missing = paths.clone();
        missing.paths.pop();
        rejects(&missing);
        let mut reordered = paths.clone();
        reordered.paths.swap(0, 1);
        rejects(&reordered);
        let mut wrong_outer_identity = paths.clone();
        wrong_outer_identity.paths[0].outer_member_identity = Some(3);
        rejects(&wrong_outer_identity);
        let mut wrong_inner_identity = paths.clone();
        wrong_inner_identity.paths[0].depth_three_paths.paths[1].outer_member_identity = Some(1);
        rejects(&wrong_inner_identity);
        let mut wrong_leaf_geometry = paths.clone();
        wrong_leaf_geometry.paths[0].depth_three_paths.paths[1]
            .depth_two_paths
            .paths[0]
            .middle_paths
            .paths[0]
            .child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_leaf_geometry);
        let mut wrong_outer_geometry = paths.clone();
        wrong_outer_geometry.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_outer_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
                .is_err()
        );

        for name in ["OuterShallow", "OuterTooDeep", "OuterDirect", "OuterArray"] {
            assert!(
                project_conventional_record_with_depth_four_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} remains outside the exact depth-four cohort"
            );
        }

        let record_fields = |name: &str| {
            let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
            let DataShape::Record { fields } = layout.shape else {
                unreachable!("fixture owner is a record")
            };
            fields
        };
        for field_symbol in [
            plan.fields.span_or_empty(record_fields("Outer"))[1].symbol,
            plan.fields.span_or_empty(record_fields("Second"))[0].symbol,
            plan.fields.span_or_empty(record_fields("First"))[0].symbol,
            plan.fields.span_or_empty(record_fields("Middle"))[0].symbol,
            plan.fields.span_or_empty(record_fields("Leaf"))[0].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_four_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every layer remains fenced"
            );
        }

        let recursive_type = checked
            .data_members(second)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "tail" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: second.symbol,
                    name: second.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_four_nested_sums_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err()
        );
        assert!(
            validate_const_materializable_record_with_depth_four_nested_sums(
                &recursive_checked,
                "Outer",
                &paths,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );

        assert!(
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &checked,
                &plan,
                second.symbol,
            )
            .is_ok(),
            "the unchanged plural depth-three API retains its prior cohort"
        );
        assert!(
            project_conventional_record_with_depth_three_nested_sums_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the prior API must not widen to the new depth-four root"
        );
    }

    #[test]
    fn plural_depth_five_paths_compose_depth_four_custody_and_retain_fences() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] { #1 choice: Choice; }
            data Middle [copy] { #1 leaf: Leaf; }
            data First [copy] { #1 middle: Middle; }
            data Second [copy] { #1 first: First; #2 alternate: First; #3 tail: u8; }
            data Third [copy] { #1 first: Second; #2 alternate: Second; #3 tail: u8; }
            data Outer [copy] {
                #1 lead: u8;
                #2 left: Third;
                #3 right: Third;
                #4 tail: u16;
            }

            data OuterShallow [copy] { #1 second: Second; }
            data Fourth [copy] { #1 third: Third; }
            data OuterTooDeep [copy] { #1 fourth: Fourth; }
            data OuterDirect [copy] { #1 third: Third; #2 direct: Choice; }
            data OuterArray [copy] { #1 third: Third; #2 choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let third = definition("Third");
        let paths = project_conventional_record_with_depth_five_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete depth-five occurrence cohort should project");
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(paths.paths[0].outer_field, "left");
        assert_eq!(paths.paths[0].outer_member_identity, Some(2));
        assert_eq!(paths.paths[1].outer_field, "right");
        assert_eq!(paths.paths[1].outer_member_identity, Some(3));
        assert_eq!(paths.paths[0].depth_four_paths.paths.len(), 2);
        assert_eq!(paths.paths[1].depth_four_paths.paths.len(), 2);
        assert_eq!(
            paths.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 48, 92][..])
        );
        assert_eq!(paths.outer_layout.size, Some(96));

        let choice = |number: Option<u16>| match number {
            Some(value) => BuildTimeValue::Case {
                variant: "Number".into(),
                payload: vec![("value".into(), BuildTimeValue::Int(i64::from(value)))],
            },
            None => BuildTimeValue::Case {
                variant: "Empty".into(),
                payload: Vec::new(),
            },
        };
        let first_value = |selected| BuildTimeValue::Struct {
            type_name: "First".into(),
            fields: vec![(
                "middle".into(),
                BuildTimeValue::Struct {
                    type_name: "Middle".into(),
                    fields: vec![(
                        "leaf".into(),
                        BuildTimeValue::Struct {
                            type_name: "Leaf".into(),
                            fields: vec![("choice".into(), selected)],
                        },
                    )],
                },
            )],
        };
        let second_value = |first_choice, alternate_choice, tail| BuildTimeValue::Struct {
            type_name: "Second".into(),
            fields: vec![
                ("first".into(), first_value(first_choice)),
                ("alternate".into(), first_value(alternate_choice)),
                ("tail".into(), BuildTimeValue::Int(tail)),
            ],
        };
        let third_value = |first, alternate, tail| BuildTimeValue::Struct {
            type_name: "Third".into(),
            fields: vec![
                ("first".into(), first),
                ("alternate".into(), alternate),
                ("tail".into(), BuildTimeValue::Int(tail)),
            ],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xaa)),
                (
                    "left".into(),
                    third_value(
                        second_value(choice(None), choice(Some(0x1122)), 0xb1),
                        second_value(choice(Some(0x3344)), choice(None), 0xb2),
                        0xb3,
                    ),
                ),
                (
                    "right".into(),
                    third_value(
                        second_value(choice(Some(0x5566)), choice(Some(0x7788)), 0xc1),
                        second_value(choice(None), choice(Some(0x99aa)), 0xc2),
                        0xc3,
                    ),
                ),
                ("tail".into(), BuildTimeValue::Int(0xbbcc)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_five_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("depth-five value custody should compose the depth-four carriers");
        assert_eq!(carrier.occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 2);
        let mut expected = vec![0; 96];
        expected[0] = 0xaa;
        expected[12..16].copy_from_slice(&1_u32.to_le_bytes());
        expected[16..18].copy_from_slice(&0x1122_u16.to_le_bytes());
        expected[20] = 0xb1;
        expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
        expected[28..30].copy_from_slice(&0x3344_u16.to_le_bytes());
        expected[40] = 0xb2;
        expected[44] = 0xb3;
        expected[48..52].copy_from_slice(&1_u32.to_le_bytes());
        expected[52..54].copy_from_slice(&0x5566_u16.to_le_bytes());
        expected[56..60].copy_from_slice(&1_u32.to_le_bytes());
        expected[60..62].copy_from_slice(&0x7788_u16.to_le_bytes());
        expected[64] = 0xc1;
        expected[76..80].copy_from_slice(&1_u32.to_le_bytes());
        expected[80..82].copy_from_slice(&0x99aa_u16.to_le_bytes());
        expected[84] = 0xc2;
        expected[88] = 0xc3;
        expected[92..94].copy_from_slice(&0xbbcc_u16.to_le_bytes());
        assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

        let big_endian = validate_const_materializable_record_with_depth_five_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("the same complete cohort should stage in big-endian order");
        assert_eq!(&big_endian.bytes()[12..16], &1_u32.to_be_bytes());
        assert_eq!(&big_endian.bytes()[16..18], &0x1122_u16.to_be_bytes());
        assert_ne!(
            carrier.non_authoritative_materialization_report_fingerprint(),
            big_endian.non_authoritative_materialization_report_fingerprint()
        );

        let mut destination = [0x5a; 100];
        carrier
            .apply(&checked, &mut destination)
            .expect("complete replay should permit one atomic copy");
        assert_eq!(&destination[..96], carrier.bytes());
        assert_eq!(&destination[96..], &[0x5a; 4]);
        let mut short = [0x6b; 95];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 95]);

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthFiveRecordSumPathsLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "Outer",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut missing = paths.clone();
        missing.paths.pop();
        rejects(&missing);
        let mut extra = paths.clone();
        extra.paths.push(extra.paths[0].clone());
        rejects(&extra);
        let mut reordered = paths.clone();
        reordered.paths.swap(0, 1);
        rejects(&reordered);
        let mut wrong_outer_identity = paths.clone();
        wrong_outer_identity.paths[0].outer_member_identity = Some(3);
        rejects(&wrong_outer_identity);
        let mut wrong_inner_identity = paths.clone();
        wrong_inner_identity.paths[0].depth_four_paths.paths[1].outer_member_identity = Some(1);
        rejects(&wrong_inner_identity);
        let mut wrong_leaf_geometry = paths.clone();
        wrong_leaf_geometry.paths[0].depth_four_paths.paths[1]
            .depth_three_paths
            .paths[0]
            .depth_two_paths
            .paths[0]
            .middle_paths
            .paths[0]
            .child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_leaf_geometry);
        let mut wrong_child_extent = paths.clone();
        wrong_child_extent.paths[0]
            .depth_four_paths
            .outer_layout
            .size = Some(48);
        rejects(&wrong_child_extent);
        let mut wrong_child_alignment = paths.clone();
        wrong_child_alignment.paths[0]
            .depth_four_paths
            .outer_layout
            .align = 8;
        rejects(&wrong_child_alignment);
        let mut wrong_outer_geometry = paths.clone();
        wrong_outer_geometry.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_outer_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
                .is_err()
        );
        let mut changed_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut changed_value else {
            unreachable!()
        };
        fields[0].1 = BuildTimeValue::Int(0xab);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &paths,
                    &changed_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        for name in ["OuterShallow", "OuterTooDeep", "OuterDirect", "OuterArray"] {
            assert!(
                project_conventional_record_with_depth_five_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} remains outside the exact depth-five cohort"
            );
        }

        let record_fields = |name: &str| {
            let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
            let DataShape::Record { fields } = layout.shape else {
                unreachable!("fixture owner is a record")
            };
            fields
        };
        for field_symbol in [
            plan.fields.span_or_empty(record_fields("Outer"))[1].symbol,
            plan.fields.span_or_empty(record_fields("Third"))[0].symbol,
            plan.fields.span_or_empty(record_fields("Second"))[0].symbol,
            plan.fields.span_or_empty(record_fields("First"))[0].symbol,
            plan.fields.span_or_empty(record_fields("Middle"))[0].symbol,
            plan.fields.span_or_empty(record_fields("Leaf"))[0].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_five_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every record layer remains fenced"
            );
        }

        let recursive_type = checked
            .data_members(third)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "tail" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: third.symbol,
                    name: third.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_five_nested_sums_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err()
        );
        assert!(
            validate_const_materializable_record_with_depth_five_nested_sums(
                &recursive_checked,
                "Outer",
                &paths,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );

        assert!(
            project_conventional_record_with_depth_four_nested_sums_materialization_layout(
                &checked,
                &plan,
                third.symbol,
            )
            .is_ok(),
            "the unchanged plural depth-four API retains its prior cohort"
        );
        assert!(
            project_conventional_record_with_depth_four_nested_sums_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the prior API must not widen to the new depth-five root"
        );
    }

    #[test]
    fn plural_depth_six_paths_compose_depth_five_custody_and_retain_fences() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] { #1 choice: Choice; }
            data Middle [copy] { #1 leaf: Leaf; }
            data First [copy] { #1 middle: Middle; }
            data Second [copy] { #1 first: First; }
            data Third [copy] { #1 second: Second; }
            data Fourth [copy] { #1 third: Third; }
            data Outer [copy] { #1 left: Fourth; #2 right: Fourth; }

            data OuterTooDeep [copy] { #1 outer: Outer; }
            data OuterDirect [copy] { #1 fourth: Fourth; #2 direct: Choice; }
            data OuterArray [copy] { #1 fourth: Fourth; #2 choices: [Choice; 1]; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let fourth = definition("Fourth");
        let paths = project_conventional_record_with_depth_six_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete depth-six occurrence cohort should project");
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(paths.paths[0].outer_field, "left");
        assert_eq!(paths.paths[0].outer_member_identity, Some(1));
        assert_eq!(paths.paths[1].outer_field, "right");
        assert_eq!(paths.paths[1].outer_member_identity, Some(2));
        assert_eq!(paths.paths[0].depth_five_paths.paths.len(), 1);
        assert_eq!(paths.paths[1].depth_five_paths.paths.len(), 1);
        assert_eq!(paths.outer_layout.offsets.as_deref(), Some(&[0, 8][..]));
        assert_eq!(paths.outer_layout.size, Some(16));

        let choice = |number: Option<u16>| match number {
            Some(value) => BuildTimeValue::Case {
                variant: "Number".into(),
                payload: vec![("value".into(), BuildTimeValue::Int(i64::from(value)))],
            },
            None => BuildTimeValue::Case {
                variant: "Empty".into(),
                payload: Vec::new(),
            },
        };
        let leaf = |choice| BuildTimeValue::Struct {
            type_name: "Leaf".into(),
            fields: vec![("choice".into(), choice)],
        };
        let middle = |choice| BuildTimeValue::Struct {
            type_name: "Middle".into(),
            fields: vec![("leaf".into(), leaf(choice))],
        };
        let first = |choice| BuildTimeValue::Struct {
            type_name: "First".into(),
            fields: vec![("middle".into(), middle(choice))],
        };
        let second = |choice| BuildTimeValue::Struct {
            type_name: "Second".into(),
            fields: vec![("first".into(), first(choice))],
        };
        let third = |choice| BuildTimeValue::Struct {
            type_name: "Third".into(),
            fields: vec![("second".into(), second(choice))],
        };
        let fourth_value = |choice| BuildTimeValue::Struct {
            type_name: "Fourth".into(),
            fields: vec![("third".into(), third(choice))],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("left".into(), fourth_value(choice(None))),
                ("right".into(), fourth_value(choice(Some(0x1122)))),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_six_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("depth-six value custody should compose the depth-five carriers");
        assert_eq!(carrier.occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
        assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
        let mut expected = vec![0; 16];
        expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
        expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
        assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

        let big_endian = validate_const_materializable_record_with_depth_six_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("the same complete cohort should stage in big-endian order");
        assert_eq!(&big_endian.bytes()[8..12], &1_u32.to_be_bytes());
        assert_eq!(&big_endian.bytes()[12..14], &0x1122_u16.to_be_bytes());
        assert_ne!(
            carrier.non_authoritative_materialization_report_fingerprint(),
            big_endian.non_authoritative_materialization_report_fingerprint()
        );

        let mut destination = [0x5a; 20];
        carrier
            .apply(&checked, &mut destination)
            .expect("complete replay should permit one atomic copy");
        assert_eq!(&destination[..16], carrier.bytes());
        assert_eq!(&destination[16..], &[0x5a; 4]);
        let mut short = [0x6b; 15];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 15]);

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthSixRecordSumPathsLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "Outer",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut missing = paths.clone();
        missing.paths.pop();
        rejects(&missing);
        let mut extra = paths.clone();
        extra.paths.push(extra.paths[0].clone());
        rejects(&extra);
        let mut reordered = paths.clone();
        reordered.paths.swap(0, 1);
        rejects(&reordered);
        let mut wrong_outer_identity = paths.clone();
        wrong_outer_identity.paths[0].outer_member_identity = Some(2);
        rejects(&wrong_outer_identity);
        let mut wrong_inner_identity = paths.clone();
        wrong_inner_identity.paths[0].depth_five_paths.paths[0].outer_member_identity = Some(2);
        rejects(&wrong_inner_identity);
        let mut wrong_leaf_geometry = paths.clone();
        wrong_leaf_geometry.paths[0].depth_five_paths.paths[0]
            .depth_four_paths
            .paths[0]
            .depth_three_paths
            .paths[0]
            .depth_two_paths
            .paths[0]
            .middle_paths
            .paths[0]
            .child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_leaf_geometry);
        let mut wrong_child_extent = paths.clone();
        wrong_child_extent.paths[0]
            .depth_five_paths
            .outer_layout
            .size = Some(16);
        rejects(&wrong_child_extent);
        let mut wrong_outer_geometry = paths.clone();
        wrong_outer_geometry.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 4 };
        rejects(&wrong_outer_geometry);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
                .is_err()
        );

        for name in ["Fourth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
            assert!(
                project_conventional_record_with_depth_six_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} remains outside the exact depth-six cohort"
            );
        }

        let record_fields = |name: &str| {
            let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
            let DataShape::Record { fields } = layout.shape else {
                unreachable!("fixture owner is a record")
            };
            fields
        };
        for (owner, index) in [
            ("Outer", 0),
            ("Fourth", 0),
            ("Third", 0),
            ("Second", 0),
            ("First", 0),
            ("Middle", 0),
            ("Leaf", 0),
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: plan.fields.span_or_empty(record_fields(owner))[index].symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_six_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every record layer remains fenced"
            );
        }

        let recursive_type = checked
            .data_members(fourth)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "third" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: fourth.symbol,
                    name: fourth.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_six_nested_sums_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "a recursive depth-six path must reject during bounded projection"
        );
        assert!(
            validate_const_materializable_record_with_depth_six_nested_sums(
                &recursive_checked,
                "Outer",
                &paths,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "a recursive depth-six path must reject during value replay"
        );

        assert!(
            project_conventional_record_with_depth_five_nested_sums_materialization_layout(
                &checked,
                &plan,
                fourth.symbol,
            )
            .is_ok(),
            "the unchanged plural depth-five API retains its prior cohort"
        );
        assert!(
            project_conventional_record_with_depth_five_nested_sums_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the prior API must not widen to the new depth-six root"
        );
    }

    #[test]
    fn producer_reachability_validates_siblings_after_an_already_found_sum() {
        let checked = checked(
            r#"
            data Choice [copy] { case Empty; }
            data Trap [copy] { choice: Choice; later: u8; }
            data Root [copy] { trap: Trap; }
            "#,
        );
        let trap = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Trap")
            .expect("Trap definition");
        let trap_symbol = trap.symbol;
        let trap_name = trap.name.clone();
        let later_type = checked
            .data_members(trap)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "later" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("later field");
        let root = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Root")
            .expect("Root definition");
        let trap_type = checked
            .data_members(root)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "trap" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("trap field");
        assert!(matches!(
            SumReachability::new(&checked).type_contains_sum(trap_type),
            Ok(true)
        ));

        let mut recursive = checked.clone();
        recursive.typed.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: trap_symbol,
                name: trap_name.clone(),
            },
        );
        assert!(
            SumReachability::new(&recursive)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later recursive sibling must not hide behind an earlier sum"
        );

        let mut malformed = checked;
        malformed.typed.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: trap_name,
            },
        );
        assert!(
            SumReachability::new(&malformed)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later malformed nominal branch must not hide behind an earlier sum"
        );
    }

    #[test]
    fn plural_depth_two_record_chains_retain_complete_occurrence_custody() {
        let checked = checked(
            r#"
            data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
            data Leaf [copy] { #1 lead: u8; #2 choice: Choice; #3 tail: u8; }
            data Middle [copy] {
                #1 lead: u8;
                #2 first: Leaf;
                #3 marker: u16;
                #4 second: Leaf;
                #5 tail: u8;
            }
            data Outer [copy] {
                #1 lead: u8;
                #2 first: Middle;
                #3 marker: u16;
                #4 second: Middle;
                #5 tail: u8;
            }

            data Deep [copy] { #1 middle: Middle; }
            data OuterDeep [copy] { #1 deep: Deep; }
            data OuterDirect [copy] { #1 first: Middle; #2 direct: Choice; }
            data OuterArray [copy] { #1 first: Middle; #2 choices: [Choice; 1]; }
            data Mixed [copy] { #1 common: u8; case #2 Variant(#1 value: u16); }
            data OuterMixed [copy] { #1 first: Middle; #2 mixed: Mixed; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let paths = project_conventional_record_with_depth_two_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete plural depth-two chain set should project");
        assert_eq!(
            paths.outer_layout.offsets.as_deref(),
            Some(&[0, 4, 48, 52, 96][..])
        );
        assert_eq!(paths.outer_layout.size, Some(100));
        assert_eq!(paths.paths.len(), 2);
        assert_eq!(
            paths
                .paths
                .iter()
                .map(|path| (path.outer_field.as_str(), path.outer_member_identity))
                .collect::<Vec<_>>(),
            [("first", Some(2)), ("second", Some(4))]
        );
        for occurrence in &paths.paths {
            assert_eq!(
                occurrence.middle_paths.outer_layout.offsets.as_deref(),
                Some(&[0, 4, 20, 24, 40][..])
            );
            assert_eq!(occurrence.middle_paths.outer_layout.size, Some(44));
            assert_eq!(occurrence.middle_paths.paths.len(), 2);
            assert_eq!(
                occurrence
                    .middle_paths
                    .paths
                    .iter()
                    .map(|path| (path.outer_field.as_str(), path.outer_member_identity))
                    .collect::<Vec<_>>(),
                [("first", Some(2)), ("second", Some(4))]
            );
            for leaf in &occurrence.middle_paths.paths {
                assert_eq!(leaf.inner_layout.offsets.as_deref(), Some(&[0, 4, 12][..]));
                assert_eq!(leaf.inner_layout.size, Some(16));
                assert_eq!(leaf.child_sum_layouts.len(), 1);
                assert_eq!(leaf.child_sum_layouts[0].field, "choice");
                assert_eq!(leaf.child_sum_layouts[0].member_identity, Some(2));
            }
        }
        assert!(
            project_conventional_record_with_depth_two_nested_sum_materialization_layout(
                &checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "the singular API must not silently select one plural occurrence"
        );

        let empty = || BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let number = |value| BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        };
        let leaf = |lead, choice, tail| BuildTimeValue::Struct {
            type_name: "Leaf".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(lead)),
                ("choice".into(), choice),
                ("tail".into(), BuildTimeValue::Int(tail)),
            ],
        };
        let middle = |lead, first, marker, second, tail| BuildTimeValue::Struct {
            type_name: "Middle".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(lead)),
                ("first".into(), first),
                ("marker".into(), BuildTimeValue::Int(marker)),
                ("second".into(), second),
                ("tail".into(), BuildTimeValue::Int(tail)),
            ],
        };
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("lead".into(), BuildTimeValue::Int(0xe0)),
                (
                    "first".into(),
                    middle(
                        0xa0,
                        leaf(0xa1, empty(), 0xa2),
                        0x1122,
                        leaf(0xa3, number(0x3344), 0xa4),
                        0xa5,
                    ),
                ),
                ("marker".into(), BuildTimeValue::Int(0x5566)),
                (
                    "second".into(),
                    middle(
                        0xb0,
                        leaf(0xb1, number(0x7788), 0xb2),
                        0x99aa,
                        leaf(0xb3, empty(), 0xb4),
                        0xb5,
                    ),
                ),
                ("tail".into(), BuildTimeValue::Int(0xe1)),
            ],
        };
        let carrier = validate_const_materializable_record_with_depth_two_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the complete plural report should rejoin exact value custody");
        assert_eq!(carrier.occurrences().len(), 2);
        assert_eq!(carrier.occurrences()[0].outer_field(), "first");
        assert_eq!(carrier.occurrences()[1].outer_field(), "second");
        assert_eq!(carrier.occurrences()[0].middle().inner_records().len(), 2);
        assert_eq!(carrier.occurrences()[1].middle().inner_records().len(), 2);

        let mut expected = vec![0; 100];
        expected[0] = 0xe0;
        expected[4] = 0xa0;
        expected[8] = 0xa1;
        expected[20] = 0xa2;
        expected[24..26].copy_from_slice(&0x1122_u16.to_le_bytes());
        expected[28] = 0xa3;
        expected[32..36].copy_from_slice(&1_u32.to_le_bytes());
        expected[36..38].copy_from_slice(&0x3344_u16.to_le_bytes());
        expected[40] = 0xa4;
        expected[44] = 0xa5;
        expected[48..50].copy_from_slice(&0x5566_u16.to_le_bytes());
        expected[52] = 0xb0;
        expected[56] = 0xb1;
        expected[60..64].copy_from_slice(&1_u32.to_le_bytes());
        expected[64..66].copy_from_slice(&0x7788_u16.to_le_bytes());
        expected[68] = 0xb2;
        expected[72..74].copy_from_slice(&0x99aa_u16.to_le_bytes());
        expected[76] = 0xb3;
        expected[88] = 0xb4;
        expected[92] = 0xb5;
        expected[96] = 0xe1;
        assert_eq!(carrier.bytes(), expected);

        let mut destination = [0x5a; 104];
        carrier
            .apply(&checked, &mut destination)
            .expect("the complete padded image should copy atomically");
        assert_eq!(&destination[..100], carrier.bytes());
        assert_eq!(&destination[100..], &[0x5a; 4]);
        let mut short = [0x6b; 99];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 99]);

        let mut renamed = paths.clone();
        renamed.outer_layout.entries[1].field = "renamed_middle".into();
        renamed.paths[0].outer_field = "renamed_middle".into();
        renamed.paths[0].middle_paths.outer_layout.entries[1].field = "renamed_leaf".into();
        renamed.paths[0].middle_paths.paths[0].outer_field = "renamed_leaf".into();
        renamed.paths[0].middle_paths.paths[0].inner_layout.entries[1].field =
            "renamed_choice".into();
        renamed.paths[0].middle_paths.paths[0].child_sum_layouts[0].field = "renamed_choice".into();
        carrier
            .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
            .expect("stable-numbered plural names at all three layers are presentation-only");

        let rejects =
            |mutated: &psi_layout_plans::ConventionalDepthTwoRecordSumPathsLayoutReport| {
                assert!(
                    carrier
                        .replay_against(
                            &checked,
                            "Outer",
                            mutated,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err()
                );
            };
        let mut missing_outer = paths.clone();
        missing_outer.paths.pop();
        rejects(&missing_outer);
        let mut extra_outer = paths.clone();
        extra_outer.paths.push(paths.paths[0].clone());
        rejects(&extra_outer);
        let mut reordered_outer = paths.clone();
        reordered_outer.paths.swap(0, 1);
        rejects(&reordered_outer);
        let mut missing_middle = paths.clone();
        missing_middle.paths[0].middle_paths.paths.pop();
        rejects(&missing_middle);
        let mut extra_middle = paths.clone();
        extra_middle.paths[0]
            .middle_paths
            .paths
            .push(paths.paths[0].middle_paths.paths[0].clone());
        rejects(&extra_middle);
        let mut reordered_middle = paths.clone();
        reordered_middle.paths[0].middle_paths.paths.swap(0, 1);
        rejects(&reordered_middle);
        let mut wrong_outer_identity = paths.clone();
        wrong_outer_identity.paths[0].outer_member_identity = Some(4);
        rejects(&wrong_outer_identity);
        let mut wrong_middle_identity = paths.clone();
        wrong_middle_identity.paths[0].middle_paths.paths[0].outer_member_identity = Some(4);
        rejects(&wrong_middle_identity);
        let mut wrong_child_identity = paths.clone();
        wrong_child_identity.paths[0].middle_paths.paths[0].child_sum_layouts[0].member_identity =
            Some(3);
        rejects(&wrong_child_identity);
        let mut wrong_outer_layout = paths.clone();
        wrong_outer_layout.outer_layout.entries[1].placement =
            LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_outer_layout);
        let mut wrong_middle_layout = paths.clone();
        wrong_middle_layout.paths[0]
            .middle_paths
            .outer_layout
            .entries[1]
            .placement = LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_middle_layout);
        let mut wrong_leaf_layout = paths.clone();
        wrong_leaf_layout.paths[0].middle_paths.paths[0]
            .inner_layout
            .entries[1]
            .placement = LayoutPlacementReport::At { offset: 8 };
        rejects(&wrong_leaf_layout);
        let mut wrong_child_row = paths.clone();
        wrong_child_row.paths[0].middle_paths.paths[0].child_sum_layouts[0]
            .layout
            .cases[1]
            .payload_fields[0]
            .offset += 1;
        rejects(&wrong_child_row);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
                .is_err()
        );
        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is an outer record")
        };
        fields[2].1 = BuildTimeValue::Int(0x5567);
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "Outer",
                    &paths,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        for name in ["OuterDirect", "OuterArray", "OuterDeep", "OuterMixed"] {
            assert!(
                project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} must remain outside the plural depth-two cohort"
            );
        }

        let outer_layout = unique_data_layout(&plan, outer.symbol, "Outer").unwrap();
        let DataShape::Record {
            fields: outer_fields,
        } = outer_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let middle_definition = definition("Middle");
        let middle_layout = unique_data_layout(&plan, middle_definition.symbol, "Middle").unwrap();
        let DataShape::Record {
            fields: middle_fields,
        } = middle_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let leaf_definition = definition("Leaf");
        let leaf_layout = unique_data_layout(&plan, leaf_definition.symbol, "Leaf").unwrap();
        let DataShape::Record {
            fields: leaf_fields,
        } = leaf_layout.shape
        else {
            unreachable!("fixture is a record")
        };

        let mut wrong_top_type_symbol = plan.clone();
        wrong_top_type_symbol.fields.span_mut_or_empty(outer_fields)[1].type_symbol =
            leaf_definition.symbol;
        assert!(
            project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                &checked,
                &wrong_top_type_symbol,
                outer.symbol,
            )
            .is_err(),
            "the plural top edge must rejoin the exact laid type symbol"
        );
        let mut wrong_top_descriptor_symbol = plan.clone();
        wrong_top_descriptor_symbol
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: leaf_definition.symbol,
            name: middle_definition.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                &checked,
                &wrong_top_descriptor_symbol,
                outer.symbol,
            )
            .is_err(),
            "the plural top edge must rejoin the exact descriptor symbol"
        );
        let mut wrong_top_descriptor_name = plan.clone();
        wrong_top_descriptor_name
            .fields
            .span_mut_or_empty(outer_fields)[1]
            .type_descriptor = TypeLayoutDescriptor::Named {
            symbol: middle_definition.symbol,
            name: leaf_definition.name.clone(),
        };
        assert!(
            project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                &checked,
                &wrong_top_descriptor_name,
                outer.symbol,
            )
            .is_err(),
            "the plural top edge must rejoin the exact descriptor spelling"
        );

        for field_symbol in [
            plan.fields.span_or_empty(outer_fields)[1].symbol,
            plan.fields.span_or_empty(middle_fields)[1].symbol,
            plan.fields.span_or_empty(leaf_fields)[0].symbol,
        ] {
            let mut special_plan = plan.clone();
            special_plan
                .repeated_fields
                .push(crate::RepeatedFieldLayout {
                    field: field_symbol,
                    element_stride: 16,
                });
            assert!(
                project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "target-dependent placement at every record layer must reject"
            );
        }

        let recursive_type = checked
            .data_members(middle_definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "lead" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .unwrap();
        let mut recursive_checked = checked.clone();
        recursive_checked
            .typed
            .type_reference_table
            .substitute_node(
                recursive_type,
                TypeReferenceNode::Named {
                    symbol: middle_definition.symbol,
                    name: middle_definition.name.clone(),
                },
            );
        assert!(
            project_conventional_record_with_depth_two_nested_sums_materialization_layout(
                &recursive_checked,
                &plan,
                outer.symbol,
            )
            .is_err(),
            "recursive depth-two chains must fail through bounded reachability"
        );
    }

    #[test]
    fn nested_record_path_keeps_erased_values_semantic_but_nonphysical() {
        let checked = checked(
            r#"
            data Choice [copy] { case Empty; case Number(value: u8); }
            data Inner [copy] { choice: Choice; proof [erased]: u64; }
            data Outer [copy] {
                prefix: u8;
                inner: Inner;
                witness [erased]: u32;
                suffix: u16;
            }
            "#,
        );
        let outer = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let path = project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("erased fields do not create physical path rows");
        assert_eq!(path.outer_layout.entries.len(), 3);
        assert_eq!(path.inner_layout.entries.len(), 1);
        assert_eq!(path.child_sum_layouts.len(), 1);

        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(7)),
                (
                    "inner".into(),
                    BuildTimeValue::Struct {
                        type_name: "Inner".into(),
                        fields: vec![
                            (
                                "choice".into(),
                                BuildTimeValue::Case {
                                    variant: "Number".into(),
                                    payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                                },
                            ),
                            ("proof".into(), BuildTimeValue::Int(99)),
                        ],
                    },
                ),
                ("witness".into(), BuildTimeValue::Int(17)),
                ("suffix".into(), BuildTimeValue::Int(0x1122)),
            ],
        };
        let carrier = validate_const_materializable_record_with_nested_sum_record(
            &checked,
            "Outer",
            &path,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("valid erased values remain required without occupying bytes");
        assert_eq!(
            carrier.bytes(),
            &[7, 0, 0, 0, 1, 0, 0, 0, 0x5c, 0, 0, 0, 0x22, 0x11, 0, 0]
        );

        let mut malformed_erased = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut malformed_erased else {
            unreachable!("fixture is outer record")
        };
        fields[2].1 = BuildTimeValue::Bool(true);
        assert!(
            validate_const_materializable_record_with_nested_sum_record(
                &checked,
                "Outer",
                &path,
                &malformed_erased,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "erased fields remain part of exact typed value validation"
        );
    }

    #[test]
    fn target_layout_projects_one_live_record_with_sum_materialization_pair() {
        let checked = checked(
            r#"
            data Choice [copy] {
                case Empty;
                case Pair(left: u16, right: u32);
            }
            data Envelope [copy] {
                prefix: u8;
                choice: Choice;
                suffix: u16;
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Envelope")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
            &checked,
            &plan,
            definition.symbol,
        )
        .expect("target runtime layout should project the exact paired evidence");
        assert_eq!(outer.offsets.as_deref(), Some(&[0, 4, 16][..]));
        assert_eq!(outer.size, Some(20));
        assert_eq!(outer.align, 4);
        assert_eq!(nested_rows.len(), 1);
        assert_eq!(nested_rows[0].field, "choice");
        assert_eq!(nested_rows[0].layout.size, 12);
        assert_eq!(nested_rows[0].layout.align, 4);

        let value = BuildTimeValue::Struct {
            type_name: "Envelope".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(7)),
                (
                    "choice".into(),
                    BuildTimeValue::Case {
                        variant: "Pair".into(),
                        payload: vec![
                            ("left".into(), BuildTimeValue::Int(0x1122)),
                            ("right".into(), BuildTimeValue::Int(0x3344_5566)),
                        ],
                    },
                ),
                ("suffix".into(), BuildTimeValue::Int(0x7788)),
            ],
        };
        let materialized = validate_const_materializable_record_with_conventional_sum(
            &checked,
            "Envelope",
            &outer,
            &nested_rows[0].layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced pair should feed the nested-sum carrier");
        assert_eq!(
            materialized.bytes(),
            &[
                7, 0, 0, 0, 1, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33, 0x88, 0x77, 0, 0,
            ]
        );
    }

    #[test]
    fn target_layout_projects_every_direct_sum_occurrence_and_keeps_broader_shapes_fenced() {
        let checked = checked(
            r#"
            data Choice [copy] { case Empty; case Number(value: u8); }
            data Multiple [copy] { first: Choice; second: Choice; }
            data ErasedAlso [copy] { live: Choice; proof [erased]: Choice; }
            data ArrayOwner [copy] { choices: [Choice; 2]; }
            data ArrayWithNeighbor [copy] { bytes: [u8; 2]; choices: [Choice; 2]; suffix: u16; }
            data ZeroArrayOwner [copy] { choices: [Choice; 0]; }
            data TwoArrayOwner [copy] { first: [Choice; 1]; second: [Choice; 2]; }
            data DirectAndNestedArrayOwner [copy] {
                direct: [Choice; 1];
                nested: [[Choice; 1]; 1];
            }
            data Inner [copy] { choice: Choice; }
            data RecursiveOwner [copy] { inner: Inner; }
            data Mixed [copy] { common: u8; case Empty; }
            data MixedOwner [copy] { mixed: Mixed; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let multiple = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Multiple")
            .unwrap();
        let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
            &checked,
            &plan,
            multiple.symbol,
        )
        .expect("all direct runtime sum occurrences should project in authored order");
        assert_eq!(outer.offsets.as_deref(), Some(&[0, 8][..]));
        assert_eq!(
            nested_rows
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(nested_rows[0].member_identity, None);
        assert_eq!(nested_rows[1].member_identity, None);
        assert_eq!(nested_rows[0].layout, nested_rows[1].layout);

        let erased_also = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ErasedAlso")
            .unwrap();
        let (erased_outer, erased_rows) =
            project_conventional_record_with_sum_materialization_layout(
                &checked,
                &plan,
                erased_also.symbol,
            )
            .expect("erased sum fields are not runtime materialization occurrences");
        assert_eq!(erased_outer.offsets.as_deref(), Some(&[0][..]));
        assert_eq!(
            erased_rows
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["live"]
        );

        let array_owner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ArrayOwner")
            .unwrap();
        let (array_outer, array_row) =
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &plan,
                array_owner.symbol,
            )
            .expect("one direct nonzero literal sum array should project compactly");
        assert_eq!(array_outer.offsets.as_deref(), Some(&[0][..]));
        assert_eq!(array_row.field, "choices");
        assert_eq!(array_row.member_identity, None);
        assert_eq!(array_row.element_count, 2);
        assert_eq!(array_row.element_stride, array_row.element_layout.size);
        assert_eq!(array_row.element_stride, 8);

        let two_array_owner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "TwoArrayOwner")
            .unwrap();
        let (two_array_outer, two_array_rows) =
            project_conventional_record_with_sum_arrays_materialization_layout(
                &checked,
                &plan,
                two_array_owner.symbol,
            )
            .expect("every direct sum-array occurrence should project in authored order");
        assert_eq!(two_array_outer.offsets.as_deref(), Some(&[0, 8][..]));
        assert_eq!(two_array_outer.size, Some(24));
        assert_eq!(
            two_array_rows
                .iter()
                .map(|row| (row.field.as_str(), row.element_count, row.element_stride))
                .collect::<Vec<_>>(),
            [("first", 1, 8), ("second", 2, 8)]
        );
        let two_array_value = BuildTimeValue::Struct {
            type_name: "TwoArrayOwner".into(),
            fields: vec![
                (
                    "first".into(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Case {
                        variant: "Number".into(),
                        payload: vec![("value".into(), BuildTimeValue::Int(0x11))],
                    }]),
                ),
                (
                    "second".into(),
                    BuildTimeValue::Array(vec![
                        BuildTimeValue::Case {
                            variant: "Empty".into(),
                            payload: Vec::new(),
                        },
                        BuildTimeValue::Case {
                            variant: "Number".into(),
                            payload: vec![("value".into(), BuildTimeValue::Int(0x22))],
                        },
                    ]),
                ),
            ],
        };
        let two_array_materialized =
            validate_const_materializable_record_with_conventional_sum_arrays(
                &checked,
                "TwoArrayOwner",
                &two_array_outer,
                &two_array_rows,
                &two_array_value,
                ByteOrder::LittleEndian,
            )
            .expect("the plural target report should rejoin plural value custody");
        assert_eq!(
            two_array_materialized.bytes(),
            &[
                1, 0, 0, 0, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x22, 0, 0, 0,
            ]
        );
        let two_array_layout =
            unique_data_layout(&plan, two_array_owner.symbol, "TwoArrayOwner").unwrap();
        let DataShape::Record {
            fields: two_array_fields,
        } = two_array_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let second_array_symbol = plan.fields.span_or_empty(two_array_fields)[1].symbol;
        let mut repeated_second_array_plan = plan.clone();
        repeated_second_array_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: second_array_symbol,
                element_stride: 16,
            });
        assert!(
            project_conventional_record_with_sum_arrays_materialization_layout(
                &checked,
                &repeated_second_array_plan,
                two_array_owner.symbol,
            )
            .is_err(),
            "target-dependent placement on the second qualifying array must reject"
        );

        let array_data_layout =
            unique_data_layout(&plan, array_owner.symbol, "ArrayOwner").unwrap();
        let DataShape::Record {
            fields: array_fields,
        } = array_data_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let mut substituted_plan = plan.clone();
        substituted_plan.fields.span_mut_or_empty(array_fields)[0].type_symbol =
            SymbolHandle::invalid();
        assert!(
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &substituted_plan,
                array_owner.symbol,
            )
            .is_err(),
            "an inconsistent laid array element symbol must reject"
        );

        let neighbor_owner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ArrayWithNeighbor")
            .unwrap();
        let (neighbor_outer, neighbor_row) =
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &plan,
                neighbor_owner.symbol,
            )
            .expect("the compact target report should preserve ordinary sibling fields");
        let materialized = validate_const_materializable_record_with_conventional_sum_array(
            &checked,
            "ArrayWithNeighbor",
            &neighbor_outer,
            &neighbor_row,
            &BuildTimeValue::Struct {
                type_name: "ArrayWithNeighbor".into(),
                fields: vec![
                    (
                        "bytes".into(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Int(0xaa),
                            BuildTimeValue::Int(0xbb),
                        ]),
                    ),
                    (
                        "choices".into(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Case {
                                variant: "Empty".into(),
                                payload: Vec::new(),
                            },
                            BuildTimeValue::Case {
                                variant: "Number".into(),
                                payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                            },
                        ]),
                    ),
                    ("suffix".into(), BuildTimeValue::Int(0x1122)),
                ],
            },
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced compact report should rejoin indexed materialization");
        assert_eq!(neighbor_outer.offsets.as_deref(), Some(&[0, 4, 20][..]));
        assert_eq!(
            materialized.bytes(),
            &[
                0xaa, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x5c, 0, 0, 0, 0x22, 0x11, 0,
                0,
            ]
        );
        let neighbor_data_layout =
            unique_data_layout(&plan, neighbor_owner.symbol, "ArrayWithNeighbor").unwrap();
        let DataShape::Record {
            fields: neighbor_fields,
        } = neighbor_data_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let neighbor_field_symbol = plan.fields.span_or_empty(neighbor_fields)[0].symbol;
        let mut repeated_neighbor_plan = plan.clone();
        repeated_neighbor_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: neighbor_field_symbol,
                element_stride: 2,
            });
        assert!(
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &repeated_neighbor_plan,
                neighbor_owner.symbol,
            )
            .is_err(),
            "target-dependent repeated placement on a neighboring field must reject"
        );

        let multiple_layout = unique_data_layout(&plan, multiple.symbol, "Multiple").unwrap();
        let DataShape::Record {
            fields: multiple_fields,
        } = multiple_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let direct_field_symbol = plan.fields.span_or_empty(multiple_fields)[0].symbol;
        let mut repeated_direct_plan = plan.clone();
        repeated_direct_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: direct_field_symbol,
                element_stride: 16,
            });
        assert!(
            project_conventional_record_with_sum_materialization_layout(
                &checked,
                &repeated_direct_plan,
                multiple.symbol,
            )
            .is_err(),
            "legacy direct projection must not flatten target-dependent outer placement"
        );

        for name in [
            "ZeroArrayOwner",
            "TwoArrayOwner",
            "RecursiveOwner",
            "MixedOwner",
        ] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_sum_array_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the single direct nonzero sum-array rung"
            );
        }

        for name in [
            "ZeroArrayOwner",
            "DirectAndNestedArrayOwner",
            "RecursiveOwner",
            "MixedOwner",
        ] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_sum_arrays_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the plural direct sum-array rung"
            );
        }

        for name in ["ArrayOwner", "RecursiveOwner", "MixedOwner"] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_sum_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the direct nested-sum rung"
            );
        }
    }
}
