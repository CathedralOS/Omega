use crate::EmissionPlanningInput;
use crate::blocker;
use omega_backend_report_types::EmissionBlocker;
use omega_layout::{
    FieldLayout, LayoutPlan, MachineLayout, field_data_layout_fields, field_machine_layout,
};
use psi_arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;

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
    // SPLICED-LIVE liveness (2026-07-11g): a call spliced through inline
    // hops keeps its record `reachable = false` (the original state is never
    // scheduled) while its copy EXECUTES inside a case -- skipping
    // unreachable calls made fully-inline chains fence-invisible, so an
    // AMBIGUOUS receiver (two same-family calls, chain walk refuses,
    // by-type fallback reads the FIRST instance) was silent-wrong (probed
    // native 71 vs interp 70). A machine is live when the entry (or a live
    // machine) calls into it; a call is examined when reachable OR its
    // source machine is live.
    let live_machines = spliced_live_machines(input);
    // The per-machine COMPOSED storage base, mirroring the runtime chain
    // walk's anchor+hop math (entry = 0; each live call adds its receiver
    // offset within the source machine's layout; self calls on the same
    // data add 0). A machine reached with two DISTINCT bases (or through an
    // unresolvable hop) is poisoned (`None`) -- calls out of it cannot
    // prove their instance and stay refusable.
    let machine_bases = composed_machine_bases(input, &live_machines);
    for (_, state_call) in input.state_calls.calls.iter() {
        if state_call.receiver_name.as_str().is_empty() {
            continue;
        }
        if !state_call.reachable && !live_machines.contains(&state_call.source_key.machine) {
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

        // A receiver parameter whose exact caller-field base was retained in
        // the composed environment is served by receiver_base_for's context
        // walk. Keep the same uniqueness condition as the spliced-call path:
        // two calls into one attached-data family from the same source state
        // could select different parameter instances and must remain fenced.
        let receiver_is_source_field =
            input
                .layouts
                .fields
                .span(source_layout.fields)
                .is_some_and(|fields| {
                    fields
                        .iter()
                        .any(|field| field.name == state_call.receiver_name)
                });
        let parameter_base_is_bound = !receiver_is_source_field
            && machine_bases
                .iter()
                .find(|(machine, _)| *machine == state_call.source_key.machine)
                .is_some_and(|(_, anchor)| {
                    anchor
                        .params
                        .iter()
                        .any(|(name, base)| name == &state_call.receiver_name && base.is_some())
                });
        let unique_parameter_target = input
            .state_calls
            .calls
            .iter()
            .filter(|(_, other)| {
                (other.reachable || live_machines.contains(&other.source_key.machine))
                    && other.source_key.machine == state_call.source_key.machine
                    && other.source_key.state == state_call.source_key.state
                    && (other.target_key.machine == state_call.target_key.machine
                        || machine_layout_by_symbol(input.layouts, other.target_key.machine)
                            .and_then(|layout| layout.attached_data.as_deref())
                            == Some(target_attached_data))
            })
            .count()
            == 1;
        if parameter_base_is_bound && unique_parameter_target {
            continue;
        }

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
                let Some((call_key, statement_index, call_ordinal, _)) = input
                    .runtime_flow
                    .context_call_sites
                    .get(state.context.0 as usize)
                else {
                    continue;
                };
                if *call_key != state_call.source_key
                    || *statement_index != state_call.statement_index
                    || *call_ordinal != state_call.call_ordinal
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

        // SPLICED (unreachable) calls serve exactly when the runtime chain
        // walk can recover their receiver (receiver_base.rs): the source
        // machine's base COMPOSED through a unique hop chain, AND this is
        // the UNIQUE live call from its (machine, state) into the target's
        // data family (the walk's final hop), AND the receiver path
        // resolves. Anything else falls through to the by-type compare,
        // which accepts exactly the offsets the fallback resolves correctly
        // (the AMBIGUOUS second receiver blocks; the first-instance call
        // passes the equal-offsets compare and the fallback IS correct).
        if !state_call.reachable {
            let source_anchor = machine_bases
                .iter()
                .find(|(machine, _)| *machine == state_call.source_key.machine)
                .map(|(_, anchor)| anchor);
            let composed = source_anchor.is_some_and(|anchor| anchor.base.is_some());
            let unique_in_family = input
                .state_calls
                .calls
                .iter()
                .filter(|(_, other)| {
                    (other.reachable || live_machines.contains(&other.source_key.machine))
                        && other.source_key.machine == state_call.source_key.machine
                        && other.source_key.state == state_call.source_key.state
                        && (other.target_key.machine == state_call.target_key.machine
                            || machine_layout_by_symbol(input.layouts, other.target_key.machine)
                                .and_then(|layout| layout.attached_data.as_deref())
                                == Some(target_attached_data))
                })
                .count()
                == 1;
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
            if composed && unique_in_family && resolved.is_some() {
                continue;
            }
            // A PARAM/LOCAL receiver inside spliced code, three tiers:
            // (1) SERVED (2026-07-11i): the composed anchor's param
            //     environment binds the receiver -- the runtime walk follows
            //     the same binding to the passed instance's storage, so the
            //     call delivers per-instance and needs no fence.
            // (2) EXACT by luck of uniqueness: at most ONE family instance
            //     exists, so the by-type walk's pick IS the passed instance.
            // (3) BLOCKED: multiple instances, no binding -- the by-type
            //     walk would read the FIRST regardless of the argument
            //     (silent-wrong 7-for-9, probed native 71 vs interp 70).
            if state_call.receiver_name.as_str() != "self"
                && input
                    .layouts
                    .fields
                    .span(source_layout.fields)
                    .is_none_or(|fields| {
                        !fields
                            .iter()
                            .any(|field| field.name == state_call.receiver_name)
                    })
            {
                let bound_by_env = source_anchor.is_some_and(|anchor| {
                    anchor.params.iter().any(|(name, bound)| {
                        name.as_str() == state_call.receiver_name.as_str() && bound.is_some()
                    })
                });
                if bound_by_env && unique_in_family {
                    continue; // tier 1: the binding serves it
                }
                let instances = machine_layout_by_symbol(input.layouts, input.entry_key.machine)
                    .map(|entry_layout| {
                        count_family_instances(
                            input.layouts,
                            entry_layout,
                            state_call.target_key.machine,
                            target_attached_data,
                            &mut Vec::new(),
                        )
                    })
                    .unwrap_or(0);
                if instances > 1 {
                    blockers.insert(blocker(
                        "state calls",
                        &format!(
                            "method call receiver `{}` is a parameter or local inside \
                             spliced code, and {} `{}` instances are reachable -- the \
                             by-type walk would read the FIRST instance regardless of \
                             which one was passed. Pass through a contained field, or \
                             give each instance a distinct data type.",
                            state_call.receiver_name, instances, target_attached_data,
                        ),
                    ));
                }
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

/// How many distinct storage instances of the target's data family the
/// entry's layout tree holds -- the by-type walk's search space. Mirrors
/// `first_type_match_offset`'s descent but COUNTS matches instead of taking
/// the first: at 0 or 1, the by-type pick is provably exact; at 2+, a
/// param-receiver call cannot prove which instance it was handed.
fn count_family_instances(
    layouts: &LayoutPlan,
    machine_layout: &MachineLayout,
    target_machine: SymbolHandle,
    target_attached_data: &str,
    visited: &mut Vec<SymbolHandle>,
) -> usize {
    if visited.contains(&machine_layout.symbol) {
        return 0;
    }
    visited.push(machine_layout.symbol);
    let count = count_family_instances_in_span(
        layouts,
        machine_layout.fields,
        target_machine,
        target_attached_data,
        visited,
    );
    visited.pop();
    count
}

fn count_family_instances_in_span(
    layouts: &LayoutPlan,
    fields_span: HandleSpan<FieldLayout>,
    target_machine: SymbolHandle,
    target_attached_data: &str,
    visited: &mut Vec<SymbolHandle>,
) -> usize {
    let Some(fields) = layouts.fields.span(fields_span) else {
        return 0;
    };
    let mut count = 0;
    for field in fields {
        if field.type_name.as_ref() == target_attached_data {
            count += 1;
            continue;
        }
        if let Some(nested) = field_machine_layout(layouts, field) {
            if nested.symbol == target_machine {
                count += 1;
                continue;
            }
            count += count_family_instances(
                layouts,
                nested,
                target_machine,
                target_attached_data,
                visited,
            );
            continue;
        }
        if let Some(data_fields) = field_data_layout_fields(layouts, field) {
            count += count_family_instances_in_span(
                layouts,
                data_fields,
                target_machine,
                target_attached_data,
                visited,
            );
        }
    }
    count
}

/// Machines whose code EXECUTES via splices even though their own states are
/// never scheduled: the entry seeds the set; any live machine's calls make
/// their targets live. Bounded fixpoint (the call graph is acyclic by
/// language rule; the bound is a backstop).
fn spliced_live_machines(input: &EmissionPlanningInput<'_>) -> Vec<SymbolHandle> {
    let mut live = vec![input.entry_key.machine];
    for _ in 0..64 {
        let mut changed = false;
        for (_, call) in input.state_calls.calls.iter() {
            if !(call.reachable || live.contains(&call.source_key.machine)) {
                continue;
            }
            if !live.contains(&call.target_key.machine) {
                live.push(call.target_key.machine);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    live
}

/// A machine's composed anchor: its storage BASE plus the PARAM ENVIRONMENT
/// its machine-typed `&mut` parameters were bound with (param name ->
/// absolute base). `None` anywhere = poisoned (reached two ways with
/// different answers, or through an unresolvable hop).
#[derive(Default)]
struct MachineAnchor {
    base: Option<usize>,
    params: Vec<(psi_checked_trees::name::Identifier, Option<usize>)>,
}

/// Per-machine COMPOSED storage base + param environment, mirroring the
/// runtime chain walk's hop math (receiver_base.rs -- KEEP IN LOCKSTEP):
/// entry = 0; a named receiver adds its offset in the SOURCE machine's
/// layout; a self call on the same attached data keeps the source base; a
/// single-segment PARAM receiver answers from the source's environment
/// (the param-binding serve, 2026-07-11i). `&mut` (MutableAlias) arguments
/// bind the target's params to the argument path's absolute base. A machine
/// (or param) reached with two DISTINCT answers is POISONED -- the fence is
/// machine-granular where the runtime walk is position-granular, so this
/// mirror is strictly more conservative (refuses more, never less).
fn composed_machine_bases(
    input: &EmissionPlanningInput<'_>,
    live_machines: &[SymbolHandle],
) -> Vec<(SymbolHandle, MachineAnchor)> {
    let mut anchors: Vec<(SymbolHandle, MachineAnchor)> = vec![(
        input.entry_key.machine,
        MachineAnchor {
            base: Some(0),
            params: Vec::new(),
        },
    )];
    let position_of = |anchors: &[(SymbolHandle, MachineAnchor)], machine: SymbolHandle| {
        anchors
            .iter()
            .position(|(candidate, _)| *candidate == machine)
    };
    for _ in 0..64 {
        let mut changed = false;
        for (_, call) in input.state_calls.calls.iter() {
            if !(call.reachable || live_machines.contains(&call.source_key.machine)) {
                continue;
            }
            let target = call.target_key.machine;
            if target == input.entry_key.machine {
                continue; // the entry's anchor is pinned
            }
            let Some(source_position) = position_of(&anchors, call.source_key.machine) else {
                continue; // source not reached yet this pass
            };
            let (source_base, hop_base, bindings) = {
                let source = &anchors[source_position].1;
                let hop_base = hop_receiver_base(input, call, source.base, &source.params);
                (source.base, hop_base, param_bindings(input, call, source))
            };
            let _ = source_base;
            match position_of(&anchors, target) {
                None => {
                    anchors.push((
                        target,
                        MachineAnchor {
                            base: hop_base,
                            params: bindings,
                        },
                    ));
                    changed = true;
                }
                Some(target_position) => {
                    let target_anchor = &mut anchors[target_position].1;
                    if target_anchor.base != hop_base && target_anchor.base.is_some() {
                        target_anchor.base = None;
                        changed = true;
                    }
                    for (name, bound) in bindings {
                        match target_anchor
                            .params
                            .iter_mut()
                            .find(|(existing, _)| *existing == name)
                        {
                            None => {
                                target_anchor.params.push((name, bound));
                                changed = true;
                            }
                            Some((_, existing)) => {
                                if *existing != bound && existing.is_some() {
                                    *existing = None;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    anchors
}

/// The `&mut` (MutableAlias) argument bindings a call hands its target:
/// param name -> the argument path's ABSOLUTE base (a field of the source
/// machine at source base + offset, or a bare name forwarded from the
/// source's own environment). Unresolvable arguments bind `None` (poison)
/// so a conflicting later binding cannot resurrect them.
fn param_bindings(
    input: &EmissionPlanningInput<'_>,
    call: &omega_state_calls::StateCall,
    source: &MachineAnchor,
) -> Vec<(psi_checked_trees::name::Identifier, Option<usize>)> {
    let mut bindings = Vec::new();
    let Some(arguments) = input.state_calls.arguments.span(call.arguments) else {
        return bindings;
    };
    let source_layout = machine_layout_by_symbol(input.layouts, call.source_key.machine);
    for argument in arguments {
        if argument.kind != omega_state_calls::StateCallArgumentKind::MutableAlias {
            continue;
        }
        let mut segments: Vec<&psi_checked_trees::name::Identifier> = Vec::new();
        if !collect_expression_path_segments(
            &input.state_calls.expressions,
            argument.expression,
            &mut segments,
        ) {
            bindings.push((argument.parameter_name.clone(), None));
            continue;
        }
        let field_segments = match segments.first() {
            Some(root) if root.as_str() == "self" => &segments[1..],
            _ => &segments[..],
        };
        let bound = match field_segments {
            [single] => source
                .params
                .iter()
                .find(|(name, _)| name == *single)
                .map(|(_, bound)| *bound)
                .unwrap_or_else(|| {
                    source.base.and_then(|base| {
                        source_layout.and_then(|layout| {
                            omega_layout::field_path_offset(
                                input.layouts,
                                layout.fields,
                                std::slice::from_ref(*single),
                            )
                            .map(|offset| base + offset)
                        })
                    })
                }),
            [] => None,
            path => source.base.and_then(|base| {
                source_layout.and_then(|layout| {
                    let owned: Vec<psi_checked_trees::name::Identifier> =
                        path.iter().map(|segment| (*segment).clone()).collect();
                    omega_layout::field_path_offset(input.layouts, layout.fields, &owned)
                        .map(|offset| base + offset)
                })
            }),
        };
        bindings.push((argument.parameter_name.clone(), bound));
    }
    bindings
}

/// Mirror of the runtime walk's `collect_expression_path_segments`
/// (receiver_base.rs -- KEEP IN LOCKSTEP): a `&mut` argument's spelled name
/// path, or `false` for unresolvable shapes.
fn collect_expression_path_segments<'table>(
    table: &'table psi_checked_trees::expression::ExpressionTable,
    expression: psi_checked_trees::expression::ExpressionHandle,
    segments: &mut Vec<&'table psi_checked_trees::name::Identifier>,
) -> bool {
    use psi_checked_trees::expression::ExpressionNode;
    match table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            collect_expression_path_segments(table, inner.target, segments)
        }
        ExpressionNode::Name(path) => {
            segments.extend(table.name_path_members(path.members).iter());
            true
        }
        ExpressionNode::Member(member) => {
            if !collect_expression_path_segments(table, member.receiver, segments) {
                return false;
            }
            segments.push(&member.member);
            true
        }
        _ => false,
    }
}

/// The ABSOLUTE base a call's receiver refers to (the fence-side mirror of
/// the walk's hop math -- KEEP IN LOCKSTEP with receiver_base.rs): the
/// source base for a same-data self call, the environment's binding for a
/// single-segment param receiver, source base + offset for a field path,
/// `None` otherwise.
fn hop_receiver_base(
    input: &EmissionPlanningInput<'_>,
    call: &omega_state_calls::StateCall,
    base: Option<usize>,
    params: &[(psi_checked_trees::name::Identifier, Option<usize>)],
) -> Option<usize> {
    let receiver_name = call.receiver_name.as_str();
    let source_layout = machine_layout_by_symbol(input.layouts, call.source_key.machine)?;
    if receiver_name == "self" {
        let source_attached = source_layout.attached_data.as_deref();
        let target_attached = machine_layout_by_symbol(input.layouts, call.target_key.machine)
            .and_then(|layout| layout.attached_data.as_deref());
        return (source_attached.is_some() && source_attached == target_attached)
            .then_some(base)
            .flatten();
    }
    if receiver_name.is_empty() {
        return None;
    }
    let segments = input
        .state_calls
        .receiver_path_segments
        .span(call.receiver_path)
        .unwrap_or(&[]);
    let field_segments = match segments.first() {
        Some(root) if root.as_str() == "self" => &segments[1..],
        _ => segments,
    };
    if field_segments.len() <= 1
        && let Some((_, bound)) = params
            .iter()
            .find(|(name, _)| name.as_str() == receiver_name)
    {
        return *bound;
    }
    let base = base?;
    if field_segments.is_empty() {
        return omega_layout::field_path_offset(
            input.layouts,
            source_layout.fields,
            std::slice::from_ref(&call.receiver_name),
        )
        .map(|offset| base + offset);
    }
    omega_layout::field_path_offset(input.layouts, source_layout.fields, field_segments)
        .map(|offset| base + offset)
}

/// Walk the receiver's field path through the layout plan, accumulating each
/// segment's offset; descend into intermediate segments' machine layouts.
/// `None` when any segment is not a field of the current layout (or an
/// intermediate has no machine layout to descend into) -- callers treat
/// unprovable as blocked.
fn receiver_path_offset(
    layouts: &LayoutPlan,
    source_layout: &MachineLayout,
    field_segments: &[psi_checked_trees::name::Identifier],
) -> Option<usize> {
    // The SHARED walk (omega_layout::field_path_offset): per-instance
    // dispatch resolution and this fence agree by construction.
    omega_layout::field_path_offset(layouts, source_layout.fields, field_segments)
}

fn spelled_path(segments: &[psi_checked_trees::name::Identifier]) -> String {
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
