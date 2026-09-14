//! Derivation of a phase-aware symbolic materialization plan from a layout
//! report, symbolic field values, and resolved relocation targets, including
//! bounded traversal of interior layouts along symbolic paths.

use crate::field_values::ScalarFieldValue;
use crate::layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use crate::materialization::{MaterializationDiagnostic, SymbolicMaterializationPlan};
use crate::materialization_field_identities::{
    MaterializationFieldKey, materialization_field_key, stable_identity_suffix,
    symbolic_index_display, symbolic_path_display, symbolic_path_hops,
    validate_materialization_field_identities,
};
use crate::placement::{
    ByteOrder, ConsumptionInstant, MaterializationAction, MaterializationContext,
    MaterializationWrite, StoredIntegerFit,
};
use crate::stored_integer_writes::{
    apply_fragment, scalar_fragment, validate_fragment, validate_stored_integer_value,
    validate_write, validate_write_source_value,
};
use crate::symbolic_values::{RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldValue};

/// Derives a phase-aware consumer plan. `resolve` is compiler/provider
/// infrastructure; source code never receives its returned address.
///
/// A symbolic value spelling inner hops (`outer.field`, `outer.field.sub`)
/// additionally needs the interior layout carriers of the records it crosses;
/// derive those paths through
/// [`derive_symbolic_materialization_with_inner_layouts`].
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

/// `derive_symbolic_materialization` extended with interior layout carriers.
///
/// `inner_layouts` carries the compiler-derived interior layout of a nested
/// record beside the flat outer plan. A symbolic value spelling inner hops
/// (`outer.field`, `outer[index].field`, `outer.field[index]`, or deeper
/// chains like `outer.field.sub`) resolves each crossed record boundary's
/// `At` placement in turn, then selects inside the innermost element's
/// retained interior layout, so the exact path stays symbolic until the write
/// offset is assigned. Each carrier's own `inner_layouts` binds the next
/// boundary's interior, so record depth is data the traversal walks rather
/// than a family of depth-specific implementations; the walk is bounded by
/// [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`]. Supplying an inner layout no
/// symbolic path traverses is rejected: a carrier that outlives the semantic
/// path it describes would let a stale interior join a renamed or reshaped
/// schema.
pub fn derive_symbolic_materialization_with_inner_layouts(
    layout: &LayoutPlanReport,
    inner_layouts: &[SymbolicFieldInnerLayout],
    symbolic_fields: &[SymbolicFieldValue],
    context: MaterializationContext,
    mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<SymbolicMaterializationPlan, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "symbolic materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    let placement = context
        .placement
        .joined_with_layout(layout.align, byte_len)?;
    validate_materialization_field_identities(layout)?;

    // `field[index]` is a distinct semantic slot from `field[j]` and from the
    // whole-field `field`, so the exact index joins the name and identity when
    // detecting a duplicate supply. Each inner hop does the same: `outer.field`
    // is a distinct slot from `outer`, from `outer.field[index]`, and from
    // `outer.field.sub`. Without the complete path two elements of one array
    // or two members of nested records would collide even though they write
    // disjoint placements. The segment bound is a compiler resource limit, not
    // a language limit: it caps the work a malformed or adversarial path can
    // make derivation perform.
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
    let mut planned =
        std::collections::BTreeMap::<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>>::new();
    for entry in &layout.entries {
        planned
            .entry(materialization_field_key(
                &entry.field,
                entry.member_identity,
            ))
            .or_default()
            .push(entry);
    }
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

    // Bind each supplied inner carrier to its field key before any symbolic
    // path resolves through it. A carrier is evidence the compiler produced
    // for one named field; a duplicate or a carrier for a field the enclosing
    // plan never placed would silently substitute one record's interior for
    // another's. Carriers nest the same way records do: each boundary's own
    // `inner_layouts` binds the interior of the record fields inside its
    // interior, so a deeper path resolves against the same shape the compiler
    // derived. Preparation is one bounded recursion over the carrier tree.
    let mut carrier_nodes = Vec::new();
    let (top_carriers, top_carrier_order) =
        prepare_inner_layouts(inner_layouts, &planned, "", 0, true, &mut carrier_nodes)?;

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
            // Each segment resolves inside the plan the previous record
            // boundary supplied: the outer validated plan at the first hop,
            // then the enclosing element's retained interior. The leaf writes
            // inside its own record extent; every earlier hop must name the
            // one element the rest of the path lives in.
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
                let selected = select_materialization_entries(
                    entries,
                    element_index,
                    if depth == 0 { field } else { prefix.as_str() },
                )?;
                if depth == last {
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
                        // The leaf member may not escape its enclosing
                        // record's own extent: the outer check below bounds
                        // the composed write by the whole plan, but only this
                        // per-level bound keeps a malformed carrier from
                        // reaching into neighboring fields.
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
                // The next hop needs one enclosing element. An unindexed
                // segment on a repeated record covers several elements, so
                // the path below it cannot name a destination;
                // `outer[i].field` spells which element the member belongs to.
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
                let Some(&node_id) = current_carriers.get(&key) else {
                    return Err(MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` has no supplied inner layout for `{prefix}`"
                    )));
                };
                let node = &carrier_nodes[node_id];
                // The claimed interior must fit inside the enclosing record's
                // extent at this offset. Per-level bounds compose: the leaf
                // check above bounds the member inside this interior, and
                // this bound keeps the whole interior inside its enclosing
                // element, so a malformed carrier cannot place writes past
                // the record boundary it describes.
                let interior_end = usize::try_from(offset)
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
                base_offset = base_offset.checked_add(offset).ok_or_else(|| {
                    MaterializationDiagnostic(format!(
                        "symbolic field `{path_display}` composes an out-of-range destination offset"
                    ))
                })?;
                current_planned = &node.planned;
                current_layout = node.layout;
                current_carriers = &node.nested;
                current_byte_len = node.byte_len;
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

/// One interior layout carrier prepared for symbolic traversal: the carrier's
/// validated interior plan indexed by field key, its fixed extent, and the
/// prepared carriers bound to record fields inside it. Nodes live in one
/// arena so the traversal walks record depth as data instead of recursing a
/// type the compiler already knows.
struct PreparedInnerLayout<'a> {
    /// The carrier's validated interior plan; drift checks read its entries.
    layout: &'a LayoutPlanReport,
    /// The interior record's fixed extent in bytes. Every member write and
    /// every nested record boundary inside the carrier must fit within it.
    byte_len: usize,
    /// Interior entries indexed by materialization key.
    planned: std::collections::BTreeMap<MaterializationFieldKey, Vec<&'a LayoutFieldEntryReport>>,
    /// Prepared nested carriers by the enclosing field key they bind to.
    nested: std::collections::BTreeMap<MaterializationFieldKey, usize>,
    /// Nested carrier node ids in supply order, so untraversed reporting is
    /// deterministic rather than key order.
    nested_order: Vec<usize>,
    /// Diagnostic spelling of the record path this carrier serves (`slot`,
    /// `slot.sub`).
    path_display: String,
}

/// Validates and indexes one level of supplied interior carriers. `planned`
/// is the enclosing plan's field-keyed entries — the outer validated plan at
/// the top level, or the parent carrier's interior below it. Each carrier's
/// own `inner_layouts` recurses through this same preparation, bounded by
/// [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`] so a malformed carrier tree
/// cannot make preparation unbounded. Returns the key-indexed carrier map
/// plus the node ids in supply order.
fn prepare_inner_layouts<'a>(
    carriers: &'a [SymbolicFieldInnerLayout],
    planned: &std::collections::BTreeMap<MaterializationFieldKey, Vec<&'a LayoutFieldEntryReport>>,
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
        let inner_byte_len = carrier
            .inner_layout
            .size
            .ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "inner layout for `{path_display}` requires a fixed-size interior layout plan"
                ))
            })
            .and_then(|size| {
                usize::try_from(size).map_err(|_| {
                    MaterializationDiagnostic(format!(
                        "inner layout size {size} for `{path_display}` cannot be represented on this compiler host"
                    ))
                })
            })?;
        validate_materialization_field_identities(&carrier.inner_layout)?;
        let mut planned_inner = std::collections::BTreeMap::<
            MaterializationFieldKey,
            Vec<&LayoutFieldEntryReport>,
        >::new();
        for entry in &carrier.inner_layout.entries {
            planned_inner
                .entry(materialization_field_key(
                    &entry.field,
                    entry.member_identity,
                ))
                .or_default()
                .push(entry);
        }
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
                &format!("{path_display}."),
                depth + 1,
                false,
                nodes,
            )?
        };
        let node_id = nodes.len();
        nodes.push(PreparedInnerLayout {
            layout: &carrier.inner_layout,
            byte_len: inner_byte_len,
            planned: planned_inner,
            nested,
            nested_order,
            path_display,
        });
        bound.insert(key, node_id);
        order.push(node_id);
    }
    Ok((bound, order))
}

/// Finds the first supplied interior carrier no symbolic path traversed, in
/// supply order with a parent before its nested carriers. A carrier whose
/// parent is untraversed can never be reached, so the parent reports first.
fn first_untraversed_inner_layout(
    nodes: &[PreparedInnerLayout<'_>],
    order: &[usize],
    traversed: &std::collections::BTreeSet<usize>,
) -> Option<usize> {
    for &node_id in order {
        if !traversed.contains(&node_id) {
            return Some(node_id);
        }
        if let Some(deeper) =
            first_untraversed_inner_layout(nodes, &nodes[node_id].nested_order, traversed)
        {
            return Some(deeper);
        }
    }
    None
}

/// Resolves a symbolic `element_index` to the exact element `At` placement it
/// names. An index is the second hop of a field/index path (`field[index]`):
/// every layout entry for the field must be an element `At`, and the index must
/// name one of those elements. Entries are ordered by offset so the index
/// selects the semantic element rather than whichever entry the producer
/// emitted first. Without an index the symbolic value covers every placement,
/// which is how a fragmented field is tiled. The bound is checked here, before
/// any destination byte offset is assigned, so physical lowering cannot change
/// which element is accessed.
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
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(symbolic.width_bits),
            0,
            0,
            u64::from(symbolic.width_bits),
        ),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            ..
        } => (offset, stored_width, 0, 0, stored_width),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` uses a materializer-incompatible placement"
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("symbolic source bit range overflows".into()))?;
    if source_end > u64::from(symbolic.width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` placement reads through bit {source_end}, past its {}-bit source",
            symbolic.width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("symbolic destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{field_display}` placement writes through bit {destination_end}, past its {container_width}-bit container"
        )));
    }
    Ok(MaterializationWrite {
        field: field_display.to_owned(),
        target: symbolic.target,
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "symbolic field `{field_display}` fragment width {width} is too large"
            ))
        })?,
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
