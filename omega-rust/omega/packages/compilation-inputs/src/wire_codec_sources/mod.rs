//! Wire codec machine source synthesis.
//!
//! `Schema::encode` and `Schema::decode` reach stage 04 as intrinsics with no
//! callee: the borrow layer sees an invalid target symbol and the receiver is
//! the schema data, so no `BorrowCallFact`/`FlowCallFact` exists and the call
//! omits at `statement sequence: call: flow call`. The downstream emit chain
//! needs no changes once a callee exists — a machine attached to the schema
//! resolves through the ordinary `attached_machine_state_symbol` arm — so the
//! missing piece is exactly the callee. This module synthesizes one ordinary
//! checked machine per actually-called (schema, operation) pair as source
//! text, and `assemble` injects it through the same Psi source-parsing
//! path the build prelude uses, so the generated bodies are admitted by the
//! real pipeline rather than constructed in any representation by hand
//! ("codecs are ordinary checked library machines whose generated bodies are
//! Admitted").
//!
//! Generation follows the supported wire storage shapes:
//!
//! * Only `data` declarations in product-scope sources whose non-erased
//!   members are all `#N`-identified fields of a directly expressible shape
//!   (scalars, fixed arrays, borrowed slices, `FixedVec`, and scalar-member
//!   nested messages). Borrowed non-byte slices can encode but need owning
//!   destination storage to decode. Unsupported shapes decline generation.
//! * Only operations whose argument types render from occurrence-local
//!   declarations, composed field/array projections, or a same-source
//!   machine's declared exclusive-reference result. Borrowed places and
//!   existing reference values use the same declaration evidence. Every site
//!   for one (schema, operation) must agree on one signature, including sites
//!   whose types remain unknown, or no machine is emitted.
//! * Never for uncalled pairs: an unused generated machine whose body meets
//!   an open wall would newly fail a program that currently compiles.
//! * Storage shape is independent of predicate validation. Encoding reads
//!   established values; decoding must execute the destination's declared
//!   predicates before assignment. The parser's bound subject occurrences
//!   supply substitution, and ordinary source checking verifies each guarded
//!   store. Neither a matching carrier nor compiler-generated origin grants a
//!   domain. Unavailable predicate source declines decoding, not encoding.
//!
//! The emitted state chains mirror `wire_encode_body`/`wire_decode_body` in
//! the checked interpreter byte-for-byte: current-era varint, per-field tag
//! varint, scalar varint content with zigzag for signed members, packed
//! length-in-bytes framing for fixed arrays, canonical-LEB128 read rules
//! (truncation, eleventh group, tenth payload above one, overlong zero
//! terminal), the sticky `bad` flag with every operation still running after
//! a violation, and the final cursor/verdict write-throughs. One deliberate
//! difference: a repeated-body open whose length is out of range runs the
//! element reads with `end = 0` rather than the interpreter's wrapped cursor
//! sum — u64 `+` would trap where the interpreter only flags — and the close
//! check flags `bad` either way.
//! A failed domain guard also skips replacement: Invalid leaves partial output
//! unspecified, but it cannot expose invalid bytes in an established field.
mod analysis;
mod emission;

use std::collections::{HashMap, HashSet};

use source::{DependencyScope, SourceMap, SourceOrigin};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceHandle;

use analysis::{
    WireMemberShape, collect_call_sites, collect_domains, collect_plain_fields, wire_schema_members,
};
use emission::{emit_decode_machine, emit_encode_machine};

/// The assembled source text of every generated codec machine, or None when
/// nothing call-directed is generatable this compile.
pub(crate) fn generated_wire_codec_source(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
) -> Option<String> {
    let product_sources: HashSet<usize> = sources
        .files()
        .filter(|file| {
            file.dependency_scope == DependencyScope::Product
                && file.origin != SourceOrigin::Toolchain
        })
        .map(|file| file.source_id.0)
        .collect();

    // A file carrying a package or module item owns a scope the injected
    // default-package source cannot name; its data is not a candidate.
    let mut scoped_sources: HashSet<usize> = HashSet::new();
    for handle in syntax_trees.root_item_handles() {
        let path = match syntax_trees.tables.items.item(*handle) {
            Item::Package(package) => package.path,
            Item::Module(module) => module.path,
            _ => continue,
        };
        if let Some(first) = syntax_trees
            .tables
            .items
            .identifier_path_members(path)
            .first()
        {
            scoped_sources.insert(first.source_span().source_id.0);
        }
    }

    // Plain `data` declarations also serve call-site typing: `&sample` names a
    // `let sample: BlobSample`, `&mut main.buffer` a field of `Main`. The
    // field map completes before member resolution so a nested-message
    // member can name a declaration declared after it.
    let mut data_fields: HashMap<String, HashMap<String, TypeReferenceHandle>> = HashMap::new();
    for handle in syntax_trees.root_item_handles() {
        let Some(data) = syntax_trees.tables.items.item(*handle).as_data() else {
            continue;
        };
        if !product_sources.contains(&data.name.source_span().source_id.0) {
            continue;
        }
        if let Some(fields) = collect_plain_fields(syntax_trees, data) {
            data_fields.insert(data.name.as_str(syntax_trees).to_string(), fields);
        }
    }
    let mut schemas: Vec<(String, Vec<WireMemberShape>)> = Vec::new();
    for handle in syntax_trees.root_item_handles() {
        let Some(data) = syntax_trees.tables.items.item(*handle).as_data() else {
            continue;
        };
        let source_id = data.name.source_span().source_id.0;
        if !product_sources.contains(&source_id) || scoped_sources.contains(&source_id) {
            continue;
        }
        if let Some(members) = wire_schema_members(syntax_trees, &data_fields, data) {
            schemas.push((data.name.as_str(syntax_trees).to_string(), members));
        }
    }
    if schemas.is_empty() {
        return None;
    }
    let schema_names: HashSet<&str> = schemas.iter().map(|(name, _)| name.as_str()).collect();

    let domains = collect_domains(syntax_trees, sources, &product_sources);

    let mut sites: HashMap<(String, &'static str), Vec<Vec<String>>> = HashMap::new();
    for handle in syntax_trees.root_item_handles() {
        let Some(machine) = syntax_trees.tables.items.item(*handle).as_machine() else {
            continue;
        };
        collect_call_sites(
            syntax_trees,
            machine,
            &schema_names,
            &data_fields,
            &mut sites,
        );
    }

    let mut emitted = String::new();
    for (schema_name, members) in &schemas {
        for operation in ["encode", "decode"] {
            let Some(calls) = sites.get(&(schema_name.clone(), operation)) else {
                continue;
            };
            let signature = calls[0].clone();
            if calls.iter().any(|site| site != &signature) {
                // Sites disagree on argument types: attaching one machine
                // would bind a wrong callee somewhere. Decline entirely.
                continue;
            }
            let expected = if operation == "encode" { 3 } else { 4 };
            if signature.len() != expected {
                continue;
            }
            if operation == "encode" {
                emit_encode_machine(
                    &mut emitted,
                    syntax_trees,
                    sources,
                    &domains,
                    &data_fields,
                    &schemas,
                    schema_name,
                    &signature,
                    members,
                );
            } else {
                emit_decode_machine(
                    &mut emitted,
                    syntax_trees,
                    sources,
                    &domains,
                    &data_fields,
                    &schemas,
                    schema_name,
                    &signature,
                    members,
                );
            }
        }
    }
    if emitted.is_empty() {
        None
    } else {
        Some(emitted)
    }
}
