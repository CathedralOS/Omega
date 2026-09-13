//! Record locals participate in the scalar graph's ordered statement prefix.
//! Scalar fields use ordinary computation continuations in authored order;
//! completed fields commit through the same record emitter as Unit bodies.
//! Only the completed root enters the local namespace. Nested owned records
//! transfer into their parent, while a root survives through selected arguments.
//! Whole local initializers bind fresh block homes: unrestricted values copy,
//! affine values move. Retire a moved source name only once construction finishes.
//! Final live locals rebind in declaration order at the selected cleanup edge;
//! this preserves disposal order without remapping survivors after every move.

use super::*;
use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
};

pub(super) struct Prepared {
    pub(super) symbol: symbols::SymbolHandle,
    pub(super) place: PlaceId,
    pub(super) consumed: Vec<(symbols::SymbolHandle, PlaceId)>,
    statement: u32,
    steps: Vec<Step>,
}

enum Step {
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
    Record {
        construction: Construction,
        prefix: Vec<QualifiedScalarType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Construction {
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    fields: Vec<(StructuralFieldId, Field)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Scalar {
        position: usize,
        scalar_type: ScalarType,
        bounded: bool,
    },
    Structural(PlaceId),
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
    crate::attached_unit::structural_values::source_custody::validate(
        checked, machine, state, operation,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls,
        ..
    } = operation
    else {
        return unsupported("scalar structural statement has no value producer");
    };
    if !calls.is_empty() {
        return unsupported("scalar record construction requires structural call result custody");
    }
    let (_, source) = source_custody::authored_state(checked, state)?;
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
    let mut preparation = Preparation {
        checked,
        qualifications,
        bindings,
        types,
        next_place,
        prefix: value_types.to_vec(),
        steps: Vec::new(),
        consumed: Vec::new(),
    };
    let mut place = preparation.record(*value, local.type_reference)?;
    if result.multiplicity == Multiplicity::Affine
        && matches!(
            checked
                .facts
                .values
                .structural_values
                .nodes
                .get(*value)
                .kind,
            CheckedStructuralValueKind::Place(_)
        )
    {
        let structural_type = types
            .iter()
            .find(|declaration| declaration.identity == result.type_identity)
            .ok_or(LoweringError::Unsupported(
                "moved record lost its declared carrier",
            ))?
            .id;
        place = preparation.bind_place(
            StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            structural_type,
            StructuralMultiplicity::Affine,
        )?;
    }
    Ok(Prepared {
        symbol: local.symbol,
        place,
        statement: result.statement_index,
        steps: preparation.steps,
        consumed: preparation.consumed,
    })
}

struct Preparation<'a> {
    checked: &'a CheckedTrees,
    qualifications: &'a PreparedScalarQualifications,
    bindings: &'a storage::ScalarBindings,
    types: &'a [StructuralTypeDeclaration],
    next_place: &'a mut u64,
    prefix: Vec<QualifiedScalarType>,
    steps: Vec<Step>,
    consumed: Vec<(symbols::SymbolHandle, PlaceId)>,
}

impl Preparation<'_> {
    fn record(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<PlaceId, LoweringError> {
        let plans = &self.checked.facts.values.structural_values;
        let node = plans.nodes.get(value);
        let identity = self.checked.normalized_type_identity(reference);
        let declaration = self
            .types
            .iter()
            .find(|declaration| declaration.identity == identity.as_str())
            .ok_or(LoweringError::Unsupported(
                "scalar record has no structural type namespace",
            ))?;
        let StructuralTypeShape::Record {
            fields: declared_fields,
        } = &declaration.shape
        else {
            return unsupported("scalar record has a nonrecord structural declaration");
        };
        if let CheckedStructuralValueKind::Place(argument) = &node.kind {
            if argument.type_identity != identity.as_str() {
                return unsupported("scalar record transfer changed its carrier");
            }
            let source = self.bindings.owned_argument(argument)?;
            return match self.checked.type_multiplicity(reference) {
                Multiplicity::Unrestricted => {
                    self.bind_place(source, declaration.id, StructuralMultiplicity::Unrestricted)
                }
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
        let CheckedStructuralValueKind::Record {
            data_symbol,
            fields,
        } = node.kind
        else {
            return unsupported("scalar structural construction requires record value custody");
        };
        let owner = self
            .checked
            .data_definitions()
            .iter()
            .find(|owner| owner.symbol == data_symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar record declaration is absent",
            ))?;
        let fields = plans
            .record_fields
            .span(fields)
            .ok_or(LoweringError::Unsupported(
                "scalar record field span is stale",
            ))?;
        let mut initialized = Vec::new();
        for (ordinal, field) in fields.iter().enumerate() {
            let authored = self
                .checked
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    checked_trees::data::DataMember::Field(candidate)
                        if candidate.symbol == field.field =>
                    {
                        Some(candidate)
                    }
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar record field belongs to another declaration",
                ))?;
            let identity = authored
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| authored.name.as_str().to_owned());
            let position = declared_fields
                .iter()
                .position(|field| field.identity == identity)
                .ok_or(LoweringError::Unsupported(
                    "scalar record field is absent from its structural declaration",
                ))?;
            let target = &declared_fields[position];
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
                        return unsupported("scalar record field substituted its carrier");
                    }
                    let role = CheckedScalarExpressionRole::RecordField {
                        expression: node.expression,
                        field_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("record field ordinal exceeds u32")
                        })?,
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
                        bounded: matches!(
                            target.field_type,
                            StructuralFieldType::BoundedInteger(_)
                        ),
                    }
                }
                CheckedStructuralRecordFieldValue::Structural(child) => {
                    let StructuralFieldType::Structural(child_type) = target.field_type else {
                        return unsupported("nested record has a scalar destination");
                    };
                    if !self.types.iter().any(|declaration| {
                        declaration.id == child_type
                            && declaration.identity
                                == self
                                    .checked
                                    .normalized_type_identity(field.type_reference)
                                    .as_str()
                    }) {
                        return unsupported("nested record substituted its structural carrier");
                    }
                    Field::Structural(self.record(child, field.type_reference)?)
                }
            };
            initialized.push((position, (target.id, field_value)));
        }
        initialized.sort_by_key(|(position, _)| *position);
        if initialized.len() != declared_fields.len()
            || initialized
                .iter()
                .enumerate()
                .any(|(ordinal, (position, _))| ordinal != *position)
        {
            return unsupported("scalar record did not initialize its exact field roster");
        }
        let multiplicity = match self.checked.type_multiplicity(reference) {
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Linear => {
                return unsupported("scalar record construction cannot establish linear authority");
            }
        };
        let place = place_id(allocate_dense(self.next_place)?);
        self.steps.push(Step::Record {
            construction: Construction {
                place,
                structural_type: declaration.id,
                multiplicity,
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
            },
            prefix: self.prefix.clone(),
        });
        Ok(place)
    }

    fn bind_place(
        &mut self,
        source: StructuralArgument,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(self.next_place)?);
        self.steps.push(Step::Bind {
            source,
            destination: StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
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
        let completed_types = self
            .steps
            .last()
            .map(|step| match step {
                Step::Scalar { prefix, .. }
                | Step::Record { prefix, .. }
                | Step::Bind { prefix, .. } => prefix,
            })
            .ok_or(LoweringError::Unsupported(
                "record construction has no completion",
            ))?;
        let mut target = computations.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed_types.clone(),
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: computations::parameters(source_types),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
        for step in self.steps.into_iter().rev() {
            target = match step {
                Step::Bind {
                    source,
                    destination,
                    prefix,
                } => {
                    let copied = computations.push(LoweredScalarBranchState {
                        structural_parameters: vec![destination],
                        parameter_types: prefix.clone(),
                        bindings: Vec::new(),
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Jump {
                            target,
                            arguments: computations::parameters(&prefix),
                            structural_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    });
                    computations.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        parameter_types: prefix.clone(),
                        bindings: Vec::new(),
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Jump {
                            target: copied,
                            arguments: computations::parameters(&prefix),
                            structural_arguments: vec![source],
                            trivial_affine_discards: Vec::new(),
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
                    symbols::SymbolHandle::invalid(),
                    bindings,
                    &prefix,
                    result_type,
                    target,
                )?,
                Step::Record {
                    construction,
                    prefix,
                } => computations.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    bindings: Vec::new(),
                    structural_effects: vec![LoweredScalarEffect::EstablishRecord(construction)],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: computations::parameters(&prefix),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
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
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    let mut fields = Vec::with_capacity(construction.fields.len());
    for (field, value) in &construction.fields {
        let value = match value {
            Field::Scalar {
                position,
                scalar_type,
                bounded,
            } => {
                let value = values.get(*position).ok_or(LoweringError::Unsupported(
                    "record field lost its completed scalar",
                ))?;
                if value.scalar_type != *scalar_type {
                    return unsupported("completed record field changed its carrier");
                }
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
        };
        fields.push(terminal_psi::RecordFieldInitializer {
            field: *field,
            value,
        });
    }
    crate::attached_unit::structural_values::record::emit_completed(
        construction.place,
        construction.structural_type,
        construction.multiplicity,
        fields,
        operations,
    );
    Ok(())
}

pub(crate) fn declarations(
    operations: &OperationBuffer,
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations
        .iter()
        .filter_map(|operation| match (&operation.kind, &operation.result) {
            (OperationKind::EstablishRecord { .. }, OperationResult::Structural(result)) => {
                Some(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                })
            }
            _ => None,
        })
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
    let (machine, source) = source_custody::authored_state(checked, state)?;
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
            is_self: false,
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
    let discards = destinations
        .iter()
        .rev()
        .map(|parameter| parameter.place)
        .collect();
    let cleanup = computations.push(LoweredScalarBranchState {
        structural_parameters: destinations,
        parameter_types: parameter_types.to_vec(),
        bindings: Vec::new(),
        structural_effects: Vec::new(),
        terminator: LoweredScalarBranchTerminator::Jump {
            target,
            arguments: computations::parameters(parameter_types),
            structural_arguments: std::mem::take(structural_arguments),
            trivial_affine_discards: discards,
        },
    });
    Ok(computations.push(LoweredScalarBranchState {
        structural_parameters: Vec::new(),
        parameter_types: parameter_types.to_vec(),
        bindings: Vec::new(),
        structural_effects: Vec::new(),
        terminator: LoweredScalarBranchTerminator::Jump {
            target: cleanup,
            arguments: computations::parameters(parameter_types),
            structural_arguments: owners,
            trivial_affine_discards: Vec::new(),
        },
    }))
}
