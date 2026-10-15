//! Declaration-owned field qualifications consumed by checking and fact flow.
//!
//! `declared_fields` enumerates one case-aware roster for parameter entry,
//! return obligations, and post-call restoration. Conditional payload paths
//! retain exact domain instances; the roster does not establish any value.
//! Owned-result custody deliberately stops at references, while the readable
//! referent roster follows them under their declared access.
//!
//! Assignment lookup and domain identity/implication queries below serve both
//! write enforcement and fact transport. Byte predicates come from declared
//! domain bodies, never recognized domain names. Construction and replacement
//! still prove membership before publishing the corresponding facts.

use symbols::SymbolHandle;
pub use typed_trees::byte_predicates::{ByteSequencePredicate, domain_byte_predicate};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod declared_fields;
pub use declared_fields::{
    FieldDomainRow, WHOLE_ELEMENT_EXTENT, declared_data_field_domain_rows,
    declared_type_domain_rows, exact_declared_field, readable_slice_element,
};

/// The declared type of an assignment destination, preserving domain and
/// capacity constraints. Machine-attached fields and direct state
/// parameter/local places share the same write-establishment rule; only the
/// lookup route differs.
pub fn assignment_target_type_reference(
    program: &dyn crate::CheckingInput,
    _machine: &Machine,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // The common place-type walk retains every field and collection hop,
    // including runtime indices and closed generic substitutions. Assignment
    // destinations are already resolved, so their declared symbol identity is
    // stable throughout this state; no value fact is inferred by this query.
    crate::expression_type_reference_in_state(
        program,
        state.symbol,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        target,
    )
}

/// The predicate-domain refinements declared on an assignment destination.
/// This is the common source for write checking and post-write flow
/// establishment so a mutable parameter cannot regain domain facts unless the
/// assigned value was checked against every predicate declaration in the
/// conjunction.
pub fn assignment_target_domain_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    assignment_target_domain_identities(program, machine, state, target)
        .into_iter()
        .map(|(symbol, _)| symbol)
        .collect()
}

/// [`assignment_target_domain_symbols`] paired with each constraint's interned
/// identity, for producers that seed provable membership facts.
pub fn assignment_target_domain_identities(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    assignment_target_domain_constraints(program, machine, state, target)
        .into_iter()
        .map(|domain| (domain.symbol, domain.semantic_id))
        .collect()
}

/// Retain the complete declared application when proving a write. A symbol
/// alone loses the normalized instance and any selected static arguments.
pub fn assignment_target_domain_constraints<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
) -> Vec<&'program typed_trees::types::DomainConstraint> {
    let Some(type_reference) = assignment_target_type_reference(program, machine, state, target)
    else {
        return Vec::new();
    };
    declared_domain_constraints(program, type_reference)
        .into_iter()
        .filter(|domain| {
            program.domain_definitions().iter().any(|definition| {
                definition.symbol == domain.symbol && definition.predicate_body.is_present()
            })
        })
        .collect()
}

/// Resolve a state place to its exact declared leaf type through the shared
/// field, reference and collection projection. This query describes declared
/// storage and supplies no value-membership evidence.
pub fn direct_state_place_type_reference(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::expression_type_reference_in_state(
        program,
        state.symbol,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        expression,
    )
}

/// Resolve a `self.field` target (`Member(Name(self), field)` or
/// `Name ["self", field]`) to the field's DECLARED type reference (constraints
/// intact) via the machine's attached data. Mirrors proof
/// `obligations::attached_data_field_type` (#63).
pub fn attached_data_field_type(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // A `self.a.b.c` field path -- ONE level (`self.f`) or NESTED. Descend into
    // each intermediate field's data type so a nested domained field's declared
    // type is resolved (its domain is then enforced at writes and trusted at
    // reads, the same two sides one-level fields already have).
    let path = self_field_path(program, expression)?;
    let (last, parents) = path.split_last()?;

    let attached = machine.attached_data.as_ref()?;
    let mut data = program.data_definition_by_name_str(attached.as_str())?;
    for segment in parents {
        let field_type = data_field_type_by_name(program, data, segment)?;
        let next = type_reference_data_name(program, field_type)?;
        data = program.data_definition_by_name_str(next.as_str())?;
    }
    data_field_type_by_name(program, data, last)
}

/// The segments of a `self.a.b.c` field-access path AFTER `self` (so `self.f` is
/// `["f"]`, `self.a.b` is `["a", "b"]`), or `None` if `expression` is not a
/// `self`-rooted field access. Handles both the nested `Member` chain and a flat
/// `Name` path the parser may produce.
fn self_field_path(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let mut path = self_field_path(program, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(name) => {
            match program.expression_table.name_path_members(name.members) {
                [first, rest @ ..] if first.is_self_receiver() => Some(
                    rest.iter()
                        .map(|segment| segment.as_str().to_owned())
                        .collect(),
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The declared type of a constructed field: a case literal's PAYLOAD field
/// (for the named variant) or a record/common struct field.
pub fn construction_field_type(
    program: &dyn crate::CheckingInput,
    data_definition: &typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == case_name =>
                {
                    Some(variant)
                }
                _ => None,
            })
    {
        for payload_field in program.data_payload_fields(variant) {
            if payload_field.name.as_str() == field_name {
                return payload_field
                    .type_reference
                    .is_valid()
                    .then_some(payload_field.type_reference);
            }
        }
    }
    data_field_type_by_name(program, data_definition, field_name)
}

pub fn data_field_type_by_name(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// The field in `data` whose retained `member_symbol` matches when live, else
/// whose `member_name` matches — the member spelling retained references
/// carry. Unlike [`data_field_type_by_name`], the returned handle is not
/// validity-gated.
pub fn data_field_type_by_symbol_or_name(
    program: &dyn crate::CheckingInput,
    data_definition: &typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let field = member.as_field()?;

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}

/// The one data definition a retained type reference names. A live `symbol`
/// is exact identity: a same-named record declared in another module never
/// answers for it, however the definitions are ordered. Only a reference that
/// retained no symbol may stand in by `name`, and only when that name names
/// one definition outright.
pub fn data_definition_by_symbol_or_name<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
    name: &typed_trees::name::Identifier,
) -> Option<&'program typed_trees::data::DataDefinition> {
    if symbol.is_valid() {
        return crate::value_custody::places::unique_data_definition_by_symbol(program, symbol);
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == *name);
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(definition)
}

/// The declared type of `member_symbol`/`member_name` reached through
/// `type_reference`: reference and constraint shells peel, Named and Generic
/// resolve to their data definition through `data_definition` (call sites
/// carry their own symbol/name precedence), and the member lookup is the
/// same symbol-preferred, name-fallback contract as
/// [`data_field_type_by_symbol_or_name`]. Every other node kind carries no
/// named member surface.
pub fn data_field_type_reference_by_symbol_or_name(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
    data_definition: impl for<'program> Fn(
        &'program dyn crate::CheckingInput,
        SymbolHandle,
        &typed_trees::name::Identifier,
    ) -> Option<&'program typed_trees::data::DataDefinition>,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => data_field_type_reference_by_symbol_or_name(
            program,
            *referee,
            member_symbol,
            member_name,
            data_definition,
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => program
            .data_application_definition(type_reference)
            .or_else(|| program.closed_data_definition(type_reference))
            .or_else(|| data_definition(program, *base_symbol, base_name))
            .and_then(|data_definition| {
                data_field_type_by_symbol_or_name(
                    program,
                    data_definition,
                    member_symbol,
                    member_name,
                )
            }),
        TypeReferenceNode::Named { symbol, name } => data_definition(program, *symbol, name)
            .and_then(|data_definition| {
                data_field_type_by_symbol_or_name(
                    program,
                    data_definition,
                    member_symbol,
                    member_name,
                )
            }),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

/// The readable form of a declared type: peel `&`/`&mut` reference shells and
/// domain `Constrained` wrappers. A write-only reference exposes no readable
/// storage and returns `None`; fixed-array elements and nominal fields are
/// resolved against the peeled reference. Callers that need the declared
/// surface (`&[u8] in Utf8`) keep the original handle.
pub fn readable_type_reference(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => reference = *referee,
            TypeReferenceNode::Reference { .. } => return None,
            _ => return Some(reference),
        }
    }
    None
}

/// A readable fixed array's element type and literal length, peeling the
/// `&`/`&mut`/mut-access and domain-constraint shells. Non-literal lengths are
/// lowered before checking; anything unresolved fails closed (`None`).
pub fn readable_fixed_array_elements(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, usize)> {
    let reference = readable_type_reference(program, reference)?;
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Some((*element_type, *length)),
        _ => None,
    }
}

/// The non-generic data definition named by a readable type reference (`T`,
/// `&T`, `&mut T`, `T in D`), or `None` when the reference does not name data
/// storage. Generic substitutions need their own structural evidence rather
/// than a nominal-name guess.
pub fn readable_nominal_definition(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let reference = readable_type_reference(program, reference)?;
    owned_nominal_data_definition(program, reference)
}

/// The non-generic nominal data definition an OWNED value of `type_reference`
/// instantiates. Unlike [`readable_nominal_definition`] this deliberately does
/// NOT peel reference shells: `-> &mut Room` returns a borrow, not owned `Room`
/// storage, so result-field obligations must not be synthesized for it.
pub fn owned_nominal_data_definition(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let symbol = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return owned_nominal_data_definition(program, *base_type);
        }
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if arguments.is_empty() => *base_symbol,
        _ => return None,
    };
    // Resolved `Self` names the attached machine. Its retained data symbol is
    // the nominal storage identity, for both input and result declarations.
    // Normalize that relation here rather than giving receiver obligations a
    // different meaning. An absent attachment never resolves by spelling.
    let symbol = program
        .machine_by_symbol(symbol)
        .map_or(symbol, |machine| machine.attached_data_symbol);
    let first = program.data_definition_by_symbol(symbol)?;
    if first.type_parameters.is_empty() {
        return Some(first);
    }
    // A later concrete declaration may share the symbol with its template.
    program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol && data.type_parameters.is_empty())
}

/// Declared child obligations of result storage or an explicitly selected
/// exclusive referent. Entry and normal return use the same roster: a
/// qualified array element is an obligation just like a qualified data field.
/// A second nominal-only walk would lose element qualifications after calls.
/// Root qualifications have their own contract; fixed-array extent rollups
/// are evidence, not additional obligations. Reference results themselves own
/// no storage, so their callers must explicitly select the readable referent.
pub fn declared_result_field_domain_paths(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<FieldDomainIdentity> {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(type_reference);
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. }
    ) {
        return Vec::new();
    }
    declared_type_domain_rows(program, type_reference)
        .into_iter()
        .filter(|row| !row.transfer && !row.path.is_empty())
        .map(|row| (row.path, row.domain_symbol, row.semantic_domain))
        .collect()
}

/// Exact field path and declared domain instance; an indexed family symbol
/// alone cannot identify the membership restored across a call.
pub type FieldDomainIdentity = (
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
);

type OwnedFieldDomainIdentities = Vec<FieldDomainIdentity>;

/// Lazy owned-result qualification answers for this checking invocation.
/// Queries retain exact type-reference generations; a private mutated
/// projection computes independently rather than borrowing its source's rows.
pub(crate) struct OwnedFieldDomainQueries {
    rows: Vec<(
        TypeReferenceHandle,
        std::sync::OnceLock<OwnedFieldDomainIdentities>,
    )>,
}

impl OwnedFieldDomainQueries {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut rows: Vec<_> = program
            .type_reference_table
            .type_reference_rows()
            .map(|(reference, _)| (reference, std::sync::OnceLock::new()))
            .collect();
        rows.sort_by_key(|(reference, _)| (reference.arena_index(), reference.generation()));
        Self { rows }
    }
}

/// Exact qualifications in owned result storage. Borrowed referents are not
/// owned result claims; predicate readers keep their separate readable walk.
pub fn declared_owned_field_domain_identities(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> OwnedFieldDomainIdentities {
    let compute = || declared_owned_field_domain_identities_uncached(program, reference);
    let Some(memos) = program.checking_memos() else {
        return compute();
    };
    let queries = memos
        .owned_field_domains
        .get_or_init(|| OwnedFieldDomainQueries::of(program));
    let Ok(position) = queries.rows.binary_search_by_key(
        &(reference.arena_index(), reference.generation()),
        |(reference, _)| (reference.arena_index(), reference.generation()),
    ) else {
        return compute();
    };
    queries.rows[position].1.get_or_init(compute).clone()
}

fn declared_owned_field_domain_identities_uncached(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> OwnedFieldDomainIdentities {
    fn visit(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        prefix: &mut Vec<facts::PlaceSegment>,
        ancestors: &mut Vec<SymbolHandle>,
        output: &mut Vec<(
            Vec<facts::PlaceSegment>,
            SymbolHandle,
            language_semantics::SemanticDomainId,
        )>,
    ) {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if matches!(
            program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Reference { .. }
        ) {
            return;
        }
        if !prefix.is_empty() {
            output.extend(
                domain_constraint_identities(program, reference)
                    .into_iter()
                    .map(|(symbol, identity)| (prefix.clone(), symbol, identity)),
            );
        }
        if let Some(data) = owned_nominal_data_definition(program, carrier) {
            if ancestors.contains(&data.symbol) {
                return;
            }
            ancestors.push(data.symbol);
            for field in program.data_member_fields(program.data_members(data)) {
                let length = prefix.len();
                // A payload promise is conditional on its exact case path;
                // it never becomes a sibling alternative's field promise.
                crate::push_field_place_segments(program, prefix, field.symbol);
                visit(program, field.type_reference, prefix, ancestors, output);
                prefix.truncate(length);
            }
            ancestors.pop();
        } else if let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(carrier)
        {
            if *length == 0 {
                return;
            }
            // Every element has the same identities below its index. Visit
            // one element under a placeholder index and repeat the result per
            // index rather than walking each element of a large buffer.
            let mut element = Vec::new();
            let mut element_prefix = vec![facts::PlaceSegment::FixedIndex { index: 0 }];
            visit(
                program,
                *element_type,
                &mut element_prefix,
                ancestors,
                &mut element,
            );
            for index in 0..*length {
                for (path, symbol, identity) in &element {
                    let mut indexed = Vec::with_capacity(prefix.len() + path.len());
                    indexed.extend_from_slice(prefix);
                    indexed.push(facts::PlaceSegment::FixedIndex { index });
                    indexed.extend_from_slice(&path[1..]);
                    output.push((indexed, *symbol, *identity));
                }
            }
        }
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut output,
    );
    output
}

// `domain_requires_provenance` runs per DomainMembership fact per context
// per statement and its alias walk visited every node with a whole-table
// scan. The symbol-to-domain lookup resolves through the index the program
// owns (`TypedTrees::domain_position_by_symbol`): its revision guard
// rebuilds on any mutation of the domain arena, so no per-caller freshness
// key is needed.
fn domain_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::domain::DomainDefinition> {
    typed_trees::domain::domain_by_symbol(program, symbol)
}

/// Transparent aliases retain their constituents' provenance requirement;
/// testing only the alias's own route list would permit predicate-only minting.
pub fn domain_requires_provenance(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> bool {
    fn visit(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
        ancestors: &mut Vec<SymbolHandle>,
    ) -> bool {
        if ancestors.contains(&symbol) {
            return true;
        }
        let Some(domain) = domain_by_symbol(program, symbol) else {
            return true;
        };
        if !domain.establishment_routes.is_empty() {
            return true;
        }
        ancestors.push(symbol);
        let routed = domain.alias.as_ref().is_some_and(|alias| {
            alias
                .constituents
                .iter()
                .any(|part| visit(program, part.domain_symbol, ancestors))
        }) || typed_trees::domain::subject_membership_facts(program, domain)
            .any(|membership| visit(program, membership.domain_symbol, ancestors));
        ancestors.pop();
        routed
    }
    visit(program, symbol, &mut Vec::new())
}

/// Alias atoms must all be predicate- and route-free. This declaration rule
/// governs both retained qualification uses and scalar computation eligibility.
pub fn domain_is_vacuous(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
    stack: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return false;
    };
    if let Some(alias) = domain.alias.as_ref() {
        if alias.constituents.is_empty() {
            return false;
        }
        stack.push(domain_symbol);
        let vacuous = alias
            .constituents
            .iter()
            .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
        stack.pop();
        return vacuous;
    }
    !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
}

/// The data-type name a field's type reference names (peeling `&`/`&mut` and a
/// domain `Constrained` wrapper), for descending a nested field path into the
/// next data definition.
fn type_reference_data_name(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
        TypeReferenceNode::Reference { referee, .. } => type_reference_data_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_data_name(program, *base_type)
        }
        _ => None,
    }
}

/// The exact closed data owner of a field type. Closed generic
/// applications rejoin their retained substituted instance; neither a template
/// nor a same-spelled declaration can supply its fields or layout. Peeling a
/// borrow here only finds its referent: store callers separately reject
/// reference crossings and establish exclusive authority.
pub fn data_definition_for_field_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let type_reference = program
        .type_reference_table
        .peeled_type_reference(type_reference);
    program.closed_data_definition(type_reference)
}

/// The carrier-aware normalized declaration symbols for every declared-domain
/// member of a type-reference conjunction. Arithmetic-policy constraints are a
/// distinct node and are deliberately absent. This never re-resolves a short
/// name globally.
pub fn domain_constraint_symbols(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    domain_constraint_identities(program, type_reference)
        .into_iter()
        .map(|(symbol, _)| symbol)
        .collect()
}

/// Declared domain constraints paired with the identity the typer interned on
/// each constraint. An indexed application (`Resident<P, T>`) is proved only
/// against an exact instance identity (`checks::contracts::prover`), so a
/// membership fact seeded from a declared type must carry that identity; the
/// definition symbol alone proves atomic domains only. For an atomic domain
/// the identity is the definition's own.
pub fn domain_constraint_identities(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    declared_domain_constraints(program, type_reference)
        .into_iter()
        .map(|domain| (domain.symbol, domain.semantic_id))
        .collect()
}

fn declared_domain_constraints(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<&typed_trees::types::DomainConstraint> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            declared_domain_constraints(program, *referee)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => Some(domain),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The predicate-bearing subset of [`domain_constraint_symbols`]. Bodyless
/// domains are binding qualifications whose facts must come from retained
/// establishment evidence, not from predicate proof.
pub fn predicate_domain_constraint_symbols(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    predicate_domain_constraint_identities(program, type_reference)
        .into_iter()
        .map(|(symbol, _)| symbol)
        .collect()
}

/// The predicate-bearing subset of [`domain_constraint_identities`].
pub fn predicate_domain_constraint_identities(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
    domain_constraint_identities(program, type_reference)
        .into_iter()
        .filter(|(symbol, _)| {
            program
                .domain_definition(*symbol)
                .is_some_and(|domain| domain.predicate_body.is_present())
        })
        .collect()
}

/// Whether one declared domain implies another by normalized semantic identity
/// or by an explicit domain-membership chain.
///
/// Capacity-specialized declarations keep distinct carrier-specific symbols so
/// operator lookup can still select the declaration for `[u8; 8]` versus
/// `[u8; 16]`. Their shared `semantic_id`, however, is the proof identity:
/// validation requires repeated declarations with that identity to have equal
/// predicate bodies, semantic roles, and normalized fact sets. Consequently a value established in one
/// carrier specialization satisfies the same semantic domain on another
/// carrier. Comparing only symbols would make the documented
/// `[u8; N]::Utf8` family fracture at every borrow, concat, or call boundary.
pub fn declared_domain_implies(
    program: &dyn crate::CheckingInput,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    typed_trees::domain::declared_domain_implies(program, source_domain, target_domain)
}

/// Membership transports the same recognized byte-content predicate between
/// byte carriers. Ownership, capacity, layout and access still require their
/// independent formation judgments; equal predicates grant none of those.
/// Routed or indexed theories never reduce to this predicate-only relation.
pub fn domain_membership_implies(
    program: &dyn crate::CheckingInput,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    // This legacy relation has no instance arguments. Even an intermediate
    // indexed membership must not be reduced to its family symbol: another
    // application of that family has no implicit variance relationship.
    if !typed_trees::domain::has_symbol_only_proof(program, source_domain)
        || !typed_trees::domain::has_symbol_only_proof(program, target_domain)
    {
        return false;
    }
    if declared_domain_implies(program, source_domain, target_domain) {
        return true;
    }

    let Some(source) = program.domain_definition(source_domain) else {
        return false;
    };
    let Some(target) = program.domain_definition(target_domain) else {
        return false;
    };
    let bytes = |reference| {
        is_fixed_byte_carrier(program, reference) || is_byte_slice_carrier(program, reference)
    };
    if !bytes(source.target_type)
        || !bytes(target.target_type)
        || source.semantic_roles != Default::default()
        || target.semantic_roles != Default::default()
    {
        return false;
    }

    match (
        domain_byte_predicate(program, source_domain),
        domain_byte_predicate(program, target_domain),
    ) {
        (Some(source), Some(target)) => source == target,
        _ => false,
    }
}

fn is_fixed_byte_carrier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => is_fixed_byte_carrier(program, *base_type),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            program.type_reference_table.primitive_type(*element_type)
                == Some(typed_trees::types::PrimitiveType::U8)
        }
        _ => false,
    }
}

fn is_byte_slice_carrier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => is_byte_slice_carrier(program, *base_type),
        TypeReferenceNode::Slice { element_type } => {
            program.type_reference_table.primitive_type(*element_type)
                == Some(typed_trees::types::PrimitiveType::U8)
        }
        _ => false,
    }
}

// --- comptime byte-predicate machinery (moved here from
// `checks::contracts::grants` so the `semantic` fact-producer can reuse it) ---

/// Whether the string literal `expression`'s compile-time bytes satisfy
/// `domain_symbol`'s declared comptime byte-predicate fact. `false` when
/// `expression` is not a string literal, or the domain has no recognized
/// comptime byte-predicate fact.
pub fn string_literal_expression_grants_domain(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let typed_trees::expression::ExpressionNode::String(literal) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    let Some(predicate) = domain_byte_predicate(program, domain_symbol) else {
        return false;
    };
    predicate.holds_for(literal)
}

/// Whether the string literal `expression` may become the stored value of a
/// shared view declared as `type_reference`: every declared domain must be a
/// plain byte-predicate domain -- no establishment route or semantic role,
/// whose membership the octets cannot carry -- that the literal's own bytes
/// grant. A literal view establishes nothing, so the octets are the whole
/// evidence; Terminal replays the same judgment from them.
pub fn string_literal_grants_view_domains(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::String(_)
    ) && domain_constraint_symbols(program, type_reference)
        .into_iter()
        .all(|symbol| {
            domain_byte_predicate(program, symbol).is_some()
                && program.domain_definition(symbol).is_some_and(|domain| {
                    domain.establishment_routes.is_empty()
                        && domain.semantic_roles == Default::default()
                })
                && string_literal_expression_grants_domain(program, expression, symbol)
        })
}

/// Whether the domain's ZERO/ZII value -- for a slice carrier, the EMPTY byte
/// sequence -- provably satisfies `domain_symbol`'s declared facts. True only
/// when its sole fact is a recognized comptime byte predicate that holds for
/// `&[]`. An unrecognized fact set returns `false` (we cannot prove
/// the zero value is in-domain -> conservative).
///
/// This underwrites the machine-field entry-invariant in
/// `semantic::field_domains`: that invariant treats a field's declared domain as
/// ALWAYS-holding at machine entry, which is sound for a read-with-no-prior-write
/// ONLY IF the field's zero value is itself in-domain. Utf8/NoNul/AsciiOnly admit
/// the empty sequence; a `len > 0`-style domain (e.g. `non_empty`) does not, so
/// its entry-invariant must be withheld.
pub fn domain_admits_empty_byte_sequence(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_byte_predicate(program, domain_symbol).is_some_and(|predicate| predicate.holds_for(&[]))
}

/// Whether `domain_symbol`'s sole fact is a recognized comptime byte predicate
/// that is preserved under concatenation, so a `left + right` whose two operands
/// are each in the domain is itself in the domain. Underwrites the concat-domain
/// law in `checks::contracts::writes::value_proves_domain`.
pub fn domain_is_concat_preserving(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_byte_predicate(program, domain_symbol)
        .is_some_and(ByteSequencePredicate::is_concat_preserving)
}

/// Whether `domain_symbol`'s sole fact is a recognized comptime byte predicate
/// preserved under SUBSLICING, so a `base[a..b]` whose `base` is in the domain is
/// itself in the domain. Underwrites the subslice-domain grant in
/// `checks::contracts::calls::subslice_grants_domain`. True only for per-byte
/// facts (`no_nul`, `ascii_only`); `false` for `valid_utf8`/`non_empty`.
pub fn domain_is_subslice_preserving(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_byte_predicate(program, domain_symbol)
        .is_some_and(ByteSequencePredicate::is_subslice_preserving)
}

/// The fixed-array capacity `N` of a `[u8; N]`-shaped owned carrier (peeling a
/// leading domain `Constrained` wrapper), or `None` for a type with no inline
/// capacity such as a `&[u8]` view. The fixed-array length is a `Literal` by the
/// time checking runs (the orchestration const-eval pass lowers `ConstParameter`
/// / `ConstCall` lengths first), so an unresolved length conservatively yields
/// `None`. Used by the length-fits check to bound writes into a bounded text
/// carrier.
pub fn type_reference_fixed_array_capacity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_fixed_array_capacity(program, *base_type)
        }
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(capacity),
            ..
        } => Some(*capacity),
        _ => None,
    }
}
