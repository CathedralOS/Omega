//! Exact invocation predicates over Boolean fields and total integer comparisons.

use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use symbols::{BuiltinTypeAtom, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

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
        machine,
        parameters,
        remaining: 4096,
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

struct Reader<'program> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    machine: &'program Machine,
    parameters: &'program [StateParameter],
    remaining: usize,
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
            ExpressionNode::Name(_) => {
                let position = self.parameter(expression, false)?;
                if self.primitive(self.parameters[position].type_reference, depth + 1)?
                    != Some((PrimitiveType::Bool, BuiltinTypeAtom::Bool))
                {
                    return None;
                }
                let mut scalar_position = 0;
                for parameter in &self.parameters[..position] {
                    // This is the established dense scalar namespace; structural
                    // field roots below retain their full authored ordinal.
                    if self
                        .primitive(parameter.type_reference, depth + 1)?
                        .is_some()
                    {
                        scalar_position += 1;
                    }
                }
                Some(CheckedBooleanExpression::Parameter {
                    position: scalar_position,
                })
            }
            ExpressionNode::Member(_) => {
                let field = self.field_path(expression, depth + 1)?;
                if self.primitive(field.type_reference, depth + 1)?
                    != Some((PrimitiveType::Bool, BuiltinTypeAtom::Bool))
                {
                    return None;
                }
                Some(CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: u32::try_from(field.parameter_position).ok()?,
                    path: field.path,
                })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && super::operator_is_builtin(self.operators, expression) =>
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
                ) && super::operator_is_builtin(self.operators, expression) =>
            {
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
                ExpressionNode::Name(_) => {
                    let position = self.parameter(operand, false)?;
                    if self
                        .primitive(self.parameters[position].type_reference, depth + 1)?
                        .is_some_and(|(_, atom)| fixed_integer_atom(atom))
                    {
                        return Some(true);
                    }
                }
                ExpressionNode::Member(_) => {
                    let field = self.field_path(operand, depth + 1)?;
                    if self
                        .primitive(field.type_reference, depth + 1)?
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
                ExpressionNode::Name(_) => {
                    let parameter_position = self.parameter(operand, false)?;
                    let type_reference = self.parameters[parameter_position].type_reference;
                    let (primitive_type, atom) = self.primitive(type_reference, depth + 1)??;
                    if !fixed_integer_atom(atom) {
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
                        type_reference,
                    ));
                }
                ExpressionNode::Member(_) => {
                    let field = self.field_path(operand, depth + 1)?;
                    let (primitive_type, atom) =
                        self.primitive(field.type_reference, depth + 1)??;
                    if !fixed_integer_atom(atom) {
                        return None;
                    }
                    subjects[position] = Some((
                        CheckedScalarExpression::StructuralParameterField {
                            parameter_position: u32::try_from(field.parameter_position).ok()?,
                            path: field.path,
                            primitive_type,
                        },
                        field.type_reference,
                    ));
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
            self.machine,
            expression,
            subjects,
        )
    }

    fn parameter(&self, expression: ExpressionHandle, allow_self: bool) -> Option<usize> {
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
                && if parameter.is_self {
                    allow_self
                        && parameter.name.as_str() == "self"
                        && name.symbol == self.machine.symbol
                        && name.head_symbol == self.machine.symbol
                } else {
                    name.symbol == parameter.symbol && name.head_symbol == parameter.symbol
                }
        })
    }

    fn field_path(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
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
        let self_root =
            receiver.path.is_empty() && self.parameters[receiver.parameter_position].is_self;
        let owner = self.nominal(
            receiver.type_reference,
            receiver.path.is_empty(),
            self_root,
            depth + 1,
        )?;
        let field =
            if receiver.path.is_empty() && self.parameters[receiver.parameter_position].is_self {
                if owner.symbol != self.machine.attached_data_symbol {
                    return None;
                }
                validation::exact_self_field(self.program, self.machine, expression)?
            } else {
                let mut fields =
                    self.program
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
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned()),
            ));
        receiver.type_reference = field.type_reference;
        Some(receiver)
    }

    fn nominal(
        &mut self,
        mut type_reference: TypeReferenceHandle,
        root: bool,
        self_root: bool,
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
                    // Resolution assigns SelfType the attached machine's
                    // symbol; typing retains that exact symbol as Named Self.
                    // Only the validated self formal may use this alias.
                    let self_alias =
                        self_root && *symbol == self.machine.symbol && name.as_str() == "Self";
                    let data_symbol = if self_alias {
                        self.machine.attached_data_symbol
                    } else {
                        *symbol
                    };
                    let mut owners = self
                        .program
                        .data_definitions()
                        .iter()
                        .filter(|owner| owner.symbol == data_symbol);
                    let owner = owners.next()?;
                    return (owners.next().is_none()
                        && self.program.symbols.get(data_symbol).kind == SymbolKind::Data
                        && (self_alias || owner.name.as_str() == name.as_str())
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
