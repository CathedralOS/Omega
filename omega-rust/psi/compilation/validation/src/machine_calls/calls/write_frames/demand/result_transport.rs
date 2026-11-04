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

    /// Resolve the full normal-result relation using immutable integer actuals
    /// from one invocation. Unknown coordinates keep the ordinary union;
    /// unavailable ordinary evidence is never treated as an empty relation.
    /// This scratch context cannot enter the state-keyed transport memo.
    pub fn reference_result_transports_for_indices(
        &self,
        symbol: SymbolHandle,
        indices: &[Option<usize>],
    ) -> Option<Vec<ReferenceResultTransport>> {
        let Some(state) = self.program.state_by_symbol(symbol) else {
            // A trait requirement member owns no stored machine state and no
            // invocation parameter context: its signature-only transport is
            // an opaque invocation referent independent of the actuals.
            return self.reference_result_transports(symbol);
        };
        if self.program.state_parameters(state).len() != indices.len() {
            return None;
        }
        if !indices.iter().any(Option::is_some) {
            return self.reference_result_transports(symbol);
        }
        if let Some(ordinary) = self.reference_result_transports(symbol)
            && !ordinary.iter().any(|transport| {
                let source = match transport {
                    ReferenceResultTransport::Reference(leaf) => &leaf.source,
                    ReferenceResultTransport::Value { source, .. } => source,
                };
                source.segments.iter().any(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
                    )
                })
            })
        {
            return Some(ordinary);
        }
        let mut inference = FrameInference::default();
        inference.with_invocation(self.program, state, &[], indices, |inference| {
            self.result_transports_in_context(symbol, inference)
        })
    }

    /// Retain whole-value moves and reference leaves under the same invocation
    /// context. Consumers select stored descriptors from the completed actual.
    pub fn reference_result_transports_for_arguments(
        &self,
        symbol: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
    ) -> Option<Vec<ReferenceResultTransport>> {
        let inference = FrameInference::default();
        let indices = inference.argument_indices(self.program, arguments);
        self.reference_result_transports_for_indices(symbol, &indices)
    }

    /// The flat projection still cannot select payloads of a moved value.
    /// Apply invocation context to the full relation first, then retain that
    /// refusal rather than letting a state-only flat query preempt context.
    pub fn reference_result_leaf_origins_for_arguments(
        &self,
        symbol: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
    ) -> Option<Vec<ReferenceResultLeafOrigin>> {
        self.reference_result_transports_for_arguments(symbol, arguments)?
            .into_iter()
            .map(|transport| match transport {
                ReferenceResultTransport::Reference(leaf) => Some(leaf),
                ReferenceResultTransport::Value { .. } => None,
            })
            .collect()
    }

    fn result_transports_in_context(
        &self,
        symbol: SymbolHandle,
        inference: &mut FrameInference,
    ) -> Option<Vec<ReferenceResultTransport>> {
        let Some(state) = self.program.state_by_symbol(symbol) else {
            // A trait requirement member owns no stored machine state: its
            // signature is the complete relation. An exclusive result is the
            // completed invocation's own opaque referent — the same
            // provenance call-site routes mint — carried as a transport leaf
            // whose call identity the caller reassigns.
            let (_, signature) = crate::selected_requirement_signature(self.program, symbol)?;
            let referent = exclusive_transport_referent(self.program, signature.return_type)?;
            return Some(vec![invocation_result_transport(referent)]);
        };
        let machine = self.program.machine_holding_state(symbol)?;
        let relation = super::super::result_origins::state_result_origins(
            self.program,
            machine,
            state,
            state.return_type,
            &self.tables.symbols,
            inference,
            true,
        );
        let Some(relation) = relation else {
            // A signature-only callee (`satisfies via` binding or opaque
            // boundary) exports no body relation. Its declared exclusive
            // result is the completed invocation's own opaque referent.
            if machine.body_is_present {
                return None;
            }
            let referent = exclusive_transport_referent(self.program, state.return_type)?;
            return Some(vec![invocation_result_transport(referent)]);
        };
        let mut transports = Vec::new();
        for leaf in relation.references {
            // This is a union of normal routes. A move on one route
            // cannot mask a constructed or unresolved leaf on another.
            // Unknown reference origins therefore still refuse here;
            // the capture consumer independently checks case presence.
            let mut origin = leaf.origin?.into_symbolic_place()?;
            // A caller cannot interpret this callee's selector expression.
            // Preserve its checked window dimension without treating it as an
            // element index or inventing exact bounds in the caller's scope.
            super::super::summary_windows::coarsen_origin(self.program, &mut origin);
            transports.push(ReferenceResultTransport::Reference(
                ReferenceResultLeafOrigin {
                    result_path: leaf.local_segments,
                    source: LocalWriteSource::from(origin),
                },
            ));
        }
        for moved in relation.moves {
            transports.push(ReferenceResultTransport::Value {
                result_path: moved.local_segments,
                source: LocalWriteSource {
                    invocation_output: moved.source.output_identity(),
                    invocation_result: moved.source.result_home().map(|(call, referent)| {
                        super::super::caller_aliases::InvocationResultHome { call, referent }
                    }),
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

/// The declared referee behind a signature-only callee's exclusive result —
/// `None` when the return is not an exclusive reference at all. This is the
/// same referent `exclusive_result_referent` selects on the call-site route;
/// the transport boundary has no call type bindings, so it reads the declared
/// head directly.
fn exclusive_transport_referent(
    program: &dyn crate::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let result_type =
        typed_trees::operator::unconstrained_type_reference(program.typed_program(), return_type)?;
    let typed_trees::types::TypeReferenceNode::Reference {
        access, referee, ..
    } = program.type_reference_table.type_reference(result_type)
    else {
        return None;
    };
    if !access.is_exclusive() {
        return None;
    }
    typed_trees::operator::unconstrained_type_reference(program.typed_program(), *referee)
}

/// The opaque invocation-owned referent a signature-only callee's exclusive
/// result names. `call` stays invalid at this state/signature-keyed level:
/// every call-site instantiation rebases the producing expression.
fn invocation_result_transport(
    referent: typed_trees::types::TypeReferenceHandle,
) -> ReferenceResultTransport {
    ReferenceResultTransport::Reference(ReferenceResultLeafOrigin {
        result_path: Vec::new(),
        source: LocalWriteSource {
            invocation_output: None,
            invocation_result: Some(super::super::caller_aliases::InvocationResultHome {
                call: typed_trees::expression::ExpressionHandle::invalid(),
                referent,
            }),
            is_image_storage: false,
            recasts: Vec::new(),
            representation: None,
            path: String::new(),
            collection_coarse: false,
            root: SymbolHandle::invalid(),
            segments: Vec::new(),
        },
    })
}
