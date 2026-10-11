//! Place-rooted structural value production: each root's custody (borrowed,
//! copied, projected, owned) selects its own establishment path.
//! All sources publish the stored-carrier identity used by the type registry,
//! not a second row for ordinary qualification. Exact declared types remain on
//! nodes and carrier comparisons; descriptors and bounded-byte shapes retain
//! the shells required by their representation. No lookup grants membership.

use super::super::{Builder, ExpressionHandle, ExpressionNode, PrimitiveType, TypeReferenceNode};
use super::place_types::{needs_copied_place_root, projection_root_expression};
use super::value_predicates::{is_view_element_copy, plain_or_policy_scalar_field};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// A computed shared reference with the same unrestricted carrier retains
    /// its descriptor on reborrow. It does not reconstruct a view from the
    /// loan's backing: that would lose the producer's actual range and effects.
    /// Existing stored places keep their loan route; access attenuation and
    /// affine carriers need their own ordinary reference establishment.
    pub(super) fn computed_shared_reborrow(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let borrow = self
            .program
            .expression_table
            .borrow_expression(expression)?;
        if borrow.access != language_semantics::ReferenceAccess::Shared
            || !matches!(
                self.program.type_reference_table.type_reference(expected),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                }
            )
            || self.program.type_multiplicity(expected)
                != language_semantics::Multiplicity::Unrestricted
        {
            return None;
        }
        let target = borrow.target;
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            target,
        )?;
        if place.root.as_expression().is_none() || !place.segments.is_empty() {
            return None;
        }
        let actual = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(actual)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let source = self.structural_value(target, actual)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            path: Vec::new(),
            type_identity: validation::stored_structural_type_identity(
                self.program,
                actual,
                &[],
                &[],
            ),
        })
    }

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
        let selection = crate::values::scalar::scalar_lowering::selected_element(
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
        let data = self.program.data_definition_by_symbol(*symbol)?;
        let mut reads = Vec::new();
        for member in self.program.data_members(data) {
            let field = member.as_field()?;
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
                            checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            ),
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
    pub(super) fn borrowed_leaf_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<(
        checked_trees::CheckedUnitStructuralArgumentPlan,
        TypeReferenceHandle,
    )> {
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let symbol = place.root.as_symbol()?;
        // Value planning precedes the final structural parameter roster. Its
        // dense positions change when a borrowed receiver is retained, so an
        // authored leaf must keep its symbol until binding resolves that roster.
        // StructuralLocal already represents both live locals and named formals;
        // independent source replay still checks the exact authored root.
        let source =
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        // A copied value observes its selector at this evaluation point. Use
        // the computation's scalar namespace, including captured immutable
        // locals, rather than replaying the selector's original storage read.
        // The retained coordinate grants no bounds or copy permission.
        let (projected, path) = crate::execution::terminal_unit::calls::projected_selected_path(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            &place,
            &|selector| self.local_selector(selector),
        )?;
        if !validation::declared_carriers_match(self.program, projected, expected) {
            return None;
        }
        // Copying a leaf lends its stored carrier. Its declared qualification
        // remains on `projected` and in the comparison above, while descriptors
        // and bounded-byte shapes follow the same registry rule as owned values.
        let type_identity =
            validation::stored_structural_type_identity(self.program, projected, &[], &[]);
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

    /// One projected owned child of an existing owner. The checked ownership
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is built as a whole `Place` (or
    /// `Call` product) node carrying the exact root identity.
    pub(super) fn projected_owned_place(
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
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
            None,
            None,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let root_expression = projection_root_expression(self.program, expression)?;
        let root_reference = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let source = self.structural_value(root_expression, root_reference)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity: validation::stored_structural_type_identity(
                self.program,
                projected,
                &[],
                &[],
            ),
        })
    }

    /// One shared borrow of an exact place selected as a `&T` result. The
    /// referent stays owned by its established root; the authored target's
    /// canonical root and path become a `SharedBorrow` source so lowering can
    /// rejoin borrowed custody at the selection's block parameter instead of
    /// moving a child. The ordinary producer and type catalog determine the
    /// referent's representation, not a record-only contents list. The target
    /// must be the exact place semantics admitted: an existing or computed root under record
    /// fields and literal fixed indexes, whose projected leaf type must equal
    /// the declared referent. A computed root retains its backing producer;
    /// the descriptor owns neither that backing nor its cleanup obligations.
    pub(super) fn borrowed_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let Some(borrow) = self.program.expression_table.borrow_expression(expression) else {
            return None;
        };
        if borrow.access != language_semantics::ReferenceAccess::Shared {
            return None;
        }
        let shell = validation::reference_shell(self.program, expected)?;
        if shell.access != borrow.access {
            return None;
        }
        let referent = shell.referee;
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            borrow.target,
        )?;
        if !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        }) {
            return None;
        }
        // A source is storage, not necessarily a named binding. Retain the
        // producer of an anonymous root explicitly so construction schedules
        // can establish it before lending the selected path. Borrow checking
        // still owns the lifetime and access of this exact occurrence.
        let (source, source_value) = match place.root {
            facts::PlaceRoot::Symbol(symbol) => (
                self.structural_source(symbol),
                checked_trees::CheckedStructuralValueHandle::invalid(),
            ),
            facts::PlaceRoot::Expression(root) => {
                let root_reference = validation::canonical_place_type_reference(
                    self.program,
                    self.state,
                    self.statement_index,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: Vec::new(),
                    },
                )?;
                let value = self.structural_value(root, root_reference)?;
                (
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression: root,
                    },
                    value,
                )
            }
            _ => return None,
        };
        let (mut projected, mut path) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
                None,
                None,
            )?;
        // A restricted field has no borrowed shape; its loan stays the place
        // observation that transports the field invariant to each read.
        if let Some(facts::PlaceSegment::Field { symbol }) = place.segments.last()
            && crate::execution::terminal_unit::types::field_retains_scalar_restriction(
                self.program,
                *symbol,
                projected,
            )
        {
            return None;
        }
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
            source_value,
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    projected,
                    &[],
                    &[],
                ),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        })
    }

    pub(super) fn owned_structural_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let Some(name) = self.program.expression_table.name_path_node(expression) else {
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
            || !(validation::has_contents(
        self.program,
        expected,
        &[],
        validation::ContentsRequirement::StructuralConstruction
    )
                    || validation::has_contents(
        self.program,
        expected,
        &[],
        validation::ContentsRequirement::LinearOwned
    )
                || validation::has_contents(
        self.program,
        expected,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView
    )
                || validation::reference_result_custody::is_reference_record(self.program, expected)
                || validation::reference_result_custody::stored_loan_parts(self.program, expected).is_some()
                || validation::borrowed_slice_view_result(self.program, expected))
        {
            return None;
        }
        let owner = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(owner, self.state)?;
        let (source, reference) = if let Some((parameter_index, type_reference)) =
            self.structural_parameter(name.symbol)
        {
            // A reference formal is borrowed ingress, not owned descriptor
            // storage. Its value must retain the ordinary loan/view route.
            // Locals may own stored carriers even when their type is a view.
            if validation::reference_shell(self.program, type_reference).is_some() {
                return None;
            }
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
                    // An uninitialized local of a zero-constructible shape is
                    // established by its declaration (the ZeroedStructural root
                    // minted at its statement) and filled by later stores; it
                    // transfers as completely as an initialized one.
                    ordinal < self.statement_index
                        && (local.initial_value.is_valid()
                            || (self
                                .program
                                .primitive_type_reference(local.type_reference)
                                .is_none()
                                && validation::zeroed_structural_parts(
                                    self.program,
                                    local.type_reference,
                                )))
                })?;
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                local.type_reference,
            )
        };
        // Stored carrier identity decides transport. Source checking owns
        // qualification compatibility (including predicate-only weakening);
        // the completed source still supplies its actual membership roster.
        // Keep reference descriptors and representation-bearing shells exact.
        if validation::stored_structural_type_identity(self.program, reference, &[], &[])
            != validation::stored_structural_type_identity(self.program, expected, &[], &[])
            || !(validation::has_contents(
                self.program,
                reference,
                &[],
                validation::ContentsRequirement::StructuralConstruction,
            ) || validation::has_contents(
                self.program,
                reference,
                &[],
                validation::ContentsRequirement::LinearOwned,
            ) || validation::has_contents(
                self.program,
                reference,
                &[],
                validation::ContentsRequirement::OwnedOrSharedView,
            ) || validation::reference_result_custody::is_reference_record(
                self.program,
                reference,
            ) || validation::reference_result_custody::stored_loan_parts(
                self.program,
                reference,
            )
            .is_some()
                || validation::borrowed_slice_view_result(self.program, reference))
        {
            return None;
        }
        // Every owned value names ShapeCollector's stored carrier namespace.
        // Qualifications remain on the node and completed source contract,
        // not as competing storage rows. Stored reference descriptors
        // and bounded byte owners keep the shells their representation needs.
        let type_identity =
            validation::stored_structural_type_identity(self.program, reference, &[], &[]);
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity,
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }
}
