/*
We derive `SymbolicMaterializationPlan` instances by joining validated layout
geometry (`LayoutPlanReport` and nested record/sum interiors) with symbolic
field values under placement phase and address-range constraints.
*/

use crate::layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalSumLayoutReport, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport,
};
use crate::materialization::field_identities::{
    MaterializationFieldKey, materialization_field_key, planned_entries_by_key,
    stable_identity_suffix, symbolic_index_display, symbolic_path_display, symbolic_path_hops,
    validate_conventional_sum_materialization_identities,
    validate_materialization_field_identities,
};
use crate::materialization::field_values::ScalarFieldValue;
use crate::materialization::stored_integer_writes::{
    apply_fragment, materializer_scalar_window, scalar_fragment, validate_fragment,
    validate_stored_integer_value, validate_write, validate_write_source_value,
};
use crate::materialization::{MaterializationDiagnostic, SymbolicMaterializationPlan};
use crate::placement::{
    ByteOrder, ConsumptionInstant, MaterializationAction, MaterializationContext,
    MaterializationWrite, StoredIntegerFit,
};
use crate::symbolic_values::{
    RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldInterior, SymbolicFieldValue,
};

pub fn derive_symbolic_materialization(
    layout: &LayoutPlanReport,
    symbolic_fields: &[SymbolicFieldValue],
    context: MaterializationContext,
    resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<SymbolicMaterializationPlan, MaterializationDiagnostic> {
    derive_symbolic_materialization_with_inner_layouts(
        layout,
        &[],
        symbolic_fields,
        context,
        resolve,
    )
}

pub fn derive_symbolic_materialization_with_inner_layouts(
    layout: &LayoutPlanReport,
    inner_layouts: &[SymbolicFieldInnerLayout],
    symbolic_fields: &[SymbolicFieldValue],
    context: MaterializationContext,
    mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<SymbolicMaterializationPlan, MaterializationDiagnostic> {
    let byte_len =
        crate::materialization::fixed_layout_byte_len(layout, "symbolic materialization")?;
    let placement = context
        .placement
        .joined_with_layout(layout.align, byte_len)?;
    validate_materialization_field_identities(layout)?;

    let mut supplied = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for symbolic in symbolic_fields {
        let path_display = symbolic_path_display(symbolic);
        let hops = symbolic_path_hops(symbolic);
        if hops.len() > CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{path_display}` exceeds the compiler's {CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT}-segment record path bound"
            )));
        }
        if !names.insert(
            hops.iter()
                .map(|&(field, _, element_index)| (field, element_index))
                .collect::<Vec<_>>(),
        ) {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{path_display}` is supplied more than once"
            )));
        }
        if !supplied.insert(
            hops.iter()
                .map(|&(field, member_identity, element_index)| {
                    (
                        materialization_field_key(field, member_identity),
                        element_index,
                    )
                })
                .collect::<Vec<_>>(),
        ) {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{path_display}` repeats stable member identity #{}",
                hops.iter()
                    .find_map(|&(_, member_identity, _)| member_identity)
                    .expect("only numbered symbolic values can collide after name validation")
            )));
        }
    }
    let planned = planned_entries_by_key(layout);
    for symbolic in symbolic_fields {
        let key = materialization_field_key(&symbolic.field, symbolic.member_identity);
        let Some(entries) = planned.get(&key) else {
            let suffix = stable_identity_suffix(symbolic.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has no entry in the validated layout plan{suffix}",
                symbolic.field
            )));
        };
        let entry_names = entries
            .iter()
            .map(|entry| entry.field.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(drifted) = layout.entries.iter().find(|entry| {
            entry_names.contains(entry.field.as_str())
                && materialization_field_key(&entry.field, entry.member_identity) != key
        }) {
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` fragments do not retain one stable member identity",
                drifted.field
            )));
        }
    }

    let mut carrier_nodes = Vec::new();
    let (top_carriers, top_carrier_order) = prepare_inner_layouts(
        inner_layouts,
        &planned,
        byte_len,
        "",
        0,
        true,
        &mut carrier_nodes,
    )?;

    let mut traversed_inner = std::collections::BTreeSet::new();
    let prepared_writes = symbolic_fields
        .iter()
        .map(|symbolic| {
            let path_display = symbolic_path_display(symbolic);
            let hops = symbolic_path_hops(symbolic);
            let mut current_planned = &planned;
            let mut current_layout = layout;
            let mut current_carriers = &top_carriers;
            let mut current_byte_len = byte_len;
            let mut base_offset = 0_u64;
            let mut prefix = String::new();
            let mut writes = Vec::new();
            let last = hops.len() - 1;
            for (depth, &(field, member_identity, element_index)) in hops.iter().enumerate() {
                if depth > 0 {
                    prefix.push('.');
                }
                prefix.push_str(field);
                prefix.push_str(&symbolic_index_display(element_index));
                let key = materialization_field_key(field, member_identity);
                let Some(entries) = current_planned.get(&key) else {
                    let suffix = stable_identity_suffix(member_identity);
                    return Err(MaterializationDiagnostic(if depth == 0 {
                        format!(
                            "symbolic field `{field}` has no entry in the validated layout plan{suffix}"
                        )
                    } else {
                        format!(
                            "symbolic field `{path_display}` has no entry in the inner layout plan{suffix}"
                        )
                    }));
                };
                let entry_names = entries
                    .iter()
                    .map(|entry| entry.field.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                if let Some(drifted) = current_layout.entries.iter().find(|entry| {
                    entry_names.contains(entry.field.as_str())
                        && materialization_field_key(&entry.field, entry.member_identity) != key
                }) {
                    return Err(MaterializationDiagnostic(if depth == 0 {
                        format!(
                            "layout field `{}` fragments do not retain one stable member identity",
                            drifted.field
                        )
                    } else {
                        format!(
                            "inner layout field `{}` fragments do not retain one stable member identity",
                            drifted.field
                        )
                    }));
                }
                if depth == last
                    && let Some(&node_id) = current_carriers.get(&key)
                    && carrier_nodes[node_id].has_no_storage()
                {
                    return Err(MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` cannot write an empty interior"
                    )));
                }
                if depth == last {
                    let selected = select_materialization_entries(
                        entries,
                        element_index,
                        if depth == 0 { field } else { prefix.as_str() },
                    )?;
                    for entry in selected {
                        let mut write = write_from_entry(
                            entry,
                            symbolic,
                            if depth == 0 {
                                symbolic.field.as_str()
                            } else {
                                path_display.as_str()
                            },
                        )?;
                        validate_write(current_byte_len, &write)?;
                        write.container_byte_offset = base_offset
                            .checked_add(write.container_byte_offset)
                            .ok_or_else(|| {
                                MaterializationDiagnostic(format!(
                                    "symbolic field `{path_display}` composes an out-of-range destination offset"
                                ))
                            })?;
                        validate_write(byte_len, &write)?;
                        writes.push((entry.placement, write));
                    }
                    continue;
                }
                let Some(&node_id) = current_carriers.get(&key) else {
                    return Err(MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` has no supplied inner layout for `{prefix}`"
                    )));
                };
                let node = &carrier_nodes[node_id];
                let repetition = match &node.interior {
                    PreparedInterior::Record { repetition, .. } => {
                        repetition.map(|repeated| ("record", repeated))
                    }
                    PreparedInterior::Sum { repetition, .. } => {
                        repetition.map(|repeated| ("sum", repeated))
                    }
                };
                let enclosing_offset = match repetition {
                    Some((kind, (element_count, element_stride))) => {
                        let index = element_index.ok_or_else(|| {
                            MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` requires an element index into the repeated {kind} field `{prefix}`"
                            ))
                        })?;
                        if index >= element_count {
                            return Err(MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` element index {index} is outside its {element_count} element placements"
                            )));
                        }
                        let base = entries.iter().filter_map(|entry| match entry.placement {
                            LayoutPlacementReport::At { offset } => Some(offset),
                            _ => None,
                        }).min().ok_or_else(|| MaterializationDiagnostic(format!(
                            "inner layout for `{prefix}` has no whole `At` placement"
                        )))?;
                        base.checked_add(
                            index.checked_mul(element_stride).ok_or_else(|| {
                                MaterializationDiagnostic(format!(
                                    "symbolic field `{path_display}` composes an out-of-range interior offset"
                                ))
                            })?,
                        )
                        .ok_or_else(|| {
                            MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` composes an out-of-range interior offset"
                            ))
                        })?
                    }
                    _ => {
                        let selected = select_materialization_entries(
                            entries,
                            element_index,
                            if depth == 0 { field } else { prefix.as_str() },
                        )?;
                        let [enclosing_entry] = selected.as_slice() else {
                            return Err(MaterializationDiagnostic(if depth == 0 {
                                format!(
                                    "symbolic field `{path_display}` requires the outer field `{field}` to resolve to exactly one element placement, found {}",
                                    selected.len()
                                )
                            } else {
                                format!(
                                    "symbolic field `{path_display}` requires the enclosing field `{prefix}` to resolve to exactly one element placement, found {}",
                                    selected.len()
                                )
                            }));
                        };
                        let LayoutPlacementReport::At { offset } = enclosing_entry.placement else {
                            return Err(MaterializationDiagnostic(if depth == 0 {
                                format!(
                                    "symbolic field `{path_display}` requires the outer field `{field}` to use a whole `At` placement"
                                )
                            } else {
                                format!(
                                    "symbolic field `{path_display}` requires the enclosing field `{prefix}` to use a whole `At` placement"
                                )
                            }));
                        };
                        offset
                    }
                };
                let interior_end = usize::try_from(enclosing_offset)
                    .ok()
                    .and_then(|start| start.checked_add(node.byte_len))
                    .ok_or_else(|| {
                        MaterializationDiagnostic(format!(
                            "symbolic field `{path_display}` composes an out-of-range interior offset"
                        ))
                    })?;
                if interior_end > current_byte_len {
                    return Err(MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` interior layout for `{prefix}` exceeds the enclosing {current_byte_len}-byte record extent"
                    )));
                }
                traversed_inner.insert(node_id);
                base_offset = base_offset.checked_add(enclosing_offset).ok_or_else(|| {
                    MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` composes an out-of-range destination offset"
                    ))
                })?;
                match &node.interior {
                    PreparedInterior::Record {
                        planned,
                        layout,
                        nested,
                        ..
                    } => {
                        current_planned = planned;
                        current_layout = *layout;
                        current_carriers = nested;
                        current_byte_len = node.byte_len;
                    }
                    PreparedInterior::Sum {
                        layout: sum_layout, ..
                    } => {
                        let Some(&(case_field, case_identity, case_index)) = hops.get(depth + 1)
                        else {
                            return Err(MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` requires the sum path `{prefix}` to spell a selected case and payload field or a common field"
                            )));
                        };
                        if case_index.is_some() {
                            return Err(MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` case `{prefix}.{case_field}` cannot carry an element index"
                            )));
                        }
                        let case_key = materialization_field_key(case_field, case_identity);
                        let (leaf, leaf_display) = match sum_layout.cases.iter().find(|case| {
                            materialization_field_key(&case.case, case.member_identity) == case_key
                        }) {
                            Some(case) => {
                                let Some(&(payload_field, payload_identity, payload_index)) =
                                    hops.get(depth + 2)
                                else {
                                    return Err(MaterializationDiagnostic(format!(
                                        "symbolic field `{path_display}` requires a payload field below case `{prefix}.{case_field}`"
                                    )));
                                };
                                if depth + 2 != last {
                                    return Err(MaterializationDiagnostic(format!(
                                        "symbolic field `{path_display}` continues below sum payload `{prefix}.{case_field}.{payload_field}`; a case payload is the leaf of a sum path"
                                    )));
                                }
                                if payload_index.is_some() {
                                    return Err(MaterializationDiagnostic(format!(
                                        "symbolic field `{path_display}` payload `{prefix}.{case_field}.{payload_field}` cannot carry an element index; a sum payload retains one extent per field"
                                    )));
                                }
                                let payload_key =
                                    materialization_field_key(payload_field, payload_identity);
                                let payload = case
                                    .payload_fields
                                    .iter()
                                    .find(|payload| {
                                        materialization_field_key(
                                            &payload.field,
                                            payload.member_identity,
                                        ) == payload_key
                                    })
                                    .ok_or_else(|| {
                                        MaterializationDiagnostic(format!(
                                            "symbolic field `{path_display}` spells no payload field `{payload_field}` of case `{prefix}.{case_field}`"
                                        ))
                                    })?;
                                (
                                    payload,
                                    format!("{prefix}.{case_field}.{payload_field}"),
                                )
                            }
                            None => {
                                let common = sum_layout
                                    .common_fields
                                    .iter()
                                    .find(|common| {
                                        materialization_field_key(
                                            &common.field,
                                            common.member_identity,
                                        ) == case_key
                                    })
                                    .ok_or_else(|| {
                                        MaterializationDiagnostic(format!(
                                            "symbolic field `{path_display}` spells no case or common field `{case_field}` of the inner sum layout for `{prefix}`"
                                        ))
                                    })?;
                                if depth + 1 != last {
                                    return Err(MaterializationDiagnostic(format!(
                                        "symbolic field `{path_display}` continues below sum common field `{prefix}.{case_field}`; a common field is the leaf of a sum path"
                                    )));
                                }
                                (common, format!("{prefix}.{case_field}"))
                            }
                        };
                        let leaf_bits = leaf.size.checked_mul(8).ok_or_else(|| {
                            MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` composes an out-of-range payload extent"
                            ))
                        })?;
                        if u64::from(symbolic.width_bits) > leaf_bits {
                            return Err(MaterializationDiagnostic(format!(
                                "symbolic field `{path_display}` width {} exceeds the {leaf_bits}-bit sum member `{leaf_display}`",
                                symbolic.width_bits
                            )));
                        }
                        let leaf_entry = LayoutFieldEntryReport {
                            field: leaf.field.clone(),
                            member_identity: leaf.member_identity,
                            placement: LayoutPlacementReport::At { offset: leaf.offset },
                        };
                        let mut write =
                            write_from_entry(&leaf_entry, symbolic, &path_display)?;
                        validate_write(node.byte_len, &write)?;
                        write.container_byte_offset = base_offset
                            .checked_add(write.container_byte_offset)
                            .ok_or_else(|| {
                                MaterializationDiagnostic(format!(
                                    "symbolic field `{path_display}` composes an out-of-range destination offset"
                                ))
                            })?;
                        validate_write(byte_len, &write)?;
                        writes.push((leaf_entry.placement, write));
                        break;
                    }
                }
            }
            Ok(writes)
        })
        .collect::<Result<Vec<_>, MaterializationDiagnostic>>()?;

    if let Some(node_id) =
        first_untraversed_inner_layout(&carrier_nodes, &top_carrier_order, &traversed_inner)
    {
        return Err(MaterializationDiagnostic(format!(
            "no symbolic field path traverses the supplied inner layout for `{}`",
            carrier_nodes[node_id].path_display
        )));
    }

    let mut resolved_targets = std::collections::BTreeMap::new();
    let mut actions = Vec::new();
    for (symbolic_index, symbolic) in symbolic_fields.iter().enumerate() {
        let writes = &prepared_writes[symbolic_index];
        let resolved = if let Some(resolved) = resolved_targets.get(&symbolic.target) {
            *resolved
        } else {
            let resolved = resolve(symbolic.target);
            if let Some(source_value) = resolved {
                for (_, candidate_writes) in symbolic_fields
                    .iter()
                    .zip(&prepared_writes)
                    .filter(|(candidate, _)| candidate.target == symbolic.target)
                {
                    for (_, write) in candidate_writes {
                        validate_write_source_value(write, source_value, "symbolic")?;
                    }
                }
            }
            resolved_targets.insert(symbolic.target, resolved);
            resolved
        };
        for (placement, write) in writes.iter().cloned() {
            let action = match resolved {
                Some(source_value) => MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                },
                None if context.consumption == ConsumptionInstant::AfterOmegaHandoff => {
                    MaterializationAction::RuntimeWriter(write)
                }
                None => match placement {
                    LayoutPlacementReport::At { .. }
                        if context.native_pointer_relocation_bits == Some(symbolic.width_bits) =>
                    {
                        MaterializationAction::NativePointerRelocation {
                            field: write.field.clone(),
                            target: symbolic.target,
                            destination_byte_offset: write.container_byte_offset,
                            width_bits: symbolic.width_bits,
                        }
                    }
                    LayoutPlacementReport::At { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes symbolic field `{}` before Omega entry, but the target has no native {}-bit pointer relocation",
                            write.field, symbolic.width_bits
                        )));
                    }
                    LayoutPlacementReport::IntegerAt { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes stored-integer field `{}` before Omega entry; symbolic materialization has no integer fit proof",
                            write.field
                        )));
                    }
                    LayoutPlacementReport::Bits { .. } => {
                        return Err(MaterializationDiagnostic(format!(
                            "loader consumes fragmented symbolic field `{}` before Omega entry; unresolved fragments require a fixed address or a post-handoff writer",
                            write.field
                        )));
                    }
                },
            };
            actions.push(action);
        }
    }

    Ok(SymbolicMaterializationPlan {
        byte_len,
        byte_order: context.byte_order,
        placement,
        actions,
    })
}

enum PreparedInterior<'a> {
    Record {
        layout: &'a LayoutPlanReport,
        planned:
            std::collections::BTreeMap<MaterializationFieldKey, Vec<&'a LayoutFieldEntryReport>>,
        nested: std::collections::BTreeMap<MaterializationFieldKey, usize>,
        nested_order: Vec<usize>,
        repetition: Option<(u64, u64)>,
    },
    Sum {
        layout: &'a ConventionalSumLayoutReport,
        repetition: Option<(u64, u64)>,
    },
}

struct PreparedInnerLayout<'a> {
    byte_len: usize,
    interior: PreparedInterior<'a>,
    path_display: String,
}

impl PreparedInnerLayout<'_> {
    fn has_no_storage(&self) -> bool {
        let repetition = match self.interior {
            PreparedInterior::Record { repetition, .. }
            | PreparedInterior::Sum { repetition, .. } => repetition,
        };
        match repetition {
            Some((0, _)) => true,
            Some((count, stride)) => self.byte_len == 0 && (count == 1 || stride == 0),
            None => self.byte_len == 0,
        }
    }
}

fn prepare_inner_layouts<'a>(
    carriers: &'a [SymbolicFieldInnerLayout],
    planned: &std::collections::BTreeMap<MaterializationFieldKey, Vec<&'a LayoutFieldEntryReport>>,
    enclosing_byte_len: usize,
    path_prefix: &str,
    depth: usize,
    outermost: bool,
    nodes: &mut Vec<PreparedInnerLayout<'a>>,
) -> Result<
    (
        std::collections::BTreeMap<MaterializationFieldKey, usize>,
        Vec<usize>,
    ),
    MaterializationDiagnostic,
> {
    let mut bound = std::collections::BTreeMap::new();
    let mut order = Vec::new();
    for carrier in carriers {
        let path_display = format!("{path_prefix}{}", carrier.field);
        let key = materialization_field_key(&carrier.field, carrier.member_identity);
        if !planned.contains_key(&key) {
            return Err(MaterializationDiagnostic(if outermost {
                format!(
                    "inner layout for `{path_display}` binds to a field the validated layout plan does not contain"
                )
            } else {
                format!(
                    "inner layout for `{path_display}` binds to a field the enclosing interior layout plan does not contain"
                )
            }));
        }
        if bound.contains_key(&key) {
            return Err(MaterializationDiagnostic(format!(
                "inner layout for `{path_display}` is supplied more than once"
            )));
        }
        let repetition = carrier.inner_layout.repetition();
        let (interior, byte_len) = match carrier.inner_layout.interior() {
            SymbolicFieldInterior::Record(inner_layout) => prepare_record_interior(
                inner_layout,
                repetition,
                carrier,
                &path_display,
                depth,
                nodes,
            )?,
            SymbolicFieldInterior::Sum(sum_layout) => {
                if !carrier.inner_layouts.is_empty() {
                    return Err(MaterializationDiagnostic(format!(
                        "inner layout for `{path_display}` binds a sum interior; its case payload fields carry no nested record carriers"
                    )));
                }
                let byte_len = prepare_sum_interior(sum_layout, &path_display)?;
                if let Some((_, element_stride)) = repetition
                    && element_stride < sum_layout.size
                {
                    return Err(MaterializationDiagnostic(format!(
                        "inner layout for `{path_display}` strides repeated sum elements by {element_stride} bytes inside their {}-byte extent",
                        sum_layout.size
                    )));
                }
                (
                    PreparedInterior::Sum {
                        layout: sum_layout,
                        repetition,
                    },
                    byte_len,
                )
            }
        };
        let node = PreparedInnerLayout {
            byte_len,
            interior,
            path_display,
        };
        if node.has_no_storage() {
            let entries = &planned[&key];
            if !matches!(entries.as_slice(), [entry]
                if matches!(entry.placement, LayoutPlacementReport::At { offset }
                    if offset <= enclosing_byte_len as u64))
            {
                return Err(MaterializationDiagnostic(format!(
                    "empty inner layout for `{}` requires one whole `At` placement within the enclosing {enclosing_byte_len}-byte record extent",
                    node.path_display
                )));
            }
        } else {
            let entries = &planned[&key];
            let mut offsets = Vec::with_capacity(entries.len());
            for entry in entries {
                let LayoutPlacementReport::At { offset } = entry.placement else {
                    return Err(MaterializationDiagnostic(format!(
                        "inner layout for `{}` requires a whole `At` placement",
                        node.path_display
                    )));
                };
                offsets.push(offset);
            }
            offsets.sort_unstable();
            let extent = match repetition {
                Some((count, stride)) if offsets.len() == 1 => count
                    .checked_sub(1)
                    .and_then(|last| last.checked_mul(stride))
                    .and_then(|span| span.checked_add(byte_len as u64)),
                Some((count, stride)) => {
                    if u64::try_from(offsets.len()).ok() != Some(count)
                        || offsets
                            .windows(2)
                            .any(|pair| pair[1].checked_sub(pair[0]) != Some(stride))
                    {
                        return Err(MaterializationDiagnostic(format!(
                            "inner layout for `{}` element placements drift from its count or stride",
                            node.path_display
                        )));
                    }
                    Some(byte_len as u64)
                }
                None => Some(byte_len as u64),
            };
            if offsets.iter().any(|offset| {
                extent
                    .and_then(|extent| offset.checked_add(extent))
                    .is_none_or(|end| end > enclosing_byte_len as u64)
            }) {
                return Err(MaterializationDiagnostic(format!(
                    "interior layout for `{}` exceeds the enclosing {enclosing_byte_len}-byte record extent",
                    node.path_display
                )));
            }
        }
        let node_id = nodes.len();
        nodes.push(node);
        bound.insert(key, node_id);
        order.push(node_id);
    }
    Ok((bound, order))
}

fn prepare_record_interior<'a>(
    interior_layout: &'a LayoutPlanReport,
    repetition: Option<(u64, u64)>,
    carrier: &'a SymbolicFieldInnerLayout,
    path_display: &str,
    depth: usize,
    nodes: &mut Vec<PreparedInnerLayout<'a>>,
) -> Result<(PreparedInterior<'a>, usize), MaterializationDiagnostic> {
    let interior_size = interior_layout.size.ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "inner layout for `{path_display}` requires a fixed-size interior layout plan"
        ))
    })?;
    let inner_byte_len = usize::try_from(interior_size).map_err(|_| {
        MaterializationDiagnostic(format!(
            "inner layout size {interior_size} for `{path_display}` cannot be represented on this compiler host"
        ))
    })?;
    if let Some((_, element_stride)) = repetition
        && element_stride < interior_size
    {
        return Err(MaterializationDiagnostic(format!(
            "inner layout for `{path_display}` strides repeated record elements by {element_stride} bytes inside their {interior_size}-byte extent"
        )));
    }
    validate_materialization_field_identities(interior_layout)?;
    let planned_inner = planned_entries_by_key(interior_layout);
    let (nested, nested_order) = if carrier.inner_layouts.is_empty() {
        (std::collections::BTreeMap::new(), Vec::new())
    } else {
        if depth + 1 >= CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
            return Err(MaterializationDiagnostic(format!(
                "inner layout for `{path_display}` nests beyond the compiler's {CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT}-segment record path bound"
            )));
        }
        prepare_inner_layouts(
            &carrier.inner_layouts,
            &planned_inner,
            inner_byte_len,
            &format!("{path_display}."),
            depth + 1,
            false,
            nodes,
        )?
    };
    Ok((
        PreparedInterior::Record {
            layout: interior_layout,
            planned: planned_inner,
            nested,
            nested_order,
            repetition,
        },
        inner_byte_len,
    ))
}

fn prepare_sum_interior(
    layout: &ConventionalSumLayoutReport,
    path_display: &str,
) -> Result<usize, MaterializationDiagnostic> {
    let byte_len = usize::try_from(layout.size).map_err(|_| {
        MaterializationDiagnostic(format!(
            "inner sum layout size {} for `{path_display}` cannot be represented on this compiler host",
            layout.size
        ))
    })?;
    let tag_end = layout
        .tag_offset
        .checked_add(layout.tag_size)
        .ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "inner layout for `{path_display}` composes an out-of-range sum tag"
            ))
        })?;
    if tag_end > layout.size {
        return Err(MaterializationDiagnostic(format!(
            "inner layout for `{path_display}` places the sum tag outside its {}-byte extent",
            layout.size
        )));
    }
    for common in &layout.common_fields {
        let common_end = common.offset.checked_add(common.size).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "inner layout for `{path_display}` composes an out-of-range sum common field `{}`",
                common.field
            ))
        })?;
        if common_end > layout.size {
            return Err(MaterializationDiagnostic(format!(
                "inner layout for `{path_display}` places common field `{}` outside the sum's {}-byte extent",
                common.field, layout.size
            )));
        }
    }
    for case in &layout.cases {
        for payload in &case.payload_fields {
            let payload_end = payload.offset.checked_add(payload.size).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "inner layout for `{path_display}` composes an out-of-range sum payload field `{}`",
                    payload.field
                ))
            })?;
            if payload_end > layout.size {
                return Err(MaterializationDiagnostic(format!(
                    "inner layout for `{path_display}` places payload field `{}` of case `{}` outside the sum's {}-byte extent",
                    payload.field, case.case, layout.size
                )));
            }
        }
    }
    validate_conventional_sum_materialization_identities(layout)?;
    Ok(byte_len)
}

fn first_untraversed_inner_layout(
    nodes: &[PreparedInnerLayout<'_>],
    order: &[usize],
    traversed: &std::collections::BTreeSet<usize>,
) -> Option<usize> {
    for &node_id in order {
        if nodes[node_id].has_no_storage() {
            continue;
        }
        if !traversed.contains(&node_id) {
            return Some(node_id);
        }
        if let PreparedInterior::Record { nested_order, .. } = &nodes[node_id].interior
            && let Some(deeper) = first_untraversed_inner_layout(nodes, nested_order, traversed)
        {
            return Some(deeper);
        }
    }
    None
}

fn select_materialization_entries<'a>(
    entries: &'a [&'a LayoutFieldEntryReport],
    element_index: Option<u64>,
    field_display: &str,
) -> Result<Vec<&'a LayoutFieldEntryReport>, MaterializationDiagnostic> {
    let Some(element_index) = element_index else {
        return Ok(entries.to_vec());
    };
    if entries
        .iter()
        .any(|entry| !matches!(entry.placement, LayoutPlacementReport::At { .. }))
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` element index {element_index} cannot address a fragmented or stored-integer placement"
        )));
    }
    let mut elements = entries.to_vec();
    elements.sort_by_key(|entry| match entry.placement {
        LayoutPlacementReport::At { offset } => offset,
        LayoutPlacementReport::IntegerAt { .. } | LayoutPlacementReport::Bits { .. } => u64::MAX,
    });
    let index = usize::try_from(element_index).map_err(|_| {
        MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` element index {element_index} cannot be represented on this host"
        ))
    })?;
    let Some(entry) = elements.get(index) else {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` element index {element_index} is outside its {} element placements",
            elements.len()
        )));
    };
    Ok(vec![*entry])
}

fn write_from_entry(
    entry: &LayoutFieldEntryReport,
    symbolic: &SymbolicFieldValue,
    field_display: &str,
) -> Result<MaterializationWrite, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) =
        materializer_scalar_window(
            entry.placement,
            symbolic.width_bits,
            "symbolic",
            field_display,
        )?;
    Ok(MaterializationWrite {
        field: field_display.to_owned(),
        target: symbolic.target,
        container_byte_offset: container,
        container_width_bits: container_width,
        destination_lsb,
        source_lsb,
        width,
        stored_integer_fit: match entry.placement {
            LayoutPlacementReport::IntegerAt {
                stored_width,
                interpretation,
                ..
            } => Some(StoredIntegerFit {
                source_width_bits: symbolic.width_bits,
                stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                    MaterializationDiagnostic(format!(
                        "symbolic field `{field_display}` has an invalid stored-integer width"
                    ))
                })?,
                interpretation,
            }),
            LayoutPlacementReport::At { .. } | LayoutPlacementReport::Bits { .. } => None,
        },
    })
}

pub(crate) fn apply_scalar_entry(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_value(entry, value)?;
    let fragment = scalar_fragment(entry, value.width_bits)?;
    validate_fragment(
        bytes.len(),
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
    )?;
    apply_fragment(
        bytes,
        byte_order,
        &value.field,
        fragment.container_byte_offset,
        fragment.container_width_bits,
        fragment.destination_lsb,
        fragment.source_lsb,
        fragment.width,
        value.value,
    )
}
