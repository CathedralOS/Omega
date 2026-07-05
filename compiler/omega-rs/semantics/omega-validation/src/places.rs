use crate::locals::WritableRoots;
use crate::struct_literals::data_declares_field;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataDefinition;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_assignment_target_handle(
    program: &TypedTrees,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine: &Machine,
    state_name: &str,
) {
    let machine_name = machine.name.as_str();
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment target must be a named place"
        )));
        return;
    }

    // A direct `self.<field>` target must name an actual field of the machine's
    // attached data. An unknown field (a typo) gets a clear "no field" error instead
    // of falling through to the "not mutable" message, which cannot tell a
    // nonexistent field from a real-but-immutable one. Scoped to the DIRECT
    // `self.<field>` shape (checked against the top-level data fields, which is
    // exactly the set writable via `self.<field>`); nested `self.a.b` and bare locals
    // are left to the existing writable-roots check.
    if let Some(field_name) = direct_self_field_member(program, target)
        && let Some(data) = machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment: data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            data.name.as_str()
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        // The writable set cannot distinguish a nonexistent root (a typo) from a real
        // but non-mutable one, so append a conditional typo hint -- correct whatever
        // the cause. (A full "data X has no field Y" check is the separate
        // unknown-field validation-gap TASK.)
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` \
             because it is not mutable in this state (if `{root_name}` is undeclared -- a typo -- \
             no such field or local exists)"
        )));
    }
}

fn is_mutable_place_handle(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => is_mutable_place_handle(program, indexed.collection),
        ExpressionNode::Member(member) => is_mutable_place_handle(program, member.receiver),
        ExpressionNode::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name_handle(program: &TypedTrees, expression: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_name_handle(program, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && program
                            .expression_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self") =>
                {
                    Some(member.member.as_str())
                }
                _ => expression_root_name_handle(program, member.receiver),
            }
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|name| name.as_str()),
        _ => None,
    }
}

/// The field name of a DIRECT `self.<field>` place, whether it lowered as a
/// `Member(Name([self]), field)` or a two-segment `Name([self, field])` path.
/// `None` for anything deeper (`self.a.b`), a bare local, or a non-`self` receiver.
fn direct_self_field_member(program: &TypedTrees, target: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(target) {
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(path) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let receiver = program.expression_table.name_path_members(path.members);
            (receiver.len() == 1 && receiver[0].as_str() == "self")
                .then(|| member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 2 && members[0].as_str() == "self")
                .then(|| members[1].as_str())
        }
        _ => None,
    }
}

/// The machine's attached-data `DataDefinition`, resolved by name. `None` for a
/// machine with no attached data (a free machine) or an unresolvable data name.
fn machine_attached_data<'a>(
    program: &'a TypedTrees,
    machine: &Machine,
) -> Option<&'a DataDefinition> {
    let attached = machine.attached_data.as_ref()?;
    // VERSIONED data (`Counter::v1`) has version-specific fields that a naive
    // top-level field list does not capture (a cross-version field like `timestamp`
    // is legally reachable but not in this version's `data_members`). Skip the
    // unknown-field check for it -- leave those to the version-access / writable-roots
    // diagnostics -- rather than mis-report "no field".
    if attached
        .as_str()
        .rsplit("::")
        .next()
        .is_some_and(omega_core::versioning::is_version_selector)
    {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == attached.as_str())
}

/// The DECLARED type of a simple place argument: a bare local/parameter name
/// (`msg`) or an attached-data field (`self.buffer`), through the `&mut`
/// marker. `None` for shapes this scope cannot type (those re-resolve during
/// instruction selection). Shared by the wire-call argument checks and the
/// machine-call type-parameter bound check (frozen decision 13).
pub(crate) fn declared_place_type(
    program: &TypedTrees,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let raw = declared_place_type_raw(program, current_machine, current_state, argument)?;
    unwrapped_type_reference(program, raw)
}

/// Like [`declared_place_type`] but returns the place's type reference WITHOUT
/// unwrapping the `Constrained`/`Reference` shells -- callers that need the
/// arithmetic domain (decision 17) read it from this raw handle.
pub fn declared_place_type_raw(
    program: &TypedTrees,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    if let ExpressionNode::Mutable(inner) = program.expression_table.expression(handle) {
        handle = *inner;
    }

    let members: Vec<String> = collect_member_path(program, handle)?;

    match members.as_slice() {
        [name] => {
            if let Some(state) = current_state {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let omega_typed_trees::statement::StatementNode::LocalData(local) = statement
                        && local.name.as_str() == name
                    {
                        return local.type_reference.is_valid().then_some(local.type_reference);
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
                    omega_typed_trees::data::DataMember::Field(field)
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
            let unwrapped = unwrapped_type_reference(program, receiver_type)?;
            let TypeReferenceNode::Named { name, .. } =
                program.type_reference_table.type_reference(unwrapped)
            else {
                return None;
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.name == *name)?;
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
fn collect_member_path(
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

/// Resolve a 3+-member place by walking each hop through its struct/sum type:
/// the root is `self` (the machine's attached data) or a local/parameter of
/// data type; every intermediate hop must land on a Named data definition; the
/// LAST hop returns the field/payload type reference RAW (constraints intact)
/// so callers read the arithmetic domain (decision 17) and range refinement.
fn resolve_nested_member_path(
    program: &TypedTrees,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
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

/// The data/sum definition behind a type reference (through `&`/`in Domain`
/// shells). `None` for primitives, arrays, and unknown names.
fn data_definition_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&omega_typed_trees::data::DataDefinition> {
    let unwrapped = unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(unwrapped)
    else {
        return None;
    };
    program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)
}

/// Resolve a bare local-data or state-parameter name to its declared type.
fn local_or_parameter_type(
    program: &TypedTrees,
    current_state: Option<&omega_typed_trees::state::State>,
    name: &str,
) -> Option<TypeReferenceHandle> {
    let state = current_state?;
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let omega_typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.name.as_str() == name
        {
            return local.type_reference.is_valid().then_some(local.type_reference);
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
fn data_field_or_payload_type(
    program: &TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    for member in program.data_members(data) {
        if let omega_typed_trees::data::DataMember::Field(field) = member
            && field.name.as_str() == field_name
        {
            return field.type_reference.is_valid().then_some(field.type_reference);
        }
        if let omega_typed_trees::data::DataMember::Variant(variant) = member {
            for field in program.data_payload_fields(variant) {
                if field.name.as_str() == field_name {
                    return field.type_reference.is_valid().then_some(field.type_reference);
                }
            }
        }
    }
    None
}

/// Unwrap reference and constraint shells so the structural type underneath
/// (`[u8; N]`, `usize`, a data name) is inspectable.
pub fn unwrapped_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => unwrapped_type_reference(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            unwrapped_type_reference(program, *base_type)
        }
        _ => Some(type_reference),
    }
}
