//! Exact plural depth-sixteen conventional-sum materialization projection.

use super::*;
use psi_layout_plans::{
    ConventionalDepthSixteenRecordSumOccurrenceLayoutReport,
    ConventionalDepthSixteenRecordSumPathsLayoutReport,
};

/// Project the complete nonempty authored-order set of exact depth-sixteen
/// record chains:
/// `Outer -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-fifteen
/// report for its exact fourteenth record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthSixteenRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

pub(super) fn project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthSixteenRecordSumPathsLayoutReport, Diagnostic> {
    let definition =
        unique_data_definition(program, data_symbol, "plural depth-sixteen sum owner")?;
    validate_closed_copy_record(program, definition, "plural depth-sixteen sum owner")?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for plural depth-sixteen sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for plural depth-sixteen sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-sixteen sum outer report exceeds compiler resources")
        })?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-sixteen sum outer offsets exceed compiler resources")
        })?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error("plural depth-sixteen sum path report exceeds compiler resources")
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
                "plural depth-sixteen sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
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
                    "plural depth-sixteen sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural depth-sixteen sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural depth-sixteen sum outer field `{}` does not name the required fourteenth record",
                    declared.name
                )));
            }
            let depth_fifteen_paths = project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout_with_reachability(
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
                    "target runtime layout field `{}` is not the exact declared fourteenth record",
                    declared.name
                )));
            };
            if laid.type_symbol != named.symbol
                || *laid_symbol != named.symbol
                || laid_name.as_str() != named.name.as_str()
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` substitutes its fourteenth record type",
                    declared.name
                )));
            }
            if usize_to_u64(laid.layout.size, "depth-sixteen fourteenth-record extent")?
                != depth_fifteen_paths
                    .outer_layout
                    .size
                    .expect("plural depth-fifteen projection has fixed extent")
                || usize_to_u64(
                    laid.layout.alignment,
                    "depth-sixteen fourteenth-record alignment",
                )? != depth_fifteen_paths.outer_layout.align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact fourteenth-record extent/alignment",
                    declared.name
                )));
            }
            for fifteenth_occurrence in &depth_fifteen_paths.paths {
                for fourteenth_occurrence in &fifteenth_occurrence.inner.paths {
                    for thirteenth_occurrence in &fourteenth_occurrence.inner.paths {
                        for twelfth_occurrence in &thirteenth_occurrence.inner.paths {
                            for eleventh_occurrence in &twelfth_occurrence.inner.paths
                            {
                                for tenth_occurrence in &eleventh_occurrence.inner.paths {
                                    for ninth_occurrence in &tenth_occurrence.inner.paths
                                    {
                                        for eighth_occurrence in
                                            &ninth_occurrence.inner.paths
                                        {
                                            for seventh_occurrence in
                                                &eighth_occurrence.inner.paths
                                            {
                                                for sixth_occurrence in
                                                    &seventh_occurrence.inner.paths
                                                {
                                                    for fifth_occurrence in
                                                        &sixth_occurrence.inner.paths
                                                    {
                                                        for fourth_occurrence in
                                                            &fifth_occurrence.inner.paths
                                                        {
                                                            for third_occurrence in
                                                                &fourth_occurrence
                                                                    .inner
                                                                    .paths
                                                            {
                                                                for second_occurrence in
                                                                    &third_occurrence
                                                                        .inner
                                                                        .paths
                                                                {
                                                                    total_leaf_paths = total_leaf_paths
                                                        .checked_add(
                                                            second_occurrence
                                                                .inner
                                                                .paths
                                                                .len(),
                                                        )
                                                        .ok_or_else(|| {
                                                            Diagnostic::error(
                                                                "plural depth-sixteen leaf-path count overflows",
                                                            )
                                                        })?;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural depth-sixteen paths exceed bounded total leaf occurrences",
                ));
            }
            paths.push(ConventionalDepthSixteenRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                inner: depth_fifteen_paths,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural depth-sixteen outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural depth-sixteen sum projection requires a nonempty qualifying record-chain set",
        ));
    }
    Ok(ConventionalDepthSixteenRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural depth-sixteen outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural depth-sixteen outer record alignment",
            )?,
        },
        paths,
    })
}
