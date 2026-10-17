//! Places: what an assignment target, argument or projection names as
//! storage, and the type its declaration gives that storage.
//!
//! `validate_assignment_target_handle` is the one check here. The
//! assignment-statement validator in `program_validation::statements` runs it
//! on each assignment target, and it reports the first of these that fails:
//! the target must be a named place, a direct `self.<field>` must name a field
//! of the attached data, a nested member path must name declared fields
//! (`member_paths::first_unknown_nested_field`), and the root must be writable
//! in this state, or, for a shared-receiver atomic operation, reachable.
//!
//! The rest of the module answers place queries that validators, checking and
//! lowering read. `declared_place_type_raw` is the main one. After looking
//! through a borrow it tries, in order: a machine's const or value parameter,
//! a bare attached field, a case projection (`projected_members`), a `self`
//! field (`exact_self_field`), a member expression through its receiver's
//! declared type, a name path (`member_paths`), a constant array literal, a
//! call result, and an indexed element (`declared_indexed_projection_type_raw`).
//! It keeps reference and constraint shells; `declared_place_type` and
//! `unwrapped_type_reference` remove them. The remaining children are queries
//! of their own: `attached_storage` (the binding a declaration reaches the
//! attached data through, receiver or data parameter, and the field query
//! built on it), `builtin_coordinates` (whether length, index and subslice
//! selectors have their builtin meaning), `structural_scalar_field` (a
//! scalar read of a field of a local record), `parameter_scoped` (types rooted
//! in an abstract signature's parameters) and `result_shape` (whether an
//! expression's result is a reference). `owner_tests` is test-only.

pub(crate) mod binding_observations;
pub mod collection_operations;
mod declaration_types;
pub use declaration_types::declaration_type_reference;

use crate::value_custody::locals::WritableRoots;
use crate::value_custody::struct_literals::data_declares_field;
use access_plans::BorrowPolarity;
use diagnostics::Diagnostic;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// An incoming exclusive reference parameter lends its referent; it is not an
/// owned descriptor slot. A mutable shared-reference binding may instead
/// replace its descriptor without writing the shared referent. Receiving types,
/// origin stability and write permission must retain that same distinction,
/// including when an exclusive referent is itself a reference-valued slot.
pub(crate) fn assignment_targets_parameter_referent(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    target: ExpressionHandle,
) -> bool {
    crate::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        state.statement_nodes.count() as usize,
        target,
    )
    .is_some_and(|place| {
        place.segments.is_empty()
            && place
                .root
                .as_symbol()
                .and_then(|symbol| program.state_parameter_by_symbol(state, symbol))
                .is_some_and(|parameter| {
                    crate::reference_shell(program, parameter.type_reference)
                        .is_some_and(|shell| shell.access.is_exclusive())
                })
    })
}

/// Check that an assignment's target names a place this state may reach with
/// the polarity `receiver` demands: `Exclusive` for an ordinary write, or
/// `Shared` when the assignment is the parser's carrier for a sealed atomic
/// operation on atomic storage (see `value_custody::atomic_operations`). The
/// place-shape and unknown-field checks are the same for both; only the
/// final root demand differs.
pub(crate) fn validate_assignment_target_handle(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    state_name: &str,
    receiver: BorrowPolarity,
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

    // A shared-receiver operation (an atomic store/fetch/swap/exchange on an
    // atomic cell) needs the root to exist, not to be mutable: `&self`
    // reaches its attached fields exactly as it does for a read.
    if receiver == BorrowPolarity::Shared {
        if !writable_roots.contains_for_shared_access(root_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` atomic operation cannot reach \
                 `{root_name}` because no such field or local exists in this state"
            )));
        }
        return;
    }

    let access = crate::value_custody::reference_access::assignment_storage_write_access(
        program,
        machine,
        current_state,
        writable_roots.statements,
        target,
    );
    let writes_borrowed_storage =
        access == Some(crate::value_custody::expression_types::StorageWriteAccess::BorrowedStorage);
    if !writable_roots.contains_for_write(root_name, target_is_bare_name)
        && !writes_borrowed_storage
    {
        // The writable set cannot distinguish a nonexistent root (a typo) from a real
        // but non-mutable one, so append a conditional typo hint -- correct whatever
        // the cause. (A full "data X has no field Y" check is the separate
        // unknown-field validation-gap TASK.)
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` \
             because it is not mutable in this state (if `{root_name}` is undeclared -- a typo -- \
             no such field or local exists)"
        )));
    } else if access.is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment requires mutable source access \
             through every reference crossed by its destination"
        )));
    }
}

fn is_mutable_place_handle(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => is_mutable_place_handle(program, indexed.collection),
        ExpressionNode::Member(member) => is_mutable_place_handle(program, member.receiver),
        ExpressionNode::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name_handle(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<&str> {
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
                            .is_some_and(|name| name.is_self_receiver()) =>
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
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
) -> Option<&str> {
    match program.expression_table.expression(target) {
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let Some(path) = program.expression_table.name_path_node(member.receiver) else {
                return None;
            };
            let receiver = program.expression_table.name_path_members(path.members);
            (receiver.len() == 1 && receiver[0].is_self_receiver()).then(|| member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 2 && members[0].is_self_receiver()).then(|| members[1].as_str())
        }
        _ => None,
    }
}

/// Select one live nominal declaration without replacing retained identity by
/// a coincidentally matching diagnostic name.
fn data_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<&DataDefinition> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != symbols::SymbolKind::Data {
        return None;
    }
    match crate::declarations::data_queries::memoized_data_definition_lookup(program, symbol) {
        crate::declarations::data_queries::DataDefinitionLookup::Unique(position) => {
            program.data_definitions().get(position as usize)
        }
        _ => None,
    }
}

/// The attached declaration selected for this machine. Diagnostic attachment
/// spelling is not authority to recover a missing or different nominal owner.
pub(crate) fn machine_attached_data<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<&'a DataDefinition> {
    data_definition_by_symbol(program, machine.attached_data_symbol)
}

/// The DECLARED type of a simple place argument: a bare local/parameter name
/// (`msg`) or an attached-data field (`self.buffer`), through the `&mut`
/// marker. `None` for shapes this scope cannot type (those re-resolve during
/// instruction selection). Shared by the wire-call argument checks and the
/// machine-call type-parameter bound check (frozen decision 13).
pub(crate) fn declared_place_type(
    program: &dyn crate::CheckingInput,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let raw = declared_place_type_raw(program, current_machine, current_state, argument)?;
    unwrapped_type_reference(program, raw)
}

pub fn assignment_value_type(
    program: &dyn crate::CheckingInput,
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
/// arithmetic domain (decision 17) read it from this raw handle. This type query
/// also recognizes declared call results and substituted constant arrays; a
/// result here does not establish that an expression denotes storage.
pub fn declared_place_type_raw(
    program: &dyn crate::CheckingInput,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    if let Some(inner) = program.expression_table.borrow_expression(handle) {
        handle = inner.target;
    }

    // A contract result is a declaration-backed value rather than runtime
    // storage. Keep its exact owning occurrence and full result type, just as
    // selected calls retain their declared result below. This query supplies
    // no writable place or proof of a result predicate.
    if let Some(result) = crate::reserved_result_place(program, handle)
        && result.machine_symbol == current_machine.symbol
    {
        return Some(result.type_reference);
    }

    if let Some(path) = program.expression_table.name_path_node(handle)
        && program.expression_table.name_path_is_single_member(path)
        && let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol)
        && let typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
    {
        // Captured conformance and callable binders keep the exact arena
        // declaration named by this occurrence, even when they are outside
        // the machine's local telescope. Their carrier grants no writable place.
        return Some(type_reference);
    }

    if let Some(path) = program.expression_table.name_path_node(handle)
        && path.head_symbol == path.symbol
        && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Field
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
    {
        // A field declaration alone is not a runtime place. Its explicit
        // parameter or local receiver must supply the storage identity.
        return None;
    }

    if projected_members::contains_case_projection(program, handle) {
        return projected_members::declared_case_projection_type(
            program,
            current_machine,
            current_state,
            handle,
        );
    }

    if let Some(field) = exact_self_field(program, current_machine, handle) {
        return Some(field.type_reference);
    }

    // Preserve the retained root while traversing ordinary member expressions.
    // Flattening first loses a bare inherited field's declaration identity.
    if let Some(member) = program.expression_table.member_node(handle) {
        let receiver =
            declared_place_type_raw(program, current_machine, current_state, member.receiver)?;
        // Read the selected field under this receiver's reaching application.
        // An unmaterialized generic declaration still stores an open `T`;
        // looking up that field alone would erase the actual reference shell.
        // Retained selector admission and type projection share the same
        // application evidence used by canonical place resolution.
        let symbol = member_resolution::effective_member_symbol_from_position(
            program,
            member.receiver,
            Some(member_resolution::MemberPosition::Reference(receiver)),
            member,
        );
        return project_type_reference_from_segments(
            program,
            receiver,
            &[facts::PlaceSegment::Field { symbol }],
        );
    }
    if let Some(members) = collect_member_path(program, handle) {
        return declared_member_path_type(program, current_machine, current_state, &members);
    }
    match program.expression_table.expression(handle) {
        ExpressionNode::ArrayLiteral(_) => crate::declared_constant_array_type(program, handle),
        ExpressionNode::Call(_) => crate::machine_calls::calls::call_result_type(program, handle),
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
pub(crate) fn declared_member_field_symbol(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() {
        return None;
    }
    let receiver = declared_place_type_raw(program, machine, state, member.receiver)?;
    let data = data_definition_for_type(program, receiver)?;
    Some(data_field_or_payload(program, data, member.member.as_str())?.symbol)
}

/// The declared type of the one binding a normalized immutable-bound symbol
/// resolves to: a unique state parameter or local across the program, the same
/// program-wide uniqueness contract the bound normalizers in
/// `immutable_integer_bounds` already require. `None` when the symbol names
/// zero or several bindings. This recovers only the declared carrier for
/// bound-floor evidence; mutability vetting and stored-value identity stay
/// with the normalization that produced the symbol.
pub(crate) fn bound_symbol_declared_type(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    let mut found = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol && found.replace(parameter.type_reference).is_some() {
                    return None;
                }
            }
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
            {
                if let Some(local) = statement.as_local_data()
                    && local.symbol == symbol
                    && found.replace(local.type_reference).is_some()
                {
                    return None;
                }
            }
        }
    }
    found
}

mod attached_storage;
mod builtin_coordinates;
mod exact_self_field;
mod structural_scalar_field;
pub use structural_scalar_field::{
    StructuralScalarField, StructuralSource, saved_dispatch_call, saved_dispatch_subject,
    structural_scalar_field, structural_scalar_field_at_coordinate,
    structural_scalar_field_in_parameters, structural_source,
};
mod member_paths;
mod parameter_scoped;
pub(crate) use parameter_scoped::has_retained_builtin_index_meaning;
pub(crate) use parameter_scoped::parameter_scoped_type_reference;
mod projected_members;
mod result_shape;
pub use attached_storage::{
    AttachedStorageRoot, attached_storage_root, canonical_storage_label, declaration_storage_label,
    exact_attached_storage_field, expression_names_attached_storage,
};
pub use builtin_coordinates::{
    assignment_target_indexes, assignment_target_selectors, collection_length_receiver,
    declared_collection_extent, fixed_array_type_length, has_builtin_subslice_meaning,
    indexed_operation_has_builtin_meaning, mutable_byte_slice_parameter_element,
    subslice_expression, whole_collection_view_source,
};
pub use builtin_coordinates::{
    place_has_builtin_coordinates, value_element_has_builtin_coordinate,
};
pub use exact_self_field::exact_attached_field;
pub use exact_self_field::exact_self_field;
pub(super) use exact_self_field::single_member_receiver_field;
use member_paths::{collect_member_path, data_field_or_payload, data_field_or_payload_type};
pub(crate) use member_paths::{
    data_definition_for_type, declared_member_path_type, first_unknown_nested_field,
};
pub(crate) use projected_members::applied_data_member_field;
pub(crate) use projected_members::declared_case_projection_field;
pub use projected_members::exact_data_member_field;
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
    program: &dyn crate::CheckingInput,
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
/// Every index hop uses the shared effective element projection, retaining a
/// numeric element's inherited policy before the next hop or field access.
pub(crate) fn declared_indexed_projection_type_raw(
    program: &dyn crate::CheckingInput,
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
                if program.expression_table.indexed_on_range(indexed) {
                    return None;
                }
                let collection_type = declared_place_type_raw(
                    program,
                    current_machine,
                    current_state,
                    indexed.collection,
                )?;
                let mut leaf = collection_element_type_reference(program, collection_type)?;
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

/// The legacy spelling of `TypedTrees::unwrapped_type_reference`, kept for the
/// callers inside checking and stage 04 that still name it. Peeling a
/// `Reference` or `Constrained` shell is a pure question about the type table,
/// so the implementation lives with the table in `typed-trees`; this delegates
/// rather than restating it, leaving one implementation to maintain.
pub fn unwrapped_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    program.unwrapped_type_reference(type_reference)
}

/// The declared carrier beneath a type's `Constrained` shells: a predicate
/// qualification constrains what a place's contents satisfy, never the shape
/// of the storage that carries it. Which qualifications a declaration asserts
/// or discards is the weakening law's question —
/// `proof_contracts::domain_weakening` — already adjudicated by the checker
/// before a custody join asks what storage a copy, move, or borrow
/// reproduces.
pub fn peel_declared_constraints(
    program: &dyn crate::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    type_reference
}

/// The element type a `FixedArray` or `Slice` carrier declares — `None` for
/// every other carrier shape.
pub fn collection_element_type(
    program: &dyn crate::CheckingInput,
    collection: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(collection) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

/// The declared reference access beneath a type's `Constrained` shells —
/// `None` when the peeled carrier is not a `Reference`.
pub fn declared_reference_access(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<language_core::ReferenceAccess> {
    let peeled = peel_declared_constraints(program, type_reference);
    let TypeReferenceNode::Reference { access, .. } =
        program.type_reference_table.type_reference(peeled)
    else {
        return None;
    };
    Some(*access)
}

/// Two declared types name the same storage carrier when they differ only by
/// `Constrained` shells, at any depth: a `&[u8] in Utf8` leaf and a `&[u8]`
/// destination both describe the 16-byte shared-view descriptor. Reference
/// access, fixed extents, and generic arguments stay strict — they change
/// layout or custody, which no qualification erases. A pair that differs in
/// any other way keeps its full normalized-identity comparison.
pub fn declared_carriers_match(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    let left = peel_declared_constraints(program, left);
    let right = peel_declared_constraints(program, right);
    match (
        program.type_reference_table.type_reference(left),
        program.type_reference_table.type_reference(right),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: left_referee,
                access: left_access,
                ..
            },
            TypeReferenceNode::Reference {
                referee: right_referee,
                access: right_access,
                ..
            },
        ) => {
            left_access == right_access
                && declared_carriers_match(program, *left_referee, *right_referee)
        }
        (
            TypeReferenceNode::Slice {
                element_type: left_element,
            },
            TypeReferenceNode::Slice {
                element_type: right_element,
            },
        ) => declared_carriers_match(program, *left_element, *right_element),
        (
            TypeReferenceNode::FixedArray {
                element_type: left_element,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right_element,
                length: right_length,
            },
        ) => {
            left_length == right_length
                && declared_carriers_match(program, *left_element, *right_element)
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: left_symbol,
                arguments: left_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: right_symbol,
                arguments: right_arguments,
                ..
            },
        ) => {
            let left_handles = program
                .type_reference_table
                .type_reference_handles(*left_arguments);
            let right_handles = program
                .type_reference_table
                .type_reference_handles(*right_arguments);
            left_symbol == right_symbol
                && left_handles.len() == right_handles.len()
                && left_handles.iter().zip(right_handles.iter()).all(
                    |(left_argument, right_argument)| {
                        declared_carriers_match(program, *left_argument, *right_argument)
                    },
                )
        }
        _ => program.normalized_type_identity(left) == program.normalized_type_identity(right),
    }
}

/// Whether a declared type is wholly zero-constructible: a primitive scalar,
/// a fixed array of literal extent whose element qualifies, a named record
/// whose fields all qualify recursively, or a named sum whose first declared
/// case exists and whose payload fields qualify recursively. Zeroed storage
/// selects the first case (the tag stores the case ordinal and all-zero bits
/// name ordinal 0), so only that case's payload contributes cells; a float
/// payload is +0.0 and an aggregate payload is its own zero value
/// (dependent_values.md, default domains and zero initialization).
/// Reference and constrained payload carriers still decline: a reference has
/// no zero spelling and a constrained carrier may exclude 0. Common fields
/// beside cases follow the ordinary record rule. This is shape admission,
/// not proof of a declaration's domain constraints, which source validation
/// checks separately. Byte sequences and erased members remain unsupported.
pub fn zeroed_structural_parts(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let mut pending = vec![type_reference];
    while let Some(current) = pending.pop() {
        let Some(reference) = unwrapped_type_reference(program, current) else {
            return false;
        };
        if program.primitive_type_reference(reference).is_some() {
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(_),
            } => pending.push(*element_type),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                // Zero storage follows the same closed declaration as ordinary
                // construction. Region arguments do not change layout, and a
                // type application must use its substituted fields rather than
                // the unspecialized declaration's parameter types.
                let Some(data) = program.closed_data_definition(reference) else {
                    return false;
                };
                let members = program.data_members(data);
                let mut variants = members.iter().filter_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant) => Some(variant),
                    _ => None,
                });
                let Some(first_case) = variants.next() else {
                    for member in members {
                        let Some(field) = member.as_field() else {
                            return false;
                        };
                        if field.relevance.is_erased() {
                            return false;
                        }
                        pending.push(field.type_reference);
                    }
                    continue;
                };
                // Common fields are present regardless of the selected case.
                // They use the same recursive zero establishment as record
                // fields, while only the first case contributes a payload.
                for member in members {
                    if let Some(field) = member.as_field() {
                        if field.relevance.is_erased() {
                            return false;
                        }
                        pending.push(field.type_reference);
                    }
                }
                for field in program.data_payload_fields(first_case) {
                    if field.relevance.is_erased()
                        || matches!(
                            program
                                .type_reference_table
                                .type_reference(field.type_reference),
                            TypeReferenceNode::Reference { .. }
                                | TypeReferenceNode::Constrained { .. }
                        )
                    {
                        return false;
                    }
                    pending.push(field.type_reference);
                }
            }
            _ => return false,
        }
    }
    true
}

mod declared_symbols;
pub use declared_symbols::{
    RuntimeFieldSubject, data_member_symbol_by_name, declared_data_type, expression_root_symbol,
    first_valid_name_path_symbol, machine_symbol_from_type_reference_handle, nearest_scoped_local,
    runtime_field_subject, state_scoped_name_symbol, structural_member_identity,
};
mod canonical_projection;
pub use canonical_projection::{
    CanonicalPlace, canonical_collection_element_place, canonical_place_from_expression,
    canonical_place_from_expression_in_state, element_place_segment, index_place_segment,
    push_field_place_segments,
};
mod contextual_projection;
pub use contextual_projection::contextual_canonical_place_from_expression;
mod member_resolution;
pub use member_resolution::{
    effective_member_symbol, effective_member_symbol_from_type_symbol,
    expression_place_type_reference, expression_type_symbol,
    resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
mod projection_types;
pub use projection_types::{
    canonical_place_source_type_application, canonical_place_type_reference,
    collection_element_type_reference, expression_type_reference_in_state,
    loan_carrier_has_owned_storage, project_type_reference_from_segments,
    unique_data_definition_by_symbol,
};
pub mod receiver_projections;
pub use receiver_projections::{
    projected_statement_receiver_place, projected_statement_receiver_type_reference,
};
