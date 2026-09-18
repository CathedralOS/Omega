//! Value-sensitive result admission for compiler-run semantic machines.
//!
//! `ConstEvaluable(T, value)` is deliberately target-neutral: it admits only
//! closed, freely copyable semantic values that can cross the interpreter
//! boundary as an owned snapshot. Layout and byte materialization are later,
//! separate judgments.

use language_semantics::{DataSupplyMode, Multiplicity};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind, TypeParameterKind};
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

/// The `name` recorded on a `Named`/`Generic` reference is the use-site
/// spelling; the symbol already carries the resolved identity. A
/// module-qualified occurrence prefixes the declared name with its owner path
/// (`geom::Point`, `geom::Box<u64>`) while the definition keeps the
/// declaration's own name (`Point`, `Box<u64>`), so consistency requires the
/// spelling to end at the declared name on a `::` boundary. Matching a suffix
/// keeps `::` inside a generic instance's argument spelling part of the
/// declared tail rather than misparsing it as owner path.
fn consistent_nominal_spelling(recorded: &str, declared: &str) -> bool {
    recorded == declared
        || recorded
            .strip_suffix(declared)
            .is_some_and(|prefix| prefix.ends_with("::"))
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
    if !consistent_nominal_spelling(name, definition.name.as_str()) {
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
    {
        return Err(format!(
            "{path} has open or generic aggregate type `{}`",
            definition.name
        ));
    }
    // A synthesized concrete generic instance is a closed nominal aggregate:
    // its members were substituted during synthesis. Its retained application
    // origin is admitted only when that origin itself is closed — erased
    // borrow-region arguments absent, the exact open template as base, and
    // every argument a closed const atom or closed type. Anything else means
    // the value's identity still mentions an open construct.
    if let Some(application) = definition.generic_instance {
        require_closed_generic_application(program, definition, application, path)?;
    }
    if definition.quotient.is_some() {
        return Err(format!(
            "{path} has quotient type `{}`; quotient representatives require their separate checked admission",
            definition.name
        ));
    }
    Ok(())
}

/// The retained structural origin of a synthesized generic instance. Only a
/// CLOSED application may cross the const boundary: the base must resolve to
/// the open generic template, lifetime arguments must already be erased, and
/// each argument must satisfy its parameter kind — a closed const literal for
/// `const`/`value` parameters, a closed type for `type` parameters. Machine
/// parameters can never supply a const-evaluable argument.
fn require_closed_generic_application(
    program: &TypedTrees,
    definition: &DataDefinition,
    application: TypeReferenceHandle,
    path: &str,
) -> Result<(), String> {
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        lifetime_arguments,
        arguments,
    } = program.type_reference_table.type_reference(application)
    else {
        return Err(format!(
            "{path} has generic aggregate type `{}` whose origin is not a resolved generic application",
            definition.name
        ));
    };
    if !lifetime_arguments.is_empty() {
        return Err(format!(
            "{path} has generic aggregate type `{}` with non-erased lifetime arguments",
            definition.name
        ));
    }
    if !base_symbol.is_valid() {
        return Err(format!(
            "{path} has generic aggregate type `{base_name}` without an exact nominal base identity"
        ));
    }
    let mut bases = program
        .data_definitions()
        .iter()
        .filter(|candidate| candidate.symbol == *base_symbol);
    let base = bases.next().ok_or_else(|| {
        format!("{path} has generic aggregate type `{base_name}` with an unknown base data type")
    })?;
    if bases.next().is_some() {
        return Err(format!(
            "{path} has ambiguous nominal base identity for `{base_name}`"
        ));
    }
    if !consistent_nominal_spelling(base_name.as_str(), base.name.as_str())
        || base.generic_instance.is_some()
    {
        return Err(format!(
            "{path} has generic aggregate type `{}` whose base `{base_name}` is not the open generic template",
            definition.name
        ));
    }
    let parameters = program.data_type_parameters(base);
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    if arguments.len() != parameters.len() {
        return Err(format!(
            "{path} has generic aggregate type `{}` supplying {} argument(s) for {} base parameter(s)",
            definition.name,
            arguments.len(),
            parameters.len()
        ));
    }
    let mut active_data = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
        require_closed_argument(
            program,
            &parameter.kind,
            *argument,
            &format!("{path} parameter `{}`", parameter.name),
            &mut active_data,
        )?;
    }
    Ok(())
}

/// One generic argument judged against its parameter kind. `const`/`value`
/// parameters admit only a compile-known resolved integer literal; `type`
/// parameters admit only a closed const-boundary type; machine parameters
/// admit nothing at this boundary.
fn require_closed_argument(
    program: &TypedTrees,
    kind: &TypeParameterKind,
    argument: TypeReferenceHandle,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    match kind {
        TypeParameterKind::Type => require_closed_type(program, argument, path, active_data),
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
            match program.type_reference_table.type_reference(argument) {
                TypeReferenceNode::Named { symbol, name }
                    if !symbol.is_valid()
                        && !name.as_str().is_empty()
                        && name.as_str().chars().all(|c| c.is_ascii_digit()) =>
                {
                    Ok(())
                }
                _ => Err(format!(
                    "{path} has a non-literal const argument and is not a closed const application"
                )),
            }
        }
        TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => Err(format!(
            "{path} is not a data parameter; only closed const atoms and closed types can cross the const boundary"
        )),
    }
}

/// Type-level closedness for a generic argument in `type` position: a
/// primitive, unit, a literal-length fixed array of closed elements, a
/// constrained closed base, or a closed nominal aggregate (itself possibly a
/// closed generic instance). References, slices, open generics, proof-static
/// expressions, dynamic traits, and interior-mutable types are not closed.
fn require_closed_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    path: &str,
    active_data: &mut Vec<SymbolHandle>,
) -> Result<(), String> {
    if !type_reference.is_valid() {
        return Err(format!("{path} has an invalid argument type"));
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Named { symbol, name } => {
            if name.as_str().starts_with("Atomic") {
                return Err(format!(
                    "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                ));
            }
            if PrimitiveType::from_name(name.as_str()).is_some() {
                return Ok(());
            }
            if !symbol.is_valid() {
                return Err(format!(
                    "{path} names `{name}` without an exact nominal type identity"
                ));
            }
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|candidate| candidate.symbol == *symbol);
            let definition = definitions
                .next()
                .ok_or_else(|| format!("{path} names unknown data type `{name}`"))?;
            if definitions.next().is_some() {
                return Err(format!(
                    "{path} has ambiguous nominal type identity for `{name}`"
                ));
            }
            if !consistent_nominal_spelling(name.as_str(), definition.name.as_str()) {
                return Err(format!(
                    "{path} has inconsistent nominal type spelling `{name}` for `{}`",
                    definition.name
                ));
            }
            require_closed_data_shape(program, definition, path)?;
            if active_data.contains(symbol) {
                return Err(format!(
                    "{path} reaches recursive data `{name}` through a generic argument"
                ));
            }
            active_data.push(*symbol);
            let result = (|| {
                for member in program.data_members(definition) {
                    match member {
                        DataMember::Field(field) => require_closed_type(
                            program,
                            field.type_reference,
                            &format!("{path}.{}", field.name),
                            active_data,
                        )?,
                        DataMember::Variant(variant) => {
                            for field in program.data_payload_fields(variant) {
                                require_closed_type(
                                    program,
                                    field.type_reference,
                                    &format!("{path}::{}.{}", variant.name, field.name),
                                    active_data,
                                )?;
                            }
                        }
                    }
                }
                Ok(())
            })();
            active_data.pop();
            result
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let FixedArrayLength::Literal(_) = length else {
                return Err(format!(
                    "{path} has a non-literal array length and is not a closed const type"
                ));
            };
            require_closed_type(program, *element_type, &format!("{path}[]"), active_data)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            require_closed_type(program, *base_type, path, active_data)
        }
        TypeReferenceNode::Reference { .. } => Err(format!("{path} has reference type")),
        TypeReferenceNode::Slice { .. } => Err(format!("{path} has slice type")),
        TypeReferenceNode::Generic { .. } => Err(format!(
            "{path} has an open or generic aggregate type and is not a closed const type"
        )),
        TypeReferenceNode::ConstExpression(_) => Err(format!(
            "{path} has a proof-static expression type, not a runtime const value type"
        )),
        TypeReferenceNode::DynamicTrait { .. } => Err(format!("{path} has a dynamic trait type")),
    }
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
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
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

    /// EVALUATED-FOREIGN-BINDINGS: a synthesized const-generic instance is a
    /// closed nominal aggregate, but the snapshot must still match its exact
    /// nominal name and substituted member types.
    #[test]
    fn closed_generic_instance_snapshots_are_value_sensitive() {
        let typed = typed_normalized(
            r#"
            data Holder<T> [copy] { slot: T; }
            machine holder_value() -> Holder<[u8; 2]> {
                Holder { slot: "\x01\x02" }
            }
            "#,
        );

        // The value is well-formed and the instance admits it.
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "holder_value")
            .expect("machine");
        require_const_evaluable_result(
            &typed,
            machine,
            &BuildTimeValue::Struct {
                type_name: "Holder<[u8; 2]>".to_owned(),
                fields: vec![(
                    "slot".to_owned(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Int(1), BuildTimeValue::Int(2)]),
                )],
            },
        )
        .expect("a closed `Holder<[u8; 2]>` snapshot is ConstEvaluable");

        // The template's authored spelling is not the instance identity.
        let name_error = reject(
            &typed,
            "holder_value",
            BuildTimeValue::Struct {
                type_name: "Holder".to_owned(),
                fields: vec![(
                    "slot".to_owned(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Int(1), BuildTimeValue::Int(2)]),
                )],
            },
        );
        assert!(
            name_error.contains("expected record `Holder<[u8; 2]>`"),
            "{name_error}"
        );

        // Substituted member types are enforced exactly.
        let member_error = reject(
            &typed,
            "holder_value",
            BuildTimeValue::Struct {
                type_name: "Holder<[u8; 2]>".to_owned(),
                fields: vec![("slot".to_owned(), BuildTimeValue::Int(1))],
            },
        );
        assert!(
            member_error.contains("expected an array value"),
            "{member_error}"
        );

        // A dynamically sized snapshot cannot cross even inside an instance.
        let text_error = reject(
            &typed,
            "holder_value",
            BuildTimeValue::Struct {
                type_name: "Holder<[u8; 2]>".to_owned(),
                fields: vec![("slot".to_owned(), BuildTimeValue::Text(vec![1, 2]))],
            },
        );
        assert!(text_error.contains("contains Text"), "{text_error}");
    }

    /// A `Named` reference records the use-site spelling while its symbol
    /// carries the resolved identity. Admission accepts a module-owner
    /// qualified spelling of the selected declaration and still rejects a
    /// spelling whose leaf names a different declaration.
    #[test]
    fn nominal_spelling_uses_selected_home_identity() {
        let value = || BuildTimeValue::Struct {
            type_name: "Packet".to_owned(),
            fields: vec![
                ("code".to_owned(), BuildTimeValue::Int(1)),
                ("valid".to_owned(), BuildTimeValue::Bool(true)),
            ],
        };
        let mut typed = typed(SOURCE);
        let return_type = {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "packet_value")
                .expect("machine");
            typed.machine_states(machine)[0].return_type
        };
        let symbol = match typed.type_reference_table.type_reference(return_type) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
            _ => panic!("packet result is nominal"),
        };

        // The same selected symbol under a module-owner spelling admits.
        typed.type_reference_table.substitute_node(
            return_type,
            typed_trees::types::TypeReferenceNode::Named {
                symbol,
                name: typed_trees::name::Identifier::generated("geom::Packet"),
            },
        );
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "packet_value")
            .expect("machine");
        require_const_evaluable_result(&typed, machine, &value())
            .expect("an owner-qualified spelling of the selected carrier admits");

        // A leaf that is not the selected declaration's name still rejects.
        typed.type_reference_table.substitute_node(
            return_type,
            typed_trees::types::TypeReferenceNode::Named {
                symbol,
                name: typed_trees::name::Identifier::generated("geom::Parcel"),
            },
        );
        let error = reject(&typed, "packet_value", value());
        assert!(
            error.contains("inconsistent nominal type spelling `geom::Parcel`"),
            "{error}"
        );
    }

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
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn typed_normalized(source: &str) -> typed_trees::TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
        )
        .expect("synthesize closed generic instances");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }
}
