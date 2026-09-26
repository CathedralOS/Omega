//! Canonical wire form for a checked mathematical signature.
//!
//! A kernel `Signature` — the ordered `Vec<Declaration>` an authored
//! `let`/`boundary let` cohort elaborates to — is untrusted producer
//! evidence of which declarations a program's mathematical claims resolve
//! against. The wire carries every distinct `Term` reachable from the
//! declarations exactly once, in postorder, so a node only ever references
//! earlier table entries and equal subterms share one entry. Table identity
//! is content, not producer handle identity: two producers encoding the same
//! signature emit the same bytes. Decoding rebuilds a fresh `TermArena`; the
//! kernel then re-decides the signature through `check_signature` — there is
//! no producer success flag in this format to trust.
//!
//! The declaration table comes after the term table: each declaration is its
//! level arity, its statement's table index and an optional body's table
//! index, in signature order. `Term::Constant` nodes name a declaration
//! position and carry their level arguments inline. A constant never encodes
//! what its declaration is — that is the signature's job — so a forged index
//! can only point at a checked declaration or nowhere.
//!
//! This is the signature half of the mathematical-certificate wire
//! (`sections::semantic_module::mathematical_certificate_wire`): the term
//! node tags, sort and level encodings, table order and canonicality rule
//! are identical, so one declaration encodes to the same table bytes in
//! either wire. Depth is bounded on both directions: deeper terms are a
//! producer resource refusal, not a decoder stack hazard.

// Authored ahead of its consumer. `8efa96ec70` made this module crate-internal
// "until the lowering leg wires it": PROOF-CONTRACT-MIGRATION owns connecting
// the checked signature to Terminal evidence, and its producer half already
// carries the same allowance
// (`04_typed-trees-to-checked-trees/src/proof/mathematical_signature.rs`'s
// `evidence`/`authored`). The allowance sits on the module rather than on each
// item because the whole file is one wire format with one pending consumer; it
// comes off when that leg lands, and until then `-D warnings` would otherwise
// reject a format the board asked for.
#![allow(dead_code)]

use std::collections::HashMap;

use proof_admission::{Declaration, Level, Signature, Sort, Term, TermArena, TermHandle};

use crate::sections::proof_bundle::ProofCodecError;
use crate::sections::proof_bundle::wire::{Reader, Writer};

const MAGIC: &[u8; 8] = b"PSISIG\0\0";
const FORMAT_MARKER: u16 = 1;
/// Matches the certificate wire's term depth bound; exceeding it refuses the
/// signature, never decides against the declarations it carries.
const MAX_SIGNATURE_TERM_DEPTH: u32 = 256;
/// Same bound for level expressions nested inside a sort.
const MAX_SIGNATURE_LEVEL_DEPTH: u32 = 256;

/// A decoded signature: the materialized term table plus the declaration
/// list whose handles name its statements and bodies in that arena.
pub struct DecodedMathematicalSignature {
    pub arena: TermArena,
    pub signature: Signature,
}

/// Encode one signature against the arena its declaration handles resolve
/// in. Only nodes reachable from the declarations are emitted, so
/// producer-side scratch (normalization residue, unused lemmas) never enters
/// the signature. A `Term::Dummy` anywhere reachable is a producer defect
/// and rejects.
pub fn encode_mathematical_signature(
    arena: &TermArena,
    signature: &Signature,
) -> Result<Vec<u8>, ProofCodecError> {
    let mut nodes = Writer::default();
    let mut by_handle = HashMap::new();
    let mut by_bytes = HashMap::new();
    let mut count = 0u32;
    // Declaration statements and bodies are the only roots: they join the
    // table in signature order.
    for declaration in signature.declarations() {
        encode_term(
            &mut nodes,
            arena,
            declaration.ty,
            &mut by_handle,
            &mut by_bytes,
            &mut count,
            0,
        )?;
        if let Some(body) = declaration.body {
            encode_term(
                &mut nodes,
                arena,
                body,
                &mut by_handle,
                &mut by_bytes,
                &mut count,
                0,
            )?;
        }
    }

    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u32(count);
    writer.bytes(&nodes.finish());
    writer.len("mathematical signature", signature.len())?;
    for declaration in signature.declarations() {
        writer.u32(declaration.level_arity);
        writer.u32(by_handle[&declaration.ty]);
        match declaration.body {
            Some(body) => {
                writer.u8(1);
                writer.u32(by_handle[&body]);
            }
            None => writer.u8(0),
        }
    }
    Ok(writer.finish())
}

/// Decode one signature, materializing its term table into a fresh arena.
/// Decoding rejects forward child references, out-of-table declaration
/// roots, trailing bytes, and any table whose re-encoding differs from the
/// input — unreachable nodes are not part of the canonical form, so two byte
/// strings never decode to the same signature.
pub fn decode_mathematical_signature(
    bytes: &[u8],
) -> Result<DecodedMathematicalSignature, ProofCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(ProofCodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(ProofCodecError::UnsupportedFormatMarker(format_marker));
    }
    let node_count =
        usize::try_from(reader.count()?).map_err(|_| ProofCodecError::UnexpectedEnd)?;
    if node_count > reader.remaining() {
        return Err(ProofCodecError::UnexpectedEnd);
    }
    let mut arena = TermArena::new();
    let mut handles = Vec::with_capacity(node_count);
    let mut depths = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let (term, depth) = decode_term(&mut reader, &handles, &depths)?;
        if depth > MAX_SIGNATURE_TERM_DEPTH {
            return Err(ProofCodecError::MalformedMathematicalSignature(
                "signature term nesting too deep",
            ));
        }
        depths.push(depth);
        handles.push(arena.insert(term));
    }
    let declaration_count =
        usize::try_from(reader.count()?).map_err(|_| ProofCodecError::UnexpectedEnd)?;
    // A declaration occupies at least nine bytes (arity, statement index,
    // body flag), so a count beyond the remaining input cannot parse.
    if declaration_count > reader.remaining() {
        return Err(ProofCodecError::UnexpectedEnd);
    }
    let mut declarations = Vec::with_capacity(declaration_count);
    for _ in 0..declaration_count {
        let level_arity = reader.u32()?;
        let ty = decode_root(&mut reader, &handles)?;
        let body = if reader.boolean()? {
            Some(decode_root(&mut reader, &handles)?)
        } else {
            None
        };
        declarations.push(Declaration {
            level_arity,
            ty,
            body,
        });
    }
    if reader.remaining() != 0 {
        return Err(ProofCodecError::TrailingBytes(reader.remaining()));
    }
    let signature = Signature::from_declarations(declarations);
    // Canonicality: re-encoding the decoded signature must reproduce the
    // input exactly, so unreachable nodes and alternate tables cannot alias
    // one signature.
    if encode_mathematical_signature(&arena, &signature)? != bytes {
        return Err(ProofCodecError::NonCanonicalEncoding);
    }
    Ok(DecodedMathematicalSignature { arena, signature })
}

/// Encode one term and its transitive children, each exactly once and always
/// before its parents; returns the node's table index.
///
/// Table identity is content, not producer handle identity: a node's key is
/// its own wire bytes (tag plus already-canonical child indices), so two
/// producers encoding the same signature emit the same bytes and two
/// distinct handles holding equal terms share one table entry. `by_handle`
/// only short-circuits revisits of an already-emitted handle.
fn encode_term(
    table: &mut Writer,
    arena: &TermArena,
    handle: TermHandle,
    by_handle: &mut HashMap<TermHandle, u32>,
    by_bytes: &mut HashMap<Vec<u8>, u32>,
    count: &mut u32,
    depth: u32,
) -> Result<u32, ProofCodecError> {
    if let Some(&index) = by_handle.get(&handle) {
        return Ok(index);
    }
    if depth >= MAX_SIGNATURE_TERM_DEPTH {
        return Err(ProofCodecError::MalformedMathematicalSignature(
            "signature term nesting too deep",
        ));
    }
    let mut node = Writer::default();
    match arena.get(handle) {
        Term::Dummy => {
            return Err(ProofCodecError::MalformedMathematicalSignature(
                "dummy term cannot be certified",
            ));
        }
        Term::Variable(index) => {
            node.u8(1);
            node.u32(index);
        }
        Term::Sort(sort) => {
            node.u8(2);
            encode_sort(&mut node, sort)?;
        }
        Term::Pi { domain, codomain } => {
            let domain = encode_term(table, arena, domain, by_handle, by_bytes, count, depth + 1)?;
            let codomain = encode_term(
                table,
                arena,
                codomain,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(3);
            node.u32(domain);
            node.u32(codomain);
        }
        Term::Lambda { domain, body } => {
            let domain = encode_term(table, arena, domain, by_handle, by_bytes, count, depth + 1)?;
            let body = encode_term(table, arena, body, by_handle, by_bytes, count, depth + 1)?;
            node.u8(4);
            node.u32(domain);
            node.u32(body);
        }
        Term::Apply { function, argument } => {
            let function = encode_term(
                table,
                arena,
                function,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let argument = encode_term(
                table,
                arena,
                argument,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(5);
            node.u32(function);
            node.u32(argument);
        }
        Term::Sigma { domain, codomain } => {
            let domain = encode_term(table, arena, domain, by_handle, by_bytes, count, depth + 1)?;
            let codomain = encode_term(
                table,
                arena,
                codomain,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(6);
            node.u32(domain);
            node.u32(codomain);
        }
        Term::Pair { first, second } => {
            let first = encode_term(table, arena, first, by_handle, by_bytes, count, depth + 1)?;
            let second = encode_term(table, arena, second, by_handle, by_bytes, count, depth + 1)?;
            node.u8(7);
            node.u32(first);
            node.u32(second);
        }
        Term::Fst { pair } => {
            let pair = encode_term(table, arena, pair, by_handle, by_bytes, count, depth + 1)?;
            node.u8(8);
            node.u32(pair);
        }
        Term::Snd { pair } => {
            let pair = encode_term(table, arena, pair, by_handle, by_bytes, count, depth + 1)?;
            node.u8(9);
            node.u32(pair);
        }
        Term::Two => {
            node.u8(10);
        }
        Term::TwoZero => {
            node.u8(11);
        }
        Term::TwoOne => {
            node.u8(12);
        }
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            let motive = encode_term(table, arena, motive, by_handle, by_bytes, count, depth + 1)?;
            let zero_branch = encode_term(
                table,
                arena,
                zero_branch,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let one_branch = encode_term(
                table,
                arena,
                one_branch,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let scrutinee = encode_term(
                table,
                arena,
                scrutinee,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(13);
            node.u32(motive);
            node.u32(zero_branch);
            node.u32(one_branch);
            node.u32(scrutinee);
        }
        Term::Id { ty, left, right } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            let left = encode_term(table, arena, left, by_handle, by_bytes, count, depth + 1)?;
            let right = encode_term(table, arena, right, by_handle, by_bytes, count, depth + 1)?;
            node.u8(14);
            node.u32(ty);
            node.u32(left);
            node.u32(right);
        }
        Term::Refl { ty, value } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            let value = encode_term(table, arena, value, by_handle, by_bytes, count, depth + 1)?;
            node.u8(15);
            node.u32(ty);
            node.u32(value);
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            let motive = encode_term(table, arena, motive, by_handle, by_bytes, count, depth + 1)?;
            let base = encode_term(table, arena, base, by_handle, by_bytes, count, depth + 1)?;
            let endpoint = encode_term(
                table,
                arena,
                endpoint,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let proof = encode_term(table, arena, proof, by_handle, by_bytes, count, depth + 1)?;
            node.u8(16);
            node.u32(motive);
            node.u32(base);
            node.u32(endpoint);
            node.u32(proof);
        }
        Term::W { carrier, children } => {
            let carrier =
                encode_term(table, arena, carrier, by_handle, by_bytes, count, depth + 1)?;
            let children = encode_term(
                table,
                arena,
                children,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(17);
            node.u32(carrier);
            node.u32(children);
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            let carrier =
                encode_term(table, arena, carrier, by_handle, by_bytes, count, depth + 1)?;
            let children = encode_term(
                table,
                arena,
                children,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let label = encode_term(table, arena, label, by_handle, by_bytes, count, depth + 1)?;
            let function = encode_term(
                table,
                arena,
                function,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(18);
            node.u32(carrier);
            node.u32(children);
            node.u32(label);
            node.u32(function);
        }
        Term::IndW { motive, step, tree } => {
            let motive = encode_term(table, arena, motive, by_handle, by_bytes, count, depth + 1)?;
            let step = encode_term(table, arena, step, by_handle, by_bytes, count, depth + 1)?;
            let tree = encode_term(table, arena, tree, by_handle, by_bytes, count, depth + 1)?;
            node.u8(19);
            node.u32(motive);
            node.u32(step);
            node.u32(tree);
        }
        Term::Constant {
            declaration,
            levels,
        } => {
            node.u8(20);
            node.u32(declaration);
            node.len("constant level arguments", levels.len())?;
            for level in levels {
                encode_level(&mut node, level, 0)?;
            }
        }
        Term::Empty => {
            node.u8(21);
        }
        Term::EmptyElim { ty, scrutinee } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            let scrutinee = encode_term(
                table,
                arena,
                scrutinee,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(22);
            node.u32(ty);
            node.u32(scrutinee);
        }
        Term::Squash { ty } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            node.u8(23);
            node.u32(ty);
        }
        Term::SquashIntro { ty, value } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            let value = encode_term(table, arena, value, by_handle, by_bytes, count, depth + 1)?;
            node.u8(24);
            node.u32(ty);
            node.u32(value);
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => {
            let proposition = encode_term(
                table,
                arena,
                proposition,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let function = encode_term(
                table,
                arena,
                function,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            let scrutinee = encode_term(
                table,
                arena,
                scrutinee,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(25);
            node.u32(proposition);
            node.u32(function);
            node.u32(scrutinee);
        }
        Term::Box { ty } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            node.u8(26);
            node.u32(ty);
        }
        Term::BoxIntro { ty, value } => {
            let ty = encode_term(table, arena, ty, by_handle, by_bytes, count, depth + 1)?;
            let value = encode_term(table, arena, value, by_handle, by_bytes, count, depth + 1)?;
            node.u8(27);
            node.u32(ty);
            node.u32(value);
        }
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            let motive = encode_term(table, arena, motive, by_handle, by_bytes, count, depth + 1)?;
            let body = encode_term(table, arena, body, by_handle, by_bytes, count, depth + 1)?;
            let scrutinee = encode_term(
                table,
                arena,
                scrutinee,
                by_handle,
                by_bytes,
                count,
                depth + 1,
            )?;
            node.u8(28);
            node.u32(motive);
            node.u32(body);
            node.u32(scrutinee);
        }
    }
    let bytes = node.finish();
    if let Some(&index) = by_bytes.get(&bytes) {
        // The same term under another producer handle shares the earlier
        // table entry; remember the alias so revisits cost one lookup.
        by_handle.insert(handle, index);
        return Ok(index);
    }
    // Children already occupy lower table indices, so the node takes the
    // next position — the postorder the decoder requires.
    let index = *count;
    table.bytes(&bytes);
    by_bytes.insert(bytes, index);
    by_handle.insert(handle, index);
    *count += 1;
    Ok(index)
}

fn encode_sort(writer: &mut Writer, sort: Sort) -> Result<(), ProofCodecError> {
    let tag = match sort {
        Sort::Type(_) => 1,
        Sort::Strict(_) => 2,
    };
    writer.u8(tag);
    encode_level(writer, sort.level(), 0)
}

/// Write one level expression inline: `constant | parameter | successor |
/// maximum`. Levels are small trees with no sharing table — equal levels
/// inside equal sorts already share the sort's table entry — and `depth`
/// bounds nesting exactly like the term bound.
fn encode_level(writer: &mut Writer, level: Level, depth: u32) -> Result<(), ProofCodecError> {
    if depth >= MAX_SIGNATURE_LEVEL_DEPTH {
        return Err(ProofCodecError::MalformedMathematicalSignature(
            "level nesting too deep",
        ));
    }
    match level {
        Level::Constant(value) => {
            writer.u8(1);
            writer.u32(value);
        }
        Level::Parameter(index) => {
            writer.u8(2);
            writer.u32(index);
        }
        Level::Successor(inner) => {
            writer.u8(3);
            encode_level(writer, *inner, depth + 1)?;
        }
        Level::Maximum(left, right) => {
            writer.u8(4);
            encode_level(writer, *left, depth + 1)?;
            encode_level(writer, *right, depth + 1)?;
        }
    }
    Ok(())
}

/// Decode one table node at `handles.len()` position; every child index must
/// name an earlier entry.
fn decode_term(
    reader: &mut Reader<'_>,
    handles: &[TermHandle],
    depths: &[u32],
) -> Result<(Term, u32), ProofCodecError> {
    let child = |reader: &mut Reader<'_>| -> Result<(TermHandle, u32), ProofCodecError> {
        let index = usize::try_from(reader.u32()?).map_err(|_| {
            ProofCodecError::MalformedMathematicalSignature("term child index too large")
        })?;
        if index >= handles.len() {
            return Err(ProofCodecError::MalformedMathematicalSignature(
                "term child does not precede its parent",
            ));
        }
        Ok((handles[index], depths[index]))
    };
    Ok(match reader.u8()? {
        1 => (Term::Variable(reader.u32()?), 1),
        2 => (Term::Sort(decode_sort(reader)?), 1),
        3 => {
            let (domain, domain_depth) = child(reader)?;
            let (codomain, codomain_depth) = child(reader)?;
            (
                Term::Pi { domain, codomain },
                1 + domain_depth.max(codomain_depth),
            )
        }
        4 => {
            let (domain, domain_depth) = child(reader)?;
            let (body, body_depth) = child(reader)?;
            (
                Term::Lambda { domain, body },
                1 + domain_depth.max(body_depth),
            )
        }
        5 => {
            let (function, function_depth) = child(reader)?;
            let (argument, argument_depth) = child(reader)?;
            (
                Term::Apply { function, argument },
                1 + function_depth.max(argument_depth),
            )
        }
        6 => {
            let (domain, domain_depth) = child(reader)?;
            let (codomain, codomain_depth) = child(reader)?;
            (
                Term::Sigma { domain, codomain },
                1 + domain_depth.max(codomain_depth),
            )
        }
        7 => {
            let (first, first_depth) = child(reader)?;
            let (second, second_depth) = child(reader)?;
            (
                Term::Pair { first, second },
                1 + first_depth.max(second_depth),
            )
        }
        8 => {
            let (pair, pair_depth) = child(reader)?;
            (Term::Fst { pair }, 1 + pair_depth)
        }
        9 => {
            let (pair, pair_depth) = child(reader)?;
            (Term::Snd { pair }, 1 + pair_depth)
        }
        10 => (Term::Two, 1),
        11 => (Term::TwoZero, 1),
        12 => (Term::TwoOne, 1),
        13 => {
            let (motive, motive_depth) = child(reader)?;
            let (zero_branch, zero_depth) = child(reader)?;
            let (one_branch, one_depth) = child(reader)?;
            let (scrutinee, scrutinee_depth) = child(reader)?;
            (
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                },
                1 + motive_depth
                    .max(zero_depth)
                    .max(one_depth)
                    .max(scrutinee_depth),
            )
        }
        14 => {
            let (ty, ty_depth) = child(reader)?;
            let (left, left_depth) = child(reader)?;
            let (right, right_depth) = child(reader)?;
            (
                Term::Id { ty, left, right },
                1 + ty_depth.max(left_depth).max(right_depth),
            )
        }
        15 => {
            let (ty, ty_depth) = child(reader)?;
            let (value, value_depth) = child(reader)?;
            (Term::Refl { ty, value }, 1 + ty_depth.max(value_depth))
        }
        16 => {
            let (motive, motive_depth) = child(reader)?;
            let (base, base_depth) = child(reader)?;
            let (endpoint, endpoint_depth) = child(reader)?;
            let (proof, proof_depth) = child(reader)?;
            (
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                },
                1 + motive_depth
                    .max(base_depth)
                    .max(endpoint_depth)
                    .max(proof_depth),
            )
        }
        17 => {
            let (carrier, carrier_depth) = child(reader)?;
            let (children, children_depth) = child(reader)?;
            (
                Term::W { carrier, children },
                1 + carrier_depth.max(children_depth),
            )
        }
        18 => {
            let (carrier, carrier_depth) = child(reader)?;
            let (children, children_depth) = child(reader)?;
            let (label, label_depth) = child(reader)?;
            let (function, function_depth) = child(reader)?;
            (
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                },
                1 + carrier_depth
                    .max(children_depth)
                    .max(label_depth)
                    .max(function_depth),
            )
        }
        19 => {
            let (motive, motive_depth) = child(reader)?;
            let (step, step_depth) = child(reader)?;
            let (tree, tree_depth) = child(reader)?;
            (
                Term::IndW { motive, step, tree },
                1 + motive_depth.max(step_depth).max(tree_depth),
            )
        }
        20 => {
            let declaration = reader.u32()?;
            let level_count = usize::try_from(reader.count()?).map_err(|_| {
                ProofCodecError::MalformedMathematicalSignature("constant level count too large")
            })?;
            // Every level expression takes at least one byte, so a count
            // beyond the remaining input cannot parse.
            if level_count > reader.remaining() {
                return Err(ProofCodecError::UnexpectedEnd);
            }
            let mut levels = Vec::with_capacity(level_count);
            for _ in 0..level_count {
                levels.push(decode_level(reader, 0)?);
            }
            (
                Term::Constant {
                    declaration,
                    levels,
                },
                1,
            )
        }
        21 => (Term::Empty, 1),
        22 => {
            let (ty, ty_depth) = child(reader)?;
            let (scrutinee, scrutinee_depth) = child(reader)?;
            (
                Term::EmptyElim { ty, scrutinee },
                1 + ty_depth.max(scrutinee_depth),
            )
        }
        23 => {
            let (ty, ty_depth) = child(reader)?;
            (Term::Squash { ty }, 1 + ty_depth)
        }
        24 => {
            let (ty, ty_depth) = child(reader)?;
            let (value, value_depth) = child(reader)?;
            (
                Term::SquashIntro { ty, value },
                1 + ty_depth.max(value_depth),
            )
        }
        25 => {
            let (proposition, proposition_depth) = child(reader)?;
            let (function, function_depth) = child(reader)?;
            let (scrutinee, scrutinee_depth) = child(reader)?;
            (
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                },
                1 + proposition_depth.max(function_depth).max(scrutinee_depth),
            )
        }
        26 => {
            let (ty, ty_depth) = child(reader)?;
            (Term::Box { ty }, 1 + ty_depth)
        }
        27 => {
            let (ty, ty_depth) = child(reader)?;
            let (value, value_depth) = child(reader)?;
            (Term::BoxIntro { ty, value }, 1 + ty_depth.max(value_depth))
        }
        28 => {
            let (motive, motive_depth) = child(reader)?;
            let (body, body_depth) = child(reader)?;
            let (scrutinee, scrutinee_depth) = child(reader)?;
            (
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                },
                1 + motive_depth.max(body_depth).max(scrutinee_depth),
            )
        }
        tag => return Err(ProofCodecError::InvalidTag("MathematicalTerm", tag)),
    })
}

fn decode_sort(reader: &mut Reader<'_>) -> Result<Sort, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => Sort::Type(decode_level(reader, 0)?),
        2 => Sort::Strict(decode_level(reader, 0)?),
        tag => return Err(ProofCodecError::InvalidTag("MathematicalSort", tag)),
    })
}

/// Read one level expression. Decoding preserves the exact syntax — a
/// `Maximum(v, u)` stays in that order — so re-encoding reproduces the
/// input bytes and canonicality is byte equality. The kernel decides
/// semantic level equality; the wire only has to carry it faithfully.
fn decode_level(reader: &mut Reader<'_>, depth: u32) -> Result<Level, ProofCodecError> {
    if depth >= MAX_SIGNATURE_LEVEL_DEPTH {
        return Err(ProofCodecError::MalformedMathematicalSignature(
            "level nesting too deep",
        ));
    }
    Ok(match reader.u8()? {
        1 => Level::Constant(reader.u32()?),
        2 => Level::Parameter(reader.u32()?),
        3 => Level::Successor(Box::new(decode_level(reader, depth + 1)?)),
        4 => Level::Maximum(
            Box::new(decode_level(reader, depth + 1)?),
            Box::new(decode_level(reader, depth + 1)?),
        ),
        tag => return Err(ProofCodecError::InvalidTag("MathematicalLevel", tag)),
    })
}

fn decode_root(
    reader: &mut Reader<'_>,
    handles: &[TermHandle],
) -> Result<TermHandle, ProofCodecError> {
    let index = usize::try_from(reader.u32()?)
        .map_err(|_| ProofCodecError::MalformedMathematicalSignature("root index too large"))?;
    handles.get(index).copied().ok_or({
        ProofCodecError::MalformedMathematicalSignature("root index outside the term table")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DecodedMathematicalSignature, FORMAT_MARKER, Level, MAGIC, ProofCodecError, Signature,
        Sort, Term, TermArena, TermHandle, decode_mathematical_signature,
        encode_mathematical_signature,
    };
    use proof_admission::Declaration;

    fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
        arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
    }

    fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
        arena.insert(Term::Variable(index))
    }

    fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
        arena.insert(Term::Pi { domain, codomain })
    }

    fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
        arena.insert(Term::Lambda { domain, body })
    }

    /// `id : Π(A : Type u0). Π(x : A). A` defined as `λA. λx. x` over one
    /// universe parameter, plus a `Two : Type 0` assumption whose statement
    /// shares no subterms — five table nodes for the definition, one for the
    /// assumption.
    fn identity_signature() -> (TermArena, Signature) {
        let mut arena = TermArena::new();
        let type_parameter = type_sort(&mut arena, 0);
        let two = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let inner = lambda(&mut arena, bound, bound);
        let identity = lambda(&mut arena, type_parameter, inner);
        let codomain_domain = variable(&mut arena, 0);
        let codomain_body = variable(&mut arena, 1);
        let codomain = pi(&mut arena, codomain_domain, codomain_body);
        let statement = pi(&mut arena, type_parameter, codomain);
        let sort_parameter = Term::Sort(Sort::Type(Level::Parameter(0)));
        let parameter_statement = arena.insert(sort_parameter);
        let parameter_identity = lambda(&mut arena, parameter_statement, bound);
        let parameter_codomain = pi(&mut arena, parameter_statement, codomain);
        (
            arena,
            Signature::from_declarations(vec![
                Declaration {
                    level_arity: 0,
                    ty: two,
                    body: None,
                },
                Declaration::definition(1, parameter_codomain, parameter_identity),
                Declaration {
                    level_arity: 0,
                    ty: statement,
                    body: Some(identity),
                },
            ]),
        )
    }

    #[test]
    fn a_signature_round_trips_exactly() {
        let (arena, signature) = identity_signature();
        let bytes = encode_mathematical_signature(&arena, &signature).expect("encode");
        let DecodedMathematicalSignature {
            arena: decoded_arena,
            signature: decoded,
        } = decode_mathematical_signature(&bytes).expect("decode");
        assert_eq!(decoded.len(), 3);
        assert_eq!(
            encode_mathematical_signature(&decoded_arena, &decoded).expect("re-encode"),
            bytes
        );
        let definition = &decoded.declarations()[1];
        assert_eq!(definition.level_arity, 1);
        assert!(definition.body.is_some());
        assert!(decoded.declarations()[0].is_assumption());
    }

    #[test]
    fn equal_signatures_encode_to_identical_bytes() {
        let (first_arena, first) = identity_signature();
        let (second_arena, second) = identity_signature();
        assert_eq!(
            encode_mathematical_signature(&first_arena, &first).expect("encode first"),
            encode_mathematical_signature(&second_arena, &second).expect("encode second"),
        );
    }

    #[test]
    fn a_forged_declaration_root_rejects() {
        let (arena, signature) = identity_signature();
        let mut bytes = encode_mathematical_signature(&arena, &signature).expect("encode");
        // The last declaration's statement index sits at bytes.len() - 5:
        // trailing body flag (1 byte) + body index (4 bytes).
        let position = bytes.len() - 5;
        bytes[position] = u8::MAX;
        bytes[position + 1] = u8::MAX;
        bytes[position + 2] = u8::MAX;
        bytes[position + 3] = u8::MAX;
        assert!(decode_mathematical_signature(&bytes).is_err());
    }

    #[test]
    fn trailing_bytes_reject() {
        let (arena, signature) = identity_signature();
        let mut bytes = encode_mathematical_signature(&arena, &signature).expect("encode");
        bytes.push(0);
        assert!(matches!(
            decode_mathematical_signature(&bytes),
            Err(ProofCodecError::TrailingBytes(1))
        ));
    }

    #[test]
    fn a_unreachable_table_node_rejects_as_non_canonical() {
        let (arena, signature) = identity_signature();
        let mut bytes = encode_mathematical_signature(&arena, &signature).expect("encode");
        // Raise the recorded node count and prepend an unreachable
        // `Term::Two` node at the table head: every later child index still
        // parses, but the unreachable node means re-encoding drops it.
        let node_count = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
        bytes[10..14].copy_from_slice(&(node_count + 1).to_le_bytes());
        bytes.splice(14..14, [10]);
        assert!(matches!(
            decode_mathematical_signature(&bytes),
            Err(ProofCodecError::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn wrong_magic_and_marker_reject() {
        let (arena, signature) = identity_signature();
        let bytes = encode_mathematical_signature(&arena, &signature).expect("encode");
        let mut bad_magic = bytes.clone();
        bad_magic[0] = b'X';
        assert!(matches!(
            decode_mathematical_signature(&bad_magic),
            Err(ProofCodecError::InvalidMagic)
        ));
        let mut bad_marker = bytes;
        bad_marker[MAGIC.len()] = (FORMAT_MARKER + 1) as u8;
        assert!(matches!(
            decode_mathematical_signature(&bad_marker),
            Err(ProofCodecError::UnsupportedFormatMarker(m)) if m == FORMAT_MARKER + 1
        ));
    }

    #[test]
    fn a_dummy_term_in_a_declaration_rejects() {
        let mut arena = TermArena::new();
        let ty = type_sort(&mut arena, 0);
        let dummy = arena.insert(Term::Dummy);
        let signature = Signature::from_declarations(vec![Declaration {
            level_arity: 0,
            ty,
            body: Some(dummy),
        }]);
        assert!(matches!(
            encode_mathematical_signature(&arena, &signature),
            Err(ProofCodecError::MalformedMathematicalSignature(_))
        ));
    }
}
