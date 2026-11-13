//! Source operator and named-value type queries. These retain exact typed
//! declarations and signatures; they grant no body or proof judgment.

use crate::{
    TypedTrees,
    data::{DataDefinition, DataField, DataMember},
    expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCastExpression,
        TableMemberExpression,
    },
    machine::Machine,
    state::State,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use numerics::literals::FloatFormat;
use symbols::SymbolHandle;

/// Primitive/static leaf queries supplied by the source preparation context.
/// None means no declared type is available; it never grants a type or fact.
pub trait ContextualSourceTypes {
    fn typed(&self) -> &TypedTrees;
    fn expression_result_type(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle>;
    fn cast_result_type(&self, cast: &TableCastExpression) -> Option<TypeReferenceHandle>;
    fn selected_operator_result_type(
        &self,
        symbol: SymbolHandle,
        result: TypeReferenceHandle,
        operands: &[Option<TypeReferenceHandle>],
    ) -> Option<TypeReferenceHandle>;
    fn array_literal_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle>;
}

pub fn builtin_type_reference(
    program: &TypedTrees,
    atom: symbols::BuiltinTypeAtom,
) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

fn mathematical_parameter_type_reference(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    program
        .mathematical_definitions()
        .iter()
        .find_map(|definition| {
            let parameter = program
                .mathematical_parameters(definition.parameters)
                .iter()
                .find(|parameter| parameter.symbol == symbol)?;
            match program.mathematical_type(parameter.ty) {
                crate::mathematical::MathematicalType::Ordinary(reference) => Some(*reference),
                _ => None,
            }
        })
}

pub fn type_is_primitive(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut seen = Vec::new();
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || seen.contains(&reference)
        {
            return false;
        }
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => return program.primitive_type_reference(reference).is_some(),
        }
    }
}

fn exact_data_member_field<'a>(
    program: &'a TypedTrees,
    data: &'a DataDefinition,
    member: &TableMemberExpression,
) -> Option<&'a DataField> {
    let mut found = None;
    for item in program.data_members(data) {
        let fields = match item {
            DataMember::Field(field) if member.case_variant.is_none() => {
                std::slice::from_ref(field)
            }
            DataMember::Variant(variant)
                if member
                    .case_variant
                    .as_ref()
                    .is_none_or(|case| case.as_str() == variant.name.as_str()) =>
            {
                program.data_payload_fields(variant)
            }
            _ => continue,
        };
        for field in fields {
            if field.name != member.member
                || !field.symbol.is_valid()
                || !field.type_reference.is_valid()
                || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
            {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(field);
        }
    }
    found
}

fn data_field_type_reference(
    program: &TypedTrees,
    data: &DataDefinition,
    member: &TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    exact_data_member_field(program, data, member).map(|field| field.type_reference)
}

fn field_type_reference(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    member: &TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    let mut cursor = crate::type_system::source_projections::SourceTypeCursor::new(reference);
    let symbol = if member.member_symbol.is_valid() {
        member.member_symbol
    } else {
        let base = program
            .type_reference_table
            .peeled_type_reference(reference);
        let owner = program.type_reference_table.type_symbol(base);
        let data = program
            .closed_data_definition(reference)
            .or_else(|| program.unique_data_definition_by_symbol(owner))?;
        exact_data_member_field(program, data, member)?.symbol
    };
    cursor.field(program, symbol)
}

pub fn static_type_argument_matches(
    program: &crate::TypedTrees,
    supplied: &StaticMachineArgument,
    inferred: TypeReferenceHandle,
) -> bool {
    if !program
        .type_reference_table
        .contains_type_reference(inferred)
        || supplied.const_literal.is_some()
        || supplied.evidence_projection.is_some()
    {
        return false;
    }
    if supplied.type_reference.is_valid() {
        if !program
            .type_reference_table
            .contains_type_reference(supplied.type_reference)
        {
            return false;
        }
        let supplied = program.normalized_type_identity(supplied.type_reference);
        let inferred = program.normalized_type_identity(inferred);
        return supplied.is_valid() && inferred.is_valid() && supplied == inferred;
    }
    match program.type_reference_table.type_reference(inferred) {
        TypeReferenceNode::Named { symbol, .. } => {
            supplied.application.is_none()
                && symbol.is_valid()
                && supplied.symbol.is_valid()
                && supplied.symbol == *symbol
        }
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => {
            let Some(application) = supplied.application.as_ref() else {
                return false;
            };
            base_symbol.is_valid()
                && supplied.symbol.is_valid()
                && supplied.symbol == *base_symbol
                && lifetime_arguments.is_empty()
                && application.lifetime_arguments.is_empty()
                && application.arguments.len() == arguments.len()
                && application
                    .arguments
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .all(|(supplied, inferred)| {
                        static_type_argument_matches(program, supplied, *inferred)
                    })
        }
        TypeReferenceNode::Callable { .. }
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}
pub fn expression_type_reference_in_state(
    context: &impl ContextualSourceTypes,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let program = context.typed();
    match program.declared_callable_expression_type_reference(expression) {
        Ok(Some(reference)) => return Some(reference),
        Ok(None) => {}
        Err(_) => return None,
    }
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(saved) => {
            program.saved_value_source(saved).and_then(|subject| {
                expression_type_reference_in_state(context, state_symbol, statement_index, subject)
            })
        }
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| {
                expression_type_reference_in_state(
                    context,
                    state_symbol,
                    statement_index,
                    arm.value,
                )
            }),
        ExpressionNode::Atomic(atomic) => {
            expression_type_reference_in_state(context, state_symbol, statement_index, atomic.value)
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .first()
                .map(|name| name.as_str());
            symbol_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                path.symbol,
                name,
            )
        }
        ExpressionNode::Member(member) => expression_type_reference_in_state(
            context,
            state_symbol,
            statement_index,
            member.receiver,
        )
        .and_then(|receiver| field_type_reference(program, receiver, member))
        .or_else(|| data_binding_field_type_reference(program, state_symbol, member)),
        ExpressionNode::Borrow(_) | ExpressionNode::Indexed(_) => program
            .machine_holding_state(state_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .and_then(|(machine, state)| {
                context.expression_result_type(machine, state, expression)
            }),
        ExpressionNode::Cast(cast) => context.cast_result_type(cast),
        ExpressionNode::Call(call) => program
            .machine_holding_state(state_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .and_then(|(machine, state)| context.expression_result_type(machine, state, expression))
            .or_else(|| {
                crate::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            }),
        ExpressionNode::Binary(binary) => {
            let operands = [
                expression_type_reference_in_state(
                    context,
                    state_symbol,
                    statement_index,
                    binary.left,
                ),
                expression_type_reference_in_state(
                    context,
                    state_symbol,
                    statement_index,
                    binary.right,
                ),
            ];
            if operands.iter().any(Option::is_some)
                && let Some(spelling) = crate::operator::binary_operator_spelling(binary.operator)
            {
                let candidates = crate::operator::resolve_spelling_for_operands(
                    program,
                    spelling,
                    &operands,
                    program.expression_table.source_span(expression),
                );
                if let [candidate] = candidates.as_slice() {
                    if candidate.domain.is_none() {
                        return context.selected_operator_result_type(
                            candidate.operator.symbol,
                            candidate.operator.return_type,
                            &operands,
                        );
                    }
                    return None;
                } else if !candidates.is_empty() {
                    return None;
                }
            }
            use crate::expression::BinaryOperator;
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) {
                return builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool);
            }
            operands[0].or(operands[1])
        }
        ExpressionNode::UndeterminedNaN(format) => float_type_reference(program, *format),
        ExpressionNode::Float(literal) => literal
            .landing()
            .and_then(|format| float_type_reference(program, format)),
        ExpressionNode::TypeExpression(_) => None,
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol),
        ExpressionNode::ArrayLiteral(_) => context.array_literal_type(expression),
        // The exact callable query above either supplied its type or refused its stale row.
        ExpressionNode::MachineValue(_) => None,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_) => None,
    }
}

fn float_type_reference(
    program: &crate::TypedTrees,
    format: FloatFormat,
) -> Option<TypeReferenceHandle> {
    let primitive = match format {
        FloatFormat::F32 => crate::types::PrimitiveType::F32,
        FloatFormat::F64 => crate::types::PrimitiveType::F64,
    };
    let atom = match primitive {
        crate::types::PrimitiveType::F32 => symbols::BuiltinTypeAtom::F32,
        crate::types::PrimitiveType::F64 => symbols::BuiltinTypeAtom::F64,
        _ => return None,
    };
    builtin_type_reference(program, atom)
}

fn symbol_type_reference_in_state(
    program: &crate::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() && name.is_none() {
        return None;
    }

    let state = program.state_by_symbol(state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            (symbol.is_valid() && parameter.symbol == symbol)
                || (!symbol.is_valid() && name.is_some_and(|name| parameter.name.as_str() == name))
        })
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            local_type_reference_before_statement(program, state, statement_index, symbol, name)
        })
}

fn local_type_reference_before_statement(
    program: &crate::TypedTrees,
    state: &crate::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let local = statement.as_local_data()?;
            ((symbol.is_valid() && local.symbol == symbol)
                || (!symbol.is_valid() && name.is_some_and(|name| local.name.as_str() == name)))
            .then_some(local.type_reference)
        })
}

fn data_binding_field_type_reference(
    program: &crate::TypedTrees,
    state_symbol: SymbolHandle,
    member: &crate::expression::TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    let path = program.expression_table.name_path_node(member.receiver)?;
    let (machine, state) = program
        .machine_holding_state(state_symbol)
        .zip(program.state_by_symbol(state_symbol))?;
    let data_binding = program.data_binding_parameter(machine, program.state_parameters(state))?;
    let path_members = program.expression_table.name_path_members(path.members);
    let names_data_binding = path_members.len() == 1
        && ((path.symbol.is_valid() && path.symbol == data_binding.symbol)
            || (path.head_symbol.is_valid() && path.head_symbol == data_binding.symbol));
    if !names_data_binding {
        return None;
    }

    let data = program.data_definition_by_symbol(machine.attached_data_symbol)?;
    data_field_type_reference(program, data, member)
}
pub fn builtin_member_type_reference(
    program: &crate::TypedTrees,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    match program.symbols.builtin_type_member(symbol)? {
        symbols::BuiltinTypeMember::AddressBound => program
            .symbols
            .builtin_type_symbol(symbols::BuiltinType::Int)
            .and_then(|symbol| {
                program
                    .type_reference_table
                    .find_named_type_reference(symbol)
            }),
    }
}

pub fn named_value_type_reference(
    program: &crate::TypedTrees,
    path: &crate::expression::TableNamePath,
) -> Option<TypeReferenceHandle> {
    if let Some(reference) = builtin_member_type_reference(program, path.symbol) {
        return Some(reference);
    }
    let [_] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if let Some(reference) = mathematical_parameter_type_reference(program, path.symbol) {
        return Some(reference);
    }
    if path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && let Some(parameter) = program.first_data_type_parameter_by_symbol(path.symbol)
        && let crate::data::TypeParameterKind::Const { type_reference }
        | crate::data::TypeParameterKind::Value { type_reference } = parameter.kind
    {
        return Some(type_reference);
    }
    let matches_symbol = |candidate: symbols::SymbolHandle| {
        candidate.is_valid()
            && ((path.symbol.is_valid() && candidate == path.symbol)
                || (path.head_symbol.is_valid() && candidate == path.head_symbol))
    };

    if let Some(type_reference) = hinted_named_value_type_reference(program, path, matches_symbol) {
        return Some(type_reference);
    }

    program.first_named_value_declared_type(path.symbol, path.head_symbol)
}

fn hinted_named_value_type_reference(
    program: &crate::TypedTrees,
    path: &crate::expression::TableNamePath,
    matches_symbol: impl Fn(symbols::SymbolHandle) -> bool,
) -> Option<TypeReferenceHandle> {
    let binder = if path.symbol.is_valid() {
        path.symbol
    } else {
        path.head_symbol
    };
    if !binder.is_valid() {
        return None;
    }
    let parent = program.symbols.get(binder).parent;
    if !parent.is_valid() {
        return None;
    }
    match program.symbols.get(parent).kind {
        symbols::SymbolKind::Domain => program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == parent)
            .filter(|domain| matches_symbol(domain.subject_symbol))
            .map(|domain| domain.target_type),
        symbols::SymbolKind::Data => {
            let definition = program.data_definition_by_symbol(parent)?;
            if definition.predicate_subject_symbol.is_valid()
                && matches_symbol(definition.predicate_subject_symbol)
                && program
                    .symbols
                    .get(definition.predicate_subject_symbol)
                    .parent
                    == definition.symbol
                && program
                    .symbols
                    .get(definition.predicate_subject_symbol)
                    .kind
                    == symbols::SymbolKind::Parameter
            {
                return definition
                    .predicate_subject_type
                    .is_valid()
                    .then_some(definition.predicate_subject_type);
            }
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) if matches_symbol(field.symbol) => {
                        Some(field.type_reference)
                    }
                    _ => None,
                })
        }
        symbols::SymbolKind::Variant => {
            let owner = program.symbols.get(parent).parent;
            let definition = program.data_definition_by_symbol(owner)?;
            let variant =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == parent => Some(variant),
                        _ => None,
                    })?;
            program
                .data_payload_fields(variant)
                .iter()
                .find(|field| matches_symbol(field.symbol))
                .map(|field| field.type_reference)
        }
        symbols::SymbolKind::Machine => {
            if let Some(machine) = program.machine(parent) {
                return program
                    .machine_owned_data(machine)
                    .iter()
                    .find(|owned| matches_symbol(owned.symbol))
                    .map(|owned| owned.type_reference);
            }
            None
        }
        symbols::SymbolKind::Proposition => program
            .proposition_definition_by_symbol(parent)
            .and_then(|proposition| {
                program
                    .proposition_parameters(proposition)
                    .iter()
                    .find(|parameter| matches_symbol(parameter.symbol))
                    .map(|parameter| parameter.type_reference)
            }),
        symbols::SymbolKind::Measure => program
            .measures()
            .iter()
            .find(|measure| measure.symbol == parent)
            .and_then(|measure| measure.parameter.as_ref())
            .filter(|parameter| {
                path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && parameter.symbol == path.symbol
            })
            .map(|parameter| parameter.type_reference),
        symbols::SymbolKind::State => {
            let grandparent = program.symbols.get(parent).parent;
            if program.symbols.get(grandparent).kind != symbols::SymbolKind::Machine {
                return None;
            }
            let (_, state) = program.machine_state(grandparent, parent)?;
            state_symbol_declared_type(program, state, matches_symbol)
        }
        _ => None,
    }
}

fn state_symbol_declared_type(
    program: &crate::TypedTrees,
    state: &crate::state::State,
    matches_symbol: impl Fn(SymbolHandle) -> bool,
) -> Option<TypeReferenceHandle> {
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| matches_symbol(parameter.symbol))
    {
        return Some(parameter.type_reference);
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let Some(local) = statement.as_local_data()
            && matches_symbol(local.symbol)
        {
            return Some(local.type_reference);
        }
    }
    None
}

/// Structural callable types have no nominal declaration namespace.
pub fn nominal_type_symbol(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let mut seen = Vec::new();
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || seen.contains(&reference)
        {
            return None;
        }
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => {
                return symbol.is_valid().then_some(*symbol);
            }
            TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit => return None,
        }
    }
}

/// Source receiving types distinguish replacing a reference binding from
/// writing its referent. Ownership and alias validity remain proof obligations.
pub trait AssignmentSourceTypes {
    fn typed(&self) -> &TypedTrees;
    fn declared_target_type(
        &self,
        machine: &Machine,
        state: &State,
        target: ExpressionHandle,
    ) -> Option<TypeReferenceHandle>;
    fn value_type(
        &self,
        machine: &Machine,
        state: &State,
        value: ExpressionHandle,
    ) -> Option<TypeReferenceHandle>;
    fn reference_shell(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<(language_semantics::ReferenceAccess, TypeReferenceHandle)>;
    fn atomic_assignment(
        &self,
        machine: &Machine,
        state: &State,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> bool;
}

pub fn assignment_stored_value_type(
    context: &impl AssignmentSourceTypes,
    machine: &Machine,
    state: &State,
    assignment: &crate::statement::TableAssignment,
) -> Option<TypeReferenceHandle> {
    let program = context.typed();
    if assignment.receiving_type.is_valid() {
        return program
            .type_reference_table
            .contains_type_reference(assignment.receiving_type)
            .then_some(assignment.receiving_type);
    }
    let reference = context.declared_target_type(machine, state, assignment.target)?;
    let Some((access, referee)) = context.reference_shell(reference) else {
        return Some(reference);
    };
    if context.atomic_assignment(machine, state, assignment.target, assignment.value) {
        return Some(referee);
    }
    if let Some(path) = program.expression_table.name_path_node(assignment.target)
        && program.expression_table.name_path_is_single_member(path)
        && let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == path.symbol)
        && context
            .reference_shell(parameter.type_reference)
            .is_none_or(|(access, _)| access != language_semantics::ReferenceAccess::Shared)
    {
        return Some(referee);
    }
    if access == language_semantics::ReferenceAccess::Shared {
        return Some(reference);
    }
    if program
        .expression_table
        .borrow_expression(assignment.value)
        .is_some()
    {
        return Some(reference);
    }
    let value = context.value_type(machine, state, assignment.value)?;
    if context.reference_shell(value).is_some() {
        Some(reference)
    } else {
        Some(referee)
    }
}

/// A mutable shared-reference parameter needs a local binding home only when
/// this assignment receives the reference carrier itself. Referent writes keep
/// the original incoming parameter and remain subject to access/loan checking.
pub fn assignment_rebinds_parameter(
    context: &impl AssignmentSourceTypes,
    machine: &Machine,
    state: &State,
    statement: &crate::statement::StatementNode,
) -> Result<Option<SymbolHandle>, &'static str> {
    let Some(assignment) = statement.as_assignment() else {
        return Ok(None);
    };
    let program = context.typed();
    let Some(path) = program.expression_table.name_path_node(assignment.target) else {
        return Ok(None);
    };
    if !program.expression_table.name_path_is_single_member(path) {
        return Ok(None);
    }
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == path.symbol)
    else {
        return Ok(None);
    };
    if !parameter.is_mutable {
        return Ok(None);
    }
    if !program
        .type_reference_table
        .contains_type_reference(parameter.type_reference)
    {
        return Err("rebound parameter lost its declared source type");
    }
    let Some((access, _)) = context.reference_shell(parameter.type_reference) else {
        return Ok(None);
    };
    if access != language_semantics::ReferenceAccess::Shared {
        return Ok(None);
    }
    let receiving = assignment_stored_value_type(context, machine, state, assignment)
        .ok_or("rebound parameter assignment has no resolved receiving type")?;
    if !program
        .type_reference_table
        .contains_type_reference(receiving)
    {
        return Err("rebound parameter assignment has a stale receiving type");
    }
    Ok(context.reference_shell(receiving).map(|_| parameter.symbol))
}
