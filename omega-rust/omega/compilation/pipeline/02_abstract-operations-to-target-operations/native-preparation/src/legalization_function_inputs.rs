//! Prepare ordered input rows once for one immutable optimization function.
//! Each query still checks the original role, shape and uniqueness requirements.

use crate::LegalizationError;
use abstract_operations::AbstractOperation;
use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
};

pub(crate) struct FunctionInputs<'function> {
    function: &'function PsiOptimizationFunction,
    values: Vec<&'function ValueDefinition>,
    declarations: Vec<&'function StructuralPlaceDeclaration>,
    arrivals: Vec<&'function StructuralParameterDeclaration>,
    block_arrivals: Vec<(PlaceId, BlockId, &'function StructuralParameterDeclaration)>,
    trivial_locals: Vec<(PlaceId, BlockId, usize, &'function AbstractOperation)>,
    results: Vec<(PlaceId, OperationId, &'function StructuralOperationResult)>,
    byte_literals: Vec<(PlaceId, OperationId, &'function AbstractOperation)>,
}

impl<'function> FunctionInputs<'function> {
    pub(crate) fn new(function: &'function PsiOptimizationFunction) -> Self {
        let mut values: Vec<_> = function.parameters.iter().collect();
        let mut declarations: Vec<_> = function.structural_places.iter().collect();
        let mut arrivals: Vec<_> = function.structural_parameters.iter().collect();
        let mut block_arrivals = Vec::new();
        let mut trivial_locals = Vec::new();
        let mut results = Vec::new();
        let mut byte_literals = Vec::new();
        for block in &function.blocks {
            values.extend(&block.parameters);
            arrivals.extend(&block.structural_parameters);
            block_arrivals.extend(
                block
                    .structural_parameters
                    .iter()
                    .map(|parameter| (parameter.place, block.id, parameter)),
            );
            for (position, node) in block.nodes.iter().enumerate() {
                values.extend(&node.definitions);
                if let AbstractOperation::EstablishTrivialAffineLocal { place, .. } =
                    &node.operation
                {
                    trivial_locals.push((place.id, block.id, position, &node.operation));
                }
                if let AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation,
                    place,
                    ..
                } = &node.operation
                {
                    byte_literals.push((place.id, *psi_operation, &node.operation));
                }
                if let Some((producer, result)) = node.operation.structural_result() {
                    results.push((result.place, producer, result));
                }
            }
        }
        values.sort_by_key(|row| row.value);
        declarations.sort_by_key(|row| row.id);
        arrivals.sort_by_key(|row| row.place);
        block_arrivals.sort_by_key(|row| row.0);
        trivial_locals.sort_by_key(|row| row.0);
        results.sort_by_key(|row| row.0);
        byte_literals.sort_by_key(|row| row.0);
        Self {
            function,
            values,
            declarations,
            arrivals,
            block_arrivals,
            trivial_locals,
            results,
            byte_literals,
        }
    }

    pub(crate) fn function(&self) -> &'function PsiOptimizationFunction {
        self.function
    }

    pub(crate) fn value_type(&self, value: ValueId) -> Option<ScalarType> {
        let first = self.values.partition_point(|row| row.value < value);
        self.values
            .get(first)
            .filter(|row| row.value == value)
            .map(|row| row.scalar_type)
    }

    pub(crate) fn trivial_affine_local(
        &self,
        place: PlaceId,
    ) -> Option<(OperationId, StructuralTypeId, (BlockId, usize))> {
        let first = self.trivial_locals.partition_point(|row| row.0 < place);
        let mut matching = self.trivial_locals[first..].iter().take_while(|row| row.0 == place)
            .filter_map(|(_, block, position, operation)| crate::legalization_scalar_graph::aggregate_results::owned_arguments::trivial_affine_candidate(*block, *position, operation, place));
        let establishment = matching.next()?;
        matching.next().is_none().then_some(establishment)
    }

    pub(crate) fn arrival_parameter(
        &self,
        place: PlaceId,
    ) -> Option<&'function StructuralParameterDeclaration> {
        let first = self.declarations.partition_point(|row| row.id < place);
        let mut declarations = self.declarations[first..]
            .iter()
            .take_while(|row| row.id == place);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let first = self.arrivals.partition_point(|row| row.place < place);
        let mut parameters = self.arrivals[first..]
            .iter()
            .take_while(|row| row.place == place);
        let parameter = *parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        match declaration.kind {
            StructuralPlaceKind::Parameter { position }
                if position == parameter.position
                    && self
                        .function
                        .structural_parameters
                        .iter()
                        .any(|row| row.place == place) => {}
            StructuralPlaceKind::BlockParameter { block, position }
                if block != self.function.entry
                    && position == parameter.position
                    && self
                        .function
                        .block(block)
                        .and_then(|block| block.structural_parameters.get(position as usize))
                        .is_some_and(|row| row.place == place) => {}
            _ => return None,
        }
        Some(parameter)
    }

    pub(crate) fn address_join_parameter(
        &self,
        place: PlaceId,
        plan: &crate::native_geometry::NativePlan<'_>,
    ) -> Option<(BlockId, &'function StructuralParameterDeclaration)> {
        let first = self.block_arrivals.partition_point(|row| row.0 < place);
        let declarations = self.declarations.partition_point(|row| row.id < place);
        crate::legalization_scalar_graph::address_joins::parameter_from_rows(
            self.function.entry,
            place,
            plan,
            self.block_arrivals[first..]
                .iter()
                .take_while(|row| row.0 == place)
                .map(|(_, block, parameter)| (*block, *parameter)),
            self.declarations[declarations..]
                .iter()
                .take_while(|row| row.id == place)
                .copied(),
        )
    }

    pub(crate) fn source_result(
        &self,
        place: PlaceId,
    ) -> Result<(OperationId, &'function StructuralOperationResult), LegalizationError> {
        let first = self.results.partition_point(|row| row.0 < place);
        let mut matching = self.results[first..]
            .iter()
            .take_while(|row| row.0 == place)
            .map(|(_, producer, result)| (*producer, *result));
        let result = matching.next().ok_or(LegalizationError::custody())?;
        if matching.next().is_some() {
            return Err(LegalizationError::custody());
        }
        Ok(result)
    }

    /// Find the one operation that establishes the byte literal `place`.
    /// The place must have exactly one declaration, of literal kind, and
    /// exactly one producer whose place and borrowed byte-view type match it;
    /// anything else is refused.
    pub(crate) fn byte_literal(&self, place: PlaceId) -> Option<(OperationId, StructuralTypeId)> {
        let first = self.declarations.partition_point(|row| row.id < place);
        let mut declarations = self.declarations[first..]
            .iter()
            .take_while(|row| row.id == place);
        let declaration = *declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type: identity,
            ..
        } = declaration.kind
        else {
            return None;
        };
        let first = self.byte_literals.partition_point(|row| row.0 < place);
        let mut producers = self.byte_literals[first..]
            .iter()
            .take_while(|row| row.0 == place);
        let (_, operation, establishment) = *producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        let AbstractOperation::EstablishByteSequenceLiteral {
            place: established,
            structural_type,
            ..
        } = establishment
        else {
            return None;
        };
        (established == declaration
            && structural_type.id == identity
            && structural_type.shape.is_borrowed_byte_view())
        .then_some((operation, identity))
    }
}
