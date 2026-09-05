use super::*;

impl<'program> Evaluator<'program> {
    /// Evaluate an argument entering an Omega state parameter while preserving
    /// any sealed mutable recast view. The target parameter receives the same
    /// view metadata under its own name instead of degrading to the source
    /// byte-array reference at the state edge.
    pub(super) fn eval_state_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<EvaluatedArgument> {
        let initializer = self.mutable_scalar_recast_initializer(argument, frame)?;
        if let Some((source, recast)) = initializer {
            return Ok(EvaluatedArgument {
                cell: self.allocate_cell(Value::Ref(source))?,
                mutable_recast: Some(recast),
            });
        }
        let mutable_recast = self
            .mutable_recast_path(argument, frame)?
            .map(|(recast, _)| recast);
        Ok(EvaluatedArgument {
            cell: self.eval_argument(argument, frame)?,
            mutable_recast,
        })
    }

    pub(super) fn expression_type_reference(
        &self,
        expression: ExpressionHandle,
        frame: &Frame,
    ) -> Option<TypeReferenceHandle> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => self.expression_type_reference(inner.target, frame),
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.expression_type_reference(indexed.collection, frame)?;
                self.collection_element_type(collection)
            }
            ExpressionNode::Member(member) => {
                if let ExpressionNode::Name(path) =
                    self.program.expression_table.expression(member.receiver)
                {
                    let members = self
                        .program
                        .expression_table
                        .name_path_members(path.members);
                    if matches!(members, [head] if head.as_str() == "self") {
                        return self.machine_attached_field_type(
                            frame.machine_symbol,
                            member.member.as_str(),
                        );
                    }
                }
                let receiver = self.expression_type_reference(member.receiver, frame)?;
                self.projected_member_type(receiver, member.member.as_str())
            }
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let (mut current, rest) = match members {
                    [] => return None,
                    [head] if head.as_str() == "self" => return None,
                    [head, field, rest @ ..] if head.as_str() == "self" => (
                        self.machine_attached_field_type(frame.machine_symbol, field.as_str())?,
                        rest,
                    ),
                    [head, rest @ ..] => {
                        match frame.type_locals.borrow().get(head.as_str()).copied() {
                            Some(local) => (local, rest),
                            None => (
                                self.machine_attached_field_type(
                                    frame.machine_symbol,
                                    head.as_str(),
                                )?,
                                rest,
                            ),
                        }
                    }
                };
                for member in rest {
                    current = self.projected_member_type(current, member.as_str())?;
                }
                Some(current)
            }
            _ => None,
        }
    }

    fn machine_attached_field_type(
        &self,
        machine_symbol: SymbolHandle,
        field_name: &str,
    ) -> Option<TypeReferenceHandle> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)?;
        let name = machine.attached_data.as_ref()?;
        let data = self.find_data_by_name(name.as_str())?;
        self.program.data_members(data).iter().find_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == field_name).then_some(field.type_reference)
        })
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
                TypeReferenceNode::Named { name, .. } => {
                    let data = self.find_data_by_name(name.as_str())?;
                    return self.program.data_members(data).iter().find_map(|member| {
                        let DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.name.as_str() == member_name).then_some(field.type_reference)
                    });
                }
                _ => return None,
            }
        }
    }

    pub(super) fn collection_element_type(
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

    pub(super) fn eval_recast_to_type(
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
                    Value::Ref(cell) => cell.borrow().clone(),
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
                    Value::Ref(cell) => cell.borrow().clone(),
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
    pub(super) fn expression_is_unsigned64(
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
    pub(super) fn expression_scalar_type(
        &self,
        expression: checked_trees::expression::ExpressionHandle,
        frame: &Frame,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        match self.program.expression_table.expression(expression) {
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
            // A LANDED float literal witnesses its format (the F2a suffix /
            // F2b destination / F2c comparison stamps): an anonymous constant
            // guard tree (`16777216.0 + 1.0` against an f32 place) has no
            // declared destination, so the stamped literal is the node-width
            // witness that drives per-op f32 rounding in eval_float_binary.
            ExpressionNode::Float(literal) => literal.landing().map(|format| {
                (
                    match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    },
                    ArithmeticDomain::Exact,
                )
            }),
            // A binary node computes in the PROMOTED type: mixed widths
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
                // `self.field` spelled as a two-member path. NOTE: this route
                // also witnesses INDEXED element reads -- trace-verified
                // 2026-07-10n: a RUNTIME-indexed `self.sarr[i]` reaches this
                // witness as a Name([self, sarr]) whose field type peels to
                // the ELEMENT primitive + the ARRAY's domain via
                // primitive_type_reference/arithmetic_domain (a CONST-indexed
                // `self.sarr[1]` arrives as a true Indexed node and returns
                // None from the fallthrough -- its sibling operand's witness
                // covers the pair via `.or()`). Pinned by
                // pass/slices/runtime_saturating_array_element_guard_exit.
                if members.len() == 2 && members[0].as_str() == "self" {
                    return self.attached_field_scalar_type(frame, members[1].as_str());
                }
                if members.len() == 1 && members[0].as_str() != "self" {
                    return frame
                        .scalar_locals
                        .borrow()
                        .get(members[0].as_str())
                        .copied();
                }
                None
            }
            // `self.field` spelled as a Member node.
            ExpressionNode::Member(member) => {
                let receiver_is_self =
                    match self.program.expression_table.expression(member.receiver) {
                        ExpressionNode::Name(path) => {
                            let members = self
                                .program
                                .expression_table
                                .name_path_members(path.members);
                            members.len() == 1 && members[0].as_str() == "self"
                        }
                        _ => false,
                    };
                if !receiver_is_self {
                    return None;
                }
                self.attached_field_scalar_type(frame, member.member.as_str())
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

    /// The executing machine's attached-data field's declared scalar
    /// (primitive, arithmetic-domain); `None` for a non-scalar field.
    fn attached_field_scalar_type(
        &self,
        frame: &Frame,
        field_name: &str,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == frame.machine_symbol)?;
        let data_name = machine.attached_data.as_ref()?;
        let data = self.find_data_by_name(data_name.as_str())?;
        self.program
            .data_members(data)
            .iter()
            .find_map(|candidate| match candidate {
                checked_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    let primitive = self
                        .program
                        .primitive_type_reference(field.type_reference)?;
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(field.type_reference);
                    Some((primitive, domain))
                }
                _ => None,
            })
    }
}
