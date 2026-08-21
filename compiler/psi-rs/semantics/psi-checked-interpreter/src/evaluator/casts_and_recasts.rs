use super::*;

impl<'program> Evaluator<'program> {
    /// The target `PrimitiveType` of a cast's full type reference.
    pub(super) fn cast_target_primitive(
        &self,
        target_type: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        self.program.primitive_type_reference(target_type)
    }

    /// Apply an `as` cast with width/signedness semantics: int<->float conversions and
    /// integer narrowing/widening (wrapping to the target width, sign- or zero-extending on
    /// read per the SOURCE signedness, which the value carries as its width tag).
    pub(super) fn eval_cast(
        &self,
        value: Value,
        source: Option<PrimitiveType>,
        target: Option<PrimitiveType>,
        domain: ArithmeticDomain,
    ) -> EvalResult<Value> {
        let Some(target) = target else {
            // A cast to a non-primitive (e.g. a trait object) is a no-op identity here.
            return Ok(value);
        };
        match target {
            PrimitiveType::F32 | PrimitiveType::F64 => {
                let format = if target == PrimitiveType::F32 {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                let meaning = match value {
                    Value::Float(source) => {
                        FloatSemantics::convert(format, &FloatMeaning::from_f64(source))
                    }
                    Value::Int(value) => {
                        let value = if source.is_some_and(is_unsigned_integer_primitive) {
                            BigInt::from_u64(value as u64)
                        } else {
                            BigInt::from_i64(value)
                        };
                        FloatSemantics::from_integer(format, &value)
                    }
                    _ => return trap(format!("cast to {target:?} of non-numeric")),
                };
                Ok(Value::Float(meaning.to_interpreter_value(format)))
            }
            PrimitiveType::Bool => Ok(Value::Bool(value.as_bool().unwrap_or(false))),
            integer => {
                // Int -> int reinterprets at the target width; the result
                // keeps the target's width tag so later ops/casts wrap.
                let raw = match value {
                    // FLOAT -> int is one of three distinct conversion
                    // operations. Wrapping has no float reading and is
                    // rejected by validation.
                    Value::Float(value) => {
                        let format = semantic_integer_format(integer)
                            .expect("a primitive integer has a semantic format");
                        let meaning = FloatMeaning::from_f64(value);
                        let converted = match domain {
                            ArithmeticDomain::Saturating => {
                                FloatSemantics::to_integer_saturating(&meaning, format)
                            }
                            ArithmeticDomain::Trapping => FloatSemantics::to_integer_trapping(
                                &meaning, format,
                            )
                            .map_err(|reason| {
                                Halt::Trap(float_to_integer_trap_message(integer, reason, true))
                            })?,
                            ArithmeticDomain::Exact => FloatSemantics::to_integer_exact(
                                &meaning, format,
                            )
                            .map_err(|reason| {
                                Halt::Trap(float_to_integer_trap_message(integer, reason, false))
                            })?,
                            ArithmeticDomain::Wrapping => {
                                return trap(
                                    "Wrapping float-to-integer conversion has no semantic reading"
                                        .to_owned(),
                                );
                            }
                        };
                        return Ok(Value::Int(big_integer_runtime_value(&converted, integer)));
                    }
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("cast to integer of non-numeric".to_owned()))?,
                };
                Ok(Value::Int(wrap_to_width(raw, integer)))
            }
        }
    }

    /// §5b recast (`&x as &T`): bit-REINTERPRET, never convert. Validation
    /// (psi-validation recasts.rs, rung A) guarantees equal scalar widths
    /// and fences bool/text/records, so the reinterpretation below is total.
    /// A SNAPSHOT of the source's bits is sound for the shared-only rung:
    /// borrow exclusivity freezes the source while the view lives. Native
    /// needs no twin -- the emitted load already reads the place's bytes
    /// through the stated type.
    /// The interior half of the §5b recast: a LITERAL-indexed read over a
    /// byte array assembles `size_of(target)` bytes little-endian (floats
    /// from the assembled bits). `Ok(None)` when the shape is not the
    /// interior class (the scalar-pun path then evaluates normally).
    pub(super) fn eval_interior_recast(
        &mut self,
        cast: &psi_typed_trees::expression::TableCastExpression,
        target: Option<PrimitiveType>,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let ExpressionNode::Indexed(indexed) =
            self.program.expression_table.expression(cast.value).clone()
        else {
            return Ok(None);
        };
        // Literal or RUNTIME offset (rung C1): both evaluate to the byte
        // position the view starts at.
        let offset_value = match self.program.expression_table.expression(indexed.index) {
            ExpressionNode::Integer(literal) => literal.value_i64(),
            _ => self.eval_expression(indexed.index, frame)?.as_int(),
        };
        let Some(offset) = offset_value.and_then(|value| usize::try_from(value).ok()) else {
            return Ok(None);
        };
        let collection = self.eval_expression(indexed.collection, frame)?;
        let Value::Array(cells) = collection else {
            return Ok(None);
        };
        // RUNG C2: a RECORD target assembles field-by-field at
        // natural-alignment offsets (each field at the next multiple of its
        // own size -- LOCKSTEP with the layout rule; the drift canary pins
        // agreement).
        let Some(target) = target else {
            return self.assemble_record_view_type(cast.target_type, &cells, offset);
        };
        self.assemble_scalar_byte_region(&cells, offset, target)
            .map(Some)
    }
}
