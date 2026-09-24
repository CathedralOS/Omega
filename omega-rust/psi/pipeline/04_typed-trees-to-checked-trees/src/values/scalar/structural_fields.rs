//! Resolve selected structural scalar reads against their authored parameter roots.
use crate::values::scalar::expression_facts::is_integer;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedOperatorResolutionStatus;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedStructuralPredicatePathSegment;
use language_semantics::declaration_selection::CollectionMeasure;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

#[cfg(test)]
mod tests;

pub(super) fn structural_sequence_length(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let Some((parameter_position, path, selected_type)) =
        structural_parameter_place(program, parameters, member.receiver)
    else {
        // A view local's `.len` observes the extent its establishment stored,
        // exactly as a whole view parameter's does.
        let (symbol, _) = view_local_root(program, parameters, member.receiver)?;
        return Some(CheckedScalarExpression::StructuralParameterByteLength {
            root: checked_trees::CheckedStorageRoot::ViewLocal { symbol },
            path: Vec::new(),
        });
    };
    // A pure parameter-rooted array projection has a type-owned extent. Do
    // not strip a constrained carrier: its live length can differ from its
    // backing capacity. Dynamic byte views retain their runtime read below.
    let mut extent_type = selected_type;
    while let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(extent_type)
    {
        extent_type = *referee;
    }
    if let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(extent_type)
    {
        let length = u64::try_from(*length).ok()?;
        let literal = numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            &length.to_string(),
        )
        .ok()?
        .with_landing(numerics::literals::IntegerLanding {
            landed_type: numerics::literals::LandedIntegerType::U64,
            domain: ArithmeticDomain::Exact,
        });
        return Some(CheckedScalarExpression::IntegerLiteral { literal });
    }
    if path.is_empty() {
        let TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable,
            ..
        } = program.type_reference_table.type_reference(
            parameters
                .get(usize::try_from(parameter_position).ok()?)?
                .type_reference,
        )
        else {
            return None;
        };
        // Any element family keeps a runtime extent on its view descriptor:
        // byte views count bytes, element views count elements, record
        // elements included.
        let TypeReferenceNode::Slice { .. } = program.type_reference_table.type_reference(*referee)
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
        // `.len` reads the same live extent.
        || !matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(program, selected_type, &[]),
            Some(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView
            )
        )
    {
        return None;
    }
    Some(CheckedScalarExpression::StructuralParameterByteLength {
        root: checked_trees::CheckedStorageRoot::Parameter {
            index: parameter_position,
        },
        path,
    })
}

/// The `let` symbol and declared type of an immutable borrowed view local
/// that `expression` names whole, when that name is not a state parameter. The observation only
/// names the local: the plan that sequences the body decides whether the
/// local's view was established before this use, and lowering resolves the
/// symbol to its published place or refuses.
pub(super) fn view_local_root(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
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
    let mut locals = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.symbol == path.symbol =>
            {
                Some(local)
            }
            _ => None,
        });
    let local = locals.next()?;
    (locals.next().is_none()
        && !local.is_mutable
        && crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            local.type_reference,
        ))
    .then_some((path.symbol, local.type_reference))
}

/// Empty spelling resolution is builtin only when the exact operand types
/// independently carry the builtin index meaning.
pub(super) fn indexed_read_is_builtin(
    program: &TypedTrees,
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
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    collection_type: TypeReferenceHandle,
    index: ExpressionHandle,
) -> bool {
    use language_core::OperatorSpelling;
    let Some((machine, state)) = program.machines().iter().find_map(|machine| {
        program.machine_states(machine).iter().find_map(|state| {
            let authored = program.state_parameters(state);
            (authored.len() == parameters.len()
                && authored
                    .iter()
                    .zip(parameters)
                    .all(|(left, right)| left.symbol == right.symbol))
            .then_some((machine, state))
        })
    }) else {
        return false;
    };
    let operands = [
        Some(collection_type),
        validation::declared_place_type_raw(program, machine, Some(state), index),
    ];
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Index,
        &operands,
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            OperatorSpelling::Index,
            &operands,
        )
}

pub(super) fn structural_parameter_field_path(
    program: &TypedTrees,
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
                let (position, _) = exact_self_parameter(program, parameters, expression)?;
                return u32::try_from(position).ok();
            }
            let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                program
                    .expression_table
                    .name_path_member_symbols(name.member_symbols)
                    .iter()
                    .copied()
                    .find(|symbol| symbol.is_valid())
            });
            let name_text = program
                .expression_table
                .name_path_members(name.members)
                .last();
            parameters
                .iter()
                .position(|parameter| {
                    if let Some(symbol) = name_symbol {
                        parameter.symbol == symbol
                    } else {
                        name_text.is_some_and(|name| parameter.name == *name)
                    }
                })
                .and_then(|position| u32::try_from(position).ok())
        }
        ExpressionNode::Member(member) => {
            let parameter =
                structural_parameter_field_path(program, parameters, member.receiver, fields)?;
            let field_identity = |field: &typed_trees::data::DataField| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            };
            if parameters.get(usize::try_from(parameter).ok()?)?.is_self
                && matches!(
                    program.expression_table.expression(member.receiver),
                    ExpressionNode::Name(_)
                )
            {
                // A rejected self alias cannot fall through to a global field
                // search: only the attached declaration owns this projection.
                if !member.member_symbol.is_valid()
                    || program.symbols.get(member.member_symbol).kind != symbols::SymbolKind::Field
                {
                    return None;
                }
                let (_, machine) = exact_self_parameter(program, parameters, member.receiver)?;
                let field = validation::exact_self_field(program, machine, expression)?;
                if field.relevance.is_erased() {
                    return None;
                }
                fields.push(CheckedStructuralPredicatePathSegment::Field(
                    field_identity(field),
                ));
                return Some(parameter);
            }
            if let Some(case_name) = &member.case_variant {
                let (case, field) = program.data_definitions().iter().find_map(|data| {
                    program.data_members(data).iter().find_map(|candidate| {
                        let typed_trees::data::DataMember::Variant(variant) = candidate else {
                            return None;
                        };
                        if variant.name != *case_name {
                            return None;
                        }
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.symbol == member.member_symbol)
                            .map(|field| {
                                (
                                    variant
                                        .identity
                                        .map(|identity| format!("#{identity}"))
                                        .unwrap_or_else(|| variant.name.as_str().to_owned()),
                                    field_identity(field),
                                )
                            })
                    })
                })?;
                fields.push(CheckedStructuralPredicatePathSegment::Case(case));
                fields.push(CheckedStructuralPredicatePathSegment::Field(field));
            } else {
                let identity = if member.member_symbol.is_valid() {
                    program.data_definitions().iter().find_map(|data| {
                        program.data_members(data).iter().find_map(|candidate| {
                            let typed_trees::data::DataMember::Field(field) = candidate else {
                                return None;
                            };
                            (field.symbol == member.member_symbol).then(|| field_identity(field))
                        })
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

fn exact_self_parameter<'program>(
    program: &'program TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(usize, &'program typed_trees::machine::Machine)> {
    use symbols::SymbolKind;

    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || program.symbols.get(name.symbol).kind != SymbolKind::Machine
        || !matches!(program.expression_table.name_path_members(name.members),
            [spelling] if spelling.is_self_receiver())
    {
        return None;
    }
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == name.symbol);
    let machine = machines.next()?;
    if machines.next().is_some() {
        return None;
    }
    // `self` is declared on every state of the machine with a distinct
    // parameter symbol, so the caller's parameter list names exactly one
    // authoring state — not necessarily the entry state.
    let state = program.machine_states(machine).iter().find(|state| {
        let state_symbol = program.symbols.get(state.symbol);
        state_symbol.kind == SymbolKind::State
            && state_symbol.parent == machine.symbol
            && program.state_parameters(state) == parameters
    })?;
    let mut receivers = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self);
    let (position, parameter) = receivers.next()?;
    let parameter_symbol = program.symbols.get(parameter.symbol);
    if receivers.next().is_some()
        || parameter.is_const
        || !parameter.name.is_self_receiver()
        || parameter_symbol.kind != SymbolKind::Parameter
        || parameter_symbol.parent != state.symbol
        || program.symbols.name(parameter.symbol) != parameter.name.as_str()
        || parameters
            .iter()
            .filter(|candidate| candidate.symbol == parameter.symbol)
            .count()
            != 1
    {
        return None;
    }
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|owner| owner.symbol == machine.attached_data_symbol);
    let owner = owners.next()?;
    if owners.next().is_some()
        || program.symbols.get(owner.symbol).kind != SymbolKind::Data
        || program.symbols.name(owner.symbol) != owner.name.as_str()
        || machine.attached_data.as_ref() != Some(&owner.name)
    {
        return None;
    }
    let mut reference = parameter.type_reference;
    for _ in 0..64 {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, name }
                if (*symbol == machine.symbol && name.as_str() == "Self")
                    || (*symbol == owner.symbol && *name == owner.name) =>
            {
                return Some((position, machine));
            }
            _ => return None,
        }
    }
    None
}

/// The resolved path is `frozen` when the root parameter and every
/// intermediate receiver are immutable: no `mut` formal and no exclusive
/// reference. Declared constraints on frozen storage are read invariants;
/// mutable storage can be reborrowed through a pointee type that drops them,
/// so only the live snapshots speak for it.
pub(crate) fn resolve_structural_parameter_path(
    program: &TypedTrees,
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
    let mut receiver = parameter.type_reference;
    let mut frozen = !parameter.is_mutable && !exclusive_reference(program, receiver);
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
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
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
                            let typed_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };
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
        }
        if exclusive_reference(program, receiver) {
            frozen = false;
        }
    }
    selected_case
        .is_none()
        .then_some((parameter.symbol, segments, receiver, frozen))
}

/// Whether this reference resolves through an exclusive borrow (`&mut`/`&w`).
/// Shared references keep their referent frozen for the borrow's lifetime.
pub(crate) fn exclusive_reference(
    program: &TypedTrees,
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

pub(super) fn fixed_index_element_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    element_index: u64,
) -> Option<TypeReferenceHandle> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return None;
                };
                return (element_index < u64::try_from(*length).ok()?).then_some(*element_type);
            }
            _ => return None,
        }
    }
}

pub(crate) fn structural_data(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let (symbol, name) = loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: name,
                ..
            } => break (*symbol, name),
            _ => return None,
        }
    };
    let symbol = program
        .machines()
        .iter()
        .find(|machine| {
            symbol.is_valid() && machine.symbol == symbol && machine.attached_data_symbol.is_valid()
        })
        .map_or(symbol, |machine| machine.attached_data_symbol);
    program.data_definitions().iter().find(|definition| {
        if symbol.is_valid() {
            definition.symbol == symbol
        } else {
            definition.name == *name
        }
    })
}

pub(super) fn lower_structural_parameter_field(
    program: &TypedTrees,
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
    if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
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
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    u32,
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
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
        if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(authored) {
            let (_, _, collection_type) =
                structural_parameter_place(program, parameters, indexed.collection)?;
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
        let ExpressionNode::Member(member) = program.expression_table.expression(authored) else {
            break;
        };
        let selected = selected_fields.next()?;
        if member.member_symbol.is_valid() && selected != member.member_symbol {
            // Attached machines retain inherited field symbols; canonical
            // places identify the original data declaration's storage.
            let ExpressionNode::Name(receiver) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let mut machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == receiver.symbol);
            let machine = machines.next()?;
            if machines.next().is_some() {
                return None;
            }
            let field = validation::exact_self_field(program, machine, authored)?;
            if field.symbol != selected {
                return None;
            }
        }
        authored = member.receiver;
    }
    let root = crate::flow::normalized_event_place_root(program, place.root);
    let facts::PlaceRoot::Symbol(_) = root else {
        return None;
    };
    let parameter_position = parameters.iter().position(|parameter| {
        crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(parameter.symbol),
        ) == root
    })?;
    let mut path = Vec::new();
    for segment in &place.segments {
        path.push(match segment {
            facts::PlaceSegment::Field { symbol } => {
                let field = program.data_definitions().iter().find_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            typed_trees::data::DataMember::Variant(variant) => program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == *symbol),
                            _ => None,
                        })
                })?;
                CheckedStructuralPredicatePathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                )
            }
            facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate)
                    })
                })?;
                CheckedStructuralPredicatePathSegment::Case(
                    case.identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| case.name.as_str().to_owned()),
                )
            }
            facts::PlaceSegment::FixedIndex { index } => {
                CheckedStructuralPredicatePathSegment::FixedIndex(u64::try_from(*index).ok()?)
            }
            _ => return None,
        });
    }
    let parameter_position = u32::try_from(parameter_position).ok()?;
    let (resolved_root, segments, type_reference, _) =
        resolve_structural_parameter_path(program, parameters, parameter_position, &path)?;
    if crate::flow::normalized_event_place_root(program, facts::PlaceRoot::Symbol(resolved_root))
        != root
        || segments != place.segments
    {
        return None;
    }
    Some((parameter_position, path, type_reference))
}

/// The static field path inside one element of type `element`, spelled by
/// each field's declared identity exactly as a parameter field path is: every
/// segment is a non-erased field of the record the previous step reached.
/// Returns the path and the leaf's declared type.
pub(super) fn element_field_path(
    program: &TypedTrees,
    element: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<(
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    let mut current = element;
    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let owner = loop {
            match program.type_reference_table.type_reference(current) {
                TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    break program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.symbol == *symbol)?;
                }
                _ => return None,
            }
        };
        let field = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                    Some(field)
                }
                _ => None,
            })?;
        if field.relevance.is_erased() {
            return None;
        }
        path.push(CheckedStructuralPredicatePathSegment::Field(
            field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
        ));
        current = field.type_reference;
    }
    Some((path, current))
}
