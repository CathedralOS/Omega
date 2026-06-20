use crate::locals::WritableRoots;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_assignment_target_handle(
    program: &TypedTrees,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine_name: &str,
    state_name: &str,
) {
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment target must be a named place"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` because it is not mutable in this state"
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
pub(crate) fn declared_place_type_raw(
    program: &TypedTrees,
    current_machine: &omega_typed_trees::machine::Machine,
    current_state: Option<&omega_typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    if let ExpressionNode::Mutable(inner) = program.expression_table.expression(handle) {
        handle = *inner;
    }

    let members: Vec<String> = match program.expression_table.expression(handle) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect(),
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(receiver) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let mut names: Vec<String> = program
                .expression_table
                .name_path_members(receiver.members)
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect();
            names.push(member.member.as_str().to_owned());
            names
        }
        _ => return None,
    };

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
            case_payload_field_type(program, data, field_name)
        }
        _ => None,
    }
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

/// Find a CASE-VARIANT PAYLOAD field named `field_name` on a sum definition and
/// return its declared type reference (constraints intact), searched across
/// every variant. Scoped to payload fields (stage 1 of the fact catalog over sum
/// cases); a plain struct field of a local/parameter is a separate obligation
/// hole tracked elsewhere.
fn case_payload_field_type(
    program: &TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    for member in program.data_members(data) {
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
pub(crate) fn unwrapped_type_reference(
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
