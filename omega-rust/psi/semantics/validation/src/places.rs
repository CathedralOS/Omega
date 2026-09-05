use crate::locals::WritableRoots;
use crate::struct_literals::data_declares_field;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_assignment_target_handle(
    program: &TypedTrees,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
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

    // The data-typed local/parameter and nested-target twin of the direct check
    // (same walker as the READ side):
    // `self.o.inner.nonexistent = 7` / `self.o.bogus.value = 7` used to fall
    // through to the backend's "needs runtime storage write lowering" blocker --
    // loud but MISLEADING (it reads as a missing lowering, not a typo). Report
    // the missing member on its resolved container instead. The walker's skips
    // (contained-machine/owned-data roots and non-data hops)
    // keep every legal write untouched.
    if let Some((container, member)) =
        first_unknown_nested_field(program, machine, current_state, target)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment: data `{container}` has \
             no field `{member}` (check the spelling of the field name)"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    // BARE reassignment of a whole local (`x = 2`) is gated on `let mut`;
    // MEMBER/INDEX writes (`q.x = 3`, `buf[i] = b`) are the ZII
    // construction-by-fill idiom and stay ungated (the fill targets
    // interior storage, and the divergence this gate closes -- the stale
    // initializer fold -- only fires on whole-local rebinding).
    let target_is_bare_name = matches!(
        program.expression_table.expression(target),
        ExpressionNode::Name(_)
    );

    if !writable_roots.contains_for_write(root_name, target_is_bare_name) {
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
pub(crate) fn direct_self_field_member(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Option<&str> {
    match program.expression_table.expression(target) {
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let receiver = program.expression_table.name_path_members(path.members);
            (receiver.len() == 1 && receiver[0].as_str() == "self").then(|| member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 2 && members[0].as_str() == "self").then(|| members[1].as_str())
        }
        _ => None,
    }
}

/// The machine's attached-data `DataDefinition`, resolved by name. `None` for a
/// machine with no attached data (a free machine) or an unresolvable data name.
pub(crate) fn machine_attached_data<'a>(
    program: &'a TypedTrees,
    machine: &Machine,
) -> Option<&'a DataDefinition> {
    let attached = machine.attached_data.as_ref()?;
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
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let raw = declared_place_type_raw(program, current_machine, current_state, argument)?;
    unwrapped_type_reference(program, raw)
}

pub(crate) fn assignment_value_type(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
) -> TypeReferenceHandle {
    // Assignment through a reference writes its referent, not the reference
    // carrier. Preserve the referent's constraints and arithmetic policy.
    match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => destination,
    }
}

/// Like [`declared_place_type`] but returns the place's type reference WITHOUT
/// unwrapping the `Constrained`/`Reference` shells -- callers that need the
/// arithmetic domain (decision 17) read it from this raw handle.
pub fn declared_place_type_raw(
    program: &TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    if let ExpressionNode::Borrow(inner) = program.expression_table.expression(handle) {
        handle = inner.target;
    }

    if let Some(members) = collect_member_path(program, handle) {
        return declared_member_path_type(program, current_machine, current_state, &members);
    }
    match program.expression_table.expression(handle) {
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call),
        ExpressionNode::Member(member) => {
            let receiver =
                declared_place_type_raw(program, current_machine, current_state, member.receiver)?;
            let data = data_definition_for_type(program, receiver)?;
            data_field_or_payload_type(program, data, member.member.as_str())
        }
        ExpressionNode::Indexed(_) => {
            declared_indexed_projection_type_raw(program, current_machine, current_state, handle)
        }
        _ => None,
    }
}

/// Resolve the exact declaration symbol at the leaf of a retained named/member
/// place. Expression member symbols can be synthesized accessor identities;
/// semantic facts that name storage must retain the field/local declaration
/// reached by the receiver's declared type instead.
pub(crate) fn declared_place_leaf_symbol(
    program: &TypedTrees,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    statement_index: usize,
    argument: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let mut handle = argument;
    if let ExpressionNode::Borrow(inner) = program.expression_table.expression(handle) {
        handle = inner.target;
    }
    let members = collect_member_path(program, handle)?;
    let (root, rest) = members.split_first()?;
    if rest.is_empty() {
        return lexical_place_declaration_before(program, current_state?, statement_index, root)
            .map(|(symbol, _)| symbol);
    }

    let mut current_data = if root == "self" {
        let first = rest.first()?;
        if let Some(owned) = program
            .machine_owned_data(current_machine)
            .iter()
            .find(|owned| owned.name.as_str() == first)
        {
            if rest.len() == 1 {
                return Some(owned.symbol);
            }
            data_definition_for_type(program, owned.type_reference)?
        } else {
            let attached = current_machine.attached_data.as_ref()?;
            program
                .data_definitions()
                .iter()
                .find(|data| data.name == *attached)?
        }
    } else {
        let (_, receiver_type) =
            lexical_place_declaration_before(program, current_state?, statement_index, root)?;
        data_definition_for_type(program, receiver_type)?
    };

    for (index, member_name) in rest.iter().enumerate() {
        let field = data_field_or_payload(program, current_data, member_name)?;
        if index + 1 == rest.len() {
            return Some(field.symbol);
        }
        current_data = data_definition_for_type(program, field.type_reference)?;
    }
    None
}

fn lexical_place_declaration_before(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    name: &str,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let mut matches = Vec::new();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
    {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.name.as_str() == name
        {
            matches.push((local.symbol, local.type_reference));
        }
    }
    for parameter in program.state_parameters(state) {
        if parameter.name.as_str() == name {
            matches.push((parameter.symbol, parameter.type_reference));
        }
    }
    match matches.as_slice() {
        [(symbol, type_reference)] if symbol.is_valid() && type_reference.is_valid() => {
            Some((*symbol, *type_reference))
        }
        _ => None,
    }
}

mod member_paths;
mod result_shape;
use member_paths::{collect_member_path, data_field_or_payload, data_field_or_payload_type};
pub(crate) use member_paths::{
    data_definition_for_type, declared_member_path_type, first_unknown_nested_field,
    nested_receiver_type_name,
};
pub(crate) use result_shape::expression_result_is_reference;

/// Unwrap reference and constraint shells so the structural type underneath
/// (`[u8; N]`, `usize`, a data name) is inspectable.
/// The declared leaf type of an indexed assignment target (`self.xs[i]`,
/// `buf[k]`, or `self.rows[i].field`), unwrapped like [`declared_place_type`].
/// This is also the indexed branch of `declared_place_type`: it resolves the
/// collection's `[T; N]` / `[T]` element and then walks any projected fields so
/// cross-class, narrowing, and nominal store checks see the destination type.
/// `None` for a non-indexed target, an unresolvable collection, or a collection
/// that is not an array/slice.
pub(crate) fn declared_indexed_projection_type(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    declared_indexed_projection_type_raw(program, current_machine, current_state, target)
        .and_then(|element_type| unwrapped_type_reference(program, element_type))
}

/// [`declared_indexed_projection_type`] WITHOUT the final unwrap: the element
/// or projected field's declared type with its constraint shells intact, so a
/// range-refined leaf (`[i32 [0..=7]; N]` or `rows[i].count: i32 [0..=7]`)
/// keeps its range for operand analysis and arithmetic-domain resolution.
pub(crate) fn declared_indexed_projection_type_raw(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = target;
    let mut projected_fields = Vec::new();
    loop {
        match program.expression_table.expression(handle) {
            ExpressionNode::Borrow(inner) => handle = inner.target,
            ExpressionNode::Member(member) => {
                projected_fields.push(member.member.as_str());
                handle = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                // A range denotes a window, not one element. Its destination
                // shape is checked by the range-assignment rule that knows the
                // normalized width; treating it as the element type turns
                // `[u8; M]` replacement into a spurious array-to-`u8` store.
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                let collection_type = declared_place_type(
                    program,
                    current_machine,
                    current_state,
                    indexed.collection,
                )?;
                let mut leaf = match program.type_reference_table.type_reference(collection_type) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => return None,
                };
                for field_name in projected_fields.iter().rev() {
                    let data = data_definition_for_type(program, leaf)?;
                    leaf = data_field_or_payload_type(program, data, field_name)?;
                }
                return Some(leaf);
            }
            _ => return None,
        }
    }
}

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
