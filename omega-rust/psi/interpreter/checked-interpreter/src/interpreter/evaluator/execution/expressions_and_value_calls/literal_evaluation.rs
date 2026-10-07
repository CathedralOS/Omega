use crate::interpreter::evaluator::{
    BTreeMap, Cell, DataMember, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame,
    SymbolHandle, Value, trap,
};

impl<'program> Evaluator<'program> {
    pub(super) fn expression_receiver_is_namespace(&self, receiver: ExpressionHandle) -> bool {
        if !receiver.is_valid() {
            return false;
        }
        let ExpressionNode::Name(path) = self.program.expression_table.expression(receiver) else {
            return false;
        };
        self.is_declaration_namespace(path.symbol)
    }

    /// Construct a `data` value from a struct literal `Type { field: value, .. }`. Fields
    /// not named take the type's default; named fields override. A case literal
    /// (`Command::Say { text: ... }`) constructs an Enum value instead: the case name is
    /// the tag and the named payload fields fill the case's declared payload.
    pub(super) fn eval_struct_literal(
        &mut self,
        literal: &typed_trees::expression::TableStructLiteral,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        if let Some(case_name) = &literal.case_name {
            return self.eval_case_literal(literal, case_name.as_str(), frame);
        }
        let type_name = literal.type_name.as_str().to_owned();
        // The resolved literal already selected its exact nominal owner. A
        // module-qualified constructor spelling is not the declaration's
        // canonical name, so the name scan cannot find it; prefer the
        // resolved symbol, exactly as `eval_case_literal` does. The value
        // carries the declaration's name, not the authored spelling.
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == literal.type_symbol && literal.type_symbol.is_valid())
            .or_else(|| self.find_data_by_name(&type_name));
        let (type_symbol, type_name, mut fields) = if let Some(data) = data {
            let mut fields = BTreeMap::new();
            self.populate_data_fields(data, &mut fields)?;
            (data.symbol, data.name.as_str().to_owned(), fields)
        } else {
            (SymbolHandle::invalid(), type_name, BTreeMap::new())
        };
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let field_type = self.field_type_reference(type_symbol, field.name.as_str());
            let value =
                self.eval_expression_at_type(field.value, field_type.unwrap_or_default(), frame)?;
            // Coerce the field value to the field's declared width/domain, matching
            // the native store into the field slot (`Point { x: a+b }` with `a+b`
            // = 300 into a u8 field reads 44). The field type carries its own
            // domain, so resolve it directly.
            let value = match field_type {
                Some(type_reference) => self.coerce_scalar_value(value, type_reference)?,
                None => value,
            };
            let value = self.capture_value_operand(value, field_type.unwrap_or_default())?;
            fields.insert(field.name.as_str().into(), self.allocate_cell(value)?);
        }
        Ok(Value::Struct {
            type_symbol,
            type_name: type_name.into(),
            fields,
        })
    }

    /// Construct a payload-carrying case value `Type::Case { field: value, .. }`. Payload
    /// cells follow the case's DECLARED field order; unnamed payload fields default, named
    /// literal fields override, and a literal field that is not part of the case's payload
    /// traps.
    pub(super) fn eval_case_literal(
        &mut self,
        literal: &typed_trees::expression::TableStructLiteral,
        case_name: &str,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let type_name = literal.type_name.as_str();
        let Some(data) = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == literal.type_symbol && literal.type_symbol.is_valid())
        else {
            return trap(format!("unknown data type `{type_name}` in case literal"));
        };
        let Some(variant) =
            self.program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if Some(variant.symbol) == literal.case_symbol => {
                        Some(variant)
                    }
                    _ => None,
                })
        else {
            return trap(format!("`{type_name}` has no case `{case_name}`"));
        };

        let mut payload = Vec::new();
        // MIXED shapes: the COMMON fields exist in every case and come first.
        // Case construction ZERO-initializes them (frozen decision 7's rule;
        // never the declared default -- validation rejects defaults on mixed
        // common fields), unless the literal names them below.
        for member in self.program.data_members(data) {
            let DataMember::Field(common_field) = member else {
                continue;
            };
            let name: crate::value::ValueName = common_field.name.as_str().into();
            let value = self.default_value_for_type(common_field.type_reference)?;
            payload.push((name, self.allocate_cell(value)?));
        }
        for field in self.program.data_payload_fields(variant) {
            let name: crate::value::ValueName = field.name.as_str().into();
            let value = self.default_value_for_type(field.type_reference)?;
            payload.push((name, self.allocate_cell(value)?));
        }
        for field in self.program.expression_table.struct_fields(literal.fields) {
            // Use this case's payload declaration, not a same-named field of
            // another variant. Common fields keep their enclosing record type.
            let field_type = self
                .program
                .data_payload_fields(variant)
                .iter()
                .find(|declared| declared.name == field.name)
                .map(|declared| declared.type_reference)
                .or_else(|| self.field_type_reference(data.symbol, field.name.as_str()));
            let value =
                self.eval_expression_at_type(field.value, field_type.unwrap_or_default(), frame)?;
            let value = self.capture_value_operand(value, field_type.unwrap_or_default())?;
            let Some(slot) = payload
                .iter_mut()
                .find(|(name, _)| **name == *field.name.as_str())
            else {
                return trap(format!(
                    "case `{type_name}::{case_name}` has no payload field `{}`",
                    field.name.as_str()
                ));
            };
            slot.1 = self.allocate_cell(value)?;
        }

        Ok(Value::Enum {
            type_symbol: data.symbol,
            variant_name: case_name.into(),
            payload,
        })
    }

    pub(super) fn eval_call_expression_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Cell> {
        // Share the same argument-evaluation rules as state calls (incl. reference
        // forwarding for bare-place arguments that already hold a `&mut`).
        self.eval_argument(argument, frame)
    }
}
