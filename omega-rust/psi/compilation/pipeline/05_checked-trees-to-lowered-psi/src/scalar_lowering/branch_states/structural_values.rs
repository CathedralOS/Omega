//! Structural construction participates in a branch state's evaluation order.
//! Scalar fields use ordinary computation continuations in authored order.
//! Completed aggregates reuse record, case and array emitters. Their shapes
//! determine establishment, never a separate caller-specific evaluation path.
//! Construction is independent of its destination: a local publishes the
//! completed root, while an expression consumer observes and retires it.
//! Nested owned aggregates transfer into their parent, while a root survives
//! through selected arguments.
//! Whole local initializers bind fresh block homes: unrestricted values copy,
//! affine values move. Retire a moved source name only once construction finishes.
//! Final live locals rebind in declaration order at the selected cleanup edge;
//! this preserves disposal order without remapping survivors after every move.
use super::super::{
    StructuralFieldId, StructuralFieldType, StructuralTypeId, StructuralTypeShape,
    ValueDeclaration, allocate_dense, place_id,
};
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, Multiplicity, PlaceId,
    PreparedScalarQualifications, QualifiedScalarType, ScalarType, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, computations, storage, unsupported,
};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::emission::claim_bindings::{
    PreparedClaim, claims_below_path, emit_establishment_claims, prepare_establishment_claims,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarArrayConstruction, LoweredScalarBranchState, LoweredScalarBranchTerminator,
    LoweredScalarEffect,
};
use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
};

mod calls;

/// Reserved homes are independent of evaluation. The expression expansion and
/// statement prefix share one construction schedule, without inventing locals
/// for temporary values or allocating places in a separate namespace.
#[derive(Clone)]
pub(crate) struct Storage {
    places: Vec<(checked_trees::CheckedStructuralValueHandle, PlaceId)>,
    results: Vec<(u32, PlaceId)>,
}

impl Storage {
    pub(crate) fn reserve(
        checked: &CheckedTrees,
        root: checked_trees::CheckedStructuralValueHandle,
        next_place: &mut u64,
    ) -> Result<Self, LoweringError> {
        let mut storage = Self {
            places: Vec::new(),
            results: Vec::new(),
        };
        storage.reserve_value(checked, root, next_place)?;
        Ok(storage)
    }

    pub(crate) fn place(
        &self,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Result<PlaceId, LoweringError> {
        self.places
            .iter()
            .find(|(candidate, _)| *candidate == value)
            .map(|(_, place)| *place)
            .ok_or(LoweringError::Unsupported(
                "structural value has no reserved home",
            ))
    }
}

pub(crate) struct PreparedValue {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    /// Source homes moved whole into the value's members.
    pub(crate) consumed: Vec<PlaceId>,
    statement: u32,
    steps: Vec<Step>,
}

enum Step {
    Argument {
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        ordinal: usize,
        argument: checked_trees::CheckedCallScalarArgument,
        prefix: Vec<QualifiedScalarType>,
    },
    Value {
        prepared: PreparedValue,
        prefix: Vec<QualifiedScalarType>,
    },
    Call {
        prepared: super::unit_operations::Prepared,
        prefix: Vec<QualifiedScalarType>,
    },
    Bind {
        source: StructuralArgument,
        destination: StructuralParameterDeclaration,
        prefix: Vec<QualifiedScalarType>,
    },
    Scalar {
        role: CheckedScalarExpressionRole,
        result_type: QualifiedScalarType,
        prefix: Vec<QualifiedScalarType>,
    },
    Aggregate {
        construction: Construction,
        prefix: Vec<QualifiedScalarType>,
    },
    Establish {
        effect: LoweredScalarEffect,
        prefix: Vec<QualifiedScalarType>,
    },
    Zeroed {
        place: PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        claims: Vec<PreparedClaim>,
        prefix: Vec<QualifiedScalarType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Construction {
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    claims: Vec<PreparedClaim>,
    contents: AggregateContents,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AggregateContents {
    Fields {
        kind: AggregateKind,
        fields: Vec<(StructuralFieldId, Field)>,
        declarations: Vec<terminal_psi::StructuralFieldDeclaration>,
    },
    Elements(Vec<PlaceId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateKind {
    Record,
    Case(semantic_vocabulary::StructuralCaseId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Scalar {
        position: usize,
        scalar_type: ScalarType,
        bounded: bool,
    },
    Structural(PlaceId),
    Zero {
        scalar_type: ScalarType,
        bounded: bool,
    },
}

/// The caller validates source custody and owns publication or disposal of the
/// completed value. This schedule does not grant a name or extend its lifetime.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_value(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    state: symbols::SymbolHandle,
    owner: facts::PlaceRoot,
    statement: u32,
    value: checked_trees::CheckedStructuralValueHandle,
    calls: &[checked_trees::CheckedStructuralValueCall],
    reference: checked_trees::types::TypeReferenceHandle,
    bindings: &storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
    types: &[StructuralTypeDeclaration],
    reserved: &Storage,
) -> Result<PreparedValue, LoweringError> {
    let (machine, _) = crate::declarations::state_declaration(checked, state)?;
    let structural_type = carrier_declaration(checked, types, reference)?.id;
    let claims = prepare_establishment_claims(
        checked,
        machine.symbol,
        state,
        statement as usize,
        owner,
        reference,
        structural_type,
        types,
    )?;
    let mut preparation = Preparation {
        checked,
        qualifications,
        state,
        calls,
        consumed_calls: Vec::new(),
        bindings,
        types,
        reserved,
        prefix: value_types.to_vec(),
        steps: Vec::new(),
        consumed: Vec::new(),
        claims,
    };
    let mut place = preparation.value(value, reference, &[])?;
    if preparation.consumed_calls.len() != calls.len() {
        return unsupported("structural value contains unconsumed call producers");
    }
    if checked.type_multiplicity(reference) == Multiplicity::Affine
        && matches!(
            checked.facts.values.structural_values.nodes.get(value).kind,
            CheckedStructuralValueKind::Place(_)
        )
    {
        place = preparation.bind_place(
            value,
            StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            structural_type,
            StructuralMultiplicity::Affine,
        )?;
    }
    Ok(PreparedValue {
        place,
        structural_type,
        statement,
        steps: preparation.steps,
        consumed: preparation.consumed,
    })
}

/// Layout lookup uses the carrier; the caller keeps the declared type for
/// qualifications, access and ownership throughout the value schedule. A
/// reference carrier remains a reference, rather than becoming an owned value.
fn carrier_declaration<'types>(
    checked: &CheckedTrees,
    types: &'types [StructuralTypeDeclaration],
    reference: checked_trees::types::TypeReferenceHandle,
) -> Result<&'types StructuralTypeDeclaration, LoweringError> {
    let identity = validation::stored_structural_type_identity(&checked.typed, reference, &[], &[]);
    let mut declarations = types
        .iter()
        .filter(|declaration| declaration.identity == identity);
    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
        "structural value lost its declared carrier",
    ))?;
    if declarations.next().is_some() {
        return unsupported("structural value carrier is ambiguous");
    }
    Ok(declaration)
}

struct Preparation<'a> {
    checked: &'a CheckedTrees,
    qualifications: &'a PreparedScalarQualifications,
    state: symbols::SymbolHandle,
    calls: &'a [checked_trees::CheckedStructuralValueCall],
    consumed_calls: Vec<checked_trees::CheckedStructuralValueHandle>,
    bindings: &'a storage::ScalarBindings,
    types: &'a [StructuralTypeDeclaration],
    reserved: &'a Storage,
    prefix: Vec<QualifiedScalarType>,
    steps: Vec<Step>,
    consumed: Vec<PlaceId>,
    claims: Vec<PreparedClaim>,
}

impl Preparation<'_> {
    fn value(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
        claim_path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<PlaceId, LoweringError> {
        let plans = &self.checked.facts.values.structural_values;
        let node = plans.nodes.get(value);
        let declaration = carrier_declaration(self.checked, self.types, reference)?;
        if let CheckedStructuralValueKind::Call { .. } = &node.kind {
            let mut producers = self.calls.iter().filter(|call| call.value == value);
            let producer = producers.next().ok_or(LoweringError::Unsupported(
                "structural value call has no retained producer",
            ))?;
            if producers.next().is_some() || self.consumed_calls.contains(&value) {
                return unsupported("structural value call has duplicate producer custody");
            }
            let place = self.prepare_calls(producer)?;
            self.consumed_calls.push(value);
            return Ok(place);
        }
        if let CheckedStructuralValueKind::Place(argument) = &node.kind {
            let source = self.bindings.owned_argument(argument)?;
            if self.checked.type_multiplicity(reference) == Multiplicity::Unrestricted {
                return self.bind_place(
                    value,
                    source,
                    declaration.id,
                    StructuralMultiplicity::Unrestricted,
                );
            }
            // An affine or linear member moves its whole source home, a named
            // local or a parameter, directly into EstablishRecord; the claims
            // it holds continue beneath the member's field. Checking already
            // retired the source's own disposal at this move.
            if !source.path.is_empty() {
                return unsupported("record member moves only a whole owned source");
            }
            if self.consumed.contains(&source.place) {
                return unsupported("record value transfers its source more than once");
            }
            self.consumed.push(source.place);
            return Ok(source.place);
        }
        if let CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } = &node.kind {
            let (machine, state) =
                crate::declarations::state_declaration(self.checked, self.state)?;
            let mut source = self.bindings.copied_place_source(
                source,
                machine,
                self.checked.state_parameters(state),
                self.types,
            )?;
            source.formation_qualifications = self
                .checked
                .facts
                .qualifications
                .value_formations
                .span(node.qualification_decisions)
                .ok_or(LoweringError::Unsupported(
                    "copied membership decisions are stale",
                ))?
                .iter()
                .map(|decision| decision.semantic_domain)
                .collect();
            let place = self.reserved.place(value)?;
            self.steps.push(Step::Establish {
                effect: LoweredScalarEffect::CopyStructuralValue {
                    source,
                    place,
                    structural_type: declaration.id,
                },
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        if matches!(
            &node.kind,
            CheckedStructuralValueKind::ZeroedStructural {}
                | CheckedStructuralValueKind::ZeroedScalarArray { .. }
        ) {
            if matches!(
                &node.kind,
                CheckedStructuralValueKind::ZeroedScalarArray { .. }
            ) {
                computations::arrays::validate_layout(declaration.id, self.types)?;
            }
            // A declared-only local establishes a zeroed whole shape: mint the
            // fresh home with no field operands, mirroring the Unit path's
            // EstablishZeroedStructural emission.
            let multiplicity = terminal_multiplicity(self.checked.type_multiplicity(reference));
            let place = self.reserved.place(value)?;
            self.steps.push(Step::Zeroed {
                place,
                structural_type: declaration.id,
                multiplicity,
                claims: claims_below_path(&self.claims, claim_path),
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        if let CheckedStructuralValueKind::Case(subject) = &node.kind {
            let source = validation::scalar_case_constructor(&self.checked.typed, node.expression)
                .ok_or(LoweringError::Unsupported(
                    "computed case lost its exact authored constructor",
                ))?;
            let place = self.reserved.place(value)?;
            let slot = computations::cases::reserve(
                self.checked,
                node.expression,
                &source,
                self.types,
                place,
            )?;
            let fields =
                crate::expression_preparation::computation_graph::fields(self.checked, subject)?;
            if fields.len() != slot.fields.len()
                || fields
                    .iter()
                    .zip(&slot.fields)
                    .any(|(field, reserved)| field.symbol != reserved.symbol)
            {
                return unsupported("computed case field roster changed after reservation");
            }
            let mut completed = Vec::new();
            for (ordinal, (field, reserved)) in fields.iter().zip(&slot.fields).enumerate() {
                let result_type = computations::computation_value_type(
                    self.checked,
                    self.qualifications,
                    field.value,
                    self.bindings,
                    &self.prefix,
                )?;
                completed.push(reserved.completed(LoweredDirectExpression::Parameter {
                    position: self.prefix.len(),
                    scalar_type: result_type.scalar_type,
                }));
                self.steps.push(Step::Scalar {
                    role: CheckedScalarExpressionRole::StructuralValueField {
                        expression: node.expression,
                        field_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("computed case field ordinal overflow")
                        })?,
                    },
                    result_type,
                    prefix: self.prefix.clone(),
                });
                self.prefix.push(result_type);
            }
            self.steps.push(Step::Establish {
                effect: LoweredScalarEffect::EstablishScalarCase(
                    computations::cases::Construction {
                        place,
                        structural_type: slot.structural_type,
                        multiplicity: slot.multiplicity,
                        case: slot.case,
                        fields: completed,
                        claims: claims_below_path(&self.claims, claim_path),
                    },
                ),
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        if let CheckedStructuralValueKind::ScalarArray { elements } = &node.kind {
            computations::arrays::validate_layout(declaration.id, self.types)?;
            let place = self.reserved.place(value)?;
            let mut completed = Vec::with_capacity(elements.len());
            for (ordinal, element) in elements.iter().enumerate() {
                let result_type = computations::computation_value_type(
                    self.checked,
                    self.qualifications,
                    *element,
                    self.bindings,
                    &self.prefix,
                )?;
                completed.push(LoweredDirectExpression::Parameter {
                    position: self.prefix.len(),
                    scalar_type: result_type.scalar_type,
                });
                self.steps.push(Step::Scalar {
                    role: CheckedScalarExpressionRole::StructuralArrayElement {
                        expression: node.expression,
                        element_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("scalar array leaf ordinal overflow")
                        })?,
                    },
                    result_type,
                    prefix: self.prefix.clone(),
                });
                self.prefix.push(result_type);
            }
            self.steps.push(Step::Establish {
                effect: LoweredScalarEffect::EstablishScalarArray(LoweredScalarArrayConstruction {
                    place,
                    structural_type: declaration.id,
                    elements: completed,
                }),
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        if let CheckedStructuralValueKind::FixedArray { elements } = &node.kind {
            let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                return unsupported("structural array has a nonarray carrier");
            };
            let reference = self.checked.facts.resolved_types.carrier(reference);
            if !reference.is_valid() {
                return unsupported("structural array source carrier is missing");
            }
            let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                self.checked.type_reference_table.type_reference(reference)
            else {
                return unsupported("structural array source has a nonarray carrier");
            };
            let element_reference = *element_type;
            if u64::try_from(elements.len()).ok() != Some(length)
                || carrier_declaration(self.checked, self.types, element_reference)?.id != element
            {
                return unsupported("structural array elements differ from their declared carrier");
            }
            let mut completed = Vec::with_capacity(elements.len());
            for (element_index, child) in elements.iter().enumerate() {
                let mut child_path = claim_path.to_vec();
                child_path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                    u64::try_from(element_index)
                        .map_err(|_| LoweringError::Unsupported("array claim index exceeds u64"))?,
                ));
                completed.push(self.value(*child, element_reference, &child_path)?);
            }
            let place = self.reserved.place(value)?;
            let multiplicity = terminal_multiplicity(self.checked.type_multiplicity(reference));
            self.steps.push(Step::Aggregate {
                construction: Construction {
                    place,
                    structural_type: declaration.id,
                    multiplicity,
                    claims: claims_below_path(&self.claims, claim_path),
                    contents: AggregateContents::Elements(completed),
                },
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        let (data_symbol, fields) = match node.kind {
            checked_trees::record_or_structural_case_pattern!(data_symbol,
                fields, ..) => (data_symbol, fields),
            _ => return unsupported("structural construction requires aggregate value custody"),
        };
        let owner = self.checked.data_definition_by_symbol(data_symbol).ok_or(
            LoweringError::Unsupported("aggregate declaration is absent"),
        )?;
        // Field identities are declaration-local: common members and case
        // payloads have separate namespaces. Join each roster to its source
        // symbols before scheduling operands, then share one evaluation path.
        let (kind, common, payload) = match &node.kind {
            CheckedStructuralValueKind::StructuralCase { case, .. } => {
                let (common, cases) = match &declaration.shape {
                    StructuralTypeShape::Sum { cases } => (&[][..], cases),
                    StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
                    _ => return unsupported("case construction has a non-sum shape"),
                };
                let variant = self
                    .checked
                    .data_members(owner)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Variant(variant)
                            if variant.symbol == *case =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "aggregate selected a foreign case",
                    ))?;
                let selected =
                    terminal_psi::structural_case_by_identity(cases, &variant.path_identity())
                        .ok_or(LoweringError::Unsupported(
                            "aggregate case declaration is absent",
                        ))?;
                (
                    AggregateKind::Case(selected.id),
                    common,
                    Some((variant, selected)),
                )
            }
            // A record, the only other aggregate kind admitted above.
            _ => {
                let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
                    return unsupported("record construction has a non-record shape");
                };
                (AggregateKind::Record, fields.as_slice(), None)
            }
        };
        let mut declared_fields = Vec::new();
        for target in common {
            let source = self
                .checked
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    checked_trees::data::DataMember::Field(field)
                        if field.path_identity() == target.identity =>
                    {
                        Some(field)
                    }
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "aggregate common field declaration is absent",
                ))?;
            declared_fields.push((
                source.symbol,
                target,
                matches!(kind, AggregateKind::Case(_)),
            ));
        }
        if let Some((variant, selected)) = payload {
            for target in &selected.fields {
                let source = self
                    .checked
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.path_identity() == target.identity)
                    .ok_or(LoweringError::Unsupported(
                        "aggregate payload field declaration is absent",
                    ))?;
                declared_fields.push((source.symbol, target, false));
            }
        }
        let fields = plans
            .record_fields
            .span(fields)
            .ok_or(LoweringError::Unsupported("aggregate field span is stale"))?;
        let mut initialized = Vec::new();
        for (ordinal, field) in fields.iter().enumerate() {
            let position = declared_fields
                .iter()
                .position(|(symbol, _, _)| *symbol == field.field)
                .ok_or(LoweringError::Unsupported(
                    "aggregate field is absent from its structural declaration",
                ))?;
            let (_, target, _) = declared_fields[position];
            // Source custody replays every semantic initializer. Erased fields
            // keep their declaration identity but produce no executable operand,
            // storage or cleanup, including for nested initializer values.
            if target.relevance.is_erased() {
                if !matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                    return unsupported("erased record field has a runtime carrier");
                }
                continue;
            }
            let field_value = match field.value {
                CheckedStructuralRecordFieldValue::Scalar(root) => {
                    let result_type = computations::computation_value_type(
                        self.checked,
                        self.qualifications,
                        root,
                        self.bindings,
                        &self.prefix,
                    )?;
                    let field_ordinal = u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("aggregate field ordinal exceeds u32")
                    })?;
                    let role = match kind {
                        AggregateKind::Record => CheckedScalarExpressionRole::RecordField {
                            expression: node.expression,
                            field_ordinal,
                        },
                        AggregateKind::Case(_) => {
                            CheckedScalarExpressionRole::StructuralValueField {
                                expression: node.expression,
                                field_ordinal,
                            }
                        }
                    };
                    let value_position = self.prefix.len();
                    self.steps.push(Step::Scalar {
                        role,
                        result_type,
                        prefix: self.prefix.clone(),
                    });
                    self.prefix.push(result_type);
                    Field::Scalar {
                        position: value_position,
                        scalar_type: result_type.scalar_type,
                        bounded: terminal_semantics::scalar_field_requires_evidence(
                            &target.field_type,
                        ),
                    }
                }
                CheckedStructuralRecordFieldValue::Structural(child) => {
                    let mut child_path = claim_path.to_vec();
                    if let AggregateKind::Case(case) = kind
                        && !declared_fields[position].2
                    {
                        child_path.push(terminal_psi::StructuralPathSegment::Case(case));
                    }
                    child_path.push(terminal_psi::StructuralPathSegment::Field(
                        target.identity.clone(),
                    ));
                    Field::Structural(self.value(child, field.type_reference, &child_path)?)
                }
            };
            initialized.push((position, (target.id, field_value)));
        }
        // Case literals default unnamed common scalar fields only after all
        // authored operands. The same declaration-owned range obligation still
        // applies to that zero; no structural child or payload is invented.
        for (position, (_, target, defaults_to_zero)) in declared_fields.iter().enumerate() {
            if !defaults_to_zero
                || target.relevance.is_erased()
                || initialized
                    .iter()
                    .any(|(initialized, _)| *initialized == position)
            {
                continue;
            }
            let scalar_type = target
                .field_type
                .scalar_type()
                .ok_or(LoweringError::Unsupported(
                    "an unnamed common field has no scalar zero",
                ))?;
            initialized.push((
                position,
                (
                    target.id,
                    Field::Zero {
                        scalar_type,
                        bounded: terminal_semantics::scalar_field_requires_evidence(
                            &target.field_type,
                        ),
                    },
                ),
            ));
        }
        initialized.sort_by_key(|(position, _)| *position);
        if !initialized
            .iter()
            .map(|(position, _)| *position)
            .eq(declared_fields
                .iter()
                .enumerate()
                .filter(|(_, (_, field, _))| !field.relevance.is_erased())
                .map(|(position, _)| position))
        {
            return unsupported("aggregate did not initialize its exact field roster");
        }
        let multiplicity = terminal_multiplicity(self.checked.type_multiplicity(reference));
        let place = self.reserved.place(value)?;
        self.steps.push(Step::Aggregate {
            construction: Construction {
                place,
                structural_type: declaration.id,
                multiplicity,
                claims: claims_below_path(&self.claims, claim_path),
                contents: AggregateContents::Fields {
                    kind,
                    fields: initialized.into_iter().map(|(_, field)| field).collect(),
                    declarations: declared_fields
                        .iter()
                        .map(|(_, field, _)| (*field).clone())
                        .collect(),
                },
            },
            prefix: self.prefix.clone(),
        });
        Ok(place)
    }

    fn bind_place(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        source: StructuralArgument,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> Result<PlaceId, LoweringError> {
        let place = self.reserved.place(value)?;
        self.steps.push(Step::Bind {
            source,
            destination: StructuralParameterDeclaration {
                place,
                position: 0,
                structural_type,
                multiplicity,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            prefix: self.prefix.clone(),
        });
        Ok(place)
    }
}

impl PreparedValue {
    pub(crate) fn finish(
        self,
        state: symbols::SymbolHandle,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        target: usize,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<usize, LoweringError> {
        let completed_types = self
            .steps
            .last()
            .map(|step| match step {
                Step::Call { prefix, .. }
                | Step::Argument { prefix, .. }
                | Step::Value { prefix, .. }
                | Step::Scalar { prefix, .. }
                | Step::Aggregate { prefix, .. }
                | Step::Establish { prefix, .. }
                | Step::Zeroed { prefix, .. }
                | Step::Bind { prefix, .. } => prefix,
            })
            .ok_or(LoweringError::Unsupported(
                "record construction has no completion",
            ))?;
        let mut target = computations.push(LoweredScalarBranchState::jumping(
            target,
            Vec::new(),
            completed_types.clone(),
            source_types,
            Vec::new(),
        ));
        for step in self.steps.into_iter().rev() {
            target = match step {
                Step::Call { prepared, prefix } => {
                    prepared.finish_completed(&prefix, target, computations)
                }
                Step::Argument {
                    coordinate,
                    ordinal,
                    argument,
                    prefix,
                } => computations.call_arguments(
                    state,
                    coordinate,
                    std::slice::from_ref(&argument),
                    ordinal,
                    bindings,
                    &prefix,
                    target,
                )?,
                Step::Value { prepared, prefix } => {
                    prepared.finish(state, bindings, &prefix, target, computations)?
                }
                Step::Bind {
                    source,
                    destination,
                    prefix,
                } => {
                    let copied = computations.push(LoweredScalarBranchState::jumping(
                        target,
                        vec![destination],
                        prefix.clone(),
                        &prefix,
                        Vec::new(),
                    ));
                    computations.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        parameter_types: prefix.clone(),
                        erased_formal_types: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        bindings: Vec::new(),
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Jump {
                            target: copied,
                            arguments: computations::parameters(&prefix),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: vec![source],
                            cleanup_actions: Vec::new(),
                        },
                    })
                }
                Step::Scalar {
                    role,
                    result_type,
                    prefix,
                } => computations.retained_value(
                    state,
                    self.statement,
                    role,
                    bindings,
                    &prefix,
                    result_type,
                    target,
                )?,
                Step::Establish { effect, prefix } => {
                    computations.push(LoweredScalarBranchState::jumping(
                        target,
                        Vec::new(),
                        prefix.clone(),
                        &prefix,
                        vec![effect],
                    ))
                }
                Step::Aggregate {
                    construction,
                    prefix,
                } => computations.push(LoweredScalarBranchState::jumping(
                    target,
                    Vec::new(),
                    prefix.clone(),
                    &prefix,
                    vec![LoweredScalarEffect::EstablishAggregate(construction)],
                )),
                Step::Zeroed {
                    place,
                    structural_type,
                    multiplicity,
                    claims,
                    prefix,
                } => computations.push(LoweredScalarBranchState::jumping(
                    target,
                    Vec::new(),
                    prefix.clone(),
                    &prefix,
                    vec![LoweredScalarEffect::EstablishZeroedStructural {
                        place,
                        structural_type,
                        multiplicity,
                        claims,
                    }],
                )),
            };
        }
        Ok(target)
    }
}

pub(crate) fn emit(
    construction: &Construction,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    let claims = emit_establishment_claims(&construction.claims, &mut calls.claims)?;
    let (kind, selected_fields, declarations) = match &construction.contents {
        AggregateContents::Fields {
            kind,
            fields,
            declarations,
        } => (kind, fields, declarations),
        AggregateContents::Elements(elements) => {
            crate::unit::attached_unit::structural_values::fixed_array::emit_completed(
                construction.place,
                construction.structural_type,
                construction.multiplicity,
                elements
                    .iter()
                    .map(|place| StructuralArgument {
                        place: *place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    })
                    .collect(),
                operations,
            )?;
            operations.set_structural_result_claims(construction.place, claims)?;
            return Ok(());
        }
    };
    let mut fields = Vec::with_capacity(selected_fields.len());
    for (field, value) in selected_fields {
        let value = match value {
            Field::Scalar {
                position, bounded, ..
            } => {
                let value = values.get(*position).ok_or(LoweringError::Unsupported(
                    "record field lost its completed scalar",
                ))?;
                terminal_psi::RecordFieldValue::Scalar {
                    value: value.id,
                    range_obligation: if *bounded {
                        Some(calls.allocate_requirement()?)
                    } else {
                        None
                    },
                }
            }
            Field::Structural(place) => {
                terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                    place: *place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                })
            }
            Field::Zero {
                scalar_type,
                bounded,
            } => {
                let value = crate::emission::operation_emission::expressions::emit_scalar_leaf(
                    crate::unit::attached_unit::structural_values::record::zero_constant(
                        *scalar_type,
                    ),
                    *scalar_type,
                    next_value,
                    operations,
                );
                terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation: if *bounded {
                        Some(calls.allocate_requirement()?)
                    } else {
                        None
                    },
                }
            }
        };
        fields.push(terminal_psi::RecordFieldInitializer {
            field: *field,
            value,
        });
    }
    match *kind {
        AggregateKind::Record => {
            crate::unit::attached_unit::structural_values::record::emit_completed(
                construction.place,
                construction.structural_type,
                construction.multiplicity,
                fields,
                declarations,
                operations,
            )?
        }
        AggregateKind::Case(case) => {
            crate::unit::attached_unit::structural_values::structural_case::emit_completed(
                construction.place,
                construction.structural_type,
                construction.multiplicity,
                case,
                fields,
                operations,
            )
        }
    };
    operations.set_structural_result_claims(construction.place, claims)?;
    Ok(())
}
