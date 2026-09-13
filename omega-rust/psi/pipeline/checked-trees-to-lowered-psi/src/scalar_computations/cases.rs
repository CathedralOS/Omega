//! Case construction keeps its structural place outside the scalar prefix.
//! Fields finish in authored order; observation precedes the temporary's
//! normal-edge disposal, including when only one selective path constructs it.

use super::*;
use checked_trees::data::DataMember;
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
    CheckedScalarComputationStructuralArgument,
};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};

pub(crate) mod source;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: checked_trees::expression::ExpressionHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) case: StructuralCaseId,
    cases: Vec<(symbols::SymbolHandle, StructuralCaseId)>,
    pub(super) fields: Vec<(symbols::SymbolHandle, StructuralFieldId, ScalarType)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Construction {
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) case: StructuralCaseId,
    pub(super) fields: Vec<(StructuralFieldId, LoweredDirectExpression)>,
}

pub(crate) fn fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    checked
        .facts
        .values
        .scalar_computations
        .case_fields
        .span(subject.fields)
        .ok_or(LoweringError::Unsupported(
            "computed case has a stale field span",
        ))
}

pub(crate) fn operand_fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarComputationStructuralArgument,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => fields(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(&[]),
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            unsupported("case membership cannot observe an array")
        }
    }
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        for subject in constructions(checked, &plans.nodes.get(handle).kind)? {
            source::construction(checked, subject)?;
            if slots
                .iter()
                .any(|slot| slot.expression == subject.expression)
            {
                continue;
            }
            let source = validation::scalar_case_constructor(&checked.typed, subject.expression)
                .ok_or(LoweringError::Unsupported(
                    "computed case lost its constructor",
                ))?;
            slots.push(reserve(
                checked,
                subject.expression,
                &source,
                types,
                place_id(allocate_dense(next_place)?),
            )?);
        }
    }
    Ok(slots)
}

/// Resolve one exact constructor against the shared structural namespace.
/// The caller owns allocation and the result's lifetime.
pub(crate) fn reserve(
    checked: &CheckedTrees,
    expression: checked_trees::expression::ExpressionHandle,
    source: &validation::ScalarCaseConstructor,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
) -> Result<Slot, LoweringError> {
    let identity = checked.normalized_type_identity(source.type_reference);
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == identity.as_str())
        .ok_or(LoweringError::Unsupported(
            "computed case has no structural type namespace",
        ))?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("computed case requires a plain sum");
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(source.type_reference)
    else {
        return unsupported("computed case has no nominal owner");
    };
    let owner = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
        .ok_or(LoweringError::Unsupported("computed case owner is absent"))?;
    let mut case_bindings = Vec::new();
    let mut selected = None;
    for member in checked.data_members(owner) {
        let DataMember::Variant(variant) = member else {
            continue;
        };
        let identity = variant
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| variant.name.as_str().to_owned());
        let case = cases.iter().find(|case| case.identity == identity).ok_or(
            LoweringError::Unsupported("computed case declaration is absent"),
        )?;
        case_bindings.push((variant.symbol, case.id));
        if variant.symbol == source.case {
            selected = Some((variant, case));
        }
    }
    let (variant, case) = selected.ok_or(LoweringError::Unsupported(
        "computed case selected a foreign owner",
    ))?;
    let authored_fields = checked.data_payload_fields(variant);
    let retained = &source.fields;
    if retained.len() != case.fields.len() || retained.len() != authored_fields.len() {
        return unsupported("computed case field namespace disagrees");
    }
    let mut field_bindings = Vec::new();
    for field in retained {
        let authored = authored_fields
            .iter()
            .find(|candidate| candidate.symbol == field.0)
            .ok_or(LoweringError::Unsupported(
                "computed case field has a foreign owner",
            ))?;
        let identity = authored
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| authored.name.as_str().to_owned());
        let declaration = case
            .fields
            .iter()
            .find(|candidate| candidate.identity == identity)
            .ok_or(LoweringError::Unsupported(
                "computed case field declaration is absent",
            ))?;
        let primitive = checked
            .primitive_type_reference(authored.type_reference)
            .ok_or(LoweringError::Unsupported(
                "computed case field is not scalar",
            ))?;
        let scalar_type = terminal_scalar_type(primitive)?;
        if !matches!(
            declaration.field_type,
            StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
        ) || declaration.field_type.scalar_type() != Some(scalar_type)
            || declaration.relevance != language_core::BindingRelevance::Relevant
        {
            return unsupported("computed case field requires its exact plain scalar type");
        }
        field_bindings.push((field.0, declaration.id, scalar_type));
    }
    let multiplicity = match checked.type_multiplicity(source.type_reference) {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => {
            return unsupported("computed case cannot discard linear custody");
        }
    };
    Ok(Slot {
        expression,
        place,
        structural_type: declaration.id,
        multiplicity,
        case: case.id,
        cases: case_bindings,
        fields: field_bindings,
    })
}

impl Slot {
    pub(crate) fn construction(
        &self,
        values: &[ValueDeclaration],
    ) -> Result<Construction, LoweringError> {
        if values.len() != self.fields.len() {
            return unsupported("case construction lost its completed field roster");
        }
        let fields = self
            .fields
            .iter()
            .zip(values)
            .enumerate()
            .map(|(ordinal, ((_, field, scalar_type), value))| {
                if value.scalar_type != *scalar_type || !value.qualifications.is_empty() {
                    return unsupported("case construction changed its completed field type");
                }
                Ok((*field, parameter(ordinal, (*scalar_type).into())))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        Ok(Construction {
            place: self.place,
            structural_type: self.structural_type,
            multiplicity: self.multiplicity,
            case: self.case,
            fields,
        })
    }
}

/// Namespace discovery follows retained computation roots. Source replay and
/// establishment validation remain separate from catalog selection.
pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<String>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut types = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        for subject in constructions(checked, &plans.nodes.get(handle).kind)? {
            let identity = checked
                .normalized_type_identity(subject.type_reference)
                .into_string();
            if !types.contains(&identity) {
                types.push(identity);
            }
        }
    }
    Ok(types)
}

/// Constructors share one namespace whether observed locally or passed owned.
fn constructions<'a>(
    checked: &'a CheckedTrees,
    node: &'a CheckedScalarComputationKind,
) -> Result<Vec<&'a CheckedScalarCaseConstruction>, LoweringError> {
    match node {
        CheckedScalarComputationKind::CaseMembership {
            subject: CheckedScalarComputationStructuralArgument::Case(subject),
            ..
        } => Ok(vec![subject]),
        CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } => Ok(checked
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .span(*structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed case call has stale structural arguments",
            ))?
            .iter()
            .filter_map(|argument| match argument {
                CheckedScalarComputationStructuralArgument::Case(subject) => Some(subject),
                _ => None,
            })
            .collect()),
        _ => Ok(Vec::new()),
    }
}

impl Expansion<'_> {
    pub(super) fn case_membership(
        &mut self,
        subject: &CheckedScalarComputationStructuralArgument,
        observed_case: symbols::SymbolHandle,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let subject = match subject {
            CheckedScalarComputationStructuralArgument::Case(subject) => subject,
            CheckedScalarComputationStructuralArgument::Place(argument) => {
                let (place, case) = site
                    .bindings
                    .structural_local_observation(argument, observed_case)?;
                return Ok(self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                        expression: Box::new(
                            LoweredBooleanReturnExpression::StructuralCaseMembership {
                                source: place,
                                case,
                            },
                        ),
                    }),
                ));
            }
            CheckedScalarComputationStructuralArgument::Array { .. } => {
                return unsupported("case membership cannot observe an array");
            }
        };
        let slot = self
            .cases
            .iter()
            .find(|slot| slot.expression == subject.expression)
            .ok_or(LoweringError::Unsupported(
                "computed case has no reserved structural result",
            ))?
            .clone();
        let case = slot
            .cases
            .iter()
            .find(|(symbol, _)| *symbol == observed_case)
            .map(|(_, case)| *case)
            .ok_or(LoweringError::Unsupported(
                "computed membership selected a foreign case",
            ))?;
        let retained = fields(self.checked, subject)?;
        if retained.len() != slot.fields.len() {
            return unsupported("computed case field roster changed after reservation");
        }
        let mut operands = Vec::new();
        let mut completed_types = input_types.to_vec();
        let mut completed_fields = Vec::new();
        for (field, (symbol, identity, scalar_type)) in retained.iter().zip(&slot.fields) {
            let operand = Argument::Computation(field.value);
            if field.symbol != *symbol
                || self.argument_type(&operand, site, input_types)? != (*scalar_type).into()
            {
                return unsupported("computed case field differs from its reserved scalar operand");
            }
            completed_fields.push((
                *identity,
                parameter(completed_types.len(), (*scalar_type).into()),
            ));
            completed_types.push((*scalar_type).into());
            operands.push(operand);
        }
        let mut outgoing = parameters(input_types);
        outgoing.push(parameter(completed_types.len(), ScalarType::Boolean.into()));
        let observation = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            bindings: vec![LoweredScalarBinding::Expression(
                LoweredDirectExpression::Boolean {
                    expression: Box::new(
                        LoweredBooleanReturnExpression::StructuralCaseMembership {
                            source: slot.place,
                            case,
                        },
                    ),
                },
            )],
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: outgoing,
                structural_arguments: Vec::new(),
                trivial_affine_discards: if slot.multiplicity == StructuralMultiplicity::Affine {
                    vec![slot.place]
                } else {
                    Vec::new()
                },
            },
        });
        let constructor = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::EstablishScalarCase(Construction {
                place: slot.place,
                structural_type: slot.structural_type,
                multiplicity: slot.multiplicity,
                case: slot.case,
                fields: completed_fields,
            })],
            terminator: LoweredScalarBranchTerminator::Jump {
                target: observation,
                arguments: parameters(&completed_types),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
        self.sequence(&operands, input_types, constructor, site, active)
    }
}

pub(crate) fn emit(
    effect: &Construction,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut fields = Vec::new();
    for (field, expression) in &effect.fields {
        validate_direct_parameter_types(expression, &types)?;
        if !matches!(expression, LoweredDirectExpression::Parameter { .. }) {
            return unsupported("case construction requires completed scalar fields");
        }
        fields.push(terminal_psi::ScalarCaseField {
            field: *field,
            value: emit_direct_expression(expression, values, next_value, operations),
            range_obligation: None,
        });
    }
    // Canonical publication orders identities only after every expression has
    // completed; it never changes authored evaluation order.
    fields.sort_by_key(|field| field.field);
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Structural(StructuralOperationResult {
            place: effect.place,
            structural_type: effect.structural_type,
            multiplicity: effect.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: effect.case,
            fields,
        },
    });
    Ok(())
}

pub(crate) fn declarations(
    operations: &[Operation],
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(operation.kind, OperationKind::EstablishScalarCase { .. }) {
            return None;
        }
        let OperationResult::Structural(result) = &operation.result else {
            return None;
        };
        Some(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation.id,
                structural_type: result.structural_type,
            },
        })
    })
}
