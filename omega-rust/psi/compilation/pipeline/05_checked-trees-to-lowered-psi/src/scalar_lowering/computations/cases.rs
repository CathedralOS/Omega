//! Case construction keeps its structural place outside the scalar prefix.
//! Fields finish in authored order; observation precedes the temporary's
//! normal-edge disposal, including when only one selective path constructs it.
//! Completed scalar operands initialize the same owned case storage as nested
//! payloads. A carrier word is an initializer, not an immutable replacement for
//! a cell: atomic payloads must retain their backing through subsequent loans.
use super::super::{
    Multiplicity, Operation, OperationKind, OperationResult, PlaceId, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, ValueDeclaration, allocate_dense,
    emit_direct_expression, place_id, terminal_scalar_type,
};
use super::{
    Argument, CheckedScalarComputationKind, CheckedTrees, Computation, Expansion, LoweringError,
    QualifiedScalarType, ScalarType, Site, parameter, parameters, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::place_declarations::operation_result_declaration;
use crate::expression_preparation::computation_graph::fields;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
};
use checked_trees::data::DataMember;
use checked_trees::{CheckedScalarCaseConstruction, CheckedScalarComputationStructuralArgument};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};

mod observed_values;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: checked_trees::expression::ExpressionHandle,
    pub(super) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: StructuralMultiplicity,
    pub(crate) case: StructuralCaseId,
    cases: Vec<(symbols::SymbolHandle, StructuralCaseId)>,
    pub(crate) fields: Vec<ReservedField>,
}

/// One payload field resolved against its structural declaration before any
/// operand is evaluated.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReservedField {
    pub(super) field: StructuralFieldId,
    pub(crate) scalar_type: ScalarType,
    /// The declaration restricts the carrier to a range, so establishment must
    /// request a proof that the completed operand lies inside it.
    bounded: bool,
}

impl ReservedField {
    pub(crate) fn completed(self, value: LoweredDirectExpression) -> ConstructedField {
        ConstructedField {
            field: self.field,
            value,
            bounded: self.bounded,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstructedField {
    field: StructuralFieldId,
    value: LoweredDirectExpression,
    bounded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Construction {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: StructuralMultiplicity,
    pub(crate) case: StructuralCaseId,
    pub(crate) fields: Vec<ConstructedField>,
    pub(crate) claims: Vec<crate::emission::claim_bindings::PreparedClaim>,
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_in_machines(machines);
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        for subject in constructions(checked, &plans.nodes.get(handle).kind)? {
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
                identities,
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    expression: checked_trees::expression::ExpressionHandle,
    source: &validation::ScalarCaseConstructor,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
) -> Result<Slot, LoweringError> {
    let identity = checked.normalized_type_identity(source.type_reference);
    let declaration = crate::terminal_identities::structural_declaration_by_identity(
        types,
        identities,
        checked,
        identity,
        "computed case has no structural type namespace",
    )?;
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
        .data_definition_by_symbol(*symbol)
        .ok_or(LoweringError::Unsupported("computed case owner is absent"))?;
    let mut case_bindings = Vec::new();
    let mut selected = None;
    for member in checked.data_members(owner) {
        let DataMember::Variant(variant) = member else {
            continue;
        };
        // The checked shape carries only the cases this instance can hold;
        // a refuted case has no tag to bind (`validation::impossible_variant`).
        if validation::impossible_variant(checked, variant) {
            continue;
        }
        let identity =
            checked_trees::CheckedMemberIdentity::of_member(variant.identity, variant.name.clone());
        let case = terminal_psi::structural_case_by_identity(
            cases,
            identities.intern(&identity.spelling()),
        )
        .ok_or(LoweringError::Unsupported(
            "computed case declaration is absent",
        ))?;
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
        let identity = checked_trees::CheckedMemberIdentity::of_member(
            authored.identity,
            authored.name.clone(),
        );
        let declaration =
            terminal_psi::field_by_identity(&case.fields, identities.intern(&identity.spelling()))
                .ok_or(LoweringError::Unsupported(
                    "computed case field declaration is absent",
                ))?;
        let primitive = checked
            .primitive_type_reference(authored.type_reference)
            .ok_or(LoweringError::Unsupported(
                "computed case field is not scalar",
            ))?;
        let scalar_type = terminal_scalar_type(primitive)?;
        // A bounded payload shares its plain carrier; the declared range is
        // not assumed of the operand but requested as an establishment proof.
        if declaration.field_type.scalar_type() != Some(scalar_type)
            || declaration.relevance != language_core::BindingRelevance::Relevant
        {
            return unsupported("computed case field requires its exact scalar carrier");
        }
        field_bindings.push(ReservedField {
            field: declaration.id,
            scalar_type,
            bounded: terminal_semantics::scalar_field_requires_evidence(&declaration.field_type),
        });
    }
    let multiplicity = terminal_multiplicity(checked.type_multiplicity(source.type_reference));
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
            .enumerate()
            .map(|(ordinal, reserved)| {
                reserved.completed(parameter(ordinal, reserved.scalar_type.into()))
            })
            .collect::<Vec<_>>();
        Ok(Construction {
            place: self.place,
            structural_type: self.structural_type,
            multiplicity: self.multiplicity,
            case: self.case,
            fields,
            claims: Vec::new(),
        })
    }
}

/// Namespace discovery follows retained computation roots. Source replay and
/// establishment validation remain separate from catalog selection.
pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
) -> Result<Vec<typed_trees::type_identity::TypeIdentityId>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_in_machines(machines);
    let mut types = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        if let CheckedScalarComputationKind::CaseMembership { receiver_value, .. } =
            &plans.nodes.get(handle).kind
            && receiver_value.is_valid()
        {
            let root = observed_values::unique_root(checked, *receiver_value)?;
            let identity = checked.normalized_type_identity(root.type_reference);
            if !types.contains(&identity) {
                types.push(identity);
            }
        }
        for subject in constructions(checked, &plans.nodes.get(handle).kind)? {
            let identity = checked.normalized_type_identity(subject.type_reference);
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
    // The tag observation retains its complete receiver beside its ordinary coordinates.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn case_membership(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        observation: checked_trees::CheckedScalarComputationHandle,

        subject: &CheckedScalarComputationStructuralArgument,
        receiver_value: checked_trees::CheckedStructuralValueHandle,
        observed_case: symbols::SymbolHandle,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        if receiver_value.is_valid() {
            return observed_values::lower(
                self,
                identities,
                observation,
                receiver_value,
                observed_case,
                input_types,
                target,
                site,
            );
        }

        let subject = match subject {
            CheckedScalarComputationStructuralArgument::Case(subject) => subject,
            CheckedScalarComputationStructuralArgument::Place(argument) => {
                let (_, state) = crate::declarations::state_declaration(self.checked, site.state)?;
                let source = site
                    .bindings
                    .borrowed_structural_home(argument, self.checked.state_parameters(state))?;
                if !terminal_psi::StructuralAccess::SharedBorrow.admits(source.access) {
                    return unsupported("case observation requires readable source custody");
                }
                if argument.path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    )
                }) {
                    // A runtime-selected element observes "some element": its
                    // selector is read before the tag and the observation owns
                    // the element's bound.
                    let identity = structural_case_identity(self.checked, observed_case)?;
                    let expression = site.bindings.element_case_observation(
                        identities,
                        argument,
                        &source,
                        &identity.spelling(),
                    )?;
                    return Ok(self.binding(
                        input_types,
                        input_types.len(),
                        target,
                        LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                            expression: Box::new(expression),
                        }),
                    ));
                }
                let structural_type = site.bindings.structural_argument_type(&source).ok_or(
                    LoweringError::Unsupported("case observation lost its established carrier"),
                )?;
                let identity = structural_case_identity(self.checked, observed_case)?;
                let (path, case) =
                    crate::expression_preparation::bindings::structural_cases::resolve_observation_at(
                        identities,
                        self.structural_types,
                        structural_type,
                        &argument.path,
                        &identity,
                    )?;
                let mut selected = source.path;
                selected.extend(path);
                return Ok(self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                        expression: Box::new(
                            LoweredBooleanReturnExpression::StructuralCaseMembership {
                                source: source.place,
                                path: selected,
                                case,
                            },
                        ),
                    }),
                ));
            }
        };
        // This observer has only a no-code retirement, unlike a call operand
        // whose normal continuation carries the checked nominal action roster.
        // Constructor identity itself grants neither kind of disposition.
        if !crate::content_classes::plain_owned_with_numeric_constraints(
            self.checked,
            subject.type_reference,
        ) || !matches!(
            self.checked.type_multiplicity(subject.type_reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        ) {
            return unsupported("computed case observation requires its nontrivial cleanup plan");
        }
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
        for (field, reserved) in retained.iter().zip(&slot.fields) {
            let operand = Argument::Computation(field.value);
            self.argument_type(identities, &operand, site, input_types)?;
            completed_fields.push(reserved.completed(parameter(
                completed_types.len(),
                reserved.scalar_type.into(),
            )));
            completed_types.push(reserved.scalar_type.into());
            operands.push(operand);
        }
        let mut outgoing = parameters(input_types);
        outgoing.push(parameter(completed_types.len(), ScalarType::Boolean.into()));
        let observation = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: vec![LoweredScalarBinding::Expression(
                LoweredDirectExpression::Boolean {
                    expression: Box::new(
                        LoweredBooleanReturnExpression::StructuralCaseMembership {
                            source: slot.place,
                            path: Vec::new(),
                            case,
                        },
                    ),
                },
            )],
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: outgoing,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: if slot.multiplicity == StructuralMultiplicity::Affine {
                    vec![
                        crate::unit::unit_cleanup::PreparedNormalCleanupAction::discard_root(
                            slot.place,
                        ),
                    ]
                } else {
                    Vec::new()
                },
            },
        });
        let constructor = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::EstablishScalarCase(Construction {
                place: slot.place,
                structural_type: slot.structural_type,
                multiplicity: slot.multiplicity,
                case: slot.case,
                fields: completed_fields,
                claims: Vec::new(),
            })],
            terminator: LoweredScalarBranchTerminator::Jump {
                target: observation,
                arguments: parameters(&completed_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        });
        self.sequence(
            identities,
            &operands,
            input_types,
            constructor,
            site,
            active,
        )
    }
}

pub(crate) fn emit(
    effect: &Construction,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut fields = Vec::new();
    for field in &effect.fields {
        validate_direct_parameter_types(&field.value, &types)?;
        if !matches!(field.value, LoweredDirectExpression::Parameter { .. }) {
            return unsupported("case construction requires completed scalar fields");
        }
        let value = emit_direct_expression(&field.value, values, next_value, operations);
        // The verifier reconstructs the declared range against this exact
        // completed payload value. The identity requests that proof; it does
        // not assert that the payload is in range.
        let range_obligation = if field.bounded {
            Some(calls.allocate_requirement()?)
        } else {
            None
        };
        fields.push(terminal_psi::RecordFieldInitializer {
            field: field.field,
            value: terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            },
        });
    }
    // Canonical publication orders identities only after every expression has
    // completed; it never changes authored evaluation order.
    fields.sort_by_key(|field| field.field);
    operations.push_leaf(
        OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: effect.place,
            structural_type: effect.structural_type,
            multiplicity: effect.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: crate::emission::claim_bindings::emit_establishment_claims(
                &effect.claims,
                &mut calls.claims,
            )?,
        }),
        if fields.is_empty() {
            // Payloadless cases remain pure tag establishments. Guarded exit
            // receipts independently recognize this exact empty operation.
            OperationKind::EstablishScalarCase {
                result_case: effect.case,
                fields: Vec::new(),
            }
        } else {
            OperationKind::EstablishStructuralCase {
                result_case: effect.case,
                fields,
            }
        },
    );
    crate::emission::claim_bindings::record_prepared_introductions(
        operations,
        effect.place,
        &effect.claims,
        &calls.claims,
        crate::emission::claim_bindings::ClaimEstablishmentKind::Construction,
    )?;
    Ok(())
}

pub(crate) fn declarations(
    operations: &[Operation],
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(
            operation.kind,
            OperationKind::EstablishScalarCase { .. }
                | OperationKind::EstablishStructuralCase { .. }
        ) {
            return None;
        }
        operation_result_declaration(operation)
    })
}

/// Parameter-field observations retain a case by symbol; the structural-case
/// resolver keys on the declared identity spelling, so recover it from the
/// case's own declaration.
pub(crate) fn structural_case_identity(
    checked: &CheckedTrees,
    observed_case: symbols::SymbolHandle,
) -> Result<checked_trees::CheckedMemberIdentity, LoweringError> {
    checked
        .data_definitions()
        .iter()
        .flat_map(|data| checked.data_members(data).iter())
        .filter_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == observed_case => {
                Some(checked_trees::CheckedMemberIdentity::of_member(
                    variant.identity,
                    variant.name.clone(),
                ))
            }
            _ => None,
        })
        .next()
        .ok_or(LoweringError::Unsupported(
            "computed membership lost its case owner",
        ))
}
