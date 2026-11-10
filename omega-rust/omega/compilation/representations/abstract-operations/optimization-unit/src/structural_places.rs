/*
We provide structural-place, dominance, and field-value evidence queries over a
`PsiOptimizationFunction` (`FunctionAnalysis`, `FieldEvidence`, `dominators`,
`established_case_at`, `field_value_reads`, and `sole_case_at_path`). Both
specialization passes and their independent replay validators query these routines
so neither side duplicates CFG dominance walks or structural path resolution.
*/

use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerType, IntegerValue, OperationId,
    PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    exclusive_borrow_pair_pattern, record_or_mixed_shape_pattern,
};

use super::{
    FieldValueResolution, FieldValueRow, FoldedFieldValue, ForwardedFieldValue, NodeLocation,
    OptimizationBlock, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, ScalarConstantValue,
    ValueDefinition, ValueDefinitionSite,
};

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

pub fn operation_result_placement(
    function: &PsiOptimizationFunction,
    result: &StructuralOperationResult,
    producer: OperationId,
) -> bool {
    function
        .structural_place(result.place)
        .is_some_and(|place| {
            matches!(
                place.kind,
                StructuralPlaceKind::OperationResult {
                    producer: declared,
                    structural_type,
                } if declared == producer && structural_type == result.structural_type
            )
        })
}

pub(crate) fn establishment_fields(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
) -> Option<&[terminal_psi::RecordFieldInitializer]> {
    function
        .block_nodes()
        .find_map(|node| match &node.operation {
            abstract_operations::establish_record_or_case_pattern!(
                psi_operation,
                result,
                fields,
                ..
            ) if *psi_operation == producer && result.place == place => Some(fields.as_slice()),
            _ => None,
        })
}

pub fn structural_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<PlaceId> {
    establishment_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Structural(argument)
                if argument.path.is_empty()
                    && argument.access == terminal_psi::StructuralAccess::Owned =>
            {
                Some(argument.place)
            }
            _ => None,
        })
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

pub(crate) fn descended_field(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: TerminalIdentityIndex,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.identity == identity)
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

pub(crate) fn canonicalize_ordinary_path(
    unit: &PsiOptimizationUnit,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::AllElements => return None,
            StructuralPathSegment::Field(identity) => {
                let fields = record_fields(unit, current)?;
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                let next = field.field_type.as_structural()?;
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                current = next;
            }
            StructuralPathSegment::FixedIndex(index) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } if index < length => {
                    canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                    current = *element;
                }
                _ => return None,
            },
            StructuralPathSegment::Case(_)
            | StructuralPathSegment::RuntimeIndex { .. }
            | StructuralPathSegment::RepresentationView { .. }
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedRange { .. } => return None,
        }
    }
    Some(canonical)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedFieldKind {
    Boolean,
    Integer,
}

impl ObservedFieldKind {
    pub fn of(scalar_type: ScalarType) -> Option<Self> {
        match scalar_type {
            ScalarType::Boolean => Some(Self::Boolean),
            ScalarType::Integer(_) => Some(Self::Integer),
            ScalarType::IeeeFloat(_) => None,
        }
    }
}

pub struct FunctionAnalysis {
    pub definitions: BTreeMap<ValueId, ValueDefinition>,
    pub dominators: super::DominatorSets,
}

pub fn function_analysis(function: &PsiOptimizationFunction) -> FunctionAnalysis {
    let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect();
    let mut definitions: BTreeMap<ValueId, ValueDefinition> = BTreeMap::new();
    for parameter in &function.parameters {
        definitions.insert(parameter.value, *parameter);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            definitions.insert(parameter.value, *parameter);
        }
        for node in &block.nodes {
            for definition in &node.definitions {
                definitions.insert(definition.value, *definition);
            }
            for edge in &node.successors {
                predecessors
                    .get_mut(&edge.target)
                    .expect("validated successor target")
                    .insert(block.id);
            }
        }
    }
    FunctionAnalysis {
        definitions,
        dominators: dominators(
            function.entry,
            function.blocks.iter().map(|block| block.id),
            &predecessors,
        ),
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
                let fields = record_fields(unit, current)?;
                match fields
                    .iter()
                    .find(|field| field.id == *identity)?
                    .field_type
                {
                    StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::FixedIndex(_)) => {
                match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::Case(identity)) => {
                let cases = match shape(unit, current)? {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                cases
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EstablishmentScalar {
    Initializer(OperationId, ValueId),
    Zeroed(OperationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenFieldValue {
    pub producer: Option<OperationId>,
    pub resolution: FieldValueResolution,
}

pub(crate) fn record_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    establishment_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Scalar { value, .. } => Some(*value),
            _ => None,
        })
}

pub(crate) fn case_field_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    establishment_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match initializer.value {
            terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
            terminal_psi::RecordFieldValue::Structural(_) => None,
        })
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

pub(crate) fn matches_observed_kind(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> bool {
    matches!(
        (kind, declaration.field_type.scalar_type()),
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean))
            | (ObservedFieldKind::Integer, Some(ScalarType::Integer(_)))
    )
}

pub fn bound_value(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    if kind != ObservedFieldKind::Integer {
        return None;
    }
    let StructuralFieldType::BoundedInteger(bound) = declaration.field_type else {
        return None;
    };
    (bound.minimum() == bound.maximum()).then_some(ProvenFieldValue {
        producer: None,
        resolution: FieldValueResolution::Constant(FoldedFieldValue::Integer(bound.minimum())),
    })
}

pub(crate) fn zeroed_fold(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> Option<FoldedFieldValue> {
    match (kind, declaration.field_type.scalar_type()) {
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean)) => {
            Some(FoldedFieldValue::Boolean(false))
        }
        (ObservedFieldKind::Integer, Some(ScalarType::Integer(integer_type))) => {
            Some(FoldedFieldValue::Integer(
                if integer_type.sign() == semantic_vocabulary::IntegerSign::Signed {
                    semantic_vocabulary::IntegerValue::Signed(0)
                } else {
                    semantic_vocabulary::IntegerValue::Unsigned(0)
                },
            ))
        }
        _ => None,
    }
}

pub(crate) fn constant_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<FoldedFieldValue> {
    let mut current = value;
    let mut visiting = std::collections::BTreeSet::new();
    loop {
        if !visiting.insert(current) {
            return None;
        }
        if let Some(node) = function.block_nodes().find(|node| {
            node.definitions
                .iter()
                .any(|definition| definition.value == current)
        }) {
            return match &node.operation {
                O::BooleanConstant {
                    value: constant, ..
                } => Some(FoldedFieldValue::Boolean(*constant)),
                O::IntegerConstant {
                    value: constant, ..
                } => Some(FoldedFieldValue::Integer(*constant)),
                _ => None,
            };
        }
        let owner = function.blocks.iter().find(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.value == current)
        })?;
        let mut argument = None;
        for edge in function
            .block_edges()
            .filter(|edge| edge.target == owner.id)
        {
            let binding = edge.binding(current)?;
            match argument {
                None => argument = Some(binding.argument),
                Some(seen) if seen != binding.argument => return None,
                _ => {}
            }
        }
        current = argument?;
    }
}

pub(crate) fn matches_constant_kind(value: FoldedFieldValue, kind: ObservedFieldKind) -> bool {
    matches!(
        (value, kind),
        (FoldedFieldValue::Boolean(_), ObservedFieldKind::Boolean)
            | (FoldedFieldValue::Integer(_), ObservedFieldKind::Integer)
    )
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

pub(crate) fn descended_type(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Option<StructuralTypeId> {
    let shape = &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == current)?
        .shape;
    match segment {
        StructuralPathSegment::AllElements => None,
        StructuralPathSegment::Case(_) | StructuralPathSegment::FixedRange { .. } => None,
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => terminal_semantics::representation_view_declaration(
            unit.structural_types.iter(),
            *structural_type,
            *stored_scalar,
        )
        .map(|declaration| declaration.id),
        StructuralPathSegment::Field(identity) => {
            let fields = record_fields(unit, current)?;
            match terminal_psi::field_by_identity(fields, *identity)?.field_type {
                StructuralFieldType::Structural(next) => Some(next),
                _ => None,
            }
        }
        StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
            match shape {
                StructuralTypeShape::FixedArray { element, .. } => Some(*element),
                _ => None,
            }
        }
        StructuralPathSegment::Referent => match shape {
            StructuralTypeShape::Reference { referent, .. } => Some(*referent),
            _ => None,
        },
    }
}

pub(crate) fn comparison_operands(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    ordering: IntegerComparisonOrdering,
    left: ValueId,
    right: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let left_is_parameter = block.parameters.iter().any(|p| p.value == left);
    let right_is_parameter = block.parameters.iter().any(|p| p.value == right);
    let (parameter, bound, parameter_on_left) = match (left_is_parameter, right_is_parameter) {
        (true, false) => (left, right, true),
        (false, true) => (right, left, false),
        _ => return None,
    };
    let (bound_value, integer_type) = prefix.iter().find_map(|node| {
        let O::IntegerConstant {
            result,
            scalar_type,
            value,
            ..
        } = &node.operation
        else {
            return None;
        };
        if *result != bound {
            return None;
        }
        let integer_type = scalar_type.as_integer()?;
        Some((*value, integer_type))
    })?;
    if !block.parameters.iter().any(|declaration| {
        declaration.value == parameter
            && declaration.scalar_type == ScalarType::Integer(integer_type)
    }) {
        return None;
    }
    Some((
        parameter,
        DispatchCondition::IntegerComparison {
            ordering,
            bound: bound_value,
            parameter_on_left,
            integer_type,
        },
    ))
}

pub fn integer_comparison_condition(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    condition: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let mut resolved = None;
    for node in prefix {
        let (ordering, result, left, right) = match &node.operation {
            O::IntegerConstant { .. } | O::BooleanConstant { .. } => continue,
            O::IntegerEqual {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::Equal, *result, *left, *right),
            O::IntegerLessThan {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::LessThan, *result, *left, *right),
            O::IntegerLessOrEqual {
                result,
                left,
                right,
                ..
            } => (
                IntegerComparisonOrdering::LessOrEqual,
                *result,
                *left,
                *right,
            ),
            _ => return None,
        };
        if result != condition || resolved.is_some() {
            return None;
        }
        resolved = Some(comparison_operands(block, prefix, ordering, left, right)?);
    }
    resolved
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerComparisonOrdering {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchCondition {
    DirectParameter,
    IntegerComparison {
        ordering: IntegerComparisonOrdering,
        bound: IntegerValue,
        parameter_on_left: bool,
        integer_type: IntegerType,
    },
}

impl DispatchCondition {
    pub fn resolve(&self, constant: &ScalarConstantValue) -> Option<bool> {
        match (self, constant) {
            (Self::DirectParameter, ScalarConstantValue::Boolean(value)) => Some(*value),
            (
                Self::IntegerComparison {
                    ordering,
                    bound,
                    parameter_on_left,
                    integer_type,
                },
                ScalarConstantValue::Integer(argument),
            ) => {
                let (left, right) = if *parameter_on_left {
                    (*argument, *bound)
                } else {
                    (*bound, *argument)
                };
                let order = integer_type.compare(left, right)?;
                Some(match ordering {
                    IntegerComparisonOrdering::Equal => order == Ordering::Equal,
                    IntegerComparisonOrdering::LessThan => order == Ordering::Less,
                    IntegerComparisonOrdering::LessOrEqual => order != Ordering::Greater,
                })
            }
            _ => None,
        }
    }
}

pub fn reachable_blocks_after_conditional_fold(
    function: &PsiOptimizationFunction,
    source: BlockId,
    selected_edge: EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let block = function.block(block_id)?;
        for edge in block.successors() {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

pub fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.block_parameters())
        .chain(function.node_definitions())
        .copied()
        .find(|definition| definition.value == value)
}

pub(crate) fn roster_cases(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&[StructuralCaseDeclaration]> {
    match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            Some(cases.as_slice())
        }
        _ => None,
    }
}

pub(crate) fn roster_first_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = roster_cases(unit, type_id)?;
    cases.first().map(|case| case.id)
}

pub(crate) fn roster_sole_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = roster_cases(unit, type_id)?;
    let [sole] = cases else {
        return None;
    };
    Some(sole.id)
}

pub fn sole_case_at_path(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    resolved_sole_case(unit, declared_structural_type(function, declaration)?, path)
}

pub(crate) fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralCaseId> {
    sole_case_at_path(unit, function, declaration, &[])
}

pub fn resolved_sole_case(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    let mut current = root;
    for segment in path {
        current = descended_type(unit, current, segment)?;
    }
    roster_sole_case(unit, current)
}

pub(crate) fn descended_field_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
    next: StructuralTypeId,
    visiting: &mut BTreeSet<PlaceId>,
) -> Option<(PlaceId, Establishment)> {
    let child = structural_child(function, producer, place, field)?;
    let declaration = function.structural_place(child)?;
    if declared_structural_type(function, declaration) != Some(next) || !visiting.insert(child) {
        return None;
    }
    Some((child, root_producer(function, declaration)))
}

pub(crate) fn leaf_copy_source(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(PlaceId, &StructuralCopyPath)> {
    let StructuralPlaceKind::OperationResult { producer, .. } =
        function.structural_place(place)?.kind
    else {
        return None;
    };
    function
        .block_nodes()
        .find_map(|node| match &node.operation {
            O::StructuralLeafCopy {
                psi_operation,
                result,
                source,
                path,
            } if *psi_operation == producer && result.place == place => {
                if matches!(path, StructuralCopyPath::Ordinary(path) if path.is_empty()) {
                    let source_type =
                        declared_structural_type(function, function.structural_place(*source)?);
                    if source_type != Some(result.structural_type) {
                        return None;
                    }
                }
                Some((*source, path))
            }
            _ => None,
        })
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PathStep<'a> {
    Ordinary(&'a StructuralPathSegment),
    Canonical(CanonicalStructuralPathSegment),
}
impl<'a> Position<'a> {
    pub(crate) fn descend_step(
        self,
        unit: &'a PsiOptimizationUnit,
        step: PathStep<'_>,
    ) -> Option<Position<'a>> {
        match (self, step) {
            (Self::Type(_), PathStep::Canonical(CanonicalStructuralPathSegment::Referent)) => None,
            (Self::Type(current), PathStep::Ordinary(segment)) => match segment {
                StructuralPathSegment::AllElements => None,
                StructuralPathSegment::Field(identity) => {
                    descended_field(unit, current, *identity).map(|(_, next)| Self::Type(next))
                }
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. } => match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                },
                StructuralPathSegment::Referent
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::RepresentationView { .. }
                | StructuralPathSegment::FixedRange { .. } => None,
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => descended_field_id(unit, current, identity).map(|(_, next)| Self::Type(next)),
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::FixedIndex(_)),
            ) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                _ => None,
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Case(identity)),
            ) => {
                let cases = match shape(unit, current)? {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                cases
                    .iter()
                    .find(|case| case.id == identity)
                    .map(Self::CasePayload)
            }
            (
                Self::CasePayload(case),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => match case
                .fields
                .iter()
                .find(|field| field.id == identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                _ => None,
            },
            (Self::CasePayload(_), _) => None,
        }
    }
}

pub fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<(StructuralCaseId, Option<OperationId>)> {
    if let Some((producer, case)) = established_case_at(unit, function, declaration, &[]) {
        return Some((case, Some(producer)));
    }
    sole_case(unit, function, declaration).map(|case| (case, None))
}
pub fn established_case_at(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<(OperationId, StructuralCaseId)> {
    let mut place = declaration.id;
    let mut producer = root_producer(function, declaration);
    let mut declared = declared_structural_type(function, declaration);
    let mut segments: Vec<PathStep> = path.iter().map(PathStep::Ordinary).collect();
    let mut visiting = BTreeSet::from([place]);
    loop {
        match (producer, segments.as_slice()) {
            (Establishment::Variant(producer, case), []) => {
                return Some((producer, case));
            }
            (Establishment::Zeroed(operation), _) => {
                let mut position = Position::Type(declared?);
                for step in &segments {
                    position = position.descend_step(unit, *step)?;
                }
                let Position::Type(terminal) = position else {
                    return None;
                };
                return roster_first_case(unit, terminal).map(|case| (operation, case));
            }
            (
                Establishment::Variant(operation, result_case),
                [
                    PathStep::Canonical(CanonicalStructuralPathSegment::Case(observed_case)),
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
                    ..,
                ],
            ) if result_case == *observed_case => {
                let identity = *identity;
                let Position::Type(next) = Position::Type(declared?)
                    .descend_step(unit, segments[0])?
                    .descend_step(unit, segments[1])?
                else {
                    return None;
                };
                let (child, child_producer) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = child_producer;
                declared = Some(next);
                segments.drain(..2);
            }
            (Establishment::Record(operation), [step, ..]) => {
                let (field, next) = match step {
                    PathStep::Ordinary(StructuralPathSegment::Field(identity)) => {
                        descended_field(unit, declared?, *identity)?
                    }
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)) => {
                        descended_field_id(unit, declared?, *identity)?
                    }
                    _ => return None,
                };
                let (child, child_producer) =
                    descended_field_child(function, operation, place, field, next, &mut visiting)?;
                place = child;
                producer = child_producer;
                declared = Some(next);
                segments.drain(..1);
            }
            (Establishment::None, _) => {
                if let Some((source, spelled)) = leaf_copy_source(function, place) {
                    let source_declaration = function.structural_place(source)?;
                    let source_type = declared_structural_type(function, source_declaration)?;
                    let copied = match spelled {
                        StructuralCopyPath::Case(path) => path.clone(),
                        StructuralCopyPath::Ordinary(path) => {
                            canonicalize_ordinary_path(unit, source_type, path)?
                        }
                    };
                    let mut position = Position::Type(source_type);
                    for segment in &copied {
                        position = position.descend_step(unit, PathStep::Canonical(*segment))?;
                    }
                    if !matches!(position, Position::Type(landed) if Some(landed) == declared) {
                        return None;
                    }
                    if !visiting.insert(source) {
                        return None;
                    }
                    place = source;
                    producer = root_producer(function, source_declaration);
                    declared = Some(source_type);
                    segments.splice(..0, copied.iter().copied().map(PathStep::Canonical));
                    continue;
                }
                let bound = bound_place(function, place)?;
                let bound_declaration = function.structural_place(bound)?;
                let bound_type = declared_structural_type(function, bound_declaration);
                if declared.is_some() && bound_type != declared {
                    return None;
                }
                if !visiting.insert(bound) {
                    return None;
                }
                place = bound;
                producer = root_producer(function, bound_declaration);
                declared = bound_type;
            }
            _ => return None,
        }
    }
}
pub fn bound_place(function: &PsiOptimizationFunction, place: PlaceId) -> Option<PlaceId> {
    let StructuralPlaceKind::BlockParameter { block, .. } = function.structural_place(place)?.kind
    else {
        return None;
    };
    if place_is_rewritten(function, place) {
        return None;
    }
    let mut bound = None;
    for edge in function.block_edges().filter(|edge| edge.target == block) {
        let binding = edge.structural_binding(place)?;
        let argument = &binding.argument;
        if !argument.path.is_empty()
            || !matches!(
                argument.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
        {
            return None;
        }
        match bound {
            None => bound = Some(argument.place),
            Some(seen) if seen != argument.place => return None,
            _ => {}
        }
    }
    bound
}
fn place_is_rewritten(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().any(|node| {
            operation_rewrites_place(&node.operation, place)
                || node.successors.iter().any(|edge| {
                    edge.structural_bindings
                        .iter()
                        .any(|binding| argument_rewrites_place(&binding.argument, place))
                })
        })
    })
}
pub fn argument_rewrites_place(argument: &StructuralArgument, place: PlaceId) -> bool {
    argument.place == place
        && (!argument.path.is_empty()
            || matches!(argument.access, exclusive_borrow_pair_pattern!()))
}
fn operation_rewrites_place(operation: &O, place: PlaceId) -> bool {
    if match operation {
        O::PrimitiveLocalStore { destination, .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(destination, ..)
        | abstract_operations::byte_sequence_store_pair_pattern!(destination, ..)
        | O::EstablishElementView { destination, .. } => *destination == place,
        O::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == place,
        O::StoreStructuralField { destination, .. } => destination.place() == place,
        O::StructuralScalarFieldStore { destination, .. } => destination.place() == place,
        O::MoveStructuralField { source, .. } => source.place() == place,
        O::AtomicEvent { event, .. } => event.place() == Some(place),
        _ => false,
    } {
        return true;
    }
    structural_argument_operands(operation).any(|argument| argument_rewrites_place(argument, place))
}
fn structural_argument_operands(operation: &O) -> impl Iterator<Item = &StructuralArgument> {
    let mut operands: Vec<&StructuralArgument> = Vec::new();
    match operation {
        O::EstablishStructuralArray { elements, .. } => operands.extend(elements),
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
            for initializer in fields {
                if let Some(argument) = initializer.value.as_structural() {
                    operands.push(argument);
                }
            }
        }
        O::EstablishElementView { source, .. } | O::EstablishReference { source, .. } => {
            operands.push(source);
        }
        O::StoreStructuralField { value, .. } => operands.push(value),
        abstract_operations::call_structural_or_boundary_call_pattern!(structural_arguments) => {
            operands.extend(structural_arguments.iter())
        }
        _ => {}
    }
    operands.into_iter()
}
fn establishment_scalar(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<EstablishmentScalar> {
    let mut place = evidence.declaration.id;
    let mut producer = evidence.root_producer;
    let mut declared = evidence.root_type;
    let mut segments = path.to_vec();
    let mut visiting = BTreeSet::from([place]);
    loop {
        let crossing = match (producer, segments.as_slice()) {
            (
                RootProducer::Variant(operation, result_case),
                [
                    CanonicalStructuralPathSegment::Case(observed_case),
                    CanonicalStructuralPathSegment::Field(identity),
                    ..,
                ],
            ) if result_case == *observed_case => Some((operation, *identity, 2)),
            (
                RootProducer::Record(operation),
                [CanonicalStructuralPathSegment::Field(identity), ..],
            ) => Some((operation, *identity, 1)),
            _ => None,
        };
        if let Some((operation, identity, depth)) = crossing {
            let mut position = Position::Type(declared?);
            for segment in &segments[..depth] {
                position = position.descend(unit, segment)?;
            }
            let Position::Type(next) = position else {
                return None;
            };
            let (child, child_producer) =
                descended_field_child(function, operation, place, identity, next, &mut visiting)?;
            place = child;
            producer = child_producer;
            declared = Some(next);
            segments.drain(..depth);
            continue;
        }
        match (producer, segments.as_slice()) {
            (RootProducer::Record(producer), []) => {
                return record_initializer(function, producer, place, field)
                    .map(|initializer| EstablishmentScalar::Initializer(producer, initializer));
            }
            (RootProducer::Zeroed(operation), _) => {
                let mut position = Position::Type(declared?);
                for segment in &segments {
                    position = position.descend(unit, segment)?;
                }
                position.field_declaration(unit, field)?;
                return Some(EstablishmentScalar::Zeroed(operation));
            }
            (
                RootProducer::Variant(producer, result_case),
                [CanonicalStructuralPathSegment::Case(observed_case)],
            ) if result_case == *observed_case => {
                return case_field_initializer(function, producer, place, field)
                    .map(|initializer| EstablishmentScalar::Initializer(producer, initializer));
            }
            (RootProducer::None, _) => {
                if let Some((source, spelled)) = leaf_copy_source(function, place) {
                    let declaration = function.structural_place(source)?;
                    let source_type = declared_structural_type(function, declaration)?;
                    let copied = match spelled {
                        StructuralCopyPath::Case(path) => path.clone(),
                        StructuralCopyPath::Ordinary(path) => {
                            canonicalize_ordinary_path(unit, source_type, path)?
                        }
                    };
                    let mut position = Position::Type(source_type);
                    for segment in &copied {
                        position = position.descend(unit, segment)?;
                    }
                    if !matches!(position, Position::Type(landed) if Some(landed) == declared) {
                        return None;
                    }
                    if !visiting.insert(source) {
                        return None;
                    }
                    place = source;
                    producer = root_producer(function, declaration);
                    declared = Some(source_type);
                    segments.splice(..0, copied.iter().copied());
                    continue;
                }
                let bound = bound_place(function, place)?;
                let declaration = function.structural_place(bound)?;
                let bound_type = declared_structural_type(function, declaration);
                if declared.is_some() && bound_type != declared {
                    return None;
                }
                if !visiting.insert(bound) {
                    return None;
                }
                place = bound;
                producer = root_producer(function, declaration);
                declared = bound_type;
            }
            _ => return None,
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn proven_field_value(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    let declaration = observed_field_declaration(unit, evidence, path, field)?;
    if !matches_observed_kind(declaration, kind) {
        return None;
    }
    let establishment = establishment_scalar(unit, evidence, function, path, field);
    match establishment {
        Some(EstablishmentScalar::Zeroed(producer)) => {
            return Some(ProvenFieldValue {
                producer: Some(producer),
                resolution: FieldValueResolution::Constant(zeroed_fold(declaration, kind)?),
            });
        }
        Some(EstablishmentScalar::Initializer(producer, initializer)) => {
            if let Some(value) = constant_definition(function, initializer)
                && matches_constant_kind(value, kind)
            {
                return Some(ProvenFieldValue {
                    producer: Some(producer),
                    resolution: FieldValueResolution::Constant(value),
                });
            }
        }
        None => {}
    }
    if let Some(proven) = bound_value(declaration, kind) {
        return Some(proven);
    }
    let EstablishmentScalar::Initializer(producer, initializer) = establishment? else {
        return None;
    };
    forwarded_field_value(
        function,
        analysis,
        block,
        node_index,
        node,
        result,
        scalar_type,
        initializer,
    )
    .map(|forwarded| ProvenFieldValue {
        producer: Some(producer),
        resolution: FieldValueResolution::Forward(forwarded),
    })
}

fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FieldValueRow> {
    let O::StructuralScalarFieldRead {
        psi_operation,
        result,
        source,
        path,
        field,
    } = &node.operation
    else {
        return None;
    };
    let kind = ObservedFieldKind::of(result.scalar_type)?;
    let (result, scalar_type) = (result.value, result.scalar_type);
    if *source != evidence.declaration.id {
        return None;
    }
    let proven = proven_field_value(
        unit,
        evidence,
        function,
        analysis,
        block,
        node_index,
        node,
        result,
        scalar_type,
        path,
        *field,
        kind,
    )?;
    Some(FieldValueRow {
        site: NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(node_index).ok()?,
        },
        psi_operation: *psi_operation,
        result,
        source: evidence.declaration.id,
        path: path.clone(),
        field: *field,
        producer: proven.producer,
        resolution: proven.resolution,
    })
}

pub fn field_value_reads(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FieldValueRow> {
    let analysis = function_analysis(function);
    let mut reads = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) =
                admit_field_node(unit, evidence, function, &analysis, block, node_index, node)
            else {
                continue;
            };
            reads.push(row);
        }
    }
    reads.sort_by_key(|row| row.site);
    reads
}

#[allow(clippy::too_many_arguments)]
fn forwarded_field_value(
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    initializer: ValueId,
) -> Option<ForwardedFieldValue> {
    if initializer == result {
        return None;
    }
    let definition = analysis.definitions.get(&initializer)?;
    if definition.scalar_type != scalar_type {
        return None;
    }
    if !retirable_scalar_node(block, node_index, node, result, scalar_type) {
        return None;
    }
    if !receiver_custody_distinct(block, node_index, node) {
        return None;
    }
    let mut uses = BTreeSet::new();
    for use_block in &function.blocks {
        for (use_index, use_node) in use_block.nodes.iter().enumerate() {
            let referenced = use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == result);
            if !referenced {
                continue;
            }
            if use_node.operation.operates_on_byte_sequence_storage()
                || use_node
                    .operation
                    .runtime_indices()
                    .iter()
                    .any(|(index, _)| *index == result)
            {
                return None;
            }
            let site = NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(use_index).ok()?,
            };
            let dominates = match definition.site {
                ValueDefinitionSite::FunctionParameter(_) => true,
                ValueDefinitionSite::BlockParameter {
                    block: defining, ..
                } => analysis
                    .dominators
                    .get(&use_block.id)
                    .is_some_and(|set| set.contains(&defining)),
                ValueDefinitionSite::Node {
                    block: defining,
                    node: defined_at,
                } => {
                    if defining == use_block.id {
                        defined_at < site.node
                    } else {
                        analysis
                            .dominators
                            .get(&use_block.id)
                            .is_some_and(|set| set.contains(&defining))
                    }
                }
            };
            if !dominates {
                return None;
            }
            uses.insert(site);
        }
    }
    Some(ForwardedFieldValue {
        initializer,
        scalar_type,
        uses: uses.into_iter().collect(),
    })
}

pub fn retirable_scalar_node(
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
) -> bool {
    let Ok(coordinate) = u32::try_from(node_index) else {
        return false;
    };
    let definition = ValueDefinition {
        value: result,
        scalar_type,
        site: ValueDefinitionSite::Node {
            block: block.id,
            node: coordinate,
        },
    };
    node.definitions == [definition]
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && node_index
            .checked_add(1)
            .is_some_and(|next| block.nodes.get(next).is_some())
}

pub fn receiver_custody_distinct(
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
) -> bool {
    node_index.checked_add(1).is_some_and(|next| {
        block.nodes.get(next).is_some_and(|receiver| {
            !receiver
                .provenance
                .iter()
                .any(|source| node.provenance.contains(source))
        })
    })
}

pub fn field_value_accounting(
    input_function: &PsiOptimizationFunction,
    rows: &[FieldValueRow],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let machine = input_function.machine;
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    let mut fold_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    let mut use_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    for row in rows {
        match &row.resolution {
            FieldValueResolution::Constant(_) => {
                fold_sites.insert((row.site.block, row.site.node));
            }
            FieldValueResolution::Forward(forwarded) => {
                removals
                    .entry(row.site.block)
                    .or_default()
                    .insert(row.site.node);
                for site in &forwarded.uses {
                    use_sites.insert((site.block, site.node));
                }
            }
        }
    }
    let earliest_removal = input_function
        .blocks
        .iter()
        .position(|block| removals.contains_key(&block.id));
    let mut affected = BTreeSet::new();
    let mut provenance = Vec::new();
    for (block_position, block) in input_function.blocks.iter().enumerate() {
        let removal = removals.get(&block.id);
        let first_removed = removal.and_then(|set| set.iter().next().copied());
        let in_suffix = earliest_removal.is_some_and(|position| block_position > position);
        let mut block_affected = removal.is_some() || in_suffix;
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).ok()?;
            let coordinate = (block.id, node_index);
            if fold_sites.contains(&coordinate) || use_sites.contains(&coordinate) {
                block_affected = true;
            }
            let retired = removal.is_some_and(|set| set.contains(&node_index));
            let shifted = first_removed.is_some_and(|first| node_index > first);
            if !retired
                && !shifted
                && !in_suffix
                && !fold_sites.contains(&coordinate)
                && !use_sites.contains(&coordinate)
            {
                continue;
            }
            if !retired && node.provenance.is_empty() {
                continue;
            }
            let input_site = PsiRealizationSite::Node(NodeLocation {
                machine,
                block: block.id,
                node: node_index,
            });
            let earlier_removals = removal
                .map(|set| set.iter().filter(|removed| **removed < node_index).count())
                .unwrap_or(0);
            let output_index = node_index.checked_sub(u32::try_from(earlier_removals).ok()?)?;
            let output_site = PsiRealizationSite::Node(NodeLocation {
                machine,
                block: block.id,
                node: output_index,
            });
            provenance.push(ProvenanceRewrite {
                input: input_site,
                disposition: ProvenanceDisposition::RealizedAt(output_site),
                sources: node.provenance.clone(),
                work: node.work.clone(),
            });
        }
        if block_affected {
            affected.insert(block.id);
        }
    }
    provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), provenance))
}
