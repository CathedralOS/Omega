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

pub(crate) fn structural_sequence_length(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    selector: impl FnMut(ExpressionHandle, ExpressionHandle) -> Option<CheckedScalarExpression>,
) -> Option<CheckedScalarExpression> {
    let receiver = program
        .expression_table
        .authored_length_receiver(expression)?;
    let projection = super::scalar_lowering::storage_projection(
        program, operators, receiver, parameters, selector,
    )?;
    let selected_type = projection.selected_type;
    validation::field_domains::readable_type_reference(program, selected_type)?;
    // A raw fixed array has a type-owned extent, unlike a constrained byte
    // owner whose stored live count can differ from capacity. A call-free
    // selector can still trap. Refusing that fold here routes it through Apply,
    // whose evaluated selector parameters retain the original operations even
    // when its final count template is constant.
    let extent_type = program
        .type_reference_table
        .dereferenced_type_reference(selected_type);
    if let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(extent_type)
    {
        if projection.path.iter().any(|step| match step {
            checked_trees::CheckedScalarReadProjection::Static(_) => false,
            checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                selector.expression.contains_trapping_operation()
            }
        }) {
            return None;
        }
        return facts::u64_integer_literal(u64::try_from(*length).ok()?, PrimitiveType::U64);
    }
    let byte_carrier =
        crate::execution::terminal_unit::types::byte_sequence_carrier(program, selected_type, &[]);
    let element_view = crate::execution::terminal_unit::types::borrowed_slice_view_element(
        program,
        selected_type,
        &[],
    );
    if !matches!(
        byte_carrier,
        Some(
            checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                | checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
        )
    ) && element_view.is_none()
    {
        return None;
    }
    Some(CheckedScalarExpression::StructuralSequenceLength {
        root: projection.root,
        path: projection.path,
    })
}

/// The `let` symbol and declared type of an immutable borrowed view local
/// that `expression` names whole, when that name is not a state parameter. The observation only
/// names the local: the plan that sequences the body decides whether the
/// local's view was established before this use, and lowering resolves the
/// symbol to its published place or refuses.
pub(crate) fn view_local_root(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let (symbol, local) = whole_local_binding(program, parameters, expression)?;
    // Root custody follows the readable reference shell, not its descriptor
    // geometry. Fixed-array loans and runtime-length views observe the same
    // established local; classifying only slice/named carriers loses the
    // fixed loan before element selection. Establishment and each read still
    // enforce the access, extent and lifetime of the retained declaration.
    (!local.is_mutable
        && validation::reference_shell(program, local.type_reference).is_some_and(|shell| {
            matches!(
                shell.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            )
        }))
    .then_some((symbol, local.type_reference))
}

/// The `let` symbol and declared type of an owned structural local that
/// `expression` names whole, when that name is not a state parameter or a
/// view local. The plan that sequences the body decides whether the local's
/// place was established before this use; lowering resolves the symbol to
/// that place or refuses.
pub(crate) fn owned_local_root(
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
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
        // An applied data owner is storage just like a named declaration.
        // Use its prepared declaration rather than the source spelling;
        // lexical regions can keep a closed carrier in application form.
        _ => program.closed_data_definition(carrier_type).is_some(),
    };
    owned.then_some((symbol, local.type_reference))
}

/// Empty spelling resolution is builtin only when the exact operand types
/// independently carry the builtin index meaning.
pub(crate) fn indexed_read_is_builtin(
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

pub(crate) fn structural_parameter_field_path(
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
            let field_identity = |field: &typed_trees::data::DataField| {
                checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone())
            };
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
                            .map(|field| {
                                (
                                    checked_trees::CheckedMemberIdentity::of_member(
                                        variant.identity,
                                        variant.name.clone(),
                                    ),
                                    field_identity(field),
                                )
                            })
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
                    checked_trees::CheckedMemberIdentity::of_member(None, member.member.clone())
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
        &mut |_, _| Some(()),
    )?;
    Some((parameter.symbol, segments, selected_type, frozen))
}

/// Admit total entry observations through the ordinary declaration walk.
/// Geometry alone can peel references or select a view element; an entry
/// predicate must retain readable root custody and cannot hide dereferences or
/// a dynamic bounds obligation inside its static path. Case validity remains
/// the ordinary contract checker's obligation.
pub(crate) fn readable_entry_parameter_path(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    position: u32,
    path: &[CheckedStructuralPredicatePathSegment],
    remaining: &mut usize,
) -> Option<TypeReferenceHandle> {
    let parameter = parameters.get(usize::try_from(position).ok()?)?;
    if parameter.is_const || parameter.relevance.is_erased() || path.len() >= 64 {
        return None;
    }
    let mut at_root = true;
    let (_, selected_type, _) = resolve_structural_path(
        program,
        parameter.type_reference,
        parameter.is_mutable,
        path,
        &mut |mut receiver, segment| {
            for _ in 0..64 {
                *remaining = remaining.checked_sub(1)?;
                if !program
                    .type_reference_table
                    .contains_type_reference(receiver)
                {
                    return None;
                }
                match program.type_reference_table.type_reference(receiver) {
                    TypeReferenceNode::Constrained { base_type, .. } => receiver = *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } if at_root && *access != language_core::ReferenceAccess::WriteOnly => {
                        receiver = *referee
                    }
                    TypeReferenceNode::Reference { .. } => return None,
                    node => {
                        if matches!(
                            segment,
                            Some(CheckedStructuralPredicatePathSegment::FixedIndex(_))
                        ) && !matches!(
                            node,
                            TypeReferenceNode::FixedArray {
                                length: typed_trees::types::FixedArrayLength::Literal(_),
                                ..
                            }
                        ) {
                            return None;
                        }
                        if matches!(
                            segment,
                            Some(
                                CheckedStructuralPredicatePathSegment::Field(_)
                                    | CheckedStructuralPredicatePathSegment::Case(_)
                            )
                        ) {
                            // A concrete declaration must own these members;
                            // open generic templates supply no executable shape.
                            let definition = program.closed_data_definition(receiver)?;
                            if program.symbols.get(definition.symbol).kind
                                != symbols::SymbolKind::Data
                            {
                                return None;
                            }
                        }
                        if matches!(
                            segment,
                            Some(
                                CheckedStructuralPredicatePathSegment::Field(_)
                                    | CheckedStructuralPredicatePathSegment::FixedIndex(_)
                            )
                        ) {
                            at_root = false;
                        }
                        return Some(());
                    }
                }
            }
            None
        },
    )?;
    Some(selected_type)
}

/// Resolve each projection against the declaration reached by its predecessor,
/// independent of the storage root's custody. Keeping this join common prevents
/// local observations from losing fixed indexes or accepting a foreign field.
fn resolve_structural_path(
    program: &dyn ::validation::CheckingInput,
    declared_type: TypeReferenceHandle,
    is_mutable: bool,
    path: &[CheckedStructuralPredicatePathSegment],
    observe_receiver: &mut impl FnMut(
        TypeReferenceHandle,
        Option<&CheckedStructuralPredicatePathSegment>,
    ) -> Option<()>,
) -> Option<(Vec<facts::PlaceSegment>, TypeReferenceHandle, bool)> {
    let mut receiver = declared_type;
    let mut frozen = !is_mutable && !exclusive_reference(program, receiver);
    let mut segments = Vec::new();
    let mut selected_case = None;
    for segment in path {
        observe_receiver(receiver, Some(segment))?;
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
                    (*identity
                        == checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        ))
                    .then_some(variant)
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
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ) == *identity
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
    observe_receiver(receiver, None)?;
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

pub(crate) fn fixed_index_element_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    element_index: u64,
) -> Option<TypeReferenceHandle> {
    let carrier = peeled_indexed_carrier(program, reference)?;
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { length, .. } => match length {
            typed_trees::types::FixedArrayLength::Literal(length) => (element_index
                < u64::try_from(*length).ok()?)
            .then(|| validation::collection_element_type_reference(program, reference))?,
            // A recipe extent admits the element type; the index obligation
            // against the closed extent is the range check's recorded row.
            typed_trees::types::FixedArrayLength::ConstCall { .. }
                if program.constant_recipe_for_array_length(carrier).is_valid() =>
            {
                validation::collection_element_type_reference(program, reference)
            }
            _ => None,
        },
        // A slice declares the element, not its current extent. Resolving
        // this observation's type supplies no bounds proof; the authored
        // index still owes its ordinary live-length obligation.
        TypeReferenceNode::Slice { .. } => {
            validation::collection_element_type_reference(program, reference)
        }
        _ => None,
    }
}

/// The element type a runtime selector reaches inside a collection: the
/// same unwrap `fixed_index_element_type` walks, with no literal to prove
/// against the extent — the read's own obligation checks the bound.
pub(crate) fn runtime_index_element_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let carrier = peeled_indexed_carrier(program, reference)?;
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { length, .. } => {
            (matches!(length, typed_trees::types::FixedArrayLength::Literal(_))
                || program.constant_recipe_for_array_length(carrier).is_valid())
            .then(|| validation::collection_element_type_reference(program, reference))?
        }
        TypeReferenceNode::Slice { .. } => {
            validation::collection_element_type_reference(program, reference)
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

pub(crate) fn lower_structural_parameter_field(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let (parameter_position, path, mut type_reference) =
        structural_parameter_place(program, parameters, expression)?;
    if path.is_empty() {
        // A whole primitive loan and a projected primitive leaf observe
        // storage in the same formal-place namespace. Bare scalar parameters
        // remain value formals; write-only loans grant no read.
        let shell = validation::reference_shell(program, type_reference)?;
        if !matches!(
            shell.access,
            language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
        ) {
            return None;
        }
        type_reference =
            validation::field_domains::readable_type_reference(program, type_reference)?;
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

pub(crate) fn structural_parameter_place(
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
pub(crate) fn structural_storage_place(
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
                CheckedStructuralPredicatePathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                )
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
                CheckedStructuralPredicatePathSegment::Case(
                    checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ),
                )
            }
            facts::PlaceSegment::FixedIndex { index } => {
                CheckedStructuralPredicatePathSegment::FixedIndex(u64::try_from(*index).ok()?)
            }
            _ => return None,
        });
    }
    let (segments, type_reference, _) =
        resolve_structural_path(program, declared_type, is_mutable, &path, &mut |_, _| {
            Some(())
        })?;
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
pub(crate) fn element_member_field<'program>(
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
