//! Record reads bind a declared field once, then resolve the current local place
//! at each evaluation site. They materialize an ordinary scalar binding before
//! later effects; neither a synthetic parameter nor a delayed return load may
//! stand in for that observation. Source replay separately reconstructs custody.
//! The same leaf declaration can occur below several children of one root.
//! Bind the complete checked subject to its canonical carrier path, rather than
//! conflating reads by final field identity or projected type alone.

use super::*;
use checked_trees::data::DataMember;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

#[derive(Clone)]
pub(crate) struct Binding {
    symbol: symbols::SymbolHandle,
    subject: CheckedUnitStructuralArgumentPlan,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    field: StructuralFieldId,
    scalar_type: ScalarType,
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<Binding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut bindings: Vec<Binding> = Vec::new();
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
    {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let node = plans.nodes.get(handle);
            let CheckedScalarComputationKind::StructuralField {
                source_expression,
                subject,
                field,
            } = &node.kind
            else {
                continue;
            };
            if bindings
                .iter()
                .any(|binding| binding.symbol == *field && binding.subject == *subject)
            {
                continue;
            }
            let field_symbol = checked.symbols.get(*field);
            if !field.is_valid() || field_symbol.kind != symbols::SymbolKind::Field {
                return unsupported("record read has no resolved field identity");
            }
            let mut owners = checked
                .data_definitions()
                .iter()
                .filter(|owner| owner.symbol == field_symbol.parent);
            let owner = owners.next().ok_or(LoweringError::Unsupported(
                "record read has no declared field owner",
            ))?;
            if owners.next().is_some() {
                return unsupported("record read has ambiguous field owners");
            }
            let authored = checked
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(candidate) if candidate.symbol == *field => Some(candidate),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported("record read field is absent"))?;
            let identity = authored
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| authored.name.as_str().to_owned());
            let source = validation::local_scalar_record_field(
                &checked.typed,
                machine,
                root.state,
                root.statement_ordinal,
                *source_expression,
            )
            .ok_or(LoweringError::Unsupported(
                "record read lost its source declaration path",
            ))?;
            let root_identity = checked.normalized_type_identity(source.type_reference);
            let mut declarations = types
                .iter()
                .filter(|declaration| declaration.identity == root_identity.as_str());
            let mut declaration = declarations.next().ok_or(LoweringError::Unsupported(
                "record read has no structural namespace",
            ))?;
            if declarations.next().is_some() {
                return unsupported("record read has ambiguous structural namespaces");
            }
            let mut path = Vec::new();
            for segment in &subject.path {
                let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment
                else {
                    return unsupported("record read requires declared record field projections");
                };
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return unsupported("record read carrier path left its record namespace");
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let selected = matching.next().ok_or(LoweringError::Unsupported(
                    "record read carrier field is absent",
                ))?;
                let StructuralFieldType::Structural(nested) = selected.field_type else {
                    return unsupported("record read carrier field is not structural");
                };
                if matching.next().is_some() || selected.relevance.is_erased() {
                    return unsupported("record read carrier field is ambiguous or erased");
                }
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    selected.id,
                ));
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == nested)
                    .ok_or(LoweringError::Unsupported(
                        "record read carrier type is absent",
                    ))?;
            }
            if declaration.identity != subject.type_identity {
                return unsupported("record read carrier path changed its declared type");
            }
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("record read requires a record namespace");
            };
            let mut fields = fields.iter().filter(|field| field.identity == identity);
            let retained = fields.next().ok_or(LoweringError::Unsupported(
                "record read has no declared field identity",
            ))?;
            let scalar_type = terminal_scalar_type(node.primitive_type)?;
            if fields.next().is_some()
                || retained.relevance.is_erased()
                || !matches!(
                    retained.field_type,
                    StructuralFieldType::Scalar(_) | StructuralFieldType::BoundedInteger(_)
                )
                || retained.field_type.scalar_type() != Some(scalar_type)
            {
                return unsupported("record read changed its scalar field carrier");
            }
            bindings.push(Binding {
                symbol: *field,
                subject: subject.clone(),
                path,
                field: retained.id,
                scalar_type,
            });
        }
    }
    Ok(bindings)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_source(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: checked_trees::expression::ExpressionHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<(), LoweringError> {
    let source = validation::local_scalar_record_field(
        &checked.typed,
        machine,
        state,
        statement,
        expression,
    )
    .ok_or(LoweringError::Unsupported(
        "record read lost its authored local and field",
    ))?;
    if subject.source
        != (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: source.local,
        })
        || subject.access != CheckedStructuralAccess::SharedBorrow
        || subject.path
            != source
                .path
                .iter()
                .cloned()
                .map(checked_trees::CheckedUnitStructuralPathSegment::Field)
                .collect::<Vec<_>>()
        || subject.type_identity
            != checked
                .normalized_type_identity(source.carrier_type_reference)
                .as_str()
        || source.field != field
        || source.primitive_type != primitive
    {
        return unsupported("record read differs from its authored local and field");
    }
    Ok(())
}

pub(super) fn observation(
    fields: &[Binding],
    bindings: &storage::ScalarBindings,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let scalar_type = terminal_scalar_type(primitive)?;
    let mut matching = fields.iter().filter(|binding| {
        binding.symbol == field && binding.subject == *subject && binding.scalar_type == scalar_type
    });
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "record read lost its field binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("record read has ambiguous field bindings");
    }
    // The ordinary shared-argument join already resolves an exact live local
    // without transferring it. Field replay admits only StructuralLocal sources;
    // use that same current storage mapping instead of a parallel read map.
    let source = bindings.shared_structural_argument(subject)?.place;
    Ok(match scalar_type {
        ScalarType::Boolean => LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                source,
                path: binding.path.clone(),
                field: binding.field,
            }),
        },
        ScalarType::Integer(_) => LoweredDirectExpression::StructuralField {
            source,
            path: binding.path.clone(),
            field: binding.field,
            scalar_type,
        },
        _ => return unsupported("record read requires an integer or Boolean field"),
    })
}
