use crate::interpreter::evaluator::{
    BTreeMap, Cell, DataMember, EvalResult, Evaluator, ExpressionHandle, Frame, Value, trap,
};

impl<'program> Evaluator<'program> {
    pub(super) fn expression_receiver_is_namespace(&self, receiver: ExpressionHandle) -> bool {
        if !receiver.is_valid() {
            return false;
        }
        let Some(path) = self.program.expression_table.name_path_node(receiver) else {
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
        expression: ExpressionHandle,
        literal: &typed_trees::expression::TableStructLiteral,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        if let Some(case_name) = &literal.case_name {
            return self.eval_case_literal(expression, literal, case_name.as_str(), frame);
        }
        // The constructor occurrence retains its applied field telescope.
        // A nominal template's open members cannot supply this value's storage.
        let Some(data) =
            validation::selected_construction_definition(self.program, expression, literal)
        else {
            return trap("record construction has no exact receiving data shape");
        };
        let type_symbol = data.symbol;
        let type_name = data.name.as_str().to_owned();
        let mut fields = BTreeMap::new();
        self.populate_data_fields(data, &mut fields)?;
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let Some(field_type) = validation::selected_construction_field_type(
                self.program,
                expression,
                literal,
                field.field_symbol,
            ) else {
                return trap(format!(
                    "record field `{}` has no checked receiving type",
                    field.name
                ));
            };
            let value = self.eval_expression_at_type(field.value, field_type, frame)?;
            // Coerce the field value to the field's declared width/domain, matching
            // the native store into the field slot (`Point { x: a+b }` with `a+b`
            // = 300 into a u8 field reads 44). The field type carries its own
            // domain, so resolve it directly.
            let value = self.coerce_scalar_value(value, field_type)?;
            let value = self.capture_value_operand(value, field_type)?;
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
        expression: ExpressionHandle,
        literal: &typed_trees::expression::TableStructLiteral,
        case_name: &str,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let type_name = literal.type_name.as_str();
        let Some(data) =
            validation::selected_construction_definition(self.program, expression, literal)
        else {
            return trap(format!(
                "case `{type_name}::{case_name}` has no exact receiving data shape"
            ));
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
            let Some(common_field) = member.as_field() else {
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
            let Some(field_type) = validation::selected_construction_field_type(
                self.program,
                expression,
                literal,
                field.field_symbol,
            ) else {
                return trap(format!(
                    "case field `{}` has no checked receiving type",
                    field.name
                ));
            };
            let value = self.eval_expression_at_type(field.value, field_type, frame)?;
            let value = self.capture_value_operand(value, field_type)?;
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
