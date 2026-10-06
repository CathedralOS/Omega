//! Structural construction participates in the scalar graph's evaluation order.
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
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarArrayConstruction, LoweredScalarBranchState, LoweredScalarBranchTerminator,
    LoweredScalarEffect,
};
use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
};

mod calls;

pub(super) struct Prepared {
    pub(super) symbol: symbols::SymbolHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    pub(super) consumed: Vec<(symbols::SymbolHandle, PlaceId)>,
    value: PreparedValue,
}

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
    pub(crate) consumed: Vec<(symbols::SymbolHandle, PlaceId)>,
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
        prefix: Vec<QualifiedScalarType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Construction {
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    kind: AggregateKind,
    fields: Vec<(StructuralFieldId, Field)>,
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

pub(super) fn prepare(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    bindings: &storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Prepared, LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls,
        ..
    } = operation
    else {
        return unsupported("scalar structural statement has no value producer");
    };
    let (_, source) = crate::declarations::state_declaration(checked, state)?;
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("scalar structural producer lost its local destination");
    };
    // Independently check the local's eventual drop or exact initializer move.
    // The operation flag does not establish which owner survives to this exit.
    validation::record_local_disposition(
        &checked.typed,
        &checked.facts,
        machine,
        state,
        result.statement_index,
    )
    .ok_or(LoweringError::Unsupported(
        "scalar record lost its exact final disposition",
    ))?;
    let reserved = Storage::reserve(checked, *value, next_place)?;
    let prepared = prepare_value(
        checked,
        qualifications,
        state,
        result.statement_index,
        *value,
        calls,
        local.type_reference,
        bindings,
        value_types,
        types,
        &reserved,
    )?;
    Ok(Prepared {
        symbol: local.symbol,
        place: prepared.place,
        structural_type: prepared.structural_type,
        consumed: prepared.consumed.clone(),
        value: prepared,
    })
}

/// The caller validates source custody and owns publication or disposal of the
/// completed value. This schedule does not grant a name or extend its lifetime.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_value(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    state: symbols::SymbolHandle,
    statement: u32,
    value: checked_trees::CheckedStructuralValueHandle,
    calls: &[checked_trees::CheckedStructuralValueCall],
    reference: checked_trees::types::TypeReferenceHandle,
    bindings: &storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
    types: &[StructuralTypeDeclaration],
    reserved: &Storage,
) -> Result<PreparedValue, LoweringError> {
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
    };
    let structural_type = carrier_declaration(checked, types, reference)?.id;
    let mut place = preparation.value(value, reference)?;
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
    let carrier = crate::unit::attached_unit::structural_carrier_type(checked, reference)?;
    let identity = checked.normalized_type_identity(carrier);
    let mut declarations = types
        .iter()
        .filter(|declaration| declaration.identity == identity.as_str());
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
    consumed: Vec<(symbols::SymbolHandle, PlaceId)>,
}

impl Preparation<'_> {
    fn value(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
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
            return match self.checked.type_multiplicity(reference) {
                Multiplicity::Unrestricted => self.bind_place(
                    value,
                    source,
                    declaration.id,
                    StructuralMultiplicity::Unrestricted,
                ),
                Multiplicity::Affine => {
                    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } = argument.source
                    else {
                        return unsupported(
                            "record parameter moves require their own source custody join",
                        );
                    };
                    if self
                        .consumed
                        .iter()
                        .any(|(candidate, place)| *candidate == symbol || *place == source.place)
                    {
                        return unsupported("record value transfers its source more than once");
                    }
                    self.consumed.push((symbol, source.place));
                    // A nested owner transfers directly into EstablishRecord.
                    // A whole local initializer instead binds a fresh block home.
                    Ok(source.place)
                }
                Multiplicity::Linear => {
                    unsupported("scalar record cannot transfer linear authority")
                }
            };
        }
        if let CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } = &node.kind {
            let (machine, state) =
                crate::declarations::state_declaration(self.checked, self.state)?;
            let source = self.bindings.copied_place_source(
                source,
                machine,
                self.checked.state_parameters(state),
                self.types,
            )?;
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
            let multiplicity = match self.checked.type_multiplicity(reference) {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => {
                    return unsupported(
                        "scalar zeroed construction cannot establish linear authority",
                    );
                }
            };
            let place = self.reserved.place(value)?;
            self.steps.push(Step::Zeroed {
                place,
                structural_type: declaration.id,
                multiplicity,
                prefix: self.prefix.clone(),
            });
            return Ok(place);
        }
        if let CheckedStructuralValueKind::Case(subject) = &node.kind {
            crate::expression_preparation::source_custody::case_sources::construction(
                self.checked,
                subject,
            )?;
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
            if slot.structural_type != declaration.id {
                return unsupported("computed case changed its expected carrier");
            }
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
                if result_type.scalar_type != reserved.scalar_type {
                    return unsupported("computed case field changed its scalar carrier");
                }
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
        let (data_symbol, fields) = match node.kind {
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields,
            }
            | CheckedStructuralValueKind::StructuralCase {
                data_symbol,
                fields,
                ..
            } => (data_symbol, fields),
            _ => return unsupported("structural construction requires aggregate value custody"),
        };
        let owner = self
            .checked
            .data_definitions()
            .iter()
            .find(|owner| owner.symbol == data_symbol)
            .ok_or(LoweringError::Unsupported(
                "aggregate declaration is absent",
            ))?;
        // Field identities are declaration-local: common members and case
        // payloads have separate namespaces. Join each roster to its source
        // symbols before scheduling operands, then share one evaluation path.
        let (kind, common, payload) = match (&node.kind, &declaration.shape) {
            (CheckedStructuralValueKind::Record { .. }, StructuralTypeShape::Record { fields }) => {
                (AggregateKind::Record, fields.as_slice(), None)
            }
            (CheckedStructuralValueKind::StructuralCase { case, .. }, shape) => {
                let (common, cases) = match shape {
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
                let selected = cases
                    .iter()
                    .find(|candidate| candidate.identity == variant.path_identity())
                    .ok_or(LoweringError::Unsupported(
                        "aggregate case declaration is absent",
                    ))?;
                (
                    AggregateKind::Case(selected.id),
                    common,
                    Some((variant, selected)),
                )
            }
            _ => return unsupported("aggregate changed its structural shape"),
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
                    if target.field_type.scalar_type() != Some(result_type.scalar_type) {
                        return unsupported("aggregate operand changed its scalar carrier");
                    }
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
                    let child_type =
                        crate::unit::attached_unit::structural_values::field_carrier::resolve(
                            self.checked,
                            self.types,
                            &target.field_type,
                            field.type_reference,
                        )?;
                    if carrier_declaration(self.checked, self.types, field.type_reference)?.id
                        != child_type
                    {
                        return unsupported("nested record substituted its structural carrier");
                    }
                    Field::Structural(self.value(child, field.type_reference)?)
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
        let multiplicity = match self.checked.type_multiplicity(reference) {
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Linear => {
                return unsupported("aggregate construction cannot establish linear authority");
            }
        };
        let place = self.reserved.place(value)?;
        self.steps.push(Step::Aggregate {
            construction: Construction {
                place,
                structural_type: declaration.id,
                multiplicity,
                kind,
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
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

impl Prepared {
    pub(super) fn finish(
        self,
        state: symbols::SymbolHandle,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        target: usize,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<usize, LoweringError> {
        self.value
            .finish(state, bindings, source_types, target, computations)
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
        let mut target = computations.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: computations::parameters(source_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        });
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
                    let copied = computations.push(LoweredScalarBranchState {
                        structural_parameters: vec![destination],
                        parameter_types: prefix.clone(),
                        erased_formal_types: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        bindings: Vec::new(),
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Jump {
                            target,
                            arguments: computations::parameters(&prefix),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        },
                    });
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
                Step::Establish { effect, prefix } => computations.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![effect],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: computations::parameters(&prefix),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                }),
                Step::Aggregate {
                    construction,
                    prefix,
                } => computations.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![LoweredScalarEffect::EstablishAggregate(construction)],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: computations::parameters(&prefix),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                }),
                Step::Zeroed {
                    place,
                    structural_type,
                    multiplicity,
                    prefix,
                } => computations.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![LoweredScalarEffect::EstablishZeroedStructural {
                        place,
                        structural_type,
                        multiplicity,
                    }],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: computations::parameters(&prefix),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                }),
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
    let mut fields = Vec::with_capacity(construction.fields.len());
    for (field, value) in &construction.fields {
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
    match construction.kind {
        AggregateKind::Record => {
            crate::unit::attached_unit::structural_values::record::emit_completed(
                construction.place,
                construction.structural_type,
                construction.multiplicity,
                fields,
                operations,
            )
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
    Ok(())
}

/// Complete selected scalar operands before ending the source locals' lives.
/// This intermediate edge is necessary even when the destination has no scalar
/// arguments; cleanup cannot be hoisted before a guard or an argument read.
pub(super) fn exit_target(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    bindings: &storage::ScalarBindings,
    parameter_types: &[QualifiedScalarType],
    structural_arguments: &mut Vec<StructuralArgument>,
    target: usize,
    computations: &mut computations::Expansion<'_>,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<usize, LoweringError> {
    let (machine, source) = crate::declarations::state_declaration(checked, state)?;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup lost its source graph",
        ))?;
    let retained = graph
        .states
        .iter()
        .find(|candidate| candidate.state == state)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup lost its source state",
        ))?;
    let mut owners = Vec::new();
    let mut destinations = Vec::new();
    for operation in &retained.unit_operations {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } = operation
        else {
            continue;
        };
        if result.multiplicity != Multiplicity::Affine {
            continue;
        }
        if !validation::record_local_disposition(
            &checked.typed,
            &checked.facts,
            machine.symbol,
            state,
            result.statement_index,
        )
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup lost its exact local disposition",
        ))? {
            continue;
        }
        let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize)
        else {
            return unsupported("scalar cleanup lost its authored local");
        };
        let argument =
            bindings.owned_argument(&checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                path: Vec::new(),
                type_identity: result.type_identity.clone(),
                access: checked_trees::CheckedStructuralAccess::Owned,
            })?;
        let structural_type = types
            .iter()
            .find(|declaration| declaration.identity == result.type_identity)
            .ok_or(LoweringError::Unsupported(
                "scalar cleanup lost its record carrier",
            ))?
            .id;
        owners.push(argument);
        destinations.push(StructuralParameterDeclaration {
            place: place_id(allocate_dense(next_place)?),
            position: u32::try_from(destinations.len())
                .map_err(|_| LoweringError::Unsupported("local cleanup frontier exceeds u32"))?,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    }
    if owners.is_empty() {
        return Ok(target);
    }
    // Moves give locals a mixture of operation-result and block-parameter homes.
    // Bind the final live owners once in declaration order, after selected reads,
    // so reverse parameter disposal agrees with source order without repeatedly
    // remapping every survivor at each move. The ordinary edge verifier checks
    // consumption, fresh binding and the complete resulting frontier.
    let cleanup_actions = destinations
        .iter()
        .rev()
        .map(|parameter| terminal_psi::TerminalAffineCleanupAction::DiscardRoot(parameter.place))
        .collect();
    let cleanup = computations.push(LoweredScalarBranchState {
        structural_parameters: destinations,
        parameter_types: parameter_types.to_vec(),
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        structural_effects: Vec::new(),
        terminator: LoweredScalarBranchTerminator::Jump {
            target,
            arguments: computations::parameters(parameter_types),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: std::mem::take(structural_arguments),
            cleanup_actions,
        },
    });
    Ok(computations.push(LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        parameter_types: parameter_types.to_vec(),
        erased_formal_types: Vec::new(),
        erased_proof_formals: Vec::new(),
        bindings: Vec::new(),
        structural_effects: Vec::new(),
        terminator: LoweredScalarBranchTerminator::Jump {
            target: cleanup,
            arguments: computations::parameters(parameter_types),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: owners,
            cleanup_actions: Vec::new(),
        },
    }))
}
