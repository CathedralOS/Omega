//! Shared resolution of an encoding-DOMAIN refinement declared on a
//! machine-attached-data field (`out: &[u8] in Utf8`), used by both the
//! write-enforcement check (`checks::contracts::writes`) and the flow-stage
//! re-establishment of the field-domain invariant after a write
//! (`flow::transfers`). #66.
//!
//! This neutral crate-root module also owns the comptime byte-predicate
//! machinery (`ByteSequencePredicate`, `domain_byte_predicate`,
//! `string_literal_expression_grants_domain`) so it is reachable from BOTH the
//! checker (`checks/`, construction-grant discharge) and the fact-producer
//! (`semantic/`). The policy of which bytes are in a domain lives in the DOMAIN
//! declaration's body facts; this module only provides the reusable comptime
//! byte-predicate primitives and evaluates them per-literal. A domain without
//! exactly one recognized comptime byte-predicate fact grants nothing. There is NO
//! hardcoded domain name here.

use psi_symbols::SymbolHandle;
pub(crate) use psi_typed_trees::byte_predicates::{ByteSequencePredicate, domain_byte_predicate};
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// The declared type of an assignment destination, preserving domain and
/// capacity constraints. Machine-attached fields and direct state
/// parameter/local places share the same write-establishment rule; only the
/// lookup route differs.
pub(crate) fn assignment_target_type_reference(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    target: psi_typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    attached_data_field_type(program, machine, target)
        .or_else(|| direct_state_place_type_reference(program, state, target))
}

/// The predicate-domain refinements declared on an assignment destination.
/// This is the common source for write checking and post-write flow
/// establishment so a mutable parameter cannot regain domain facts unless the
/// assigned value was checked against every predicate declaration in the
/// conjunction.
pub(crate) fn assignment_target_domain_symbols(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    target: psi_typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    let Some(type_reference) = assignment_target_type_reference(program, machine, state, target)
    else {
        return Vec::new();
    };
    predicate_domain_constraint_symbols(program, type_reference)
}

/// Resolve a state parameter/local target, including nested data members, to
/// its declared leaf type. This is the non-`self` sibling of
/// [`attached_data_field_type`]: the root type comes from the parameter/local,
/// then each member descends through the ordinary data declaration.
pub(crate) fn direct_state_place_type_reference(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let (symbol, members) = state_place_path(program, state, expression)?;
    let mut type_reference = if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        parameter
            .type_reference
            .is_valid()
            .then_some(parameter.type_reference)?
    } else {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                psi_typed_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == symbol && local.type_reference.is_valid() =>
                {
                    Some(local.type_reference)
                }
                _ => None,
            })?
    };

    for member in members {
        let data = data_definition_for_field_type(program, type_reference)?;
        type_reference = data_field_type_by_name(program, data, &member)?;
    }
    Some(type_reference)
}

fn state_place_path(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, Vec<String>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => state_place_path(program, state, inner.target),
        ExpressionNode::Member(member) => {
            let (symbol, mut members) = state_place_path(program, state, member.receiver)?;
            members.push(member.member.as_str().to_owned());
            Some((symbol, members))
        }
        ExpressionNode::Name(path) => {
            let names = program.expression_table.name_path_members(path.members);
            let symbol =
                path.head_symbol
                    .is_valid()
                    .then_some(path.head_symbol)
                    .or_else(|| path.symbol.is_valid().then_some(path.symbol))
                    .or_else(|| {
                        let root_name = names.first()?.as_str();
                        program
                            .state_parameters(state)
                            .iter()
                            .find(|parameter| parameter.name.as_str() == root_name)
                            .map(|parameter| parameter.symbol)
                            .or_else(|| {
                                program
                                    .statement_table
                                    .statements(state.statement_nodes)
                                    .iter()
                                    .find_map(|statement| match statement {
                                        psi_typed_trees::statement::StatementNode::LocalData(
                                            local,
                                        ) if local.name.as_str() == root_name => Some(local.symbol),
                                        _ => None,
                                    })
                            })
                    })?;
            Some((
                symbol,
                names
                    .iter()
                    .skip(1)
                    .map(|name| name.as_str().to_owned())
                    .collect(),
            ))
        }
        _ => None,
    }
}

/// The machine whose attached data owns the place a `self.field` target refers to.
pub(crate) fn machine_by_symbol(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<&Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

/// Resolve a `self.field` target (`Member(Name(self), field)` or
/// `Name ["self", field]`) to the field's DECLARED type reference (constraints
/// intact) via the machine's attached data. Mirrors psi-proof
/// `obligations::attached_data_field_type` (#63).
pub(crate) fn attached_data_field_type(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // A `self.a.b.c` field path -- ONE level (`self.f`) or NESTED. Descend into
    // each intermediate field's data type so a nested domained field's declared
    // type is resolved (its domain is then enforced at writes and trusted at
    // reads, the same two sides one-level fields already have).
    let path = self_field_path(program, expression)?;
    let (last, parents) = path.split_last()?;

    let attached = machine.attached_data.as_ref()?;
    let mut data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    for segment in parents {
        let field_type = data_field_type_by_name(program, data, segment)?;
        let next = type_reference_data_name(program, field_type)?;
        data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == next.as_str())?;
    }
    data_field_type_by_name(program, data, last)
}

/// The segments of a `self.a.b.c` field-access path AFTER `self` (so `self.f` is
/// `["f"]`, `self.a.b` is `["a", "b"]`), or `None` if `expression` is not a
/// `self`-rooted field access. Handles both the nested `Member` chain and a flat
/// `Name` path the parser may produce.
fn self_field_path(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let mut path = self_field_path(program, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(name) => {
            match program.expression_table.name_path_members(name.members) {
                [first, rest @ ..] if first.as_str() == "self" => Some(
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

fn data_field_type_by_name(
    program: &psi_typed_trees::TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == field_name =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// The data-type name a field's type reference names (peeling `&`/`&mut` and a
/// domain `Constrained` wrapper), for descending a nested field path into the
/// next data definition.
fn type_reference_data_name(
    program: &psi_typed_trees::TypedTrees,
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

/// The data definition a field's type names (a struct-typed field, peeling
/// `&`/`&mut` and a domain wrapper), or `None` if the field is not data-typed.
/// Used to descend a nested field path for the entry-invariant seed.
pub(crate) fn data_definition_for_field_type(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    let name = type_reference_data_name(program, type_reference)?;
    program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name.as_str())
}

/// The carrier-aware normalized declaration symbols for every declared-domain
/// member of a type-reference conjunction. Arithmetic-policy constraints are a
/// distinct node and are deliberately absent. This never re-resolves a short
/// name globally.
pub(crate) fn domain_constraint_symbols(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            domain_constraint_symbols(program, *referee)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                    Some(domain.symbol)
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The predicate-bearing subset of [`domain_constraint_symbols`]. Bodyless
/// domains are binding qualifications whose facts must come from retained
/// establishment evidence, not from predicate proof.
pub(crate) fn predicate_domain_constraint_symbols(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    domain_constraint_symbols(program, type_reference)
        .into_iter()
        .filter(|symbol| {
            program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == *symbol)
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
pub(crate) fn declared_domain_implies(
    program: &psi_typed_trees::TypedTrees,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    psi_typed_trees::domain::declared_domain_implies(program, source_domain, target_domain)
}

/// Whether an established membership in `source_domain` proves membership in
/// `target_domain`, including the one representation-changing projection that
/// preserves a byte sequence exactly: an owned bounded `[u8; N]` carrier viewed
/// as `[u8]`.
///
/// Declared domains remain storage-bound, so the fixed carrier and slice view
/// intentionally have distinct semantic identities. The language nevertheless
/// specifies that projecting bounded text to a view carries its domain. Admit
/// that implication only when both declarations consist of the same single
/// compiler-recognized byte predicate. This proves that the live bytes exposed
/// by the view satisfy precisely the required theory without conflating the two
/// carrier identities or blessing unrelated cross-carrier recasts.
pub(crate) fn domain_membership_implies(
    program: &psi_typed_trees::TypedTrees,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    if declared_domain_implies(program, source_domain, target_domain) {
        return true;
    }

    let Some(source) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == source_domain)
    else {
        return false;
    };
    let Some(target) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == target_domain)
    else {
        return false;
    };
    if !is_fixed_byte_carrier(program, source.target_type)
        || !is_byte_slice_carrier(program, target.target_type)
        || source.name.as_str().rsplit("::").next() != target.name.as_str().rsplit("::").next()
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
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => is_fixed_byte_carrier(program, *base_type),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            program.type_reference_table.primitive_type(*element_type)
                == Some(psi_typed_trees::types::PrimitiveType::U8)
        }
        _ => false,
    }
}

fn is_byte_slice_carrier(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => is_byte_slice_carrier(program, *base_type),
        TypeReferenceNode::Slice { element_type } => {
            program.type_reference_table.primitive_type(*element_type)
                == Some(psi_typed_trees::types::PrimitiveType::U8)
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
pub(crate) fn string_literal_expression_grants_domain(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let psi_typed_trees::expression::ExpressionNode::String(literal) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    let Some(predicate) = domain_byte_predicate(program, domain_symbol) else {
        return false;
    };
    predicate.holds_for(literal)
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
pub(crate) fn domain_admits_empty_byte_sequence(
    program: &psi_typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_byte_predicate(program, domain_symbol).is_some_and(|predicate| predicate.holds_for(&[]))
}

/// Whether `domain_symbol`'s sole fact is a recognized comptime byte predicate
/// that is preserved under concatenation, so a `left + right` whose two operands
/// are each in the domain is itself in the domain. Underwrites the concat-domain
/// law in `checks::contracts::writes::value_proves_domain`.
pub(crate) fn domain_is_concat_preserving(
    program: &psi_typed_trees::TypedTrees,
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
pub(crate) fn domain_is_subslice_preserving(
    program: &psi_typed_trees::TypedTrees,
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
pub(crate) fn type_reference_fixed_array_capacity(
    program: &psi_typed_trees::TypedTrees,
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
