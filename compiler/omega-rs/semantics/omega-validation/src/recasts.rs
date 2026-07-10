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
    let Some(target) = PrimitiveType::from_name(&target_name) else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast target `{target_name}` is not a scalar primitive; \
             record/array re-views (byte-granular tiling over plan-laid layouts) land \
             with the byte-view rung"
        )));
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
    if let Some((offset, region_length)) =
        interior_byte_region_source(program, machine, state, source)
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

/// Rung B's interior source: `<[u8; N] place>[k]` with a LITERAL index --
/// returns `(k, N)`. `None` for runtime indexes (rung C), non-byte
/// elements, and non-indexed shapes.
fn interior_byte_region_source(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<(i64, i64)> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    // RUNG C1: a RUNTIME offset (`&self.buf[k] as &u32`) discharges through
    // the index place's enforced interval -- its declared range (dependent
    // maxima substitute through the field's own range) bounds the offset,
    // so `high(k) + size(T) <= N` is the footprint check. The interval is
    // store-enforced/caller-proved by the R1 machinery, so it is a true
    // bound at every read.
    let offset = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Integer(literal) => {
            let offset = literal.value_i64()?;
            if offset < 0 {
                return None;
            }
            offset
        }
        _ => {
            let raw = crate::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.index,
            )?;
            let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
            let high = interval.high()?;
            if interval.low().is_some_and(|low| low < 0) || high < 0 {
                return None;
            }
            high
        }
    };
    let collection_type = crate::places::declared_place_type(
        program,
        machine,
        Some(state),
        indexed.collection,
    )?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
        ..
    } = program.type_reference_table.type_reference(collection_type)
    else {
        return None;
    };
    let element_is_byte = crate::places::unwrapped_type_reference(program, *element_type)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
        == Some(PrimitiveType::U8);
    if !element_is_byte {
        return None;
    }
    let omega_typed_trees::types::FixedArrayLength::Literal(length) = length else {
        return None;
    };
    Some((offset, *length as i64))
}
