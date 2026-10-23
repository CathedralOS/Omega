use std::collections::{HashMap, HashSet};

use arena::HandleSpan;
use source::{SourceMap, SourceSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, Machine};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Scalar wire members the compact_binary codecs carry. `u8`/`u16`/`i8`/`i16`
/// are excluded deliberately: the interpreter's byte-size-keyed table reads a
/// one-byte field as boolean-shaped and refuses two-byte fields, so a
/// generated machine for them could not hold the same semantics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarKind {
    U32,
    U64,
    I32,
    I64,
    Bool,
}

impl ScalarKind {
    /// `wire_scalar_in_range`'s declared range as a guard on the raw payload
    /// (unsigned kinds, bool) or on the unzigzagged wide value (i32). u64 and
    /// i64 span their carriers, so no check is emitted.
    pub(super) fn decode_range_guard(self, operand: &str) -> Option<String> {
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
    pub(super) fn name(self) -> &'static str {
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
    pub(super) fn decode_store_expression(self, operand: &str) -> String {
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
pub(super) enum SliceElement {
    Byte,
    Scalar(ScalarKind),
}

#[derive(Clone)]
pub(super) enum MemberShape {
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

pub(super) struct WireMemberShape {
    pub(super) name: String,
    pub(super) number: u64,
    pub(super) shape: MemberShape,
}

/// Canonical unsigned LEB128 bytes (`wire_varint_bytes`).
pub(super) fn varint_bytes(mut value: u64) -> Vec<u8> {
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

/// A domain's parser-bound value occurrences belong to its declared carrier,
/// not the generated decoder's destination record (also named `value`).
pub(super) struct DomainPredicate {
    pub(super) carrier: String,
    pub(super) predicate: StorePredicate,
}

/// Literal source segments between the parser's subject occurrences. Joining
/// them with a decoded operand substitutes only value uses: cast types, domain
/// names, member labels, comments and quoted text retain their authored bytes.
/// The parser already owns binder scope; source synthesis must not rediscover
/// it by matching identifier spellings or rename through an intermediate `self`.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct StorePredicate {
    pub(super) segments: Vec<String>,
}

/// Shape admission is separate from executable validation. Encoding consumes
/// an already-qualified value; only decoding needs to establish its predicates.
pub(super) type StoreFacts = Result<Vec<StorePredicate>, ()>;

impl StorePredicate {
    pub(super) fn from_source(
        sources: &SourceMap,
        span: SourceSpan,
        subjects: &[SourceSpan],
    ) -> Self {
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

    pub(super) fn instantiate(&self, operand: &str) -> String {
        self.segments.join(operand)
    }
}

/// Retain each domain's authored subject occurrences for source generation.
/// Predicate spans restrict substitution to the declaration's lexical scope;
/// synthetic parent-membership subjects have no source to copy.
pub(super) fn collect_domains(
    syntax_trees: &SyntaxTrees,
    sources: &SourceMap,
    product_sources: &HashSet<usize>,
) -> HashMap<String, Vec<DomainPredicate>> {
    let mut names = syntax_trees
        .tables
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = syntax_trees
                .tables
                .expressions
                .identifier_path_members(*path)
            else {
                return None;
            };
            let span = syntax_trees.tables.expressions.source_span(handle);
            (product_sources.contains(&span.source_id.0) && span.span.start < span.span.end)
                .then_some((name.as_str(syntax_trees), span))
        })
        .collect::<Vec<_>>();
    names.sort_unstable_by_key(|(_, span)| (span.source_id.0, span.span.start, span.span.end));
    names.dedup();
    let mut domains: HashMap<String, Vec<DomainPredicate>> = HashMap::new();
    for handle in syntax_trees.root_item_handles() {
        let Some(domain) = syntax_trees.tables.items.item(*handle).as_domain() else {
            continue;
        };
        if !product_sources.contains(&domain.name.source_span().source_id.0) {
            continue;
        }
        let Some(carrier) = type_text(syntax_trees, domain.target_type) else {
            continue;
        };
        let subjects = names
            .iter()
            .filter_map(|(name, span)| {
                domain
                    .subject
                    .as_ref()
                    .is_some_and(|subject| subject.as_str(syntax_trees) == *name)
                    .then_some(*span)
            })
            .collect::<Vec<_>>();
        for fact in domain.facts.handles() {
            if let Some(span) = syntax_trees.tables.items.proof_fact_source_span(fact) {
                domains
                    .entry(domain.name.as_str(syntax_trees).to_owned())
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
pub(super) fn constraint_facts(
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
                let qualified = format!("{base_name}::{}", domain.name.as_str(syntax_trees));
                for key in [domain.name.as_str(syntax_trees), qualified.as_str()] {
                    if let Some(carriers) = domains.get(key) {
                        for predicate in carriers {
                            if predicate.carrier == base_name {
                                if !facts.contains(&predicate.predicate) {
                                    facts.push(predicate.predicate.clone());
                                }
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
pub(super) fn value_field_facts(
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
        TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == expected.name() => {
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
pub(super) fn store_type_facts(
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
/// data-parameter field types at call sites). Declarations carrying case variants are
/// declined — field typing needs the plain record shape.
pub(super) fn collect_plain_fields(
    syntax_trees: &SyntaxTrees,
    data: &syntax_trees::item::DataDefinition,
) -> Option<HashMap<String, TypeReferenceHandle>> {
    let mut fields = HashMap::new();
    for member in syntax_trees.tables.items.data_members(data.members) {
        let DataMember::Field(field) = member else {
            return None;
        };
        fields.insert(
            field.name.as_str(syntax_trees).to_string(),
            field.type_reference,
        );
    }
    Some(fields)
}

/// A wire schema is a plain data declaration whose members all carry `#N`
/// identities and classify to supported wire shapes. Anything richer —
/// variants, retired members, parameters, `where` facts, declared properties,
/// non-scalar field types — declines and keeps meeting the intrinsic wall.
pub(super) fn wire_schema_members(
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
            name: field.name.as_str(syntax_trees).to_string(),
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

/// A constrained member's qualification counts for the wire codec only when
/// every constraint is an argument-free domain — parameterized constraints
/// are beyond the wire surface.
pub(super) fn constraints_are_plain_domains(
    syntax_trees: &SyntaxTrees,
    constraints: HandleSpan<syntax_trees::types::TypeConstraintNode>,
) -> bool {
    syntax_trees
        .tables
        .type_references
        .constraints(constraints)
        .iter()
        .all(|constraint| {
            matches!(
                constraint,
                syntax_trees::types::TypeConstraintNode::Domain(domain)
                    if domain.arguments.is_empty()
            )
        })
}

pub(super) fn member_shape(
    syntax_trees: &SyntaxTrees,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    handle: TypeReferenceHandle,
) -> Option<MemberShape> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => {
            if let Some(kind) = scalar_kind(name.as_str(syntax_trees)) {
                Some(MemberShape::Scalar(kind))
            } else if data_fields.contains_key(name.as_str(syntax_trees)) {
                Some(MemberShape::NestedMessage {
                    schema: name.as_str(syntax_trees).to_string(),
                })
            } else {
                None
            }
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } if base_name.as_str(syntax_trees) == "FixedVec" => {
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
                if !constraints_are_plain_domains(syntax_trees, *constraints) {
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
        } if constraints_are_plain_domains(syntax_trees, *constraints) => {
            member_shape(syntax_trees, data_fields, *base_type)
        }
        _ => None,
    }
}

pub(super) fn slice_element_of_type(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<SliceElement> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == "u8" => {
            Some(SliceElement::Byte)
        }
        TypeReferenceNode::Named(name) => {
            scalar_kind(name.as_str(syntax_trees)).map(SliceElement::Scalar)
        }
        _ => None,
    }
}

pub(super) fn scalar_kind_of_type(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<ScalarKind> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => scalar_kind(name.as_str(syntax_trees)),
        _ => None,
    }
}

/// A const-generic capacity argument's literal value — `4` in
/// `FixedVec<i32, 4>` currently parses as a bare `Named` type reference
/// whose identifier text is the decimal literal (const-generic arguments
/// are not yet elaborated into `ConstExpression` nodes); a
/// `ConstExpression` wrapping an integer literal is also accepted, and any
/// other spelling declines.
pub(super) fn const_literal_capacity(
    syntax_trees: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<usize> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(identifier) => {
            identifier.as_str(syntax_trees).parse::<usize>().ok()
        }
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

pub(super) fn scalar_kind(name: &str) -> Option<ScalarKind> {
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
pub(super) fn collect_call_sites(
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    schema_names: &HashSet<&str>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    sites: &mut HashMap<(String, &'static str), Vec<Vec<String>>>,
) {
    for state_handle in syntax_trees.tables.items.state_handles(machine.states) {
        let state = syntax_trees.tables.items.state(*state_handle);
        let mut bindings = HashMap::new();
        for parameter_handle in syntax_trees.tables.items.state_parameters(state.parameters) {
            let parameter = syntax_trees.tables.items.state_parameter(*parameter_handle);
            bindings.insert(
                parameter.name.as_str(syntax_trees),
                parameter.type_reference,
            );
        }
        for statement_handle in syntax_trees.tables.items.statements(state.statements) {
            if let StatementNode::LocalData(local) =
                syntax_trees.tables.statements.statement(*statement_handle)
            {
                bindings.insert(local.name.as_str(syntax_trees), local.type_reference);
            }
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
            let schema_name = receiver[0].as_str(syntax_trees);
            if !schema_names.contains(schema_name) {
                continue;
            }
            let operation = match call.target.as_str(syntax_trees) {
                "encode" => "encode",
                "decode" => "decode",
                _ => continue,
            };
            // Unknown sites participate in agreement; omitting one could attach
            // a signature inferred from another, unrelated occurrence.
            let mut arguments =
                render_argument_types(syntax_trees, &bindings, data_fields, call.arguments)
                    .unwrap_or_default();
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

/// Render from declarations visible at this occurrence. Whether an operand
/// borrows storage now or transports an existing reference does not change its
/// parameter type. The generated callee still undergoes ordinary checking;
/// declaration types grant neither a loan nor a storage-origin relation.
fn render_argument_types(
    syntax_trees: &SyntaxTrees,
    bindings: &HashMap<&str, TypeReferenceHandle>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    arguments: HandleSpan<ExpressionHandle>,
) -> Option<Vec<String>> {
    syntax_trees
        .tables
        .statements
        .expression_handles(arguments)
        .iter()
        .map(
            |argument| match syntax_trees.tables.expressions.expression(*argument) {
                ExpressionNode::Borrow(borrow) => {
                    let target =
                        expression_type(syntax_trees, bindings, data_fields, borrow.target)?;
                    let access = if borrow.access.is_exclusive() {
                        "&mut "
                    } else {
                        "&"
                    };
                    Some(format!("{access}{}", type_text(syntax_trees, target)?))
                }
                ExpressionNode::Call(call) => exclusive_call_result_type(syntax_trees, call),
                _ => type_text(
                    syntax_trees,
                    expression_type(syntax_trees, bindings, data_fields, *argument)?,
                ),
            },
        )
        .collect()
}

/// Follow declared projections without consulting initial values or guessing
/// storage origins. State-local bindings keep sibling and later declarations
/// from choosing a codec signature for this occurrence.
fn expression_type(
    syntax_trees: &SyntaxTrees,
    bindings: &HashMap<&str, TypeReferenceHandle>,
    data_fields: &HashMap<String, HashMap<String, TypeReferenceHandle>>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match syntax_trees.tables.expressions.expression(expression) {
        ExpressionNode::Name(path) => bindings
            .get(sole_member_name(syntax_trees, *path)?)
            .copied(),
        ExpressionNode::Member(member) => {
            let receiver = expression_type(syntax_trees, bindings, data_fields, member.receiver)?;
            let name = syntax_trees
                .tables
                .type_references
                .type_reference(projection_type(syntax_trees, receiver))
                .as_named()?;
            data_fields
                .get(name.as_str(syntax_trees))?
                .get(member.member.as_str(syntax_trees))
                .copied()
        }
        ExpressionNode::Indexed(indexed) => {
            let collection =
                expression_type(syntax_trees, bindings, data_fields, indexed.collection)?;
            match syntax_trees
                .tables
                .type_references
                .type_reference(projection_type(syntax_trees, collection))
            {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }
        _ => None,
    }
}

fn projection_type(
    syntax_trees: &SyntaxTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    loop {
        reference = match syntax_trees
            .tables
            .type_references
            .type_reference(reference)
        {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return reference,
        };
    }
}

/// The parameter type of an exclusive reference returned by one root machine
/// in this source. A receiver, a static machine argument, a shared result, or
/// two machines with the same name decline: those are not one declared result.
fn exclusive_call_result_type(
    syntax_trees: &SyntaxTrees,
    call: &syntax_trees::expression::TableCallExpression,
) -> Option<String> {
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    let mut rendered = None;
    for handle in syntax_trees.root_item_handles() {
        let Some(candidate) = syntax_trees.tables.items.item(*handle).as_machine() else {
            continue;
        };
        if candidate.name.as_str(syntax_trees) != call.target.as_str(syntax_trees) {
            continue;
        }
        let state_handle = *syntax_trees
            .tables
            .items
            .state_handles(candidate.states)
            .first()?;
        let result = type_text(
            syntax_trees,
            syntax_trees.tables.items.state(state_handle).return_type,
        )?;
        if !result.starts_with("&mut ") || rendered.is_some() {
            return None;
        }
        rendered = Some(result);
    }
    rendered
}

/// `&[u8; N]`/`&mut [u8; N]` -> `&[u8]`/`&mut [u8]`: the buffer parameter's
/// canonical view form. Anything not a fixed byte-array borrow declines so a
/// mis-positioned argument keeps its rendered type and the mismatch surfaces.
pub(super) fn buffer_view(rendered: &str) -> Option<String> {
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

/// The sole member of a single-segment identifier path — `None` for an
/// empty or multi-segment path.
pub(super) fn sole_member_name(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
) -> Option<&str> {
    let members = syntax_trees
        .tables
        .expressions
        .identifier_path_members(path);
    (members.len() == 1).then(|| members[0].as_str(syntax_trees))
}

pub(super) fn type_text(syntax_trees: &SyntaxTrees, handle: TypeReferenceHandle) -> Option<String> {
    match syntax_trees.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str(syntax_trees).to_string()),
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
