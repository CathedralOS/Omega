//! Canonical wire form for mathematical derivation certificates.
//!
//! A [`MathematicalCertificate`] is untrusted producer evidence for one
//! complete judgment `Γ ⊢ t : T` of the common mathematical kernel
//! (`proof-admission`'s `mathematical_core`). The wire carries every distinct
//! `Term` reachable from the certificate's roots exactly once, in postorder,
//! so a node only ever references earlier table entries and equal subterms
//! share one entry. Table identity is content, not producer handle identity:
//! two producers encoding the same judgment emit the same bytes. Decoding
//! rebuilds a fresh `TermArena`; the kernel then re-decides the judgment —
//! there is no producer success flag in this format to trust.
//!
//! The roots come after the table: the context bindings in order
//! (outermost-first, the order `Context::extend` consumes), then the evidence
//! term, then the claimed type. Decoding rejects forward child references,
//! out-of-table roots, trailing bytes, and any table whose re-encoding
//! differs from the input — unreachable nodes are not part of the canonical
//! form, so two byte strings never decode to the same certificate.
//!
//! Depth is bounded on both directions: deeper terms are a producer resource
//! refusal, not a false judgment and not a decoder stack hazard.
//!
//! The certificate also carries its level arity — the number of universe
//! parameters the judgment is polymorphic over — and sorts carry level
//! expressions (`constant | parameter | successor | maximum`), so a
//! universe-polymorphic judgment survives the wire exactly.
//!
//! Format 3 carries the ambient declaration signature between the term
//! table and the judgment roots: each declaration is its level arity, its
//! statement's table index and an optional body's table index, in signature
//! order. `Term::Constant` nodes name a declaration position and carry
//! their level arguments inline. A constant never encodes what its
//! declaration is — that is the signature's job — so a forged index can
//! only point at a checked declaration or nowhere.

use std::collections::HashMap;

use proof_admission::{
    Declaration, Level, MathematicalCertificate, Sort, Term, TermArena, TermHandle,
};

use super::CodecError;
use super::wire::{Reader, Writer};

const MAGIC: &[u8; 8] = b"PSICORE\0";
/// Format 3 adds the declaration signature and `Term::Constant`; format 2
/// carried level expressions but no declarations, and format 1 held only
/// closed constant levels. Neither must decode a signature as judgment
/// roots.
const FORMAT_MARKER: u16 = 3;
/// Matches the other codec term depth bounds; exceeding it refuses the
/// certificate, never decides against the judgment it carries.
const MAX_MATHEMATICAL_TERM_DEPTH: u32 = 256;
/// Same bound for level expressions nested inside a sort.
const MAX_MATHEMATICAL_LEVEL_DEPTH: u32 = 256;

/// A decoded certificate: the materialized term table plus the root handles
/// that name its judgment in that arena.
pub struct DecodedMathematicalCertificate {
    pub arena: TermArena,
    pub certificate: MathematicalCertificate,
}

/// Encode one certificate against the arena its handles resolve in.
///
/// Only nodes reachable from the context bindings, the term, and the claimed
/// type are emitted, so producer-side scratch (normalization residue, unused
/// lemmas) never enters the certificate. A `Term::Dummy` anywhere reachable
/// is a producer defect and rejects.
pub fn encode_mathematical_certificate(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> Result<Vec<u8>, CodecError> {
    let mut nodes = Writer::default();
    let mut by_handle = HashMap::new();
    let mut by_bytes = HashMap::new();
    let mut count = 0u32;
    // Declaration statements and bodies are roots: the signature's terms
    // join the table before the judgment's, in signature order.
    for declaration in &certificate.signature {
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
    for &binding in &certificate.context {
        encode_term(
            &mut nodes,
            arena,
            binding,
            &mut by_handle,
            &mut by_bytes,
            &mut count,
            0,
        )?;
    }
    encode_term(
        &mut nodes,
        arena,
        certificate.term,
        &mut by_handle,
        &mut by_bytes,
        &mut count,
        0,
    )?;
    encode_term(
        &mut nodes,
        arena,
        certificate.expected,
        &mut by_handle,
        &mut by_bytes,
        &mut count,
        0,
    )?;

    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u32(certificate.level_arity);
    writer.u32(count);
    writer.bytes(&nodes.finish());
    writer.len(
        "mathematical certificate signature",
        certificate.signature.len(),
    )?;
    for declaration in &certificate.signature {
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
    writer.len(
        "mathematical certificate context",
        certificate.context.len(),
    )?;
    for &binding in &certificate.context {
        writer.u32(by_handle[&binding]);
    }
    writer.u32(by_handle[&certificate.term]);
    writer.u32(by_handle[&certificate.expected]);
    Ok(writer.finish())
}

/// Decode one certificate, materializing its term table into a fresh arena.
pub fn decode_mathematical_certificate(
    bytes: &[u8],
) -> Result<DecodedMathematicalCertificate, CodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format_marker));
    }
    let level_arity = reader.u32()?;
    let node_count = usize::try_from(reader.count()?).map_err(|_| CodecError::UnexpectedEnd)?;
    if node_count > reader.remaining() {
        return Err(CodecError::UnexpectedEnd);
    }
    let mut arena = TermArena::new();
    let mut handles = Vec::with_capacity(node_count);
    let mut depths = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let (term, depth) = decode_term(&mut reader, &handles, &depths)?;
        if depth > MAX_MATHEMATICAL_TERM_DEPTH {
            return Err(CodecError::MalformedMathematicalCertificate(
                "mathematical term nesting too deep",
            ));
        }
        depths.push(depth);
        handles.push(arena.insert(term));
    }
    let signature_count =
        usize::try_from(reader.count()?).map_err(|_| CodecError::UnexpectedEnd)?;
    // A declaration occupies at least nine bytes (arity, statement index,
    // body flag), so a count beyond the remaining input cannot parse.
    if signature_count > reader.remaining() {
        return Err(CodecError::UnexpectedEnd);
    }
    let mut signature = Vec::with_capacity(signature_count);
    for _ in 0..signature_count {
        let level_arity = reader.u32()?;
        let ty = decode_root(&mut reader, &handles)?;
        let body = if reader.boolean()? {
            Some(decode_root(&mut reader, &handles)?)
        } else {
            None
        };
        signature.push(Declaration {
            level_arity,
            ty,
            body,
        });
    }
    let context_count = usize::try_from(reader.count()?).map_err(|_| CodecError::UnexpectedEnd)?;
    if context_count > reader.remaining() {
        return Err(CodecError::UnexpectedEnd);
    }
    let mut context = Vec::with_capacity(context_count);
    for _ in 0..context_count {
        context.push(decode_root(&mut reader, &handles)?);
    }
    let term = decode_root(&mut reader, &handles)?;
    let expected = decode_root(&mut reader, &handles)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    let certificate = MathematicalCertificate {
        signature,
        level_arity,
        context,
        term,
        expected,
    };
    // Canonicality: re-encoding the decoded judgment must reproduce the
    // input exactly, so unreachable nodes and alternate tables cannot alias
    // one certificate.
    if encode_mathematical_certificate(&arena, &certificate)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(DecodedMathematicalCertificate { arena, certificate })
}

/// Encode one term and its transitive children, each exactly once and always
/// before its parents; returns the node's table index.
///
/// Table identity is content, not producer handle identity: a node's key is
/// its own wire bytes (tag plus already-canonical child indices), so two
/// producers encoding the same judgment emit the same bytes and two distinct
/// handles holding equal terms share one table entry. `by_handle` only
/// short-circuits revisits of an already-emitted handle.
fn encode_term(
    table: &mut Writer,
    arena: &TermArena,
    handle: TermHandle,
    by_handle: &mut HashMap<TermHandle, u32>,
    by_bytes: &mut HashMap<Vec<u8>, u32>,
    count: &mut u32,
    depth: u32,
) -> Result<u32, CodecError> {
    if let Some(&index) = by_handle.get(&handle) {
        return Ok(index);
    }
    if depth >= MAX_MATHEMATICAL_TERM_DEPTH {
        return Err(CodecError::MalformedMathematicalCertificate(
            "mathematical term nesting too deep",
        ));
    }
    let mut node = Writer::default();
    match arena.get(handle) {
        Term::Dummy => {
            return Err(CodecError::MalformedMathematicalCertificate(
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

fn encode_sort(writer: &mut Writer, sort: Sort) -> Result<(), CodecError> {
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
fn encode_level(writer: &mut Writer, level: Level, depth: u32) -> Result<(), CodecError> {
    if depth >= MAX_MATHEMATICAL_LEVEL_DEPTH {
        return Err(CodecError::MalformedMathematicalCertificate(
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
) -> Result<(Term, u32), CodecError> {
    let child = |reader: &mut Reader<'_>| -> Result<(TermHandle, u32), CodecError> {
        let index = usize::try_from(reader.u32()?).map_err(|_| {
            CodecError::MalformedMathematicalCertificate("term child index too large")
        })?;
        if index >= handles.len() {
            return Err(CodecError::MalformedMathematicalCertificate(
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
                CodecError::MalformedMathematicalCertificate("constant level count too large")
            })?;
            // Every level expression takes at least one byte, so a count
            // beyond the remaining input cannot parse.
            if level_count > reader.remaining() {
                return Err(CodecError::UnexpectedEnd);
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
        tag => return Err(CodecError::InvalidTag("MathematicalTerm", tag)),
    })
}

fn decode_sort(reader: &mut Reader<'_>) -> Result<Sort, CodecError> {
    Ok(match reader.u8()? {
        1 => Sort::Type(decode_level(reader, 0)?),
        2 => Sort::Strict(decode_level(reader, 0)?),
        tag => return Err(CodecError::InvalidTag("MathematicalSort", tag)),
    })
}

/// Read one level expression. Decoding preserves the exact syntax — a
/// `Maximum(v, u)` stays in that order — so re-encoding reproduces the
/// input bytes and canonicality is byte equality. The kernel decides
/// semantic level equality; the wire only has to carry it faithfully.
fn decode_level(reader: &mut Reader<'_>, depth: u32) -> Result<Level, CodecError> {
    if depth >= MAX_MATHEMATICAL_LEVEL_DEPTH {
        return Err(CodecError::MalformedMathematicalCertificate(
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
        tag => return Err(CodecError::InvalidTag("MathematicalLevel", tag)),
    })
}

fn decode_root(reader: &mut Reader<'_>, handles: &[TermHandle]) -> Result<TermHandle, CodecError> {
    let index = usize::try_from(reader.u32()?)
        .map_err(|_| CodecError::MalformedMathematicalCertificate("root index too large"))?;
    handles.get(index).copied().ok_or({
        CodecError::MalformedMathematicalCertificate("root index outside the term table")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CodecError, FORMAT_MARKER, Level, MAGIC, MAX_MATHEMATICAL_LEVEL_DEPTH,
        MathematicalCertificate, Sort, Term, TermArena, TermHandle,
        decode_mathematical_certificate, encode_mathematical_certificate,
    };

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

    /// `λ(A : Type 0). λ(x : A). x` and `Π(A : Type 0). Π(x : A). A`.
    fn polymorphic_identity() -> (TermArena, MathematicalCertificate) {
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let bound = variable(&mut arena, 0);
        let inner = lambda(&mut arena, bound, bound);
        let identity = lambda(&mut arena, type_zero, inner);
        let codomain_domain = variable(&mut arena, 0);
        let codomain_body = variable(&mut arena, 1);
        let codomain = pi(&mut arena, codomain_domain, codomain_body);
        let expected = pi(&mut arena, type_zero, codomain);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: identity,
            expected,
        };
        (arena, certificate)
    }

    #[test]
    fn a_certificate_round_trips_exactly() {
        let (arena, certificate) = polymorphic_identity();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        let decoded = decode_mathematical_certificate(&bytes).expect("decode");
        assert!(decoded.certificate.context.is_empty());
        // Seven distinct nodes: Sort(Type 0), Variable(0), Variable(1), the
        // inner lambda, the outer lambda, the inner Π and the outer Π.
        assert_eq!(decoded.arena.len(), 7);
        assert_eq!(
            encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
                .expect("re-encode"),
            bytes,
        );
    }

    #[test]
    fn shared_subterms_encode_once() {
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        // A second, handle-distinct `Sort(Type 0)` — equal content must share
        // the first's table entry because the wire identifies terms by
        // content, not by producer handle.
        let type_zero_again = type_sort(&mut arena, 0);
        // Π(A : Type 0). Type 0 → Type 0 — a type whose codomain shares the
        // domain's children, and a lambda sharing the same sort content.
        let inner = pi(&mut arena, type_zero, type_zero_again);
        let expected = pi(&mut arena, type_zero, inner);
        let bound = variable(&mut arena, 0);
        let body = lambda(&mut arena, bound, type_zero_again);
        let term = lambda(&mut arena, type_zero, body);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term,
            expected,
        };
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        // Distinct nodes: Sort(Type 0), Variable(0), inner λ, outer λ,
        // inner Π, outer Π — equal subterms appear once even across handles.
        assert_eq!(u32::from_le_bytes(bytes[14..18].try_into().unwrap()), 6);
        decode_mathematical_certificate(&bytes).expect("decode");
    }

    #[test]
    fn a_dummy_term_is_a_producer_defect_not_a_certificate() {
        let mut arena = TermArena::new();
        let type_zero = type_sort(&mut arena, 0);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: TermHandle::default(),
            expected: type_zero,
        };
        assert_eq!(
            encode_mathematical_certificate(&arena, &certificate),
            Err(CodecError::MalformedMathematicalCertificate(
                "dummy term cannot be certified",
            ))
        );
    }

    #[test]
    fn invalid_tags_and_envelope_reject() {
        let (arena, certificate) = polymorphic_identity();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

        let mut bad_magic = bytes.clone();
        bad_magic[0] ^= 0xff;
        assert!(matches!(
            decode_mathematical_certificate(&bad_magic),
            Err(CodecError::InvalidMagic)
        ));

        let mut bad_marker = bytes.clone();
        bad_marker[8] = 0xff;
        assert!(matches!(
            decode_mathematical_certificate(&bad_marker),
            Err(CodecError::UnsupportedFormatMarker(0x00ff))
        ));

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_mathematical_certificate(&trailing),
            Err(CodecError::TrailingBytes(1))
        ));

        // Node tag 0 is not a term.
        let mut bad_tag = vec![];
        bad_tag.extend_from_slice(MAGIC);
        bad_tag.extend_from_slice(&FORMAT_MARKER.to_le_bytes());
        bad_tag.extend_from_slice(&0_u32.to_le_bytes());
        bad_tag.extend_from_slice(&1_u32.to_le_bytes());
        bad_tag.push(0);
        assert!(matches!(
            decode_mathematical_certificate(&bad_tag),
            Err(CodecError::InvalidTag("MathematicalTerm", 0))
        ));
    }

    #[test]
    fn a_forward_child_reference_rejects() {
        // One node: Π(0, 0) referencing itself.
        let mut bytes = vec![];
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT_MARKER.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(3);
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // empty context
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // term root
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // expected root
        assert!(matches!(
            decode_mathematical_certificate(&bytes),
            Err(CodecError::MalformedMathematicalCertificate(
                "term child does not precede its parent",
            ))
        ));
    }

    #[test]
    fn two_primitive_nodes_round_trip_and_verify() {
        use proof_admission::{Budget, DEFAULT_CONVERSION_STEPS, verify_mathematical_certificate};

        let mut arena = TermArena::new();
        // Γ = A : Type 0, a : A, t : Two proves
        // `(caseTwo(C, a, a, zero), caseTwo(C, a, a, one)) : Σ(_:A). A`
        // for the constant family `C := λ(_:Two). A` — the wire must carry
        // the `Two` type, both constructors, and the eliminator.
        let type_zero = type_sort(&mut arena, 0);
        let a_binding = variable(&mut arena, 0);
        let two_binding = arena.insert(Term::Two);
        let family_domain = arena.insert(Term::Two);
        // Under C's binder (depth 4) A is index 3.
        let a_under = variable(&mut arena, 3);
        let family = lambda(&mut arena, family_domain, a_under);
        let a_term = variable(&mut arena, 1);
        let zero = arena.insert(Term::TwoZero);
        let on_zero = arena.insert(Term::CaseTwo {
            motive: family,
            zero_branch: a_term,
            one_branch: a_term,
            scrutinee: zero,
        });
        let a_again = variable(&mut arena, 1);
        let one = arena.insert(Term::TwoOne);
        let on_one = arena.insert(Term::CaseTwo {
            motive: family,
            zero_branch: a_again,
            one_branch: a_again,
            scrutinee: one,
        });
        let term = arena.insert(Term::Pair {
            first: on_zero,
            second: on_one,
        });
        // Σ(_:A). A in depth 3: the domain is index 2, and under the
        // binder A is index 3.
        let sigma_domain = variable(&mut arena, 2);
        let sigma_codomain = variable(&mut arena, 3);
        let expected = arena.insert(Term::Sigma {
            domain: sigma_domain,
            codomain: sigma_codomain,
        });
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: vec![type_zero, a_binding, two_binding],
            term,
            expected,
        };
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
        // Thirteen distinct nodes: Sort(Type 0), Variable(0), Two,
        // Variable(3), C's lambda, Variable(1), TwoZero, the `zero`
        // elimination, TwoOne, the `one` elimination, the pair,
        // Variable(2), and the Σ.
        assert_eq!(decoded.arena.len(), 13);
        verify_mathematical_certificate(
            &mut decoded.arena,
            &decoded.certificate,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .expect("the decoded judgment must re-check in the kernel");
    }

    #[test]
    fn unreachable_table_nodes_are_not_canonical() {
        let (arena, certificate) = polymorphic_identity();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        // Forged: one unreachable `Variable(0)` node appended after the real
        // table, signature and roots unchanged.
        let roots_len = 4 + 4 + 4 + 4; // empty signature + context counts + term + expected
        let split = bytes.len() - roots_len;
        let node_count = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
        let mut forged = Vec::new();
        forged.extend_from_slice(&bytes[..14]); // magic, marker, level arity
        forged.extend_from_slice(&(node_count + 1).to_le_bytes());
        forged.extend_from_slice(&bytes[18..split]);
        forged.push(1);
        forged.extend_from_slice(&0_u32.to_le_bytes());
        forged.extend_from_slice(&bytes[split..]);
        match decode_mathematical_certificate(&forged) {
            Err(error) => assert_eq!(error, CodecError::NonCanonicalEncoding),
            Ok(_) => panic!("an unreachable table node must not decode"),
        }
    }

    /// `A : Type max(u+1, v), x : A ⊢ x : A` at level arity 2 — a context
    /// whose binding sorts hold parameter, successor, and maximum level
    /// syntax.
    fn level_polymorphic_context() -> (TermArena, MathematicalCertificate) {
        let mut arena = TermArena::new();
        let u_plus_one = Level::Parameter(0).successor().unwrap();
        let level = u_plus_one.maximum(Level::Parameter(1));
        let binding_type = arena.insert(Term::Sort(Sort::Type(level)));
        let bound = variable(&mut arena, 0);
        let shifted = variable(&mut arena, 1);
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 2,
            context: vec![binding_type, bound],
            term: bound,
            expected: shifted,
        };
        (arena, certificate)
    }

    #[test]
    fn level_expressions_inside_sorts_round_trip_exactly() {
        let (arena, certificate) = level_polymorphic_context();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        let decoded = decode_mathematical_certificate(&bytes).expect("decode");
        assert_eq!(decoded.certificate.level_arity, 2);
        // The decoded binding carries the exact level syntax — decoding
        // does not normalize `max(u+1, v)` or confuse a parameter for a
        // constant.
        assert_eq!(
            decoded.arena.get(decoded.certificate.context[0]),
            Term::Sort(Sort::Type(
                Level::Parameter(0)
                    .successor()
                    .unwrap()
                    .maximum(Level::Parameter(1))
            ))
        );
        assert_eq!(
            encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
                .expect("re-encode"),
            bytes,
        );
    }

    #[test]
    fn a_level_polymorphic_certificate_re_verifies_after_the_wire() {
        use proof_admission::{Budget, DEFAULT_CONVERSION_STEPS, verify_mathematical_certificate};

        let (arena, certificate) = level_polymorphic_context();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
        verify_mathematical_certificate(
            &mut decoded.arena,
            &decoded.certificate,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .expect("the polymorphic judgment must re-check in the kernel");
    }

    #[test]
    fn stale_and_unknown_markers_do_not_decode() {
        let (arena, certificate) = polymorphic_identity();
        let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
        // Formats 0–2 predate the signature section; an unknown marker is
        // refused the same way rather than guessing a layout.
        for marker in [0_u16, 1, 2, 9] {
            let mut stale = bytes.clone();
            stale[8..10].copy_from_slice(&marker.to_le_bytes());
            assert!(matches!(
                decode_mathematical_certificate(&stale),
                Err(CodecError::UnsupportedFormatMarker(seen)) if seen == marker
            ));
        }
    }

    #[test]
    fn malformed_levels_reject() {
        // A sort node whose level carries an unknown tag is not a level.
        let mut bytes = vec![];
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT_MARKER.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // level arity
        bytes.extend_from_slice(&1_u32.to_le_bytes()); // one node
        bytes.push(2); // Sort
        bytes.push(1); // Type layer
        bytes.push(0); // unknown level tag
        assert!(matches!(
            decode_mathematical_certificate(&bytes),
            Err(CodecError::InvalidTag("MathematicalLevel", 0))
        ));

        // Depth is a producer resource bound, not a judgment: a successor
        // chain past the level bound refuses on encode and on decode
        // alike, before any term table entry is accepted.
        let mut deep = Level::Constant(0);
        for _ in 0..MAX_MATHEMATICAL_LEVEL_DEPTH {
            deep = Level::Successor(Box::new(deep));
        }
        let mut arena = TermArena::new();
        let deep_sort = arena.insert(Term::Sort(Sort::Type(deep)));
        let certificate = MathematicalCertificate {
            signature: Vec::new(),
            level_arity: 0,
            context: Vec::new(),
            term: deep_sort,
            expected: deep_sort,
        };
        assert_eq!(
            encode_mathematical_certificate(&arena, &certificate),
            Err(CodecError::MalformedMathematicalCertificate(
                "level nesting too deep",
            ))
        );

        let mut bytes = vec![];
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT_MARKER.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(2);
        bytes.push(1);
        bytes.extend(std::iter::repeat_n(
            3, // Successor
            MAX_MATHEMATICAL_LEVEL_DEPTH as usize,
        ));
        bytes.push(1); // Constant
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            decode_mathematical_certificate(&bytes),
            Err(CodecError::MalformedMathematicalCertificate(
                "level nesting too deep",
            ))
        ));
    }
}
