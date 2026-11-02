//! Original typed input obligations survive independently of an output pointer.
//! The actual completed output supplies the generation; its applied declaration
//! supplies the source bounds. No current descriptor or receiving annotation
//! reconstructs the inputs after a later opaque write.

use super::super::Execution;
use checked_trees::{BorrowCallFact, FlowOperandFact, FlowOperandRetainedSupplier};
use facts::PlaceSegment;

impl Execution<'_, '_, '_> {
    pub(super) fn capture_output_lifetime_sources(
        &mut self,
        parameters: &[typed_trees::signature::StateParameter],
        inputs: &[FlowOperandFact],
        call: &BorrowCallFact,
        identity: facts::InvocationOutputIdentity,
        owner_path: &[PlaceSegment],
    ) {
        let captured = (|| {
            let output_parameter = parameters
                .iter()
                .position(|parameter| parameter.symbol == identity.parameter_symbol)?;
            let relation =
                validation::borrow_contracts::resolve_provider_outputs(self.program, parameters)
                    .ok()?;
            if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete
            {
                return None;
            }
            let output = relation.outputs.iter().find(|output| {
                output.destination
                    == validation::borrow_contracts::ProviderOutputDestination::Parameter(
                        output_parameter,
                    )
                    && crate::borrow::owner_place_segments(&output.owner_path) == owner_path
            })?;
            let mut suppliers = Vec::new();
            for source in &output.dependencies {
                let actual = *inputs.get(source.parameter_index)?;
                let input = self.context.control.operands.insert(actual);
                let source_path = crate::borrow::owner_place_segments(&source.source_path);
                let (operand, mut projection) = if source.source
                    == checked_trees::ViewReturnInputSource::Stored
                    && validation::reference_shell(self.program, actual.type_reference).is_some()
                {
                    if !self
                        .context
                        .control
                        .operand_retentions
                        .is_valid(actual.retention)
                    {
                        return None;
                    }
                    let profile = self
                        .context
                        .control
                        .operand_retentions
                        .get(actual.retention);
                    if !self.context.control.operands.is_valid(profile.stored_value) {
                        return None;
                    }
                    (
                        profile.stored_value,
                        self.context
                            .control
                            .operand_retained_paths
                            .span(profile.stored_projection)?
                            .to_vec(),
                    )
                } else {
                    (input, Vec::new())
                };
                projection.extend_from_slice(&source_path);
                let projection = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(projection);
                let source_path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(source_path);
                let owner_path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(owner_path.iter().copied());
                suppliers.push(FlowOperandRetainedSupplier {
                    operand,
                    projection,
                    contract: Some(checked_trees::FlowOperandLifetimeContract {
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        invocation: call.authored_expression,
                        call_ordinal: call.call_ordinal,
                        output_parameter: Some(output_parameter),
                        input,
                        parameter_index: source.parameter_index,
                        source: source.source,
                        source_type: source.source_type,
                        source_path,
                        owner_path,
                    }),
                });
            }
            Some(
                self.context
                    .control
                    .operand_retained_suppliers
                    .insert_many(suppliers),
            )
        })();
        let owner_path = self
            .context
            .control
            .operand_retained_paths
            .insert_many(owner_path.iter().copied());
        self.context.control.output_lifetime_sources.append(
            checked_trees::FlowOutputLifetimeSources {
                output: identity,
                owner_path,
                sources: captured,
            },
        );
    }
}
