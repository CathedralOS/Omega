//! Rejoin scalar reads with authored bindings, storage, and operand positions.

use super::*;

mod case_membership;

#[cfg(test)]
mod tests;

/// Reads retain their occurrence, operand position, and exact binding or place.
/// Unary and cast wrappers do not change the read's identity. Greater-than
/// comparisons use the same operand normalization as the checked producer.
pub(super) fn validate(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    source: &SourceRoot,
) -> Result<(), LoweringError> {
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        .ok_or(LoweringError::Unsupported(
            "storage read has no exact scalar expression",
        ))?;
    let expression = if binding.role == CheckedScalarExpressionRole::Guard {
        guard_subject(checked, source.expression)
    } else {
        source.expression
    };
    validate_expression(
        checked,
        binding.state,
        binding.statement_ordinal,
        expression,
        retained,
    )
}

/// Rejoin fixed-integer and Boolean reads to their exact source scope.
/// Computation operands use this same check without inventing a pure-plan row.
pub(crate) fn validate_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (_, state) = authored_state(checked, state)?;
    let mut authored_reads = Vec::new();
    let mut member_paths = Vec::new();
    collect_authored_storage_reads(
        checked,
        state,
        statement,
        expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut authored_reads,
        &mut member_paths,
    )?;
    let namespace = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement as usize)
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if !local.is_mutable
                            && local.initial_value.is_valid()
                            && checked
                                .primitive_type_reference(local.type_reference)
                                .is_some() =>
                    {
                        Some(local.symbol)
                    }
                    _ => None,
                }),
        )
        .collect::<Vec<_>>();
    let namespace = ReadNamespace {
        scalar: namespace,
        structural: checked.state_parameters(state),
        owned_field_paths: member_paths,
        owned: checked
            .state_parameters(state)
            .iter()
            .map(|parameter| {
                owned_record(checked, parameter)
                    .map(|_| parameter.symbol)
                    .unwrap_or_default()
            })
            .collect(),
    };
    let mut retained_reads = Vec::new();
    collect_scalar_storage_reads(retained, &namespace, &mut Vec::new(), &mut retained_reads);
    if retained_reads != authored_reads {
        return unsupported("scalar read differs from its authored binding or mutable place");
    }
    Ok(())
}

/// Guard lowering erases the builtin `subject == true` wrapper, not its subject.
fn guard_subject(checked: &CheckedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return expression;
    };
    if binary.operator != checked_trees::expression::BinaryOperator::Equal
        || checked
            .facts
            .operators
            .expression_use(expression)
            .is_some_and(|operator| {
                operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    {
        return expression;
    }
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (ExpressionNode::Boolean(true), _) => binary.right,
        (_, ExpressionNode::Boolean(true)) => binary.left,
        _ => expression,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadKind {
    Storage,
    Parameter,
    Local,
    OwnedField(Vec<checked_trees::CheckedStructuralPredicatePathSegment>),
    CaseMembership {
        path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        case: String,
    },
}

type StorageReadOccurrence = (Vec<usize>, symbols::SymbolHandle, PrimitiveType, ReadKind);

struct ReadNamespace<'checked> {
    scalar: Vec<symbols::SymbolHandle>,
    structural: &'checked [checked_trees::signature::StateParameter],
    owned_field_paths: Vec<Vec<usize>>,
    // Structural positions are authored positions, including scalar formals.
    owned: Vec<symbols::SymbolHandle>,
}

fn owned_record<'checked>(
    checked: &'checked CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
) -> Option<&'checked checked_trees::data::DataDefinition> {
    if parameter.is_self
        || parameter.is_const
        || parameter.is_mutable
        || !parameter.symbol.is_valid()
        || !matches!(
            checked.type_multiplicity(parameter.type_reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
    {
        return None;
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    if !symbol.is_valid() || checked.symbols.get(*symbol).kind != symbols::SymbolKind::Data {
        return None;
    }
    let mut definitions = checked
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == *symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some()
        || checked
            .data_members(definition)
            .iter()
            .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    Some(definition)
}

fn authored_owned_field(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    let ExpressionNode::Member(member) = checked.expression_table.expression(expression) else {
        return Ok(None);
    };
    if member.case_variant.is_some() {
        return Ok(None);
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(member.receiver) else {
        return Ok(None);
    };
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        return Ok(None);
    };
    let Some(owner) = owned_record(checked, parameter) else {
        return Ok(None);
    };
    if parameters.next().is_some()
        || name.head_symbol != parameter.symbol
        || !matches!(checked.expression_table.name_path_members(name.members),
            [spelling] if spelling.as_str() == parameter.name.as_str())
        || !member.member_symbol.is_valid()
    {
        return unsupported("owned scalar field has no exact authored parameter or member");
    }
    let mut fields = checked
        .data_members(owner)
        .iter()
        .filter_map(|candidate| match candidate {
            checked_trees::data::DataMember::Field(field)
                if field.symbol == member.member_symbol =>
            {
                Some(field)
            }
            _ => None,
        });
    let field = fields.next().ok_or(LoweringError::Unsupported(
        "owned scalar field belongs to another record",
    ))?;
    if fields.next().is_some()
        || field.name != member.member
        || checked.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
        || checked.symbols.get(field.symbol).parent != owner.symbol
    {
        return unsupported("owned scalar field differs from its resolved declaration");
    }
    let Some(primitive) = checked
        .primitive_type_reference(field.type_reference)
        .filter(|primitive| supported_mutable_parameter(*primitive))
    else {
        return Ok(None);
    };
    if field.relevance.is_erased() || checked.data_members(owner).iter().filter(|candidate| {
        matches!(candidate, checked_trees::data::DataMember::Field(candidate)
            if candidate.name == field.name || field.identity.is_some() && candidate.identity == field.identity)
    }).count() != 1 {
        return unsupported("owned scalar field has no unique relevant identity");
    }
    let (machine, _) = authored_state(checked, state.symbol)?;
    if validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
        .and_then(|reference| checked.primitive_type_reference(reference))
        != Some(primitive)
    {
        return unsupported("owned scalar field differs from its declared place type");
    }
    let identity = field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned());
    Ok(Some((
        parameter.symbol,
        primitive,
        ReadKind::OwnedField(vec![
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity),
        ]),
    )))
}

fn collect_owned_field(
    parameter_position: u32,
    field_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    primitive: PrimitiveType,
    namespace: &ReadNamespace,
    path: &[usize],
    reads: &mut Vec<StorageReadOccurrence>,
) {
    // Nested, indexed, receiver and borrowed projections retain their existing owners.
    if !supported_mutable_parameter(primitive)
        || !matches!(field_path, [checked_trees::CheckedStructuralPredicatePathSegment::Field(_)])
        // Match members present in typed source, including equality expanded
        // during typing. Later plan-only expansion has no source member here.
        || !namespace.owned_field_paths.iter().any(|authored| authored == path)
    {
        return;
    }
    if let Some(symbol) = namespace
        .owned
        .get(parameter_position as usize)
        .filter(|symbol| symbol.is_valid())
    {
        reads.push((
            path.to_vec(),
            *symbol,
            primitive,
            ReadKind::OwnedField(field_path.to_vec()),
        ));
    }
}

fn authored_storage_read(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    before: u32,
    name: &checked_trees::expression::TableNamePath,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return Ok(None);
    }
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(before as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
            _ => None,
        });
    if let Some(local) = locals.next() {
        if locals.next().is_some() {
            return unsupported("scalar storage read has duplicate authored declarations");
        }
        return Ok(checked
            .primitive_type_reference(local.type_reference)
            .filter(|primitive| local.is_mutable || supported_mutable_parameter(*primitive))
            .map(|primitive| {
                (
                    local.symbol,
                    primitive,
                    if local.is_mutable {
                        ReadKind::Storage
                    } else {
                        ReadKind::Local
                    },
                )
            }));
    }
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        return Ok(None);
    };
    if parameters.next().is_some() {
        return unsupported("scalar storage read has duplicate authored parameters");
    }
    if let Some((primitive, _)) = super::primitive_references::parameter_type(checked, parameter) {
        return Ok(Some((parameter.symbol, primitive, ReadKind::Storage)));
    }
    Ok(checked
        .primitive_type_reference(parameter.type_reference)
        .filter(|primitive| parameter.is_mutable || supported_mutable_parameter(*primitive))
        .map(|primitive| {
            (
                parameter.symbol,
                primitive,
                if parameter.is_mutable {
                    ReadKind::Storage
                } else {
                    ReadKind::Parameter
                },
            )
        }))
}

fn collect_authored_storage_reads(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    before: u32,
    expression: ExpressionHandle,
    path: &mut Vec<usize>,
    active: &mut Vec<ExpressionHandle>,
    reads: &mut Vec<StorageReadOccurrence>,
    member_paths: &mut Vec<Vec<usize>>,
) -> Result<(), LoweringError> {
    use checked_trees::expression::BinaryOperator;
    if !checked.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return unsupported("scalar storage source contains a stale or cyclic expression");
    }
    active.push(expression);
    if let Some((subject, case)) = case_membership::authored(checked, state, expression)? {
        reads.push((
            path.clone(),
            subject,
            PrimitiveType::Bool,
            ReadKind::CaseMembership {
                path: Vec::new(),
                case,
            },
        ));
        active.pop();
        return Ok(());
    }
    match checked.expression_table.expression(expression) {
        ExpressionNode::Match(_) => {
            return unsupported("scalar dispatch requires selective computation source custody");
        }
        ExpressionNode::Name(name) => {
            if let Some((symbol, primitive, kind)) =
                authored_storage_read(checked, state, before, name)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            }
        }
        ExpressionNode::Member(member) => {
            if member.case_variant.is_none()
                && matches!(
                    checked.expression_table.expression(member.receiver),
                    ExpressionNode::Name(_)
                )
            {
                member_paths.push(path.clone());
            }
            if let Some((symbol, primitive, kind)) =
                authored_owned_field(checked, state, expression)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            }
        }
        ExpressionNode::Binary(binary) => {
            let (left, right) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (binary.right, binary.left)
            } else {
                (binary.left, binary.right)
            };
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_authored_storage_reads(
                    checked,
                    state,
                    before,
                    operand,
                    path,
                    active,
                    reads,
                    member_paths,
                )?;
                path.pop();
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_authored_storage_reads(
                checked,
                state,
                before,
                unary.operand,
                path,
                active,
                reads,
                member_paths,
            )?;
        }
        ExpressionNode::Cast(cast) => {
            collect_authored_storage_reads(
                checked,
                state,
                before,
                cast.value,
                path,
                active,
                reads,
                member_paths,
            )?;
        }
        ExpressionNode::Indexed(indexed) => {
            path.push(0);
            collect_authored_storage_reads(
                checked,
                state,
                before,
                indexed.index,
                path,
                active,
                reads,
                member_paths,
            )?;
            path.pop();
        }
        // These source forms are owned by literal, structural, or computation
        // custody. Their roots are not primitive scalar content reads.
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    active.pop();
    Ok(())
}

fn collect_scalar_storage_reads(
    expression: &CheckedScalarExpression,
    namespace: &ReadNamespace,
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            reads.push((path.clone(), *symbol, *primitive_type, ReadKind::Storage));
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } => {
            path.push(0);
            collect_scalar_storage_reads(index, namespace, path, reads);
            path.pop();
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. } => {
            collect_scalar_storage_reads(operand, namespace, path, reads);
        }
        CheckedScalarExpression::Boolean(expression) => {
            collect_boolean_storage_reads(expression, namespace, path, reads);
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }
        | CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            if supported_mutable_parameter(*primitive_type) {
                reads.push((
                    path.clone(),
                    namespace.scalar.get(*position).copied().unwrap_or_default(),
                    *primitive_type,
                    if matches!(expression, CheckedScalarExpression::Parameter { .. }) {
                        ReadKind::Parameter
                    } else {
                        ReadKind::Local
                    },
                ));
            }
        }
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path: field_path,
            primitive_type,
        } => {
            collect_owned_field(
                *parameter_position,
                field_path,
                *primitive_type,
                namespace,
                path,
                reads,
            );
        }
        CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. } => {}
    }
}

fn collect_boolean_storage_reads(
    expression: &CheckedBooleanExpression,
    namespace: &ReadNamespace,
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedBooleanExpression::StorageRead { symbol } => {
            reads.push((
                path.clone(),
                *symbol,
                PrimitiveType::Bool,
                ReadKind::Storage,
            ));
        }
        CheckedBooleanExpression::Not(operand) => {
            collect_boolean_storage_reads(operand, namespace, path, reads);
        }
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_boolean_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            reads.push((
                path.clone(),
                namespace.scalar.get(*position).copied().unwrap_or_default(),
                PrimitiveType::Bool,
                if matches!(expression, CheckedBooleanExpression::Parameter { .. }) {
                    ReadKind::Parameter
                } else {
                    ReadKind::Local
                },
            ));
        }
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path: field_path,
        } => {
            collect_owned_field(
                *parameter_position,
                field_path,
                PrimitiveType::Bool,
                namespace,
                path,
                reads,
            );
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            reads.push((
                path.clone(),
                namespace
                    .structural
                    .get(subject.parameter_position as usize)
                    .map(|parameter| parameter.symbol)
                    .unwrap_or_default(),
                PrimitiveType::Bool,
                ReadKind::CaseMembership {
                    path: subject.path.clone(),
                    case: case.clone(),
                },
            ));
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {}
    }
}
