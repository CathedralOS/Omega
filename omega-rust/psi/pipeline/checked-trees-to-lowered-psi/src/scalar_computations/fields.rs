//! Record reads bind a declared field once, then resolve the current local place
//! at each evaluation site. They materialize an ordinary scalar binding before
//! later effects; neither a synthetic parameter nor a delayed return load may
//! stand in for that observation. Source replay separately reconstructs custody.

use super::*;
use checked_trees::data::DataMember;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

pub(crate) struct Binding {
    symbol: symbols::SymbolHandle,
    type_identity: String,
    field: StructuralFieldId,
    scalar_type: ScalarType,
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<Binding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut bindings: Vec<Binding> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let node = plans.nodes.get(handle);
        let CheckedScalarComputationKind::StructuralField { subject, field, .. } = &node.kind
        else {
            continue;
        };
        if bindings.iter().any(|binding| {
            binding.symbol == *field && binding.type_identity == subject.type_identity
        }) {
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
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.identity == subject.type_identity);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "record read has no structural namespace",
        ))?;
        if declarations.next().is_some() {
            return unsupported("record read has ambiguous structural namespaces");
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
            type_identity: subject.type_identity.clone(),
            field: retained.id,
            scalar_type,
        });
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
        || !subject.path.is_empty()
        || subject.type_identity
            != checked
                .normalized_type_identity(source.type_reference)
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
        binding.symbol == field
            && binding.type_identity == subject.type_identity
            && binding.scalar_type == scalar_type
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
                field: binding.field,
            }),
        },
        ScalarType::Integer(_) => LoweredDirectExpression::StructuralField {
            source,
            field: binding.field,
            scalar_type,
        },
        _ => return unsupported("record read requires an integer or Boolean field"),
    })
}
