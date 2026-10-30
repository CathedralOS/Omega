//! Independently encode a materialized initializer after declaration binding.
//!
//! Current and retained declarations share resolved identities here. No source
//! header is reconstructed, and the receipt supplies neither fields nor types.
//! The original declared carrier remains intact; only authenticated explicit
//! storage representations project their semantic value for payload encoding.

use language_core::constant_value::EvaluatedConstant;
use language_semantics::const_value::{
    CanonicalConstIdentity, DecodedCanonicalConstValue as Value,
};
use numerics::literals::FloatFormat;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataDefinition, DataField, DataMember};
use symbol_resolved_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteralField,
};
use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
use symbols::{BuiltinTypeAtom, SymbolHandle};

#[derive(Clone, Copy)]
struct Operand<'program> {
    reference: &'program TypeReference,
    frame: usize,
}

struct Binding<'program> {
    parameter: SymbolHandle,
    actual: Operand<'program>,
}

struct Encoder<'program> {
    program: &'program SymbolResolvedTrees,
    bindings: Vec<Binding<'program>>,
}

pub(super) fn validate(
    program: &SymbolResolvedTrees,
    destination: &TypeReference,
    materialized: ExpressionHandle,
    expected: &EvaluatedConstant,
) -> Result<(), String> {
    let mut encoder = Encoder {
        program,
        bindings: Vec::new(),
    };
    let operand = Operand {
        reference: destination,
        frame: 0,
    };
    let actual = if let ExpressionNode::UndeterminedNaN(format) =
        program.tables.bodies.expressions.expression(materialized)
    {
        let operand = encoder.unconstrained(operand)?;
        let TypeReference::Named { symbol, .. } = operand.reference else {
            return Err("undetermined NaN lost its declared floating carrier".into());
        };
        if float_format(program.symbols.builtin_type_atom(*symbol)) != Some(*format) {
            return Err("undetermined NaN conflicts with its declared floating carrier".into());
        }
        EvaluatedConstant::UndeterminedNaN(*format)
    } else {
        let value = encoder.value(operand, materialized, 0)?;
        let identity = CanonicalConstIdentity::from_declaration_value(&value)
            .ok_or("materialized initializer exceeds canonical declaration encoding limits")?;
        EvaluatedConstant::Encoded(identity.encoding)
    };
    if &actual != expected {
        return Err("materialized value drifted from its canonical result".into());
    }
    Ok(())
}

impl<'program> Encoder<'program> {
    fn resolve(&self, mut operand: Operand<'program>) -> Result<Operand<'program>, String> {
        for _ in 0..=self.bindings.len() {
            let TypeReference::Named { symbol, .. } = operand.reference else {
                return Ok(operand);
            };
            let Some(binding) = self.bindings[..operand.frame]
                .iter()
                .rev()
                .find(|binding| binding.parameter == *symbol)
            else {
                return Ok(operand);
            };
            operand = binding.actual;
        }
        Err("materialized initializer has cyclic declaration arguments".into())
    }

    fn unconstrained(&self, mut operand: Operand<'program>) -> Result<Operand<'program>, String> {
        loop {
            operand = self.resolve(operand)?;
            let TypeReference::Constrained(constraint) = operand.reference else {
                return Ok(operand);
            };
            operand.reference = self.program.child_type_reference(constraint.base_type);
        }
    }

    fn value(
        &mut self,
        operand: Operand<'program>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Result<Value, String> {
        if depth >= 64 {
            return Err("materialized initializer exceeds canonical declaration depth".into());
        }
        let operand = self.unconstrained(operand)?;
        let program = self.program;
        if let TypeReference::Generic(application) = operand.reference
            && let Some(definition) = program.data_definition_by_symbol(application.base_symbol)
            && is_layout_storage(program, definition)
        {
            let arguments = program.child_type_references(application.arguments);
            if arguments.len() != 2 || !application.lifetime_arguments.is_empty() {
                return Err("explicit storage initializer lost its complete application".into());
            }
            // The producer's full delivered type is checked before literal
            // materialization; receiving typed replay checks it again. Retain
            // the original declaration and policy, projecting only the value.
            return self.value(
                Operand {
                    reference: &arguments[0],
                    frame: operand.frame,
                },
                expression,
                depth + 1,
            );
        }
        let table = &program.tables.bodies.expressions;
        match operand.reference {
            TypeReference::FixedArray(array) => {
                let ExpressionNode::ArrayLiteral(elements) = table.expression(expression) else {
                    return Err("materialized initializer is not its declared array".into());
                };
                let length = self.array_length(&array.length, operand.frame)?;
                let elements = table.expression_handles(*elements);
                if elements.len() != length {
                    return Err("materialized initializer array length drifted".into());
                }
                let element = Operand {
                    reference: program.child_type_reference(array.element_type),
                    frame: operand.frame,
                };
                let type_name = format!("[{}; {length}]", self.label(element, depth + 1)?);
                let values = elements
                    .iter()
                    .map(|element_expression| self.value(element, *element_expression, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array { type_name, values })
            }
            TypeReference::Named { symbol, .. }
                if program.symbols.builtin_type_atom(*symbol).is_some() =>
            {
                self.scalar(
                    program
                        .symbols
                        .builtin_type_atom(*symbol)
                        .ok_or("scalar initializer lost builtin identity")?,
                    expression,
                )
            }
            TypeReference::Named { symbol, .. } => {
                let definition = program
                    .data_definition_by_symbol(*symbol)
                    .ok_or("materialized initializer lost its selected data declaration")?;
                self.nominal(
                    definition,
                    None,
                    operand.frame,
                    definition.name.as_str().to_owned(),
                    expression,
                    depth,
                )
            }
            TypeReference::Generic(application) => {
                let definition = program
                    .data_definition_by_symbol(application.base_symbol)
                    .ok_or("materialized initializer lost its selected generic data declaration")?;
                let label = self.label(operand, depth + 1)?;
                self.nominal(
                    definition,
                    Some(application),
                    operand.frame,
                    label,
                    expression,
                    depth,
                )
            }
            _ => Err("materialized initializer has no canonical declaration carrier".into()),
        }
    }

    fn nominal(
        &mut self,
        definition: &'program DataDefinition,
        application: Option<&'program symbol_resolved_trees::types::GenericTypeReference>,
        frame: usize,
        label: String,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Result<Value, String> {
        let program = self.program;
        // Materialization reconstructs an authored template constructor, and
        // receiving preparation may rejoin that exact closed instance. Keep
        // both declaration identities; another instance of the same template
        // is not a permitted constructor for this destination.
        let actual_type = match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::StructLiteral(literal) => literal.type_symbol,
            ExpressionNode::Name(path)
                if program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
            {
                program.symbols.get(path.symbol).parent
            }
            _ => return Err("materialized initializer lost its nominal constructor".into()),
        };
        let (definition, application) = if actual_type == definition.symbol {
            (definition, application)
        } else if let Some(TypeReference::Generic(origin)) = &definition.generic_instance {
            if actual_type != origin.base_symbol {
                return Err("materialized initializer selects a different nominal instance".into());
            }
            (
                program
                    .data_definition_by_symbol(origin.base_symbol)
                    .ok_or("materialized instance lost its template declaration")?,
                Some(origin),
            )
        } else {
            return Err("materialized initializer selects a different nominal carrier".into());
        };
        if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || definition.quotient.is_some()
        {
            return Err("materialized initializer has no ordinary source-owned constructor".into());
        }
        let start = self.bindings.len();
        let parameters = program.data_type_parameters(definition.type_parameters);
        if let Some(application) = application {
            let arguments = program.child_type_references(application.arguments);
            if arguments.len() != parameters.len()
                || application.lifetime_arguments.len() != definition.lifetime_parameters.len()
            {
                return Err(
                    "materialized initializer lost its complete generic application".into(),
                );
            }
            for (parameter, argument) in parameters.iter().zip(arguments) {
                self.bindings.push(Binding {
                    parameter: parameter.symbol,
                    actual: Operand {
                        reference: argument,
                        frame,
                    },
                });
            }
        } else if !parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
            return Err(
                "materialized initializer requires a closed declaration application".into(),
            );
        }
        let bound_frame = self.bindings.len();
        let result = (|| {
            let table = &program.tables.bodies.expressions;
            let (actual_type, case, supplied) = match table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => (
                    literal.type_symbol,
                    literal.case_symbol,
                    table.struct_fields(literal.fields),
                ),
                ExpressionNode::Name(path)
                    if program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
                {
                    (
                        program.symbols.get(path.symbol).parent,
                        Some(path.symbol),
                        &[][..],
                    )
                }
                _ => return Err("materialized initializer lost its nominal constructor".into()),
            };
            if actual_type != definition.symbol {
                return Err("materialized initializer selects a different nominal carrier".into());
            }
            let members = program.data_members(definition.members);
            if let Some(case) = case {
                let variant = members
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == case => Some(variant),
                        _ => None,
                    })
                    .ok_or("materialized initializer case belongs to a different declaration")?;
                let fields = program
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect::<Vec<_>>();
                Ok(Value::Variant {
                    type_name: label,
                    case_name: variant.name.as_str().to_owned(),
                    fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                })
            } else {
                let fields = members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if members
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    return Err("materialized initializer omitted its declared case".into());
                }
                Ok(Value::Record {
                    type_name: label,
                    fields: self.fields(&fields, supplied, bound_frame, depth + 1)?,
                })
            }
        })();
        self.bindings.truncate(start);
        result
    }

    fn fields(
        &mut self,
        declared: &[&'program DataField],
        supplied: &'program [TableStructLiteralField],
        frame: usize,
        depth: usize,
    ) -> Result<Vec<(String, Value)>, String> {
        if declared.len() != supplied.len() {
            return Err("materialized initializer field roster drifted".into());
        }
        declared
            .iter()
            .map(|field| {
                let mut matches = supplied
                    .iter()
                    .filter(|actual| actual.field_symbol == field.symbol);
                let actual = matches.next().ok_or_else(|| {
                    format!("materialized initializer lost field `{}`", field.name)
                })?;
                if matches.next().is_some() {
                    return Err("materialized initializer repeats a declared field".into());
                }
                Ok((
                    field.name.as_str().to_owned(),
                    self.value(
                        Operand {
                            reference: &field.type_reference,
                            frame,
                        },
                        actual.value,
                        depth,
                    )?,
                ))
            })
            .collect()
    }

    fn scalar(&self, atom: BuiltinTypeAtom, expression: ExpressionHandle) -> Result<Value, String> {
        let node = self
            .program
            .tables
            .bodies
            .expressions
            .expression(expression);
        if atom == BuiltinTypeAtom::Bool {
            return match node {
                ExpressionNode::Boolean(value) => Ok(Value::Boolean(*value)),
                _ => Err("materialized initializer conflicts with bool".into()),
            };
        }
        if let Some(format) = float_format(Some(atom)) {
            let literal = match node {
                ExpressionNode::Float(literal) => literal.clone(),
                ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                    numerics::literals::FloatLiteral::parse(
                        &literal
                            .value_bignum()
                            .ok_or("materialized floating initializer lost its integer value")?
                            .to_string(),
                    )
                    .ok_or("materialized floating initializer is not exact")?
                }
                _ => return Err("materialized initializer conflicts with floating carrier".into()),
            };
            if literal.landing().is_some_and(|actual| actual != format) {
                return Err("materialized initializer conflicts with floating landing".into());
            }
            let bits =
                match format {
                    FloatFormat::F32
                        if literal.presented_bits().is_some() || !literal.value_f32().is_nan() =>
                    {
                        u64::from(literal.value_f32().to_bits())
                    }
                    FloatFormat::F64
                        if literal.presented_bits().is_some() || !literal.value_f64().is_nan() =>
                    {
                        literal.value_f64().to_bits()
                    }
                    _ => return Err(
                        "materialized floating initializer lacks determined representation bits"
                            .into(),
                    ),
                };
            return Ok(Value::Float { format, bits });
        }
        let carrier = integer_carrier(atom)
            .ok_or("materialized initializer has an unsupported builtin carrier")?;
        let ExpressionNode::Integer(literal) = node else {
            return Err("materialized initializer conflicts with integer carrier".into());
        };
        if literal.landing().is_some_and(|landing| {
            landing.landed_type.name() != carrier
                || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
        }) {
            return Err("materialized initializer conflicts with integer landing".into());
        }
        let value = literal
            .value_i64()
            .map(i128::from)
            .or_else(|| literal.value_u64().map(i128::from))
            .ok_or("materialized initializer integer exceeds canonical storage")?;
        let integer_type = numerics::literals::LandedIntegerType::from_name(carrier)
            .ok_or("materialized integer initializer lost its selected carrier")?;
        let (minimum, maximum) = integer_type.bounds();
        if value < minimum || value > maximum {
            return Err(format!(
                "materialized const value `{value}` does not fit `{carrier}`"
            ));
        }
        Ok(Value::Integer {
            type_name: carrier.to_owned(),
            value,
        })
    }

    fn array_length(&self, length: &FixedArrayLength, frame: usize) -> Result<usize, String> {
        match length {
            FixedArrayLength::Literal(length) => Ok(*length),
            FixedArrayLength::ConstParameter { symbol, .. } => {
                let binding = self.bindings[..frame]
                    .iter()
                    .rev()
                    .find(|binding| binding.parameter == *symbol)
                    .ok_or("materialized array extent has no exact declaration binding")?;
                let argument = self.resolve(binding.actual)?;
                let TypeReference::Named { name, .. } = argument.reference else {
                    return Err("materialized array extent is not a completed integer".into());
                };
                name.as_str().parse().map_err(|_| {
                    "materialized array extent is not a closed nonnegative integer".into()
                })
            }
            _ => Err("materialized array extent is not complete".into()),
        }
    }

    fn label(&self, operand: Operand<'program>, depth: usize) -> Result<String, String> {
        if depth >= 64 {
            return Err("materialized carrier label exceeds canonical depth".into());
        }
        let operand = self.unconstrained(operand)?;
        match operand.reference {
            TypeReference::Named { symbol, name, .. } => {
                Ok(self.program.data_definition_by_symbol(*symbol).map_or_else(
                    || name.as_str().to_owned(),
                    |definition| definition.name.as_str().to_owned(),
                ))
            }
            TypeReference::FixedArray(array) => Ok(format!(
                "[{}; {}]",
                self.label(
                    Operand {
                        reference: self.program.child_type_reference(array.element_type),
                        frame: operand.frame
                    },
                    depth + 1
                )?,
                self.array_length(&array.length, operand.frame)?
            )),
            TypeReference::Generic(application) => {
                let name = self
                    .program
                    .data_definition_by_symbol(application.base_symbol)
                    .map_or_else(
                        || self.program.symbols.name(application.base_symbol),
                        |definition| definition.name.as_str(),
                    );
                let arguments = self
                    .program
                    .child_type_references(application.arguments)
                    .iter()
                    .map(|reference| {
                        self.label(
                            Operand {
                                reference,
                                frame: operand.frame,
                            },
                            depth + 1,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{name}<{}>", arguments.join(", ")))
            }
            _ => Err("materialized initializer carrier has no canonical label".into()),
        }
    }
}

fn is_layout_storage(program: &SymbolResolvedTrees, definition: &DataDefinition) -> bool {
    program
        .symbols
        .symbol_source_span(definition.symbol)
        .is_some_and(|reference| {
            crate::preparation::declaration_selection::is_core_layout_storage_source(
                &program.symbols,
                definition.name.as_str(),
                definition.supply_mode,
                reference,
            )
        })
}

fn float_format(atom: Option<BuiltinTypeAtom>) -> Option<FloatFormat> {
    match atom {
        Some(BuiltinTypeAtom::F32) => Some(FloatFormat::F32),
        Some(BuiltinTypeAtom::F64) => Some(FloatFormat::F64),
        _ => None,
    }
}

fn integer_carrier(atom: BuiltinTypeAtom) -> Option<&'static str> {
    (atom.is_fixed_integer() || atom == BuiltinTypeAtom::Address).then_some(atom.symbol_name())
}
