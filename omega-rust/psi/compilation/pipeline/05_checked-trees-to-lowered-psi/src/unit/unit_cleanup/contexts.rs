//! Nominal cleanup uses the ordinary callable contract namespace. Prepare a
//! hook's subject before lowering requires, then independently rejoin the
//! checked cleanup roster to those authored clauses. No requirement is inferred
//! from a caller's need to dispose of a value.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralTypeShape, LoweringError, PlaceId, PrimitiveType, Proposition, ScalarTerm,
    ScalarType, StructuralFieldType, StructuralTypeShape, SymbolHandle, unsupported,
};
use terminal_psi::{StructuralParameterDeclaration, StructuralTypeDeclaration};

pub(crate) struct SignatureContext {
    pub(crate) receiver: Option<PlaceId>,
    requires: Vec<Proposition>,
}

impl SignatureContext {
    pub(crate) fn validate_contract(&self, requires: &[Proposition]) -> Result<(), LoweringError> {
        if self.requires != requires {
            return unsupported(
                "nominal cleanup requirements differ from the authored callable contract",
            );
        }
        Ok(())
    }
}

/// Bounded nominal plans dispose their complete caller roster on every exit.
/// They cannot turn a contextual implication into an unconditional field fact.
pub(crate) fn validate_unconditional_contextual_requirements(
    checked: &CheckedTrees,
    source: SymbolHandle,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .contextual_cleanup
        .entry_requirements
        .rows
        .iter()
        .any(|(_, row)| {
            row.machine == source
                && row.antecedent != checked_trees::CheckedBooleanExpression::Constant(true)
        })
    {
        return unsupported("guarded contextual cleanup requires an exact edge cleanup plan");
    }
    Ok(())
}

pub(super) fn validate_requirements(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<(), LoweringError> {
    validate_unconditional_contextual_requirements(checked, nominal.machine.machine)?;
    let field = |position: u32, identity: &str| {
        let parameter = nominal
            .machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.position == position)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal cleanup caller requirement root is absent",
            ))?;
        let shape = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .structural_types
            .iter()
            .find(|shape| shape.identity == parameter.type_identity)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal cleanup receiver shape is absent",
            ))?;
        let valid = matches!(&shape.shape, CheckedUnitStructuralTypeShape::Record { fields }
            if fields.iter().any(|field| field.identity == identity
                && !field.relevance.is_erased()
                && field.field_type == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::Bool)));
        if !valid {
            return unsupported(
                "contextual nominal cleanup requirement field is absent, erased, or non-Boolean",
            );
        }
        Ok(())
    };
    for (index, requirement) in nominal.caller_requirements.iter().enumerate() {
        field(
            requirement.source_parameter_index,
            &requirement.field_identity,
        )?;
        if nominal.caller_requirements[..index].contains(requirement) {
            return unsupported("contextual nominal cleanup caller requirements are duplicated");
        }
    }
    for cleanup in &nominal.cleanups {
        let parameter_index =
            cleanup
                .source_parameter_index()
                .ok_or(LoweringError::Unsupported(
                    "nominal parameter cleanup acquired another source",
                ))?;
        let source_position = nominal
            .machine
            .structural_parameters
            .get(parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup parameter is absent",
            ))?
            .position;
        for (index, requirement) in cleanup.requirements.iter().enumerate() {
            field(source_position, &requirement.field_identity)?;
            if cleanup.requirements[..index].contains(requirement)
                || !nominal.caller_requirements.iter().any(|caller| {
                    caller.source_parameter_index == source_position
                        && caller.field_identity == requirement.field_identity
                        && caller.expected == requirement.expected
                })
            {
                return unsupported(
                    "contextual nominal cleanup requirements are duplicated or lack a caller premise",
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn field_clause(
    types: &[StructuralTypeDeclaration],
    parameter: &StructuralParameterDeclaration,
    identity: &str,
    expected: bool,
) -> Result<Proposition, LoweringError> {
    let field = types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().find(|field| {
                field.identity == identity
                    && !field.relevance.is_erased()
                    && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
            }),
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "contextual nominal cleanup terminal field identity drifted",
        ))?;
    Ok(Proposition::Equal(
        ScalarTerm::boolean(expected),
        ScalarTerm::boolean_field(parameter.place, field.id),
    ))
}

pub(crate) fn prepare_signature(
    checked: &CheckedTrees,
    source: SymbolHandle,
    predicate_parameters: &mut Vec<StructuralParameterDeclaration>,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Option<SignatureContext>, LoweringError> {
    let plans = &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines;
    let mut owners = plans
        .iter()
        .filter(|nominal| nominal.machine.machine == source);
    let nominal = owners.next();
    if owners.next().is_some() {
        return unsupported("nominal cleanup callable has duplicate checked plans");
    }
    let mut requires = Vec::new();
    if let Some(nominal) = nominal {
        validate_requirements(checked, nominal)?;
        for requirement in &nominal.caller_requirements {
            let checked_parameter = nominal
                .machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == requirement.source_parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup caller parameter is absent",
                ))?;
            let parameter = predicate_parameters
                .iter()
                .find(|parameter| parameter.position == checked_parameter.position)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup caller predicate parameter is absent",
                ))?;
            requires.push(field_clause(
                types,
                parameter,
                &requirement.field_identity,
                requirement.expected,
            )?);
        }
    }
    let mut hooks =
        super::selected_cleanups(checked).filter(|cleanup| cleanup.cleanup_machine == source);
    let hook = hooks.next();
    let mut receiver = None;
    if let Some(hook) = hook {
        if nominal.is_some() {
            return unsupported("nominal cleanup dispatcher is its own hook");
        }
        super::unique_unit_machine(
            crate::unit::attached_unit::bodies::UnitPlans::published(
                &checked.facts.flow.terminal_unit_effects,
            ),
            source,
        )?;
        let structural_type = types
            .iter()
            .find(|declaration| declaration.identity == hook.type_identity)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup hook owner type is absent",
            ))?
            .id;
        if predicate_parameters.is_empty() && !hook.requirements.is_empty() {
            // Reserve through the closure allocator, but publish no executable
            // parameter/place: this receiver exists only to substitute clauses
            // at each consuming cleanup action.
            predicate_parameters.push(StructuralParameterDeclaration {
                place: crate::terminal_identities::place_id(
                    crate::terminal_identities::allocate_dense(next_place)?,
                ),
                position: 0,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        }
        if let Some(parameter) = predicate_parameters.first() {
            receiver = Some(parameter.place);
            for requirement in &hook.requirements {
                requires.push(field_clause(
                    types,
                    parameter,
                    &requirement.field_identity,
                    requirement.expected,
                )?);
            }
        }
    }
    if nominal.is_none() && hook.is_none() {
        return Ok(None);
    }
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut requires)?;
    Ok(Some(SignatureContext { receiver, requires }))
}
