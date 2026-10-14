//! Nominal cleanup uses the ordinary callable contract namespace. Prepare a
//! hook's subject before lowering requires. Its complete contract then lowers
//! once through `entry_requirements`, exactly as for an ordinary call. A cleanup
//! action selects that signature, never a second Boolean-field contract lane.

use super::{
    CheckedTrees, LoweringError, PlaceId, Proposition, ScalarTerm, StructuralTypeShape,
    SymbolHandle, unsupported,
};
use terminal_psi::{
    StructuralParameterDeclaration, StructuralTypeDeclaration, find_structural_type,
};

pub(crate) struct SignatureContext {
    pub(crate) receiver: Option<PlaceId>,
    dispatcher_requires: Option<Vec<Proposition>>,
}

impl SignatureContext {
    pub(crate) fn validate_contract(&self, requires: &[Proposition]) -> Result<(), LoweringError> {
        if self
            .dispatcher_requires
            .as_deref()
            .is_some_and(|expected| expected != requires)
        {
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

pub(crate) fn field_clause(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &[StructuralTypeDeclaration],
    parameter: &StructuralParameterDeclaration,
    identity: &checked_trees::CheckedMemberIdentity,
    expected: bool,
) -> Result<Proposition, LoweringError> {
    // `validate_requirements` admitted a relevant Boolean field of this
    // parameter's record; resolve its identity to the lowered field.
    let field = find_structural_type(types, parameter.structural_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => {
                terminal_psi::field_by_identity(fields, identities.intern(&identity.spelling()))
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "contextual nominal cleanup requirement names an unlowered field",
        ))?;
    Ok(Proposition::Equal(
        ScalarTerm::boolean(expected),
        ScalarTerm::boolean_field(parameter.place, field.id),
    ))
}

pub(crate) fn prepare_signature(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
    let mut owners = plans.iter().filter(|nominal| nominal.machine == source);
    let nominal = owners.next();
    if owners.next().is_some() {
        return unsupported("nominal cleanup callable has duplicate checked plans");
    }
    let mut requires = Vec::new();
    if let Some(nominal) = nominal {
        validate_unconditional_contextual_requirements(checked, source)?;
        for requirement in &nominal.caller_requirements {
            let parameter = predicate_parameters
                .iter()
                .find(|parameter| parameter.position == requirement.source_parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup caller predicate parameter is absent",
                ))?;
            requires.push(field_clause(
                identities,
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
        let contract =
            checked
                .facts
                .contract_plans
                .for_machine(source)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup hook contract is absent",
                ))?;
        if contract
            .closed_scalar_values
            .authored_requires()
            .iter()
            .any(Option::is_none)
        {
            return unsupported(
                "nominal cleanup hook lacks a complete callable prerequisite roster",
            );
        }
        let structural_type = crate::terminal_identities::structural_declaration_by_identity(
            types,
            identities,
            checked,
            hook.type_identity,
            "nominal cleanup hook owner type is absent",
        )?
        .id;
        if predicate_parameters.is_empty() {
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
        }
    }
    if nominal.is_none() && hook.is_none() {
        return Ok(None);
    }
    crate::scalar_lowering::contracts::canonicalize_requires(&mut requires)?;
    Ok(Some(SignatureContext {
        receiver,
        dispatcher_requires: nominal.map(|_| requires),
    }))
}
