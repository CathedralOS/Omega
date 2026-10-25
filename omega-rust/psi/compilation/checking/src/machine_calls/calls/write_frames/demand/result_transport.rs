//! Finite normal-result transport preserves whole-value moves beside formed
//! reference origins. Consumers must resolve a moved input against the exact
//! completed operand before publishing any payload; a type frontier alone
//! neither selects a case nor establishes a reference's backing.

use super::super::{LocalWriteSource, ReferenceResultLeafOrigin, ReferenceResultTransport};
use super::{CallFrameResolver, FrameInference, SymbolHandle, memoized};

impl CallFrameResolver<'_> {
    /// Complete chronological result relation. Callers must substitute each
    /// source through their own completed operands before publishing addresses.
    pub fn reference_result_transports(
        &self,
        symbol: SymbolHandle,
    ) -> Option<Vec<ReferenceResultTransport>> {
        memoized(&self.caches.reference_result_transports, symbol, || {
            self.result_transports_in_context(symbol, &mut FrameInference::default())
        })
    }

    /// A fixed actual may refine a relative selector, never the declared
    /// lifetime frontier. Specialised rows are invocation scratch and cannot
    /// enter the state-keyed transport memo.
    pub fn reference_result_leaf_origins_for_arguments(
        &self,
        symbol: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
    ) -> Option<Vec<ReferenceResultLeafOrigin>> {
        let ordinary = self.reference_result_leaf_origins(symbol)?;
        if !ordinary.iter().any(|leaf| {
            leaf.source
                .segments
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        }) {
            return Some(ordinary);
        }
        let state = self.program.state_by_symbol(symbol)?;
        if self.program.state_parameters(state).len() != arguments.len() {
            return None;
        }
        let mut inference = FrameInference::default();
        let indices = inference.argument_indices(self.program, arguments);
        if !indices.iter().any(Option::is_some) {
            return Some(ordinary);
        }
        inference.with_invocation(self.program, state, &[], &indices, |inference| {
            self.result_transports_in_context(symbol, inference)?
                .into_iter()
                .map(|transport| match transport {
                    ReferenceResultTransport::Reference(leaf) => Some(leaf),
                    ReferenceResultTransport::Value { .. } => None,
                })
                .collect()
        })
    }

    fn result_transports_in_context(
        &self,
        symbol: SymbolHandle,
        inference: &mut FrameInference,
    ) -> Option<Vec<ReferenceResultTransport>> {
        let state = self.program.state_by_symbol(symbol)?;
        let machine = self.program.machine_holding_state(symbol)?;
        let relation = super::super::result_origins::state_result_origins(
            self.program,
            machine,
            state,
            state.return_type,
            &self.tables.symbols,
            inference,
            true,
        )?;
        let mut transports = Vec::new();
        for leaf in relation.references {
            // This is a union of normal routes. A move on one route
            // cannot mask a constructed or unresolved leaf on another.
            // Unknown reference origins therefore still refuse here;
            // the capture consumer independently checks case presence.
            transports.push(ReferenceResultTransport::Reference(
                ReferenceResultLeafOrigin {
                    result_path: leaf.local_segments,
                    source: LocalWriteSource::from(leaf.origin?.into_symbolic_place()?),
                },
            ));
        }
        for moved in relation.moves {
            transports.push(ReferenceResultTransport::Value {
                result_path: moved.local_segments,
                source: LocalWriteSource {
                    invocation_output: moved.source.output_identity(),
                    is_image_storage: moved.source.is_static(),
                    recasts: moved.source.recasts,
                    representation: moved.source.representation,
                    path: if moved.source.root.is_valid() {
                        self.program.symbols.name(moved.source.root).to_owned()
                    } else {
                        String::new()
                    },
                    collection_coarse: false,
                    root: if moved.source.builtin_coordinates {
                        moved.source.root
                    } else {
                        SymbolHandle::invalid()
                    },
                    segments: moved.source.segments,
                },
                type_reference: moved.type_reference,
            });
        }
        Some(transports)
    }
}
