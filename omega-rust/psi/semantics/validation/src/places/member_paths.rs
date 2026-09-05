use super::unwrapped_type_reference;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Resolve the declared type of an already-retained statement receiver path.
/// Statement-position calls preserve name members even when their synthesized
/// accessor target symbols are intentionally absent from authored provenance.
pub(crate) fn declared_member_path_type(
    program: &TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    members: &[String],
) -> Option<TypeReferenceHandle> {
    match members {
        [name] => {
            if let Some(state) = current_state {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let typed_trees::statement::StatementNode::LocalData(local) = statement
                        && local.name.as_str() == name
                    {
                        return local
                            .type_reference
                            .is_valid()
                            .then_some(local.type_reference);
                    }
                }
                for parameter in program.state_parameters(state) {
                    if parameter.name.as_str() == name {
                        return parameter
                            .type_reference
                            .is_valid()
                            .then_some(parameter.type_reference);
                    }
                }
            }
            None
        }
        [root, field_name] if root == "self" => {
            let attached = current_machine.attached_data.as_ref()?;
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.name == *attached)?;
            let field = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == field_name =>
                    {
                        Some(field)
                    }
                    _ => None,
                })?;
            field
                .type_reference
                .is_valid()
                .then_some(field.type_reference)
        }
        [root, field_name] => {
            // A CASE-PAYLOAD field of a local/parameter of sum type (`s.index`
            // where `s: Slot` and `Slot` has a case payload `index`). A
            // destructure arm (`Slot::Found { index } -> ..`) lowers the binding
            // `index` to `s.index`, so resolving it here lets the S4 fact catalog
            // see the payload binding's declared type (its arithmetic carries the
            // decision-17 obligation) AND its range refinement (a `[a..=b]`
            // payload field discharges that obligation). Sound only because
            // construction enforces the field range (see
            // struct_literals::enforce_construction_field_ranges).
            let receiver_type = local_or_parameter_type(program, current_state, root)?;
            let data = data_definition_for_type(program, receiver_type)?;
            data_field_or_payload_type(program, data, field_name)
        }
        // A NESTED place, 3+ members (`self.p.x`, `self.a.inner.x`, or a
        // FIELD-stored enum payload's `self.m.dx` -- a destructure arm lowers
        // the payload binding onto the receiver field's path): walk each hop
        // through its struct/sum definition so the last hop's declared type --
        // domain and range intact -- reaches the decision-17 exact check. This
        // used to return None, which silently EXEMPTED nested-field arithmetic
        // from the overflow obligation (it wrapped).
        path if path.len() >= 3 => {
            resolve_nested_member_path(program, current_machine, current_state, path)
        }
        _ => None,
    }
}

/// Collect a place expression's member path (`self.p.x` -> ["self","p","x"]),
/// descending NESTED `Member` receivers. `None` for non-name shapes (indexed
/// receivers, calls) -- those re-resolve during instruction selection.
pub(super) fn collect_member_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<Vec<String>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => Some(
            program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
        ),
        ExpressionNode::Member(member) => {
            let mut names = collect_member_path(program, member.receiver)?;
            names.push(member.member.as_str().to_owned());
            Some(names)
        }
        _ => None,
    }
}

/// The first member of a data-typed local/parameter path, or a 3+-segment
/// `self` place path, that is MISSING from its
/// resolved containing data definition: `Some((container, member))` for
/// `self.o.inner.nonexistent` (final missing) and `self.o.bogus.value`
/// (intermediate missing) -- both used to compile and silently read a ZII 0.
/// Deliberately conservative -- every unresolvable shape SKIPS (`None`) so only
/// a provably-missing field on a provably-plain container reports:
/// - a non-`self` root that is not a data-typed local/param (machines,
///   schemas, cases) skips;
/// - a `self` path whose FIRST hop names a CONTAINED machine or the machine's
///   OWNED data skips (those live on the machine, not the attached data --
///   `self.counter.value` with `contains counter: Counter` is legal);
/// - a hop through a non-data type (array, slice, primitive, generic
///   parameter) skips -- other checks own those shapes.
pub(crate) fn first_unknown_nested_field(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<(String, String)> {
    let path = collect_member_path(program, expression)?;
    if path.len() < 2 {
        return None;
    }
    let (root, rest) = path.split_first()?;
    // Direct `self.<field>` has a dedicated diagnostic at both read and write
    // sites. Two-segment non-self roots used to skip both that check and this
    // walker, silently accepting `typed_local.missing` as a ZII/default read.
    if root == "self" && path.len() < 3 {
        return None;
    }
    let mut current_data = if root == "self" {
        // `self.<owned>.…` roots on the MACHINE, not its attached data.
        let first_hop = rest.first()?;
        let is_machine_owned_data = program
            .machine_owned_data(current_machine)
            .iter()
            .any(|owned| owned.name.as_str() == *first_hop);
        if is_machine_owned_data {
            return None;
        }
        let attached = current_machine.attached_data.as_ref()?;
        program
            .data_definitions()
            .iter()
            .find(|data| data.name == *attached)?
    } else {
        let receiver_type = local_or_parameter_type(program, current_state, root)?;
        data_definition_for_type(program, receiver_type)?
    };
    for (position, hop) in rest.iter().enumerate() {
        let Some(hop_type) = data_field_or_payload_type(program, current_data, hop) else {
            // Missing from a resolved, plain container: the silent-ZII read.
            return Some((current_data.name.as_str().to_owned(), hop.clone()));
        };
        if position + 1 == rest.len() {
            return None;
        }
        // An intermediate hop through a non-data type (array/primitive/generic):
        // not this check's shape.
        current_data = data_definition_for_type(program, hop_type)?;
    }
    None
}

/// Resolve a 3+-member place by walking each hop through its struct/sum type:
/// the root is `self` (the machine's attached data) or a local/parameter of
/// data type; every intermediate hop must land on a Named data definition; the
/// LAST hop returns the field/payload type reference RAW (constraints intact)
/// so callers read the arithmetic domain (decision 17) and range refinement.
fn resolve_nested_member_path(
    program: &TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    path: &[String],
) -> Option<TypeReferenceHandle> {
    let (root, rest) = path.split_first()?;
    let mut current_data = if root == "self" {
        let attached = current_machine.attached_data.as_ref()?;
        program
            .data_definitions()
            .iter()
            .find(|data| data.name == *attached)?
    } else {
        let receiver_type = local_or_parameter_type(program, current_state, root)?;
        data_definition_for_type(program, receiver_type)?
    };
    let (last, intermediates) = rest.split_last()?;
    for hop in intermediates {
        let hop_type = data_field_or_payload_type(program, current_data, hop)?;
        current_data = data_definition_for_type(program, hop_type)?;
    }
    data_field_or_payload_type(program, current_data, last)
}

/// The declared type NAME behind a `self.a.b` nested member chain -- the
/// value-call receiver resolution (calls.rs) dispatches nested method
/// receivers through it (`self.p.second.stored()` resolves to `second`'s
/// declared type so the attached machine lookup finds the method). `None`
/// when any hop fails to land on a Named data field: those chains keep the
/// existing unresolved-call error.
pub(crate) fn nested_receiver_type_name<'program>(
    program: &'program TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    path: &[String],
) -> Option<&'program str> {
    let type_reference = resolve_nested_member_path(program, current_machine, current_state, path)?;
    let unwrapped = unwrapped_type_reference(program, type_reference)?;
    match program.type_reference_table.type_reference(unwrapped) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// The data/sum definition behind a named or applied-generic type reference
/// (through `&`/`in Domain` shells). Type arguments do not change the declared
/// member names. `None` for primitives, arrays, and unknown names.
pub(crate) fn data_definition_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let unwrapped = unwrapped_type_reference(program, type_reference)?;
    let name = match program.type_reference_table.type_reference(unwrapped) {
        TypeReferenceNode::Named { name, .. } => name,
        TypeReferenceNode::Generic { base_name, .. } => base_name,
        _ => return None,
    };
    program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)
}

/// Resolve a bare local-data or state-parameter name to its declared type.
fn local_or_parameter_type(
    program: &TypedTrees,
    current_state: Option<&typed_trees::state::State>,
    name: &str,
) -> Option<TypeReferenceHandle> {
    let state = current_state?;
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.name.as_str() == name
        {
            return local
                .type_reference
                .is_valid()
                .then_some(local.type_reference);
        }
    }
    for parameter in program.state_parameters(state) {
        if parameter.name.as_str() == name {
            return parameter
                .type_reference
                .is_valid()
                .then_some(parameter.type_reference);
        }
    }
    None
}

/// Find a field named `field_name` on a data/sum definition and return its
/// declared type reference (constraints intact): a plain struct FIELD, or a
/// CASE-VARIANT PAYLOAD field searched across every variant. Lets the S4 fact
/// catalog see a `local.field` / `param.field` read's declared type (so its
/// arithmetic carries the decision-17 obligation) AND range refinement (a
/// `[a..=b]` field discharges it). Sound because every write to such a field is
/// range-enforced: construction (struct_literals::enforce_construction_field_ranges)
/// and assignment (the bounded-assignment obligation on `self.field`/`p.field`).
pub(super) fn data_field_or_payload_type(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    data_field_or_payload(program, data, field_name).map(|field| field.type_reference)
}

pub(super) fn data_field_or_payload<'program>(
    program: &'program TypedTrees,
    data: &'program typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<&'program typed_trees::data::DataField> {
    for member in program.data_members(data) {
        if let typed_trees::data::DataMember::Field(field) = member
            && field.name.as_str() == field_name
            && field.type_reference.is_valid()
        {
            return Some(field);
        }
        if let typed_trees::data::DataMember::Variant(variant) = member {
            for field in program.data_payload_fields(variant) {
                if field.name.as_str() == field_name && field.type_reference.is_valid() {
                    return Some(field);
                }
            }
        }
    }
    None
}
