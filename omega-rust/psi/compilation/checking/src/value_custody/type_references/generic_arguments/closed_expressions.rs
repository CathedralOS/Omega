//! Canonical structural composition of already typed const value recipes.
//!
//! Integer denotation comes from the shared selected-operation query, not an
//! operator-token evaluator. Constructors rejoin their exact nominal owner and
//! field symbols and encode in declaration order. Open binders and calls retain
//! their recipes for specialization/ordinary checked evaluation. Source
//! formation precedes this query; complete canonical carrier/predicate replay
//! follows every produced value, including receivers before projection.

use super::{
    AppliedValueCarrier, CanonicalConstValue, DataMember, DecodedCanonicalConstValue,
    PrimitiveType, SymbolHandle, TypeReferenceHandle, TypeReferenceNode, canonical_carrier_label,
    exact_const_primitive, exact_structured_const_carrier_is_eligible,
    validate_decoded_structured_const,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// `None` retains an open/deferred recipe. An invalid completed value reports
/// its formation/carrier failure instead of acquiring a guessed specialization.
pub fn normalize_closed_const_expression(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    carrier: TypeReferenceHandle,
) -> Result<Option<CanonicalConstValue>, String> {
    let carrier = AppliedValueCarrier::ordinary(carrier);
    if !exact_structured_const_carrier_is_eligible(program, carrier, &mut Vec::new()) {
        return Ok(None);
    }
    let mut context = Context {
        program,
        owner,
        active: Vec::new(),
        nodes: 0,
    };
    let Some(value) = context.value(expression, carrier)? else {
        return Ok(None);
    };
    let mut canonical = CanonicalConstValue::from_decoded(
        &value,
        program.expression_table.display_name(expression),
    )
    .ok_or_else(|| "completed const recipe exceeds canonical value encoding limits".to_owned())?;
    // The inner schema labels are replay consistency claims. The outer atom
    // retains exact declaration-owned identity across same-spelled carriers.
    if !matches!(
        value,
        DecodedCanonicalConstValue::Integer { .. } | DecodedCanonicalConstValue::Boolean(_)
    ) {
        canonical.type_name = program.normalized_type_identity_text(carrier.reference);
    }
    Ok(Some(canonical))
}

struct Context<'program> {
    program: &'program dyn crate::CheckingInput,
    owner: SymbolHandle,
    active: Vec<ExpressionHandle>,
    nodes: usize,
}

impl Context<'_> {
    fn value(
        &mut self,
        expression: ExpressionHandle,
        carrier: AppliedValueCarrier<'_>,
    ) -> Result<Option<DecodedCanonicalConstValue>, String> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
            || self.active.contains(&expression)
        {
            return Err("const recipe has an invalid or cyclic expression edge".to_owned());
        }
        if self.active.len() >= CanonicalConstValue::MAXIMUM_DEPTH
            || self.nodes >= CanonicalConstValue::MAXIMUM_NODES
        {
            return Err("const recipe exceeds canonical structural limits".to_owned());
        }
        self.nodes += 1;
        self.active.push(expression);
        let result = self.node(expression, carrier);
        self.active.pop();
        let value = result?;
        if let Some(value) = &value {
            validate_decoded_structured_const(self.program, carrier, value, &mut Vec::new())?;
        }
        Ok(value)
    }

    fn node(
        &mut self,
        expression: ExpressionHandle,
        carrier: AppliedValueCarrier<'_>,
    ) -> Result<Option<DecodedCanonicalConstValue>, String> {
        let program = self.program;
        let carrier = carrier.resolved(program);
        let expected = carrier.reference;
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            if program
                .data_type_parameter_by_symbol(path.symbol)
                .is_some_and(|parameter| parameter.kind.is_const_or_value())
            {
                return Ok(None);
            }
            if let Some(declaration) = program.const_declaration_by_symbol(path.symbol) {
                if !carrier.matches(program, declaration.declared_type) {
                    return Err(
                        "named const recipe leaf has a different declared carrier".to_owned()
                    );
                }
                let encoding = declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                    .ok_or("named const recipe leaf has no canonical value")?;
                return CanonicalConstValue::new("", encoding, "")
                    .decode_encoding()
                    .map(Some)
                    .ok_or_else(|| "named const recipe leaf is not a canonical index".to_owned());
            }
        }
        if let ExpressionNode::Member(member) = node {
            let receiver = crate::parameter_expression_result_type_reference(
                program,
                self.owner,
                &[],
                member.receiver,
            )
            .ok_or("const field projection has no independently typed receiver")?;
            // Projection folding needs a completed receiver first. An open
            // runtime read retains its typed recipe; it is not a failed const
            // record projection. Ordinary source formation still owns its
            // selected member meaning and carrier/access obligations.
            let Some(value) =
                self.value(member.receiver, AppliedValueCarrier::ordinary(receiver))?
            else {
                return Ok(None);
            };
            let field =
                crate::value_custody::places::applied_data_member_field(program, receiver, member)
                    .ok_or("const field projection lost its exact declared field")?;
            if !carrier.matches(program, field.type_reference) {
                return Err("const field projection has a different declared carrier".to_owned());
            }
            let fields = match value {
                DecodedCanonicalConstValue::Record { fields, .. }
                | DecodedCanonicalConstValue::Variant { fields, .. } => fields,
                _ => {
                    return Err(
                        "const field projection did not produce its declared record/case"
                            .to_owned(),
                    );
                }
            };
            return fields
                .into_iter()
                .find(|(name, _)| name == field.name.as_str())
                .map(|(_, value)| Some(value))
                .ok_or_else(|| "const field projection lost its canonical field value".to_owned());
        }
        if let Some(primitive) = exact_const_primitive(program, expected) {
            if primitive == PrimitiveType::Bool {
                return Ok(super::super::const_predicates::closed_boolean(
                    program, self.owner, expression,
                )
                .map(DecodedCanonicalConstValue::Boolean));
            }
            if primitive.accepts_integer_literal() {
                let Some(value) = program.closed_integer_value_in(expression, self.owner) else {
                    return Ok(None);
                };
                if value.primitive.is_some_and(|actual| actual != primitive)
                    || value
                        .type_reference
                        .is_some_and(|actual| !carrier.matches(program, actual))
                {
                    return Err(
                        "const numeric recipe changes an already selected carrier or policy"
                            .to_owned(),
                    );
                }
                let value = value.value.to_string().parse::<i128>().map_err(
                    |_| "const numeric recipe exceeds the exact canonical integer carrier",
                )?;
                return Ok(Some(DecodedCanonicalConstValue::Integer {
                    type_name: primitive.name().to_owned(),
                    value,
                }));
            }
        }
        match (program.type_reference_table.type_reference(expected), node) {
            (
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                },
                ExpressionNode::ArrayLiteral(elements),
            ) => {
                let length = carrier
                    .array_length(program, length)
                    .ok_or("const array carrier extent is not closed")?;
                let elements = program.expression_table.expression_handles(*elements);
                if elements.len() != length {
                    return Err("const array recipe has the wrong element count".to_owned());
                }
                let mut values = Vec::with_capacity(length);
                for element in elements {
                    let Some(value) = self.value(*element, carrier.child(*element_type))? else {
                        return Ok(None);
                    };
                    values.push(value);
                }
                Ok(Some(DecodedCanonicalConstValue::Array {
                    type_name: canonical_carrier_label(program, carrier),
                    values,
                }))
            }
            (TypeReferenceNode::Named { symbol, .. }, ExpressionNode::StructLiteral(literal)) => {
                if *symbol != literal.type_symbol {
                    return Err("const constructor names another nominal carrier".to_owned());
                }
                let definition = program
                    .data_definition_by_symbol(*symbol)
                    .ok_or("const constructor lost its data owner")?;
                let (case, fields) = if let Some(case) = literal.case_symbol {
                    let case = program
                        .data_members(definition)
                        .iter()
                        .filter_map(DataMember::as_variant)
                        .find(|variant| variant.symbol == case)
                        .ok_or("const constructor lost its exact case owner")?;
                    (
                        Some(case),
                        program.data_payload_fields(case).iter().collect::<Vec<_>>(),
                    )
                } else {
                    if literal.case_name.is_some() {
                        return Err("const constructor has an unbound case spelling".to_owned());
                    }
                    (
                        None,
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(DataMember::as_field)
                            .collect::<Vec<_>>(),
                    )
                };
                let supplied = program.expression_table.struct_fields(literal.fields);
                if fields.len() != supplied.len() {
                    return Err("const constructor has the wrong field count".to_owned());
                }
                let mut values = Vec::with_capacity(fields.len());
                for field in fields {
                    let mut selected = supplied
                        .iter()
                        .filter(|value| value.field_symbol == field.symbol);
                    let value = selected
                        .next()
                        .ok_or("const constructor is missing an exact declared field")?;
                    if selected.next().is_some() {
                        return Err("const constructor repeats a declared field".to_owned());
                    }
                    let field_type =
                        crate::value_custody::struct_literals::selected_construction_field_type(
                            program,
                            expression,
                            literal,
                            field.symbol,
                        )
                        .ok_or("const constructor field lost its selected carrier")?;
                    let Some(value) = self.value(value.value, carrier.child(field_type))? else {
                        return Ok(None);
                    };
                    values.push((field.name.as_str().to_owned(), value));
                }
                Ok(Some(if let Some(case) = case {
                    DecodedCanonicalConstValue::Variant {
                        type_name: definition.name.as_str().to_owned(),
                        case_name: case.name.as_str().to_owned(),
                        fields: values,
                    }
                } else {
                    DecodedCanonicalConstValue::Record {
                        type_name: definition.name.as_str().to_owned(),
                        fields: values,
                    }
                }))
            }
            _ => Ok(None),
        }
    }
}
