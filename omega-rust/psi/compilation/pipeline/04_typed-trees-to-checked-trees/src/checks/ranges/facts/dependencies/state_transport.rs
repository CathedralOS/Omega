//! Translate an incoming observation through its admitted state bindings.
//!
//! A projected read owns both a storage root and expressions selecting within
//! that storage. Rebinding only the root leaves a mixed-state place which cannot
//! agree with the destination's own contract. Selectors rejoin existing typed
//! expressions under the same simultaneous parameter map; no expression is
//! evaluated, synthesized, or equated by spelling. The incoming-edge owner has
//! already established the bindings, and the read collector still owns complete
//! dependencies and subsequent write invalidation.

use super::{EXPRESSION_WALK_DEPTH_BOUND, ExpressionDependencyScope};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::type_identity::TypeIdentityRequest;
use validation::{CanonicalPlace, CheckingInput};

pub(super) fn transport_reads(
    program: &dyn CheckingInput,
    machine: &Machine,
    scope: &ExpressionDependencyScope<'_>,
    statement_index: usize,
    reads: &mut [CanonicalPlace],
) -> bool {
    let transport = StateTransport {
        program,
        machine,
        scope,
        statement_index,
    };
    for read in reads {
        let Some(root) = transport.root(read.root) else {
            return false;
        };
        read.root = root;
        for segment in &mut read.segments {
            let PlaceSegment::Index { expression } = segment else {
                continue;
            };
            let Some((destination, _)) =
                program
                    .expression_table
                    .iter_expressions()
                    .find(|(destination, _)| {
                        transport.expressions_correspond(*expression, *destination, 0)
                    })
            else {
                return false;
            };
            *expression = destination;
        }
    }
    true
}

struct StateTransport<'context, 'scope> {
    program: &'context dyn CheckingInput,
    machine: &'context Machine,
    scope: &'context ExpressionDependencyScope<'scope>,
    statement_index: usize,
}

impl StateTransport<'_, '_> {
    fn same_computed_type(&self, source: ExpressionHandle, destination: ExpressionHandle) -> bool {
        let result = |state, expression| {
            validation::expression_result_type_reference(
                self.program,
                self.machine,
                state,
                expression,
            )
            .map(|reference| {
                self.program
                    .type_identity(TypeIdentityRequest::ordinary(reference))
            })
        };
        // Identical tokens and carried values do not equate computations under
        // different integer widths or overflow policies.
        let Some(source) = result(self.scope.evaluation, source) else {
            return false;
        };
        result(self.scope.destination, destination) == Some(source)
    }

    fn root(&self, source: PlaceRoot) -> Option<PlaceRoot> {
        let symbol = source.as_symbol()?;
        if symbol == self.machine.symbol
            || crate::checks::ranges::types::is_const_binder(self.program, self.machine, symbol)
        {
            return Some(source);
        }
        self.scope
            .parameter_roots
            .iter()
            .find(|(parameter, _)| *parameter == symbol)
            .map(|(_, destination)| PlaceRoot::Symbol(*destination))
    }

    fn places_correspond(
        &self,
        source: &CanonicalPlace,
        destination: &CanonicalPlace,
        depth: usize,
    ) -> bool {
        self.root(source.root) == Some(destination.root)
            && source.segments.len() == destination.segments.len()
            && source.segments.iter().zip(&destination.segments).all(
                |(source, destination)| match (source, destination) {
                    (
                        PlaceSegment::Index { expression: source },
                        PlaceSegment::Index {
                            expression: destination,
                        },
                    ) => self.expressions_correspond(*source, *destination, depth + 1),
                    _ => crate::flow::canonical_place_segments_equal(
                        self.program,
                        *source,
                        *destination,
                    ),
                },
            )
    }

    fn expressions_correspond(
        &self,
        source: ExpressionHandle,
        destination: ExpressionHandle,
        depth: usize,
    ) -> bool {
        let program = self.program;
        if depth >= EXPRESSION_WALK_DEPTH_BOUND
            || !program.expression_table.expression_is_valid(source)
            || !program.expression_table.expression_is_valid(destination)
        {
            return false;
        }
        let children =
            |source, destination| self.expressions_correspond(source, destination, depth + 1);
        match (
            program.expression_table.expression(source),
            program.expression_table.expression(destination),
        ) {
            (ExpressionNode::Name(_), ExpressionNode::Name(_))
            | (ExpressionNode::Member(_), ExpressionNode::Member(_))
            | (ExpressionNode::Indexed(_), ExpressionNode::Indexed(_)) => {
                let source_length = validation::collection_length_receiver(
                    program,
                    self.machine,
                    Some(self.scope.evaluation),
                    source,
                );
                let destination_length = validation::collection_length_receiver(
                    program,
                    self.machine,
                    Some(self.scope.destination),
                    destination,
                );
                match (source_length, destination_length) {
                    (Some(source), Some(destination)) => return children(source, destination),
                    (None, None) => {}
                    _ => return false,
                }
                let place = |state: &typed_trees::state::State, expression| {
                    validation::place_has_builtin_coordinates(
                        program,
                        self.machine,
                        Some(state),
                        expression,
                    )
                    .then(|| {
                        validation::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            self.statement_index,
                            expression,
                        )
                    })
                    .flatten()
                };
                match (
                    place(self.scope.evaluation, source),
                    place(self.scope.destination, destination),
                ) {
                    (Some(source), Some(destination)) => {
                        self.places_correspond(&source, &destination, depth)
                    }
                    _ => false,
                }
            }
            (ExpressionNode::Binary(source_node), ExpressionNode::Binary(destination_node)) => {
                source_node.operator == destination_node.operator
                    && self.same_computed_type(source, destination)
                    && children(source_node.left, destination_node.left)
                    && children(source_node.right, destination_node.right)
                    && validation::has_builtin_binary_expression_meaning(
                        program,
                        self.machine,
                        Some(self.scope.evaluation),
                        source,
                    )
                    && validation::has_builtin_binary_expression_meaning(
                        program,
                        self.machine,
                        Some(self.scope.destination),
                        destination,
                    )
            }
            (ExpressionNode::Unary(source_node), ExpressionNode::Unary(destination_node)) => {
                source_node.operator == destination_node.operator
                    && self.same_computed_type(source, destination)
                    && children(source_node.operand, destination_node.operand)
            }
            (ExpressionNode::Borrow(source), ExpressionNode::Borrow(destination)) => {
                source.access == destination.access && children(source.target, destination.target)
            }
            (ExpressionNode::Cast(source), ExpressionNode::Cast(destination)) => {
                let same_type =
                    |source: typed_trees::types::TypeReferenceHandle,
                     destination: typed_trees::types::TypeReferenceHandle| {
                        source.is_valid()
                            && destination.is_valid()
                            && program.type_identity(TypeIdentityRequest::ordinary(source))
                                == program.type_identity(TypeIdentityRequest::ordinary(destination))
                    };
                same_type(source.target_type, destination.target_type)
                    && same_type(source.result_type, destination.result_type)
                    && source.domain == destination.domain
                    && source.form == destination.form
                    && source.semantic_domain_id == destination.semantic_domain_id
                    && children(source.value, destination.value)
            }
            (ExpressionNode::Range(source), ExpressionNode::Range(destination)) => {
                let endpoints = |source: ExpressionHandle, destination: ExpressionHandle| {
                    if source.is_valid() || destination.is_valid() {
                        children(source, destination)
                    } else {
                        true
                    }
                };
                source.end_inclusive == destination.end_inclusive
                    && endpoints(source.start, destination.start)
                    && endpoints(source.end, destination.end)
            }
            (ExpressionNode::Integer(source), ExpressionNode::Integer(destination)) => {
                source == destination
            }
            (ExpressionNode::Boolean(source), ExpressionNode::Boolean(destination)) => {
                source == destination
            }
            (ExpressionNode::Float(source), ExpressionNode::Float(destination)) => {
                source == destination
            }
            (ExpressionNode::String(source), ExpressionNode::String(destination)) => {
                source == destination
            }
            // Calls and effectful observations cannot be reproduced by matching
            // operands. They need their own checked result/capture relation.
            _ => false,
        }
    }
}
