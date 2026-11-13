//! Source expression meanings used when mathematical declarations share a binding.
//! The reader uses resolved declarations and carriers and supplies no body facts.

use numerics::bignum::BigInt;
use numerics::literals::{FloatFormat, FloatLiteral};
use symbols::{BuiltinType, BuiltinTypeMember, SymbolHandle, SymbolKind};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::source_operation_meaning::{
    self, SourceOperation as Value, SourceOperationReader,
};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use validation::CheckingInput;

pub(crate) fn same_meaning(
    program: &dyn CheckingInput,
    owner: SymbolHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let parameters = if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == owner)
    {
        validation::mathematical_applications::ordinary_parameters(program, definition)
    } else if let Some(machine) = program.machine_by_symbol(owner) {
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        program.state_parameters(entry).to_vec()
    } else {
        return false;
    };
    source_operation_meaning::same_meaning(
        &Reader {
            program,
            owner,
            parameters: &parameters,
            expected: None,
            substitutions: None,
        },
        left,
        right,
    )
}

struct Reader<'program> {
    program: &'program dyn CheckingInput,
    owner: SymbolHandle,
    parameters: &'program [StateParameter],
    expected: Option<TypeReferenceHandle>,
    substitutions: Option<(
        &'program [validation::mathematical_applications::PreparedSourceBinding],
        &'program [Value],
    )>,
}

impl Reader<'_> {
    fn reference(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        validation::parameter_expression_result_type_reference(
            self.program,
            self.owner,
            self.parameters,
            expression,
        )
    }

    fn anonymous_integer(&self, expression: ExpressionHandle) -> Option<BigInt> {
        validation::evaluate_anonymous_numeric_expression(
            self.program,
            expression,
            &[],
            |expression| validation::has_anonymous_operator_meaning(self.program, expression),
        )?
        .to_integer_exact()
    }

    fn mathematical_integer(&self, expression: ExpressionHandle) -> bool {
        let program = self.program;
        if validation::integer_embedding_argument(program, expression).is_some() {
            return true;
        }
        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            )
        {
            let left = self.mathematical_integer(binary.left);
            let right = self.mathematical_integer(binary.right);
            return (left && right)
                || (left && self.anonymous_integer(binary.right).is_some())
                || (right && self.anonymous_integer(binary.left).is_some());
        }
        let Some(mut reference) = self.reference(expression) else {
            return false;
        };
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    return symbol.is_valid()
                        && program.symbols.builtin_type_symbol(BuiltinType::Int) == Some(*symbol);
                }
                _ => return false,
            }
        }
    }

    fn scalar_meaning(
        &self,
        reference: TypeReferenceHandle,
        value: &Value,
    ) -> Option<(PrimitiveType, numerics::arithmetic::ArithmeticDomain)> {
        let meaning = validation::scalar_observation_meaning(self.program, reference)?;
        let observed = match value {
            Value::Observed {
                reference: observed,
                ..
            } => match validation::reference_shell(self.program, *observed) {
                Some(_) => validation::declared_carriers_match(self.program, reference, *observed),
                None => {
                    validation::reference_shell(self.program, reference).is_none()
                        && validation::declared_carriers_match(self.program, reference, *observed)
                }
            },
            _ if validation::reference_shell(self.program, reference).is_none() => true,
            Value::BoundValue(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::ScalarBinary { .. }
            | Value::Constructor { .. } => true,
            Value::Representation { target, .. } => meaning.0 == *target,
            Value::IntegerEmbedding { .. }
            | Value::MathematicalBinary { .. }
            | Value::RepeatedArray { .. }
            | Value::Unshared(_) => false,
        };
        observed.then_some(meaning)
    }

    fn binary(
        &self,
        expression: ExpressionHandle,
        operand: &dyn Fn(ExpressionHandle) -> Option<Value>,
    ) -> Option<Value> {
        let program = self.program;
        let binary = program.expression_table.binary_node(expression)?;
        if binary.operator == BinaryOperator::CaseMembership {
            return None;
        }
        let left = operand(binary.left)?;
        let right = operand(binary.right)?;
        let references = [binary.left, binary.right].map(|expression| self.reference(expression));
        if let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                self.owner,
                expression,
                spelling,
                &references,
            )
            && !validation::has_builtin_or_sealed_float_comparison_meaning(
                program,
                self.owner,
                expression,
                binary.operator,
                &references,
            )
        {
            return None;
        }
        if let Some(value) =
            validation::evaluate_anonymous_numeric_comparison(program, expression, &[], |source| {
                source == expression || validation::has_anonymous_operator_meaning(program, source)
            })
        {
            return Some(Value::boolean(value));
        }
        let mathematical =
            [binary.left, binary.right].map(|expression| self.mathematical_integer(expression));
        if mathematical.iter().any(|value| *value)
            && [binary.left, binary.right]
                .into_iter()
                .zip(mathematical)
                .all(|(expression, mathematical)| {
                    mathematical || self.anonymous_integer(expression).is_some()
                })
        {
            if matches!(
                binary.operator,
                BinaryOperator::Divide | BinaryOperator::Modulo
            ) {
                return None;
            }
            return Some(Value::MathematicalBinary {
                operator: binary.operator,
                left: Box::new(left),
                right: Box::new(right),
            });
        }
        let references = validation::builtin_scalar_operand_references(
            program,
            binary,
            references,
            self.expected,
        )?;
        Some(Value::ScalarBinary {
            operator: binary.operator,
            meaning: [
                self.scalar_meaning(references[0], &left)?,
                self.scalar_meaning(references[1], &right)?,
            ],
            left: Box::new(left),
            right: Box::new(right),
        })
    }
}

impl SourceOperationReader for Reader<'_> {
    fn read(
        &self,
        expression: ExpressionHandle,
        operand: &dyn Fn(ExpressionHandle) -> Option<Value>,
    ) -> Option<Value> {
        let program = self.program;
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        if program.static_type_predicate_operands(expression).is_some() {
            return Some(
                program
                    .static_type_predicate_value(expression)
                    .map_or(Value::Unshared(expression), Value::boolean),
            );
        }
        if let Some((bindings, values)) = self.substitutions {
            if let Some((_, value)) = bindings
                .iter()
                .zip(values)
                .find(|(binding, _)| binding.expressions.contains(&expression))
            {
                return Some(value.clone());
            }
            if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
                && path.symbol.is_valid()
            {
                let mut selected = bindings
                    .iter()
                    .zip(values)
                    .filter(|(binding, _)| binding.symbol == path.symbol);
                if let Some((_, value)) = selected.next() {
                    return selected.next().is_none().then(|| value.clone());
                }
            }
        }
        if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
            && path.symbol.is_valid()
            && ((program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
                && self
                    .parameters
                    .iter()
                    .any(|parameter| parameter.symbol == path.symbol))
                || program.symbols.get(path.symbol).kind == SymbolKind::Const)
        {
            return Some(Value::BoundValue(path.symbol));
        }
        if let Some((_, argument)) = validation::integer_embedding_argument(program, expression) {
            return Some(Value::IntegerEmbedding {
                reference: self.reference(argument)?,
                value: Box::new(operand(argument)?),
            });
        }
        if let Some(value) = self.anonymous_integer(expression) {
            return Some(Value::Integer(value));
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if program.symbols.builtin_type_member(path.symbol)
                    == Some(BuiltinTypeMember::AddressBound)
                {
                    return Some(Value::BoundValue(path.symbol));
                }
                if validation::reserved_result_owner(program, expression)
                    .is_some_and(|(owner, _)| owner == self.owner)
                {
                    return None;
                }
                if let Some(definition) =
                    validation::exact_case_reference_owner(program, expression)
                {
                    let case = program.data_members(definition).iter().find_map(|member| {
                        member
                            .as_variant()
                            .filter(|case| case.symbol == path.symbol)
                    })?;
                    if !program.data_payload_fields(case).is_empty() {
                        return None;
                    }
                    let mut fields = Vec::new();
                    for field in program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| member.as_field())
                    {
                        if field.relevance.is_erased() {
                            return None;
                        }
                        fields.push((field.symbol, self.zero_value(field.type_reference)?));
                    }
                    let reference = self.reference(expression).or_else(|| {
                        program
                            .type_reference_table
                            .find_named_type_reference(definition.symbol)
                    })?;
                    return Some(Value::Constructor {
                        carrier: program.normalized_type_identity(reference),
                        case: case.symbol,
                        fields,
                    });
                }
                if !path.symbol.is_valid()
                    || program.symbols.get(path.symbol).kind == SymbolKind::Field
                {
                    return None;
                }
                Some(Value::BoundValue(path.symbol))
            }
            ExpressionNode::Integer(value) => value.value_bignum().map(Value::Integer),
            ExpressionNode::Float(value) => Some(Value::Float(value.clone())),
            ExpressionNode::Boolean(value) => Some(Value::boolean(*value)),
            ExpressionNode::Binary(_) => self.binary(expression, operand),
            ExpressionNode::Unary(unary) => {
                if unary.operator == typed_trees::expression::UnaryOperator::BitwiseNot {
                    let primitive = self
                        .reference(unary.operand)
                        .and_then(|reference| program.primitive_type_reference(reference))?;
                    return (primitive.is_fixed_integer()
                        && self
                            .reference(expression)
                            .and_then(|reference| program.primitive_type_reference(reference))
                            == Some(primitive))
                    .then_some(Value::Unshared(expression));
                }
                if unary.operator != typed_trees::expression::UnaryOperator::LogicalNot {
                    return None;
                }
                let reference = self.reference(unary.operand)?;
                if program.primitive_type_reference(reference) != Some(PrimitiveType::Bool) {
                    return None;
                }
                let value = operand(unary.operand)?;
                let meaning = self.scalar_meaning(reference, &value)?;
                Some(Value::ScalarBinary {
                    operator: BinaryOperator::Equal,
                    meaning: [meaning; 2],
                    left: Box::new(value),
                    right: Box::new(Value::boolean(false)),
                })
            }
            ExpressionNode::Cast(cast) => self.cast(expression, cast, operand),
            ExpressionNode::Borrow(borrow) => {
                let value = operand(borrow.target)?;
                if borrow.access != language_semantics::ReferenceAccess::Shared
                    || self.reference(borrow.target).is_some_and(|reference| {
                        validation::reference_shell(program, reference).is_some()
                    })
                {
                    Some(value)
                } else {
                    Some(Value::Observed {
                        reference: self.reference(expression)?,
                        value: Box::new(value),
                    })
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                self.constructor(expression, literal, operand)
            }
            ExpressionNode::Member(member) => {
                let Some(Value::Constructor { fields, .. }) = operand(member.receiver) else {
                    return Some(Value::Unshared(expression));
                };
                fields
                    .into_iter()
                    .find(|(symbol, _)| *symbol == member.member_symbol)
                    .map(|(_, value)| value)
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::ArrayLiteral(elements) =
                    program.expression_table.expression(indexed.collection)
                else {
                    return None;
                };
                let Value::Integer(index) = operand(indexed.index)? else {
                    return None;
                };
                let index = usize::try_from(index.to_u64()?).ok()?;
                operand(
                    *program
                        .expression_table
                        .expression_handles(*elements)
                        .get(index)?,
                )
            }
            ExpressionNode::ZeroValue(reference) => self.zero_value(*reference),
            ExpressionNode::Call(call)
                if validation::mathematical_applications::is_mathematical_call(program, call) =>
            {
                self.mathematical_application(expression, operand)
            }
            ExpressionNode::Call(_) => Some(Value::Unshared(expression)),
            _ => None,
        }
    }
}

impl Reader<'_> {
    fn cast(
        &self,
        expression: ExpressionHandle,
        cast: &typed_trees::expression::TableCastExpression,
        operand: &dyn Fn(ExpressionHandle) -> Option<Value>,
    ) -> Option<Value> {
        let program = self.program;
        let value = operand(cast.value)?;
        let Some(mut source) = self.reference(cast.value) else {
            if cast.form.is_recast()
                || !validation::has_anonymous_numeric_results(program, cast.value)
            {
                return None;
            }
            let primitive = program
                .primitive_type_reference(cast.target_type)
                .filter(|primitive| primitive.is_fixed_integer())?;
            let Value::Integer(_) = value.semantic_value() else {
                return None;
            };
            validation::land_anonymous_integer_expression(
                program,
                cast.value,
                primitive,
                &[],
                |expression| validation::has_anonymous_operator_meaning(program, expression),
            )?;
            return Some(Value::Observed {
                reference: self.reference(expression)?,
                value: Box::new(value),
            });
        };
        if !cast.form.is_recast()
            && validation::reference_shell(program, source).is_some()
            && self.scalar_meaning(source, &value).is_some()
        {
            source = validation::readable_observation_type_reference(program, source)?;
        }
        let integer = |reference| {
            program
                .primitive_type_reference(reference)
                .is_some_and(|primitive| {
                    primitive.is_fixed_integer() || primitive == PrimitiveType::Addr
                })
        };
        let preserves_value = !cast.form.is_recast()
            && ((integer(source) && integer(cast.target_type))
                || (validation::reference_shell(program, source).is_none()
                    && validation::reference_shell(program, cast.target_type).is_none()
                    && validation::declared_carriers_match(program, source, cast.target_type)
                    && program.arithmetic_domain_for_type_reference(source)
                        == program.arithmetic_domain_for_type_reference(cast.target_type)));
        if preserves_value {
            if integer(source) && integer(cast.target_type) {
                self.scalar_meaning(source, &value)?;
                let result = self.reference(expression)?;
                if program.primitive_type_reference(result)
                    == program.primitive_type_reference(source)
                    && program.arithmetic_domain_for_type_reference(result)
                        == program.arithmetic_domain_for_type_reference(source)
                {
                    return Some(value);
                }
                return Some(Value::Observed {
                    reference: result,
                    value: Box::new(Value::IntegerEmbedding {
                        reference: source,
                        value: Box::new(value),
                    }),
                });
            }
            return Some(value);
        }
        if cast.form.recast_access() != Some(language_semantics::ReferenceAccess::Shared) {
            return None;
        }
        let borrowed_observation = validation::reference_shell(program, source).is_none()
            && matches!(&value, Value::Observed { reference, .. } if validation::reference_shell(program, *reference).is_some_and(|shell| shell.access == language_semantics::ReferenceAccess::Shared && validation::declared_carriers_match(program, source, shell.referee)));
        if !borrowed_observation {
            self.scalar_meaning(source, &value)?;
        }
        let source = match validation::reference_shell(program, source) {
            Some(shell) if shell.access == language_semantics::ReferenceAccess::Shared => {
                shell.referee
            }
            Some(_) => return None,
            None => source,
        };
        let source = program.primitive_type_reference(source)?;
        let target = program.primitive_type_reference(cast.target_type)?;
        let scalar = |primitive: PrimitiveType| {
            primitive.is_fixed_integer()
                || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
        };
        if !scalar(source)
            || !scalar(target)
            || source.scalar_byte_size() != target.scalar_byte_size()
        {
            return None;
        }
        Some(normalize_representation(source, target, value))
    }

    fn constructor(
        &self,
        expression: ExpressionHandle,
        literal: &typed_trees::expression::TableStructLiteral,
        operand: &dyn Fn(ExpressionHandle) -> Option<Value>,
    ) -> Option<Value> {
        let program = self.program;
        let definition =
            validation::selected_construction_definition(program, expression, literal)?;
        let case = match literal.case_symbol {
            Some(_) => Some(validation::selected_construction_case(
                program, expression, literal, definition,
            )?),
            None => {
                if literal.case_name.is_some()
                    || program
                        .data_members(definition)
                        .iter()
                        .any(|member| member.as_variant().is_some())
                {
                    return None;
                }
                None
            }
        };
        let declared = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .chain(
                case.into_iter()
                    .flat_map(|case| program.data_payload_fields(case)),
            )
            .collect::<Vec<_>>();
        let supplied = program.expression_table.struct_fields(literal.fields);
        let selected = supplied
            .iter()
            .map(|field| {
                validation::selected_construction_field(
                    program,
                    expression,
                    literal,
                    field.field_symbol,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        if selected.iter().any(|field| {
            !field.symbol.is_valid()
                || !declared
                    .iter()
                    .any(|declared| declared.symbol == field.symbol)
        }) {
            return None;
        }
        let mut fields = Vec::with_capacity(declared.len());
        for field in declared {
            let mut occurrences =
                supplied
                    .iter()
                    .zip(&selected)
                    .filter_map(|(supplied, selected)| {
                        (selected.symbol == field.symbol).then_some(supplied)
                    });
            let value = if let Some(supplied) = occurrences.next() {
                if occurrences.next().is_some() {
                    return None;
                }
                operand(supplied.value)?
            } else {
                if field.relevance.is_erased()
                    || validation::type_requires_establishment(program, field.type_reference)
                    || validation::proof_only_classification(program)
                        .proof_only_mention(program, field.type_reference)
                        .is_some()
                {
                    return None;
                }
                self.zero_value(field.type_reference)?
            };
            fields.push((field.symbol, value));
        }
        let reference = self.reference(expression)?;
        Some(Value::Constructor {
            carrier: program.normalized_type_identity(reference),
            case: case.map_or_else(SymbolHandle::invalid, |case| case.symbol),
            fields,
        })
    }

    fn zero_value(&self, reference: TypeReferenceHandle) -> Option<Value> {
        if !reference.is_valid()
            || validation::proof_only_classification(self.program)
                .proof_only_mention(self.program, reference)
                .is_some()
        {
            return None;
        }
        zero_value(self.program, reference, &mut Vec::new())
    }
}

fn normalize_representation(source: PrimitiveType, target: PrimitiveType, value: Value) -> Value {
    if source == target {
        return value;
    }
    if let Value::Representation {
        source: original,
        target: intermediate,
        value: original_value,
    } = value.semantic_value()
        && *intermediate == source
    {
        return normalize_representation(*original, target, *original_value.clone());
    }
    if let Some(value) = represented_literal(source, target, &value) {
        return value;
    }
    Value::Representation {
        source,
        target,
        value: Box::new(value),
    }
}

fn represented_literal(
    source: PrimitiveType,
    target: PrimitiveType,
    value: &Value,
) -> Option<Value> {
    use numerics::bignum::BigInt;
    use numerics::literals::{FloatFormat, FloatLiteral};
    let width = u32::try_from(source.scalar_byte_size()?).ok()? * 8;
    if source.scalar_byte_size() != target.scalar_byte_size() || width > 64 || width == 0 {
        return None;
    }
    let bits = match value.semantic_value() {
        Value::Integer(value) if source.is_fixed_integer() => {
            if source.is_signed_integer() {
                value.to_i64()? as u64
            } else {
                value.to_u64()?
            }
        }
        Value::Float(value) => {
            let format = match source {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return None,
            };
            if value.landing().is_some_and(|landing| landing != format) {
                return None;
            }
            if let Some((presented_format, bits)) = value.presented_bits() {
                if presented_format != format {
                    return None;
                }
                bits
            } else {
                match format {
                    FloatFormat::F32 if !value.value_f32().is_nan() => {
                        u64::from(value.value_f32().to_bits())
                    }
                    FloatFormat::F64 if !value.value_f64().is_nan() => value.value_f64().to_bits(),
                    _ => return None,
                }
            }
        }
        _ => return None,
    } & (u64::MAX >> (64 - width));
    Some(match target {
        PrimitiveType::F32 => Value::Float(FloatLiteral::from_bits(FloatFormat::F32, bits)?),
        PrimitiveType::F64 => Value::Float(FloatLiteral::from_bits(FloatFormat::F64, bits)?),
        primitive if primitive.is_fixed_integer() => {
            Value::Integer(if primitive.is_signed_integer() {
                let shift = 64 - width;
                BigInt::from_i64(((bits << shift) as i64) >> shift)
            } else {
                BigInt::from_u64(bits)
            })
        }
        _ => return None,
    })
}

fn zero_value(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<SymbolHandle>,
) -> Option<Value> {
    if validation::type_requires_establishment(program, reference) {
        return None;
    }
    if let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            match constraint {
                typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                typed_trees::types::TypeConstraintNode::Domain(_)
                | typed_trees::types::TypeConstraintNode::Named(_) => return None,
            }
        }
        return zero_value(program, *base_type, active);
    }
    if let Some(primitive) = program.type_reference_table.primitive_type(reference) {
        return match primitive {
            PrimitiveType::Bool => Some(Value::boolean(false)),
            PrimitiveType::F32 | PrimitiveType::F64 => Some(Value::Float(
                FloatLiteral::from_f64(0.0).with_landing(if primitive == PrimitiveType::F32 {
                    FloatFormat::F32
                } else {
                    FloatFormat::F64
                }),
            )),
            primitive if primitive.accepts_integer_literal() => {
                Some(Value::Integer(BigInt::zero()))
            }
            _ => None,
        };
    }
    if let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    {
        let element = if *length == 0 {
            None
        } else {
            Some(Box::new(zero_value(program, *element_type, active)?))
        };
        return Some(Value::RepeatedArray {
            length: u64::try_from(*length).ok()?,
            element,
        });
    }
    let symbol = program.type_reference_table.type_head_symbol(reference)?;
    if !symbol.is_valid() || active.contains(&symbol) {
        return None;
    }
    let definition = program.data_constructor_definition(reference, symbol)?;
    if validation::data_requires_establishment(program, definition) {
        return None;
    }
    active.push(symbol);
    let result = (|| {
        let case = program
            .data_members(definition)
            .iter()
            .find_map(|member| member.as_variant());
        let mut fields = Vec::new();
        for field in program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .chain(
                case.into_iter()
                    .flat_map(|case| program.data_payload_fields(case)),
            )
        {
            if field.relevance.is_erased() {
                return None;
            }
            fields.push((
                field.symbol,
                zero_value(program, field.type_reference, active)?,
            ));
        }
        Some(Value::Constructor {
            carrier: program.normalized_type_identity(reference),
            case: case.map_or_else(SymbolHandle::invalid, |case| case.symbol),
            fields,
        })
    })();
    active.pop();
    result
}

impl Reader<'_> {
    fn mathematical_application(
        &self,
        expression: ExpressionHandle,
        operand: &dyn Fn(ExpressionHandle) -> Option<Value>,
    ) -> Option<Value> {
        let program = self.program;
        let view = program.prepared_mathematics()?;
        let application = view.application(expression)?;
        let captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .iter()
            .map(|capture| {
                let mut value = operand(capture.expression)?;
                let mut reference = capture.source_type;
                if validation::reference_shell(program, reference).is_some()
                    && program
                        .primitive_type_reference(capture.observation_type)
                        .is_some()
                {
                    validation::mathematical_applications::subject_observation_type_reference(
                        program,
                        reference,
                        capture.observation_type,
                    )?;
                    self.scalar_meaning(reference, &value)?;
                    value = Value::Observed {
                        reference: capture.observation_type,
                        value: Box::new(value),
                    };
                    reference = capture.observation_type;
                }
                if program
                    .primitive_type_reference(reference)
                    .is_some_and(|primitive| {
                        primitive.is_fixed_integer() || primitive == PrimitiveType::Addr
                    })
                {
                    value = Value::IntegerEmbedding {
                        reference,
                        value: Box::new(value),
                    };
                } else if validation::reference_shell(program, capture.source_type).is_none()
                    && program
                        .type_reference_table
                        .type_head_symbol(capture.source_type)
                        .and_then(|symbol| program.data_definition_by_symbol(symbol))
                        .is_some()
                {
                    value = Value::Observed {
                        reference: capture.source_type,
                        value: Box::new(value),
                    };
                }
                Some(Some(value))
            })
            .collect::<Option<Vec<_>>>()?;
        self.read_prepared(view, application, application.term, &captures, 0)
    }

    fn read_prepared(
        &self,
        view: validation::mathematical_applications::PreparedMathematicalView<'_>,
        application: &validation::mathematical_applications::PreparedApplication,
        term: proof_admission::TermHandle,
        captures: &[Option<Value>],
        depth: usize,
    ) -> Option<Value> {
        use proof_admission::Term;
        use validation::mathematical_applications::PreparedConstantMeaning as Meaning;
        if depth >= 128 {
            return None;
        }
        let program = self.program;
        let read = |term| self.read_prepared(view, application, term, captures, depth + 1);
        let source_captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .len();
        let inserted_binders = u32::try_from(captures.len().checked_sub(source_captures)?).ok()?;
        if view
            .identity_value(application, term, inserted_binders)
            .is_some()
        {
            return None;
        }
        let binary = |operator, left, right| Value::MathematicalBinary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        };
        match view.arena.get(term) {
            Term::TwoZero | Term::TwoOne => {
                Some(Value::boolean(matches!(view.arena.get(term), Term::TwoOne)))
            }
            Term::Variable(index) => captures
                .get(captures.len().checked_sub(index as usize + 1)?)?
                .clone(),
            Term::Squash { ty } => read(ty),
            Term::Id { ty, left, right } if matches!(view.arena.get(ty), Term::Two) => {
                let symbol = program
                    .symbols
                    .child_handles(program.symbols.root())?
                    .find(|symbol| {
                        program.symbols.builtin_type_atom(*symbol)
                            == Some(symbols::BuiltinTypeAtom::Bool)
                    })?;
                let reference = program
                    .type_reference_table
                    .find_named_type_reference(symbol)?;
                let meaning = (
                    PrimitiveType::Bool,
                    program.arithmetic_domain_for_type_reference(reference),
                );
                Some(Value::ScalarBinary {
                    operator: BinaryOperator::Equal,
                    meaning: [meaning; 2],
                    left: Box::new(read(left)?),
                    right: Box::new(read(right)?),
                })
            }
            Term::Id { ty, left, right } => {
                let Term::Constant { declaration, .. } = view.arena.get(ty) else {
                    return None;
                };
                if !matches!(
                    view.constant(declaration),
                    Some(Meaning::IntegerCarrier | Meaning::AddressCarrier)
                ) {
                    return None;
                }
                Some(binary(BinaryOperator::Equal, read(left)?, read(right)?))
            }
            Term::Sigma { domain, codomain } => {
                let mut extended = captures.to_vec();
                extended.push(None);
                if matches!(view.arena.get(domain), Term::Two) {
                    let Term::CaseTwo {
                        zero_branch,
                        one_branch,
                        scrutinee,
                        ..
                    } = view.arena.get(codomain)
                    else {
                        return None;
                    };
                    if !matches!(view.arena.get(scrutinee), Term::Variable(0)) {
                        return None;
                    }
                    return Some(binary(
                        BinaryOperator::Or,
                        self.read_prepared(view, application, zero_branch, &extended, depth + 1)?,
                        self.read_prepared(view, application, one_branch, &extended, depth + 1)?,
                    ));
                }
                Some(binary(
                    BinaryOperator::And,
                    read(domain)?,
                    self.read_prepared(view, application, codomain, &extended, depth + 1)?,
                ))
            }
            _ => {
                let (declaration, arguments) =
                    validation::mathematical_applications::application_spine(view, term)?;
                match (view.constant(declaration)?, arguments.as_slice()) {
                    (Meaning::Integer(value), []) => Some(Value::Integer(value.clone())),
                    (Meaning::FloatLiteral { format, bits }, []) => {
                        Some(Value::Float(FloatLiteral::from_bits(*format, *bits)?))
                    }
                    (
                        Meaning::FloatRelation {
                            operator,
                            reference,
                            ..
                        },
                        [left, right],
                    ) => {
                        let primitive = program.primitive_type_reference(*reference)?;
                        let policy = program.arithmetic_domain_for_type_reference(*reference);
                        Some(Value::ScalarBinary {
                            operator: *operator,
                            meaning: [(primitive, policy); 2],
                            left: Box::new(read(*left)?),
                            right: Box::new(read(*right)?),
                        })
                    }
                    (Meaning::AddressBound(symbol), [])
                        if program.symbols.builtin_type_member(*symbol)
                            == Some(BuiltinTypeMember::AddressBound) =>
                    {
                        Some(Value::BoundValue(*symbol))
                    }
                    (
                        Meaning::IntegerOperation(operator) | Meaning::IntegerRelation(operator),
                        [left, right],
                    ) => Some(binary(*operator, read(*left)?, read(*right)?)),
                    (Meaning::IntegerEmbedding { .. }, [argument]) => read(*argument),
                    (Meaning::Negation, [proposition]) => {
                        let Value::MathematicalBinary {
                            operator,
                            left,
                            right,
                        } = read(*proposition)?
                        else {
                            return None;
                        };
                        Some(Value::MathematicalBinary {
                            operator: operator.complement()?,
                            left,
                            right,
                        })
                    }
                    (
                        Meaning::SourceOperation {
                            expression,
                            owner,
                            source_type,
                            bindings,
                        },
                        _,
                    ) => self.substituted_source(
                        *expression,
                        *owner,
                        *source_type,
                        bindings,
                        &arguments,
                        read,
                    ),
                    _ => None,
                }
            }
        }
    }

    fn substituted_source(
        &self,
        expression: ExpressionHandle,
        owner: SymbolHandle,
        source_type: TypeReferenceHandle,
        bindings: &[validation::mathematical_applications::PreparedSourceBinding],
        arguments: &[proof_admission::TermHandle],
        read: impl Fn(proof_admission::TermHandle) -> Option<Value>,
    ) -> Option<Value> {
        use validation::mathematical_applications::PreparedSourceBindingKind;
        let program = self.program;
        if bindings.len() != arguments.len()
            || !program.expression_table.expression_is_valid(expression)
            || !owner.is_valid()
        {
            return None;
        }
        let values = bindings
            .iter()
            .zip(arguments)
            .map(|(binding, argument)| {
                if binding.kind == PreparedSourceBindingKind::Type {
                    return None;
                }
                read(*argument)
            })
            .collect::<Option<Vec<_>>>()?;
        let mut parameters = if let Some(definition) = program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == owner)
        {
            validation::mathematical_applications::ordinary_parameters(program, definition)
        } else {
            let machine = program.machine_by_own_or_state_symbol(owner)?;
            let state = program
                .state_in_machine(machine, owner)
                .or_else(|| program.machine_states(machine).first())?;
            program.state_parameters(state).to_vec()
        };
        for binding in bindings {
            if binding.symbol.is_valid() && binding.source_type.is_valid() {
                if let Some(parameter) = parameters
                    .iter_mut()
                    .find(|parameter| parameter.symbol == binding.symbol)
                {
                    parameter.type_reference = binding.source_type;
                } else {
                    parameters.push(StateParameter {
                        symbol: binding.symbol,
                        name: program.symbols.name(binding.symbol).into(),
                        type_reference: binding.source_type,
                        ..Default::default()
                    });
                }
            }
        }
        let reader = Reader {
            program,
            owner,
            parameters: &parameters,
            expected: Some(source_type),
            substitutions: Some((bindings, &values)),
        };
        let value = source_operation_meaning::read_operation(&reader, expression)?;
        Some(if source_type.is_valid() {
            Value::Observed {
                reference: source_type,
                value: Box::new(value),
            }
        } else {
            value
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::IntegerLiteral;
    use typed_trees::TypedTrees;
    use typed_trees::expression::TableBinaryExpression;

    #[test]
    fn closed_arithmetic_shares_its_value_and_rejects_a_different_result() {
        let mut program = TypedTrees::default();
        let one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let three = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
        let sum = program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: two,
                policy: ArithmeticDomain::Exact,
            }));
        let changed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: one,
                    operator: BinaryOperator::Add,
                    right: three,
                    policy: ArithmeticDomain::Exact,
                }));
        let reader = Reader {
            program: &program,
            owner: SymbolHandle::invalid(),
            parameters: &[],
            expected: None,
            substitutions: None,
        };
        assert!(source_operation_meaning::same_meaning(&reader, sum, three));
        assert!(!source_operation_meaning::same_meaning(
            &reader, changed, three
        ));
    }

    #[test]
    fn an_ordinary_call_cannot_create_a_shared_mathematical_binding() {
        let mut program = TypedTrees::default();
        let call = program
            .expression_table
            .insert(ExpressionNode::Call(Default::default()));
        let reader = Reader {
            program: &program,
            owner: SymbolHandle::invalid(),
            parameters: &[],
            expected: None,
            substitutions: None,
        };
        assert!(!source_operation_meaning::same_meaning(&reader, call, call));
    }
}
