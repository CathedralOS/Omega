use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, EvalResult, EvaluatedArgument, Evaluator, ExpressionHandle,
    ExpressionNode, FixedArrayLength, Frame, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
    Value, integer_primitive_byte_width, primitive_is_unsigned64, trap,
};
impl<'program> Evaluator<'program> {
    /// Evaluate and capture one operand at its declared parameter type before
    /// the next operand executes. Ordinary calls and selected boundary adapters
    /// share this operation. Owned aggregates keep their captured contents;
    /// references keep their backing, including sealed mutable recast metadata
    /// under the target parameter's name.
    pub(in crate::interpreter::evaluator) fn eval_state_argument(
        &mut self,
        argument: ExpressionHandle,
        destination: TypeReferenceHandle,
        frame: &mut Frame,
    ) -> EvalResult<EvaluatedArgument> {
        let cell = if self.declared_type_is_reference(destination) {
            let value = self.eval_expression_at_type(argument, destination, frame)?;
            self.allocate_cell(value)?
        } else if let Some(value) = self.anonymous_integer_landing_value(
            argument,
            self.program.primitive_type_reference(destination),
        )? {
            self.allocate_cell(value)?
        } else if matches!(
            self.program.expression_table.expression(argument),
            ExpressionNode::ArrayLiteral(_)
        ) {
            let value = self.eval_expression_at_type(argument, destination, frame)?;
            self.allocate_cell(value)?
        } else {
            self.eval_argument(argument, frame)?
        };
        // Every call route evaluates one operand at its declared destination.
        // Capture owned aggregate contents here, before the next operand can
        // mutate their source. Keeping this in the ordinary-call loop left
        // boundary adapters with aliases to arrays passed by value. Borrowed
        // destinations retain their referents and sealed recast metadata.
        let value = cell.borrow().clone();
        let value = self.capture_value_operand(value, destination)?;
        *cell.borrow_mut() = value;
        Ok(EvaluatedArgument::plain(cell))
    }

    pub(in crate::interpreter::evaluator) fn declared_type_is_reference(
        &self,
        mut destination: TypeReferenceHandle,
    ) -> bool {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(destination)
            {
                TypeReferenceNode::Constrained { base_type, .. } => destination = *base_type,
                TypeReferenceNode::Reference { .. } => return true,
                _ => return false,
            }
        }
    }

    pub(in crate::interpreter::evaluator) fn expression_type_reference(
        &self,
        expression: ExpressionHandle,
        frame: &Frame,
    ) -> Option<TypeReferenceHandle> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                let subject = self.program.saved_value_source(saved)?;
                let machine = self.program.machine_by_symbol(frame.machine_symbol)?;
                let state = self.program.state_in_machine(machine, frame.state_symbol)?;
                validation::expression_result_type_reference(self.program, machine, state, subject)
            }
            ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                self.program.machines().iter().find_map(|machine| {
                    self.program
                        .state_in_machine(machine, call.target_symbol)
                        .map(|state| state.return_type)
                })
            }
            ExpressionNode::ArrayLiteral(_) => {
                validation::declared_constant_array_type(self.program, expression)
            }
            ExpressionNode::Borrow(inner) => self.expression_type_reference(inner.target, frame),
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.expression_type_reference(indexed.collection, frame)?;
                self.collection_element_type(collection)
            }
            ExpressionNode::Member(member) => {
                let receiver = self.expression_type_reference(member.receiver, frame)?;
                self.projected_member_type(receiver, member.member.as_str())
            }
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let (_, rest) = members.split_first()?;
                let mut current = frame.local_type(path.head_symbol)?;
                for member in rest {
                    current = self.projected_member_type(current, member.as_str())?;
                }
                Some(current)
            }
            _ => None,
        }
    }

    fn projected_member_type(
        &self,
        mut type_reference: TypeReferenceHandle,
        member_name: &str,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    return self.field_type_reference(*symbol, member_name);
                }
                _ => return None,
            }
        }
    }

    pub(in crate::interpreter::evaluator) fn collection_element_type(
        &self,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                _ => return None,
            }
        }
    }

    pub(in crate::interpreter::evaluator) fn eval_recast_to_type(
        &self,
        value: Value,
        target_type: TypeReferenceHandle,
    ) -> EvalResult<Value> {
        let target = self
            .program
            .type_reference_table
            .type_reference(target_type)
            .clone();
        match target {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => self.eval_recast_to_type(value, referee),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let value = match value {
                    Value::Ref(reference) => self.read_reference(&reference)?,
                    other => other,
                };
                let Value::Array(elements) = value else {
                    return trap("fixed-array recast source is not an array");
                };
                if elements.len() != length {
                    return trap(format!(
                        "fixed-array recast expected {length} elements, found {}",
                        elements.len()
                    ));
                }
                let mut recast = Vec::with_capacity(elements.len());
                for element in elements {
                    let value = element.borrow().clone();
                    let value = self.eval_recast_to_type(value, element_type)?;
                    recast.push(self.allocate_cell(value)?);
                }
                Ok(Value::Array(recast))
            }
            TypeReferenceNode::Slice { element_type } => {
                let value = match value {
                    Value::Ref(reference) => self.read_reference(&reference)?,
                    other => other,
                };
                let Value::Array(elements) = value else {
                    return trap("slice recast source is not an array or slice");
                };
                let mut recast = Vec::with_capacity(elements.len());
                for element in elements {
                    let value = element.borrow().clone();
                    let value = self.eval_recast_to_type(value, element_type)?;
                    recast.push(self.allocate_cell(value)?);
                }
                Ok(Value::Array(recast))
            }
            _ => self.eval_recast(value, self.cast_target_primitive(target_type)),
        }
    }

    /// Best-effort STATIC witness that an expression is u64-classed
    /// (`u64`/`usize`/`addr`), used to give width-8 comparisons UNSIGNED
    /// semantics (matching the native signedness-adjusted compares). FALSE on
    /// any doubt: a false negative keeps the signed compare (today's
    /// behavior); only DECLARED types answer true, so signed compares can
    /// never be corrupted.
    pub(in crate::interpreter::evaluator) fn expression_is_unsigned64(
        &self,
        expression: checked_trees::expression::ExpressionHandle,
        frame: &Frame,
    ) -> bool {
        primitive_is_unsigned64(
            self.expression_scalar_type(expression, frame)
                .map(|(primitive, _)| primitive),
        )
    }

    /// Best-effort STATIC (primitive, arithmetic-domain) of an expression,
    /// resolved from DECLARED types only (decision 17): a NAME reads the
    /// local/param type recorded at binding, `self.field` reads the attached
    /// data field, a CAST is its target width (`as T` carries no domain
    /// clause, and the absence of one means Exact), a BINARY/UNARY node is
    /// typed by one operand witness (mixed classes are checker-rejected).
    /// `None` on any doubt -- literals are adaptive. This is what lets
    /// `acc + 50` SATURATE at the operation node when `acc` is declared
    /// `i8 in Saturating` regardless of where the result lands; the
    /// landing-seam coercions alone cannot represent an expression whose own
    /// domain differs from its landing slot's (native emits the saturating
    /// ADD itself).
    pub(in crate::interpreter::evaluator) fn expression_scalar_type(
        &self,
        expression: checked_trees::expression::ExpressionHandle,
        frame: &Frame,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                let mut result = None;
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    if let Some(arm_type) = self.expression_scalar_type(arm.value, frame) {
                        if result.is_some_and(|result| result != arm_type) {
                            return None;
                        }
                        result = Some(arm_type);
                    }
                }
                result
            }
            // A cast witnesses its target width AND its decision-17 S2
            // domain retag (`x as u8 in Saturating` -- the retag is what lets
            // the value join saturating arithmetic; without a written domain
            // the node carries Exact). The retag must reach fused arithmetic
            // (`(a as u8 in Saturating) + b` in a GUARD has no landing seam);
            // hardcoding Exact here let the wide 300 through while native's
            // witness read the retag and clamped.
            ExpressionNode::Cast(cast) => {
                Some((self.cast_target_primitive(cast.target_type)?, cast.domain))
            }
            ExpressionNode::Borrow(inner) => self.expression_scalar_type(inner.target, frame),
            ExpressionNode::Unary(unary) => self.expression_scalar_type(unary.operand, frame),
            ExpressionNode::Integer(literal) => literal.landing().map(|landing| {
                let primitive = PrimitiveType::from_landed_integer(landing.landed_type);
                (primitive, landing.domain)
            }),
            ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
                let reference = self.expression_type_reference(expression, frame)?;
                Some((
                    self.program.primitive_type_reference(reference)?,
                    self.program.arithmetic_domain_for_type_reference(reference),
                ))
            }
            // A LANDED float literal witnesses its format (the F2a suffix /
            // F2b destination / F2c comparison stamps): an anonymous constant
            // guard tree (`16777216.0 + 1.0` against an f32 place) has no
            // declared destination, so the stamped literal is the node-width
            // witness that drives per-op f32 rounding in eval_float_binary.
            ExpressionNode::UndeterminedNaN(format) => Some((
                match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                },
                ArithmeticDomain::Exact,
            )),
            ExpressionNode::Float(literal) => literal.landing().map(|format| {
                (
                    match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    },
                    ArithmeticDomain::Exact,
                )
            }),
            // Counts neither promote the shifted value's carrier nor retag
            // its arithmetic policy; only the left operand owns that type.
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) =>
            {
                self.expression_scalar_type(binary.left, frame)
            }
            // Other binary nodes compute in the PROMOTED type: mixed widths
            // auto-promote to the wider operand (u8 + i32 runs at i32 --
            // wrapping 200+100 at the node must yield 300, not the u8 44), so
            // the WIDER witness types the node. Equal widths keep the left
            // witness: add/sub/mul bits agree across signedness at one width,
            // and mixed DOMAIN classes are checker-rejected
            // (fail/expressions/arithmetic_domain_mixed).
            ExpressionNode::Binary(binary) => {
                let left = self.expression_scalar_type(binary.left, frame);
                let right = self.expression_scalar_type(binary.right, frame);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        let left_width = integer_primitive_byte_width(left.0).unwrap_or(8);
                        let right_width = integer_primitive_byte_width(right.0).unwrap_or(8);
                        Some(if right_width > left_width {
                            right
                        } else if left_width > right_width || left.1 != ArithmeticDomain::Exact {
                            left
                        } else {
                            // Equal widths, left Exact: prefer the side that
                            // carries a domain (an S2 retag on the right).
                            right
                        })
                    }
                    (left, right) => left.or(right),
                }
            }
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                if members.len() == 1 {
                    return frame.local_scalar(path.head_symbol);
                }
                let reference = self.expression_type_reference(expression, frame)?;
                Some((
                    self.program.primitive_type_reference(reference)?,
                    self.program.arithmetic_domain_for_type_reference(reference),
                ))
            }
            // A resolved value call carries the exact entry-state symbol.
            // Preserve its declared primitive/domain as the witness for an
            // enclosing operation. This matters for u64-returning helpers:
            // their high-bit results must compare and shift unsigned even
            // though the interpreter stores the raw pattern in an i64 cell.
            ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                let state = self
                    .program
                    .machines()
                    .iter()
                    .flat_map(|machine| self.program.machine_states(machine))
                    .find(|state| state.symbol == call.target_symbol)?;
                let primitive = self.program.primitive_type_reference(state.return_type)?;
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(state.return_type);
                Some((primitive, domain))
            }
            _ => None,
        }
    }
}
