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
//! text, and `assemble` injects it through the same `load_injected_source`
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
//! * Only operations with at least one statement call site whose argument
//!   types render exactly (a borrow of a named `let` local or of a `self`
//!   field). Every site for one (schema, operation) must agree on one
//!   signature, or no machine is emitted — a wrong callee is worse than a
//!   standing wall.
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

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;

use arena::Handle;
use arena::HandleSpan;
use source::DependencyScope;
use source::SourceMap;
use source::SourceOrigin;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::item::Machine;
use syntax_trees::statement::StatementNode;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

/// Scalar wire members the compact_binary codecs carry. `u8`/`u16`/`i8`/`i16`
/// are excluded deliberately: the interpreter's byte-size-keyed table reads a
/// one-byte field as boolean-shaped and refuses two-byte fields, so a
/// generated machine for them could not hold the same semantics.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarKind {
    U32,
    U64,
    I32,
    I64,
    Bool,
}

impl ScalarKind {
    /// The u64 payload expression for encoding (`wire_scalar_varint_value`):
    /// unsigned members load at their width; signed members zigzag at 64
    /// bits. Bools emit 0/1 through a branch, not this expression.
    fn encode_bits_expression(self, member: &str) -> String {
        match self {
            Self::U32 => format!("{member} as u64"),
            Self::U64 => member.to_string(),
            Self::I32 => {
                format!(
                    "(((({member} as i64) << 1) ^ (({member} as i64) >> 63)) as u64 in Wrapping) as u64"
                )
            }
            Self::I64 => {
                format!("((({member} << 1) ^ ({member} >> 63)) as u64 in Wrapping) as u64")
            }
            Self::Bool => String::new(),
        }
    }

    /// `wire_scalar_in_range`'s declared range as a guard on the raw payload
    /// (unsigned kinds, bool) or on the unzigzagged wide value (i32). u64 and
    /// i64 span their carriers, so no check is emitted.
    fn decode_range_guard(self, operand: &str) -> Option<String> {
        match self {
            Self::U32 => Some(format!("{operand} <= 4294967295")),
            Self::Bool => Some(format!("{operand} <= 1")),
            Self::I32 => Some(format!(
                "-2147483648 <= {operand} && {operand} <= 2147483647"
            )),
            Self::U64 | Self::I64 => None,
        }
    }

    /// The carrier name a member's value-field type must peel to.
    fn name(self) -> &'static str {
        match self {
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    /// The stored expression once in range: raw for u32/u64, the unzigzagged
    /// wide value narrowed for i32/i64, `raw == 1` for bool.
    fn decode_store_expression(self, operand: &str) -> String {
        match self {
            Self::U32 => format!("{operand} as u32"),
            Self::I32 => format!("{operand} as i32"),
            Self::Bool => format!("{operand} == 1"),
            Self::U64 | Self::I64 => operand.to_string(),
        }
    }
}

/// The element a borrowed-slice member views. `u8` is not a `ScalarKind`
/// (the interpreter's byte-size table reads it as boolean-shaped), so the
/// byte member gets its own variant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SliceElement {
    Byte,
    Scalar(ScalarKind),
}

#[derive(Clone)]
enum MemberShape {
    Scalar(ScalarKind),
    FixedScalarArray {
        element: ScalarKind,
        length: usize,
    },
    /// A `&[E]` borrowed view of scalars — the zero-copy member. Encoding
    /// frames it as a length varint then the elements (raw bytes for `u8`,
    /// packed varints for wider scalars); decoding a `&[u8]` re-views the
    /// decode buffer's content window rather than copying.
    BorrowedSlice {
        element: SliceElement,
    },
    /// A `FixedVec<E, N>` member: packed element varints behind a byte-length
    /// varint, like a `&[E]` member, except the element count is the
    /// vector's intrinsic `length` rather than a wire-carried count — the
    /// byte length is the only framing the wire needs.
    FixedVec {
        element: ScalarKind,
        capacity: usize,
    },
    /// A member whose type is another wire schema (`header: RoomHeader`):
    /// a byte-length varint framing the nested declaration's own tag+content
    /// sequence, with no era inside the sub-body. The `schema` name resolves
    /// to a member list at emission through the collected schemas.
    NestedMessage {
        schema: String,
    },
}

struct WireMemberShape {
    name: String,
    number: u64,
    shape: MemberShape,
}

/// Canonical unsigned LEB128 bytes (`wire_varint_bytes`).
fn varint_bytes(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            return bytes;
        }
    }
}

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
    for handle in syntax_trees.roots.items.storage_slice() {
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
    // `let sample: BlobSample`, `&mut self.buffer` a field of `Main`. The
    // field map completes before member resolution so a nested-message
    // member can name a declaration declared after it.
    let mut data_fields: HashMap<String, HashMap<String, TypeReferenceHandle>> = HashMap::new();
    for handle in syntax_trees.roots.items.storage_slice() {
        let Item::Data(data) = syntax_trees.tables.items.item(*handle) else {
            continue;
        };
        if !product_sources.contains(&data.name.source_span().source_id.0) {
            continue;
        }
        if let Some(fields) = collect_plain_fields(syntax_trees, data) {
            data_fields.insert(data.name.as_str().to_string(), fields);
        }
    }
    let mut schemas: Vec<(String, Vec<WireMemberShape>)> = Vec::new();
    for handle in syntax_trees.roots.items.storage_slice() {
        let Item::Data(data) = syntax_trees.tables.items.item(*handle) else {
            continue;
        };
        let source_id = data.name.source_span().source_id.0;
        if !product_sources.contains(&source_id) || scoped_sources.contains(&source_id) {
            continue;
        }
        if let Some(members) = wire_schema_members(syntax_trees, &data_fields, data) {
            schemas.push((data.name.as_str().to_string(), members));
        }
    }
    if schemas.is_empty() {
        return None;
    }
    let schema_names: HashSet<&str> = schemas.iter().map(|(name, _)| name.as_str()).collect();

    let domains = collect_domains(syntax_trees, sources, &product_sources);

    let mut sites: HashMap<(String, &'static str), Vec<Vec<String>>> = HashMap::new();
    for handle in syntax_trees.roots.items.storage_slice() {
        let Item::Machine(machine) = syntax_trees.tables.items.item(*handle) else {
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

/// A domain's parser-bound value occurrences belong to its declared carrier,
/// not the generated decoder's destination record (also named `value`).
struct DomainPredicate {
    carrier: String,
    predicate: StorePredicate,
}

/// Literal source segments between the parser's subject occurrences. Joining
/// them with a decoded operand substitutes only value uses: cast types, domain
/// names, member labels, comments and quoted text retain their authored bytes.
/// The parser already owns binder scope; source synthesis must not rediscover
/// it by matching identifier spellings or rename through an intermediate `self`.
#[derive(Clone)]
struct StorePredicate {
    segments: Vec<String>,
}

/// Shape admission is separate from executable validation. Encoding consumes
/// an already-qualified value; only decoding needs to establish its predicates.
type StoreFacts = Result<Vec<StorePredicate>, ()>;

impl StorePredicate {
    fn from_source(sources: &SourceMap, span: SourceSpan, subjects: &[SourceSpan]) -> Self {
        let text = sources.text_at(span);
        let mut segments = Vec::new();
        let mut copied_until = 0;
        for subject in subjects.iter().filter(|subject| {
            subject.source_id == span.source_id
                && subject.span.start >= span.span.start
                && subject.span.end <= span.span.end
        }) {
            let start = subject.span.start - span.span.start;
            let end = subject.span.end - span.span.start;
            segments.push(text[copied_until..start].to_owned());
            copied_until = end;
        }
        segments.push(text[copied_until..].to_owned());
        Self { segments }
    }

    fn instantiate(&self, operand: &str) -> String {
        self.segments.join(operand)
    }
}

/// Retain predicates over the subject already bound by domain parsing. Collect
/// its exact spans once; synthetic parent-membership subjects have no source.
fn collect_domains(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    product_sources: &HashSet<usize>,
) -> HashMap<String, Vec<DomainPredicate>> {
    let mut subjects = syntax_trees
        .tables
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let span = syntax_trees.tables.expressions.source_span(handle);
            (matches!(expression, ExpressionNode::SelfValue)
                && product_sources.contains(&span.source_id.0)
                && span.span.start < span.span.end)
                .then_some(span)
        })
        .collect::<Vec<_>>();
    subjects.sort_unstable_by_key(|span| (span.source_id.0, span.span.start, span.span.end));
    subjects.dedup();
    let mut domains: HashMap<String, Vec<DomainPredicate>> = HashMap::new();
    for handle in syntax_trees.roots.items.storage_slice() {
        let Item::Domain(domain) = syntax_trees.tables.items.item(*handle) else {
            continue;
        };
        if !product_sources.contains(&domain.name.source_span().source_id.0) {
            continue;
        }
        let Some(carrier) = type_text(syntax_trees, domain.target_type) else {
            continue;
        };
        for index in 0..domain.facts.count() {
            let fact = Handle::from_parts(
                domain.facts.start().arena_index() + index,
                domain.facts.start().generation(),
            );
            if let Some(span) = syntax_trees.tables.items.proof_fact_source_span(fact) {
                domains
                    .entry(domain.name.as_str().to_owned())
                    .or_default()
                    .push(DomainPredicate {
                        carrier: carrier.clone(),
                        predicate: StorePredicate::from_source(sources, span, &subjects),
                    });
            }
        }
    }
    domains
}

/// A constraint list's store-side predicates retain their authored subject: domain
/// `requires` spliced from authored source, range constraints rendered as
/// bound conjunctions. `Named` and arithmetic-domain constraints carry no
/// value predicate and admit nothing. A domain the generator cannot see
/// (foreign or indexed) declines the whole operation — an unverifiable
/// constraint cannot be checked into the generated machine.
fn constraint_facts(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    constraints: &[syntax_trees::types::TypeConstraintNode],
    base_name: &str,
) -> StoreFacts {
    let mut facts = Vec::new();
    for constraint in constraints {
        match constraint {
            syntax_trees::types::TypeConstraintNode::Domain(domain) => {
                if !domain.arguments.is_empty() {
                    return Err(());
                }
                // `u32 in WireValue` names the domain declared as
                // `domain u32::WireValue`: the declaration's name is
                // carrier-qualified while the constraint spells only its
                // last segment. Try the authored spelling first, then its
                // carrier-qualified form.
                let mut matched = false;
                let qualified = format!("{base_name}::{}", domain.name.as_str());
                for key in [domain.name.as_str(), qualified.as_str()] {
                    if let Some(carriers) = domains.get(key) {
                        for predicate in carriers {
                            if predicate.carrier == base_name {
                                facts.push(predicate.predicate.clone());
                                matched = true;
                            }
                        }
                    }
                }
                if !matched {
                    return Err(());
                }
            }
            syntax_trees::types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                let low = sources.text_at(syntax_trees.tables.expressions.source_span(*minimum));
                let high = sources.text_at(syntax_trees.tables.expressions.source_span(*maximum));
                let bound = if *end_inclusive { "<=" } else { "<" };
                facts.push(StorePredicate {
                    segments: vec![
                        format!("({low}) <= "),
                        " && ".to_owned(),
                        format!(" {bound} ({high})"),
                    ],
                });
            }
            syntax_trees::types::TypeConstraintNode::Named(_)
            | syntax_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
        }
    }
    Ok(facts)
}

/// The store-side fact texts a value field constrains the member with
/// (subject occurrences retained for operand substitution). `None` marks a field
/// that does not carry the member's shape — emitting a machine over it
/// would fail checking itself, so the caller declines the operation.
/// `array_length` peels a fixed array first for element stores, matching
/// the declared extent too.
fn value_field_facts(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    field: Option<TypeReferenceHandle>,
    expected: ScalarKind,
    array_length: Option<usize>,
) -> Option<StoreFacts> {
    let mut handle = field?;
    if let Some(length) = array_length {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(declared),
        } = syntax_trees.tables.type_references.type_reference(handle)
        else {
            return None;
        };
        if *declared != length {
            return None;
        }
        handle = *element_type;
    }
    let (handle, facts) = store_type_facts(syntax_trees, sources, domains, handle, expected.name());
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) if name.as_str() == expected.name() => {
            Some(facts.and_then(|facts| {
                // Boolean decoding stores `raw != 0`, not the integer operand
                // used by scalar guards. Do not substitute the wrong carrier.
                if expected == ScalarKind::Bool && !facts.is_empty() {
                    Err(())
                } else {
                    Ok(facts)
                }
            }))
        }
        _ => None,
    }
}

/// Peeling storage never discards its qualifications. Both scalar stores and
/// borrowed-view replacement validate these predicates on the decoded operand
/// before exposing it through a domain-qualified destination.
fn store_type_facts(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    mut handle: TypeReferenceHandle,
    carrier: &str,
) -> (TypeReferenceHandle, StoreFacts) {
    let mut facts = Ok(Vec::new());
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = syntax_trees.tables.type_references.type_reference(handle)
    {
        let additional = constraint_facts(
            syntax_trees,
            sources,
            domains,
            syntax_trees
                .tables
                .type_references
                .constraints(*constraints),
            carrier,
        );
        facts = facts.and_then(|mut facts| {
            facts.extend(additional?);
            Ok(facts)
        });
        handle = *base_type;
    }
    (handle, facts)
}

/// The field-name -> declared-type map for a `data` declaration (used to read
/// `self` field types at call sites). Declarations carrying case variants are
/// declined — `self.field` typing needs the plain record shape.
fn collect_plain_fields(
    syntax_trees: &SyntaxTrees,
    data: &syntax_trees::item::DataDefinition,
) -> Option<HashMap<String, TypeReferenceHandle>> {
    let mut fields = HashMap::new();
    for member in syntax_trees.tables.items.data_members(data.members) {
        let DataMember::Field(field) = member else {
            return None;
        };
        fields.insert(field.name.as_str().to_string(), field.type_reference);
    }
    Some(fields)
}

/// A wire schema is a plain data declaration whose members all carry `#N`
/// identities and classify to supported wire shapes. Anything richer —
/// variants, retired members, parameters, `where` facts, declared properties,
/// non-scalar field types — declines and keeps meeting the intrinsic wall.
fn wire_schema_members(
    syntax_trees: &SyntaxTrees,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    data: &syntax_trees::item::DataDefinition,
) -> Option<Vec<WireMemberShape>> {
    if !data.lifetime_parameters.is_empty()
        || !data.type_parameters.is_empty()
        || !data.where_facts.is_empty()
        || data.quotient.is_some()
        || data.generic_instance.is_some()
        || data.properties != Default::default()
    {
        return None;
    }
    let mut members = Vec::new();
    for member in syntax_trees.tables.items.data_members(data.members) {
        let DataMember::Field(field) = member else {
            return None;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let number = field.identity?;
        let shape = member_shape(syntax_trees, data_fields, field.type_reference)?;
        members.push(WireMemberShape {
            name: field.name.as_str().to_string(),
            number,
            shape,
        });
    }
    if members.is_empty() {
        return None;
    }
    members.sort_by_key(|member| member.number);
    Some(members)
}

fn member_shape(
    syntax_trees: &SyntaxTrees,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    handle: TypeReferenceHandle,
) -> Option<MemberShape> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => {
            if let Some(kind) = scalar_kind(name.as_str()) {
                Some(MemberShape::Scalar(kind))
            } else if data_fields.contains_key(name.as_str()) {
                Some(MemberShape::NestedMessage {
                    schema: name.as_str().to_string(),
                })
            } else {
                None
            }
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } if base_name.as_str() == "FixedVec" => {
            let arguments = syntax_trees
                .tables
                .type_references
                .type_reference_handles(*arguments);
            if arguments.len() != 2 {
                return None;
            }
            let element = scalar_kind_of_type(syntax_trees, arguments[0])?;
            let capacity = const_literal_capacity(syntax_trees, arguments[1])?;
            (capacity <= 64).then_some(MemberShape::FixedVec { element, capacity })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if *length <= 64 => scalar_kind_of_type(syntax_trees, *element_type).map(|element| {
            MemberShape::FixedScalarArray {
                element,
                length: *length,
            }
        }),
        TypeReferenceNode::Reference {
            referee, access, ..
        } if !access.is_exclusive() => {
            let mut referee = *referee;
            // `&([E] in Domain)`: a slice's own qualification binds inside the
            // reference, so peel domains (argument-free only, as below) before
            // reading the storage shape.
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = syntax_trees.tables.type_references.type_reference(referee)
            {
                if !syntax_trees
                    .tables
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| {
                        matches!(
                            constraint,
                            syntax_trees::types::TypeConstraintNode::Domain(domain)
                                if domain.arguments.is_empty()
                        )
                    })
                {
                    return None;
                }
                referee = *base_type;
            }
            match syntax_trees.tables.type_references.type_reference(referee) {
                TypeReferenceNode::Slice { element_type } => {
                    slice_element_of_type(syntax_trees, *element_type)
                        .map(|element| MemberShape::BorrowedSlice { element })
                }
                _ => None,
            }
        }
        // `&[E] in Domain`: a member's declared qualification rides the same
        // borrowed-slice storage shape; the constraints spell domains only
        // (the codec cannot reason about parameterized ones).
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } if syntax_trees
            .tables
            .type_references
            .constraints(*constraints)
            .iter()
            .all(|constraint| {
                matches!(
                    constraint,
                    syntax_trees::types::TypeConstraintNode::Domain(domain)
                        if domain.arguments.is_empty()
                )
            }) =>
        {
            member_shape(syntax_trees, data_fields, *base_type)
        }
        _ => None,
    }
}

fn slice_element_of_type(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<SliceElement> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) if name.as_str() == "u8" => Some(SliceElement::Byte),
        TypeReferenceNode::Named(name) => scalar_kind(name.as_str()).map(SliceElement::Scalar),
        _ => None,
    }
}

fn scalar_kind_of_type(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<ScalarKind> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => scalar_kind(name.as_str()),
        _ => None,
    }
}

/// A const-generic capacity argument's literal value — `4` in
/// `FixedVec<i32, 4>` currently parses as a bare `Named` type reference
/// whose identifier text is the decimal literal (const-generic arguments
/// are not yet elaborated into `ConstExpression` nodes); a
/// `ConstExpression` wrapping an integer literal is also accepted, and any
/// other spelling declines.
fn const_literal_capacity(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<usize> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(identifier) => identifier.as_str().parse::<usize>().ok(),
        TypeReferenceNode::ConstExpression(expression) => {
            let ExpressionNode::Integer(literal) =
                syntax_trees.tables.expressions.expression(*expression)
            else {
                return None;
            };
            usize::try_from(literal.value_u64()?).ok()
        }
        _ => None,
    }
}

fn scalar_kind(name: &str) -> Option<ScalarKind> {
    match name {
        "u32" => Some(ScalarKind::U32),
        "u64" => Some(ScalarKind::U64),
        "i32" => Some(ScalarKind::I32),
        "i64" => Some(ScalarKind::I64),
        "bool" => Some(ScalarKind::Bool),
        _ => None,
    }
}

/// Walk one machine's statements for `S::encode`/`S::decode` calls and render
/// each argument's parameter type. Statement calls only — the corpus form is
/// a call statement; an expression-lane codec call keeps meeting the wall.
fn collect_call_sites(
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    schema_names: &HashSet<&str>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    sites: &mut HashMap<(String, &'static str), Vec<Vec<String>>>,
) {
    for state_handle in syntax_trees.tables.items.state_handles(machine.states) {
        let state = syntax_trees.tables.items.state(*state_handle);
        for statement_handle in syntax_trees.tables.items.statements(state.statements) {
            let StatementNode::Call(call) =
                syntax_trees.tables.statements.statement(*statement_handle)
            else {
                continue;
            };
            let receiver = syntax_trees
                .tables
                .statements
                .identifier_path_members(call.receiver);
            if receiver.len() != 1 || !call.target_is_static {
                continue;
            }
            let schema_name = receiver[0].as_str();
            if !schema_names.contains(schema_name) {
                continue;
            }
            let operation = match call.target.as_str() {
                "encode" => "encode",
                "decode" => "decode",
                _ => continue,
            };
            let Some(mut arguments) =
                render_argument_types(syntax_trees, machine, data_fields, call.arguments)
            else {
                continue;
            };
            // The codec's buffer position accepts any fixed byte array: the
            // intrinsic contract validates `&[u8; N]` with the decoder
            // bounds-checking reads at runtime, so the generated signature
            // declares the view every sized array loans to rather than one
            // call site's length.
            if arguments.len() > 1
                && let Some(view) = buffer_view(&arguments[1])
            {
                arguments[1] = view;
            }
            sites
                .entry((schema_name.to_string(), operation))
                .or_default()
                .push(arguments);
        }
    }
}

/// Render each call argument as the parameter type it must bind: a borrow of a
/// named local (`&sample` -> `&BlobSample` via its `let` type) or of a `self`
/// field (`&mut self.buffer` -> `&mut [u8; 64]` via the attached data's
/// field). Anything else declines so no machine binds a guessed signature.
fn render_argument_types(
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    arguments: HandleSpan<ExpressionHandle>,
) -> Option<Vec<String>> {
    let mut rendered = Vec::new();
    for argument in syntax_trees.tables.statements.expression_handles(arguments) {
        let ExpressionNode::Borrow(borrow) = syntax_trees.tables.expressions.expression(*argument)
        else {
            return None;
        };
        let access = if borrow.access.is_exclusive() {
            "&mut "
        } else {
            "&"
        };
        let target = syntax_trees.tables.expressions.expression(borrow.target);
        let base_type = match target {
            ExpressionNode::Name(path) => {
                let members = syntax_trees
                    .tables
                    .expressions
                    .identifier_path_members(*path);
                if members.len() != 1 {
                    return None;
                }
                local_type(syntax_trees, machine, members[0].as_str())?
            }
            ExpressionNode::Member(member) => {
                // `&mut self.buffer` reads the attached data's fields; a named
                // `&mut Data` parameter (`main.buffer` in `main: &mut Main`)
                // carries the same declaration through the machine's parameter
                // list. Both spellings name the same storage.
                let data_name = match syntax_trees.tables.expressions.expression(member.receiver) {
                    ExpressionNode::SelfValue => {
                        machine.attached_data.as_ref()?.as_str().to_string()
                    }
                    ExpressionNode::Name(path) => {
                        let members = syntax_trees
                            .tables
                            .expressions
                            .identifier_path_members(*path);
                        if members.len() != 1 {
                            return None;
                        }
                        parameter_data_name(syntax_trees, machine, members[0].as_str())?
                    }
                    _ => return None,
                };
                let fields = data_fields.get(data_name.as_str())?;
                *fields.get(member.member.as_str())?
            }
            _ => return None,
        };
        rendered.push(format!("{access}{}", type_text(syntax_trees, base_type)?));
    }
    Some(rendered)
}

/// `&[u8; N]`/`&mut [u8; N]` -> `&[u8]`/`&mut [u8]`: the buffer parameter's
/// canonical view form. Anything not a fixed byte-array borrow declines so a
/// mis-positioned argument keeps its rendered type and the mismatch surfaces.
fn buffer_view(rendered: &str) -> Option<String> {
    for (prefix, access) in [("&mut [u8; ", "&mut [u8]"), ("&[u8; ", "&[u8]")] {
        if let Some(rest) = rendered.strip_prefix(prefix)
            && rest.ends_with(']')
            && rest[..rest.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            return Some(access.to_string());
        }
    }
    None
}

/// The data declaration a named machine parameter carries when its type is a
/// reference (`main: &mut Main` -> `Main`) or a bare named data (`m: Main` ->
/// `Main`). Anything else — a scalar, slice, generic, or absent name — has no
/// field map to render against and declines.
fn parameter_data_name(
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    name: &str,
) -> Option<String> {
    for state_handle in syntax_trees.tables.items.state_handles(machine.states) {
        let state = syntax_trees.tables.items.state(*state_handle);
        for parameter_handle in syntax_trees.tables.items.state_parameters(state.parameters) {
            let parameter = syntax_trees.tables.items.state_parameter(*parameter_handle);
            if parameter.name.as_str() != name {
                continue;
            }
            let mut type_reference = parameter.type_reference;
            if let TypeReferenceNode::Reference { referee, .. } = syntax_trees
                .tables
                .type_references
                .type_reference(type_reference)
            {
                type_reference = *referee;
            }
            let TypeReferenceNode::Named(named) = syntax_trees
                .tables
                .type_references
                .type_reference(type_reference)
            else {
                return None;
            };
            return Some(named.as_str().to_string());
        }
    }
    None
}

/// The declared type of a `let`/`let mut` local anywhere in the machine —
/// codec arguments are conventionally `let`-bound before the call.
fn local_type(
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    name: &str,
) -> Option<TypeReferenceHandle> {
    for state_handle in syntax_trees.tables.items.state_handles(machine.states) {
        let state = syntax_trees.tables.items.state(*state_handle);
        for statement_handle in syntax_trees.tables.items.statements(state.statements) {
            let StatementNode::LocalData(local) =
                syntax_trees.tables.statements.statement(*statement_handle)
            else {
                continue;
            };
            if local.name.as_str() == name {
                return Some(local.type_reference);
            }
        }
    }
    None
}

fn type_text(syntax_trees: &SyntaxTrees, handle: TypeReferenceHandle) -> Option<String> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Some(format!(
            "[{}; {}]",
            type_text(syntax_trees, *element_type)?,
            length
        )),
        TypeReferenceNode::Slice { element_type } => {
            Some(format!("[{}]", type_text(syntax_trees, *element_type)?))
        }
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            let access = if access.is_exclusive() { "&mut " } else { "&" };
            Some(format!("{access}{}", type_text(syntax_trees, *referee)?))
        }
        _ => None,
    }
}

// ---------- emission ----------
//
// State-label discipline: each emission helper writes `state` blocks whose
// entry label is `name` and whose terminal transition targets `next`. Helper
// internals hang sub-states off `name` (`{name}_put`, ...). `requires` on a
// byte-touching state names the guard that proves the index, and its only
// predecessor is the guarded arm.

struct MachineEmitter<'a> {
    text: &'a mut String,
    /// Shared parameter declarations (`value: &V, out: &mut B, written: &mut
    /// u64, at: u64` for encode; the decode list adds `bad: u64`).
    declarations: String,
    /// Shared argument list (`(value, out, written, at)`).
    call: String,
    /// The byte-source length expression bounds are proved against.
    limit: &'static str,
}

impl MachineEmitter<'_> {
    /// `state name(declarations, extras) requires ... { body }`
    fn state(&mut self, name: &str, extras: &str, requires: &str, body: &str) {
        let extras = if extras.is_empty() {
            String::new()
        } else {
            format!(", {extras}")
        };
        let requires = if requires.is_empty() {
            String::new()
        } else {
            format!(" requires {requires}")
        };
        let _ = writeln!(
            self.text,
            "    state {name}({}{}){requires} {{\n{body}    }}\n",
            self.declarations, extras
        );
    }

    /// `state name(declarations, extras) { transition { _ -> next(inner,
    /// extra_args) } }` — a named stage boundary hop. `extra_args` joins the
    /// shared arguments inside the parens.
    fn hop(&mut self, name: &str, extras: &str, next: &str, extra_args: &str) {
        let inner = self.call.trim_start_matches('(').trim_end_matches(')');
        self.state(
            name,
            extras,
            "",
            &format!("        transition {{ _ -> {next}({inner}{extra_args}) }}\n"),
        );
    }

    // ----- encode helpers (thread: value, out, written, at) -----

    /// Guarded literal-byte store: `transition at < out.len { true -> put _
    /// -> overflow }`; `put` writes `out[at] = byte` behind the proved guard
    /// and continues with `at + 1`.
    fn enc_byte(&mut self, name: &str, byte: u8, next: &str) {
        let put = format!("{name}_put");
        self.state(
            name,
            "",
            "",
            &format!(
                "        transition at < {} {{\n            true -> {put}{}\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call, self.call
            ),
        );
        self.state(
            &put,
            "",
            &format!("at < {}", self.limit),
            &format!(
                "        out[at] = {byte};\n        transition {{ _ -> {next}(value, out, written, at + 1) }}\n"
            ),
        );
    }

    /// Canonical LEB128 emission of u64 `bits`, entering with `bits` already a
    /// state parameter (`name` is the loop state).
    fn enc_varint_loop(&mut self, name: &str, next: &str) {
        let last = format!("{name}_last");
        let put = format!("{name}_put");
        let more = format!("{name}_more");
        let put_more = format!("{name}_put_more");
        self.state(
            name,
            "bits: u64",
            "",
            &format!(
                "        transition bits < 128 {{\n            true -> {last}(value, out, written, at, bits)\n            _ -> {more}(value, out, written, at, bits)\n        }}\n"
            ),
        );
        self.state(
            &last,
            "bits: u64",
            "",
            &format!(
                "        let byte: u8 = bits as u8;\n        transition at < {} {{\n            true -> {put}(value, out, written, at, byte)\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.state(
            &put,
            "byte: u8",
            &format!("at < {}", self.limit),
            &format!(
                "        out[at] = byte;\n        transition {{ _ -> {next}(value, out, written, at + 1) }}\n"
            ),
        );
        self.state(
            &more,
            "bits: u64",
            "",
            &format!(
                "        let byte: u8 = ((bits & 127) | 128) as u8;\n        transition at < {} {{\n            true -> {put_more}(value, out, written, at, byte, bits)\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.state(
            &put_more,
            "byte: u8, bits: u64",
            &format!("at < {}", self.limit),
            &format!(
                "        out[at] = byte;\n        transition {{ _ -> {name}(value, out, written, at + 1, bits >> 7) }}\n"
            ),
        );
    }

    /// `let bits: u64 = expr;` then the varint loop.
    fn enc_varint(&mut self, name: &str, expression: &str, next: &str) {
        let loop_state = format!("{name}_loop");
        self.state(
            name,
            "",
            "",
            &format!(
                "        let bits: u64 = {expression};\n        transition {{ _ -> {loop_state}(value, out, written, at, bits) }}\n"
            ),
        );
        self.enc_varint_loop(&loop_state, next);
    }

    /// Varint byte-length measure of `bits`, accumulating into `acc` and
    /// continuing at `next` (which takes `acc: u64 in Wrapping`). The counter
    /// rides the `Wrapping` domain: it is a byte extent bounded by buffer
    /// extent in every reachable program, but `acc + 1` is an overflow-proof
    /// obligation the checker cannot discharge inductively, so the domain
    /// names its totality instead of a bound.
    fn enc_measure(&mut self, name: &str, expression: &str, next: &str) {
        let loop_state = format!("{name}_loop");
        self.state(
            name,
            "acc: u64 in Wrapping",
            "",
            &format!(
                "        let bits: u64 = {expression};\n        transition {{ _ -> {loop_state}(value, out, written, at, acc, bits) }}\n"
            ),
        );
        self.state(
            &loop_state,
            "acc: u64 in Wrapping, bits: u64",
            "",
            &format!(
                "        transition bits < 128 {{\n            true -> {next}(value, out, written, at, acc + 1)\n            _ -> {loop_state}(value, out, written, at, acc + 1, bits >> 7)\n        }}\n"
            ),
        );
    }

    /// Byte-for-byte copy of a `&[u8]` member into the buffer: `i` walks the
    /// view while `at` walks the output. Requires on a callee must be
    /// established by a guard in its immediate caller, so the element-read
    /// and store bounds are re-tested in the state right before the call.
    fn enc_slice_bytes(&mut self, name: &str, field: &str, next: &str) {
        let walk = format!("{name}_walk");
        let bound = format!("{name}_bound");
        let put = format!("{name}_put");
        // `requires` only admits parameter operands, so the member is seeded
        // as a `view` local at the entry state: `i < view.len` is a
        // parameter-rooted term the contract namespace admits, and `view[i]`
        // proves in `put` without any member-path correspondence.
        self.state(
            name,
            "",
            "",
            &format!(
                "        let view: &[u8] = value.{field};\n        transition {{ _ -> {walk}(value, out, written, at, 0, view) }}\n"
            ),
        );
        self.state(
            &walk,
            "i: u64, view: &[u8]",
            "",
            &format!(
                "        transition i < view.len {{\n            true -> {bound}(value, out, written, at, i, view)\n            _ -> {next}(value, out, written, at)\n        }}\n"
            ),
        );
        self.state(
            &bound,
            "i: u64, view: &[u8]",
            "",
            &format!(
                "        transition at < {} && i < view.len {{\n            true -> {put}(value, out, written, at, i, view)\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.state(
            &put,
            "i: u64, view: &[u8]",
            &format!("at < {} && i < view.len", self.limit),
            &format!(
                "        out[at] = view[i];\n        transition {{ _ -> {walk}(value, out, written, at + 1, i + 1, view) }}\n"
            ),
        );
    }

    /// Packed element varints of a borrowed `&[E]` member (E wider than
    /// `u8`): measure the elements' total varint extent, publish it as the
    /// byte length, then emit the elements. `i` walks the view and `acc` the
    /// measured extent; the view's `.len` is re-read at each loop head so
    /// the index evidence stays the same name the element reads use.
    fn enc_packed_slice(&mut self, name: &str, field: &str, element: ScalarKind, next: &str) {
        self.enc_packed(
            name,
            &format!("value.{field}.len"),
            &format!("value.{field}"),
            None,
            element,
            next,
        );
    }

    /// Packed element varints behind a byte-length varint, shared by
    /// borrowed-slice (`len` elements at `value.f[i]`) and `FixedVec`
    /// (`length` elements at `value.f.items[i]`, index-bounded by the
    /// constant `bound` capacity) members.
    fn enc_packed(
        &mut self,
        name: &str,
        len_expr: &str,
        items_expr: &str,
        bound_expr: Option<&str>,
        element: ScalarKind,
        next: &str,
    ) {
        // `requires` predicates only admit parameter operands, never a
        // member-path read like `value.readings.len` or
        // `value.samples.length`. `FixedVec` seeds a `slen` local; slices
        // instead seed a `view` local bound to the member itself, so every
        // guard and requires spells `i < view.len` — a parameter-rooted
        // term the contract namespace admits — and `view[i]` reads are
        // provable inside each callee without any correspondence.
        let seed_len = bound_expr.is_some();
        let member_expr = items_expr;
        let (index_guard, loop_guard, index_requires, carry, carry_arg, items_expr) =
            match bound_expr {
                Some(bound) => (
                    format!("i < slen && i < {bound}"),
                    "i < slen".to_string(),
                    format!("i < slen && i < {bound}"),
                    ", slen: u64".to_string(),
                    ", slen".to_string(),
                    member_expr.to_string(),
                ),
                None => (
                    "i < view.len".to_string(),
                    "i < view.len".to_string(),
                    "i < view.len".to_string(),
                    format!(", view: &[{}]", element.name()),
                    ", view".to_string(),
                    "view".to_string(),
                ),
            };
        let measure_entry = format!("{name}_measure");
        let measure_elem = format!("{name}_measure_elem");
        let measure_varint = format!("{name}_measure_varint");
        let length = format!("{name}_length");
        let emit_entry = format!("{name}_emit");
        let emit_entry_target = format!("{name}_emit_seed");
        let emit_elem = format!("{name}_emit_elem");
        let emit_varint = format!("{name}_emit_varint");
        let emit_last = format!("{name}_emit_last");
        let emit_last_put = format!("{name}_emit_last_put");
        let emit_more = format!("{name}_emit_more");
        let emit_more_put = format!("{name}_emit_more_put");
        let bits_expr = |i: &str| element.encode_bits_expression(&format!("{items_expr}[{i}]"));
        // `FixedVec` seeds `slen` (`value.samples.length` is a member path
        // no predicate admits); slices seed `view` from the member itself —
        // `i < view.len` is parameter-rooted everywhere it rides. The bound
        // checks live only at the two entry states; the inner varint byte
        // loops re-enter through them, so `bits < 128` alone decides.
        if seed_len {
            let seed = format!("{name}_seed");
            self.state(
                name,
                "",
                "",
                &format!(
                    "        let slen: u64 = {len_expr};\n        transition {{ _ -> {seed}(value, out, written, at, slen) }}\n"
                ),
            );
            self.hop(&seed, "slen: u64", &measure_entry, ", 0, 0, slen");
        } else {
            self.state(
                name,
                "",
                "",
                &format!(
                    "        let view: &[{}] = {member_expr};\n        transition {{ _ -> {measure_entry}(value, out, written, at, 0, 0, view) }}\n",
                    element.name()
                ),
            );
        }
        self.state(
            &measure_entry,
            &format!("i: u64, acc: u64 in Wrapping{carry}"),
            "",
            &format!(
                "        transition {index_guard} {{\n            true -> {measure_elem}(value, out, written, at, i, acc{carry_arg})\n            _ -> {length}(value, out, written, at, i, acc)\n        }}\n"
            ),
        );
        self.state(
            &measure_elem,
            &format!("i: u64, acc: u64 in Wrapping{carry}"),
            &index_requires,
            &format!(
                "        let bits: u64 = {};\n        transition {{ _ -> {measure_varint}(value, out, written, at, i, acc, bits{carry_arg}) }}\n",
                bits_expr("i")
            ),
        );
        self.state(
            &measure_varint,
            &format!("i: u64, acc: u64 in Wrapping, bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        transition bits < 128 && {loop_guard} {{\n            true -> {measure_entry}(value, out, written, at, i + 1, acc + 1{carry_arg})\n            _ -> {measure_varint}(value, out, written, at, i, acc + 1, bits >> 7{carry_arg})\n        }}\n"
            ),
        );
        self.state(
            &length,
            "i: u64, acc: u64 in Wrapping",
            "",
            &format!(
                "        let bits: u64 = acc as u64;\n        transition {{ _ -> {length}_loop(value, out, written, at, i, bits) }}\n"
            ),
        );
        // The byte-length varint: the canonical last/more loop with `i`
        // riding as an inert carry so the emit half starts over at 0.
        {
            let last = format!("{length}_loop_last");
            let put = format!("{length}_loop_put");
            let more = format!("{length}_loop_more");
            let put_more = format!("{length}_loop_put_more");
            let loop_name = format!("{length}_loop");
            self.state(
                &loop_name,
                "i: u64, bits: u64",
                "",
                &format!(
                    "        transition bits < 128 {{\n            true -> {last}(value, out, written, at, i, bits)\n            _ -> {more}(value, out, written, at, i, bits)\n        }}\n"
                ),
            );
            self.state(
                &last,
                "i: u64, bits: u64",
                "",
                &format!(
                    "        let byte: u8 = bits as u8;\n        transition at < {} {{\n            true -> {put}(value, out, written, at, i, byte)\n            _ -> encode_overflow{}\n        }}\n",
                    self.limit, self.call
                ),
            );
            self.state(
                &put,
                "i: u64, byte: u8",
                &format!("at < {}", self.limit),
                &format!(
                    "        out[at] = byte;\n        transition {{ _ -> {emit_entry_target}(value, out, written, at + 1, 0) }}\n"
                ),
            );
            self.state(
                &more,
                "i: u64, bits: u64",
                "",
                &format!(
                    "        let byte: u8 = ((bits & 127) | 128) as u8;\n        transition at < {} {{\n            true -> {put_more}(value, out, written, at, i, byte, bits)\n            _ -> encode_overflow{}\n        }}\n",
                    self.limit, self.call
                ),
            );
            self.state(
                &put_more,
                "i: u64, byte: u8, bits: u64",
                &format!("at < {}", self.limit),
                &format!(
                    "        out[at] = byte;\n        transition {{ _ -> {loop_name}(value, out, written, at + 1, i, bits >> 7) }}\n"
                ),
            );
        }
        // The emit half re-seeds the iteration bound under each member
        // kind — `slen` for `FixedVec`, `view` for slices — so the
        // length-loop tail stays thin.
        if bound_expr.is_some() {
            self.state(
                &emit_entry_target,
                "i: u64",
                "",
                &format!(
                    "        let slen: u64 = {len_expr};\n        transition {{ _ -> {emit_entry}(value, out, written, at, i, slen) }}\n"
                ),
            );
        } else {
            self.state(
                &emit_entry_target,
                "i: u64",
                "",
                &format!(
                    "        let view: &[{}] = {member_expr};\n        transition {{ _ -> {emit_entry}(value, out, written, at, i, view) }}\n",
                    element.name()
                ),
            );
        }
        self.state(
            &emit_entry,
            &format!("i: u64{carry}"),
            "",
            &format!(
                "        transition {index_guard} {{\n            true -> {emit_elem}(value, out, written, at, i{carry_arg})\n            _ -> {next}(value, out, written, at)\n        }}\n"
            ),
        );
        self.state(
            &emit_elem,
            &format!("i: u64{carry}"),
            &index_requires,
            &format!(
                "        let bits: u64 = {};\n        transition {{ _ -> {emit_varint}(value, out, written, at, i, bits{carry_arg}) }}\n",
                bits_expr("i")
            ),
        );
        self.state(
            &emit_varint,
            &format!("i: u64, bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        transition bits < 128 {{\n            true -> {emit_last}(value, out, written, at, i, bits{carry_arg})\n            _ -> {emit_more}(value, out, written, at, i, bits{carry_arg})\n        }}\n"
            ),
        );
        self.state(
            &emit_last,
            &format!("i: u64, bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        let byte: u8 = bits as u8;\n        transition at < {} && {loop_guard} {{\n            true -> {emit_last_put}(value, out, written, at, i, byte{carry_arg})\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.state(
            &emit_last_put,
            &format!("i: u64, byte: u8{carry}"),
            &format!("at < {} && {loop_guard}", self.limit),
            &format!(
                "        out[at] = byte;\n        transition {{ _ -> {emit_entry}(value, out, written, at + 1, i + 1{carry_arg}) }}\n"
            ),
        );
        self.state(
            &emit_more,
            &format!("i: u64, bits: u64{carry}"),
            &loop_guard,
            &format!(
                "        let byte: u8 = ((bits & 127) | 128) as u8;\n        transition at < {} && {loop_guard} {{\n            true -> {emit_more_put}(value, out, written, at, i, byte, bits{carry_arg})\n            _ -> encode_overflow{}\n        }}\n",
                self.limit, self.call
            ),
        );
        self.state(
            &emit_more_put,
            &format!("i: u64, byte: u8, bits: u64{carry}"),
            &format!("at < {} && {loop_guard}", self.limit),
            &format!(
                "        out[at] = byte;\n        transition {{ _ -> {emit_varint}(value, out, written, at + 1, i, bits >> 7{carry_arg}) }}\n"
            ),
        );
    }

    /// A nested-message member's sub-body: the byte length is the static
    /// tag bytes plus each non-bool member's varint extent measured in
    /// `acc`; the body then emits the nested schema's own tag+content
    /// sequence with no era. `member_facts` admits scalar-only sub-members,
    /// so a bool content is the branch byte and everything else a varint.
    fn enc_nested(&mut self, name: &str, field: &str, sub_members: &[WireMemberShape], next: &str) {
        let measure_stage = format!("{name}_measure");
        let length = format!("{name}_length");
        let mut static_bytes: u64 = 0;
        let mut measured: Vec<&WireMemberShape> = Vec::new();
        for sub in sub_members {
            static_bytes += varint_bytes(sub.number).len() as u64;
            match &sub.shape {
                MemberShape::Scalar(ScalarKind::Bool) => static_bytes += 1,
                MemberShape::Scalar(_) => measured.push(sub),
                _ => continue,
            }
        }
        let first = if measured.is_empty() {
            length.clone()
        } else {
            measure_stage.clone()
        };
        self.hop(name, "", &first, &format!(", {static_bytes}"));
        for (index, sub) in measured.iter().enumerate() {
            let label = if index == 0 {
                measure_stage.clone()
            } else {
                format!("{name}_measure_{index}")
            };
            let measure_next = if index + 1 == measured.len() {
                length.clone()
            } else {
                format!("{name}_measure_{}", index + 1)
            };
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            self.enc_measure(
                &label,
                &kind.encode_bits_expression(&format!("value.{field}.{}", sub.name)),
                &measure_next,
            );
        }
        let first_sub = format!("{name}_subtag_0_0");
        let sub_next = |index: usize| {
            if index + 1 == sub_members.len() {
                next.to_string()
            } else {
                format!("{name}_subtag_{}_0", index + 1)
            }
        };
        self.state(
            &length,
            "acc: u64 in Wrapping",
            "",
            &format!(
                "        let bits: u64 = acc as u64;\n        transition {{ _ -> {length}_loop(value, out, written, at, bits) }}\n"
            ),
        );
        self.enc_varint_loop(&format!("{length}_loop"), &first_sub);
        for (sub_index, sub) in sub_members.iter().enumerate() {
            let after_sub = sub_next(sub_index);
            let tag_bytes = varint_bytes(sub.number);
            let content = format!("{name}_subcontent_{sub_index}");
            for (index, byte) in tag_bytes.iter().enumerate() {
                let label = format!("{name}_subtag_{sub_index}_{index}");
                let byte_next = if index + 1 == tag_bytes.len() {
                    content.clone()
                } else {
                    format!("{name}_subtag_{sub_index}_{}", index + 1)
                };
                self.enc_byte(&label, *byte, &byte_next);
            }
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            if *kind == ScalarKind::Bool {
                self.state(
                    &content,
                    "",
                    "",
                    &format!(
                        "        transition value.{field}.{} {{\n            true -> {content}_one{}\n            _ -> {content}_zero{}\n        }}\n",
                        sub.name, self.call, self.call
                    ),
                );
                self.enc_byte(&format!("{content}_one"), 1, &after_sub);
                self.enc_byte(&format!("{content}_zero"), 0, &after_sub);
            } else {
                self.enc_varint(
                    &content,
                    &kind.encode_bits_expression(&format!("value.{field}.{}", sub.name)),
                    &after_sub,
                );
            }
        }
    }

    // ----- decode helpers (thread: value, buffer, read, verdict, at, bad) -----

    /// `expect_byte`: `buffer[at]` must equal `byte`. OOB flags `bad` without
    /// consuming; a mismatch consumes and flags. `carry`/`carry_arg` thread
    /// extra state (the repeated-body `end`) through every state.
    fn dec_byte(&mut self, name: &str, byte: u8, next: &str, carry: &str, carry_arg: &str) {
        let check = format!("{name}_check");
        let oob = format!("{name}_oob");
        let inner = self.call.trim_start_matches('(').trim_end_matches(')');
        self.state(
            name,
            carry,
            "",
            &format!(
                "        transition at < {} {{\n            true -> {check}({inner}{carry_arg})\n            _ -> {oob}({inner}{carry_arg})\n        }}\n",
                self.limit
            ),
        );
        self.state(
            &check,
            carry,
            &format!("at < {}", self.limit),
            &format!(
                "        transition buffer[at] == {byte} {{\n            true -> {next}(value, buffer, read, verdict, at + 1, bad{carry_arg})\n            _ -> {next}(value, buffer, read, verdict, at + 1, bad + 1{carry_arg})\n        }}\n"
            ),
        );
        self.state(
            &oob,
            carry,
            "",
            &format!(
                "        transition {{ _ -> {next}(value, buffer, read, verdict, at, bad + 1{carry_arg}) }}\n"
            ),
        );
    }

    /// Canonical LEB128 read mirroring the interpreter loop: the `shift > 63`
    /// guard runs before the byte read (no consumption), OOB truncation is
    /// not consumed, and a consumed byte accumulates `payload << shift` with
    /// the tenth-payload and overlong-terminal checks folding into `bad`.
    /// `name` is the entry (it only ever receives `carry` extras); `next`
    /// receives `carry` then the accumulated `raw`.
    fn dec_varint(&mut self, name: &str, next: &str, carry: &str, carry_arg: &str) {
        let start = format!("{name}_start");
        let read = format!("{name}_read");
        let byte = format!("{name}_byte");
        let terminal = format!("{name}_terminal");
        let terminal_check = format!("{name}_terminal_check");
        let continue_state = format!("{name}_continue");
        let acc_shift = join_extras("acc: u64 in Wrapping, shift: u64 in Wrapping", carry, "");
        let full = join_extras(
            "acc: u64 in Wrapping, shift: u64 in Wrapping",
            carry,
            ", payload: u64 in Wrapping",
        );
        self.state(
            name,
            carry,
            "",
            &format!(
                "        transition {{ _ -> {start}(value, buffer, read, verdict, at, bad, 0 as u64 in Wrapping, 0 as u64 in Wrapping{carry_arg}) }}\n"
            ),
        );
        self.state(
            &start,
            &acc_shift,
            "",
            &format!(
                "        transition shift > 63 {{\n            true -> {next}(value, buffer, read, verdict, at, bad + 1{carry_arg}, acc as u64)\n            _ -> {read}(value, buffer, read, verdict, at, bad, acc, shift{carry_arg})\n        }}\n"
            ),
        );
        self.state(
            &read,
            &acc_shift,
            "",
            &format!(
                "        transition at < {} {{\n            true -> {byte}(value, buffer, read, verdict, at, bad, acc, shift{carry_arg})\n            _ -> {next}(value, buffer, read, verdict, at, bad + 1{carry_arg}, acc as u64)\n        }}\n",
                self.limit
            ),
        );
        self.state(
            &byte,
            &acc_shift,
            &format!("at < {}", self.limit),
            &format!(
                "        let byte: u8 = buffer[at];\n        let payload: u64 in Wrapping = (byte & 127) as u64 in Wrapping;\n        let acc2: u64 in Wrapping = acc | (payload << shift);\n        transition byte < 128 {{\n            true -> {terminal}(value, buffer, read, verdict, at + 1, bad, acc2, shift{carry_arg}, payload)\n            _ -> {continue_state}(value, buffer, read, verdict, at + 1, bad, acc2, shift{carry_arg}, payload)\n        }}\n"
            ),
        );
        self.state(
            &terminal,
            &full,
            "",
            &format!(
                "        transition payload == 0 && shift > 0 {{\n            true -> {terminal_check}(value, buffer, read, verdict, at, bad + 1, acc, shift{carry_arg}, payload)\n            _ -> {terminal_check}(value, buffer, read, verdict, at, bad, acc, shift{carry_arg}, payload)\n        }}\n"
            ),
        );
        self.state(
            &terminal_check,
            &full,
            "",
            &format!(
                "        transition shift == 63 && payload > 1 {{\n            true -> {next}(value, buffer, read, verdict, at, bad + 1{carry_arg}, acc as u64)\n            _ -> {next}(value, buffer, read, verdict, at, bad{carry_arg}, acc as u64)\n        }}\n"
            ),
        );
        self.state(
            &continue_state,
            &full,
            "",
            &format!(
                "        transition shift == 63 && payload > 1 {{\n            true -> {start}(value, buffer, read, verdict, at, bad + 1, acc, shift + 7{carry_arg})\n            _ -> {start}(value, buffer, read, verdict, at, bad, acc, shift + 7{carry_arg})\n        }}\n"
            ),
        );
    }

    /// Scalar store behind the declared range guard and any value-field
    /// constraint facts; a failure flags `bad` and skips the store, matching
    /// `wire_scalar_in_range`. Signed members unzigzag to `wide: i64` before
    /// the guard, which doubles as the store state's `requires` so the
    /// bounded assignment discharges. `carry`/`carry_arg` thread extra state
    /// (a length-prefixed body's `end`) through both states.
    fn dec_scalar_store(
        &mut self,
        name: &str,
        field: &str,
        kind: ScalarKind,
        next: &str,
        facts: &[StorePredicate],
        carry: &str,
        carry_arg: &str,
    ) {
        let store = format!("{name}_write");
        let operand = if kind.decodes_signed() { "wide" } else { "raw" };
        let entry_extras = carried_operand(carry, "raw: u64");
        let mut guard: Vec<String> = kind.decode_range_guard(operand).into_iter().collect();
        guard.extend(facts.iter().map(|fact| fact.instantiate(operand)));
        let requires = guard.join(" && ");
        let entry = if guard.is_empty() {
            format!(
                "        transition {{ _ -> {store}(value, buffer, read, verdict, at, bad{carry_arg}, {operand}) }}\n"
            )
        } else {
            format!(
                "        transition {requires} {{\n            true -> {store}(value, buffer, read, verdict, at, bad{carry_arg}, {operand})\n            _ -> {next}(value, buffer, read, verdict, at, bad + 1{carry_arg})\n        }}\n"
            )
        };
        match kind {
            ScalarKind::I32 | ScalarKind::I64 => {
                self.state(
                    name,
                    &entry_extras,
                    "",
                    &format!(
                        "        let half: u64 = raw >> 1;\n        let mask: i64 = 0 - ((raw & 1) as i64);\n        let wide: i64 = ((half as i64 in Wrapping) ^ (mask as i64 in Wrapping)) as i64;\n{entry}"
                    ),
                );
                self.state(
                    &store,
                    &carried_operand(carry, "wide: i64"),
                    &requires,
                    &format!(
                        "        value.{field} = {};\n        transition {{ _ -> {next}(value, buffer, read, verdict, at, bad{carry_arg}) }}\n",
                        kind.decode_store_expression("wide")
                    ),
                );
            }
            ScalarKind::U32 | ScalarKind::Bool | ScalarKind::U64 => {
                self.state(name, &entry_extras, "", &entry);
                self.state(
                    &store,
                    &carried_operand(carry, "raw: u64"),
                    &requires,
                    &format!(
                        "        value.{field} = {};\n        transition {{ _ -> {next}(value, buffer, read, verdict, at, bad{carry_arg}) }}\n",
                        kind.decode_store_expression("raw")
                    ),
                );
            }
        }
    }

    /// Element store for a packed fixed array: `value.f[index] = decoded`
    /// behind the element's range guard, threading `end` before `raw` (the
    /// varint continuation order).
    fn dec_element_store(
        &mut self,
        name: &str,
        field: &str,
        index: usize,
        kind: ScalarKind,
        next: &str,
        facts: &[StorePredicate],
    ) {
        let store = format!("{name}_write");
        let entry_extras = "end: u64, raw: u64";
        let operand = if kind.decodes_signed() { "wide" } else { "raw" };
        let mut guard: Vec<String> = kind.decode_range_guard(operand).into_iter().collect();
        guard.extend(facts.iter().map(|fact| fact.instantiate(operand)));
        let requires = guard.join(" && ");
        let entry = if guard.is_empty() {
            format!(
                "        transition {{ _ -> {store}(value, buffer, read, verdict, at, bad, end, {operand}) }}\n"
            )
        } else {
            format!(
                "        transition {requires} {{\n            true -> {store}(value, buffer, read, verdict, at, bad, end, {operand})\n            _ -> {next}(value, buffer, read, verdict, at, bad + 1, end)\n        }}\n"
            )
        };
        if kind.decodes_signed() {
            self.state(
                name,
                entry_extras,
                "",
                &format!(
                    "        let half: u64 = raw >> 1;\n        let mask: i64 = 0 - ((raw & 1) as i64);\n        let wide: i64 = ((half as i64 in Wrapping) ^ (mask as i64 in Wrapping)) as i64;\n{entry}"
                ),
            );
            self.state(
                &store,
                &format!("end: u64, {operand}: i64"),
                &requires,
                &format!(
                    "        value.{field}[{index}] = {};\n        transition {{ _ -> {next}(value, buffer, read, verdict, at, bad, end) }}\n",
                    kind.decode_store_expression("wide")
                ),
            );
        } else {
            self.state(name, entry_extras, "", &entry);
            self.state(
                &store,
                "end: u64, raw: u64",
                &requires,
                &format!(
                    "        value.{field}[{index}] = {};\n        transition {{ _ -> {next}(value, buffer, read, verdict, at, bad, end) }}\n",
                    kind.decode_store_expression("raw")
                ),
            );
        }
    }

    /// `FixedVec<E, N>` member: a byte-length varint opens the `end` window,
    /// then each capacity slot reads an element while bytes remain — element
    /// `k` runs only behind `at < end`, so a shorter body lands `length = k`
    /// and the close still proves `at == end`. A body longer than capacity
    /// leaves `at < end` at the close and flags `bad` through it.
    fn dec_fixed_vec(
        &mut self,
        name: &str,
        field: &str,
        element: ScalarKind,
        capacity: usize,
        next: &str,
        facts: &[StorePredicate],
    ) {
        let open = format!("{name}_open");
        let open_in = format!("{name}_open_in");
        let slack_state = format!("{name}_slack");
        let end_state = format!("{name}_end");
        let close = format!("{name}_close");
        let first = format!("{name}_element_0");
        // `end` derives as `buffer.len - ((buffer.len - at) - raw)`, one
        // named local per state so each subtraction's bound is a requires
        // premise — arithmetic call arguments cannot carry it. On failure
        // `end = 0` stands in for the wrapped cursor sum and the close
        // flags `bad`.
        self.dec_varint(name, &open, "", "");
        self.state(
            &open,
            "raw: u64",
            "",
            &format!(
                "        transition at <= {} {{\n            true -> {open_in}(value, buffer, read, verdict, at, bad, raw)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n",
                self.limit
            ),
        );
        self.state(
            &open_in,
            "raw: u64",
            &format!("at <= {}", self.limit),
            &format!(
                "        let room: u64 = buffer.len - at;\n        transition raw <= room && room <= buffer.len {{\n            true -> {slack_state}(value, buffer, read, verdict, at, bad, raw, room)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
            ),
        );
        self.state(
            &slack_state,
            "raw: u64, room: u64",
            &format!("raw <= room && room <= {}", self.limit),
            &format!(
                "        let slack: u64 = room - raw;\n        transition slack <= room && slack <= buffer.len && room <= buffer.len {{\n            true -> {end_state}(value, buffer, read, verdict, at, bad, slack, room)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
            ),
        );
        self.state(
            &end_state,
            "slack: u64, room: u64",
            &format!("slack <= room && slack <= {} && room <= {}", self.limit, self.limit),
            &format!(
                "        let end: u64 = buffer.len - slack;\n        transition {{ _ -> {first}(value, buffer, read, verdict, at, bad, end) }}\n"
            ),
        );
        for index in 0..capacity {
            let element_state = format!("{name}_element_{index}");
            let read = format!("{name}_element_{index}_read");
            let store = format!("{name}_element_{index}_store");
            let length_state = format!("{name}_length_{index}");
            let after_element = format!("{name}_element_{}", index + 1);
            self.state(
                &element_state,
                "end: u64",
                "",
                &format!(
                    "        transition at < end {{\n            true -> {read}(value, buffer, read, verdict, at, bad, end)\n            _ -> {length_state}(value, buffer, read, verdict, at, bad, end)\n        }}\n"
                ),
            );
            self.dec_varint(&read, &store, "end: u64", ", end");
            self.dec_element_store(
                &store,
                &format!("{field}.items"),
                index,
                element,
                &after_element,
                facts,
            );
        }
        for index in 0..=capacity {
            let length_state = if index == capacity {
                format!("{name}_element_{capacity}")
            } else {
                format!("{name}_length_{index}")
            };
            self.state(
                &length_state,
                "end: u64",
                "",
                &format!(
                    "        value.{field}.length = {index};\n        transition {{ _ -> {close}(value, buffer, read, verdict, at, bad, end) }}\n"
                ),
            );
        }
        self.state(
            &close,
            "end: u64",
            "",
            &format!(
                "        transition at == end {{\n            true -> {next}(value, buffer, read, verdict, at, bad)\n            _ -> {next}(value, buffer, read, verdict, at, bad + 1)\n        }}\n"
            ),
        );
    }

    /// A nested-message member: a byte-length varint opens the `end` window,
    /// the nested schema's own tag+scalar sequence decodes inside it with
    /// `end` carried through every state, and the close flags `bad` unless
    /// the cursor lands exactly on the bound.
    fn dec_nested(
        &mut self,
        name: &str,
        field: &str,
        sub_members: &[WireMemberShape],
        sub_facts: &[Vec<StorePredicate>],
        next: &str,
    ) {
        let open = format!("{name}_open");
        let open_in = format!("{name}_open_in");
        let slack_state = format!("{name}_slack");
        let end_state = format!("{name}_end");
        let close = format!("{name}_close");
        let first = format!("{name}_subtag_0_0");
        // Same window derivation as `dec_fixed_vec`: `end` comes out of
        // complement subtraction through named locals, never `at + raw`.
        self.dec_varint(name, &open, "", "");
        self.state(
            &open,
            "raw: u64",
            "",
            &format!(
                "        transition at <= {} {{\n            true -> {open_in}(value, buffer, read, verdict, at, bad, raw)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n",
                self.limit
            ),
        );
        self.state(
            &open_in,
            "raw: u64",
            &format!("at <= {}", self.limit),
            &format!(
                "        let room: u64 = buffer.len - at;\n        transition raw <= room && room <= buffer.len {{\n            true -> {slack_state}(value, buffer, read, verdict, at, bad, raw, room)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
            ),
        );
        self.state(
            &slack_state,
            "raw: u64, room: u64",
            &format!("raw <= room && room <= {}", self.limit),
            &format!(
                "        let slack: u64 = room - raw;\n        transition slack <= room && slack <= buffer.len && room <= buffer.len {{\n            true -> {end_state}(value, buffer, read, verdict, at, bad, slack, room)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
            ),
        );
        self.state(
            &end_state,
            "slack: u64, room: u64",
            &format!("slack <= room && slack <= {} && room <= {}", self.limit, self.limit),
            &format!(
                "        let end: u64 = buffer.len - slack;\n        transition {{ _ -> {first}(value, buffer, read, verdict, at, bad, end) }}\n"
            ),
        );
        for (sub_index, sub) in sub_members.iter().enumerate() {
            let content = format!("{name}_subcontent_{sub_index}");
            let store = format!("{name}_substore_{sub_index}");
            let tag_bytes = varint_bytes(sub.number);
            for (index, byte) in tag_bytes.iter().enumerate() {
                let label = format!("{name}_subtag_{sub_index}_{index}");
                let byte_next = if index + 1 == tag_bytes.len() {
                    content.clone()
                } else {
                    format!("{name}_subtag_{sub_index}_{}", index + 1)
                };
                self.dec_byte(&label, *byte, &byte_next, "end: u64", ", end");
            }
            let after_sub = if sub_index + 1 == sub_members.len() {
                close.clone()
            } else {
                format!("{name}_subtag_{}_0", sub_index + 1)
            };
            self.dec_varint(&content, &store, "end: u64", ", end");
            let MemberShape::Scalar(kind) = &sub.shape else {
                continue;
            };
            self.dec_scalar_store(
                &store,
                &format!("{field}.{}", sub.name),
                *kind,
                &after_sub,
                &sub_facts[sub_index],
                "end: u64",
                ", end",
            );
        }
        self.state(
            &close,
            "end: u64",
            "",
            &format!(
                "        transition at == end {{\n            true -> {next}(value, buffer, read, verdict, at, bad)\n            _ -> {next}(value, buffer, read, verdict, at, bad + 1)\n        }}\n"
            ),
        );
    }
}

impl ScalarKind {
    fn decodes_signed(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }
}

/// Extras list for a carried state: `{carry}, {operand}` when a carry
/// threads through, else just `{operand}`.
fn carried_operand(carry: &str, operand: &str) -> String {
    if carry.is_empty() {
        operand.to_string()
    } else {
        format!("{carry}, {operand}")
    }
}

/// Join extra parameter declarations: `base` then `carry` then `more`.
fn join_extras(base: &str, carry: &str, more: &str) -> String {
    let mut joined = base.to_string();
    if !carry.is_empty() {
        joined.push_str(", ");
        joined.push_str(carry);
    }
    joined.push_str(more);
    joined
}

/// The value type named by a signature's first argument (`&V`/`&mut V`).
fn signature_value_type(signature: &[String]) -> &str {
    signature[0]
        .trim_start_matches('&')
        .trim_start()
        .trim_start_matches("mut")
        .trim_start()
}

/// Pre-pass over members: the value type's declared field must carry the
/// member's scalar (or element scalar) shape, and the domain/range facts it
/// constrains retain their subject for the store guard.
/// `None` marks mismatched storage. Unavailable predicates stay as per-member
/// errors so encoding can use the shape while decoding declines before emission.
fn member_facts(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    schemas: &[(String, Vec<WireMemberShape>)],
    value_type: &str,
    members: &[WireMemberShape],
) -> Option<Vec<StoreFacts>> {
    let value_fields = data_fields.get(value_type)?;
    let mut all = Vec::with_capacity(members.len());
    for member in members {
        // A view's storage shape and its destination predicates are independent
        // obligations: matching &[E] cannot establish a qualified byte domain.
        if let MemberShape::BorrowedSlice { element } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let element_name = match element {
                SliceElement::Byte => "u8",
                SliceElement::Scalar(kind) => kind.name(),
            };
            let carrier = format!("[{element_name}]");
            let (field_handle, facts) =
                store_type_facts(syntax_trees, sources, domains, field_handle, &carrier);
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
            else {
                return None;
            };
            if access.is_exclusive() {
                return None;
            }
            // `&([E] in Domain)`: the slice's qualification binds inside the
            // reference; retain its predicates as well as outer qualifications.
            let (referee, referee_facts) =
                store_type_facts(syntax_trees, sources, domains, *referee, &carrier);
            let facts = facts.and_then(|mut facts| {
                facts.extend(referee_facts?);
                Ok(facts)
            });
            let TypeReferenceNode::Slice { element_type } =
                syntax_trees.tables.type_references.type_reference(referee)
            else {
                return None;
            };
            if slice_element_of_type(syntax_trees, *element_type) != Some(*element) {
                return None;
            }
            all.push(facts);
            continue;
        }
        // A `FixedVec<E, N>` member's value field must spell the same
        // instantiation; the element's own domain/range facts ride every
        // element store's guard exactly like a fixed array's.
        if let MemberShape::FixedVec { element, capacity } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
            else {
                return None;
            };
            let arguments = syntax_trees
                .tables
                .type_references
                .type_reference_handles(*arguments);
            if base_name.as_str() != "FixedVec"
                || arguments.len() != 2
                || scalar_kind_of_type(syntax_trees, arguments[0]) != Some(*element)
                || const_literal_capacity(syntax_trees, arguments[1]) != Some(*capacity)
            {
                return None;
            }
            let facts = value_field_facts(
                syntax_trees,
                sources,
                domains,
                Some(arguments[0]),
                *element,
                None,
            )?;
            all.push(facts);
            continue;
        }
        // A nested-message member's value field names the value mirror of
        // the nested schema; every nested member must be a scalar whose
        // value-side field resolves — non-scalar nesting stays on the
        // intrinsic wall, and the sub-member store guards re-collect the
        // scalar facts at emission.
        if let MemberShape::NestedMessage { schema } = &member.shape {
            let field_handle = value_fields.get(member.name.as_str()).copied()?;
            let TypeReferenceNode::Named(value_name) = syntax_trees
                .tables
                .type_references
                .type_reference(field_handle)
            else {
                return None;
            };
            let nested_fields = data_fields.get(value_name.as_str())?;
            let (_, sub_members) = schemas
                .iter()
                .find(|(name, _)| name.as_str() == schema.as_str())?;
            let mut requirements = Ok(Vec::new());
            for sub in sub_members {
                let MemberShape::Scalar(kind) = &sub.shape else {
                    return None;
                };
                let facts = value_field_facts(
                    syntax_trees,
                    sources,
                    domains,
                    nested_fields.get(sub.name.as_str()).copied(),
                    *kind,
                    None,
                )?;
                // Nested guards are emitted per field, but failure to resolve
                // any predicate must decline decoding before text is emitted.
                requirements = requirements.and(facts.map(|_| Vec::new()));
            }
            all.push(requirements);
            continue;
        }
        let (expected, array_length) = match &member.shape {
            MemberShape::Scalar(kind) => (*kind, None),
            MemberShape::FixedScalarArray { element, length } => (*element, Some(*length)),
            MemberShape::BorrowedSlice { .. }
            | MemberShape::FixedVec { .. }
            | MemberShape::NestedMessage { .. } => unreachable!("handled above"),
        };
        let facts = value_field_facts(
            syntax_trees,
            sources,
            domains,
            value_fields.get(member.name.as_str()).copied(),
            expected,
            array_length,
        )?;
        all.push(facts);
    }
    Some(all)
}

/// Per-sub-member store facts for a nested-message member: the value type's
/// `member` field names the nested value mirror; each nested scalar member's
/// facts resolve against that mirror's field map, one `Vec` per sub-member
/// for the store guards.
#[allow(clippy::too_many_arguments)]
fn nested_sub_facts(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    value_type: &str,
    member_name: &str,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema: &str,
) -> Option<Vec<Vec<StorePredicate>>> {
    let field_handle = data_fields.get(value_type)?.get(member_name).copied()?;
    let TypeReferenceNode::Named(value_name) = syntax_trees
        .tables
        .type_references
        .type_reference(field_handle)
    else {
        return None;
    };
    let nested_fields = data_fields.get(value_name.as_str())?;
    let (_, sub_members) = schemas.iter().find(|(name, _)| name.as_str() == schema)?;
    let mut all = Vec::with_capacity(sub_members.len());
    for sub in sub_members {
        let MemberShape::Scalar(kind) = &sub.shape else {
            return None;
        };
        all.push(
            value_field_facts(
                syntax_trees,
                sources,
                domains,
                nested_fields.get(sub.name.as_str()).copied(),
                *kind,
                None,
            )?
            .ok()?,
        );
    }
    Some(all)
}

/// `machine S::encode(value: &V, out: &mut B, written: &mut u64)`: the era
/// varint, then per field (number order) a tag varint and the member content,
/// ending by publishing the cursor through `written`.
fn emit_encode_machine(
    text: &mut String,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema_name: &str,
    signature: &[String],
    members: &[WireMemberShape],
) {
    if member_facts(
        syntax_trees,
        sources,
        domains,
        data_fields,
        schemas,
        signature_value_type(signature),
        members,
    )
    .is_none()
    {
        return;
    }
    let mut emitter = MachineEmitter {
        text,
        declarations: format!(
            "value: {}, out: {}, written: {}, at: u64",
            signature[0], signature[1], signature[2]
        ),
        call: "(value, out, written, at)".to_string(),
        limit: "out.len",
    };
    let _ = writeln!(
        emitter.text,
        "machine {schema_name}::encode(value: {}, out: {}, written: {}) {{\n    transition {{ _ -> era_0(value, out, written, 0) }}\n",
        signature[0], signature[1], signature[2]
    );

    // Stage labels in wire order: era, then per field tag+content, then
    // finish. Each emission names its continuation.
    let mut stages: Vec<String> = Vec::new();
    for member in members {
        stages.push(format!("field_{}_tag_0", member.number));
        stages.push(format!("field_{}_content", member.number));
    }
    let era_next = stages
        .first()
        .cloned()
        .unwrap_or_else(|| "encode_finish".to_string());
    let era_bytes = varint_bytes(0);
    for (index, byte) in era_bytes.iter().enumerate() {
        let name = format!("era_{index}");
        let next = if index + 1 == era_bytes.len() {
            era_next.clone()
        } else {
            format!("era_{}", index + 1)
        };
        emitter.enc_byte(&name, *byte, &next);
    }

    for (member_index, member) in members.iter().enumerate() {
        let content_label = format!("field_{}_content", member.number);
        let after = if member_index + 1 == members.len() {
            "encode_finish".to_string()
        } else {
            format!("field_{}_tag_0", members[member_index + 1].number)
        };
        let tag_bytes = varint_bytes(member.number);
        for (index, byte) in tag_bytes.iter().enumerate() {
            let name = format!("field_{}_tag_{index}", member.number);
            let next = if index + 1 == tag_bytes.len() {
                content_label.clone()
            } else {
                format!("field_{}_tag_{}", member.number, index + 1)
            };
            emitter.enc_byte(&name, *byte, &next);
        }
        match &member.shape {
            MemberShape::Scalar(ScalarKind::Bool) => {
                emitter.state(
                    &content_label,
                    "",
                    "",
                    &format!(
                        "        transition value.{} {{\n            true -> {}_one{}\n            _ -> {}_zero{}\n        }}\n",
                        member.name, content_label, emitter.call, content_label, emitter.call
                    ),
                );
                emitter.enc_byte(&format!("{content_label}_one"), 1, &after);
                emitter.enc_byte(&format!("{content_label}_zero"), 0, &after);
            }
            MemberShape::Scalar(kind) => {
                let bits = kind.encode_bits_expression(&format!("value.{}", member.name));
                emitter.enc_varint(&content_label, &bits, &after);
            }
            MemberShape::BorrowedSlice { element } => {
                // Each sequence has one byte-length prefix. For raw bytes
                // this is the view extent; packed scalars measure their
                // encoded extent themselves, with no element-count prefix.
                match element {
                    SliceElement::Byte => {
                        let stage = format!("field_{}_body", member.number);
                        emitter.enc_varint(
                            &content_label,
                            &format!("value.{}.len", member.name),
                            &stage,
                        );
                        emitter.enc_slice_bytes(&stage, &member.name, &after);
                    }
                    SliceElement::Scalar(kind) => {
                        emitter.enc_packed_slice(&content_label, &member.name, *kind, &after)
                    }
                }
            }
            MemberShape::FixedVec {
                element, capacity, ..
            } => {
                // The vector's intrinsic `length` bounds the packed
                // elements; the byte length is the only wire framing.
                emitter.enc_packed(
                    &content_label,
                    &format!("value.{}.length", member.name),
                    &format!("value.{}.items", member.name),
                    Some(&capacity.to_string()),
                    *element,
                    &after,
                );
            }
            MemberShape::NestedMessage { schema } => {
                // `member_facts` already proved the nested schema resolves
                // with scalar-only members.
                let Some((_, sub_members)) = schemas
                    .iter()
                    .find(|(name, _)| name.as_str() == schema.as_str())
                else {
                    continue;
                };
                emitter.enc_nested(&content_label, &member.name, sub_members, &after);
            }
            MemberShape::FixedScalarArray { element, length } => {
                // Packed elements carry a byte-length prefix: measure each
                // element's varint extent, emit that total, then the elements.
                emitter.hop(
                    &content_label,
                    "",
                    &format!("field_{}_measure_0", member.number),
                    ", 0",
                );
                for index in 0..*length {
                    let measure_next = if index + 1 == *length {
                        format!("field_{}_length", member.number)
                    } else {
                        format!("field_{}_measure_{}", member.number, index + 1)
                    };
                    emitter.enc_measure(
                        &format!("field_{}_measure_{index}", member.number),
                        &element.encode_bits_expression(&format!("value.{}[{index}]", member.name)),
                        &measure_next,
                    );
                }
                emitter.state(
                    &format!("field_{}_length", member.number),
                    "acc: u64 in Wrapping",
                    "",
                    &format!(
                        "        let bits: u64 = acc as u64;\n        transition {{ _ -> field_{}_length_loop(value, out, written, at, bits) }}\n",
                        member.number
                    ),
                );
                let element_zero = format!("field_{}_element_0", member.number);
                let length_next: &str = if *length == 0 { &after } else { &element_zero };
                emitter
                    .enc_varint_loop(&format!("field_{}_length_loop", member.number), length_next);
                for index in 0..*length {
                    let next = if index + 1 == *length {
                        after.clone()
                    } else {
                        format!("field_{}_element_{}", member.number, index + 1)
                    };
                    emitter.enc_varint(
                        &format!("field_{}_element_{index}", member.number),
                        &element.encode_bits_expression(&format!("value.{}[{index}]", member.name)),
                        &next,
                    );
                }
            }
        }
    }
    let declarations = emitter.declarations.clone();
    let _ = writeln!(
        emitter.text,
        "    state encode_finish({declarations}) {{\n        written = at;\n    }}\n    state encode_overflow({declarations}) {{\n        written = at;\n    }}\n}}\n"
    );
}

/// `machine S::decode(value: &mut V, buffer: &B, read: &mut u64, verdict:
/// &mut Verdict)`: expect the era bytes, then per field the tag bytes and the
/// content; `read` publishes the cursor and `verdict` the soundness flag.
fn emit_decode_machine(
    text: &mut String,
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    domains: &HashMap<String, Vec<DomainPredicate>>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    schemas: &[(String, Vec<WireMemberShape>)],
    schema_name: &str,
    signature: &[String],
    members: &[WireMemberShape],
) {
    let Some(facts_by_member) = member_facts(
        syntax_trees,
        sources,
        domains,
        data_fields,
        schemas,
        signature_value_type(signature),
        members,
    ) else {
        return;
    };
    let Ok(facts_by_member) = facts_by_member.into_iter().collect::<Result<Vec<_>, _>>() else {
        return;
    };
    // A borrowed slice of non-`u8` elements decodes through owning storage —
    // the zero-copy re-view has nothing to point at — so that member kind
    // keeps the whole machine unemitted rather than dangling a content stage.
    if members.iter().any(|member| {
        matches!(
            member.shape,
            MemberShape::BorrowedSlice {
                element: SliceElement::Scalar(_)
            }
        )
    }) {
        return;
    }
    let verdict_type = signature[3]
        .trim_start_matches('&')
        .trim_start()
        .trim_start_matches("mut")
        .trim_start()
        .to_string();
    let mut emitter = MachineEmitter {
        text,
        declarations: format!(
            "value: {}, buffer: {}, read: {}, verdict: {}, at: u64, bad: u64 in Wrapping",
            signature[0], signature[1], signature[2], signature[3]
        ),
        call: "(value, buffer, read, verdict, at, bad)".to_string(),
        limit: "buffer.len",
    };
    let _ = writeln!(
        emitter.text,
        "machine {schema_name}::decode(value: {}, buffer: {}, read: {}, verdict: {}) {{\n    transition {{ _ -> era_0(value, buffer, read, verdict, 0, 0) }}\n",
        signature[0], signature[1], signature[2], signature[3]
    );

    let era_bytes = varint_bytes(0);
    let first_tag = members
        .first()
        .map(|member| format!("field_{}_tag_0", member.number))
        .unwrap_or_else(|| "decode_finish".to_string());
    for (index, byte) in era_bytes.iter().enumerate() {
        let name = format!("era_{index}");
        let next = if index + 1 == era_bytes.len() {
            first_tag.clone()
        } else {
            format!("era_{}", index + 1)
        };
        emitter.dec_byte(&name, *byte, &next, "", "");
    }

    for (member_index, member) in members.iter().enumerate() {
        let content_label = format!("field_{}_content", member.number);
        let after = if member_index + 1 == members.len() {
            "decode_finish".to_string()
        } else {
            format!("field_{}_tag_0", members[member_index + 1].number)
        };
        let tag_bytes = varint_bytes(member.number);
        for (index, byte) in tag_bytes.iter().enumerate() {
            let name = format!("field_{}_tag_{index}", member.number);
            let next = if index + 1 == tag_bytes.len() {
                content_label.clone()
            } else {
                format!("field_{}_tag_{}", member.number, index + 1)
            };
            emitter.dec_byte(&name, *byte, &next, "", "");
        }
        match &member.shape {
            MemberShape::Scalar(kind) => {
                let store = format!("field_{}_store", member.number);
                emitter.dec_varint(&content_label, &store, "", "");
                emitter.dec_scalar_store(
                    &store,
                    &member.name,
                    *kind,
                    &after,
                    &facts_by_member[member_index],
                    "",
                    "",
                );
            }
            MemberShape::FixedVec { element, capacity } => {
                emitter.dec_fixed_vec(
                    &content_label,
                    &member.name,
                    *element,
                    *capacity,
                    &after,
                    &facts_by_member[member_index],
                );
            }
            MemberShape::NestedMessage { schema } => {
                let Some(sub_facts) = nested_sub_facts(
                    syntax_trees,
                    sources,
                    domains,
                    data_fields,
                    signature_value_type(signature),
                    &member.name,
                    schemas,
                    schema,
                ) else {
                    continue;
                };
                let Some((_, sub_members)) = schemas
                    .iter()
                    .find(|(name, _)| name.as_str() == schema.as_str())
                else {
                    continue;
                };
                emitter.dec_nested(
                    &content_label,
                    &member.name,
                    sub_members,
                    &sub_facts,
                    &after,
                );
            }
            MemberShape::BorrowedSlice {
                element: SliceElement::Byte,
            } => {
                // `&[u8]` decodes zero-copy: the length varint bounds the
                // content window, and the member re-views `buffer[at..end]`
                // rather than copying the bytes out of the buffer.
                let open = format!("field_{}_open", member.number);
                emitter.dec_varint(&content_label, &open, "", "");
                let open_in = format!("field_{}_open_in", member.number);
                let view_slack = format!("field_{}_slack", member.number);
                let view_prep = format!("field_{}_prep", member.number);
                let view_ok = format!("field_{}_view", member.number);
                let view_skip = format!("field_{}_skip", member.number);
                // `open` guards the cursor bound its own subtraction needs —
                // callers (the shared varint states) cannot carry a premise
                // through, so `at <= buffer.len` is tested here and only the
                // true arm does the range arithmetic.
                emitter.state(
                    &open,
                    "raw: u64",
                    "",
                    &format!(
                        "        transition at <= {} {{\n            true -> {open_in}(value, buffer, read, verdict, at, bad, raw)\n            _ -> {view_skip}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n",
                        emitter.limit
                    ),
                );
                emitter.state(
                    &open_in,
                    "raw: u64",
                    &format!("at <= {}", emitter.limit),
                    &format!(
                        "        let room: u64 = buffer.len - at;\n        transition raw <= room && room <= buffer.len {{\n            true -> {view_slack}(value, buffer, read, verdict, at, bad, raw, room)\n            _ -> {view_skip}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
                    ),
                );
                // The window's far edge derives through its complements, one
                // bound per state: `slack` must be a named local before a
                // guard can establish it (arithmetic call arguments do not
                // substitute into a callee's requires), and each derived
                // bound is re-tested before the state that needs it.
                emitter.state(
                    &view_slack,
                    "raw: u64, room: u64",
                    &format!("raw <= room && room <= {}", emitter.limit),
                    &format!(
                        "        let slack: u64 = room - raw;\n        transition slack <= room && slack <= buffer.len && room <= buffer.len {{\n            true -> {view_prep}(value, buffer, read, verdict, at, bad, slack, room)\n            _ -> {view_skip}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
                    ),
                );
                emitter.state(
                    &view_prep,
                    "slack: u64, room: u64",
                    &format!(
                        "slack <= room && slack <= {} && room <= {}",
                        emitter.limit, emitter.limit
                    ),
                    &format!(
                        "        let end: u64 = buffer.len - slack;\n        let start: u64 = buffer.len - room;\n        transition start <= end && end <= {} {{\n            true -> {view_ok}(value, buffer, read, verdict, at, bad, start, end)\n            _ -> {view_skip}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n",
                        emitter.limit
                    ),
                );
                emitter.state(
                    &view_ok,
                    "start: u64, end: u64",
                    &format!("start <= end && end <= {}", emitter.limit),
                    &format!(
                        "        let decoded: &[u8] = buffer[start..end];\n        transition {{ _ -> {view_ok}_validate(value, buffer, read, verdict, end, bad, decoded) }}\n"
                    ),
                );
                // Runtime checks establish exactly the view passed to the
                // store. Invalid bytes leave the destination untouched; the
                // codec contract leaves partial output unspecified on Invalid,
                // but ordinary checked fields must retain their invariants.
                let requires = facts_by_member[member_index]
                    .iter()
                    .map(|fact| format!("({})", fact.instantiate("decoded")))
                    .collect::<Vec<_>>()
                    .join(" && ");
                let write = format!("{view_ok}_write");
                let validation = if requires.is_empty() {
                    format!(
                        "        transition {{ _ -> {write}(value, buffer, read, verdict, at, bad, decoded) }}\n"
                    )
                } else {
                    format!(
                        "        transition {requires} {{\n            true -> {write}(value, buffer, read, verdict, at, bad, decoded)\n            _ -> {after}(value, buffer, read, verdict, at, bad + 1)\n        }}\n"
                    )
                };
                emitter.state(
                    &format!("{view_ok}_validate"),
                    "decoded: &[u8]",
                    "",
                    &validation,
                );
                emitter.state(
                    &write,
                    "decoded: &[u8]",
                    &requires,
                    &format!(
                        "        value.{} = decoded;\n        transition {{ _ -> {after}(value, buffer, read, verdict, at, bad) }}\n",
                        member.name
                    ),
                );
                emitter.state(
                    &view_skip,
                    "end: u64",
                    "",
                    &format!(
                        "        transition {{ _ -> {}(value, buffer, read, verdict, at, bad) }}\n",
                        after
                    ),
                );
            }
            MemberShape::BorrowedSlice { .. } => {}
            MemberShape::FixedScalarArray { element, length } => {
                let open = format!("field_{}_open", member.number);
                emitter.dec_varint(&content_label, &open, "", "");
                let close = format!("field_{}_close", member.number);
                let first = if *length == 0 {
                    close.clone()
                } else {
                    format!("field_{}_element_0", member.number)
                };
                // Open bounds: the length must fit the remaining buffer. On
                // failure `end = 0` stands in for the interpreter's wrapped
                // cursor sum — element reads still run and the close check
                // flags `bad` either way.
                emitter.state(
                    &open,
                    "raw: u64",
                    "",
                    &format!(
                        "        let room: u64 = buffer.len - at;\n        transition raw <= room {{\n            true -> {first}(value, buffer, read, verdict, at, bad, at + raw)\n            _ -> {first}(value, buffer, read, verdict, at, bad + 1, 0)\n        }}\n"
                    ),
                );
                for index in 0..*length {
                    let read = format!("field_{}_element_{index}", member.number);
                    let store = format!("field_{}_element_{index}_store", member.number);
                    emitter.dec_varint(&read, &store, "end: u64", ", end");
                    let next = if index + 1 == *length {
                        close.clone()
                    } else {
                        format!("field_{}_element_{}", member.number, index + 1)
                    };
                    emitter.dec_element_store(
                        &store,
                        &member.name,
                        index,
                        *element,
                        &next,
                        &facts_by_member[member_index],
                    );
                }
                emitter.state(
                    &close,
                    "end: u64",
                    "",
                    "        transition at == end {\n            true -> decode_finish(value, buffer, read, verdict, at, bad)\n            _ -> decode_finish(value, buffer, read, verdict, at, bad + 1)\n        }\n",
                );
            }
        }
    }
    let declarations = emitter.declarations.clone();
    let _ = writeln!(
        emitter.text,
        "    state decode_finish({declarations}) {{\n        read = at;\n        transition bad == 0 {{\n            true -> decode_sound(value, buffer, read, verdict, at, bad)\n            _ -> decode_invalid(value, buffer, read, verdict, at, bad)\n        }}\n    }}\n    state decode_sound({declarations}) {{\n        verdict = {verdict_type}::Sound;\n    }}\n    state decode_invalid({declarations}) {{\n        verdict = {verdict_type}::Invalid;\n    }}\n}}\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated_byte_codec(domain: &str) -> Option<String> {
        let source = format!(
            r#"{domain}
            data Message {{ #0 label: &[u8] in Text; }}
            data Sample {{ label: &[u8] in Text; }}
            data Verdict {{ case Invalid; case Sound; }}
            data Main {{ buffer: [u8; 16]; read: u64; verdict: Verdict; }}
            machine Main::main(main: &mut Main) {{
                let sample: Sample;
                Message::encode(&sample, &mut main.buffer, &mut main.read);
                Message::decode(&mut sample, &main.buffer, &mut main.read, &mut main.verdict);
            }}"#
        );
        generated_codec(&source)
    }

    fn generated_codec(source: &str) -> Option<String> {
        let mut sources = SourceMap::default();
        let source_id = sources.add("wire.omg".into(), source.to_owned()).source_id;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        generated_wire_codec_source(&syntax, &sources)
    }

    #[test]
    fn borrowed_sequences_encode_one_byte_extent_without_an_element_count() {
        for element in ["u8", "u32", "u64", "i32", "i64", "bool"] {
            let generated = generated_codec(&format!(
                "data Message {{ #0 items: &[{element}]; }}
                 data Sample {{ items: &[{element}]; }}
                 data Main {{ buffer: [u8; 64]; written: u64; }}
                 machine Main::main(main: &mut Main) {{
                     let sample: Sample;
                     Message::encode(&sample, &mut main.buffer, &mut main.written);
                 }}"
            ))
            .expect("borrowed sequence encoder");
            let content = generated
                .split("state field_0_content(")
                .nth(1)
                .expect("field content entry")
                .split("\n    state ")
                .next()
                .unwrap();
            if element == "u8" {
                assert!(content.contains("value.items.len"));
                assert!(!generated.contains("_measure("));
            } else {
                assert!(content.contains(&format!("let view: &[{element}] = value.items;")));
                assert!(
                    content
                        .contains("field_0_content_measure(value, out, written, at, 0, 0, view)")
                );
                assert!(!generated.contains("let bits: u64 = value.items.len;"));
                assert!(generated.contains("let bits: u64 = acc as u64;"));
            }
        }
    }

    #[test]
    fn byte_view_decode_guards_the_declared_predicates_before_storage() {
        let generated = generated_byte_codec(
            "domain [u8]::Text(bytes) requires valid_utf8(bytes) && no_nul(bytes);",
        )
        .expect("both ordinary codec bodies");
        assert!(generated.contains("machine Message::encode"));
        assert!(generated.contains("machine Message::decode"));
        assert!(generated.contains("transition (valid_utf8(decoded) && no_nul(decoded))"));
        assert!(
            generated.contains("decoded: &[u8]) requires (valid_utf8(decoded) && no_nul(decoded))")
        );
        assert!(generated.contains("value.label = decoded;"));
        assert!(!generated.contains("value.label = buffer[start..end];"));
        let validation = generated
            .split("state field_0_view_validate")
            .nth(1)
            .unwrap()
            .split("state field_0_view_write")
            .next()
            .unwrap();
        assert!(validation.contains("true -> field_0_view_write"));
        assert!(
            validation.contains("_ -> decode_finish(value, buffer, read, verdict, at, bad + 1)")
        );
        assert!(!validation.contains("value.label ="));
    }

    #[test]
    fn unavailable_decode_predicates_do_not_prevent_encoding() {
        for domain in ["", "domain u32::Text(number) requires number <= 10;"] {
            let generated =
                generated_byte_codec(domain).expect("encoding needs only storage shape");
            assert!(generated.contains("machine Message::encode"));
            assert!(!generated.contains("machine Message::decode"));
        }
    }

    fn predicates(source: &str, domain: &str, operand: &str) -> Vec<String> {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add(std::path::PathBuf::from("wire.omg"), source.to_owned())
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        collect_domains(&syntax, &sources, &HashSet::from([source_id.0]))[domain]
            .iter()
            .map(|fact| fact.predicate.instantiate(operand))
            .collect()
    }

    #[test]
    fn domain_subject_substitution_uses_parsed_value_occurrences() {
        for (fact, expected) in [
            (
                "0 - 10 <= value && value <= 10",
                "0 - 10 <= wide && wide <= 10",
            ),
            (
                "max_value < value_limit + value",
                "max_value < value_limit + wide",
            ),
            (
                "other . /* value λ */ value <= value",
                "other . /* value λ */ value <= wide",
            ),
            (
                "Module::value + value::limit < value",
                "Module::value + value::limit < wide",
            ),
            ("test(value, \"value λ\")", "test(wide, \"value λ\")"),
            (
                "test(Record { value: value })",
                "test(Record { value: wide })",
            ),
            ("value in value", "wide in value"),
            ("(value as value) <= 100", "(wide as value) <= 100"),
        ] {
            let source = format!("domain u32::WireValue(value) requires {fact};");
            assert_eq!(
                predicates(&source, "u32::WireValue", "wide"),
                [expected],
                "{fact}"
            );
        }
    }

    #[test]
    fn domain_subject_can_share_its_carriers_name() {
        assert_eq!(
            predicates(
                "domain u32::WireValue(u32) requires (u32 as u32) <= 100;",
                "u32::WireValue",
                "raw"
            ),
            ["(raw as u32) <= 100"]
        );
        assert_eq!(
            predicates(
                "domain u32::WireValue requires limit <= 10;",
                "u32::WireValue",
                "raw"
            ),
            ["limit <= 10"]
        );
    }
}
