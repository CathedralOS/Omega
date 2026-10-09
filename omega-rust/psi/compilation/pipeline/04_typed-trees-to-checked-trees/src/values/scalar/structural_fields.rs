//! Resolve structural observations against their declared storage and exact projections.
//!
//! Root custody distinguishes parameters, owned locals and view locals. Selection
//! does not: length, content and element observations must retain the same field
//! path and selected type rather than rediscovering a whole-local source shape.
use crate::values::scalar::expression_facts::is_integer;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedOperatorResolutionStatus;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedStructuralPredicatePathSegment;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

fn declared_data_member(
    program: &dyn ::validation::CheckingInput,
    data_index: u32,
    member_index: u32,
) -> &typed_trees::data::DataMember {
    &program.data_members(&program.data_definitions()[data_index as usize])[member_index as usize]
}

fn declared_data_field(
    program: &dyn ::validation::CheckingInput,
    position: (u32, u32, u32),
) -> Option<&typed_trees::data::DataField> {
    let member = declared_data_member(program, position.0, position.1);
    if position.2 == u32::MAX {
        match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        }
    } else {
        let variant = member.as_variant()?;
        program
            .data_payload_fields(variant)
            .get(position.2 as usize)
    }
}

pub(super) fn structural_sequence_length(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let receiver = program
        .expression_table
        .authored_length_receiver(expression)?;
    let (root, path, selected_type) = structural_storage_place(program, parameters, receiver)?;
    // A raw fixed-array projection has a type-owned extent. Do
    // not strip a constrained carrier: its live length can differ from its
    // backing capacity. Dynamic byte views retain their runtime read below.
    let extent_type = program
        .type_reference_table
        .dereferenced_type_reference(selected_type);
    if let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(extent_type)
    {
        let length = u64::try_from(*length).ok()?;
        return facts::u64_integer_literal(length, PrimitiveType::U64);
    }
    if path.is_empty() {
        // A whole bounded owner observes the live length stored beside its
        // bytes, whether the parameter owns that value or lends readable
        // access to it. Its capacity is never the answer; a write-only loan
        // still grants no observation of the stored extent.
        if matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(
                program,
                selected_type,
                &[]
            ),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
        ) && validation::reference_shell(program, selected_type).is_none_or(|shell| {
            matches!(
                shell.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            )
        }) {
            return Some(CheckedScalarExpression::StructuralParameterByteLength { root, path });
        }
        // A domain constraint (`&[u8] in Path`) wraps the reference or its
        // referent without changing where the extent lives: peel those shells
        // on both sides before identifying the view.
        let mut parameter_kind = selected_type;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(parameter_kind)
        {
            parameter_kind = *base_type;
        }
        let TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable,
            ..
        } = program.type_reference_table.type_reference(parameter_kind)
        else {
            return None;
        };
        // Any element family keeps a runtime extent on its view descriptor:
        // byte views count bytes, element views count elements, record
        // elements included.
        let mut referee_kind = *referee;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(referee_kind)
        {
            referee_kind = *base_type;
        }
        let TypeReferenceNode::Slice { .. } =
            program.type_reference_table.type_reference(referee_kind)
        else {
            return None;
        };
    } else if !matches!(
        path.last(),
        Some(CheckedStructuralPredicatePathSegment::Field(_))
    ) || path
        .iter()
        .any(|segment| matches!(segment, CheckedStructuralPredicatePathSegment::Case(_)))
        // The shared carrier classifier distinguishes a bounded byte field's
        // live length from a raw fixed array's static capacity. A borrowed
        // `&[u8]` leaf keeps a runtime length on its view descriptor, so its
        // `.len` reads the same live extent; a borrowed element view field
        // carries its element count on the same descriptor slot whatever the
        // view's access (`&[T]`, `&mut [T]`, `&write [T]`).
        || (!matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(program, selected_type, &[]),
            Some(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
            )
        ) && crate::execution::terminal_unit::types::borrowed_slice_view_element(
            program,
            selected_type,
            &[],
        )
        .is_none())
    {
        return None;
    }
    Some(CheckedScalarExpression::StructuralParameterByteLength { root, path })
}

/// The `let` symbol and declared type of an immutable borrowed view local
/// that `expression` names whole, when that name is not a state parameter. The observation only
/// names the local: the plan that sequences the body decides whether the
/// local's view was established before this use, and lowering resolves the
/// symbol to its published place or refuses.
pub(super) fn view_local_root(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let (symbol, local) = whole_local_binding(program, parameters, expression)?;
    // Either borrowed view family resolves the same `ViewLocal` root: reads
    // through an exclusive `&mut [T]`/`&mut R` loan re-view the lent
    // element exactly as the shared sibling's reads observe the shared
    // place. The root records no access side; the producing
    // establishment's custody decides whether the read composes with
    // concurrent uses.
    (!local.is_mutable
        && (crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            local.type_reference,
        ) || crate::execution::terminal_unit::types::borrowed_named_view(
            program,
            local.type_reference,
        ) || crate::execution::terminal_unit::types::element_view_shell(
            program,
            local.type_reference,
            &[],
        )
        .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Mutable)
            || crate::execution::terminal_unit::types::mutable_borrowed_view(
                program,
                local.type_reference,
            )))
    .then_some((symbol, local.type_reference))
}

/// The `let` symbol and declared type of an owned structural local that
/// `expression` names whole, when that name is not a state parameter or a
/// view local. The plan that sequences the body decides whether the local's
/// place was established before this use; lowering resolves the symbol to
/// that place or refuses.
pub(super) fn owned_local_root(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let (symbol, local) = whole_local_binding(program, parameters, expression)?;
    // Root discovery retains identity, not permission to copy the whole
    // value. A record's borrowed descendants do not prevent reading another
    // field; establishment and observation replay check the selected path's
    // custody. Preserve the declared constraints for that replay, peeling
    // only to distinguish owned storage from a reference root.
    let mut carrier_type = local.type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier_type)
    {
        carrier_type = *base_type;
    }
    let owned = match program.type_reference_table.type_reference(carrier_type) {
        TypeReferenceNode::FixedArray { .. } => true,
        TypeReferenceNode::Named { symbol, .. } => {
            symbol.is_valid() && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data
        }
        _ => false,
    };
    owned.then_some((symbol, local.type_reference))
}

/// Empty spelling resolution is builtin only when the exact operand types
/// independently carry the builtin index meaning.
pub(super) fn indexed_read_is_builtin(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    collection_type: TypeReferenceHandle,
    index: ExpressionHandle,
) -> bool {
    use language_core::OperatorSpelling;
    if let Some(selected) = operators.expression_use(expression)
        && (selected.spelling != OperatorSpelling::Index
            || selected.selected_operator_symbol.is_valid()
            || selected.candidate_count != 0
            || !matches!(
                selected.status,
                CheckedOperatorResolutionStatus::Missing
                    | CheckedOperatorResolutionStatus::BuiltinFallback
            ))
    {
        return false;
    }
    indexed_read_has_builtin_meaning(program, parameters, expression, collection_type, index)
}

fn indexed_read_has_builtin_meaning(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    collection_type: TypeReferenceHandle,
    index: ExpressionHandle,
) -> bool {
    let Some((machine_index, state_index)) =
        program.state_position_for_parameter_symbols(parameters)
    else {
        return false;
    };
    let machine = &program.machines()[machine_index as usize];
    let state = &program.machine_states(machine)[state_index as usize];
    let operands = [
        Some(collection_type),
        validation::declared_place_type_raw(program, machine, Some(state), index),
    ];
    typed_trees::operator::builtin_index_read_is_spelled(
        program,
        machine.symbol,
        expression,
        &operands,
    )
}

pub(super) fn structural_parameter_field_path(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    fields: &mut Vec<CheckedStructuralPredicatePathSegment>,
) -> Option<u32> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let position =
                structural_parameter_field_path(program, parameters, indexed.collection, fields)?;
            let (_, _, collection_type, _) =
                resolve_structural_parameter_path(program, parameters, position, fields)?;
            if !indexed_read_has_builtin_meaning(
                program,
                parameters,
                expression,
                collection_type,
                indexed.index,
            ) {
                return None;
            }
            let element_index = u64::try_from(
                program
                    .expression_table
                    .constant_integer_value(indexed.index)?,
            )
            .ok()?;
            fixed_index_element_type(program, collection_type, element_index)?;
            fields.push(CheckedStructuralPredicatePathSegment::FixedIndex(
                element_index,
            ));
            Some(position)
        }
        ExpressionNode::Name(name) => {
            if program.symbols.get(name.symbol).kind == symbols::SymbolKind::Machine
                || matches!(program.expression_table.name_path_members(name.members),
                    [spelling] if spelling.is_self_receiver())
            {
                return None;
            }
            // A parameter owns its place root by symbol. Resolution retains
            // that symbol on the name itself or on one of its path members,
            // so a path whose every member symbol is invalid roots at no
            // authored parameter and resolves nothing here.
            let root = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                program
                    .expression_table
                    .name_path_member_symbols(name.member_symbols)
                    .iter()
                    .copied()
                    .find(|symbol| symbol.is_valid())
            })?;
            parameters
                .iter()
                .position(|parameter| {
                    crate::execution::terminal_unit::types::parameter_is_root(parameter, root)
                })
                .and_then(|position| u32::try_from(position).ok())
        }
        ExpressionNode::Member(member) => {
            let parameter =
                structural_parameter_field_path(program, parameters, member.receiver, fields)?;
            let field_identity = |field: &typed_trees::data::DataField| field.path_identity();
            parameters.get(usize::try_from(parameter).ok()?)?;
            if let Some(case_name) = &member.case_variant {
                let (case, field) = program
                    .data_variant_positions_for_name(case_name.as_str())
                    .into_iter()
                    .find_map(|(data_index, member_index)| {
                        let variant =
                            declared_data_member(program, data_index, member_index).as_variant()?;
                        program
                            .data_payload_field_by_symbol(variant, member.member_symbol)
                            .map(|field| (variant.path_identity(), field_identity(field)))
                    })?;
                fields.push(CheckedStructuralPredicatePathSegment::Case(case));
                fields.push(CheckedStructuralPredicatePathSegment::Field(field));
            } else {
                let identity = if member.member_symbol.is_valid() {
                    program
                        .data_field_positions_for_symbol(member.member_symbol)
                        .iter()
                        .copied()
                        .filter(|position| position.2 == u32::MAX)
                        .find_map(|position| {
                            declared_data_field(program, position).map(&field_identity)
                        })?
                } else {
                    // Contract member expressions can reach this carrier
                    // before their field symbol is retained. Keep the
                    // authored segment in that case: path_type_reference
                    // resolves it against the exact receiver type below,
                    // so this does not perform global name-based selection.
                    member.member.as_str().to_owned()
                };
                fields.push(CheckedStructuralPredicatePathSegment::Field(identity));
            }
            Some(parameter)
        }
        _ => None,
    }
}

/// The resolved path is `frozen` when the root parameter and every
/// intermediate receiver are immutable: no `mut` formal and no exclusive
/// reference. Declared constraints on frozen storage are read invariants;
/// mutable storage can be reborrowed through a pointee type that drops them,
/// so only the live snapshots speak for it.
pub(crate) fn resolve_structural_parameter_path(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    position: u32,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Option<(
    symbols::SymbolHandle,
    Vec<facts::PlaceSegment>,
    TypeReferenceHandle,
    bool,
)> {
    let parameter = parameters.get(usize::try_from(position).ok()?)?;
    if !parameter.symbol.is_valid() {
        return None;
    }
    let (segments, selected_type, frozen) = resolve_structural_path(
        program,
        parameter.type_reference,
        parameter.is_mutable,
        path,
    )?;
    Some((parameter.symbol, segments, selected_type, frozen))
}

/// Resolve each projection against the declaration reached by its predecessor,
/// independent of the storage root's custody. Keeping this join common prevents
/// local observations from losing fixed indexes or accepting a foreign field.
fn resolve_structural_path(
    program: &dyn ::validation::CheckingInput,
    declared_type: TypeReferenceHandle,
    is_mutable: bool,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Option<(Vec<facts::PlaceSegment>, TypeReferenceHandle, bool)> {
    let mut receiver = declared_type;
    let mut frozen = !is_mutable && !exclusive_reference(program, receiver);
    let mut segments = Vec::new();
    let mut selected_case = None;
    for segment in path {
        match segment {
            CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                if selected_case.is_some() {
                    return None;
                }
                receiver = fixed_index_element_type(program, receiver, *element_index)?;
                segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*element_index).ok()?,
                });
            }
            CheckedStructuralPredicatePathSegment::Case(identity) => {
                if selected_case.is_some() {
                    return None;
                }
                let definition = structural_data(program, receiver)?;
                let variant = program.data_members(definition).iter().find_map(|member| {
                    let variant = member.as_variant()?;
                    let actual = variant
                        .identity
                        .map(|number| format!("#{number}"))
                        .unwrap_or_else(|| variant.name.as_str().to_owned());
                    (actual == *identity).then_some(variant)
                })?;
                if !variant.symbol.is_valid() {
                    return None;
                }
                segments.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                selected_case = Some(variant);
            }
            CheckedStructuralPredicatePathSegment::Field(identity) => {
                let matches = |field: &&typed_trees::data::DataField| {
                    field
                        .identity
                        .map(|number| format!("#{number}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned())
                        == *identity
                };
                let field = if let Some(variant) = selected_case.take() {
                    program.data_payload_fields(variant).iter().find(matches)?
                } else {
                    let definition = structural_data(program, receiver)?;
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| {
                            let field = member.as_field()?;
                            Some(field)
                        })
                        .find(matches)?
                };
                if !field.symbol.is_valid() || field.relevance.is_erased() {
                    return None;
                }
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                receiver = field.type_reference;
            }
            // A runtime selector names no authored place segment: the
            // canonical place's `Index` step spells the authored selector
            // expression, which the checked coordinate does not retain.
            CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => return None,
        }
        if exclusive_reference(program, receiver) {
            frozen = false;
        }
    }
    selected_case
        .is_none()
        .then_some((segments, receiver, frozen))
}

/// Whether this reference resolves through an exclusive borrow (`&mut`/`&w`).
/// Shared references keep their referent frozen for the borrow's lifetime.
pub(crate) fn exclusive_reference(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
            _ => return false,
        }
    }
}

/// Peel reference and constraint shells down to the indexed carrier's node.
/// A byte carrier (a borrowed `&[u8]` or `&[u8; N]`, or a domain-qualified
/// bounded owner) declares no element row: its extent is observed through the
/// carrier, so an index selects no static element
/// (wiki/spec/language/domains.md, "Byte containers and encoding domains").
fn peeled_indexed_carrier(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Constrained { .. }
                if validation::byte_sequence_carrier(program, reference, &[]).is_some() =>
            {
                return None;
            }
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => return Some(reference),
        }
    }
}

pub(super) fn fixed_index_element_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    element_index: u64,
) -> Option<TypeReferenceHandle> {
    let carrier = peeled_indexed_carrier(program, reference)?;
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { length, .. } => {
            let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                return None;
            };
            (element_index < u64::try_from(*length).ok()?)
                .then(|| validation::collection_element_type_reference(program, reference))?
        }
        _ => None,
    }
}

/// The element type a runtime selector reaches inside a fixed array: the
/// same unwrap `fixed_index_element_type` walks, with no literal to prove
/// against the extent — the read's own obligation checks the bound.
pub(super) fn runtime_index_element_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let carrier = peeled_indexed_carrier(program, reference)?;
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { length, .. } => {
            matches!(length, typed_trees::types::FixedArrayLength::Literal(_))
                .then(|| validation::collection_element_type_reference(program, reference))?
        }
        _ => None,
    }
}

pub(crate) fn structural_data(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let peeled = program
        .type_reference_table
        .peeled_type_reference(type_reference);
    let (symbol, name) = match program.type_reference_table.type_reference(peeled) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => {
            if let Some(instance) = program.closed_data_definition(peeled) {
                return Some(instance);
            }
            (*symbol, name)
        }
        _ => return None,
    };
    let symbol = if symbol.is_valid() {
        program
            .machine_positions_for_symbol(symbol)
            .iter()
            .map(|&position| &program.machines()[position as usize])
            .find(|machine| machine.attached_data_symbol.is_valid())
            .map_or(symbol, |machine| machine.attached_data_symbol)
    } else {
        symbol
    };
    if symbol.is_valid() {
        program.data_definition_by_symbol(symbol)
    } else {
        program
            .data_definition_positions_for_name(name.as_str())
            .first()
            .map(|position| &program.data_definitions()[*position as usize])
    }
}

pub(super) fn lower_structural_parameter_field(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let (parameter_position, path, type_reference) =
        structural_parameter_place(program, parameters, expression)?;
    if path.is_empty() {
        return None;
    }
    if matches!(
        path.last(),
        Some(CheckedStructuralPredicatePathSegment::FixedIndex(_))
    ) {
        // The primitive-storage endpoint is an array's primitive declaration,
        // bare or qualified only by an arithmetic policy. Membership-qualified
        // leaves retain their existing indexed-byte/proof route; scalar record
        // fields remain owned by field operations. An indexed carrier that
        // continues into a record field (`maps[1].value`) instead resolves its
        // scalar leaf through primitive_type_reference below.
        validation::unrestricted_builtin_primitive(program, type_reference)?;
    }
    let primitive_type = program.primitive_type_reference(type_reference)?;
    if primitive_type == PrimitiveType::Bool {
        return Some((
            CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                },
            )),
            ArithmeticDomain::Exact,
        ));
    }
    // A field observation retains its declared scalar carrier, including
    // address identity. Reading storage grants no numeric conversion.
    if !is_integer(primitive_type)
        && !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64)
    {
        return None;
    }
    Some((
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        },
        program.arithmetic_domain_for_type_reference(type_reference),
    ))
}

pub(super) fn structural_parameter_place(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    u32,
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    let (root, path, selected_type) = structural_storage_place(program, parameters, expression)?;
    let checked_trees::CheckedStorageRoot::Parameter { index } = root else {
        return None;
    };
    Some((index, path, selected_type))
}

/// Authenticate one authored storage occurrence and resolve its entire static
/// projection. Root classification supplies custody only; all roots use the
/// same canonical place and declaration traversal below.
pub(super) fn structural_storage_place(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    checked_trees::CheckedStorageRoot,
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    // Typed member nodes may leave selection to canonical place resolution.
    // A retained symbol, however, cannot contradict that exact selection.
    let mut authored = expression;
    let mut selected_fields = place
        .segments
        .iter()
        .rev()
        .filter_map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => Some(*symbol),
            _ => None,
        });
    loop {
        match program.expression_table.expression(authored) {
            ExpressionNode::Borrow(borrow) => {
                authored = borrow.target;
                continue;
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                authored = cast.value;
                continue;
            }
            _ => {}
        }
        if let Some(indexed) = program.expression_table.indexed_node(authored) {
            let (_, _, collection_type) =
                structural_storage_place(program, parameters, indexed.collection)?;
            if !indexed_read_has_builtin_meaning(
                program,
                parameters,
                authored,
                collection_type,
                indexed.index,
            ) {
                return None;
            }
            authored = indexed.collection;
            continue;
        }
        let Some(member) = program.expression_table.member_node(authored) else {
            break;
        };
        let selected = selected_fields.next()?;
        if member.member_symbol.is_valid() && selected != member.member_symbol {
            // Specialization retains the authored template member while its
            // canonical storage place names the closed instance's field. Only
            // the receiver's exact origin/member correspondence can justify
            // that change; a sibling instance or foreign field cannot.
            let closed_field =
                validation::expression_place_type_reference(program, member.receiver, &[])
                    .and_then(|reference| validation::unwrapped_type_reference(program, reference))
                    .and_then(|reference| {
                        program.closed_data_member_field(
                            reference,
                            member.member_symbol,
                            member.member.as_str(),
                        )
                    });
            if closed_field.is_some_and(|field| field.symbol == selected) {
                authored = member.receiver;
                continue;
            }
            return None;
        }
        authored = member.receiver;
    }
    let root = place.root;
    let _ = root.as_symbol()?;
    // Canonical-place recovery also serves body storage and can recover a
    // root from retained path-member symbols. A parameter observation must
    // authenticate the original occurrence too: stale or foreign head/leaf
    // symbols cannot acquire a formal's authority from that fallback.
    let name = program.expression_table.name_path_node(authored)?;
    let names = program.expression_table.name_path_members(name.members);
    let (storage_root, declared_type, is_mutable) = if matches!(names, [spelling] if spelling.is_self_receiver())
    {
        return None;
    } else {
        if !name.symbol.is_valid()
            || !name.head_symbol.is_valid()
            || (names.len() == 1 && name.symbol != name.head_symbol)
        {
            return None;
        }
        // The resolved head symbol IS which authored binder this place roots
        // at, and stage 02 mints one parameter symbol per authored binder, so
        // one roster cannot hold two entries sharing it and the uniqueness
        // bail below cannot widen. The spelling beside it is the operand a
        // renamed binder or a same-spelled sibling can disagree with.
        let mut candidates = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol == name.head_symbol);
        if let Some((position, parameter)) = candidates.next() {
            if candidates.next().is_some()
                || program.symbols.get(name.head_symbol).kind != symbols::SymbolKind::Parameter
                || root != facts::PlaceRoot::Symbol(parameter.symbol)
            {
                return None;
            }
            (
                checked_trees::CheckedStorageRoot::Parameter {
                    index: u32::try_from(position).ok()?,
                },
                parameter.type_reference,
                parameter.is_mutable,
            )
        } else {
            let (symbol, local) = whole_local_binding(program, parameters, authored)?;
            if root != facts::PlaceRoot::Symbol(symbol) {
                return None;
            }
            let storage_root = if view_local_root(program, parameters, authored).is_some() {
                checked_trees::CheckedStorageRoot::ViewLocal { symbol }
            } else {
                owned_local_root(program, parameters, authored)?;
                checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
            };
            (storage_root, local.type_reference, local.is_mutable)
        }
    };
    let mut path = Vec::new();
    for segment in &place.segments {
        path.push(match segment {
            facts::PlaceSegment::Field { symbol } => {
                let field = program
                    .data_field_positions_for_symbol(*symbol)
                    .iter()
                    .copied()
                    .find_map(|position| declared_data_field(program, position))?;
                CheckedStructuralPredicatePathSegment::Field(field.path_identity())
            }
            facts::PlaceSegment::Case { variant } => {
                let case = program
                    .data_variant_position_for_symbol(*variant)
                    .and_then(|(data_index, member_index)| {
                        match declared_data_member(program, data_index, member_index) {
                            typed_trees::data::DataMember::Variant(candidate) => Some(candidate),
                            _ => None,
                        }
                    })?;
                CheckedStructuralPredicatePathSegment::Case(case.path_identity())
            }
            facts::PlaceSegment::FixedIndex { index } => {
                CheckedStructuralPredicatePathSegment::FixedIndex(u64::try_from(*index).ok()?)
            }
            _ => return None,
        });
    }
    let (segments, type_reference, _) =
        resolve_structural_path(program, declared_type, is_mutable, &path)?;
    if segments != place.segments {
        return None;
    }
    Some((storage_root, path, type_reference))
}

/// Resolve an indexed element's member under its exact current declaration.
/// Normalized payload observations may not retain a member symbol: the selected
/// element type supplies the declaration, while any retained symbol must agree.
/// This is the same nominal selection for every indexed field, not permission
/// to capture the containing row or copy its ownership-bearing siblings.
pub(super) fn element_member_field<'program>(
    program: &'program dyn ::validation::CheckingInput,
    element: TypeReferenceHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<&'program typed_trees::data::DataField> {
    let mut current = element;
    let owner = loop {
        match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
            TypeReferenceNode::Reference {
                referee,
                access:
                    language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable,
                ..
            } => current = *referee,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                break structural_data(program, current)?;
            }
            _ => return None,
        }
    };
    let field = if let Some(origin) = owner.generic_instance {
        program.closed_data_member_field(origin, member.member_symbol, member.member.as_str())?
    } else {
        validation::exact_data_member_field(
            program,
            owner,
            member.member_symbol,
            member.member.as_str(),
            None,
        )?
    };
    if field.relevance.is_erased()
        || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
    {
        return None;
    }
    Some(field)
}

/// The `let` local `expression` names whole when that name is not a state
/// parameter: its symbol and declared local-data row. Both whole-name local
/// root lookups peel the same authored-name shape before their kind gates.
fn whole_local_binding<'a>(
    program: &'a dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    symbols::SymbolHandle,
    &'a typed_trees::statement::TableLocalData,
)> {
    let path = program.expression_table.name_path_node(expression)?;
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || parameters
            .iter()
            .any(|parameter| parameter.symbol == path.symbol)
    {
        return None;
    }
    let positions = program.local_data_positions_for_symbol(path.symbol);
    let [position] = positions.as_slice() else {
        return None;
    };
    let machine = &program.machines()[position.0 as usize];
    let state = &program.machine_states(machine)[position.1 as usize];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[position.2 as usize]
    else {
        return None;
    };
    Some((path.symbol, local))
}
