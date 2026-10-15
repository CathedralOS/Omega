//! Exact invocation predicates over Boolean fields, total integer
//! comparisons, and IEEE comparisons with exact literal endpoints.
use crate::values::operator_is_builtin;

use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use symbols::BuiltinTypeAtom;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn lower_machine_entry_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    expression: ExpressionHandle,
    _exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let parameters = super::entry_parameters(program, machine)?;
    let predicate = Reader {
        program,
        operators,
        machine: Some(machine),
        owner: machine.symbol,
        parameters,
        remaining: 4096,
        subjects: None,
    }
    .boolean(expression, 0)?;
    // All operands were checked in the exact entry namespace. This separate
    // selected-meaning owner prevents authored operations from becoming logic.
    validation::has_builtin_bound_expression_meaning(
        program,
        machine,
        program.machine_states(machine).first(),
        expression,
    )
    .then_some(predicate)
}

type EntrySubjectResolver<'program> = dyn FnMut(ExpressionHandle) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>
    + 'program;

/// Read the same predicate algebra while the caller supplies authenticated
/// invocation-entry subjects for observations made in a body state.
pub(crate) fn lower_entry_predicate(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    subjects: &mut EntrySubjectResolver<'_>,
) -> Option<CheckedBooleanExpression> {
    let parameters = super::authored_entry_parameters(program, machine)?;
    let predicate = Reader {
        program,
        operators,
        machine: Some(machine),
        owner: machine.symbol,
        parameters,
        remaining: 4096,
        subjects: Some(subjects),
    }
    .boolean(expression, 0)?;
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
        .then_some(predicate)
}

/// Scalar actuals use the ordinary checked algebra with observations rebound
/// to authenticated invocation-entry subjects. This does not replay stored
/// initializer expressions or establish arithmetic safety by itself.
pub(crate) fn lower_entry_scalar(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    primitive_type: PrimitiveType,
    subjects: &impl Fn(
        &validation::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>,
) -> Option<CheckedScalarExpression> {
    if primitive_type == PrimitiveType::Bool {
        return lower_entry_predicate(
            program,
            operators,
            machine,
            state,
            expression,
            &mut |leaf| {
                let place = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    before_statement,
                    leaf,
                )?;
                subjects(&place)
            },
        )
        .map(|value| CheckedScalarExpression::Boolean(Box::new(value)));
    }
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let mut reader = Reader {
        program,
        operators,
        machine: Some(machine),
        owner: machine.symbol,
        parameters: super::authored_entry_parameters(program, machine)?,
        remaining: 4096,
        subjects: None,
    };
    crate::values::lower_unit_scalar_argument_for_places(
        program,
        operators,
        state,
        before_statement,
        expression,
        primitive_type,
        |place, primitive| {
            let (position, path) = subjects(place)?;
            let parameter_position = usize::try_from(position).ok()?;
            let parameter = reader.parameters.get(parameter_position)?;
            if parameter.is_const || parameter.relevance.is_erased() {
                return None;
            }
            let type_reference = super::super::structural_fields::readable_entry_parameter_path(
                program,
                reader.parameters,
                position,
                &path,
                &mut reader.remaining,
            )?;
            if reader.primitive(type_reference, 0)??.0 != primitive {
                return None;
            }
            Some(if path.is_empty() {
                CheckedScalarExpression::Parameter {
                    position: reader.parameters[..parameter_position]
                        .iter()
                        .filter(|parameter| {
                            crate::values::scalar::occupies_scalar_position(program, parameter)
                        })
                        .count(),
                    primitive_type: primitive,
                }
            } else {
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: position,
                    path,
                    primitive_type: primitive,
                }
            })
        },
    )
}

struct Reader<'program> {
    program: &'program dyn ::validation::CheckingInput,
    operators: &'program CheckedOperatorFacts,
    machine: Option<&'program Machine>,
    owner: symbols::SymbolHandle,
    parameters: &'program [StateParameter],
    remaining: usize,
    subjects: Option<&'program mut EntrySubjectResolver<'program>>,
}

/// Bodyless requirements have an exact signature namespace, not a synthetic
/// machine body. Scalar and structural leaves retain that signature's exact
/// authored formal namespace.
pub(crate) fn lower_signature_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    signature: &typed_trees::signature::StateSignature,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if !signature.symbol.is_valid()
        || !program
            .state_signature_type_parameters(signature)
            .is_empty()
    {
        return None;
    }
    let parameters = program.state_signature_parameters(signature);
    for (position, parameter) in parameters.iter().enumerate() {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol)
        {
            return None;
        }
    }
    Reader {
        program,
        operators,
        machine: None,
        owner: signature.symbol,
        parameters,
        remaining: 4096,
        subjects: None,
    }
    .boolean(expression, 0)
}

/// An operator declaration publishes its crash routes over its own formal
/// parameters, exactly as a bodyless signature does: the structured form
/// binds scalar formals by dense scalar position, which the Terminal
/// operation-contract telescope reads as formal `k + 1`. Structural leaves
/// retain authored formal positions. A generic operator has no closed
/// telescope yet, so it keeps identity without inventing executable operands.
pub(crate) fn lower_operator_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    operator: &typed_trees::operator::OperatorDefinition,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if !operator.symbol.is_valid()
        || !program.operator_type_parameters(operator).is_empty()
        || !operator.lifetime_parameters.is_empty()
    {
        return None;
    }
    let parameters = program.operator_parameters(operator);
    for (position, parameter) in parameters.iter().enumerate() {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol)
        {
            return None;
        }
    }
    Reader {
        program,
        operators,
        machine: None,
        owner: operator.symbol,
        parameters,
        remaining: 4096,
        subjects: None,
    }
    .boolean(expression, 0)
}

struct FieldPath {
    parameter_position: usize,
    path: Vec<CheckedStructuralPredicatePathSegment>,
    type_reference: TypeReferenceHandle,
}

impl<'program> Reader<'program> {
    fn charge(&mut self, depth: usize) -> Option<()> {
        if depth >= 64 || self.remaining == 0 {
            self.remaining = 0;
            return None;
        }
        self.remaining -= 1;
        Some(())
    }

    fn boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        self.charge(depth)?;
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                let subject = self.subject(expression, depth + 1)?;
                if self.primitive(subject.type_reference, depth + 1)?
                    != Some((PrimitiveType::Bool, BuiltinTypeAtom::Bool))
                {
                    return None;
                }
                if subject.path.is_empty() {
                    Some(CheckedBooleanExpression::Parameter {
                        position: self.scalar_position(subject.parameter_position, depth + 1)?,
                    })
                } else {
                    Some(CheckedBooleanExpression::StructuralParameterField {
                        parameter_position: u32::try_from(subject.parameter_position).ok()?,
                        path: subject.path,
                    })
                }
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) && operator_is_builtin(self.operators, expression) =>
            {
                if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    && self.has_float_operand([binary.left, binary.right], depth + 1)?
                {
                    return self.float_comparison(
                        expression,
                        [binary.left, binary.right],
                        binary.operator,
                        depth + 1,
                    );
                }
                if matches!(
                    binary.operator,
                    BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) || (matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && self.has_integer_operand([binary.left, binary.right], depth + 1)?)
                {
                    return self.integer_comparison(
                        expression,
                        [binary.left, binary.right],
                        depth + 1,
                    );
                }
                let left = Box::new(self.boolean(binary.left, depth + 1)?);
                let right = Box::new(self.boolean(binary.right, depth + 1)?);
                if self.machine.is_none()
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                    && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.owner,
                        expression,
                        if binary.operator == BinaryOperator::Equal {
                            language_core::OperatorSpelling::Equal
                        } else {
                            language_core::OperatorSpelling::NotEqual
                        },
                        &[None, None],
                    )
                {
                    return None;
                }
                Some(match binary.operator {
                    BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                    BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                    BinaryOperator::Equal => CheckedBooleanExpression::Equal { left, right },
                    BinaryOperator::NotEqual => {
                        CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Equal {
                            left,
                            right,
                        }))
                    }
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    fn has_integer_operand(
        &mut self,
        operands: [ExpressionHandle; 2],
        depth: usize,
    ) -> Option<bool> {
        for operand in operands {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(operand) {
                return None;
            }
            match self.program.expression_table.expression(operand) {
                ExpressionNode::Integer(_) => return Some(true),
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    let subject = self.subject(operand, depth + 1)?;
                    if self
                        .primitive(subject.type_reference, depth + 1)?
                        .is_some_and(|(_, atom)| fixed_integer_atom(atom))
                    {
                        return Some(true);
                    }
                }
                _ => {}
            }
        }
        Some(false)
    }

    fn integer_comparison(
        &mut self,
        expression: ExpressionHandle,
        operands: [ExpressionHandle; 2],
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        // Dense scalar positions depend on the entire entry telescope, including
        // unread formals. Validate its type chains before counting positions;
        // dummy/stale types and cycles cannot change the entry namespace.
        for parameter in self.parameters {
            if parameter.is_const
                || self.program.symbols.name(parameter.symbol) != parameter.name.as_str()
            {
                return None;
            }
            self.primitive(parameter.type_reference, depth + 1)?;
        }
        let mut subjects = [None, None];
        for (position, operand) in operands.into_iter().enumerate() {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(operand) {
                return None;
            }
            match self.program.expression_table.expression(operand) {
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    let subject = self.subject(operand, depth + 1)?;
                    let (primitive_type, atom) =
                        self.primitive(subject.type_reference, depth + 1)??;
                    if !fixed_integer_atom(atom) {
                        return None;
                    }
                    let value = self.subject_scalar(&subject, primitive_type, depth + 1)?;
                    subjects[position] = Some((value, subject.type_reference));
                }
                ExpressionNode::Integer(_) => {}
                // This slice establishes no totality for arithmetic, calls,
                // casts, result values, or current body storage.
                _ => return None,
            }
        }
        // Literal landing, same-carrier checks, and selected operator meaning
        // stay with the existing numeric contract owner. Trapping-qualified
        // inputs are legal: the comparison itself is a total operation.
        super::super::result_contract::lower_integer_contract_comparison(
            self.program,
            self.operators,
            self.owner,
            expression,
            subjects,
        )
    }

    /// IEEE relations retain their own meaning, including unordered inputs.
    /// Read exact formal identities and already-landed literal bits; never
    /// reinterpret them as integer order or round them through a host float.
    fn float_comparison(
        &mut self,
        expression: ExpressionHandle,
        operands: [ExpressionHandle; 2],
        _operator: BinaryOperator,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        // Dense scalar positions depend on the entire entry telescope, exactly
        // as integer comparisons do.
        for parameter in self.parameters {
            if parameter.is_const
                || self.program.symbols.name(parameter.symbol) != parameter.name.as_str()
            {
                return None;
            }
            self.primitive(parameter.type_reference, depth + 1)?;
        }
        let mut subjects = [None, None];
        for (position, operand) in operands.into_iter().enumerate() {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(operand) {
                return None;
            }
            if let ExpressionNode::Float(literal) =
                self.program.expression_table.expression(operand)
            {
                let value = typed_trees::expression::ieee_float_value(literal, literal.landing()?);
                let type_reference = validation::parameter_expression_result_type_reference(
                    self.program,
                    self.owner,
                    self.parameters,
                    operand,
                )?;
                subjects[position] = Some((
                    CheckedScalarExpression::IeeeFloatLiteral { value },
                    type_reference,
                ));
                continue;
            }
            let subject = self.subject(operand, depth + 1)?;
            let (primitive_type, atom) = self.primitive(subject.type_reference, depth + 1)??;
            if !ieee_float_atom(atom) {
                return None;
            }
            subjects[position] = Some((
                self.subject_scalar(&subject, primitive_type, depth + 1)?,
                subject.type_reference,
            ));
        }
        super::super::result_contract::lower_float_contract_comparison(
            self.program,
            self.owner,
            expression,
            subjects,
        )
    }

    fn has_float_operand(&mut self, operands: [ExpressionHandle; 2], depth: usize) -> Option<bool> {
        for operand in operands {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(operand) {
                return None;
            }
            if matches!(
                self.program.expression_table.expression(operand),
                ExpressionNode::Float(_)
            ) {
                return Some(true);
            }
            if matches!(
                self.program.expression_table.expression(operand),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) {
                let subject = self.subject(operand, depth + 1)?;
                if self
                    .primitive(subject.type_reference, depth + 1)?
                    .is_some_and(|(_, atom)| ieee_float_atom(atom))
                {
                    return Some(true);
                }
            }
        }
        Some(false)
    }

    // Scalar positions are dense; structural roots retain the authored formal
    // position. Both integer and IEEE leaves use this same entry namespace.
    fn subject_scalar(
        &mut self,
        subject: &FieldPath,
        primitive_type: PrimitiveType,
        depth: usize,
    ) -> Option<CheckedScalarExpression> {
        Some(if subject.path.is_empty() {
            CheckedScalarExpression::Parameter {
                position: self.scalar_position(subject.parameter_position, depth + 1)?,
                primitive_type,
            }
        } else {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: u32::try_from(subject.parameter_position).ok()?,
                path: subject.path.clone(),
                primitive_type,
            }
        })
    }

    fn scalar_position(&mut self, position: usize, depth: usize) -> Option<usize> {
        let mut scalar_position = 0;
        for parameter in &self.parameters[..position] {
            self.primitive(parameter.type_reference, depth + 1)?;
            if super::super::occupies_scalar_position(self.program, parameter) {
                scalar_position += 1;
            }
        }
        Some(scalar_position)
    }

    fn subject(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
        if let Some(resolve) = self.subjects.as_deref_mut() {
            let (position, path) = resolve(expression)?;
            let parameter = self.parameters.get(usize::try_from(position).ok()?)?;
            if parameter.is_const {
                return None;
            }
            let type_reference = super::super::structural_fields::readable_entry_parameter_path(
                self.program,
                self.parameters,
                position,
                &path,
                &mut self.remaining,
            )?;
            return Some(FieldPath {
                parameter_position: usize::try_from(position).ok()?,
                path,
                type_reference,
            });
        }
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            let position = self.parameter(expression, false)?;
            Some(FieldPath {
                parameter_position: position,
                path: Vec::new(),
                type_reference: self.parameters[position].type_reference,
            })
        } else {
            self.field_path(expression, depth)
        }
    }

    fn parameter(&self, expression: ExpressionHandle, _allow_self: bool) -> Option<usize> {
        let name = self.program.expression_table.name_path_node(expression)?;
        let [spelling] = self
            .program
            .expression_table
            .name_path_members(name.members)
        else {
            return None;
        };
        self.parameters.iter().position(|parameter| {
            !parameter.is_const
                && parameter.name.as_str() == spelling.as_str()
                && self.program.symbols.name(parameter.symbol) == parameter.name.as_str()
                && { name.symbol == parameter.symbol && name.head_symbol == parameter.symbol }
        })
    }

    fn field_path(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
        // Bound the authored place before the shared canonical walk. Body
        // observations use the authenticated subject callback above instead,
        // so writes cannot turn current storage into an entry observation.
        let mut current = expression;
        let mut depth = depth;
        loop {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(current) {
                return None;
            }
            match self.program.expression_table.expression(current) {
                ExpressionNode::Member(member) => current = member.receiver,
                ExpressionNode::Indexed(indexed) => current = indexed.collection,
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
            depth += 1;
        }
        let (position, path, selected_type) =
            super::super::structural_fields::structural_parameter_place(
                self.program,
                self.parameters,
                expression,
            )?;
        let type_reference = super::super::structural_fields::readable_entry_parameter_path(
            self.program,
            self.parameters,
            position,
            &path,
            &mut self.remaining,
        )?;
        (type_reference == selected_type).then_some(FieldPath {
            parameter_position: usize::try_from(position).ok()?,
            path,
            type_reference,
        })
    }

    fn primitive(
        &mut self,
        mut type_reference: TypeReferenceHandle,
        mut depth: usize,
    ) -> Option<Option<(PrimitiveType, BuiltinTypeAtom)>> {
        loop {
            self.charge(depth)?;
            if !self
                .program
                .type_reference_table
                .contains_type_reference(type_reference)
            {
                return None;
            }
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, name } => {
                    let Some(primitive) = PrimitiveType::from_name(name) else {
                        return Some(None);
                    };
                    let atom = self.program.symbols.builtin_type_atom(*symbol)?;
                    if self.program.symbols.name(*symbol) != name.as_str()
                        || PrimitiveType::from_name(self.program.symbols.name(*symbol))
                            != Some(primitive)
                    {
                        return None;
                    }
                    let expected = match atom {
                        BuiltinTypeAtom::Bool | BuiltinTypeAtom::AtomicBool => PrimitiveType::Bool,
                        BuiltinTypeAtom::I8 => PrimitiveType::I8,
                        BuiltinTypeAtom::I16 => PrimitiveType::I16,
                        BuiltinTypeAtom::I32 => PrimitiveType::I32,
                        BuiltinTypeAtom::I64 => PrimitiveType::I64,
                        BuiltinTypeAtom::U8 => PrimitiveType::U8,
                        BuiltinTypeAtom::U16 => PrimitiveType::U16,
                        BuiltinTypeAtom::U32 | BuiltinTypeAtom::AtomicU32 => PrimitiveType::U32,
                        BuiltinTypeAtom::U64 | BuiltinTypeAtom::AtomicU64 => PrimitiveType::U64,
                        BuiltinTypeAtom::F32 => PrimitiveType::F32,
                        BuiltinTypeAtom::F64 => PrimitiveType::F64,
                        BuiltinTypeAtom::Address => PrimitiveType::Addr,
                        _ => return None,
                    };
                    return (primitive == expected).then_some(Some((primitive, atom)));
                }
                _ => return Some(None),
            }
            depth += 1;
        }
    }
}

fn fixed_integer_atom(atom: BuiltinTypeAtom) -> bool {
    matches!(
        atom,
        BuiltinTypeAtom::I8
            | BuiltinTypeAtom::I16
            | BuiltinTypeAtom::I32
            | BuiltinTypeAtom::I64
            | BuiltinTypeAtom::U8
            | BuiltinTypeAtom::U16
            | BuiltinTypeAtom::U32
            | BuiltinTypeAtom::U64
    )
}

fn ieee_float_atom(atom: BuiltinTypeAtom) -> bool {
    matches!(atom, BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64)
}
