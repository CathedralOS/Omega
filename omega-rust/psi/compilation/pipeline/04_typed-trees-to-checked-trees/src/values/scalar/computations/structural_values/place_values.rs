//! Place-rooted structural value production: each root's custody (borrowed,
//! copied, projected, owned) selects its own establishment path.

use super::super::{Builder, ExpressionHandle, ExpressionNode, PrimitiveType, TypeReferenceNode};
use super::place_types::{
    canonical_place_is_borrowable, is_scalar_case_place_value, needs_copied_place_root,
    projection_root_expression,
};
use super::value_predicates::{
    is_view_element_copy, plain_or_policy_scalar_field, shared_record_reference,
};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// One copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` reads which case the borrowed record currently
    /// holds. The root and path become a `SharedBorrow` `Place` source exactly
    /// as `borrowed_place` builds for `&place`, so lowering observes the held
    /// case and establishes the same constructor fresh rather than moving a
    /// child out of the loan. Owned roots still move their whole leaf through
    /// the owned arms.
    pub(super) fn scalar_case_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_scalar_case_place_value(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        let (source, leaf) = self.borrowed_leaf_argument(expression, expected)?;
        Some(CheckedStructuralValueKind::ScalarCasePlace { source, leaf })
    }

    /// One `[copy]` record element copied out of a borrowed view: the same
    /// element selection an element read takes, then one read per scalar
    /// field of the element in declaration order.

    /// One `[copy]` record element copied out of a borrowed view: the same
    /// element selection an element read takes, then one read per scalar
    /// field of the element in declaration order.
    pub(super) fn view_element_copy(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_view_element_copy(self.program, expression, expected) {
            return None;
        }
        let selection = super::super::super::scalar_lowering::selected_element(
            self.program,
            self.operators,
            expression,
            self.parameters,
            self.authored_parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        )?;
        if !selection.through_view
            || !selection.path.is_empty()
            || self
                .program
                .normalized_type_identity(selection.element_type)
                != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } = self
            .program
            .type_reference_table
            .type_reference(validation::unwrapped_type_reference(
                self.program,
                expected,
            )?)
        else {
            return None;
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        let mut reads = Vec::new();
        for member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let primitive_type = self
                .program
                .primitive_type_reference(field.type_reference)?;
            if field.relevance.is_erased()
                || primitive_type == PrimitiveType::Addr
                || !plain_or_policy_scalar_field(self.program, field.type_reference)
            {
                return None;
            }
            reads.push(
                checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                    root: selection.root,
                    path: Vec::new(),
                    index: Box::new(selection.index.clone()),
                    element_path: vec![checked_trees::CheckedScalarReadProjection::Static(
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(
                            field.path_identity(),
                        ),
                    )],
                    primitive_type,
                },
            );
        }
        (!reads.is_empty()).then_some(CheckedStructuralValueKind::ViewElementCopy { reads })
    }

    /// Copy one `Unrestricted` leaf from readable storage into a fresh owned
    /// place. The source root and its ownership or loan remain unchanged.

    /// Copy one `Unrestricted` leaf from readable storage into a fresh owned
    /// place. The source root and its ownership or loan remain unchanged.
    pub(super) fn copied_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !needs_copied_place_root(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        let (source, leaf) = self.borrowed_leaf_argument(expression, expected)?;
        Some(CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf })
    }

    /// The `SharedBorrow` argument plan a borrowed leaf read produces: the
    /// named root as a parameter/local/`self` source plus the canonical
    /// member path, with `type_identity` naming the projected leaf.

    /// The `SharedBorrow` argument plan a borrowed leaf read produces: the
    /// named root as a parameter/local/`self` source plus the canonical
    /// member path, with `type_identity` naming the projected leaf.
    pub(super) fn borrowed_leaf_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<(
        checked_trees::CheckedUnitStructuralArgumentPlan,
        TypeReferenceHandle,
    )> {
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // Value planning precedes the final structural parameter roster. Its
        // dense positions change when a borrowed receiver is retained, so an
        // authored leaf must keep its symbol until binding resolves that roster.
        // StructuralLocal already represents both live locals and named formals;
        // independent source replay still checks the exact authored root.
        let symbol = self
            .authored_parameters
            .iter()
            .find(|parameter| false && (parameter.symbol == symbol || self.machine == symbol))
            .map_or(symbol, |parameter| parameter.symbol);
        let source =
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        // The borrowed lane admits a runtime `Index` segment: the selector
        // resolves to a checked `RuntimeIndex` coordinate and terminal
        // verification re-proves `0 <= index < extent` against the enclosing
        // array's literal extent.
        let (projected, path) = crate::execution::terminal_unit::calls::projected_borrowed_path(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            &place,
        )?;
        if !validation::declared_carriers_match(self.program, projected, expected) {
            return None;
        }
        // The structural-type registry names a `&[T]` leaf by its borrowed
        // view carrier — the same identity `add_type` mints for the result —
        // so a view leaf records its unwrapped referee's identity. Arithmetic
        // policy likewise changes operations, not storage: owned leaves name
        // the policy-free carrier registered by ShapeCollector::add_type.
        // Keep `projected` below and the carrier comparison above qualified
        // by the declared leaf, never its text.
        let type_identity =
            if crate::execution::terminal_unit::types::borrowed_slice_view(self.program, projected)
            {
                validation::unwrapped_type_reference(self.program, projected).map(|unwrapped| {
                    self.program
                        .normalized_type_identity(unwrapped)
                        .into_string()
                })
            } else {
                None
            }
            .unwrap_or_else(|| {
                self.program
                    .normalized_type_identity(validation::peel_arithmetic_policy(
                        self.program,
                        projected,
                    ))
                    .into_string()
            });
        Some((
            checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity,
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
            projected,
        ))
    }

    /// One selected projected child of an existing owner. The owned-selection
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is built as a whole `Place` (or
    /// `Call` product) node carrying the exact root identity.

    /// One selected projected child of an existing owner. The owned-selection
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is built as a whole `Place` (or
    /// `Call` product) node carrying the exact root identity.
    pub(super) fn projected_selection_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        // Owned-selection receipts are recorded only after value plans are
        // built, so this projection derives the same canonical place
        // independently. The multiplicity checker's recorded transfer path is
        // compared against this path downstream, not trusted here.
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        if place.segments.is_empty()
            || !place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let root_expression = projection_root_expression(self.program, expression)?;
        let root_reference = crate::flow::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let source = self.structural_value(root_expression, root_reference)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity: self
                .program
                .normalized_type_identity(projected)
                .into_string(),
        })
    }

    /// One shared borrow of an exact place selected as a `&T` result. The
    /// referent stays owned by its established root; the authored target's
    /// canonical root and path become a `SharedBorrow` source so lowering can
    /// rejoin borrowed custody at the selection's block parameter instead of
    /// moving a child. `shared_record_reference` pins the referent to a
    /// carrier the pipeline can build -- a record, or a primitive whose
    /// structural shape is its own scalar -- and the target must be the exact
    /// place semantics admitted: a named root under record fields and literal
    /// fixed indexes, whose projected leaf type has to equal the declared
    /// referent.

    /// One shared borrow of an exact place selected as a `&T` result. The
    /// referent stays owned by its established root; the authored target's
    /// canonical root and path become a `SharedBorrow` source so lowering can
    /// rejoin borrowed custody at the selection's block parameter instead of
    /// moving a child. `shared_record_reference` pins the referent to a
    /// carrier the pipeline can build -- a record, or a primitive whose
    /// structural shape is its own scalar -- and the target must be the exact
    /// place semantics admitted: a named root under record fields and literal
    /// fixed indexes, whose projected leaf type has to equal the declared
    /// referent.
    /// One copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` reads which case the borrowed record currently
    /// holds. The root and path become a `SharedBorrow` `Place` source exactly
    /// as `borrowed_place` builds for `&place`, so lowering observes the held
    /// case and establishes the same constructor fresh rather than moving a
    /// child out of the loan. Owned roots still move their whole leaf through
    /// the owned arms.
    pub(super) fn borrowed_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::Borrow(borrow) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if borrow.access != language_semantics::ReferenceAccess::Shared {
            return None;
        }
        let referent = shared_record_reference(self.program, expected)?;
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            borrow.target,
        )?;
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        let source = self.structural_source(symbol);
        let (mut projected, mut path) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )?;
        if let TypeReferenceNode::Reference {
            referee, access, ..
        } = self.program.type_reference_table.type_reference(projected)
        {
            if *access == language_semantics::ReferenceAccess::WriteOnly {
                return None;
            }
            projected = *referee;
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        }
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(referent)
        {
            return None;
        }
        Some(CheckedStructuralValueKind::Reference {
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        })
    }

    pub(super) fn owned_structural_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || name.members.count() != 1
            // A completed owner carries the same contents a literal establishes,
            // including represented field domains; transferring it does not
            // establish those domains anew. Whole transfer preserves cleanup.
            // Shared descriptor leaves keep their loans without owning the backing;
            // neither transport grants no-code disposal of the moved value.
            || !(validation::has_contents(self.program, expected, validation::ContentsRequirement::StructuralConstruction)
                    || validation::has_contents(self.program, expected, validation::ContentsRequirement::LinearOwned)
                || validation::has_contents(self.program, expected, validation::ContentsRequirement::OwnedOrSharedView)
                || validation::reference_result_custody::is_reference_record(self.program, expected)
                || validation::reference_result_custody::stored_loan_parts(self.program, expected).is_some())
        {
            return None;
        }
        let owner = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = self
            .program
            .machine_states(owner)
            .iter()
            .find(|state| state.symbol == self.state)?;
        let (source, reference) = if let Some((parameter_index, type_reference)) =
            self.structural_parameter(name.symbol)
        {
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                },
                type_reference,
            )
        } else {
            let (_, local) = self
                .program
                .state_local_declaration(state, name.symbol)
                .filter(|&(ordinal, local)| {
                    ordinal < self.statement_index && local.initial_value.is_valid()
                })?;
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                local.type_reference,
            )
        };
        if self.program.normalized_type_identity(reference)
            != self.program.normalized_type_identity(expected)
            || !(validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::StructuralConstruction,
            ) || validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::LinearOwned,
            ) || validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::OwnedOrSharedView,
            ) || validation::reference_result_custody::is_reference_record(
                self.program,
                reference,
            ) || validation::reference_result_custody::stored_loan_parts(
                self.program,
                reference,
            )
            .is_some())
        {
            return None;
        }
        // Sequence storage has its own catalog identity: a qualified byte
        // owner retains a byte carrier plus separate qualification evidence,
        // not a second structural type named by the qualified source spelling.
        // Use the same identity owner as ShapeCollector, while the comparison
        // above still requires the complete declared types to agree.
        let type_identity = if crate::execution::terminal_unit::types::byte_sequence_carrier(
            self.program,
            reference,
            &[],
        )
        .is_some()
        {
            crate::execution::terminal_unit::types::byte_sequence_type_identity(
                self.program,
                reference,
                &[],
                &[],
            )?
        } else {
            self.program
                .normalized_type_identity(reference)
                .into_string()
        };
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity,
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }
}
