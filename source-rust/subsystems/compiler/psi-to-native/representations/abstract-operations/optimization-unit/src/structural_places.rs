/*
We provide structural-place, dominance, and field-value evidence queries over a
`PsiOptimizationFunction` (`FunctionAnalysis`, `FieldEvidence`, `dominators`,
`established_case_at`, `field_value_reads`, and `sole_case_at_path`). Both
specialization passes and their independent replay validators query these routines
so neither side duplicates CFG dominance walks or structural path resolution.
*/

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, OperationId, PlaceId, StructuralCaseId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPlaceDeclaration, StructuralTypeShape, record_or_mixed_shape_pattern,
};

use super::{PsiOptimizationFunction, PsiOptimizationUnit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootProducer {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
    Zeroed(OperationId),
}

impl RootProducer {
    pub const fn operation(&self) -> Option<OperationId> {
        match self {
            Self::Record(operation) | Self::Variant(operation, _) | Self::Zeroed(operation) => {
                Some(*operation)
            }
            Self::None => None,
        }
    }
}

pub struct FieldEvidence<'a> {
    pub declaration: &'a StructuralPlaceDeclaration,
    pub root_type: Option<StructuralTypeId>,
    pub root_producer: RootProducer,
}

pub fn field_evidence<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<FieldEvidence<'a>> {
    let declaration = function.structural_place(place)?;
    Some(FieldEvidence {
        declaration,
        root_type: declared_structural_type(function, declaration),
        root_producer: root_producer(function, declaration),
    })
}

pub fn root_producer(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> RootProducer {
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return RootProducer::None;
    };
    for node in function.block_nodes() {
        match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Record(producer);
            }
            O::EstablishCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Variant(producer, *result_case);
            }
            O::EstablishZeroedStructural {
                psi_operation,
                result,
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Zeroed(producer);
            }
            _ => {}
        }
    }
    RootProducer::None
}

pub fn declared_structural_type(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralTypeId> {
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            structural_type, ..
        }
        | StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }
        | StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => Some(structural_type),
        StructuralPlaceKind::ProviderAttachment { attachment, .. } => Some(attachment),
        StructuralPlaceKind::Parameter { .. } => function
            .structural_parameter(declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .block_structural_parameters()
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::Result => function
            .result
            .structural()
            .and_then(|result| (result.place == declaration.id).then_some(result.structural_type)),
    }
}

pub fn shape(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&StructuralTypeShape> {
    unit.structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)
        .map(|entry| &entry.shape)
}

fn record_fields(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
) -> Option<&[StructuralFieldDeclaration]> {
    match shape(unit, current)? {
        record_or_mixed_shape_pattern!(fields) => Some(fields),
        _ => None,
    }
}

pub(crate) fn descended_field_id(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: StructuralFieldId,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.id == identity)
}

pub(crate) fn descended_field_matching(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    matching: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    let fields = record_fields(unit, current)?;
    let field = fields.iter().find(|field| matching(field))?;
    match field.field_type {
        StructuralFieldType::Structural(next) => Some((field.id, next)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Position<'a> {
    Type(StructuralTypeId),
    CasePayload(&'a StructuralCaseDeclaration),
}

impl<'a> Position<'a> {
    pub fn descend(
        self,
        unit: &'a PsiOptimizationUnit,
        segment: &CanonicalStructuralPathSegment,
    ) -> Option<Position<'a>> {
        match (self, segment) {
            (Self::Type(current), CanonicalStructuralPathSegment::Referent) => {
                match shape(unit, current)? {
                    StructuralTypeShape::Reference { referent, access }
                        if *access != terminal_psi::StructuralAccess::WriteOnlyBorrow =>
                    {
                        Some(Self::Type(*referent))
                    }
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::Field(identity)) => {
                descended_field_id(unit, current, *identity).map(|(_, next)| Self::Type(next))
            }
            (Self::Type(current), CanonicalStructuralPathSegment::FixedIndex(_)) => {
                match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::Case(identity)) => {
                roster_cases(unit, current)?
                    .iter()
                    .find(|case| case.id == *identity)
                    .map(Self::CasePayload)
            }
            (Self::CasePayload(case), CanonicalStructuralPathSegment::Field(identity)) => {
                match case
                    .fields
                    .iter()
                    .find(|field| field.id == *identity)?
                    .field_type
                {
                    StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                    _ => None,
                }
            }
            (Self::CasePayload(_), _) => None,
        }
    }

    pub fn field_declaration(
        self,
        unit: &'a PsiOptimizationUnit,
        field: StructuralFieldId,
    ) -> Option<&'a StructuralFieldDeclaration> {
        let fields = match self {
            Self::Type(current) => record_fields(unit, current)?,
            Self::CasePayload(case) => case.fields.as_slice(),
        };
        fields.iter().find(|declaration| declaration.id == field)
    }
}

pub fn observed_field_declaration<'a>(
    unit: &'a PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<&'a StructuralFieldDeclaration> {
    let mut position = Position::Type(evidence.root_type?);
    for segment in path {
        position = position.descend(unit, segment)?;
    }
    position.field_declaration(unit, field)
}

pub type Establishment = RootProducer;

pub fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> super::DominatorSets {
    let blocks: Vec<_> = block_ids.collect::<BTreeSet<_>>().into_iter().collect();
    let entry_position = blocks.binary_search(&entry).ok();
    // The entry's incoming row and extra map rows were never consulted. Missing
    // nonentry rows or foreign predecessors retain the original invariant errors.
    let incoming: Vec<Vec<_>> = blocks
        .iter()
        .map(|block| {
            if *block == entry {
                Vec::new()
            } else {
                predecessors
                    .get(block)
                    .expect("all blocks indexed")
                    .iter()
                    .map(|predecessor| {
                        blocks
                            .binary_search(predecessor)
                            .expect("predecessor block is declared")
                    })
                    .collect()
            }
        })
        .collect();
    super::dominance::solve(blocks, entry_position, incoming)
}

pub(crate) fn roster_cases(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&[StructuralCaseDeclaration]> {
    match shape(unit, type_id)? {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            Some(cases.as_slice())
        }
        _ => None,
    }
}
