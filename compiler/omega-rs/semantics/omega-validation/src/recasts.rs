//! §5b recast judgment, rung A (programmable-layouts brief): `&x as &T`
//! re-views a place's bytes under a second stated shape. Legality is a
//! STATIC judgment -- a bad relation is a compile error, never unsafety.
//!
//! Rung A serves the scalar core end-to-end and fences the rest loudly:
//!
//! - **Served:** a SHARED recast between fixed-width scalar primitives of
//!   EQUAL byte size (`&i32 as &f32`, `&u32 as &i32`), bound as the direct
//!   initializer of a reference-typed let whose stated type restates the
//!   target. Facts on the SOURCE are fine (a shared re-view only WEAKENS:
//!   the fact-free target is trivially implied). Lowering is address
//!   identity: native reads the place through the stated type's load; the
//!   interpreter bit-reinterprets at the recast (sound because borrow
//!   exclusivity freezes the source while the view lives).
//! - **Fenced (byte-view rung, L4/L5):** `&mut` recasts (writable views
//!   need fact implication in BOTH directions), record/array shapes
//!   (byte-granular tiling over plan-laid layouts), interior recasts into
//!   `[u8; N]` regions (the Cathedral M2 shape), and non-let positions.
//! - **Refused absolutely:** targets that would ESTABLISH a fact the bytes
//!   don't prove (`bool`'s 0/1, text encodings) -- establishing facts is a
//!   MINT's job (fallible, case-returning), never a recast's.
//!
//! The companion rule closes the accidental-pun hole this judgment would
//! otherwise be bypassed by: a reference-typed let whose initializer is a
//! BARE borrow of a differently-typed scalar place (`let v: &f32 =
//! &self.x` over an i64) used to compile unjudged and DIVERGE (native
//! bit-punned, the interpreter delivered the semantic value; pinned by
//! canaries/fail/recast/reference_let_pun_requires_recast). Re-viewing is
//! spelled `as`; the bare mismatch refuses.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_recasts(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    // The blessed positions: direct initializers of reference-typed lets
    // (mirrors the D14 literal gate's shape -- collect the legal roots,
    // then sweep the whole expression table for strays).
    let mut blessed: Vec<ExpressionHandle> = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if !local.type_reference.is_valid() || !local.initial_value.is_valid() {
                    continue;
                }
                let TypeReferenceNode::Reference {
                    referee,
                    is_mutable: let_is_mutable,
                    ..
                } = program.type_reference_table.type_reference(local.type_reference)
                else {
                    continue;
                };
                // The `&mut x as &mut T` spelling parses as Mutable(Cast(..)):
                // the unary `&mut` wraps the postfix chain. Look through it
                // so the blessed root is the CAST node the sweep checks.
                let initializer = strip_mutable(program, local.initial_value);
                match program.expression_table.expression(initializer) {
                    ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                        blessed.push(initializer);
                        judge_scalar_recast(
                            program,
                            machine,
                            state,
                            cast,
                            *referee,
                            *let_is_mutable,
                            diagnostics,
                        );
                    }
                    _ => {
                        report_unspelled_reference_pun(
                            program,
                            machine,
                            state,
                            local.initial_value,
                            *referee,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }

    // Position sweep: a recast anywhere but a blessed root refuses. (The
    // parser builds recast nodes only from the `as &` spelling, and every
    // expression is reachable from some statement, so this catches guard /
    // argument / nested positions uniformly.)
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && cast.form.is_recast()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(
                "a recast binds to a reference-typed let (`let v: &T = &x as &T;`) in this \
                 rung; inline re-views land with the byte-view rung"
                    .to_string(),
            ));
        }
        // STR4 checked plans, slice 3 (decision 19): a NON-policy `in <Name>`
        // cast suffix is the semantic-domain QUALIFICATION spelling. It is
        // recognized here but its MINT rung (introduction authority +
        // predicate discharge) has not landed -- the staged fence names the
        // declared domain; an unmatched name gets the honest unknown error
        // the parser used to give (now with the declaration check the parser
        // could not perform).
        if let ExpressionNode::Cast(cast) = node
            && cast.semantic_domain.count() > 0
        {
            let members = program
                .expression_table
                .name_path_members(cast.semantic_domain);
            let name = members
                .first()
                .map(|member| member.as_str().to_owned())
                .unwrap_or_default();
            let declared = program.domain_definitions().iter().find(|domain| {
                domain.name.as_str() == name
                    || domain.name.as_str().ends_with(&format!("::{name}"))
            });
            match declared {
                Some(domain) => {
                    // THE MINT v1 (decision 19): in-program qualification is
                    // the OWNING package qualifying its own domain --
                    // authority is granted (sealed-vs-open bites at package
                    // boundaries, which do not exist in-program). The
                    // PREDICATE obligation must still discharge: every
                    // domain fact folds TRUE at the cast's LITERAL value
                    // (`self := <literal>`); non-literal values await the
                    // flow-integration rung and keep a staged refusal.
                    match literal_mint_discharges(program, domain, cast.value) {
                        MintJudgment::Discharged => {}
                        MintJudgment::FactFalse => {
                            diagnostics.push(Diagnostic::error(format!(
                                "`as ... in {name}` cannot mint: a `{name}` domain \
                                 fact is FALSE at this literal value -- the predicate \
                                 obligation is owed (decision 19's \"predicate \
                                 obligation not discharged\" class)",
                            )));
                        }
                        MintJudgment::NotLiteral => {
                            diagnostics.push(Diagnostic::error(format!(
                                "`as ... in {name}` mints only LITERAL values in this \
                                 rung (the domain facts fold at the literal); a \
                                 runtime value's entry routes through a validating \
                                 call or guard until the flow-integration rung lands",
                            )));
                        }
                    }
                }
                None => {
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown cast domain `{name}`: the arithmetic policies are \
                         `Wrapping`, `Saturating`, and `Trapping`, and no domain \
                         declaration names `{name}`",
                    )));
                }
            }
        }
    }
}

/// The rung-A judgment for one blessed `&x as &T` (§5b rules 1-4 over the
/// scalar fragment).
fn judge_scalar_recast(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    cast: &TableCastExpression,
    let_referee: TypeReferenceHandle,
    let_is_mutable: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!(
        "machine `{}` state `{}`",
        machine.name.as_str(),
        state.name.as_str()
    );

    if cast.form == omega_core::cast_form::CastForm::RecastMutable || let_is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: `&mut` recasts land with the byte-view rung -- a writable view \
             needs fact implication in BOTH directions (anything writable through the \
             target must leave the source valid at release); rung A serves shared \
             re-views (`&x as &T`)"
        )));
        return;
    }

    // Target: a fixed-width scalar primitive, restated by the let.
    let target_name = program
        .expression_table
        .name_path_members(cast.target_type)
        .last()
        .map(|name| name.as_str().to_string())
        .unwrap_or_default();
    // RUNG C2: a RECORD target with ALL-SCALAR fields, sized by the
    // natural-alignment rule (kept in lockstep with omega-layout by the
    // drift canary). The view snapshots size_of(record) bytes from the
    // region; member reads are frame-resident record reads.
    if PrimitiveType::from_name(&target_name).is_none() {
        if let Some(record_size) = scalar_record_size(program, &target_name) {
            let source = strip_mutable(program, cast.value);
            let interior = interior_byte_region_source(program, machine, state, source);
            if let InteriorByteRegion::OffsetUnproven {
                offset_display,
                region_length,
            } = &interior
            {
                push_offset_unproven(diagnostics, &context, offset_display, *region_length);
                return;
            }
            if let InteriorByteRegion::Bounded {
                offset,
                region_length,
            } = interior
            {
                let Some(end) = offset.checked_add(record_size as i64) else {
                    return;
                };
                if end > region_length {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: the recast target `{target_name}` needs {record_size} bytes at offset {offset}, but the region holds {region_length} -- the view would read past the buffer (§5b rule 1 is byte-granular)",
                    )));
                }
                let let_names_target = crate::places::unwrapped_type_reference(program, let_referee)
                    .map(|unwrapped| {
                        matches!(
                            program.type_reference_table.type_reference(unwrapped),
                            TypeReferenceNode::Named { name, .. } if name.as_str() == target_name
                        )
                    })
                    .unwrap_or(false);
                if !let_names_target {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: the let's declared type must restate the recast target `&{target_name}`",
                    )));
                }
                return;
            }
        }
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast target `{target_name}` is not a scalar primitive or an \
             all-scalar record over a byte region; deeper shapes land with the \
             byte-view rung"
        )));
        return;
    }
    let Some(target) = PrimitiveType::from_name(&target_name) else {
        return;
    };
    if matches!(target, PrimitiveType::Bool | PrimitiveType::String) {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast may WEAKEN facts, never establish them -- `{target_name}` \
             carries an invariant raw bytes do not prove (bool's 0/1, text encodings); \
             establishing a fact is a mint's job (fallible, case-returning)"
        )));
        return;
    }
    let let_primitive = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped));
    if let_primitive != Some(target) {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: the let's declared type must restate the recast target `&{target_name}` \
             (the stated shape is the single source of truth for reads through the view)"
        )));
        return;
    }

    // Source: a scalar place of the SAME byte width (§5b rule 1: same total
    // size; scalar alignment follows from size). Facts on the source are
    // fine under a shared view (weakening).
    let source = strip_mutable(program, cast.value);
    // RUNG B: an INTERIOR recast into a `[u8; N]` region at a STATIC offset
    // (`&self.buf[4] as &u32`): the target's footprint must fit the
    // remaining bytes (`k + size(T) <= N`). Byte buffers carry no facts and
    // align to 1; both ISAs' scalar loads tolerate the resulting unaligned
    // addresses on normal memory. The stated-type-restated check above
    // already ran; the same-width rule below does NOT apply (the source
    // region is byte-granular).
    let interior = interior_byte_region_source(program, machine, state, source);
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = &interior
    {
        push_offset_unproven(diagnostics, &context, offset_display, *region_length);
        return;
    }
    if let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior
    {
        let Some(target_size) = target.scalar_byte_size() else {
            return;
        };
        let Some(end) = offset.checked_add(target_size as i64) else {
            return;
        };
        if end > region_length {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: the recast target `{target_name}` needs {target_size} bytes at offset {offset}, but the region holds {region_length} -- the view would read past the buffer (§5b rule 1 is byte-granular)",
            )));
        }
        return;
    }
    let source_primitive = crate::places::declared_place_type(program, machine, Some(state), source)
        .and_then(|type_reference| program.primitive_type_reference(type_reference));
    let Some(source_primitive) = source_primitive else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views a PLACE's bytes -- the source must be a borrowed \
             scalar place (`&x as &{target_name}`); record sources and temporaries land \
             with the byte-view rung"
        )));
        return;
    };
    if matches!(
        source_primitive,
        PrimitiveType::Bool | PrimitiveType::String
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recasting a `{}` source lands with the byte-view rung",
            source_primitive.name()
        )));
        return;
    }
    let (Some(source_size), Some(target_size)) = (
        source_primitive.scalar_byte_size(),
        target.scalar_byte_size(),
    ) else {
        return;
    };
    if source_size != target_size {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views the SAME bytes, so the shapes must agree on \
             size (§5b rule 1) -- source `{}` is {source_size} bytes, target \
             `{target_name}` is {target_size} bytes",
            source_primitive.name()
        )));
    }
}

/// The companion hole-closer: a reference-typed let over a BARE borrow of a
/// scalar place whose type disagrees with the stated referee. Without this,
/// `let v: &f32 = &self.x;` (x: i64) is an accidental, unjudged recast --
/// native bit-puns while the interpreter delivers the semantic value.
fn report_unspelled_reference_pun(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    initializer: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = strip_mutable(program, initializer);
    let Some(source_primitive) =
        crate::places::declared_place_type(program, machine, Some(state), source)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    else {
        return;
    };
    let Some(referee_primitive) = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
    else {
        return;
    };
    if source_primitive != referee_primitive {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: reference initializer type `{}` must match the \
             stated `&{}`; re-viewing a place's bytes under another shape is spelled \
             `&x as &{}` (§5b recast)",
            machine.name.as_str(),
            state.name.as_str(),
            source_primitive.name(),
            referee_primitive.name(),
            referee_primitive.name(),
        )));
    }
}

fn strip_mutable(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => strip_mutable(program, *inner),
        _ => expression,
    }
}

/// The interior byte-region judgment's three-way answer (owner-measured
/// diagnostic split 2026-07-11: a recognized shape whose OFFSET cannot be
/// bounded must say so -- it used to fall through to the form errors
/// ("not a scalar primitive or an all-scalar record" / "source must be a
/// borrowed scalar place"), which misled: EfiMemoryDescriptor IS
/// all-scalar; the real failure was the unproven bound).
enum InteriorByteRegion {
    /// Not `<[u8; N] place>[k]` at all -- fall through to the other source
    /// classes and their form messages.
    NotInteriorShape,
    /// The shape is right, but no route bounds the runtime offset.
    OffsetUnproven {
        offset_display: String,
        region_length: i64,
    },
    /// `k` (or its proven upper bound) and `N`.
    Bounded { offset: i64, region_length: i64 },
}

/// Rungs B/C1's interior source: `<[u8; N] place>[k]`. Shape is recognized
/// FIRST (byte-element fixed array, literal length); the offset bound then
/// comes from a literal, the declared range, the dominating incoming
/// guard, or the boundary-ensures witness.
fn interior_byte_region_source(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    source: ExpressionHandle,
) -> InteriorByteRegion {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let Some(collection_type) = crate::places::declared_place_type(
        program,
        machine,
        Some(state),
        indexed.collection,
    ) else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
        ..
    } = program.type_reference_table.type_reference(collection_type)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let element_is_byte = crate::places::unwrapped_type_reference(program, *element_type)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
        == Some(PrimitiveType::U8);
    if !element_is_byte {
        return InteriorByteRegion::NotInteriorShape;
    }
    let omega_typed_trees::types::FixedArrayLength::Literal(length) = length else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let region_length = *length as i64;

    // RUNG C1: a RUNTIME offset (`&self.buf[k] as &u32`) discharges through
    // the index place's enforced interval -- its declared range (dependent
    // maxima substitute through the field's own range) bounds the offset,
    // so `high(k) + size(T) <= N` is the footprint check. The interval is
    // store-enforced/caller-proved by the R1 machinery, so it is a true
    // bound at every read. Gap #4 routes: the dominating incoming-arm
    // guard, and the R4 boundary-ensures witness.
    let offset = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Integer(literal) => literal.value_i64().filter(|offset| *offset >= 0),
        _ => {
            let declared_high = crate::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.index,
            )
            .and_then(|raw| {
                let interval =
                    crate::arithmetic_domains::range_constraint_interval(program, raw)?;
                let high = interval.high()?;
                (!interval.low().is_some_and(|low| low < 0) && high >= 0).then_some(high)
            });
            declared_high
                .or_else(|| incoming_guard_offset_bound(program, machine, state, indexed.index))
        }
    };
    match offset {
        Some(offset) => InteriorByteRegion::Bounded {
            offset,
            region_length,
        },
        None => InteriorByteRegion::OffsetUnproven {
            offset_display: program.expression_table.display_name(indexed.index),
            region_length,
        },
    }
}

fn push_offset_unproven(
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    offset_display: &str,
    region_length: i64,
) {
    diagnostics.push(Diagnostic::error(format!(
        "{context}: cannot bound the recast offset `{offset_display}` -- the region holds \
         {region_length} bytes, but no declared range, dominating incoming guard, or \
         boundary-ensures witness bounds the offset below the footprint. Bound it: declare \
         a range on the offset param, guard the transition arm (`transition \
         {offset_display} <= K {{ true -> ... }}`), or `ensures`-bound the boundary \
         out-param that feeds it",
    )));
}

/// The natural-alignment size of an ALL-SCALAR-FIELD record (each field at
/// the next multiple of its own size; total padded to the widest field).
/// `None` when the name is no data definition or any field is non-scalar.
/// LOCKSTEP: this mirrors omega-layout's scalar-record rule; the drift
/// canary pins agreement (see the C2 note in TASKS.md).
fn scalar_record_size(program: &TypedTrees, name: &str) -> Option<usize> {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)?;
    let mut offset = 0usize;
    let mut max_align = 1usize;
    for member in program.data_members(data) {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        let size = crate::places::unwrapped_type_reference(program, field.type_reference)
            .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
            .and_then(|primitive| primitive.scalar_byte_size())?;
        offset = offset.div_ceil(size) * size;
        offset += size;
        max_align = max_align.max(size);
    }
    Some(offset.div_ceil(max_align) * max_align)
}

/// The literal upper bound the incoming edges place on `offset` at this
/// state's entry: the PER-EDGE MEET (M2 gap 4a) -- EVERY incoming edge
/// machine-wide must prove a bound, and the entry bound is their MAX (the
/// weakest all satisfy). Per-edge routes, in order:
/// - a CONSTANT argument bounds at its own value;
/// - the edge's GUARDED (true) arm, whose guard conjunct `arg <= K` /
///   `arg < K` names (by display spelling) the very expression passed at
///   the param's position -- guard check and argument capture happen in
///   the same transition step, so the bound holds at entry;
/// - R4 witness (the own_machine shape): a BOUNDARY call EARLIER in the
///   source state whose `ensures <param> <= K` bounds the `&mut` argument
///   place spelled identically to the transition argument, with NO
///   intervening write to that place and NO later call (a later callee
///   holding `&mut self` could rewrite the field) between the witness and
///   the transition.
/// One unprovable edge kills the meet. Symbolic bounds (`offset +
/// desc_size < map_size`) remain -- gap 4b.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundSide {
    Upper,
    Lower,
}

fn incoming_guard_offset_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    offset: ExpressionHandle,
) -> Option<i64> {
    incoming_offset_bound(
        program,
        machine,
        state,
        offset,
        SYMBOLIC_BOUND_DEPTH,
        BoundSide::Upper,
    )
}

fn incoming_offset_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    offset: ExpressionHandle,
    depth: u8,
    side: BoundSide,
) -> Option<i64> {
    use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    // The offset must be a bare PARAM of this state; the guard bounds the
    // ARGUMENT at the call site, which becomes the param at entry.
    let ExpressionNode::Name(path) = program.expression_table.expression(offset) else {
        return None;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    // Position among NON-SELF parameters: call-site argument lists exclude
    // the receiver.
    let param_position = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == param_name.as_str())?;

    let mut meet: Option<i64> = None;
    let mut incoming_edges = 0usize;
    for source in program.machine_states(machine) {
        let source_statements = program.statement_table.statements(source.statement_nodes);
        for (statement_index, statement) in source_statements.iter().enumerate() {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                let TransitionTargetNode::Named { path, arguments, .. } =
                    program.statement_table.transition_target(target_handle)
                else {
                    continue;
                };
                let target_name = program
                    .statement_table
                    .name_path_members(path.members)
                    .last()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                if target_name != state.name.as_str() {
                    continue;
                }
                incoming_edges += 1;
                let argument = program
                    .statement_table
                    .expression_handles(*arguments)
                    .get(param_position)
                    .copied()?;
                // A constant argument bounds at its own value (both sides).
                if let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(argument)
                {
                    let value = literal.value_i64().filter(|value| *value >= 0)?;
                    meet = Some(meet.map_or(value, |existing: i64| match side {
                        BoundSide::Upper => existing.max(value),
                        BoundSide::Lower => existing.min(value),
                    }));
                    continue;
                }
                let argument_label = program.expression_table.display_name(argument);
                // Gap 4b: a SELF-FORWARDING edge (the state passes this very
                // param back to itself unchanged) preserves whatever holds at
                // entry -- it contributes nothing to the meet and must not
                // kill it.
                if source.symbol == state.symbol && argument_label == param_name.as_str() {
                    continue;
                }
                // Only the GUARDED (true) arm establishes the guard's bound;
                // the R4 ensures witness precedes the whole transition, so it
                // holds on EITHER arm (and on an Always edge).
                let guard_bound = match transition.guard {
                    TransitionGuardNode::When(guard)
                        if target_handle == transition.target =>
                    {
                        match side {
                            BoundSide::Upper => guard_upper_bound_for(
                                program, machine, source, guard, &argument_label, depth,
                            ),
                            BoundSide::Lower => guard_lower_bound_for(
                                program, machine, source, guard, &argument_label, depth,
                            ),
                        }
                    }
                    _ => None,
                };
                let edge_bound = guard_bound.or_else(|| {
                    boundary_ensures_argument_bound(
                        program,
                        machine,
                        source,
                        source_statements,
                        statement_index,
                        &argument_label,
                        side,
                    )
                })?;
                meet = Some(meet.map_or(edge_bound, |existing: i64| match side {
                    BoundSide::Upper => existing.max(edge_bound),
                    BoundSide::Lower => existing.min(edge_bound),
                }));
            }
        }
    }
    // No incoming edge at all (the entry state, or dead states) proves
    // nothing.
    if incoming_edges == 0 {
        return None;
    }
    meet
}

/// The R4 witness route: scan the statements BEFORE the transition for the
/// LAST boundary call whose `ensures <param> <= K`/`< K` bounds a `&mut`
/// argument place spelled `argument_label`; refuse if anything after that
/// witness could rewrite the place (an assignment to it, or ANY other call
/// -- callees hold `&mut self`).
fn boundary_ensures_argument_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    statements: &[omega_typed_trees::statement::StatementNode],
    transition_index: usize,
    argument_label: &str,
    side: BoundSide,
) -> Option<i64> {
    use omega_typed_trees::statement::StatementNode;
    let mut witness: Option<i64> = None;
    for statement in &statements[..transition_index] {
        match statement {
            StatementNode::Call(call) => {
                // Any call invalidates an earlier witness; this call may
                // itself mint a new one.
                witness = boundary_call_ensures_bound(
                    program,
                    machine,
                    source,
                    call,
                    argument_label,
                    side,
                );
            }
            StatementNode::Assignment(assignment) => {
                if program.expression_table.display_name(assignment.target) == argument_label {
                    witness = None;
                }
            }
            _ => {}
        }
    }
    witness
}

/// `call`'s `ensures <param> <= K`/`< K` INCLUSIVE bound for the `&mut`
/// argument place spelled `argument_label`, resolved through the receiver
/// field's declared boundary trait. None for non-boundary callees, other
/// spellings, or params without a literal upper bound.
fn boundary_call_ensures_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    call: &omega_typed_trees::statement::TableCall,
    argument_label: &str,
    side: BoundSide,
) -> Option<i64> {
    use omega_typed_trees::domain::ProofFact;
    use omega_typed_trees::signature::SignatureContractKind;
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type = program.data_members(data).iter().find_map(|member| {
        match member {
            omega_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == receiver.as_str() =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        }
    })?;
    let TypeReferenceNode::Named { name: trait_name, .. } =
        program.type_reference_table.type_reference(field_type)
    else {
        return None;
    };
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name.as_str())?;
    let signature = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name == call.target)?;
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Which non-self param position holds our place as a `&mut` argument?
    let position = arguments.iter().position(|argument| {
        matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(inner)
                if program.expression_table.display_name(*inner) == argument_label
        )
    })?;
    let parameter = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .nth(position)?;
    let mut bound: Option<i64> = None;
    for contract in program
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            // Ensures facts are literal-only here (depth 0): a symbolic RHS
            // inside a callee contract names CALLEE scope, not ours.
            let fact_bound = match side {
                BoundSide::Upper => guard_upper_bound_for(
                    program, machine, source, *expression, parameter.name.as_str(), 0,
                ),
                BoundSide::Lower => guard_lower_bound_for(
                    program, machine, source, *expression, parameter.name.as_str(), 0,
                ),
            };
            if let Some(fact_bound) = fact_bound {
                bound = Some(bound.map_or(fact_bound, |existing: i64| match side {
                    BoundSide::Upper => existing.min(fact_bound),
                    BoundSide::Lower => existing.max(fact_bound),
                }));
            }
        }
    }
    bound
}

/// `label <= K` / `label < K` within an `&&` conjunction (through the
/// `== true` desugar), by display spelling.
/// Recursion cap for symbolic bound resolution: the M2 chain needs depth 2
/// (offset bound -> map_size bound); anything deeper stays unproven.
const SYMBOLIC_BOUND_DEPTH: u8 = 2;

fn guard_upper_bound_for(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    label: &str,
    depth: u8,
) -> Option<i64> {
    use omega_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            guard_upper_bound_for(program, machine, source, binary.left, label, depth)
                .or_else(|| {
                    guard_upper_bound_for(program, machine, source, binary.right, label, depth)
                })
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_upper_bound_for(program, machine, source, binary.left, label, depth)
        }
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            // The comparison's inclusive RHS bound: a literal, or (gap 4b)
            // a symbolic NAME whose own inclusive bound resolves through
            // the per-edge meet in the SOURCE state's scope.
            let rhs_inclusive = match program.expression_table.expression(binary.right) {
                ExpressionNode::Integer(literal) => literal.value_i64()?,
                ExpressionNode::Name(_) if depth > 0 => symbolic_param_upper_bound(
                    program,
                    machine,
                    source,
                    binary.right,
                    depth - 1,
                )?,
                _ => return None,
            };
            let bound = if binary.operator == BinaryOperator::Less {
                rhs_inclusive.checked_sub(1)?
            } else {
                rhs_inclusive
            };
            // Direct match: the compared expression IS the labeled one.
            if program.expression_table.display_name(binary.left) == label {
                return Some(bound);
            }
            // Gap 4b composition: `X + Y <OP> RHS` bounds X at RHS_bound -
            // lower(Y) -- sound because Y >= lower(Y) forces X down by at
            // least that much. Both operand orders.
            if depth > 0
                && let ExpressionNode::Binary(addition) =
                    program.expression_table.expression(binary.left)
                && addition.operator == BinaryOperator::Add
            {
                for (x, y) in [
                    (addition.left, addition.right),
                    (addition.right, addition.left),
                ] {
                    if program.expression_table.display_name(x) == label
                        && let Some(y_floor) = symbolic_param_lower_bound(
                            program,
                            machine,
                            source,
                            y,
                            depth - 1,
                        )
                        && y_floor >= 0
                    {
                        return bound.checked_sub(y_floor);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// A NAME's inclusive UPPER bound in `source`'s scope: its declared range,
/// or (as a param) the per-edge meet -- the gap-4b symbolic resolution.
fn symbolic_param_upper_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    name: ExpressionHandle,
    depth: u8,
) -> Option<i64> {
    let declared = crate::places::declared_place_type_raw(program, machine, Some(source), name)
        .and_then(|raw| {
            let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
            interval.high()
        });
    declared.or_else(|| incoming_offset_bound(program, machine, source, name, depth, BoundSide::Upper))
}

/// A NAME's inclusive LOWER bound in `source`'s scope (declared range or
/// the per-edge meet's lower twin) -- the `desc_size >= sizeof` witness leg.
fn symbolic_param_lower_bound(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    name: ExpressionHandle,
    depth: u8,
) -> Option<i64> {
    let declared = crate::places::declared_place_type_raw(program, machine, Some(source), name)
        .and_then(|raw| {
            let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
            interval.low()
        });
    declared.or_else(|| incoming_offset_bound(program, machine, source, name, depth, BoundSide::Lower))
}

/// `label >= K` / `> K` within the same guard walk -- the lower twin.
fn guard_lower_bound_for(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    source: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    label: &str,
    depth: u8,
) -> Option<i64> {
    use omega_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            guard_lower_bound_for(program, machine, source, binary.left, label, depth)
                .or_else(|| {
                    guard_lower_bound_for(program, machine, source, binary.right, label, depth)
                })
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_lower_bound_for(program, machine, source, binary.left, label, depth)
        }
        BinaryOperator::GreaterOrEqual | BinaryOperator::Greater => {
            if program.expression_table.display_name(binary.left) != label {
                return None;
            }
            let ExpressionNode::Integer(literal) =
                program.expression_table.expression(binary.right)
            else {
                return None;
            };
            let k = literal.value_i64()?;
            if binary.operator == BinaryOperator::Greater {
                k.checked_add(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}


/// The mint's tri-state judgment for one qualification cast.
enum MintJudgment {
    Discharged,
    FactFalse,
    NotLiteral,
}

/// Fold every domain fact at the cast's literal value (`self := literal`).
/// Only integer literals and `self <op> literal` / `literal <op> self`
/// comparison facts fold; anything else is conservatively NotLiteral.
/// `introduction`-clause pseudo-facts (non-Binary) are skipped -- they are
/// policy, not predicate.
fn literal_mint_discharges(
    program: &TypedTrees,
    domain: &omega_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> MintJudgment {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(value) else {
        return MintJudgment::NotLiteral;
    };
    let Ok(minted) = literal.text().parse::<i128>() else {
        return MintJudgment::NotLiteral;
    };
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let side_value = |handle: ExpressionHandle| -> Option<i128> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == "self"
                    ) =>
                {
                    Some(minted)
                }
                ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (side_value(binary.left), side_value(binary.right))
        else {
            return MintJudgment::NotLiteral;
        };
        use omega_typed_trees::expression::BinaryOperator;
        let holds = match binary.operator {
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            _ => return MintJudgment::NotLiteral,
        };
        if !holds {
            return MintJudgment::FactFalse;
        }
    }
    MintJudgment::Discharged
}
