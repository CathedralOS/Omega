//! Rejoin constructor fields before their computations enter the ordinary
//! operand-scope walk. A matching final type cannot replace source identity.
use crate::expression_preparation::computation_graph::fields;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::data::DataMember;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
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
        CheckedScalarComputationStructuralArgument::Place(argument) => {
            local_place(checked, source_state, binary.left, argument)?
        }
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
    let ExpressionNode::Name(selected) = checked.expression_table.expression(binary.right) else {
        return unsupported("computed case observation lost its selected case");
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(type_reference)
    else {
        return unsupported("computed case observation lost its nominal type");
    };
    let owner = validation::exact_case_reference_owner(&checked.typed, binary.right).ok_or(
        LoweringError::Unsupported("computed case observation has no exact case owner"),
    )?;
    if selected.symbol != case || owner.symbol != *symbol {
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

/// A dispatch subject must be the exact place the plan claims: an immutable
/// local's authored name, or a member path rooted at the dense structural
/// parameter the plan indexes.
fn dispatch_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<TypeReferenceHandle, LoweringError> {
    match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => {
            local_place(checked, state, expression, argument)
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

/// Source identity is checked independently of the established-place lookup.
/// The lookup during emission additionally requires the declaration to precede use.
fn local_place(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
        argument.source
    else {
        return unsupported("case membership requires an exact structural local");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("case membership local lost its authored name");
    };
    if !symbol.is_valid()
        || name.symbol != symbol
        || name.head_symbol != symbol
        || name.members.count() != 1
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || !argument.path.is_empty()
        || checked
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == symbol)
    {
        return unsupported("case membership substituted its local source or custody");
    }
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
    let local = locals.next().ok_or(LoweringError::Unsupported(
        "case membership has no source local",
    ))?;
    if locals.next().is_some()
        || local.is_mutable
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != argument.type_identity
    {
        return unsupported("case membership changed its local declaration or type");
    }
    Ok(local.type_reference)
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
    if argument.access != CheckedStructuralAccess::SharedBorrow {
        return unsupported("case membership changed its parameter custody");
    }
    // Each authored step resolves against the retained segment kind: a member
    // field identity for `Field`, a constant element index for `FixedIndex`.
    enum Step {
        Field(symbols::SymbolHandle, ExpressionHandle),
        Index(u64),
    }
    let mut members = Vec::new();
    let mut current = expression;
    let root = loop {
        match checked.expression_table.expression(current) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("case membership has a case-variant path");
                }
                members.push(Step::Field(member.member_symbol, current));
                current = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let Some(index) = checked
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|index| u64::try_from(index).ok())
                else {
                    return unsupported("case membership lost its fixed index");
                };
                members.push(Step::Index(index));
                current = indexed.collection;
            }
            ExpressionNode::Name(name) => break name,
            _ => {
                return unsupported("case membership lost its parameter field path");
            }
        }
    };
    members.reverse();
    let parameters = checked.state_parameters(state);
    let parameter = structural_parameter(checked, state, parameter_index).ok_or(
        LoweringError::Unsupported("case membership has no source parameter"),
    )?;
    let authored_self = parameter.is_self
        && (checked.symbols.get(root.symbol).kind == symbols::SymbolKind::Machine
            || matches!(
                checked.expression_table.name_path_members(root.members),
                [spelling] if spelling.is_self_receiver()
            ));
    let authored_name = !parameter.is_self
        && (root.symbol == parameter.symbol
            || checked
                .expression_table
                .name_path_member_symbols(root.member_symbols)
                .contains(&parameter.symbol));
    if !root.symbol.is_valid() && !authored_self {
        return unsupported("case membership lost its parameter name");
    }
    if !authored_self && !authored_name {
        return unsupported("case membership substituted its parameter source");
    }
    if parameters
        .iter()
        .filter(|candidate| candidate.symbol == parameter.symbol)
        .count()
        != 1
        || members.len() != argument.path.len()
    {
        return unsupported("case membership changed its parameter or path");
    }
    let mut type_reference = parameter.type_reference;
    for (step_index, (member, segment)) in members.iter().zip(&argument.path).enumerate() {
        let unwrapped = validation::unwrapped_type_reference(checked, type_reference).ok_or(
            LoweringError::Unsupported("case membership lost its carrier type"),
        )?;
        match member {
            Step::Field(member_symbol, member_expression) => {
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(unwrapped)
                else {
                    return unsupported("case membership requires a record carrier");
                };
                // Attached-data carriers name their machine; the record lives
                // at the machine's attached data symbol.
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol)
                    .or_else(|| {
                        checked
                            .machines()
                            .iter()
                            .find(|owner_machine| owner_machine.symbol == *symbol)
                            .and_then(|owner_machine| {
                                checked
                                    .data_definitions()
                                    .iter()
                                    .find(|data| data.symbol == owner_machine.attached_data_symbol)
                            })
                    })
                    .ok_or(LoweringError::Unsupported(
                        "case membership lost its carrier owner",
                    ))?;
                // Attached fields are installed under inherited machine-child
                // symbols; a first self step rejoins through the attached
                // declaration rather than that retained member symbol.
                let field = if authored_self && step_index == 0 {
                    validation::exact_self_field(&checked.typed, machine, *member_expression)
                        .ok_or(LoweringError::Unsupported(
                            "case membership lost its attached field",
                        ))?
                } else {
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
                let identity = field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned());
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
        }
    }
    if checked.normalized_type_identity(type_reference).as_str() != argument.type_identity {
        return unsupported("case membership changed its observed type");
    }
    Ok(type_reference)
}
