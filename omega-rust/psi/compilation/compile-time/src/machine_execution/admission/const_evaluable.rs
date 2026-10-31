//! Value-sensitive result admission for compiler-run semantic machines.
//!
//! `ConstEvaluable(T, value)` is deliberately target-neutral: it admits only
//! closed, freely copyable semantic values that can cross the interpreter
//! boundary as an owned snapshot. Layout and byte materialization are later,
//! separate judgments.

use language_semantics::{DataSupplyMode, Multiplicity};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind, TypeParameterKind};
use typed_trees::machine::Machine;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use crate::BuildTimeValue;
use crate::build_time_evaluation::type_reference_table;

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
    active_data: &mut Vec<TypeIdentityId>,
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
        TypeReferenceNode::Named { name, .. } => {
            if name.as_str().starts_with("Atomic") {
                return Err(format!(
                    "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                ));
            }
            if let Some(primitive) = PrimitiveType::from_name(name.as_str()) {
                return check_primitive(path, primitive, value);
            }
            check_data(
                program,
                type_reference,
                name.as_str(),
                value,
                path,
                active_data,
            )
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
        TypeReferenceNode::Generic { base_name, .. } => check_data(
            program,
            type_reference,
            base_name.as_str(),
            value,
            path,
            active_data,
        ),
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
        (
            PrimitiveType::F32,
            BuildTimeValue::UndeterminedNaN(numerics::literals::FloatFormat::F32),
        )
        | (
            PrimitiveType::F64,
            BuildTimeValue::UndeterminedNaN(numerics::literals::FloatFormat::F64),
        ) => Ok(()),
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

/// Consume the producer-retained closed shape of either spelling. A retained
/// generic declaration need not be copied into another nominal declaration for
/// its exact closed application to cross the const boundary.
fn closed_data<'program>(
    program: &'program TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    path: &str,
) -> Result<&'program DataDefinition, String> {
    let definition = program.closed_data_definition(reference).ok_or_else(|| {
        format!("{path} has an open or generic aggregate type and is not a closed const type")
    })?;
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { .. }
    ) && !consistent_nominal_spelling(name, definition.name.as_str())
    {
        return Err(format!(
            "{path} has inconsistent nominal type spelling `{name}` for `{}`",
            definition.name
        ));
    }
    Ok(definition)
}

fn check_data(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<TypeIdentityId>,
) -> Result<(), String> {
    let definition = closed_data(program, reference, name, path)?;
    let identity = program.normalized_type_identity(reference);
    if !identity.is_valid() || active_data.contains(&identity) {
        return Err(format!(
            "{path} reaches recursive or unresolved data `{name}`, which is outside the closed aggregate const boundary"
        ));
    }
    active_data.push(identity);
    let result = (|| {
        require_closed_data_shape(program, definition, path, active_data)?;
        match DataDefinition::shape_kind_from_members(program.data_members(definition)) {
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
        }
    })();
    active_data.pop();
    result
}

fn require_closed_data_shape(
    program: &TypedTrees,
    definition: &DataDefinition,
    path: &str,
    active_data: &mut Vec<TypeIdentityId>,
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
        require_closed_generic_application(program, definition, application, path, active_data)?;
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
/// parameters can never supply a const-evaluable argument. Substituted field
/// shapes and nested argument types share this closed-application judgment.
fn require_closed_generic_application(
    program: &TypedTrees,
    definition: &DataDefinition,
    application: TypeReferenceHandle,
    path: &str,
    active_data: &mut Vec<TypeIdentityId>,
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
    let arguments = type_reference_table(program).type_reference_handles(*arguments);
    if arguments.len() != parameters.len() {
        return Err(format!(
            "{path} has generic aggregate type `{}` supplying {} argument(s) for {} base parameter(s)",
            definition.name,
            arguments.len(),
            parameters.len()
        ));
    }
    for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
        require_closed_argument(
            program,
            &parameter.kind,
            *argument,
            &format!("{path} parameter `{}`", parameter.name),
            active_data,
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
    active_data: &mut Vec<TypeIdentityId>,
) -> Result<(), String> {
    match kind {
        TypeParameterKind::Type => require_closed_type(program, argument, path, active_data),
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
            if type_reference_table(program)
                .anonymous_numeric_literal_value(argument)
                .is_some()
            {
                Ok(())
            } else {
                Err(format!(
                    "{path} has a non-literal const argument and is not a closed const application"
                ))
            }
        }
        TypeParameterKind::Implementation => {
            let operand = program
                .static_operand_from_type_reference(argument)
                .map_err(|reason| {
                    format!("{path} lost its erased implementation operand: {reason}")
                })?;
            validation::conformance_applications::close_conformance_application(program, &operand)
                .map(|_| ())
                .map_err(|reason| {
                    format!(
                        "{path} is not a closed named implementation: {}",
                        reason.message
                    )
                })
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
    active_data: &mut Vec<TypeIdentityId>,
) -> Result<(), String> {
    if !type_reference.is_valid() {
        return Err(format!("{path} has an invalid argument type"));
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Named { name, .. } => {
            if name.as_str().starts_with("Atomic") {
                return Err(format!(
                    "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                ));
            }
            if PrimitiveType::from_name(name.as_str()).is_some() {
                return Ok(());
            }
            require_closed_data_type(program, type_reference, name.as_str(), path, active_data)
        }
        TypeReferenceNode::Generic { base_name, .. } => require_closed_data_type(
            program,
            type_reference,
            base_name.as_str(),
            path,
            active_data,
        ),
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

        TypeReferenceNode::ConstExpression(_) => Err(format!(
            "{path} has a proof-static expression type, not a runtime const value type"
        )),
        TypeReferenceNode::DynamicTrait { .. } => Err(format!("{path} has a dynamic trait type")),
    }
}

fn require_closed_data_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    path: &str,
    active_data: &mut Vec<TypeIdentityId>,
) -> Result<(), String> {
    let definition = closed_data(program, reference, name, path)?;
    let identity = program.normalized_type_identity(reference);
    if !identity.is_valid() || active_data.contains(&identity) {
        return Err(format!(
            "{path} reaches recursive or unresolved data `{name}` through a generic argument"
        ));
    }
    active_data.push(identity);
    let result = (|| {
        require_closed_data_shape(program, definition, path, active_data)?;
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

fn check_record(
    program: &TypedTrees,
    definition: &DataDefinition,
    value: &BuildTimeValue,
    path: &str,
    active_data: &mut Vec<TypeIdentityId>,
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
    active_data: &mut Vec<TypeIdentityId>,
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
        BuildTimeValue::LayoutTarget(_) => "compiler-issued target context",
        BuildTimeValue::Int(_) => "integer",
        BuildTimeValue::Bool(_) => "Bool",
        BuildTimeValue::Float(_) | BuildTimeValue::UndeterminedNaN(_) => "float",
        BuildTimeValue::Text(_) => "Text",
        BuildTimeValue::Struct { .. } => "record",
        BuildTimeValue::Case { .. } => "sum case",
        BuildTimeValue::Array(_) => "array",
    }
}
