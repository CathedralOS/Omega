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
use super::wire::{Reader, Writer, bounded_count, decode_counted};

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
    let mut term_writer = MathematicalTermWriter::new(arena);
    // Declaration statements and bodies are roots: the signature's terms
    // join the table before the judgment's, in signature order.
    for declaration in &certificate.signature {
        term_writer.encode_term(declaration.ty, 0)?;
        if let Some(body) = declaration.body {
            term_writer.encode_term(body, 0)?;
        }
    }
    for &binding in &certificate.context {
        term_writer.encode_term(binding, 0)?;
    }
    term_writer.encode_term(certificate.term, 0)?;
    term_writer.encode_term(certificate.expected, 0)?;

    let MathematicalTermWriter {
        nodes,
        by_handle,
        by_bytes: _by_bytes,
        count,
        ..
    } = term_writer;

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
    let node_count = bounded_count(&mut reader, 1)?;
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
    // A declaration occupies at least one byte on the wire, so a count
    // beyond the remaining input cannot parse.
    let signature_count = bounded_count(&mut reader, 1)?;
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
    let context_count = bounded_count(&mut reader, 1)?;
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

struct MathematicalTermWriter<'arena> {
    arena: &'arena TermArena,
    nodes: Writer,
    by_handle: HashMap<TermHandle, u32>,
    by_bytes: HashMap<Vec<u8>, u32>,
    count: u32,
}

impl<'arena> MathematicalTermWriter<'arena> {
    fn new(arena: &'arena TermArena) -> Self {
        Self {
            arena,
            nodes: Writer::default(),
            by_handle: HashMap::new(),
            by_bytes: HashMap::new(),
            count: 0,
        }
    }

    /// Encode one term and its transitive children, each exactly once and always
    /// before its parents; returns the node's table index.
    ///
    /// Table identity is content, not producer handle identity: a node's key is
    /// its own wire bytes (tag plus already-canonical child indices), so two
    /// producers encoding the same judgment emit the same bytes and two distinct
    /// handles holding equal terms share one table entry. `by_handle` only
    /// short-circuits revisits of an already-emitted handle.
    fn encode_term(&mut self, handle: TermHandle, depth: u32) -> Result<u32, CodecError> {
        if let Some(&index) = self.by_handle.get(&handle) {
            return Ok(index);
        }
        if depth >= MAX_MATHEMATICAL_TERM_DEPTH {
            return Err(CodecError::MalformedMathematicalCertificate(
                "mathematical term nesting too deep",
            ));
        }
        let mut node = Writer::default();
        match self.arena.get(handle) {
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
                let domain = self.encode_term(domain, depth + 1)?;
                let codomain = self.encode_term(codomain, depth + 1)?;
                node.u8(3);
                node.u32(domain);
                node.u32(codomain);
            }
            Term::Lambda { domain, body } => {
                let domain = self.encode_term(domain, depth + 1)?;
                let body = self.encode_term(body, depth + 1)?;
                node.u8(4);
                node.u32(domain);
                node.u32(body);
            }
            Term::Apply { function, argument } => {
                let function = self.encode_term(function, depth + 1)?;
                let argument = self.encode_term(argument, depth + 1)?;
                node.u8(5);
                node.u32(function);
                node.u32(argument);
            }
            Term::Sigma { domain, codomain } => {
                let domain = self.encode_term(domain, depth + 1)?;
                let codomain = self.encode_term(codomain, depth + 1)?;
                node.u8(6);
                node.u32(domain);
                node.u32(codomain);
            }
            Term::Pair { first, second } => {
                let first = self.encode_term(first, depth + 1)?;
                let second = self.encode_term(second, depth + 1)?;
                node.u8(7);
                node.u32(first);
                node.u32(second);
            }
            Term::Fst { pair } => {
                let pair = self.encode_term(pair, depth + 1)?;
                node.u8(8);
                node.u32(pair);
            }
            Term::Snd { pair } => {
                let pair = self.encode_term(pair, depth + 1)?;
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
                let motive = self.encode_term(motive, depth + 1)?;
                let zero_branch = self.encode_term(zero_branch, depth + 1)?;
                let one_branch = self.encode_term(one_branch, depth + 1)?;
                let scrutinee = self.encode_term(scrutinee, depth + 1)?;
                node.u8(13);
                node.u32(motive);
                node.u32(zero_branch);
                node.u32(one_branch);
                node.u32(scrutinee);
            }
            Term::Id { ty, left, right } => {
                let ty = self.encode_term(ty, depth + 1)?;
                let left = self.encode_term(left, depth + 1)?;
                let right = self.encode_term(right, depth + 1)?;
                node.u8(14);
                node.u32(ty);
                node.u32(left);
                node.u32(right);
            }
            Term::Refl { ty, value } => {
                let ty = self.encode_term(ty, depth + 1)?;
                let value = self.encode_term(value, depth + 1)?;
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
                let motive = self.encode_term(motive, depth + 1)?;
                let base = self.encode_term(base, depth + 1)?;
                let endpoint = self.encode_term(endpoint, depth + 1)?;
                let proof = self.encode_term(proof, depth + 1)?;
                node.u8(16);
                node.u32(motive);
                node.u32(base);
                node.u32(endpoint);
                node.u32(proof);
            }
            Term::W { carrier, children } => {
                let carrier = self.encode_term(carrier, depth + 1)?;
                let children = self.encode_term(children, depth + 1)?;
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
                let carrier = self.encode_term(carrier, depth + 1)?;
                let children = self.encode_term(children, depth + 1)?;
                let label = self.encode_term(label, depth + 1)?;
                let function = self.encode_term(function, depth + 1)?;
                node.u8(18);
                node.u32(carrier);
                node.u32(children);
                node.u32(label);
                node.u32(function);
            }
            Term::IndW { motive, step, tree } => {
                let motive = self.encode_term(motive, depth + 1)?;
                let step = self.encode_term(step, depth + 1)?;
                let tree = self.encode_term(tree, depth + 1)?;
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
                let ty = self.encode_term(ty, depth + 1)?;
                let scrutinee = self.encode_term(scrutinee, depth + 1)?;
                node.u8(22);
                node.u32(ty);
                node.u32(scrutinee);
            }
            Term::Squash { ty } => {
                let ty = self.encode_term(ty, depth + 1)?;
                node.u8(23);
                node.u32(ty);
            }
            Term::SquashIntro { ty, value } => {
                let ty = self.encode_term(ty, depth + 1)?;
                let value = self.encode_term(value, depth + 1)?;
                node.u8(24);
                node.u32(ty);
                node.u32(value);
            }
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                let proposition = self.encode_term(proposition, depth + 1)?;
                let function = self.encode_term(function, depth + 1)?;
                let scrutinee = self.encode_term(scrutinee, depth + 1)?;
                node.u8(25);
                node.u32(proposition);
                node.u32(function);
                node.u32(scrutinee);
            }
            Term::Box { ty } => {
                let ty = self.encode_term(ty, depth + 1)?;
                node.u8(26);
                node.u32(ty);
            }
            Term::BoxIntro { ty, value } => {
                let ty = self.encode_term(ty, depth + 1)?;
                let value = self.encode_term(value, depth + 1)?;
                node.u8(27);
                node.u32(ty);
                node.u32(value);
            }
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                let motive = self.encode_term(motive, depth + 1)?;
                let body = self.encode_term(body, depth + 1)?;
                let scrutinee = self.encode_term(scrutinee, depth + 1)?;
                node.u8(28);
                node.u32(motive);
                node.u32(body);
                node.u32(scrutinee);
            }
        }
        let bytes = node.finish();
        if let Some(&index) = self.by_bytes.get(&bytes) {
            // The same term under another producer handle shares the earlier
            // table entry; remember the alias so revisits cost one lookup.
            self.by_handle.insert(handle, index);
            return Ok(index);
        }
        // Children already occupy lower table indices, so the node takes the
        // next position — the postorder the decoder requires.
        let index = self.count;
        self.nodes.bytes(&bytes);
        self.by_bytes.insert(bytes, index);
        self.by_handle.insert(handle, index);
        self.count += 1;
        Ok(index)
    }
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
            let levels = decode_counted(reader, |reader| decode_level(reader, 0))?;
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
