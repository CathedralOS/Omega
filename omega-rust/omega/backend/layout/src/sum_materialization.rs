//! Projection of the authoritative conventional pure-sum runtime layout.
//!
//! This is a report of the fixed tag-prefixed overlay selected by this crate,
//! not a back door for programmable `Layout` policies to author case/tag
//! placement.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::{DataSupplyMode, Multiplicity};
use layout_plans::{
    ConventionalNestedRecordSumOccurrenceLayoutReport, ConventionalNestedRecordSumPathLayoutReport,
    ConventionalNestedRecordSumPathsLayoutReport, ConventionalRecordSumPathsLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumCaseLayoutReport, ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

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
            schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
                program, definition,
            ),
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
        schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
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

/// Project exact conventional record paths without encoding depth in the API.
pub fn project_conventional_record_with_recursive_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalRecursiveRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_recursive_paths(program, plan, data_symbol, &mut reachability, 1)
}

fn project_recursive_paths(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
    depth: usize,
) -> Result<ConventionalRecursiveRecordSumPathsLayoutReport, Diagnostic> {
    if depth > layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
        return Err(Diagnostic::error(
            "recursive record paths exceed the compiler depth resource bound of 64",
        ));
    }
    let definition = unique_data_definition(program, data_symbol, "recursive sum owner")?;
    validate_closed_copy_record(program, definition, "recursive sum owner")?;
    let profile = record_sum_profile(program, definition, reachability)?;
    if profile.deeper {
        project_record_sum_branches(program, plan, data_symbol, reachability, depth)
            .map(ConventionalRecursiveRecordSumPathsLayoutReport::Branch)
    } else {
        let (outer_layout, child_sum_layouts) =
            project_conventional_record_with_sum_materialization_layout(
                program,
                plan,
                data_symbol,
            )?;
        Ok(ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
            outer_layout,
            child_sum_layouts,
        })
    }
}

fn project_record_sum_branches(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
    depth: usize,
) -> Result<ConventionalRecordSumPathsLayoutReport, Diagnostic> {
    let owner = "plural recursive sum owner";
    let definition = unique_data_definition(program, data_symbol, owner)?;
    validate_closed_copy_record(program, definition, owner)?;
    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for {owner} `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = relevant_record_fields(program, definition);
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for {owner} `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error(format!("{owner} report exceeds compiler resources")))?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| Diagnostic::error(format!("{owner} offsets exceed compiler resources")))?;
    let mut paths = Vec::new();
    paths
        .try_reserve_exact(declared_fields.len())
        .map_err(|_| {
            Diagnostic::error(format!("{owner} path report exceeds compiler resources"))
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
                "plural recursive sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
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
                    "plural recursive sum outer field `{}` reaches a sum through an array",
                    declared.name
                )));
            }
            let named = exact_named_data(program, declared.type_reference)?.ok_or_else(|| {
                Diagnostic::error(format!(
                    "plural recursive sum outer field `{}` lacks one exact record identity",
                    declared.name
                ))
            })?;
            if DataDefinition::shape_kind_from_members(program.data_members(named))
                != DataShapeKind::Record
            {
                return Err(Diagnostic::error(format!(
                    "plural recursive sum outer field `{}` does not name the required inner record",
                    declared.name
                )));
            }
            let inner =
                project_recursive_paths(program, plan, named.symbol, reachability, depth + 1)?;
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
            if usize_to_u64(laid.layout.size, "recursive inner-record extent")?
                != inner
                    .outer_layout()
                    .size
                    .expect("recursive inner projection has fixed extent")
                || usize_to_u64(laid.layout.alignment, "recursive inner-record alignment")?
                    != inner.outer_layout().align
            {
                return Err(Diagnostic::error(format!(
                    "target runtime layout field `{}` does not retain the exact inner-record extent/alignment from child",
                    declared.name
                )));
            }
            total_leaf_paths = total_leaf_paths
                .checked_add(inner.leaf_occurrence_count().ok_or_else(|| {
                    Diagnostic::error("plural recursive leaf-path count overflows".to_owned())
                })?)
                .ok_or_else(|| {
                    Diagnostic::error("plural recursive leaf-path count overflows".to_owned())
                })?;
            if total_leaf_paths > SumReachability::MAX_EDGES {
                return Err(Diagnostic::error(
                    "plural recursive paths exceed bounded total leaf occurrences".to_owned(),
                ));
            }
            paths.push(layout_plans::ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: declared.name.to_string(),
                outer_member_identity: declared.identity,
                inner,
            });
        }

        let offset = usize_to_u64(laid.offset, "plural recursive outer field offset")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "plural recursive sum projection requires a nonempty qualifying record-chain set"
                .to_owned(),
        ));
    }
    Ok(ConventionalRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
                program, definition,
            ),
            entries,
            offsets: Some(offsets),
            size: Some(usize_to_u64(
                data_layout.layout.size,
                "plural recursive outer record extent",
            )?),
            align: usize_to_u64(
                data_layout.layout.alignment,
                "plural recursive outer record alignment",
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
            TypeReferenceNode::FixedArray { .. }
                if reachability.type_contains_sum(field.type_reference)? =>
            {
                profile.array = true;
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
        mut type_reference: typed_trees::types::TypeReferenceHandle,
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
) -> Vec<&'a typed_trees::data::DataField> {
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
            schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
                program, definition,
            ),
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

fn exact_named_data(
    program: &CheckedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<Option<&DataDefinition>, Diagnostic> {
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
        schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
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
mod tests;
