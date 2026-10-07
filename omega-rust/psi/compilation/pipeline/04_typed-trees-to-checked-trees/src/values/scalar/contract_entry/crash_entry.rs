//! Exact invocation predicates over Boolean fields, total integer
//! comparisons, and scalar IEEE comparisons with exact literal endpoints.
use crate::values::operator_is_builtin;

use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use symbols::{BuiltinTypeAtom, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn lower_machine_entry_crash_contract_expression(
    program: &TypedTrees,
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

type EntrySubjectResolver<'program> = dyn Fn(ExpressionHandle) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>
    + 'program;

/// Read the same predicate algebra while the caller supplies authenticated
/// invocation-entry subjects for observations made in a body state.
pub(crate) fn lower_entry_predicate_with_subjects(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    subjects: &EntrySubjectResolver<'_>,
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
pub(crate) fn lower_entry_scalar_with_subjects(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    primitive_type: PrimitiveType,
    subjects: &impl Fn(
        &crate::flow::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>,
) -> Option<CheckedScalarExpression> {
    if primitive_type == PrimitiveType::Bool {
        return lower_entry_predicate_with_subjects(
            program,
            operators,
            machine,
            state,
            expression,
            &|leaf| {
                let place = crate::flow::canonical_place_from_expression_in_state(
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
    crate::values::lower_unit_scalar_argument_with_places(
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
            let (_, _, type_reference, _) = crate::values::resolve_structural_parameter_path(
                program,
                reader.parameters,
                position,
                &path,
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
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    machine: Option<&'program Machine>,
    owner: symbols::SymbolHandle,
    parameters: &'program [StateParameter],
    remaining: usize,
    subjects: Option<&'program EntrySubjectResolver<'program>>,
}

/// Bodyless requirements have an exact signature namespace, not a synthetic
/// machine body. Structural guards remain outside this scalar contract slice.
pub(crate) fn lower_signature_crash_contract_expression(
    program: &TypedTrees,
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
/// operation-contract telescope reads as formal `k + 1`. A generic operator
/// has no closed scalar telescope yet, and a structural formal keeps its
/// route identity-only (the machine-rooted field reader does not apply), so
/// both leave the route without a form rather than guess one.
pub(crate) fn lower_operator_crash_contract_expression(
    program: &TypedTrees,
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
                    let value = if subject.path.is_empty() {
                        CheckedScalarExpression::Parameter {
                            position: self
                                .scalar_position(subject.parameter_position, depth + 1)?,
                            primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::StructuralParameterField {
                            parameter_position: u32::try_from(subject.parameter_position).ok()?,
                            path: subject.path,
                            primitive_type,
                        }
                    };
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
        operator: BinaryOperator,
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
                let value = match literal.landing()? {
                    numerics::literals::FloatFormat::F32 => {
                        semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
                    }
                    numerics::literals::FloatFormat::F64 => {
                        semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
                    }
                };
                let type_reference = validation::parameter_expression_result_type_reference(
                    self.program,
                    self.owner,
                    self.parameters,
                    operand,
                )?;
                subjects[position] = Some((
                    CheckedScalarExpression::IeeeFloatLiteral { value },
                    Some(type_reference),
                ));
                continue;
            }
            let ExpressionNode::Name(_) = self.program.expression_table.expression(operand) else {
                return None;
            };
            let subject = self.subject(operand, depth + 1)?;
            if !subject.path.is_empty() {
                return None;
            }
            let parameter_position = subject.parameter_position;
            let type_reference = subject.type_reference;
            let (primitive_type, atom) = self.primitive(type_reference, depth + 1)??;
            if !ieee_float_atom(atom) {
                return None;
            }
            let mut scalar_position = 0;
            for parameter in &self.parameters[..parameter_position] {
                if self
                    .primitive(parameter.type_reference, depth + 1)?
                    .is_some()
                {
                    scalar_position += 1;
                }
            }
            subjects[position] = Some((
                CheckedScalarExpression::Parameter {
                    position: scalar_position,
                    primitive_type,
                },
                Some(type_reference),
            ));
        }
        let operand_types = subjects.each_ref().map(|subject| {
            subject
                .as_ref()
                .and_then(|(_, type_reference)| *type_reference)
        });
        let [Some((mut left, _)), Some((mut right, _))] = subjects else {
            return None;
        };
        if left.primitive_type() != right.primitive_type() {
            return None;
        }
        if self.operators.uses.iter().any(|(_, operator)| {
            operator.expression == expression
                && operator.status
                    != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        }) {
            return None;
        }
        // A machine-less reader still requires the authored relation to carry
        // builtin IEEE meaning over these operands before it becomes logic.
        // A declaration being bodyless does not grant IEEE meaning: a declared
        // relation must rejoin the exact sealed Float contract, not a lookalike.
        if self.machine.is_none()
            && !validation::has_builtin_or_sealed_float_comparison_meaning(
                self.program,
                self.owner,
                expression,
                operator,
                &operand_types,
            )
        {
            return None;
        }
        let kind = match operator {
            BinaryOperator::Equal => CheckedIeeeFloatComparisonKind::Equal,
            BinaryOperator::NotEqual => CheckedIeeeFloatComparisonKind::NotEqual,
            BinaryOperator::Less => CheckedIeeeFloatComparisonKind::LessThan,
            BinaryOperator::LessOrEqual => CheckedIeeeFloatComparisonKind::LessOrEqual,
            BinaryOperator::Greater | BinaryOperator::GreaterOrEqual => {
                std::mem::swap(&mut left, &mut right);
                if operator == BinaryOperator::Greater {
                    CheckedIeeeFloatComparisonKind::LessThan
                } else {
                    CheckedIeeeFloatComparisonKind::LessOrEqual
                }
            }
            _ => return None,
        };
        Some(CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(left),
            right: Box::new(right),
        })
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
            if let ExpressionNode::Name(_) = self.program.expression_table.expression(operand) {
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

    fn scalar_position(&mut self, position: usize, depth: usize) -> Option<usize> {
        let mut scalar_position = 0;
        for parameter in &self.parameters[..position] {
            if self
                .primitive(parameter.type_reference, depth + 1)?
                .is_some()
            {
                scalar_position += 1;
            }
        }
        Some(scalar_position)
    }

    fn subject(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
        if let Some(resolve) = self.subjects {
            let (position, path) = resolve(expression)?;
            let parameter = self.parameters.get(usize::try_from(position).ok()?)?;
            if parameter.is_const {
                return None;
            }
            let (_, _, type_reference, _) = crate::values::resolve_structural_parameter_path(
                self.program,
                self.parameters,
                position,
                &path,
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
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
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
        self.machine?;
        self.charge(depth)?;
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        let ExpressionNode::Member(member) = self.program.expression_table.expression(expression)
        else {
            let position = self.parameter(expression, true)?;
            return Some(FieldPath {
                parameter_position: position,
                path: Vec::new(),
                type_reference: self.parameters[position].type_reference,
            });
        };
        if member.case_variant.is_some() || !member.member_symbol.is_valid() {
            return None;
        }
        let mut receiver = self.field_path(member.receiver, depth + 1)?;
        let owner = self.nominal(receiver.type_reference, receiver.path.is_empty(), depth + 1)?;
        let field = {
            let mut fields = self
                .program
                .data_members(owner)
                .iter()
                .filter_map(|member_kind| {
                    let DataMember::Field(field) = member_kind else {
                        return None;
                    };
                    (field.symbol == member.member_symbol).then_some(field)
                });
            let field = fields.next()?;
            if fields.next().is_some() {
                return None;
            }
            field
        };
        if field.name.as_str() != member.member.as_str()
            || field.relevance.is_erased()
            || self.program.symbols.get(field.symbol).kind != SymbolKind::Field
            || self.program.symbols.get(field.symbol).parent != owner.symbol
            || self.program.symbols.name(field.symbol) != field.name.as_str()
        {
            return None;
        }
        // Canonical paths carry field identity (or its declaration-local name),
        // so a malformed duplicate cannot collapse two different selections.
        if self
            .program
            .data_members(owner)
            .iter()
            .filter(|candidate| {
                matches!(candidate, DataMember::Field(candidate)
                if candidate.name == field.name
                    || field.identity.is_some() && candidate.identity == field.identity)
            })
            .count()
            != 1
        {
            return None;
        }
        receiver
            .path
            .push(CheckedStructuralPredicatePathSegment::Field(
                field.path_identity(),
            ));
        receiver.type_reference = field.type_reference;
        Some(receiver)
    }

    fn nominal(
        &mut self,
        mut type_reference: TypeReferenceHandle,
        root: bool,
        mut depth: usize,
    ) -> Option<&'program DataDefinition> {
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
                // Only the root signature carries borrow/access custody. A
                // reference-valued intermediate field requires a dereference
                // path that this plain-field representation does not retain.
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if root && *access != language_core::ReferenceAccess::WriteOnly => {
                    type_reference = *referee;
                }
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, name } => {
                    let data_symbol = *symbol;
                    let mut owners = self
                        .program
                        .data_definitions()
                        .iter()
                        .filter(|owner| owner.symbol == data_symbol);
                    let owner = owners.next()?;
                    return (owners.next().is_none()
                        && self.program.symbols.get(data_symbol).kind == SymbolKind::Data
                        && owner.name.as_str() == name.as_str()
                        && self.program.symbols.name(data_symbol) == owner.name.as_str()
                        && self.program.data_type_parameters(owner).is_empty()
                        && owner.lifetime_parameters.is_empty())
                    .then_some(owner);
                }
                _ => return None,
            }
            depth += 1;
        }
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
