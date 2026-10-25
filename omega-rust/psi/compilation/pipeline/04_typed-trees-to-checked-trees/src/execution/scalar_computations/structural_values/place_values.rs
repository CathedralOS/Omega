//! Place-rooted structural value production: each root's custody (borrowed,
//! copied, projected, owned) selects its own establishment path.
//! All sources publish the stored-carrier identity used by the type registry,
//! not a second row for ordinary qualification. Exact declared types remain on
//! nodes and carrier comparisons; descriptors and bounded-byte shapes retain
//! the shells required by their representation. No lookup grants membership.
//! Readable copies retain one source subtree and selector, rather than
//! rebuilding records field by field and discarding non-scalar evidence.

use super::super::{Builder, ExpressionHandle, ExpressionNode, TypeReferenceNode};
use super::place_types::{projection_root_expression, readable_place_type};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// A shared reference with the same unrestricted carrier retains
    /// its descriptor on reborrow. It does not reconstruct a view from the
    /// loan's backing: that would lose the producer's actual range and effects.
    /// Stored and computed sources use their ordinary value producer before
    /// this reborrow. Access attenuation and affine carriers remain references.
    pub(super) fn shared_carrier_reborrow(
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
        // The target is a reference value, not the storage that its loan
        // eventually names. Canonicalizing its backing first loses a
        // computed descriptor (and the qualifications of a recast producing
        // it), even though reborrowing a stored descriptor already works.
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        let actual =
            validation::expression_result_type_reference(self.program, machine, state, target)?;
        if self.program.normalized_type_identity(actual)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let source = self.structural_value(target, actual)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            logical_root_type_identity: Default::default(),
            path: Vec::new(),
            projection_evaluations: Default::default(),
            copies_selected_value: false,
            type_identity: validation::stored_structural_type_identity(
                self.program,
                actual,
                &[],
                &[],
            ),
        })
    }

    /// A value cast that keeps its structural carrier -- an explicit domain
    /// qualification or an explicit erasure -- denotes its operand unchanged
    /// (`as` "never silently changes denotation", and no carrier transform
    /// exists between equal stored carriers). Checking has already judged the
    /// qualification at this exact cast, so the value is its operand's
    /// producer viewed whole: the same whole-value projection a same-carrier
    /// reborrow publishes. Recasts and representation changes are not this.
    pub(super) fn same_carrier_structural_cast(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let operand = validation::value_preserving_cast_operand(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        let actual =
            validation::expression_result_type_reference(self.program, machine, state, operand)?;
        let carrier = validation::stored_structural_type_identity(self.program, actual, &[], &[]);
        if carrier != validation::stored_structural_type_identity(self.program, expected, &[], &[])
        {
            return None;
        }
        let source = self.structural_value(operand, actual)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            logical_root_type_identity: Default::default(),
            path: Vec::new(),
            projection_evaluations: Default::default(),
            copies_selected_value: false,
            type_identity: carrier,
        })
    }

    /// Copy one copyable stored value into a fresh owned place. Shared loan
    /// carriers retain affine retirement custody on each independent copy;
    /// the source root and its ownership or loan remain unchanged.
    pub(super) fn copied_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if let Some(kind) = self.copied_dispatch_place(expression, expected) {
            return Some(kind);
        }
        let (source, leaf, _) = self.borrowed_leaf_argument(expression, expected)?;
        let computations = source
            .computation_operands(&self.plans.operands)
            .collect::<Vec<_>>();
        for (ordinal, value) in computations.into_iter().enumerate() {
            self.push_scalar_root(
                checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                    expression,
                    selector_ordinal: u32::try_from(ordinal).ok()?,
                },
                value,
            )?;
        }
        Some(CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf })
    }

    /// The `SharedBorrow` argument plan a borrowed leaf read produces: the
    /// named root as a parameter/local/`self` source plus the canonical
    /// member path, with `type_identity` naming the projected leaf.
    pub(in crate::execution::scalar_computations) fn borrowed_leaf_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<(
        checked_trees::CheckedUnitStructuralArgumentPlan,
        TypeReferenceHandle,
        Vec<(
            ExpressionHandle,
            checked_trees::CheckedScalarComputationHandle,
        )>,
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
        let selectors = self.projection_selector_evaluations(expression, expression)?;
        let (_, mut path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
            Some(self.machine),
            Some(&|expression| {
                selectors.iter().find_map(|(selector, value)| {
                    (*selector == expression).then_some(
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                    )
                })
            }),
        )?;
        // A local owns its descriptor, whereas a borrowed formal already
        // denotes its referent. Selecting below the local crosses that stored
        // address; copying the whole descriptor does not cross it. View
        // carriers already select their own backing on indexing: only the
        // registry's Reference shape requires a separate pointer crossing.
        if !place.segments.is_empty()
            && let Some(state) = self.program.state_by_symbol(self.state)
            && let Some((_, local)) = self.program.state_local_declaration(state, symbol)
        {
            let mut reference = local.type_reference;
            let mut crossings = Vec::new();
            while let Some(shell) = validation::reference_shell(self.program, reference) {
                if shell.access == language_semantics::ReferenceAccess::WriteOnly {
                    return None;
                }
                if !validation::stores_reference_shape(self.program, reference, &[]) {
                    break;
                }
                crossings.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                reference = shell.referee;
            }
            crossings.append(&mut path);
            path = crossings;
        }
        let projected = readable_place_type(
            self.program,
            self.state,
            self.statement_index,
            &place,
            expected,
        )?;
        // Copying a leaf lends its stored carrier. Its declared qualification
        // remains on `projected` and in the comparison above, while descriptors
        // and bounded-byte shapes follow the same registry rule as owned values.
        let type_identity =
            validation::stored_structural_type_identity(self.program, projected, &[], &[]);
        let mut source = checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path,
            projection_evaluations: Default::default(),
            type_identity,
            access: checked_trees::CheckedStructuralAccess::SharedBorrow,
        };
        source.retain_projection_evaluations(
            self.projection_selector_span(expression, &selectors)?,
            &self.plans.operands,
        )?;
        Some((source, projected, selectors))
    }

    /// One projected owned child of an existing owner. The checked ownership
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is retained whole, including an
    /// evaluated product, and selectors keep their own checked evaluations.
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
        let selectors = self.projection_selector_evaluations(expression, expression)?;
        let (projected, mut path) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
                Some(self.machine),
                Some(&|expression| {
                    selectors.iter().find_map(|(selector, value)| {
                        (*selector == expression).then_some(
                            checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                        )
                    })
                }),
            )?;
        // A checked qualification changes the receiving occurrence, not the
        // selected storage carrier. Keep projections in the same carrier
        // namespace as whole owned values; their exact membership decisions
        // are retained on the value node and consumed during emission.
        if validation::stored_structural_type_identity(self.program, projected, &[], &[])
            != validation::stored_structural_type_identity(self.program, expected, &[], &[])
        {
            return None;
        }
        for (ordinal, (_, value)) in selectors.iter().enumerate() {
            self.push_scalar_root(
                checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                    expression,
                    selector_ordinal: u32::try_from(ordinal).ok()?,
                },
                *value,
            )?;
        }
        let root_expression = projection_root_expression(self.program, expression);
        let root_reference = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        if matches!(place.root, facts::PlaceRoot::Expression(_)) {
            // The canonical place walk starts at borrowed storage. An
            // evaluated value instead owns its stored descriptor, so retain
            // each actual pointer crossing before selecting the first field.
            // View descriptors already interpret their own element paths.
            let mut reference = root_reference;
            while let Some(shell) = validation::reference_shell(self.program, reference)
                && validation::stores_reference_shape(self.program, reference, &[])
            {
                path.insert(0, checked_trees::CheckedUnitStructuralPathSegment::Referent);
                reference = shell.referee;
            }
        }
        let source = self.structural_value(root_expression, root_reference)?;
        if matches!(place.root, facts::PlaceRoot::Expression(_)) {
            // The selected copy and its evaluated containing owner have
            // independent completions. Publish the actual backing producer
            // even when the selected leaf owns a new copied value; contextual
            // cleanup must rejoin this owner, never the copy's source label.
            // This association neither evaluates it again nor grants disposal.
            self.retain_structural_value_root(source, root_reference)?;
        }
        // Copy permission belongs to the selected value. The source's own
        // checked disposition is independent: an anonymous owner's eventual
        // death cannot turn an observation into a partial ownership move.
        let projection_evaluations = self.projection_selector_span(expression, &selectors)?;
        if !checked_trees::CheckedUnitStructuralArgumentPlan::projection_evaluations_match(
            &path,
            projection_evaluations,
            &self.plans.operands,
        ) {
            return None;
        }
        Some(CheckedStructuralValueKind::Projection {
            source,
            // Symbol-rooted paths use the declared borrowed backing. An
            // expression-rooted path above already retains its physical hops.
            logical_root_type_identity: if matches!(place.root, facts::PlaceRoot::Symbol(_))
                && validation::stores_reference_shape(self.program, root_reference, &[])
            {
                validation::logical_structural_type_identity(self.program, root_reference, &[], &[])
            } else {
                Default::default()
            },
            path,
            projection_evaluations,
            copies_selected_value: super::place_types::copied_place_type(self.program, expected),
            type_identity: validation::stored_structural_type_identity(
                self.program,
                projected,
                &[],
                &[],
            ),
        })
    }

    pub(super) fn owned_structural_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        if let ExpressionNode::SavedSubject(saved) =
            self.program.expression_table.expression(expression)
        {
            let source = self.program.expression_table.saved_subject_source(saved)?;
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
            let actual =
                validation::expression_result_type_reference(self.program, machine, state, source)?;
            if self.program.normalized_type_identity(actual)
                != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            let anchor = self
                .program
                .expression_table
                .match_subject(saved.capture)
                .value_anchor;
            return Some(checked_trees::CheckedUnitStructuralArgumentPlan {
                source:
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression: anchor,
                    },
                path: Vec::new(),
                projection_evaluations: Default::default(),
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    expected,
                    &[],
                    &[],
                ),
                access: checked_trees::CheckedStructuralAccess::Owned,
            });
        }
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
            || !names_owned_structural_value(self.program, expected)
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
            || !names_owned_structural_value(self.program, reference)
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
            projection_evaluations: Default::default(),
            source,
            path: Vec::new(),
            type_identity,
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }
}

/// A declaration that names a completed structural value this transfer owns
/// directly: a construction, linear owner or view, a stored reference
/// descriptor or loan, or a borrowed slice view. Membership here is the
/// authored declaration's shape, never a source spelling or a scalar lane.
fn names_owned_structural_value(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    validation::has_contents(
        program,
        reference,
        &[],
        validation::ContentsRequirement::StructuralConstruction,
    ) || validation::has_contents(
        program,
        reference,
        &[],
        validation::ContentsRequirement::LinearOwned,
    ) || validation::has_contents(
        program,
        reference,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView,
    ) || validation::reference_result_custody::is_reference_record(program, reference)
        || validation::reference_result_custody::stored_loan_parts(program, reference).is_some()
        || validation::borrowed_slice_view_result(program, reference)
}
