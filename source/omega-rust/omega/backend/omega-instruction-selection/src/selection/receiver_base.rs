//! PER-INSTANCE receiver dispatch, the resolution half (TASKS_FS "Stolen
//! work #2"): a dispatched callee clone's machine-storage BASE, recovered
//! from the clone context's MINTING CALL SITE instead of the
//! first-type-match walk. `self.sum.checked_subtract(..)` with several
//! `Duration` fields must resolve `sum`'s storage (offset 56), not the
//! first `Duration` (offset 0).
//!
//! Chain: dispatch_index == the runtime-flow state's arena index (the
//! state-dispatch context assigns exactly that) -> RuntimeState { key,
//! context } -> RuntimeFlowPlan::context_call_sites[context] -> the minting
//! StateCall -> receiver_path -> omega_layout::field_path_offset (the SAME
//! walk the contained-receiver fence predicts with, by construction).
//!
//! SLICE 1 SCOPE: overrides only when the CALLER is the entry machine (a
//! nested caller's own base would need recursive context resolution; the
//! fence keeps guarding those chains) and only for a named non-`self`
//! receiver. `None` = no override; the by-type walk stays authoritative.
//! LIVE for dispatch-routed calls (the fence relaxes exactly there);
//! inline-routed calls stay fenced until the inline half lands.

use crate::InstructionSelectionInput;

pub(in crate::selection) fn dispatch_receiver_base(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
) -> Option<usize> {
    // The pipeline computes the table ONCE (compute_receiver_bases in the
    // backend-pipeline builder); empty when the gate is off.
    input
        .receiver_bases
        .get(dispatch_index as usize)
        .copied()
        .flatten()
}

/// The receiver base for resolving a place whose expression belongs to
/// `source_machine`, under the dispatch case `dispatch_index`.
///
/// Two routes, one seam:
/// - the state IS the case (a dispatch clone or the caller's own state):
///   the precomputed per-dispatch table answers (per-instance DISPATCH).
/// - the state is a SPLICED CALLEE under a caller case (inline branching:
///   the prelude's `self.X` reads carry the callee machine while the case
///   is the caller's) -- recover the receiver by WALKING the call chain
///   from the case's state (multi-level splices stamp a callee's reads
///   under the TOP case), composing hop offsets onto the case's own
///   table base. The UNIQUE composed base reaching the callee's data
///   wins; distinct candidates return None: the by-type walk stays, and
///   the fence keeps refusing that shape. Probed 2026-07-10t (the
///   prelude read a.seconds@0 instead of sum.seconds@56) and 2026-07-11d
///   (a two-hop splice under a non-entry caller read first@0 instead of
///   second@4).
pub(in crate::selection) fn receiver_base_for(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_machine: psi_symbols::SymbolHandle,
) -> Option<usize> {
    let state = input
        .runtime_flow
        .states
        .iter()
        .find(|(handle, _)| handle.arena_index() == dispatch_index)
        .map(|(_, state)| state)?;
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        eprintln!(
            "RB: dispatch {} case m{} s{} vs source m{}",
            dispatch_index,
            state.key.machine.arena_index(),
            state.key.state.arena_index(),
            source_machine.arena_index(),
        );
    }
    if state.key.machine == source_machine {
        return dispatch_receiver_base(input, dispatch_index);
    }
    // Spliced callee: the unique inline call from the case's state into
    // `source_machine`. The recovered receiver offset is rooted in the
    // CASE machine's layout, so it composes on the case's own base: the
    // per-dispatch table entry (slice 2 -- a non-entry caller's case
    // carries its composed base), or 0 for the entry machine (whose
    // ROOT-context states are the identity base by definition). A case
    // with no composed base cannot anchor a recovery -- refuse, the
    // by-type fallback and the fence keep their old behavior.
    let contextual_anchor = context_anchor_and_env(input, state.context, 64);
    let Some(case_base) = dispatch_receiver_base(input, dispatch_index)
        .or_else(|| (state.key.machine == input.entry_key.machine).then_some(0))
        .or_else(|| contextual_anchor.as_ref().map(|(base, _)| *base))
    else {
        return None;
    };
    // Match callee machines by ATTACHED-DATA equivalence, not machine-symbol
    // equality: the resolution sweep may land in ANY machine layout attached
    // to the same data (`SystemTime::from_unix_seconds` vs the called
    // `SystemTime::duration_since`), and the receiver identity is a property
    // of the DATA instance, not the particular machine (2026-07-10y -- the
    // reversed-operand residual's final hop).
    //
    // MULTI-LEVEL SPLICE (slice 2): the case's statements may carry callees
    // spliced through SEVERAL inline hops (`Main` -> `holder.run()` ->
    // `second.get()` stamps get's reads under MAIN's case). Walk the call
    // chain from the case's state, composing each hop's receiver offset
    // (named receiver: its offset in the CURRENT machine's layout; self
    // call on the same data: +0); the UNIQUE composed base reaching
    // `source_machine`'s data wins. Two candidates with DIFFERENT bases are
    // ambiguous -> None (same base twice is the same instance -- fine).
    // The call graph is acyclic by language rule (no recursion); the fuel
    // is a backstop, not a semantic bound.
    let source_attached = attached_data_of(input, source_machine);
    let case_env = contextual_anchor
        .map(|(_, environment)| environment)
        .unwrap_or_default();
    let mut frontier: Vec<(omega_control_flow::StateKey, usize, ParamEnv)> = vec![(
        omega_control_flow::StateKey {
            machine: state.key.machine,
            state: state.key.state,
            segment_index: 0,
        },
        case_base,
        case_env,
    )];
    let mut recovered: Option<usize> = None;
    let mut fuel = 64usize;
    while let Some((position, base, env)) = frontier.pop() {
        if fuel == 0 {
            return None;
        }
        fuel -= 1;
        for (_, call) in input.state_calls.calls.iter() {
            // NO reachability filter: a call spliced through inline hops keeps
            // its ORIGINAL record marked unreachable (the original state is
            // never scheduled -- its statements run inside the case), yet the
            // spliced copy executes for real; the walk anchors at a reachable
            // case, so every chain it follows corresponds to spliced code.
            if call.source_key.machine != position.machine
                || call.source_key.state != position.state
            {
                continue;
            }
            let hop_base = call_receiver_base(input, position.machine, call, base, &env);
            let target_matches = call.target_key.machine == source_machine
                || (source_attached.is_some()
                    && attached_data_of(input, call.target_key.machine) == source_attached);
            if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                eprintln!(
                    "RB:   hop m{} s{} -> m{} s{} recv {} base {hop_base:?} matches {target_matches}",
                    position.machine.arena_index(),
                    position.state.arena_index(),
                    call.target_key.machine.arena_index(),
                    call.target_key.state.arena_index(),
                    call.receiver_name.as_str(),
                );
            }
            if target_matches {
                let Some(candidate) = hop_base else {
                    continue; // unrecoverable receiver on the final hop
                };
                if recovered.is_some_and(|existing| existing != candidate) {
                    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                        eprintln!("RB: -> AMBIGUOUS (dispatch {dispatch_index})");
                    }
                    return None; // two distinct instances answer -- ambiguous
                }
                recovered = Some(candidate);
                continue;
            }
            if let Some(target_base) = hop_base {
                let callee_env = descend_param_env(input, position.machine, call, base, &env);
                frontier.push((call.target_key, target_base, callee_env));
            }
        }
    }
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        eprintln!(
            "RB: -> inline base {:?} (dispatch {}, case base {case_base})",
            recovered, dispatch_index,
        );
    }
    recovered
}

/// A walk position's PARAM ENVIRONMENT: the source machine's machine-typed
/// `&mut` parameters, each bound to the ABSOLUTE storage base its argument
/// named at the (unique) call that brought the walk here. Small and cloned
/// per descent -- chains are shallow and the walk is compile-time.
type ParamEnv = Vec<(psi_checked_trees::name::Identifier, usize)>;

fn context_anchor_and_env(
    input: &InstructionSelectionInput<'_>,
    context: omega_state_graph::CallContext,
    fuel: usize,
) -> Option<(usize, ParamEnv)> {
    if context.0 == 0 {
        return Some((0, ParamEnv::new()));
    }
    if fuel == 0 {
        return None;
    }
    let (call_key, statement_index, call_ordinal, parent) = input
        .runtime_flow
        .context_call_sites
        .get(context.0 as usize)
        .copied()?;
    let (parent_base, parent_env) = context_anchor_and_env(input, parent, fuel - 1)?;
    let call = input
        .state_calls
        .calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| {
            call.source_key == call_key
                && call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
        })?;
    let environment = descend_param_env(input, call_key.machine, call, parent_base, &parent_env);
    let base = if call.receiver_name.as_str().is_empty() {
        attached_data_of(input, call.target_key.machine)
            .is_none()
            .then_some(parent_base)
    } else {
        call_receiver_base(input, call_key.machine, call, parent_base, &parent_env)
    }?;
    Some((base, environment))
}

/// The ABSOLUTE storage base a call's receiver refers to, given the source
/// position's own `base` and param environment: `base` itself for a
/// machine-to-machine self call on the SAME attached data, the environment's
/// binding for a single-segment PARAM receiver (`t.get()` where `t: &mut
/// Tally` was bound at the call site -- the param-binding serve,
/// 2026-07-11i), `base` + the path's offset for a named field receiver.
/// `None` for anything unrecoverable (static/receiverless spellings,
/// foreign-data self calls, unresolved paths, unbound params).
fn call_receiver_base(
    input: &InstructionSelectionInput<'_>,
    source_machine: psi_symbols::SymbolHandle,
    call: &omega_state_calls::StateCall,
    base: usize,
    env: &ParamEnv,
) -> Option<usize> {
    let receiver_name = call.receiver_name.as_str();
    if receiver_name == "self" {
        let same_data = attached_data_of(input, call.target_key.machine).is_some()
            && attached_data_of(input, call.target_key.machine)
                == attached_data_of(input, source_machine);
        return same_data.then_some(base);
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
    // Single-segment param receiver: the environment answers absolutely.
    // (Param-ROOTED nested paths stay unrecoverable this round.)
    if field_segments.len() <= 1
        && let Some((_, bound)) = env.iter().find(|(name, _)| name.as_str() == receiver_name)
    {
        return Some(*bound);
    }
    let caller_layout = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    if field_segments.is_empty() {
        return omega_layout::field_path_offset(
            input.layouts,
            caller_layout.fields,
            std::slice::from_ref(&call.receiver_name),
        )
        .map(|offset| base + offset);
    }
    omega_layout::field_path_offset(input.layouts, caller_layout.fields, field_segments)
        .map(|offset| base + offset)
}

/// The param environment a call's TARGET position starts with: each of the
/// call's `&mut` (MutableAlias) arguments whose expression names a
/// resolvable receiver -- a field path of the SOURCE machine (bound to
/// `base` + its offset) or a single bare name already bound in the source's
/// own environment (param FORWARDING) -- binds the callee's parameter to
/// that ABSOLUTE base. Unresolvable arguments simply bind nothing: a
/// receiver hop through the unbound param refuses, and the fence keeps
/// blocking that shape.
fn descend_param_env(
    input: &InstructionSelectionInput<'_>,
    source_machine: psi_symbols::SymbolHandle,
    call: &omega_state_calls::StateCall,
    base: usize,
    env: &ParamEnv,
) -> ParamEnv {
    let mut callee_env = ParamEnv::new();
    let Some(arguments) = input.state_calls.arguments.span(call.arguments) else {
        return callee_env;
    };
    let source_layout = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout);
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
            continue;
        }
        let field_segments = match segments.first() {
            Some(root) if root.as_str() == "self" => &segments[1..],
            _ => &segments[..],
        };
        let bound = match field_segments {
            [single] => env
                .iter()
                .find(|(name, _)| name == *single)
                .map(|(_, bound)| *bound)
                .or_else(|| {
                    source_layout.and_then(|layout| {
                        omega_layout::field_path_offset(
                            input.layouts,
                            layout.fields,
                            std::slice::from_ref(*single),
                        )
                        .map(|offset| base + offset)
                    })
                }),
            [] => None,
            path => source_layout.and_then(|layout| {
                let owned: Vec<psi_checked_trees::name::Identifier> =
                    path.iter().map(|segment| (*segment).clone()).collect();
                omega_layout::field_path_offset(input.layouts, layout.fields, &owned)
                    .map(|offset| base + offset)
            }),
        };
        if let Some(bound) = bound {
            callee_env.push((argument.parameter_name.clone(), bound));
        }
    }
    callee_env
}

/// Collect a `&mut`-argument expression's spelled name path (root -> leaf)
/// into `segments`: unwraps `Mutable`, walks `Member` chains, reads `Name`
/// paths. Returns false for anything else (calls, literals, indexing --
/// unresolvable as a receiver identity this round).
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

fn attached_data_of<'plan>(
    input: &'plan InstructionSelectionInput<'_>,
    machine: psi_symbols::SymbolHandle,
) -> Option<&'plan str> {
    input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == machine)
        .and_then(|(_, layout)| layout.attached_data.as_deref())
}
