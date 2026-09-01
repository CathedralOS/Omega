use crate::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, type_reference_label, type_references_match,
    validate_generic_instance_argument_bounds, validate_type_reference_handle_with_type_parameters,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataMember, DataShapeKind};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_data_field_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        // BUILTIN-NAME SHADOW FENCE (probed 2026-07-16): type lookup searches
        // BuiltinType before Data, so a definition named like a builtin type
        // (`data Int`) compiles while every reference binds to the BUILTIN --
        // the definition is silently orphaned, and contract lemmas typed
        // against the builtin stand down at the polynomial engine as
        // out-of-language, certifying UNVALIDATED (possibly false) claims.
        // Refuse the collision at the declaration.
        if psi_symbols::builtin_type_symbols()
            .iter()
            .any(|(_, name)| name.as_str() == data_definition.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` shadows the builtin type of the same name -- references \
                 would silently bind to the builtin and this definition would never \
                 be used (its contracts would go unvalidated). Pick a different name.",
                data_definition.name
            )));
        }
        let data_members = program.data_members(data_definition);
        let type_parameters = program.data_type_parameters(data_definition);
        validate_generic_instance_argument_bounds(program, data_definition, symbols, diagnostics);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_shape(program, data_definition, data_members, diagnostics);
        validate_data_default_domain(program, data_definition, data_members, diagnostics);

        for member in data_members {
            let payload_fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => {
                    validate_payload_field_names(data_definition, variant, program, diagnostics);
                    program.data_payload_fields(variant)
                }
            };

            for field in payload_fields {
                validate_type_reference_handle_with_type_parameters(
                    program,
                    field.type_reference,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::DataField {
                        data: data_definition.name.as_str(),
                        field: field.name.as_str(),
                        generic_depth: 0,
                    },
                    type_parameters,
                    &data_definition.lifetime_parameters,
                );
            }
        }
    }
}

/// Whether zero-filled storage is already an established value of this type.
/// Representation stays zero-expressible; a range/default domain that excludes
/// zero gates VALUE establishment instead of making the declaration illegal.
/// The check composes through record fields, arrays, and the payload of the
/// zero-tag (first) sum case. Later cases do not inhabit zero storage.
pub(crate) fn type_requires_establishment(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_requires_establishment_inner(program, type_reference, &mut Vec::new())
}

fn validate_data_default_domain(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let facts = program.proof_facts.span_or_empty(definition.where_facts);
    validate_proof_facts(
        program,
        facts,
        diagnostics,
        ProofFactOwner::DataDefaultDomain(definition.name.as_str()),
    );

    for fact in facts {
        let psi_typed_trees::domain::ProofFact::Membership(membership) = fact else {
            continue;
        };
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            continue;
        };
        let psi_typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(membership.value)
        else {
            continue;
        };
        let Some(field_name) = program
            .expression_table
            .name_path_members(path.members)
            .last()
        else {
            continue;
        };
        let Some(field_type) = members.iter().find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name.as_str() => {
                Some(field.type_reference)
            }
            DataMember::Field(_) | DataMember::Variant(_) => None,
        }) else {
            continue;
        };
        let field_carrier =
            crate::places::unwrapped_type_reference(program, field_type).unwrap_or(field_type);
        let domain_carrier = crate::places::unwrapped_type_reference(program, domain.target_type)
            .unwrap_or(domain.target_type);
        if type_references_match(program, field_carrier, domain_carrier) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` default-domain fact puts field `{}` in domain `{}`, but the field carrier is `{}` while the domain classifies `{}`",
            definition.name.as_str(),
            field_name.as_str(),
            domain.name.as_str(),
            type_reference_label(program, field_carrier),
            type_reference_label(program, domain_carrier),
        )));
    }
}

fn type_requires_establishment_inner(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    seen: &mut Vec<String>,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    if let Some(interval) =
        crate::arithmetic_domains::range_constraint_interval(program, type_reference)
        && (interval.low().is_some_and(|low| low > 0)
            || interval.high().is_some_and(|high| high < 0))
    {
        return true;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_requires_establishment_inner(program, *base_type, seen)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_requires_establishment_inner(program, *element_type, seen)
        }
        TypeReferenceNode::Named { name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name.as_str())
            .is_some_and(|definition| data_requires_establishment_inner(program, definition, seen)),
        _ => false,
    }
}

pub fn data_requires_establishment(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
) -> bool {
    data_requires_establishment_inner(program, definition, &mut Vec::new())
}

fn data_requires_establishment_inner(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    seen: &mut Vec<String>,
) -> bool {
    if definition.zero_gated {
        return true;
    }
    let name = definition.name.as_str().to_owned();
    if seen.contains(&name) {
        return false;
    }
    seen.push(name.clone());

    let members = program.data_members(definition);
    let common_gated = members.iter().any(|member| match member {
        DataMember::Field(field) => {
            type_requires_establishment_inner(program, field.type_reference, seen)
        }
        DataMember::Variant(_) => false,
    });
    let zero_case_gated = members
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .is_some_and(|variant| {
            program
                .data_payload_fields(variant)
                .iter()
                .any(|field| type_requires_establishment_inner(program, field.type_reference, seen))
        });

    seen.retain(|candidate| candidate != &name);
    common_gated || zero_case_gated
}

fn validate_payload_field_names(
    data_definition: &psi_typed_trees::data::DataDefinition,
    variant: &psi_typed_trees::data::DataVariant,
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let data_members = program.data_members(data_definition);
    let payload_fields = program.data_payload_fields(variant);
    for (field_index, field) in payload_fields.iter().enumerate() {
        if payload_fields[..field_index]
            .iter()
            .any(|previous| previous.name.as_str() == field.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` has duplicate payload field `{}`",
                data_definition.name, variant.name, field.name
            )));
        }

        // Mixed shapes: a payload field may not reuse a COMMON field's name.
        // Member access (`value.name`) searches common fields first and then
        // every case's payload, so a collision would silently rebind reads.
        if data_members.iter().any(|member| {
            matches!(member, DataMember::Field(common) if common.name.as_str() == field.name.as_str())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` payload field `{}` collides with the common field of the same name",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
    }
}

/// Mixed shapes (common fields + cases) are accepted with two honest-first-cut
/// restrictions, both rejected loudly here:
///
/// - COMMON fields must be scalar primitives (bool / integers / floats).
///   Case construction zero-initializes every common field not named in the
///   literal, and a scalar zero is one storage write in both backends; zeroing
///   nested aggregates / text / slices at construction is deferred.
/// - COMMON fields may not declare default initializers. Construction of a
///   mixed value is always the case-literal form, whose rule is
///   zero-unless-named (ZII keeps that valid); a default would silently not
///   apply, so it is rejected instead of ignored.
///
/// Payload-field/common-field name collisions are rejected separately in
/// `validate_payload_field_names` (member access searches both namespaces).
fn validate_data_shape(
    program: &TypedTrees,
    data_definition: &psi_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match psi_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty | DataShapeKind::Enum | DataShapeKind::Record => {}
        DataShapeKind::Mixed => {
            // Generic declarations describe semantic shape, not a concrete
            // representation. Closed synthesis revalidates each executable
            // instance after substituting its exact field types.
            if !data_definition.type_parameters.is_empty() {
                return;
            }
            for member in data_members {
                let DataMember::Field(field) = member else {
                    continue;
                };
                // Erased common bindings have no runtime zero-initialization
                // or storage requirement; relevance validation separately
                // preserves their semantic obligations.
                if field.relevance.is_erased() {
                    continue;
                }
                let scalar = program
                    .primitive_type_reference(field.type_reference)
                    .is_some();
                if !scalar {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` common field `{}` is not a scalar primitive; mixed data shape common fields support only bool, integer, and float types for now (case construction zero-initializes unnamed common fields)",
                        data_definition.name, field.name
                    )));
                }
            }
        }
    }
}

fn validate_data_member_names(
    data_definition: &psi_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (member_index, member) in data_members.iter().enumerate() {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if data_members[..member_index]
            .iter()
            .any(|previous| data_member_name(previous) == member_name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
    }
}

fn data_member_name(member: &DataMember) -> &str {
    match member {
        DataMember::Field(field) => field.name.as_str(),
        DataMember::Variant(variant) => variant.name.as_str(),
    }
}
