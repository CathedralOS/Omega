use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::PackageReviewContractExpression;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

mod aliases;
mod points;
pub(crate) use points::checked_self_parameter_symbol;

pub(crate) fn contract_member_has_exact_collection_length(
    compilation: &CheckedCompilation,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .any(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess
                && selection.target()
                    == AuthoredDeclarationSelectionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    )
        })
}

pub(crate) fn require_exact_checked_contract_nominal_member(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    expected_member: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    let selected = exact_checked_contract_nominal_member(compilation, context, expression)?;
    if !expected_member.is_valid()
        || (selected != expected_member
            && !aliases::attached_field(compilation, context, expected_member, selected))
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member disagrees with its exact checked member-selection row",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
}

pub(crate) fn exact_checked_contract_nominal_member(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member has {} exact checked member-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != context.selection_exposure {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member has the wrong retained selection exposure",
            context.subject_kind, context.subject_name
        ))]);
    }
    let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member does not retain one exact declaration target",
            context.subject_kind, context.subject_name
        ))]);
    };
    Ok(target.selected_symbol())
}

pub(crate) fn require_exact_checked_contract_collection_length(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Result<(), Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length projection has {} exact checked member-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != context.selection_exposure {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length projection has the wrong retained selection exposure",
            context.subject_kind, context.subject_name
        ))]);
    }
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
        || selection.target()
            != AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length syntax disagrees with its exact checked intrinsic selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
}

pub(crate) fn project_contract_member_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    receiver: PackageReviewContractExpression,
    member_symbol: SymbolHandle,
    case_variant_symbol: Option<SymbolHandle>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if !member_symbol.is_valid() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract contains an unresolved member expression",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(PackageReviewContractExpression::Member {
        receiver: Box::new(receiver),
        member: nominal_identity(compilation, member_symbol)?,
        case_variant: case_variant_symbol
            .map(|symbol| nominal_identity(compilation, symbol))
            .transpose()?,
    })
}

pub(crate) fn project_computed_contract_member_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
    receiver: PackageReviewContractExpression,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let selected = exact_checked_contract_nominal_member(compilation, context, expression)?;
    if member.member_symbol.is_valid() && member.member_symbol != selected {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` computed member disagrees with its exact checked declaration selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    let selected_parent = compilation.symbols.get(selected).parent;
    let case_variant = (compilation.symbols.get(selected_parent).kind
        == symbols::SymbolKind::Variant)
        .then_some(selected_parent);
    project_contract_member_expression(compilation, context, receiver, selected, case_variant)
}

pub(crate) fn contract_member_path_source(
    compilation: &CheckedCompilation,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    Vec<typed_trees::name::Identifier>,
)> {
    use typed_trees::expression::ExpressionNode;

    match compilation.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let (root, mut members) = contract_member_path_source(compilation, member.receiver)?;
            members.push(member.member.clone());
            Some((root, members))
        }
        ExpressionNode::Name(path)
            if compilation
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1 =>
        {
            Some((expression, Vec::new()))
        }
        _ => None,
    }
}

pub(crate) fn contract_member_path_root(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<facts::PlaceRoot> {
    let typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    if context.data_symbol.is_some_and(|data_symbol| {
        is_data_subject_field_expression(compilation, data_symbol, expression)
    }) {
        return context.data_symbol.map(facts::PlaceRoot::Symbol);
    }
    if context.domain_symbol.is_none()
        && compilation
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| name.as_str() == "self")
    {
        return checked_self_parameter_symbol(compilation, context, path)
            .map(facts::PlaceRoot::Symbol);
    }
    let resolved = path
        .head_symbol
        .is_valid()
        .then_some(path.head_symbol)
        .or_else(|| path.symbol.is_valid().then_some(path.symbol));
    if let Some(symbol) = resolved {
        return Some(facts::PlaceRoot::Symbol(symbol));
    }
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    if context.domain_symbol.is_some() && name.as_str() == "self" {
        return Some(facts::PlaceRoot::Expression(expression));
    }
    context
        .parameters
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol))
}

pub(crate) fn is_data_subject_field_expression(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return false;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return false;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return false;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return false;
    }
    let selected = path.head_symbol;
    let Some(data) = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)
    else {
        return false;
    };
    compilation.data_members(data).iter().any(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return false;
        };
        field.symbol == selected && field.name == *name
    })
}

pub(crate) fn data_subject_binder_position(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<usize> {
    let typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return None;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return None;
    }
    let selected = path.head_symbol;
    let data = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)?;
    let parameter = compilation
        .data_type_parameters(data)
        .iter()
        .find(|parameter| parameter.symbol == selected && parameter.name == *name)?;
    binders
        .iter()
        .position(|(symbol, _)| *symbol == parameter.symbol)
}

pub(crate) fn checked_contract_member_path(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    checked_fact: arena::Handle<typed_trees::domain::ProofFact>,
    expression: typed_trees::expression::ExpressionHandle,
    root: facts::PlaceRoot,
    source_members: &[typed_trees::name::Identifier],
) -> Result<Vec<(Option<SymbolHandle>, SymbolHandle)>, Vec<Diagnostic>> {
    use facts::{FactPayload, FactPlace};

    if let Some(domain_symbol) = context.domain_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .domain_definition_facts
            .iter()
            .filter(|(_, record)| {
                record.domain_symbol == domain_symbol && record.fact == checked_fact
            })
        {
            for dependency in record
                .dependencies
                .iter()
                .filter(|dependency| dependency.expression == expression)
            {
                let Some((_, place)) = compilation
                    .facts
                    .semantic
                    .places
                    .iter()
                    .find(|(handle, _)| *handle == dependency.place)
                else {
                    continue;
                };
                if place.root != root {
                    continue;
                }
                if let Some(selected) =
                    checked_member_segments(compilation, place.segments, source_members)
                {
                    candidates.push(selected);
                }
            }
        }
        let [selected] = candidates.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path resolves to {} exact checked dependency records; expected one",
                context.subject_kind,
                context.subject_name,
                candidates.len()
            ))]);
        };
        return Ok(selected.clone());
    }

    if let Some(data_symbol) = context.data_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .data_definition_facts
            .iter()
            .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == checked_fact)
        {
            for dependency in record
                .dependencies
                .iter()
                .filter(|dependency| dependency.expression == expression)
            {
                let Some((_, place)) = compilation
                    .facts
                    .semantic
                    .places
                    .iter()
                    .find(|(handle, _)| *handle == dependency.place)
                else {
                    continue;
                };
                if place.root != root {
                    continue;
                }
                if let Some(selected) =
                    checked_member_segments(compilation, place.segments, source_members)
                {
                    candidates.push(selected);
                }
            }
        }
        let [selected] = candidates.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path resolves to {} exact checked dependency records; expected one",
                context.subject_kind,
                context.subject_name,
                candidates.len()
            ))]);
        };
        return Ok(selected.clone());
    }

    let (point, origin) = points::contract_point(compilation, context, checked_fact)?;
    let mut candidates = Vec::new();
    for (_, semantic_fact) in compilation.facts.semantic.facts.iter() {
        let contract_fact_matches = matches!(
            semantic_fact.payload,
            FactPayload::ContractBooleanExpression { fact, .. }
                | FactPayload::ContractDomainMembership { fact, .. }
                if fact == checked_fact
        );
        if semantic_fact.point != point
            || origin.is_some_and(|origin| semantic_fact.origin != origin)
            || !contract_fact_matches
        {
            continue;
        }
        let FactPlace::Place(place_handle) = semantic_fact.place else {
            continue;
        };
        let place = compilation.facts.semantic.places.get(place_handle);
        if place.root != root {
            continue;
        }
        let Some(selected) = checked_member_segments(compilation, place.segments, source_members)
        else {
            continue;
        };
        if !candidates.contains(&selected) {
            candidates.push(selected);
        }
    }
    let [selected] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract member path resolves to {} exact checked place rows; expected one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    Ok(selected.clone())
}

pub(crate) fn checked_member_segments(
    compilation: &CheckedCompilation,
    segments: arena::HandleSpan<facts::PlaceSegment>,
    source_members: &[typed_trees::name::Identifier],
) -> Option<Vec<(Option<SymbolHandle>, SymbolHandle)>> {
    use facts::PlaceSegment;

    let mut selected = Vec::new();
    let mut pending_case = None;
    for segment in compilation
        .facts
        .semantic
        .place_segments
        .span_or_empty(segments)
    {
        match *segment {
            PlaceSegment::Case { variant } if pending_case.is_none() => {
                pending_case = Some(variant);
            }
            PlaceSegment::Field { symbol } if symbol.is_valid() => {
                selected.push((pending_case.take(), symbol));
            }
            _ => return None,
        }
    }
    if pending_case.is_some() || selected.len() != source_members.len() {
        return None;
    }
    if selected
        .iter()
        .zip(source_members)
        .any(|((_, symbol), name)| compilation.symbols.name(*symbol) != name.as_str())
    {
        return None;
    }
    Some(selected)
}
