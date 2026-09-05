//! Value-sensitive result admission for compiler-run semantic machines.
//!
//! `ConstEvaluable(T, value)` is deliberately target-neutral: it admits only
//! closed, freely copyable semantic values that can cross the interpreter
//! boundary as an owned snapshot. Layout and byte materialization are later,
//! separate judgments.

use language_semantics::{DataSupplyMode, Multiplicity};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use crate::BuildTimeValue;

pub(super) fn require_const_evaluable_result(
    program: &TypedTrees,
    machine: &Machine,
    value: &BuildTimeValue,
) -> Result<(), String> {
    let state = entry_state(program, machine).ok_or_else(|| {
        format!(
            "machine `{}` has no state whose result can be checked for ConstEvaluable",
            machine.name
        )
    })?;
    let mut active_data = Vec::new();
    check_value(
        program,
        state.return_type,
        value,
        "result",
        &mut active_data,
    )
    .map_err(|violation| {
        format!(
            "build-time result of machine `{}` is not ConstEvaluable: {violation}",
            machine.name
        )
    })
}

fn entry_state<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
) -> Option<&'program typed_trees::state::State> {
    let states = program.machine_states(machine);
    let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}

fn check_value(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    if !type_reference.is_valid() {
        return Err(format!("{path} has an invalid declared type"));
    }
    if matches!(value, BuildTimeValue::Text(_)) {
        return Err(format!(
            "{path} contains Text; borrowed or dynamically sized text cannot cross the semantic const boundary"
        ));
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Unit => expect_value(path, value, "Unit", |value| {
            matches!(value, BuildTimeValue::Unit)
        }),
        TypeReferenceNode::Named { symbol, name } => {
            if name.as_str().starts_with("Atomic") {
                return Err(format!(
                    "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                ));
            }
            if let Some(primitive) = PrimitiveType::from_name(name.as_str()) {
                return check_primitive(path, primitive, value);
            }
            check_named_data(program, *symbol, name.as_str(), value, path, active_data)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let FixedArrayLength::Literal(expected_length) = length else {
                return Err(format!(
                    "{path} has a non-literal array length and is not a closed const type"
                ));
            };
            let BuildTimeValue::Array(elements) = value else {
                return Err(format!(
                    "{path} expected an array value, found {}",
                    kind(value)
                ));
            };
            if elements.len() != *expected_length {
                return Err(format!(
                    "{path} expected {expected_length} array element(s), found {}",
                    elements.len()
                ));
            }
            for (index, element) in elements.iter().enumerate() {
                check_value(
                    program,
                    *element_type,
                    element,
                    &format!("{path}[{index}]"),
                    active_data,
                )?;
            }
            Ok(())
        }
        TypeReferenceNode::Reference { .. } => Err(format!(
            "{path} has reference type; references cannot escape semantic const evaluation"
        )),
        TypeReferenceNode::Slice { .. } => Err(format!(
            "{path} has slice type; dynamically sized values cannot cross the semantic const boundary"
        )),
        // Constraints and arithmetic-domain annotations have already been
        // checked by typed lowering and enforced by interpreter coercion. They
        // refine the same runtime carrier; ConstEvaluable therefore follows
        // the structured base type rather than treating the annotation as a
        // new value representation.
        TypeReferenceNode::Constrained { base_type, .. } => {
            check_value(program, *base_type, value, path, active_data)
        }
        TypeReferenceNode::Generic { .. } => Err(format!(
            "{path} has an open or generic aggregate type and is not a closed const type"
        )),
        TypeReferenceNode::ConstExpression(_) => Err(format!(
            "{path} has a proof-static expression type, not a runtime const value type"
        )),
        TypeReferenceNode::DynamicTrait { .. } => Err(format!(
            "{path} has a dynamic trait type and is not a closed const type"
        )),
    }
}

fn check_primitive(
    path: &str,
    primitive: PrimitiveType,
    value: &BuildTimeValue,
) -> Result<(), String> {
    match (primitive, value) {
        (PrimitiveType::Bool, BuildTimeValue::Bool(_))
        | (PrimitiveType::F32 | PrimitiveType::F64, BuildTimeValue::Float(_)) => Ok(()),
        (primitive, BuildTimeValue::Int(_)) if primitive.accepts_integer_literal() => Ok(()),
        _ => Err(format!(
            "{path} expected `{}`, found {}",
            primitive.name(),
            kind(value)
        )),
    }
}

fn check_named_data(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    if !symbol.is_valid() {
        return Err(format!(
            "{path} names `{name}` without an exact nominal type identity"
        ));
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions
        .next()
        .ok_or_else(|| format!("{path} names unknown data type `{name}`"))?;
    if definitions.next().is_some() {
        return Err(format!(
            "{path} has ambiguous nominal type identity for `{name}`"
        ));
    }
    if definition.name.as_str() != name {
        return Err(format!(
            "{path} has inconsistent nominal type spelling `{name}` for `{}`",
            definition.name
        ));
    }
    require_closed_data_shape(program, definition, path)?;
    if active_data.contains(&symbol) {
        return Err(format!(
            "{path} reaches recursive data `{name}`, which is outside the closed aggregate const boundary"
        ));
    }
    active_data.push(symbol);
    let result = match DataDefinition::shape_kind_from_members(program.data_members(definition)) {
        DataShapeKind::Empty | DataShapeKind::Record => {
            if definition.properties.multiplicity == Multiplicity::Unrestricted {
                check_record(program, definition, value, path, active_data)
            } else {
                Err(format!(
                    "{path} has affine or linear type `{}`; ConstEvaluable record results must be freely copyable",
                    definition.name
                ))
            }
        }
        // Sum admission is intentionally value-sensitive: only the realized
        // case and its payload cross the boundary. An inactive reference/Text
        // case must not contaminate an otherwise closed copy-eligible result.
        DataShapeKind::Enum => check_sum(program, definition, value, path, active_data),
        DataShapeKind::Mixed => Err(format!(
            "{path} has mixed record/sum shape `{name}`, which is outside this ConstEvaluable stage"
        )),
    };
    active_data.pop();
    result
}

fn require_closed_data_shape(
    program: &TypedTrees,
    definition: &DataDefinition,
    path: &str,
) -> Result<(), String> {
    if definition.supply_mode != DataSupplyMode::CheckedShape {
        return Err(format!(
            "{path} has boundary-opaque type `{}`, not a checked const shape",
            definition.name
        ));
    }
    if !definition.lifetime_parameters.is_empty()
        || !program.data_type_parameters(definition).is_empty()
        || definition.generic_instance.is_some()
    {
        return Err(format!(
            "{path} has open or generic aggregate type `{}`",
            definition.name
        ));
    }
    if definition.quotient.is_some() {
        return Err(format!(
            "{path} has quotient type `{}`; quotient representatives require their separate checked admission",
            definition.name
        ));
    }
    Ok(())
}

fn check_record(
    program: &TypedTrees,
    definition: &DataDefinition,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(format!(
            "{path} expected record `{}`, found {}",
            definition.name,
            kind(value)
        ));
    };
    if type_name != definition.name.as_str() {
        return Err(format!(
            "{path} expected record `{}`, found record `{type_name}`",
            definition.name
        ));
    }
    let declared_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if fields.len() != declared_fields.len() {
        return Err(format!(
            "{path} expected {} field(s), found {}",
            declared_fields.len(),
            fields.len()
        ));
    }
    for declared in declared_fields {
        let mut matches = fields
            .iter()
            .filter(|(name, _)| name == declared.name.as_str());
        let (_, field_value) = matches.next().ok_or_else(|| {
            format!(
                "{path} is missing declared field `{}` of `{}`",
                declared.name, definition.name
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "{path} repeats field `{}` of `{}`",
                declared.name, definition.name
            ));
        }
        check_value(
            program,
            declared.type_reference,
            field_value,
            &format!("{path}.{}", declared.name),
            active_data,
        )?;
    }
    Ok(())
}

fn check_sum(
    program: &TypedTrees,
    definition: &DataDefinition,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err(format!(
            "{path} expected a case of `{}`, found {}",
            definition.name,
            kind(value)
        ));
    };
    let mut variants = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(candidate) if candidate.name.as_str() == variant => Some(candidate),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        });
    let selected = variants.next().ok_or_else(|| {
        format!(
            "{path} names unknown case `{variant}` of `{}`",
            definition.name
        )
    })?;
    if variants.next().is_some() {
        return Err(format!(
            "{path} names ambiguous case `{variant}` of `{}`",
            definition.name
        ));
    }
    let declared_payload = program.data_payload_fields(selected);
    if payload.len() != declared_payload.len() {
        return Err(format!(
            "{path} case `{variant}` expected {} payload field(s), found {}",
            declared_payload.len(),
            payload.len()
        ));
    }
    for declared in declared_payload {
        let mut matches = payload
            .iter()
            .filter(|(name, _)| name == declared.name.as_str());
        let (_, payload_value) = matches.next().ok_or_else(|| {
            format!(
                "{path} case `{variant}` is missing payload field `{}`",
                declared.name
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "{path} case `{variant}` repeats payload field `{}`",
                declared.name
            ));
        }
        check_value(
            program,
            declared.type_reference,
            payload_value,
            &format!("{path}::{variant}.{}", declared.name),
            active_data,
        )?;
    }
    Ok(())
}

fn expect_value(
    path: &str,
    value: &BuildTimeValue,
    expected: &str,
    predicate: impl FnOnce(&BuildTimeValue) -> bool,
) -> Result<(), String> {
    if predicate(value) {
        Ok(())
    } else {
        Err(format!("{path} expected {expected}, found {}", kind(value)))
    }
}

fn kind(value: &BuildTimeValue) -> &'static str {
    match value {
        BuildTimeValue::Unit => "Unit",
        BuildTimeValue::Int(_) => "integer",
        BuildTimeValue::Bool(_) => "Bool",
        BuildTimeValue::Float(_) => "float",
        BuildTimeValue::Text(_) => "Text",
        BuildTimeValue::Struct { .. } => "record",
        BuildTimeValue::Case { .. } => "sum case",
        BuildTimeValue::Array(_) => "array",
    }
}

#[cfg(test)]
mod tests {
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    use super::{BuildTimeValue, require_const_evaluable_result};

    const SOURCE: &str = r#"
        data Packet [copy] { code: u8; valid: bool; }
        data Outcome [copy] { case Ready(code: u8); case Empty; }
        data AffinePacket [copy] { borrowed: &u8; }

        machine array_value() -> [u8; 2] { "\x01\x02" }
        machine packet_value() -> Packet {
            Packet { code: 1, valid: true }
        }
        machine outcome_value() -> Outcome {
            (Outcome::Ready { code: 1 })
        }
        machine affine_value(value: &u8) -> AffinePacket {
            AffinePacket { borrowed: value }
        }
    "#;

    #[test]
    fn malformed_snapshots_reject_without_panicking() {
        let typed = typed(SOURCE);

        let array_error = reject(
            &typed,
            "array_value",
            BuildTimeValue::Array(vec![BuildTimeValue::Int(1)]),
        );
        assert!(
            array_error.contains("expected 2 array element(s), found 1"),
            "{array_error}"
        );

        let record_error = reject(
            &typed,
            "packet_value",
            BuildTimeValue::Struct {
                type_name: "Packet".to_owned(),
                fields: vec![
                    ("wrong".to_owned(), BuildTimeValue::Int(1)),
                    ("valid".to_owned(), BuildTimeValue::Bool(true)),
                ],
            },
        );
        assert!(
            record_error.contains("missing declared field `code`"),
            "{record_error}"
        );

        let case_error = reject(
            &typed,
            "outcome_value",
            BuildTimeValue::Case {
                variant: "Tampered".to_owned(),
                payload: vec![],
            },
        );
        assert!(
            case_error.contains("unknown case `Tampered`"),
            "{case_error}"
        );

        let text_error = reject(&typed, "array_value", BuildTimeValue::Text(vec![1, 2]));
        assert!(text_error.contains("contains Text"), "{text_error}");

        let affine_error = reject(
            &typed,
            "affine_value",
            BuildTimeValue::Struct {
                type_name: "AffinePacket".to_owned(),
                fields: vec![("borrowed".to_owned(), BuildTimeValue::Int(1))],
            },
        );
        assert!(
            affine_error.contains("has reference type"),
            "{affine_error}"
        );
    }

    fn reject(
        typed: &typed_trees::TypedTrees,
        machine_name: &str,
        value: BuildTimeValue,
    ) -> String {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("machine");
        require_const_evaluable_result(typed, machine, &value)
            .expect_err("the malformed or ineligible value must reject")
    }

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }
}
