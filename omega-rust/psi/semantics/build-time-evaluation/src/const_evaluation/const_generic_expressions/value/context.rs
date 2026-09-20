//! Selected operator meaning belongs to an activation or a closed position.

use super::{Machine, Shape, State, primitive};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::{PrimitiveType, TypeReferenceHandle},
};

#[derive(Clone, Copy)]
pub(super) enum EvaluationContext<'program> {
    Machine(&'program Machine, &'program State),
    Closed,
}

impl EvaluationContext<'_> {
    pub(super) fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    pub(super) fn arithmetic_result(
        self,
        program: &TypedTrees,
        shape: Shape,
    ) -> Result<Shape, String> {
        if !self.is_closed() {
            return Ok(shape);
        }
        let Shape::Integer(carrier, _) = shape else {
            return Ok(shape);
        };
        let reference = reference(program, shape)?
            .and_then(|reference| validation::arithmetic_result_type_reference(program, reference))
            .ok_or("closed arithmetic result has unresolved type qualifications")?;
        Ok(Shape::Integer(carrier, reference))
    }

    pub(super) fn joined_result(
        self,
        program: &TypedTrees,
        shapes: &[Shape],
        peer: Shape,
    ) -> Result<Shape, String> {
        if !self.is_closed() {
            return Ok(peer);
        }
        let Shape::Integer(carrier, _) = peer else {
            return Ok(peer);
        };
        let references = shapes
            .iter()
            .map(|shape| reference(program, *shape))
            .collect::<Result<Vec<_>, _>>()?;
        let anonymous = shapes
            .iter()
            .map(|shape| matches!(shape, Shape::Anonymous(_)))
            .collect::<Vec<_>>();
        let reference = validation::join_result_type_references(program, &references, &anonymous)
            .ok_or("closed Match result has incompatible type qualifications")?;
        Ok(Shape::Integer(carrier, reference))
    }

    pub(super) fn has_builtin(self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        match self {
            Self::Machine(machine, state) => validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ),
            // Closed positions decide every operator after reconstructing its
            // operand shapes, before any selective execution or rational landing.
            Self::Closed => true,
        }
    }

    pub(super) fn require_operator(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        left: Shape,
        right: Shape,
    ) -> Result<(), String> {
        if !self.is_closed() {
            return Ok(());
        }
        use language_core::OperatorSpelling as Spelling;
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return Err("constant operator lost its binary node".into());
        };
        let spelling = match binary.operator {
            BinaryOperator::Add => Spelling::Add,
            BinaryOperator::Subtract => Spelling::Subtract,
            BinaryOperator::Multiply => Spelling::Multiply,
            BinaryOperator::Divide => Spelling::Divide,
            BinaryOperator::Modulo => Spelling::Modulo,
            BinaryOperator::Equal => Spelling::Equal,
            BinaryOperator::NotEqual => Spelling::NotEqual,
            BinaryOperator::Less => Spelling::Less,
            BinaryOperator::LessOrEqual => Spelling::LessEqual,
            BinaryOperator::Greater => Spelling::Greater,
            BinaryOperator::GreaterOrEqual => Spelling::GreaterEqual,
            BinaryOperator::CaseMembership => {
                return Err("constant case membership needs its selected owner".into());
            }
            // These operations have no overloadable spelling.
            BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight => return Ok(()),
        };
        let references = [reference(program, left)?, reference(program, right)?];
        let anonymous = matches!((left, right), (Shape::Anonymous(_), Shape::Anonymous(_)));
        let builtin = if anonymous {
            typed_trees::closed_numeric::has_builtin_anonymous_operands(
                program, expression, spelling,
            )
        } else {
            typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                symbols::SymbolHandle::invalid(),
                expression,
                spelling,
                &references,
            )
        };
        let selected_elsewhere = program
            .machine_specializations
            .iter()
            .any(|specialization| {
                !typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    specialization.instance,
                    spelling,
                    &references,
                )
                .is_empty()
            });
        if builtin && !selected_elsewhere {
            Ok(())
        } else {
            Err(
                "closed constant operator requires exact context-independent builtin meaning"
                    .into(),
            )
        }
    }
}

fn reference(program: &TypedTrees, shape: Shape) -> Result<Option<TypeReferenceHandle>, String> {
    let primitive = match shape {
        Shape::Anonymous(_) => return Ok(None),
        Shape::Boolean => PrimitiveType::Bool,
        Shape::Float(format) => super::float_primitive(format),
        Shape::Integer(_, reference) if reference.is_valid() => {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return Err("constant call result type belongs to another program".into());
            }
            return Ok(Some(reference));
        }
        Shape::Integer(carrier, _) => primitive(carrier)?,
    };
    use symbols::BuiltinTypeAtom as Atom;
    let atom = match primitive {
        PrimitiveType::Bool => Atom::Bool,
        PrimitiveType::F32 => Atom::F32,
        PrimitiveType::F64 => Atom::F64,
        PrimitiveType::I8 => Atom::I8,
        PrimitiveType::I16 => Atom::I16,
        PrimitiveType::I32 => Atom::I32,
        PrimitiveType::I64 => Atom::I64,
        PrimitiveType::U8 => Atom::U8,
        PrimitiveType::U16 => Atom::U16,
        PrimitiveType::U32 => Atom::U32,
        PrimitiveType::U64 => Atom::U64,
        _ => return Err("constant operand has no exact scalar carrier".into()),
    };
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())
        .and_then(|mut children| {
            children.find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))
        })
        .ok_or("constant operand lost its builtin type")?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
        .map(Some)
        .ok_or_else(|| "constant operand lost its type reference".into())
}
