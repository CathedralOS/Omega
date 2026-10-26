//! Runtime membership uses the same exact authored selection judgment as
//! checking. It is a tag observation, never equality against a fabricated
//! payload value. The current scalar namespace names parameter referents;
//! constructor and local referents must arrive through ordinary value sequencing.
//! Normal-return contracts additionally name their authenticated result place,
//! never a synthetic parameter slot or a constructor-derived assumption.
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedStructuralParameterField;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::signature::StateParameter;

pub(crate) fn lower(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    let subject = match program.expression_table.expression(binary.left) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => binary.left,
    };
    let mut path = Vec::new();
    let parameter_position = super::structural_fields::structural_parameter_case_path(
        program, parameters, subject, &mut path,
    )?;
    let root = parameters.get(parameter_position as usize)?;
    // The existing scalar builder supplies its authored parameter namespace,
    // not a machine handle. Rejoin through the selected root symbol, which is
    // unique to its declaring state; never choose a same-named parameter.
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program.machine_states(machine).iter().find_map(|state| {
            let authored = program.state_parameters(state);
            (authored
                .get(parameter_position as usize)
                .is_some_and(|parameter| parameter.symbol == root.symbol)
                && authored == parameters)
                .then_some((machine, state))
        })
    })?;
    let case = case_identity(program, machine, Some(state), expression, binary, subject)?;
    Some(CheckedBooleanExpression::StructuralCaseMembership {
        subject: CheckedStructuralParameterField {
            parameter_position,
            path,
        }
        .into(),
        case,
    })
}

pub(crate) fn lower_result(
    program: &dyn ::validation::CheckingInput,
    owner: symbols::SymbolHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    let result = validation::reserved_result_place(program, binary.left)?;
    if result.machine_symbol != owner
        || validation::reserved_result_owner(program, result.root)? != (owner, result_type)
    {
        return None;
    }
    let machine = program.machine_by_symbol(owner)?;
    let case = case_identity(program, machine, None, expression, binary, binary.left)?;
    let path = super::structural_fields::structural_projection_path(program, &result.segments)?;
    let (path, _) =
        super::structural_fields::explicit_observation_path(program, result_type, &path)?;
    Some(CheckedBooleanExpression::StructuralCaseMembership {
        subject: checked_trees::CheckedStructuralCaseSubject {
            root: checked_trees::CheckedStructuralCaseRoot::Result,
            path,
        },
        case,
    })
}

fn case_identity(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
    subject: ExpressionHandle,
) -> Option<checked_trees::CheckedMemberIdentity> {
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    match program.expression_table.expression(binary.right) {
        ExpressionNode::Name(_) => {
            let selected = validation::applied_case_membership_variant(
                program, machine, state, expression, binary,
            )?;
            structural_case_name(program, selected)
        }
        // A payload-free case spelled as a value (`subject == T::Case`) arrives
        // as a construction, not a name: the authored test carries no
        // case-selection roster for the exact-meaning rejoin, but builtin
        // equality on a union's operands is already the same tag observation.
        ExpressionNode::StructLiteral(literal)
            if literal.case_symbol.is_some()
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            if !validation::has_builtin_binary_expression_meaning(
                program, machine, state, expression,
            ) {
                return None;
            }
            let Some(case_symbol) = literal.case_symbol else {
                return None;
            };
            let owner = program.data_definitions().iter().find(|data| {
                data.symbol == literal.type_symbol
                    && program.data_members(data).iter().any(|member| {
                        matches!(
                            member,
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.symbol == case_symbol
                                    && program.data_payload_fields(variant).is_empty()
                        )
                    })
            })?;
            let subject_type = validation::declared_place_type_raw(
                program, machine, state, subject,
            )
            .or_else(|| {
                validation::reserved_result_place(program, subject)
                    .filter(|place| place.machine_symbol == machine.symbol)
                    .map(|place| place.type_reference)
            })?;
            if crate::values::scalar::structural_fields::structural_data(program, subject_type)?
                .symbol
                != owner.symbol
            {
                return None;
            }
            structural_case_name(program, case_symbol)
        }
        _ => None,
    }
}

/// The discriminant key a checked case-membership test stores: the declared
/// `#<decimal>` identity an authored discriminant value writes, or the case's
/// declared name when no identity exists.
pub(crate) fn structural_case_name(
    program: &dyn ::validation::CheckingInput,
    selected: symbols::SymbolHandle,
) -> Option<checked_trees::CheckedMemberIdentity> {
    let case = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(case) if case.symbol == selected => Some(case),
            _ => None,
        })?;
    Some(checked_trees::CheckedMemberIdentity::of_member(
        case.identity,
        case.name.clone(),
    ))
}
