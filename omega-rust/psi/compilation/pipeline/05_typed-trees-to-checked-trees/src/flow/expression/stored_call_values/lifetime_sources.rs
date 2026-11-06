//! A helper's saved output can keep several inputs borrowed even when only one
//! supplies its address. Save every input before the helper changes its output;
//! reading that variable later would observe a different reference generation.

use super::super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowCallFact, FlowOperandFact, FlowOperandRetainedSupplier, FlowOperandStoredSource,
    FlowStorageSnapshot,
};
use facts::PlaceSegment;

impl Execution<'_, '_, '_> {
    pub(in crate::flow::expression) fn capture_stored_lifetime_sources(
        &mut self,
        input: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        snapshot: Handle<FlowStorageSnapshot>,
    ) -> Option<HandleSpan<FlowOperandStoredSource>> {
        let actual = self.context.control.operands.get(input);
        let carrier = validation::reference_result_custody::input_carrier_type(
            self.program,
            actual.type_reference,
        )?;
        let path = validation::corresponding_place_projection(self.program, carrier, path)?;
        let selected =
            validation::project_type_reference_from_segments(self.program, carrier, &path)?;
        let leaves = if validation::reference_shell(self.program, selected).is_some() {
            vec![Vec::new()]
        } else {
            validation::borrow_contracts::carried_reference_paths(self.program, selected)?
                .into_iter()
                .map(|(path, _)| crate::borrow::owner_place_segments(&path))
                .collect()
        };
        if leaves.is_empty() {
            return None;
        }
        let mut sources = Vec::new();
        for leaf in leaves {
            let mut full_path = path.clone();
            full_path.extend_from_slice(&leaf);
            let selected = validation::completed_storage::stored_reference_sources_at_path(
                self.program,
                &self.context.control,
                input,
                &full_path,
                snapshot,
            );
            if matches!(
                selected,
                validation::completed_storage::StoredOperand::Unavailable
            ) && ::validation::trace_flags::check_sites()
            {
                let actual = self.context.control.operands.get(input);
                eprintln!(
                    "CHECKSITE stored-lifetime-input-unavailable state={:?} statement={} input={input:?} expression={:?} type={:?} path={full_path:?} snapshot={snapshot:?} referents={:?} retention={:?}",
                    self.state.symbol,
                    self.statement_index,
                    actual.expression,
                    actual.type_reference,
                    self.context
                        .control
                        .operand_referents
                        .span_or_empty(actual.referents),
                    actual.retention
                );
                if self
                    .context
                    .control
                    .operand_retentions
                    .is_valid(actual.retention)
                {
                    eprintln!(
                        "CHECKSITE stored-lifetime-input-profile input={input:?} profile={:?}",
                        self.context
                            .control
                            .operand_retentions
                            .get(actual.retention)
                    );
                }
            }
            let leaf = self
                .context
                .control
                .operand_retained_paths
                .insert_many(leaf);
            self.append_stored_lifetime_sources(selected, leaf, &mut sources)?;
        }
        Some(
            self.context
                .control
                .operand_stored_sources
                .insert_many(sources),
        )
    }

    fn append_stored_lifetime_sources(
        &mut self,
        selected: validation::completed_storage::StoredOperand,
        path: HandleSpan<PlaceSegment>,
        sources: &mut Vec<FlowOperandStoredSource>,
    ) -> Option<()> {
        use validation::completed_storage::StoredOperand;
        let slot = |this: &mut Self, place: validation::CanonicalPlace| {
            checked_trees::FlowOperandReferent {
                root: place.root,
                segments: this
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments),
                ..Default::default()
            }
        };
        match selected {
            StoredOperand::Unavailable => return None,
            StoredOperand::Alternatives(alternatives) => {
                if alternatives.is_empty() {
                    return None;
                }
                for alternative in alternatives {
                    self.append_stored_lifetime_sources(alternative, path, sources)?;
                }
            }
            StoredOperand::Incoming(place) => sources.push(FlowOperandStoredSource::Incoming {
                path,
                slot: slot(self, place),
            }),
            StoredOperand::Value {
                capture,
                reference,
                projection,
                slot: place,
            } => {
                let projection = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(projection);
                let slot = place.map(|place| slot(self, place));
                sources.push(FlowOperandStoredSource::Value {
                    path,
                    operand: capture,
                    reference,
                    projection,
                    slot,
                });
            }
        }
        Some(())
    }

    pub(super) fn capture_output_lifetime_sources(
        &mut self,
        parameters: &[typed_trees::signature::StateParameter],
        inputs: HandleSpan<FlowOperandFact>,
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
                let input = inputs.handles().nth(source.parameter_index)?;
                let source_path = crate::borrow::owner_place_segments(&source.source_path);
                let stored_sources =
                    if source.source == checked_trees::ViewReturnInputSource::Stored {
                        self.capture_stored_lifetime_sources(
                            input,
                            &source_path,
                            self.storage_snapshot,
                        )?
                    } else {
                        HandleSpan::empty()
                    };
                let projection = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(source_path.iter().copied());
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
                    operand: input,
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
                        stored_sources,
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
