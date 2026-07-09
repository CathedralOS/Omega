use crate::EmissionPlanningInput;
use crate::blocker;
use omega_backend_report_types::EmissionBlocker;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_layout::{
    FieldLayout, LayoutPlan, MachineLayout, field_data_layout_fields, field_machine_layout,
};

/// Contained-machine method dispatch resolves the receiver's storage region by
/// TYPE: `nested_machine_storage_offset` (instruction selection, machine_owned.rs)
/// walks the caller's fields depth-first and takes the FIRST field whose type
/// matches the callee's attached data. With two same-type contained machines
/// (`a: Counter; b: Counter`), a call through `b` silently mutates `a` -- the
/// receiver FIELD never reaches storage resolution (the deep fix is threading
/// it through the dispatch; memory: contained-machine-same-type-aliasing).
///
/// Until that lands, reject exactly the calls the walk would misresolve: the
/// state-call plan carries `receiver_symbol`, so compare the receiver field's
/// actual offset against the offset the by-type walk picks. Equal -> the call
/// is sound (single instance, or the receiver IS the first match) and stays
/// accepted; different -> the call would run on another instance's storage,
/// which is never intended.
///
/// NESTED receivers (`self.p.second.method()`, un-gated by validation rung 3)
/// hold a STRICTER bar: the plan's `receiver_path` must fully walk the field
/// layouts to a storage offset AND that offset must equal the by-type walk's
/// pick -- anything unprovable blocks LOUDLY. (Direct-field receivers keep
/// the lenient skip-if-unresolved: locals/params as receivers resolve by
/// other routes and predate this fence.)
pub(crate) fn collect_contained_receiver_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.reachable || state_call.receiver_name.as_str().is_empty() {
            continue;
        }
        if state_call.source_key.machine == state_call.target_key.machine {
            continue;
        }
        let Some(source_layout) =
            machine_layout_by_symbol(input.layouts, state_call.source_key.machine)
        else {
            continue;
        };
        let Some(target_layout) =
            machine_layout_by_symbol(input.layouts, state_call.target_key.machine)
        else {
            continue;
        };
        let Some(target_attached_data) = target_layout.attached_data.as_deref() else {
            continue;
        };

        // PER-INSTANCE RECEIVERS (TASKS_FS "Stolen work #2"), BOTH routes:
        // - DISPATCH-ROUTED calls (state_call_routed_to_dispatch) run their
        //   clones on the receiver's true storage via the per-dispatch table
        //   (pinned by calls/runtime_dispatch_second_receiver_exit).
        // - INLINE calls serve when the receiver is recoverable at
        //   resolution time: the state's UNIQUE call to that callee machine
        //   (receiver_base_for's spliced-callee lookup; ambiguity -- two
        //   same-callee-machine calls in one state -- stays fenced; pinned
        //   by time/runtime_value_machine_receiver_field_postentry_exit).
        // Both require the entry-machine caller + a resolvable named
        // receiver path (the shared omega_layout walk -- the exact
        // compute_receiver_bases predicate; re-derived here because the
        // fence iterates CALLS, not dispatch cases).
        let inline_served = state_call.source_key.machine == input.entry_key.machine
            && input
                .state_calls
                .calls
                .iter()
                .filter(|(_, other)| {
                    other.reachable
                        && other.source_key.machine == state_call.source_key.machine
                        && other.source_key.state == state_call.source_key.state
                        && other.target_key.machine == state_call.target_key.machine
                })
                .count()
                == 1;
        if state_call.source_key.machine == input.entry_key.machine
            && (crate::dispatch_route::state_call_routed_to_dispatch(input, state_call)
                || inline_served)
        {
            let segments = input
                .state_calls
                .receiver_path_segments
                .span(state_call.receiver_path)
                .unwrap_or(&[]);
            let walk_segments = match segments.first() {
                Some(root) if root.as_str() == "self" => &segments[1..],
                _ => segments,
            };
            let resolved = if walk_segments.is_empty() {
                omega_layout::field_path_offset(
                    input.layouts,
                    source_layout.fields,
                    std::slice::from_ref(&state_call.receiver_name),
                )
            } else {
                omega_layout::field_path_offset(input.layouts, source_layout.fields, walk_segments)
            };
            if resolved.is_some() {
                continue;
            }
        }

        // SLICE 2 (NON-entry callers), DISPATCH route only: serve exactly when
        // the pipeline COMPOSED a base for EVERY clone this call minted -- the
        // per-dispatch table is the single prediction site, consulted directly
        // (no re-derived predicate): every runtime-flow state whose context
        // was minted by THIS call must carry `Some` in `receiver_bases`, and
        // at least one such state must exist. Zero-size receivers emit `None`
        // there by design and fall through to the by-type compare below
        // (single-instance zero-size receivers pass it; that route needs no
        // base at all).
        if state_call.source_key.machine != input.entry_key.machine
            && crate::dispatch_route::state_call_routed_to_dispatch(input, state_call)
        {
            let mut minted_any = false;
            let mut all_composed = true;
            for (handle, state) in input.runtime_flow.states.iter() {
                let Some((call_key, statement_index, _)) = input
                    .runtime_flow
                    .context_call_sites
                    .get(state.context.0 as usize)
                else {
                    continue;
                };
                if *call_key != state_call.source_key
                    || *statement_index != state_call.statement_index
                {
                    continue;
                }
                minted_any = true;
                all_composed &= input
                    .receiver_bases
                    .get(handle.arena_index() as usize)
                    .copied()
                    .flatten()
                    .is_some();
            }
            if minted_any && all_composed {
                continue;
            }
        }

        // The receiver's FIELD path: the plan's root->leaf spelled segments
        // with the `self` root stripped. Matched by NAME throughout: receiver
        // symbols and layout field symbols live in different arenas, so
        // handle equality can never hold.
        let path_segments = input
            .state_calls
            .receiver_path_segments
            .span(state_call.receiver_path)
            .unwrap_or(&[]);
        let field_segments = match path_segments.first() {
            Some(root) if root.as_str() == "self" => &path_segments[1..],
            _ => path_segments,
        };

        let walk_offset = first_type_match_offset(
            input.layouts,
            source_layout,
            state_call.target_key.machine,
            target_attached_data,
            0,
            &mut Vec::new(),
        );

        if field_segments.len() > 1 {
            // Nested receiver: prove the chain's storage offset and require
            // it to be the offset dispatch will pick; block anything else.
            let spelled = spelled_path(field_segments);
            match (
                receiver_path_offset(input.layouts, source_layout, field_segments),
                walk_offset,
            ) {
                (Some(true_offset), Some(predicted)) if true_offset == predicted => {}
                (Some(true_offset), Some(predicted)) => {
                    blockers.insert(blocker(
                        "state calls",
                        &format!(
                            "nested receiver `self.{}` lives at offset {}, but dispatch \
                             resolves the receiver region by TYPE and picks the first \
                             `{}` (offset {}) -- the call would run on ANOTHER \
                             instance's storage. Until per-instance dispatch lands, \
                             give each instance a distinct data type, or add a \
                             forwarding method on the outer type.",
                            spelled, true_offset, target_attached_data, predicted,
                        ),
                    ));
                }
                _ => {
                    blockers.insert(blocker(
                        "state calls",
                        &format!(
                            "nested receiver `self.{}`: cannot prove the receiver's \
                             storage offset matches the region dispatch resolves \
                             (by-TYPE walk for `{}`). Add a forwarding method on the \
                             outer type.",
                            spelled, target_attached_data,
                        ),
                    ));
                }
            }
            continue;
        }

        // Machine-to-machine self calls (`self.probe(..)` between machines
        // attached to the SAME data) dispatch on the caller's own region --
        // no by-type receiver walk is involved (D10; wrappers rely on it).
        if state_call.receiver_name.as_str() == "self" {
            continue;
        }

        // The STATIC spelling (`Worker::run(pair)`, `Duration::from_secs(n)`)
        // carries the TYPE name in receiver position: the callee is
        // receiverless (or constructs its own), reads no receiver storage,
        // and its by-value params deliver via the leaf expansion path
        // (runtime-pinned by calls/runtime_attached_machine_struct_arg_exit).
        if state_call.receiver_name.as_str() == target_attached_data {
            continue;
        }

        // Direct-field receiver: the original lenient compare.
        let Some(receiver_field) =
            input
                .layouts
                .fields
                .span(source_layout.fields)
                .and_then(|fields| {
                    fields
                        .iter()
                        .find(|field| field.name == state_call.receiver_name)
                })
        else {
            // A PARAM or LOCAL receiver (`meta.is_file()` where `meta` is a
            // state param): dispatch's by-TYPE walk can only reach FIELDS, so
            // it either reads the first same-typed field's storage (silently
            // answering for ANOTHER instance -- probed: a decoy field with
            // different data changes the result) or, with no matching field,
            // an unrelated/ZII region. Never the param's actual storage.
            // Block loudly until receiver storage threads through dispatch.
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "method call receiver `{}` is a parameter or local, but dispatch \
                     resolves the receiver region by TYPE over the caller's FIELDS -- \
                     the call would read another instance's storage (or ZII), never \
                     `{}` itself. Copy `{}` into a field of the caller's data and \
                     call through that field, or inline the method's expression.",
                    state_call.receiver_name, state_call.receiver_name, state_call.receiver_name,
                ),
            ));
            continue;
        };
        let Some(walk_offset) = walk_offset else {
            continue;
        };
        if walk_offset == receiver_field.offset {
            continue;
        }
        blockers.insert(blocker(
            "state calls",
            &format!(
                "contained-machine call receiver `{}` (a `{}` at offset {}) would run on \
                 ANOTHER instance's storage: dispatch currently resolves the receiver \
                 region by TYPE and picks the first `{}` (offset {}). Until per-instance \
                 dispatch lands, give each instance a distinct data type, or mutate \
                 `{}`'s fields directly instead of through a method call.",
                receiver_field.name,
                receiver_field.type_name,
                receiver_field.offset,
                target_attached_data,
                walk_offset,
                receiver_field.name,
            ),
        ));
    }
}

/// Walk the receiver's field path through the layout plan, accumulating each
/// segment's offset; descend into intermediate segments' machine layouts.
/// `None` when any segment is not a field of the current layout (or an
/// intermediate has no machine layout to descend into) -- callers treat
/// unprovable as blocked.
fn receiver_path_offset(
    layouts: &LayoutPlan,
    source_layout: &MachineLayout,
    field_segments: &[omega_checked_trees::name::Identifier],
) -> Option<usize> {
    // The SHARED walk (omega_layout::field_path_offset): per-instance
    // dispatch resolution and this fence agree by construction.
    omega_layout::field_path_offset(layouts, source_layout.fields, field_segments)
}

fn spelled_path(segments: &[omega_checked_trees::name::Identifier]) -> String {
    segments
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn machine_layout_by_symbol(layouts: &LayoutPlan, machine: SymbolHandle) -> Option<&MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == machine)
        .map(|(_, machine_layout)| machine_layout)
}

/// Mirror of instruction selection's `nested_machine_storage_offset` FIELD WALK
/// (machine_owned.rs): depth-first over the caller's fields, first match by the
/// callee's attached-data type name (or by nested machine symbol), earlier
/// fields' nested matches win over later direct matches. Descends both nested
/// machine-typed fields AND plain-DATA fields -- KEEP IN LOCKSTEP with the
/// backend walk; this predicts which offset the backend will resolve.
fn first_type_match_offset(
    layouts: &LayoutPlan,
    machine_layout: &MachineLayout,
    target_machine: SymbolHandle,
    target_attached_data: &str,
    base_offset: usize,
    visited: &mut Vec<SymbolHandle>,
) -> Option<usize> {
    if visited.contains(&machine_layout.symbol) {
        return None;
    }
    visited.push(machine_layout.symbol);
    let offset = first_type_match_offset_in_span(
        layouts,
        machine_layout.fields,
        target_machine,
        target_attached_data,
        base_offset,
        visited,
    );
    visited.pop();
    offset
}

fn first_type_match_offset_in_span(
    layouts: &LayoutPlan,
    fields_span: HandleSpan<FieldLayout>,
    target_machine: SymbolHandle,
    target_attached_data: &str,
    base_offset: usize,
    visited: &mut Vec<SymbolHandle>,
) -> Option<usize> {
    let fields = layouts.fields.span(fields_span)?;

    for field in fields {
        let field_offset = base_offset + field.offset;

        if field.type_name.as_ref() == target_attached_data {
            return Some(field_offset);
        }

        if let Some(nested) = field_machine_layout(layouts, field) {
            if nested.symbol == target_machine {
                return Some(field_offset);
            }
            if let Some(offset) = first_type_match_offset(
                layouts,
                nested,
                target_machine,
                target_attached_data,
                field_offset,
                visited,
            ) {
                return Some(offset);
            }
            continue;
        }

        if let Some(data_fields) = field_data_layout_fields(layouts, field)
            && let Some(offset) = first_type_match_offset_in_span(
                layouts,
                data_fields,
                target_machine,
                target_attached_data,
                field_offset,
                visited,
            )
        {
            return Some(offset);
        }
    }

    None
}
