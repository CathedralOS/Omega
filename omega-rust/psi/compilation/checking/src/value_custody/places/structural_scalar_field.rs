//! Exact declaration custody for scalar observations of structural values.
//!
//! Named locals and saved expression results share the same declaration walk.
//! A saved dispatch subject retains its authored occurrence until execution
//! planning assigns storage; looking up a field neither executes that subject
//! nor grants its case membership or ownership. A fresh receiver retains its
//! unevaluated value occurrence so planning must establish it before observation.
//! These remain use-site checks, not permissions inferred from field geometry.
//! Closed generic receivers use their retained specialized declaration at every
//! projection. Its fields already carry substituted nested types and extents;
//! reading the template instead would either reject legal observations or lose
//! the selected instance's scalar type and arithmetic policy.
//! A scalar read lends established storage; it does not copy or consume the
//! containing owner. Linear claims therefore stay on that owner's ordinary
//! custody path, just as neighboring reference permissions do. Rejecting an
//! owner by multiplicity here would confuse observation with whole-value
//! transfer and prevent reading a detached linear field before consuming it.
//! Incoming storage and local storage use that same declaration walk. The
//! last field may itself hold a readable reference: its scalar observation
//! follows the descriptor, rather than interpreting pointer bits as a value.
//! Record qualification changes the owner's theory, not its field namespace.
//! Resolve members beneath that shell while retaining the qualified source
//! type. This read establishes no membership and discharges no owner claim.

use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Source identities only; this does not grant ownership or establish loan
/// validity. Checked flow and the consuming replay still validate availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralScalarField {
    pub root: facts::PlaceRoot,
    /// A fresh receiver that must be evaluated before the read. Zero means the
    /// receiver names already established storage, including a saved subject.
    pub receiver_value: ExpressionHandle,
    pub statement_ordinal: u32,
    /// Whole storage declaration, before any field projection.
    pub type_reference: TypeReferenceHandle,
    /// Data declaration containing the final scalar field or selected payload,
    /// or the fixed array holding a final scalar element.
    pub carrier_type_reference: TypeReferenceHandle,
    /// Declared case and carrier-field identities, literal fixed-array
    /// selections and stored-reference crossings in authored order, excluding
    /// the final scalar field. A final literal element is the path's last
    /// `FixedIndex` instead.
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    /// The final scalar field. Zero when the path itself ends at a scalar
    /// element of a fixed array; the identity is then the default.
    pub field: SymbolHandle,
    pub field_identity: checked_trees::CheckedMemberIdentity,
    pub primitive_type: PrimitiveType,
    /// Governs subsequent arithmetic; observing storage does not erase policy.
    pub domain: ArithmeticDomain,
}

/// Resolve declaration-scoped observations under an explicit input telescope.
/// This is the same field geometry query as an executable use-site read, but
/// the caller owns the binder substitution and arrival availability. It grants
/// neither an additional read permission nor a fresh runtime formal.
pub fn structural_scalar_field_in_parameters(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    bindings: &[(super::CanonicalPlace, super::CanonicalPlace)],
    expression: ExpressionHandle,
) -> Option<StructuralScalarField> {
    let coordinate = super::canonical_place_from_expression(program, expression)?;
    let mut rebased = None;
    for (formal, actual) in bindings {
        if formal.root != coordinate.root {
            continue;
        }
        let Some(suffix) = coordinate.segments.strip_prefix(formal.segments.as_slice()) else {
            continue;
        };
        let mut segments = actual.segments.clone();
        segments.extend_from_slice(suffix);
        let actual = super::CanonicalPlace {
            root: actual.root.clone(),
            segments,
        };
        if rebased.as_ref().is_some_and(|selected| selected != &actual) {
            return None;
        }
        rebased = Some(actual);
    }
    let coordinate = rebased.unwrap_or(coordinate);
    let facts::PlaceRoot::Symbol(parameter) = coordinate.root else {
        return None;
    };
    structural_scalar_field_at_coordinate(program, parameters, parameter, &coordinate.segments)
}

/// Observe an already selected formal-relative coordinate. The caller owns
/// the parameter snapshot and coordinate admission; this shared declaration
/// walk only elaborates its ordinary field geometry and scalar policy. No
/// source expression or same-typed field search supplies the coordinate.
pub fn structural_scalar_field_at_coordinate(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    parameter: SymbolHandle,
    coordinate: &[facts::PlaceSegment],
) -> Option<StructuralScalarField> {
    let mut matches = parameters.iter().filter(|input| {
        input.symbol == parameter && !input.is_const && !input.relevance.is_erased()
    });
    let input = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let source_type =
        match program
            .type_reference_table
            .type_reference(crate::peel_declared_constraints(
                program,
                input.type_reference,
            )) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => *referee,
            TypeReferenceNode::Reference { .. } => return None,
            _ => input.type_reference,
        };
    let mut carrier = source_type;
    let mut path = Vec::new();
    let mut selected_case = SymbolHandle::invalid();
    for (ordinal, segment) in coordinate.iter().enumerate() {
        carrier = readable_projection_carrier(program, carrier, &mut path)?;
        match segment {
            facts::PlaceSegment::Case { variant } => {
                if selected_case.is_valid() {
                    return None;
                }
                let record = program.closed_data_definition(carrier)?;
                let mut cases = program
                    .data_members(record)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .filter(|case| case.symbol == *variant);
                let case = cases.next()?;
                if cases.next().is_some() {
                    return None;
                }
                selected_case = case.symbol;
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Case(
                    checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ),
                ));
            }
            facts::PlaceSegment::Field { symbol } => {
                let record = program.closed_data_definition(carrier)?;
                let fields = if selected_case.is_valid() {
                    let case = program
                        .data_members(record)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|case| case.symbol == selected_case)?;
                    program.data_payload_fields(case).iter().collect::<Vec<_>>()
                } else {
                    program
                        .data_members(record)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                };
                let mut fields = fields.into_iter().filter(|field| field.symbol == *symbol);
                let field = fields.next()?;
                if fields.next().is_some()
                    || field.relevance.is_erased()
                    || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
                    || program.symbols.get(field.symbol).parent
                        != if selected_case.is_valid() {
                            selected_case
                        } else {
                            record.symbol
                        }
                {
                    return None;
                }
                if ordinal + 1 == coordinate.len() {
                    let primitive_type = realized_scalar_carrier(program, field.type_reference)?;
                    return Some(StructuralScalarField {
                        root: facts::PlaceRoot::Symbol(parameter),
                        receiver_value: ExpressionHandle::invalid(),
                        statement_ordinal: 0,
                        type_reference: source_type,
                        carrier_type_reference: carrier,
                        path,
                        field: field.symbol,
                        field_identity: checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                        primitive_type,
                        domain: program.arithmetic_domain_for_type_reference(
                            program
                                .type_reference_table
                                .dereferenced_type_reference(field.type_reference),
                        ),
                    });
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                carrier = field.type_reference;
                selected_case = SymbolHandle::invalid();
            }
            facts::PlaceSegment::FixedIndex { index } if !selected_case.is_valid() => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                if index >= length {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
                carrier = *element_type;
            }
            _ => return None,
        }
    }
    None
}

/// Resolve a field path below established storage or a fresh receiver value.
/// Established storage is observed here, never equated with its initializer: member
/// writes need not rebind the local, so absence of `mut` does not freeze fields.
/// The caller must bind the expression to this authored statement's evaluation
/// graph. Open generic owners, erased fields and scalar qualifications
/// without a complete supported predicate require additional facts. Arithmetic
/// policy changes operations, not storage: retain it alongside the observation.
/// Restrictions stay on the independently reconstructed field declaration;
/// construction and every store prove them. This query grants no membership:
/// observing the established field reads its scalar carrier.
pub fn structural_scalar_field(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, ExpressionHandle)],
    mut source: impl FnMut(ExpressionHandle) -> Option<StructuralSource>,
) -> Option<StructuralScalarField> {
    let mut steps = Vec::new();
    let mut receiver = expression;
    let mut visits = 0;
    loop {
        visits += 1;
        if visits > program.expression_table.expression_count() {
            return None;
        }
        match program.expression_table.expression(receiver) {
            ExpressionNode::Name(name)
                if name.head_symbol == name.symbol
                    && program
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1
                    && bindings.iter().any(|(symbol, _)| *symbol == name.symbol) =>
            {
                let mut selected = bindings.iter().filter(|(symbol, _)| *symbol == name.symbol);
                let (_, actual) = selected.next()?;
                if selected.next().is_some() || *actual == receiver {
                    return None;
                }
                receiver = *actual;
                if steps.len() + 1 >= program.expression_table.expression_count() {
                    return None;
                }
            }
            ExpressionNode::Member(member) => {
                steps.push(Step::Member(member));
                receiver = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                steps.push(Step::Element {
                    occurrence: receiver,
                    index: indexed.index,
                });
                receiver = indexed.collection;
            }
            _ => break,
        }
    }
    let leaf = steps.first().copied()?;
    let local = source(receiver)?;
    // A literal element selection is a coordinate only when `[]` keeps its
    // builtin meaning along the whole place spine. A fresh receiver's spine
    // starts at a value, which names no place type: each of its selections is
    // authenticated at the collection carrier this walk reaches instead.
    let fresh = local.receiver_value.is_valid();
    let scope = scope(program, machine, state);
    if !fresh
        && steps
            .iter()
            .any(|step| matches!(step, Step::Element { .. }))
        && !scope.is_some_and(|(machine, state)| {
            super::place_has_builtin_coordinates(program, machine, Some(state), expression)
        })
    {
        return None;
    }
    let value_coordinate = |occurrence, collection| {
        !fresh
            || scope.is_some_and(|(machine, state)| {
                super::value_element_has_builtin_coordinate(
                    program,
                    machine,
                    Some(state),
                    occurrence,
                    collection,
                )
            })
    };
    let mut carrier = local.type_reference;
    let mut path = Vec::new();
    // Every step is resolved under its own declaration. Reusing only the final
    // field spelling would confuse identically shaped children of this root.
    for step in steps.iter().rev().take(steps.len() - 1) {
        carrier = readable_projection_carrier(program, carrier, &mut path)?;
        let member = match step {
            Step::Member(member) => member,
            // The checker's place facts fold a constant selector to its
            // element; a runtime selector is not a field path.
            Step::Element { occurrence, index } => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                if !value_coordinate(*occurrence, carrier) {
                    return None;
                }
                let index = program
                    .expression_table
                    .constant_integer_value(*index)
                    .and_then(|index| u64::try_from(index).ok())?;
                if usize::try_from(index).ok()? >= *length {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                carrier = *element_type;
                continue;
            }
        };
        let declaration_carrier = crate::peel_declared_constraints(program, carrier);
        let record = program.closed_data_definition(declaration_carrier)?;
        let field = selected_member_field(program, declaration_carrier, record, member, &mut path)?;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
        ));
        carrier = field.type_reference;
    }
    carrier = readable_projection_carrier(program, carrier, &mut path)?;
    let leaf = match leaf {
        Step::Member(leaf) => leaf,
        // A literal element of a scalar array is the observation itself: the
        // same constant selection an intermediate step takes, ending at its
        // element's declared scalar carrier.
        Step::Element { occurrence, index } => {
            let TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(carrier)
            else {
                return None;
            };
            if !value_coordinate(occurrence, carrier) {
                return None;
            }
            let index = program
                .expression_table
                .constant_integer_value(index)
                .and_then(|index| u64::try_from(index).ok())?;
            if usize::try_from(index).ok()? >= *length {
                return None;
            }
            let primitive_type =
                observable_scalar(realized_scalar_carrier(program, *element_type)?)?;
            path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                index,
            ));
            return Some(StructuralScalarField {
                root: local.root,
                receiver_value: local.receiver_value,
                statement_ordinal: local.statement_ordinal,
                type_reference: local.type_reference,
                carrier_type_reference: carrier,
                path,
                field: SymbolHandle::invalid(),
                field_identity: checked_trees::CheckedMemberIdentity::default(),
                primitive_type,
                domain: program.arithmetic_domain_for_type_reference(
                    program
                        .type_reference_table
                        .dereferenced_type_reference(*element_type),
                ),
            });
        }
    };
    let declaration_carrier = crate::peel_declared_constraints(program, carrier);
    let record = program.closed_data_definition(declaration_carrier)?;
    let field = selected_member_field(program, declaration_carrier, record, leaf, &mut path)?;
    let primitive_type =
        observable_scalar(realized_scalar_carrier(program, field.type_reference)?)?;
    Some(StructuralScalarField {
        root: local.root,
        receiver_value: local.receiver_value,
        statement_ordinal: local.statement_ordinal,
        type_reference: local.type_reference,
        carrier_type_reference: carrier,
        path,
        field: field.symbol,
        field_identity: checked_trees::CheckedMemberIdentity::of_member(
            field.identity,
            field.name.clone(),
        ),
        primitive_type,
        domain: program.arithmetic_domain_for_type_reference(
            program
                .type_reference_table
                .dereferenced_type_reference(field.type_reference),
        ),
    })
}

/// Observable scalar data retains its declared carrier. Reading an address
/// observes inert geometry; it neither converts it to a count nor lends the
/// storage that its bits may describe.
fn observable_scalar(primitive_type: PrimitiveType) -> Option<PrimitiveType> {
    matches!(
        primitive_type,
        PrimitiveType::Bool
            | PrimitiveType::Addr
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::F32
            | PrimitiveType::F64
            | PrimitiveType::Selector
    )
    .then_some(primitive_type)
}

/// Stored descriptors contribute explicit crossings to the checked path. Root
/// borrowed storage is already peeled by `structural_source`; retaining only
/// these declared crossings lets lowering follow the plan without rediscovering
/// reference access from the authored expression.
fn readable_projection_carrier(
    program: &dyn crate::CheckingInput,
    mut carrier: TypeReferenceHandle,
    path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
) -> Option<TypeReferenceHandle> {
    for _ in 0..program.type_reference_table.type_reference_count() {
        // Qualifications do not authorize a descriptor crossing. Keep the
        // declared access check, and retain qualifications on non-reference
        // leaves for the independent predicate obligations.
        match program
            .type_reference_table
            .type_reference(crate::peel_declared_constraints(program, carrier))
        {
            TypeReferenceNode::Reference {
                referee,
                access:
                    language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable,
                ..
            } => {
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                carrier = *referee;
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => return Some(carrier),
        }
    }
    None
}

/// Select under the current receiver declaration, retaining the case edge
/// before its payload field. A selected case is a path coordinate, not a
/// synthetic record type; execution still needs its established case.
fn selected_member_field<'program>(
    program: &'program dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    record: &'program typed_trees::data::DataDefinition,
    member: &typed_trees::expression::TableMemberExpression,
    path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
) -> Option<&'program typed_trees::data::DataField> {
    let field = if record.generic_instance.is_some() && member.case_variant.is_none() {
        // Normalization may retain a template field selection. Translate it
        // only through this receiver's exact instance/member correspondence;
        // a sibling instance's field cannot be selected by equal spelling.
        program.closed_data_member_field(carrier, member.member_symbol, member.member.as_str())?
    } else {
        super::exact_data_member_field(
            program,
            record,
            member.member_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|variant| variant.as_str()),
        )?
    };
    if field.relevance.is_erased()
        || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
    {
        return None;
    }
    let owner = if let Some(case_name) = &member.case_variant {
        let mut variants = program.data_members(record).iter().filter_map(|member| {
            let variant = member.as_variant()?;
            (variant.name == *case_name).then_some(variant)
        });
        let variant = variants.next()?;
        if variants.next().is_some()
            || !variant.symbol.is_valid()
            || program.symbols.get(variant.symbol).kind != symbols::SymbolKind::Variant
            || program.symbols.get(variant.symbol).parent != record.symbol
            || program
                .data_payload_fields(variant)
                .iter()
                .filter(|candidate| candidate.symbol == field.symbol)
                .count()
                != 1
        {
            return None;
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Case(
            checked_trees::CheckedMemberIdentity::of_member(variant.identity, variant.name.clone()),
        ));
        variant.symbol
    } else {
        if program
            .data_members(record)
            .iter()
            .filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol))
            .count()
            != 1
        {
            return None;
        }
        record.symbol
    };
    (program.symbols.get(field.symbol).parent == owner).then_some(field)
}

/// One projection between the local root and the observed scalar field.
#[derive(Clone, Copy)]
enum Step<'a> {
    Member(&'a typed_trees::expression::TableMemberExpression),
    Element {
        occurrence: ExpressionHandle,
        index: ExpressionHandle,
    },
}

/// The exact authored machine and state, when each symbol names exactly one:
/// the scope in which a selection's builtin coordinate meaning is decided.
fn scope(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|owner| owner.symbol == machine);
    let (Some(machine), None) = (machines.next(), machines.next()) else {
        return None;
    };
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|owner| owner.symbol == state);
    let (Some(state), None) = (states.next(), states.next()) else {
        return None;
    };
    Some((machine, state))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralSource {
    pub root: facts::PlaceRoot,
    pub receiver_value: ExpressionHandle,
    pub statement_ordinal: u32,
    pub type_reference: TypeReferenceHandle,
}

/// Rejoin a runtime expression to this exact authored statement. This does not
/// replace selective execution replay or grant validity to an unevaluated arm.
fn expression_at_statement(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let mut pending =
        crate::machine_calls::calls::statement_value_expression_roots(program, statement);
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if handle == expression {
            return true;
        }
        if !program.expression_table.expression_is_valid(handle) || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        crate::expression_traversal::children(
            program,
            program.expression_table.expression(handle),
            |child| pending.push(child),
        );
    }
    false
}

/// Rejoin a copied dispatch subject to the first arm that evaluates it.
/// The argument is the receiver expression, not its payload projection.
/// Identical spelling alone is insufficient: separately authored calls retain
/// different selection occurrences even when their arguments are identical.
/// This describes source identity; a consumer must still prove the unique
/// producing operation completed before reading its established storage.
pub fn saved_dispatch_subject(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression)
        || !matches!(table.expression(expression), ExpressionNode::Call(_))
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement = statements.get(statement_index)?;
    if !expression_at_statement(program, statement, expression) {
        return None;
    }
    let same_subject = |subject| same_authored_call(program, subject, expression);
    if !same_subject(dispatch_case_subject(program, statement)?) {
        return None;
    }
    statements[..=statement_index]
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| {
            let subject = dispatch_case_subject(program, statement)?;
            same_subject(subject).then_some((ordinal, subject))
        })
}

/// Rejoin a call anywhere beneath the saved subject to its canonical
/// evaluation, retaining its exact position inside that subject's tree.
/// Guards and payload receivers can contain separate copies of the entire
/// tree. Every copied call keeps its authored selection occurrence; a fresh
/// equal-looking call in an arm does not become a saved observation.
///
/// A shared expression handle may also occur on an independently evaluated
/// edge. Track containment per path, and decline that ambiguity rather than
/// letting a copied occurrence erase the independent edge's effects.
pub fn saved_dispatch_call(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression)
        || !matches!(table.expression(expression), ExpressionNode::Call(_))
    {
        return None;
    }
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let mut pending =
        crate::machine_calls::calls::statement_value_expression_roots(program, statement)
            .into_iter()
            .map(|node| (node, None))
            .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut found = None;
    while let Some((node, inherited)) = pending.pop() {
        if node == ExpressionHandle::invalid() {
            if inherited.is_some_and(|(_, counterpart)| counterpart != ExpressionHandle::invalid())
            {
                return None;
            }
            continue;
        }
        if !table.expression_is_valid(node) {
            return None;
        }
        let canonical =
            inherited.or_else(|| saved_dispatch_subject(program, state, statement_index, node));
        if visited.contains(&(node, canonical)) {
            continue;
        }
        visited.push((node, canonical));
        if let Some((_, counterpart)) = canonical
            && (!table.expression_is_valid(counterpart)
                || !table.expressions_structurally_equal(node, counterpart)
                || (matches!(table.expression(node), ExpressionNode::Call(_))
                    && !same_authored_call(program, node, counterpart)))
        {
            return None;
        }
        if node == expression {
            let canonical = canonical?;
            if found.is_some_and(|prior| prior != canonical) {
                return None;
            }
            found = Some(canonical);
            continue;
        }
        let mut children = Vec::new();
        crate::expression_traversal::children(program, table.expression(node), |child| {
            children.push(child);
        });
        if let Some((ordinal, counterpart)) = canonical {
            let mut counterparts = Vec::new();
            crate::expression_traversal::children(
                program,
                table.expression(counterpart),
                |child| {
                    counterparts.push(child);
                },
            );
            if children.len() != counterparts.len() {
                return None;
            }
            pending.extend(
                children
                    .into_iter()
                    .zip(counterparts)
                    .map(|(child, counterpart)| (child, Some((ordinal, counterpart)))),
            );
        } else {
            pending.extend(children.into_iter().map(|child| (child, None)));
        }
    }
    found
}

fn same_authored_call(
    program: &dyn crate::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let table = &program.expression_table;
    if !matches!(table.expression(left), ExpressionNode::Call(_))
        || !matches!(table.expression(right), ExpressionNode::Call(_))
        || !table.expressions_structurally_equal(left, right)
    {
        return false;
    }
    let mut occurrences = table.authored_selection_occurrences(left).peekable();
    occurrences.peek().is_some() && occurrences.eq(table.authored_selection_occurrences(right))
}

fn dispatch_case_subject(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
) -> Option<ExpressionHandle> {
    use typed_trees::expression::BinaryOperator;
    use typed_trees::statement::TransitionGuardNode;
    let transition = statement.as_transition()?;
    let TransitionGuardNode::When(mut guard) = transition.guard else {
        return None;
    };
    let table = &program.expression_table;
    for _ in 0..table.expression_count() {
        let Some(binary) = table.binary_node(guard) else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return None;
        }
        if binary.operator == BinaryOperator::Equal {
            if matches!(table.expression(binary.left), ExpressionNode::Boolean(true)) {
                guard = binary.right;
                continue;
            }
            if matches!(
                table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                guard = binary.left;
                continue;
            }
        }
        if crate::exact_case_reference_owner(program, binary.right).is_some() {
            return Some(binary.left);
        }
        if binary.operator == BinaryOperator::Equal
            && crate::exact_case_reference_owner(program, binary.left).is_some()
        {
            return Some(binary.right);
        }
        return None;
    }
    None
}

pub fn structural_source(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
    receiver_expression: ExpressionHandle,
) -> Option<StructuralSource> {
    if !machine.is_valid()
        || !state.is_valid()
        || !program.expression_table.expression_is_valid(expression)
    {
        return None;
    }
    let mut machines = program
        .machines()
        .iter()
        .filter(|owner| owner.symbol == machine);
    let machine = machines.next()?;
    if machines.next().is_some() {
        return None;
    }
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|owner| owner.symbol == state);
    let state = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    if !expression_at_statement(
        program,
        statements.get(statement_ordinal as usize)?,
        expression,
    ) {
        return None;
    }
    let (root, receiver_value, ordinal, source_type) =
        match program.expression_table.expression(receiver_expression) {
            ExpressionNode::Name(receiver) => {
                if !receiver.symbol.is_valid()
                    || receiver.head_symbol != receiver.symbol
                    || program
                        .expression_table
                        .name_path_members(receiver.members)
                        .len()
                        != 1
                    || program
                        .expression_table
                        .name_path_member_symbols(receiver.member_symbols)
                        != [receiver.symbol]
                {
                    return None;
                }
                let parameter = program.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == receiver.symbol
                        && !parameter.is_const
                        && !parameter.relevance.is_erased()
                });
                let local = program.state_local_declaration(state, receiver.symbol);
                let (ordinal, source_type) = match (parameter, local) {
                    (Some(parameter), None) => (0, parameter.type_reference),
                    (None, Some((ordinal, local))) => {
                        // This query identifies storage, not the value established
                        // in it. A reserved home can acquire valid fields through
                        // later writes; observation checking owns their current
                        // validity and custody at this use. Requiring a declaration
                        // initializer or a zero-valid whole type loses that home.
                        if ordinal >= statement_ordinal as usize {
                            return None;
                        }
                        (ordinal, local.type_reference)
                    }
                    _ => return None,
                };
                (
                    facts::PlaceRoot::Symbol(receiver.symbol),
                    ExpressionHandle::invalid(),
                    ordinal,
                    source_type,
                )
            }
            _ => {
                if let Some((ordinal, subject)) = saved_dispatch_subject(
                    program,
                    state,
                    statement_ordinal as usize,
                    receiver_expression,
                ) {
                    let result_type =
                        crate::expression_result_type_reference(program, machine, state, subject)?;
                    (
                        facts::PlaceRoot::Expression(subject),
                        ExpressionHandle::invalid(),
                        ordinal,
                        result_type,
                    )
                } else {
                    let result_type = crate::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        receiver_expression,
                    )?;
                    (
                        facts::PlaceRoot::Expression(receiver_expression),
                        receiver_expression,
                        statement_ordinal as usize,
                        result_type,
                    )
                }
            }
        };
    // Borrowed storage observes its referent without moving its reference.
    // An expression result must already own its home before it can be read.
    let carrier = match program
        .type_reference_table
        .type_reference(crate::peel_declared_constraints(program, source_type))
    {
        TypeReferenceNode::Reference {
            referee,
            access:
                language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable,
            ..
        } if matches!(root, facts::PlaceRoot::Symbol(_)) => *referee,
        _ => source_type,
    };
    // A scalar observation moves neither its record nor neighboring reference
    // permissions. Establishment and use-site custody are checked separately;
    // a whole-content transfer classifier is not a storage-read judgment.
    // The owner's claims remain live: observing one scalar neither transfers
    // the owner nor proves that its disposal obligation has been discharged.
    // A completed reference result owns a descriptor home. Validate the
    // selected record through its declared readable crossings while retaining
    // the descriptor type so observation emits those crossings explicitly.
    let record_carrier = readable_projection_carrier(program, carrier, &mut Vec::new())?;
    let record = program
        .closed_data_definition(crate::peel_declared_constraints(program, record_carrier))?;
    if !record.symbol.is_valid()
        || program.symbols.get(record.symbol).kind != symbols::SymbolKind::Data
    {
        return None;
    }
    Some(StructuralSource {
        root,
        receiver_value,
        statement_ordinal: u32::try_from(ordinal).ok()?,
        type_reference: carrier,
    })
}

/// Policy-only integer fields use their ordinary scalar storage, while the
/// returned observation separately retains their arithmetic policy. Closed
/// ranges and exact declared intervals share the bounded-field storage contract;
/// complete scalar predicates retain their declaration invariant. Unsupported
/// predicates and restricted non-Exact policies remain unsupported.
fn realized_scalar_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    // A readable reference is a storage crossing, not a scalar declaration.
    // Keep write-only access fenced here as at every intermediate projection.
    for _ in 0..program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee,
                access:
                    language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable,
                ..
            } => reference = *referee,
            TypeReferenceNode::Reference { .. } => return None,
            _ => break,
        }
    }
    if crate::is_arithmetic_policy_only_integer(program, reference) {
        return program.primitive_type_reference(reference);
    }
    let primitive = program.primitive_type_reference(reference)?;
    let mut bounded = false;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        let rows = program.type_reference_table.constraints(*constraints);
        if rows.is_empty() || rows.len() != constraints.count() as usize {
            return None;
        }
        for constraint in rows {
            match constraint {
                // IEEE endpoints restrict the value as a predicate over its
                // own format; they are not an integer interval.
                TypeConstraintNode::Range {
                    minimum, maximum, ..
                } if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) => {
                    crate::closed_float_range_endpoint(program, *minimum, primitive)?;
                    crate::closed_float_range_endpoint(program, *maximum, primitive)?;
                }
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => {
                    crate::closed_integer_range_bound(program, *minimum)?;
                    crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?;
                    bounded = true;
                }
                TypeConstraintNode::Domain(domain) => {
                    if let Some((minimum, maximum)) =
                        crate::exact_declared_domain_carrier_interval(program, primitive, domain)
                    {
                        if minimum > maximum {
                            return None;
                        }
                        bounded = true;
                    } else {
                        crate::scalar_field_domain_predicate(program, domain, primitive)?;
                    }
                }
                _ => return None,
            }
        }
        reference = *base_type;
    }
    // A selector stores one plain word; no bound can restrict it.
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::DynamicTrait { .. }
    ) {
        return (!bounded).then_some(PrimitiveType::Selector);
    }
    let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
    (!bounded || primitive.is_fixed_integer()).then_some(primitive)
}
