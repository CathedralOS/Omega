//! Rejoin constructor fields before their computations enter the ordinary
//! operand-scope walk. A matching final type cannot replace source identity.
use crate::expression_preparation::computation_graph::fields;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::data::DataMember;
use checked_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedScalarCaseConstruction, CheckedScalarComputationStructuralArgument,
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
type Computation = checked_trees::CheckedScalarComputationHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn construction(
    checked: &CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let source = validation::scalar_case_constructor(&checked.typed, subject.expression).ok_or(
        LoweringError::Unsupported("computed case lost its exact authored constructor"),
    )?;
    if source.type_reference != subject.type_reference || source.case != subject.case {
        return unsupported("computed case constructor type or case changed");
    }
    // This is the existing no-code ownership classifier, not an inference
    // from the currently selected payload being scalar.
    if !validation::has_plain_owned_contents_with_numeric_constraints(
        &checked.typed,
        subject.type_reference,
    ) || !matches!(
        checked.type_multiplicity(subject.type_reference),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) {
        return unsupported("computed case has no eligible no-code disposition");
    }
    let retained = fields(checked, subject)?;
    if retained.len() != source.fields.len() {
        return unsupported("computed case omitted or duplicated authored fields");
    }
    let nodes = &checked.facts.values.scalar_computations.nodes;
    source
        .fields
        .into_iter()
        .zip(retained)
        .map(|((symbol, expression, primitive), field)| {
            if symbol != field.symbol
                || !nodes.is_valid(field.value)
                || nodes.get(field.value).authored_root != expression
                || nodes.get(field.value).primitive_type != primitive
            {
                return unsupported(
                    "computed case field differs from its authored position, root or type",
                );
            }
            Ok((expression, field.value))
        })
        .collect()
}

pub(crate) fn membership(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
    subject: &CheckedScalarComputationStructuralArgument,
    case: symbols::SymbolHandle,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let (source_machine, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if source_machine.symbol != machine || !checked.expression_table.expression_is_valid(expression)
    {
        return unsupported("computed case observation has no exact source owner");
    }
    if let ExpressionNode::Match(dispatch) = checked.expression_table.expression(expression) {
        return match_membership(
            checked,
            source_machine,
            source_state,
            dispatch,
            subject,
            case,
        );
    }
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return unsupported("computed case observation lost its membership expression");
    };
    let type_reference = match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => {
            if binary.left != subject.expression {
                return unsupported("computed case changed its constructor expression");
            }
            subject.type_reference
        }
        CheckedScalarComputationStructuralArgument::Place(argument) => dispatch_place(
            checked,
            source_machine,
            source_state,
            erased_dispatch_subject(checked, source_state, binary.left).unwrap_or(binary.left),
            argument,
        )?,
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            return unsupported("case membership cannot observe an array");
        }
    };
    if !validation::has_exact_case_membership_meaning(
        &checked.typed,
        source_machine,
        Some(source_state),
        expression,
        binary,
    ) {
        return unsupported("computed case observation changed its subject or selected meaning");
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(type_reference)
    else {
        return unsupported("computed case observation lost its nominal type");
    };
    match checked.expression_table.expression(binary.right) {
        ExpressionNode::Name(selected) => {
            let owner = validation::exact_case_reference_owner(&checked.typed, binary.right)
                .ok_or(LoweringError::Unsupported(
                    "computed case observation has no exact case owner",
                ))?;
            if selected.symbol != case || owner.symbol != *symbol {
                return unsupported("computed case observation selected a foreign case");
            }
        }
        // A dispatch arm through an erased subject spells its case as a
        // payload-free constructor of the observed sum.
        ExpressionNode::StructLiteral(literal)
            if erased_dispatch_subject(checked, source_state, binary.left).is_some()
                && checked
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            if literal.case_symbol != Some(case) || literal.type_symbol != *symbol {
                return unsupported("computed case observation selected a foreign case");
            }
        }
        _ => return unsupported("computed case observation lost its selected case"),
    }
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => construction(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(Vec::new()),
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            unsupported("case membership cannot observe an array")
        }
    }
}

/// A discriminant-dispatch membership names its authored match: the retained
/// case must be exactly one of that match's classifier arms, and the subject
/// plan must read the same storage the match dispatches on. Field payloads
/// still rejoin only through their authored constructor.
fn match_membership(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    dispatch: &checked_trees::expression::TableMatchExpression,
    subject: &CheckedScalarComputationStructuralArgument,
    case: symbols::SymbolHandle,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let plan = validation::match_case_dispatch(&checked.typed, machine, state, dispatch).ok_or(
        LoweringError::Unsupported("computed case observation lost its discriminant dispatch"),
    )?;
    let type_reference = match (&plan.subject, subject) {
        (
            validation::MatchCaseSubject::Constructor,
            CheckedScalarComputationStructuralArgument::Case(construction),
        ) => {
            if construction.expression != dispatch.subject {
                return unsupported("computed case changed its constructor expression");
            }
            construction.type_reference
        }
        (
            validation::MatchCaseSubject::ImmutableLocal { .. }
            | validation::MatchCaseSubject::ParameterField,
            CheckedScalarComputationStructuralArgument::Place(argument),
        ) => dispatch_place(checked, machine, state, dispatch.subject, argument)?,
        _ => {
            return unsupported("computed case observation changed its subject source");
        }
    };
    if plan
        .arms
        .iter()
        .filter(|arm| arm.case == Some(case))
        .count()
        != 1
    {
        return unsupported("computed case observation selected a case outside its dispatch");
    }
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(type_reference)
    else {
        return unsupported("computed case observation lost its nominal type");
    };
    let mut variants = checked
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == *symbol)
        .flat_map(|data| checked.data_members(data).iter())
        .filter(|member| matches!(member, DataMember::Variant(variant) if variant.symbol == case));
    if variants.next().is_none() || variants.next().is_some() {
        return unsupported("computed case observation selected a foreign case");
    }
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => construction(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(Vec::new()),
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            unsupported("case membership cannot observe an array")
        }
    }
}

/// Replay a dispatch subject against its exact storage root and projection.
fn dispatch_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<TypeReferenceHandle, LoweringError> {
    match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => {
            named_place(checked, machine, state, expression, argument)
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => parameter_place(
            checked,
            machine,
            state,
            expression,
            argument,
            parameter_index,
        ),
        _ => unsupported("case membership substituted its structural source"),
    }
}

/// The borrowed place an erased runtime-indexed dispatch subject names
/// (`let __hoist_N = &self.items[self.w];`, followed by nothing but the
/// dispatch's transitions): stage 04 observes that place directly in each arm
/// (`control::case_observation_alias`), so the membership's plan must replay
/// against the borrow's target, not the local.
pub(crate) fn erased_dispatch_subject(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    subject: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(name) = checked.expression_table.expression(subject) else {
        return None;
    };
    let statements = checked.statement_table.statements(state.statement_nodes);
    let (index, local) = checked.state_local_declaration(state, name.symbol)?;
    if !is_erased_dispatch_subject(checked, statements, index) {
        return None;
    }
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    Some(borrow.target)
}

/// Whether statement `index` is an erased runtime-indexed dispatch subject: an
/// immutable shared borrow followed by nothing but transitions. It plans no
/// operation of its own.
pub(crate) fn is_erased_dispatch_subject(
    checked: &CheckedTrees,
    statements: &[checked_trees::statement::StatementNode],
    index: usize,
) -> bool {
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = statements.get(index)
    else {
        return false;
    };
    !local.is_mutable
        && local.symbol.is_valid()
        && matches!(
            checked.expression_table.expression(local.initial_value),
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared
                    && matches!(
                        checked.expression_table.expression(borrow.target),
                        ExpressionNode::Indexed(indexed)
                            if checked
                                .expression_table
                                .constant_integer_value(indexed.index)
                                .is_none()
                    )
        )
        && statements.get(index + 1..).is_some_and(|arms| {
            !arms.is_empty()
                && arms.iter().all(|statement| {
                    matches!(
                        statement,
                        checked_trees::statement::StatementNode::Transition(_)
                    )
                })
        })
}

/// Whether an authored selector (`self.w`) is exactly the field read a
/// `RuntimeIndex` segment names: the same root parameter and the same field
/// identities in order.
/// Whether `selector` is the bare name of the immutable integer local at dense
/// `position` of the state's scalar namespace: its non-erased scalar
/// parameters, then its initialized immutable primitive locals in declaration
/// order -- the temp symbol resolution hoists a computed subject index into.
fn selector_names_local(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    selector: ExpressionHandle,
    position: u32,
) -> bool {
    let ExpressionNode::Name(name) = checked.expression_table.expression(selector) else {
        return false;
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return false;
    }
    let parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
        .count();
    let Some((ordinal, local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if !local.is_mutable
                    && local.initial_value.is_valid()
                    && checked
                        .primitive_type_reference(local.type_reference)
                        .is_some() =>
            {
                Some(local)
            }
            _ => None,
        })
        .enumerate()
        .find(|(_, local)| local.symbol == name.symbol)
    else {
        return false;
    };
    checked
        .primitive_type_reference(local.type_reference)
        .is_some_and(|primitive| {
            matches!(
                primitive,
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
            )
        })
        && u32::try_from(parameters + ordinal).ok() == Some(position)
}

fn selector_names_field(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    selector: ExpressionHandle,
    field: &checked_trees::CheckedStructuralParameterField,
) -> bool {
    let mut members = Vec::new();
    let mut current = selector;
    let root = loop {
        match checked.expression_table.expression(current) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                members.push((member.member_symbol, current));
                current = member.receiver;
            }
            ExpressionNode::Name(name) => break name,
            _ => return false,
        }
    };
    members.reverse();
    let Some(parameter) = checked
        .state_parameters(state)
        .get(field.parameter_position as usize)
    else {
        return false;
    };
    let authored_self = parameter.is_self
        && (checked.symbols.get(root.symbol).kind == symbols::SymbolKind::Machine
            || matches!(
                checked.expression_table.name_path_members(root.members),
                [spelling] if spelling.is_self_receiver()
            ));
    if !(authored_self || (!parameter.is_self && root.symbol == parameter.symbol))
        || members.len() != field.path.len()
        || members.is_empty()
    {
        return false;
    }
    members.iter().zip(&field.path).enumerate().all(
        |(step, ((member_symbol, expression), segment))| {
            let identity = if authored_self && step == 0 {
                validation::exact_self_field(&checked.typed, machine, *expression)
                    .map(|field| field.path_identity())
            } else {
                checked
                    .data_definitions()
                    .iter()
                    .flat_map(|data| checked.data_members(data))
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.symbol == *member_symbol => {
                            Some(field.path_identity())
                        }
                        _ => None,
                    })
            };
            identity.is_some_and(|identity| {
                segment == &checked_trees::CheckedStructuralPredicatePathSegment::Field(identity)
            })
        },
    )
}

/// Rejoin the selector's authored formal before comparing its dense scalar
/// coordinate; another integer formal cannot stand in for the selected one.
fn selector_names_parameter(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    selector: ExpressionHandle,
    position: u32,
) -> bool {
    let ExpressionNode::Name(name) = checked.expression_table.expression(selector) else {
        return false;
    };
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        return false;
    };
    name.symbol.is_valid()
        && name.head_symbol == name.symbol
        && checked
            .expression_table
            .name_path_members(name.members)
            .len()
            == 1
        && parameters.next().is_none()
        && !parameter.relevance.is_erased()
        && !parameter.is_const
        && !parameter.is_mutable
        && checked.symbols.get(name.symbol).kind == symbols::SymbolKind::Parameter
        && checked.symbols.get(name.symbol).parent == state.symbol
        && super::structural_places::runtime_index_selector(checked, machine, state, selector)
            .is_ok_and(|actual| {
                actual == checked_trees::CheckedRuntimeIndex::Parameter { position }
            })
}

/// The dense structural index counts the authored parameters that keep a
/// structural plan slot: erased non-self bindings leave the namespace,
/// `const` formals carry no runtime slot, direct `Placed` view parameters
/// stay custody-only, and scalar formals never enter it. A retained
/// `self` receiver is a member, matching the plan's own index convention.
fn structural_parameter<'checked>(
    checked: &'checked CheckedTrees,
    state: &'checked checked_trees::state::State,
    parameter_index: u32,
) -> Option<&'checked checked_trees::signature::StateParameter> {
    checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            (!parameter.relevance.is_erased() || parameter.is_self || parameter.is_mutable)
                && !parameter.is_const
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
                && !matches!(
                    checked.type_reference_table.type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { referee, .. }
                        if !parameter.is_self
                            && checked
                                .placed_view_plan_for_type_reference(*referee)
                                .is_some()
                )
        })
        .nth(usize::try_from(parameter_index).ok()?)
}

/// A parameter-field dispatch subject walks the authored member path in order;
/// each member must resolve to the declared field whose identity the plan
/// retained, ending at the observed sum's exact declared type.
fn parameter_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    parameter_index: u32,
) -> Result<TypeReferenceHandle, LoweringError> {
    let parameter = structural_parameter(checked, state, parameter_index).ok_or(
        LoweringError::Unsupported("case membership has no source parameter"),
    )?;
    place_for(
        checked,
        machine,
        state,
        expression,
        argument,
        parameter.symbol,
        parameter.type_reference,
    )
}

/// Parameters and locals retain their declaration identity rather than a
/// guessed emitted position. Emission separately checks establishment at use.
pub(crate) fn named_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<TypeReferenceHandle, LoweringError> {
    let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = argument.source
    else {
        return unsupported("case observation lost its named storage source");
    };
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == symbol);
    let parameter = parameters.next();
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                Some(local)
            }
            _ => None,
        });
    let local = locals.next();
    if !symbol.is_valid()
        || parameters.next().is_some()
        || locals.next().is_some()
        || checked.symbols.get(symbol).parent != state.symbol
    {
        return unsupported("case observation substituted its storage declaration");
    }
    let declared_type = match (parameter, local) {
        (Some(parameter), None)
            if !parameter.is_const
                && !parameter.relevance.is_erased()
                && checked.symbols.get(symbol).kind == symbols::SymbolKind::Parameter =>
        {
            parameter.type_reference
        }
        (None, Some(local))
            if validation::reference_shell(&checked.typed, local.type_reference).is_none()
                && (validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    local.type_reference,
                ) || validation::has_owned_or_shared_view_contents(
                    &checked.typed,
                    local.type_reference,
                )) =>
        {
            local.type_reference
        }
        _ => return unsupported("case observation has no exact readable storage declaration"),
    };
    let mut root = expression;
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(root) || visited.contains(&root) {
            return unsupported("enum equality has a stale or cyclic parameter place");
        }
        visited.push(root);
        match checked.expression_table.expression(root) {
            ExpressionNode::Member(member) => root = member.receiver,
            ExpressionNode::Indexed(indexed) => root = indexed.collection,
            ExpressionNode::Name(name)
                if name.head_symbol == name.symbol && name.symbol == symbol =>
            {
                break;
            }
            _ => return unsupported("enum equality substituted its authored parameter root"),
        }
    }
    let mut reference = declared_type;
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("enum equality has a stale parameter type");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::WriteOnly,
                ..
            } => {
                return unsupported("enum equality cannot observe write-only storage");
            }
            _ => break,
        }
    }
    place_for(
        checked,
        machine,
        state,
        expression,
        argument,
        symbol,
        declared_type,
    )
}

fn place_for(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    symbol: symbols::SymbolHandle,
    declared_type: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, LoweringError> {
    if argument.access != CheckedStructuralAccess::SharedBorrow {
        return unsupported("case membership changed its parameter custody");
    }
    // Each authored step resolves against the retained segment kind: a member
    // field identity for `Field`, a constant element index for `FixedIndex`.
    enum Step {
        Field(symbols::SymbolHandle),
        Index(u64),
        /// A runtime-selected element; the expression is its selector.
        Runtime(ExpressionHandle),
    }
    let mut members = Vec::new();
    let mut current = expression;
    let root = loop {
        match checked.expression_table.expression(current) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("case membership has a case-variant path");
                }
                members.push(Step::Field(member.member_symbol));
                current = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                match checked
                    .expression_table
                    .constant_integer_value(indexed.index)
                {
                    Some(index) => {
                        let Ok(index) = u64::try_from(index) else {
                            return unsupported("case membership lost its fixed index");
                        };
                        members.push(Step::Index(index));
                    }
                    None => members.push(Step::Runtime(indexed.index)),
                }
                current = indexed.collection;
            }
            ExpressionNode::Name(name) => break name,
            _ => {
                return unsupported("case membership lost its parameter field path");
            }
        }
    };
    members.reverse();
    if !symbol.is_valid()
        || root.symbol != symbol
        || root.head_symbol != symbol
        || root.members.count() != 1
        || members.len() != argument.path.len()
    {
        return unsupported("case membership changed its storage root or path");
    }
    let mut type_reference = declared_type;
    for (member, segment) in members.iter().zip(&argument.path) {
        let unwrapped = validation::unwrapped_type_reference(checked, type_reference).ok_or(
            LoweringError::Unsupported("case membership lost its carrier type"),
        )?;
        match member {
            Step::Field(member_symbol) => {
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(unwrapped)
                else {
                    return unsupported("case membership requires a record carrier");
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol)
                    .ok_or(LoweringError::Unsupported(
                        "case membership lost its carrier owner",
                    ))?;
                let field = {
                    let mut fields =
                        checked
                            .data_members(owner)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) if field.symbol == *member_symbol => {
                                    Some(field)
                                }
                                _ => None,
                            });
                    let field = fields.next().ok_or(LoweringError::Unsupported(
                        "case membership lost its carrier field",
                    ))?;
                    if fields.next().is_some() {
                        return unsupported("case membership has an ambiguous carrier field");
                    }
                    field
                };
                if field.relevance.is_erased() {
                    return unsupported("case membership has an erased carrier field");
                }
                let identity = field.path_identity();
                if segment != &CheckedUnitStructuralPathSegment::Field(identity) {
                    return unsupported("case membership changed its field path");
                }
                type_reference = field.type_reference;
            }
            Step::Index(index) => {
                let TypeReferenceNode::FixedArray { element_type, .. } =
                    checked.type_reference_table.type_reference(unwrapped)
                else {
                    return unsupported("case membership requires a fixed-array carrier");
                };
                if segment != &CheckedUnitStructuralPathSegment::FixedIndex(*index) {
                    return unsupported("case membership changed its element index");
                }
                type_reference = *element_type;
            }
            Step::Runtime(selector) => {
                let element_type = match checked.type_reference_table.type_reference(unwrapped) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => element_type,
                    _ => return unsupported("case membership requires an array or slice carrier"),
                };
                let names_selector = match segment {
                    CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { position },
                    ) => selector_names_parameter(checked, machine, state, *selector, *position),
                    CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::StructuralField(field),
                    ) => selector_names_field(checked, machine, state, *selector, field),
                    CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Local { position },
                    ) => selector_names_local(checked, state, *selector, *position),
                    _ => return unsupported("case membership changed its runtime element"),
                };
                if !names_selector {
                    return unsupported("case membership substituted its element selector");
                }
                type_reference = *element_type;
            }
        }
    }
    if checked.normalized_type_identity(type_reference).as_str() != argument.type_identity {
        return unsupported("case membership changed its observed type");
    }
    Ok(type_reference)
}
