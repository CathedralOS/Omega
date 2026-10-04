use crate::unit_validation::structural_catalog::resolve_structural_path;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_semantics::{interior_member_borrow, structural_paths_may_overlap};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralProjectionPolicy {
    Unit,
    Projected,
    Boundary,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StructuralSourceContract<'a> {
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
    pub(crate) access: terminal_psi::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
    projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
    operation_result: Option<&'a terminal_psi::StructuralOperationResult>,
}

impl StructuralSourceContract<'_> {
    pub(crate) fn operation_result(&self) -> Option<&terminal_psi::StructuralOperationResult> {
        self.operation_result
    }

    /// Owning an unrestricted descriptor owns its carrier value, not mutable
    /// permission over its backing. Explicit incoming loans keep their declared
    /// access; this ceiling applies before selecting any element of the view.
    fn backing_access(
        &self,
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    ) -> terminal_psi::StructuralAccess {
        if self.access == terminal_psi::StructuralAccess::Owned
            && self.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && terminal_semantics::descriptor_results::is_shared_descriptor(
                types.values().copied(),
                self.structural_type,
            )
        {
            terminal_psi::StructuralAccess::SharedBorrow
        } else {
            self.access
        }
    }

    pub(crate) fn has_exact_qualifications(&self, required: &[StructuralDomainId]) -> bool {
        self.qualifications == required && self.projected_qualifications.is_empty()
    }

    pub(crate) fn declared_byte_predicates(
        &self,
        domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    ) -> Option<terminal_psi::ByteSequencePredicateSet> {
        if !self.projected_qualifications.is_empty() || self.has_result_evidence() {
            return None;
        }
        terminal_semantics::byte_qualification_predicates_in(
            domains.values().copied(),
            self.structural_type,
            self.qualifications,
        )
    }

    pub(crate) fn is_unqualified(&self) -> bool {
        self.qualifications.is_empty() && self.projected_qualifications.is_empty()
    }

    pub(crate) fn has_result_evidence(&self) -> bool {
        self.operation_result.is_some_and(|result| {
            !result.claims.is_empty() || !result.qualification_establishments.is_empty()
        })
    }

    pub(crate) fn carries_qualification(
        &self,
        path: &[terminal_psi::StructuralPathSegment],
        domain: StructuralDomainId,
    ) -> bool {
        if path.is_empty() {
            self.qualifications.contains(&domain)
        } else {
            self.projected_qualifications
                .iter()
                .any(|qualification| qualification.path == path && qualification.domain == domain)
        }
    }
}

pub(crate) fn structural_arguments_match(
    caller: &PsiOptimizationFunction,
    consumer: Option<semantic_vocabulary::OperationId>,
    arguments: &[terminal_psi::StructuralArgument],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    projection: StructuralProjectionPolicy,
) -> bool {
    if arguments.len() != parameters.len() {
        return false;
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        // Case selection is a call-scoped loan over existing backing. It does
        // not permit an owned extraction, boundary escape, or a new reference
        // result merely because the selected payload has the requested shape.
        let case_projection = argument
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Case(_)));
        if case_projection
            && (argument.access == terminal_psi::StructuralAccess::Owned
                || matches!(projection, StructuralProjectionPolicy::Boundary))
        {
            return false;
        }
        if let Some((carrier, qualifications, bytes)) = caller
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find_map(|node| match &node.operation {
                O::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    qualifications,
                    bytes,
                    ..
                } if place.id == argument.place => {
                    Some((structural_type.id, qualifications, bytes))
                }
                _ => None,
            })
            && !terminal_semantics::byte_literal_qualifications_hold_in(
                domains.values().copied(),
                carrier,
                qualifications,
                bytes,
            )
        {
            return false;
        }

        // A projection through a reference carrier names the carrier's exact
        // primitive referent, never a byte of the carrier itself. This mirrors
        // the verified call rule: internal calls only, exact parameter match.
        if argument
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
        {
            let selected =
                crate::unit_validation::references::call_source_type(caller, types, argument);
            let admitted = !matches!(projection, StructuralProjectionPolicy::Boundary)
                && selected == Some(parameter.structural_type)
                && argument.access == parameter.access
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty();
            if !admitted {
                return false;
            }
            continue;
        }
        let Some(source) = structural_source_contract(caller, argument.place) else {
            return false;
        };
        // Moving a carrier remains a custody operation. Lending payload must
        // also fit the backing permission; Owned is not permission to mutate
        // through a returned or copied shared descriptor.
        if argument.access != terminal_psi::StructuralAccess::Owned
            && !structural_access_can_supply(source.backing_access(types), argument.access)
        {
            return false;
        }
        // A static subloan preserves its root's custody. Field/index order is
        // checked by type resolution below, not by a source-shape roster. An
        // owned root is also a parent: the loan table grants it the exclusive
        // projected subloans a mutable parent grants.
        let static_borrowed_path = !argument.path.is_empty()
            && matches!(
                (source.access, argument.access),
                (
                    terminal_psi::StructuralAccess::MutableBorrow,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) | (
                    terminal_psi::StructuralAccess::SharedBorrow,
                    terminal_psi::StructuralAccess::SharedBorrow
                ) | (
                    terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) | (
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        | terminal_psi::StructuralAccess::SharedBorrow
                )
            )
            && argument.path.iter().all(|segment| match segment {
                terminal_psi::StructuralPathSegment::Field(identity) => !identity.is_empty(),
                terminal_psi::StructuralPathSegment::FixedIndex(_)
                | terminal_psi::StructuralPathSegment::Case(_)
                | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => true,
                terminal_psi::StructuralPathSegment::Referent
                | terminal_psi::StructuralPathSegment::FixedByteRange { .. } => false,
            });
        let path_shape_matches = match projection {
            StructuralProjectionPolicy::Unit => {
                argument.path.is_empty()
                    || static_borrowed_path
                    || is_nonempty_field_path(&argument.path)
                    || matches!(
                        argument.path.as_slice(),
                        [terminal_psi::StructuralPathSegment::FixedIndex(_)]
                            | [
                                terminal_psi::StructuralPathSegment::FixedIndex(_),
                                terminal_psi::StructuralPathSegment::FixedIndex(_),
                            ]
                    )
                    || (source.access == terminal_psi::StructuralAccess::Owned
                        && argument.access == terminal_psi::StructuralAccess::Owned
                        && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && is_partial_affine_path(types, source.structural_type, &argument.path))
            }
            StructuralProjectionPolicy::Projected => true,
            StructuralProjectionPolicy::Boundary => true,
        };
        // Borrowing lends completed storage without transferring carried permissions.
        let completed_result = source.operation_result.is_some_and(|result| {
            matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine
                    | terminal_psi::StructuralMultiplicity::Unrestricted
            ) && result.claims.is_empty()
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
        }) && !caller
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == argument.place)
            && !caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place);
        let completed_affine_result =
            completed_result && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine;
        let unrestricted_mutable_subloan = static_borrowed_path
            && argument.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
            && source.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        let unrestricted_shared_subloan = (is_nonempty_field_path(&argument.path)
            || static_borrowed_path)
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && (source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                || (source.access == terminal_psi::StructuralAccess::Owned
                    && completed_affine_result));
        // A live view binding lends its descriptor or a viewed element: its
        // custody stays with the caller, so the loan presents unrestricted
        // exactly like a parameter subloan — even when the view result
        // itself is affine (a mutable establishment's result is Affine, not
        // Unrestricted, yet nothing moves when the view is borrowed).
        let view_loan = argument.access != terminal_psi::StructuralAccess::Owned
            && matches!(
                source.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
            && types
                .get(&source.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    ) || (argument.path.is_empty() && declaration.shape.is_borrowed_byte_view())
                })
            && structural_access_can_supply(source.access, argument.access)
            && argument.access == parameter.access
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        // A bounded owned byte-sequence field supplies a borrowed byte-view
        // parameter without a structural type identity of its own. Boundary
        // calls admit the mutable presentation outright; ordinary borrowed
        // calls admit the same unrestricted subloans the verifier names.
        let buffer_presentation = (projection == StructuralProjectionPolicy::Boundary
            || (projection == StructuralProjectionPolicy::Unit && unrestricted_mutable_subloan))
            && terminal_semantics::boundary_buffer_capacity(
                types.values().copied(),
                source.structural_type,
                argument,
                parameter,
            )
            .is_some();
        // Shared observation is a storage presentation, not an arrangement
        // spelling. The already reconstructed source contract may name an
        // entry arrival, state arrival, local home or completed result. Exact
        // access, multiplicity and result evidence stay independent here;
        // current ownership later proves that the source is live.
        let shared_buffer_presentation = parameter.multiplicity
            == terminal_psi::StructuralMultiplicity::Unrestricted
            && matches!(
                source.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            )
            && source.projected_qualifications.is_empty()
            && !source.has_result_evidence()
            && terminal_semantics::shared_boundary_buffer_capacity(
                types.values().copied(),
                source.structural_type,
                argument,
                parameter,
            )
            .is_some();
        // A stored shared view field lends its own descriptor to a shared
        // view formal; its declared predicates must imply the formal's.
        let view_field_presentation = (projection == StructuralProjectionPolicy::Boundary
            || (projection == StructuralProjectionPolicy::Unit && unrestricted_shared_subloan))
            && terminal_semantics::stored_view_field_presentation(
                types.values().copied(),
                source.structural_type,
                argument,
                parameter,
            )
            .is_some_and(|declared| {
                terminal_semantics::byte_parameter_predicates_in(
                    domains.values().copied(),
                    parameter,
                )
                .is_some_and(|required| declared.implies(required))
            });
        let byte_field_presentation =
            buffer_presentation || shared_buffer_presentation || view_field_presentation;
        // Fixed windows borrow their real backing; they do not resolve to an
        // owned structural subtree. Reconstruct the exact array/view relation
        // even after optimization instead of treating equal lengths as identity.
        // Scalar-result and dynamic calls lend the same window a Unit or
        // boundary call does: the borrowed subloan shape is identical and a
        // scalar or dispatched result carries no custody of the backing.
        let presents_fixed_array = |actual: &terminal_psi::StructuralParameterDeclaration| {
            terminal_semantics::fixed_byte_array_window(
                types.values().copied(),
                actual,
                argument,
                parameter,
            )
            .is_some()
                || terminal_semantics::fixed_element_array_extent(
                    types.values().copied(),
                    actual,
                    argument,
                    parameter,
                )
                .is_some()
        };
        // The resolved source contract owns access and custody for every
        // current home. Geometry may present that backing through a loan,
        // without replacing owned authority or recognizing producer families.
        let fixed_array_presentation = matches!(
            projection,
            StructuralProjectionPolicy::Unit
                | StructuralProjectionPolicy::Projected
                | StructuralProjectionPolicy::Boundary
        ) && !source.has_result_evidence()
            && presents_fixed_array(&terminal_psi::StructuralParameterDeclaration {
                place: argument.place,
                position: parameter.position,
                is_self: false,
                structural_type: source.structural_type,
                multiplicity: source.multiplicity,
                access: source.access,
                qualifications: source.qualifications.to_vec(),
                projected_qualifications: source.projected_qualifications.to_vec(),
            })
            && !caller
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == argument.place)
            && !caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place);
        let borrowed_owner_expected = matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) && types.get(&parameter.structural_type).is_some_and(
            |declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
            },
        );
        let bounded_owner_presentation = borrowed_owner_expected
            && projection != StructuralProjectionPolicy::Boundary
            && (caller
                .arrival_parameter(argument.place)
                .is_some_and(|actual| {
                    terminal_semantics::bounded_byte_owner_capacity_in(
                        types.values().copied(),
                        domains.values().copied(),
                        actual,
                        argument,
                        parameter,
                    )
                    .is_some()
                })
                || owned_bounded_owner_loan(caller, argument, parameter, &source, types, domains))
            && !caller
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == argument.place)
            && !caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place);
        // Raw carrier equality cannot weaken a qualified owner's exclusive loan.
        if borrowed_owner_expected && !bounded_owner_presentation {
            return false;
        }
        let actual_type = if case_projection {
            terminal_semantics::canonical_static_projection(
                types.values().copied(),
                source.structural_type,
                &argument.path,
            )
            .map(|(_, declaration)| declaration.id)
        } else {
            resolve_structural_path(types, source.structural_type, &argument.path)
        };
        if actual_type.is_none()
            && !byte_field_presentation
            && !fixed_array_presentation
            && !bounded_owner_presentation
        {
            return false;
        }
        // The owned parent of an exclusive mutable subloan suspends the same
        // subtree a mutable parent does. The resolved leaf must still name
        // the parameter's type, so inline byte-field presentations — which
        // resolve to no standalone type — stay borrowed-parent forms.
        let unrestricted_owned_mutable_subloan = static_borrowed_path
            && argument.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
            && source.access == terminal_psi::StructuralAccess::Owned
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && (source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                || completed_affine_result)
            && actual_type == Some(parameter.structural_type);
        if (!path_shape_matches && !fixed_array_presentation)
            || (actual_type != Some(parameter.structural_type)
                && !fixed_array_presentation
                && !byte_field_presentation
                && !bounded_owner_presentation)
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let indexed_write_only_path_is_material = || {
            !argument.path.iter().any(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            }) || (is_material_write_only_type(types, source.structural_type)
                && actual_type
                    .and_then(|actual| types.get(&actual))
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                                | terminal_psi::StructuralTypeShape::Record { .. }
                        )
                    }))
        };
        let unrestricted_write_only_subloan = !argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && matches!(
                source.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && indexed_write_only_path_is_material();
        // Terminal admits indexed write-only arguments only as unrestricted
        // material subloans. Linear fallback cannot supply that authority.
        if projection == StructuralProjectionPolicy::Unit
            && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && argument.path.iter().any(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            })
            && !unrestricted_write_only_subloan
        {
            return false;
        }
        // The shared loan is unrestricted; the original affine owner remains
        // live and retains its independent transfer or cleanup obligation.
        // A resolved Owned + Affine contract is already proof of a
        // caller-owned whole place — arrival parameters, produced results,
        // and establishments all resolve through it — so liveness belongs to
        // the ownership replay, not a producer roster here.
        let shared_affine_loan = argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.access == terminal_psi::StructuralAccess::Owned
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine;
        let completed_result_loan = completed_result
            && argument.access != terminal_psi::StructuralAccess::Owned
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        let shared_arrival_loan = source.access == terminal_psi::StructuralAccess::Owned
            && matches!(
                source.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine
                    | terminal_psi::StructuralMultiplicity::Unrestricted
            )
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && (argument.path.is_empty() || is_nonempty_field_path(&argument.path))
            && caller
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .any(|parameter| parameter.place == argument.place);
        let actual_multiplicity = if shared_affine_loan
            || completed_result_loan
            || shared_arrival_loan
            || view_loan
            || shared_buffer_presentation
        {
            terminal_psi::StructuralMultiplicity::Unrestricted
        } else if argument.path.is_empty() {
            source.multiplicity
        } else if unrestricted_write_only_subloan
            || fixed_array_presentation
            || bounded_owner_presentation
            || unrestricted_mutable_subloan
            || unrestricted_owned_mutable_subloan
            || unrestricted_shared_subloan
            || (buffer_presentation
                && source.access == terminal_psi::StructuralAccess::MutableBorrow
                && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted)
        {
            terminal_psi::StructuralMultiplicity::Unrestricted
        } else if parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            && is_partial_affine_path(types, source.structural_type, &argument.path)
        {
            terminal_psi::StructuralMultiplicity::Affine
        } else {
            terminal_psi::StructuralMultiplicity::Linear
        };
        let copied_byte_predicates = projection != StructuralProjectionPolicy::Boundary
            && !parameter.is_self
            && argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::Owned
            && parameter.access == terminal_psi::StructuralAccess::Owned
            && source.structural_type == parameter.structural_type
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.projected_qualifications.is_empty()
            && !caller
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == argument.place)
            && !caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
            && terminal_semantics::byte_parameter_predicates_in(
                domains.values().copied(),
                parameter,
            )
            .zip(consumer.and_then(|consumer| {
                crate::unit_validation::function_structure::copied_field_predicates_at_call(
                    types.values().copied(),
                    caller,
                    argument.place,
                    consumer,
                )
            }))
            .is_some_and(|(required, available)| available.implies(required));
        // Replay primitive byte membership in the current graph; producer
        // annotations cannot manufacture a source's content predicates.
        let byte_content_qualifications = argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            && types.values().any(|row| {
                row.id == parameter.structural_type
                    && matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(_)
                    )
            })
            && terminal_semantics::byte_parameter_predicates_in(
                domains.values().copied(),
                parameter,
            )
            .zip(
                crate::unit_validation::function_structure::whole_source_predicates(
                    types.values().copied(),
                    domains.values().copied(),
                    caller,
                    argument.place,
                ),
            )
            .is_some_and(|(required, available)| available.implies(required));
        if actual_multiplicity != parameter.multiplicity
            || (!bounded_owner_presentation
                && !view_field_presentation
                && !copied_byte_predicates
                && !byte_content_qualifications
                && parameter.qualifications.iter().any(|qualification| {
                    !source.carries_qualification(&argument.path, *qualification)
                }))
            || parameter
                .projected_qualifications
                .iter()
                .any(|qualification| {
                    let mut path = argument.path.clone();
                    path.extend(qualification.path.iter().cloned());
                    !source.carries_qualification(&path, qualification.domain)
                })
            || (projection == StructuralProjectionPolicy::Unit
                && !bounded_owner_presentation
                && !argument.path.is_empty()
                && (!source.qualifications.is_empty()
                    || ((unrestricted_write_only_subloan
                        || unrestricted_mutable_subloan
                        || unrestricted_owned_mutable_subloan)
                        && !parameter.qualifications.is_empty())))
        {
            return false;
        }
    }
    let receiver = parameters.iter().position(|parameter| parameter.is_self);
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            // The receiver's claim is the callee's own context: a borrow
            // strictly inside it is a subloan under the loan parent-child
            // table, never an independent second claim.
            let interior_loan = receiver.is_some_and(|index| {
                (first == index && interior_member_borrow(left, right))
                    || (second == index && interior_member_borrow(right, left))
            });
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && !interior_loan
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access)
                    // Moving an affine owner is exclusive even when the other
                    // argument only borrows a child. Block arrivals retain that
                    // obligation just like parameters and operation results.
                    || ((left.access == terminal_psi::StructuralAccess::Owned
                        || right.access == terminal_psi::StructuralAccess::Owned)
                        && structural_source_contract(caller, left.place)
                            .is_some_and(|source| source.access == terminal_psi::StructuralAccess::Owned
                                && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine)))
            {
                return false;
            }
        }
    }
    true
}

/// Indexed non-observing projection cannot traverse stored references or
/// descriptors. The validated catalog is acyclic; shared children are inspected
/// once without imposing a private depth limit.
fn is_material_write_only_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            return false;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::Atomic(_)
                        | StructuralFieldType::IeeeFloat(_) => {}
                        StructuralFieldType::Structural(child) => pending.push(child),
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::PredicateByteSequence { .. }
                        | StructuralFieldType::RefinedScalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::Erased { .. } => return false,
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            // Service authority is not write-only material storage.
            StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => return false,
        }
    }
    true
}

/// A whole completed bounded byte owner lent shared to the same owner carrier.
/// Every byte predicate the formal requires is replayed from the result's
/// current invariants in this graph. Producer identity is already authenticated
/// by the structural catalog and source contract; this storage presentation
/// does not depend on which operation established it.
fn owned_bounded_owner_loan(
    caller: &PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    source: &StructuralSourceContract<'_>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    if !argument.path.is_empty()
        || argument.access != terminal_psi::StructuralAccess::SharedBorrow
        || parameter.access != terminal_psi::StructuralAccess::SharedBorrow
        || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || parameter.is_self
        || !parameter.projected_qualifications.is_empty()
        || source.access != terminal_psi::StructuralAccess::Owned
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !source.projected_qualifications.is_empty()
        || !source.operation_result.is_some_and(|result| {
            result.place == argument.place
                && result.structural_type == source.structural_type
                && result.multiplicity == source.multiplicity
                && result.claims.is_empty()
                && result.qualification_establishments.is_empty()
        })
        || !types
            .get(&source.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
            })
        || source.structural_type != parameter.structural_type
    {
        return false;
    }
    let Some(required) =
        terminal_semantics::byte_parameter_predicates_in(domains.values().copied(), parameter)
    else {
        return false;
    };
    required.is_empty()
        || crate::unit_validation::function_structure::whole_source_predicates(
            types.values().copied(),
            domains.values().copied(),
            caller,
            argument.place,
        )
        .is_some_and(|available| available.implies(required))
}

/// Reconstructs declared custody without deciding whether a particular use is
/// legal. Literal descriptors have the same source facts at every consumer;
/// availability, backing access, predicates and ownership are checked separately.
pub(crate) fn structural_source_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .arrival_parameter(place)
        .map(|parameter| StructuralSourceContract {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
            operation_result: None,
        })
        .or_else(|| {
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| structural_operation_result_contract(caller, &node.operation))
                .find(|source| {
                    source
                        .operation_result
                        .is_some_and(|result| result.place == place)
                })
        })
        .or_else(|| trivial_affine_local_contract(caller, place))
        .or_else(|| {
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| {
                    let O::EstablishByteSequenceLiteral {
                        place: declaration,
                        structural_type,
                        qualifications,
                        ..
                    } = &node.operation
                    else {
                        return None;
                    };
                    (declaration.id == place).then_some(StructuralSourceContract {
                        structural_type: structural_type.id,
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        qualifications: qualifications.as_slice(),
                        projected_qualifications: &[],
                        operation_result: None,
                    })
                })
        })
}

/// An empty-record local minted by `EstablishTrivialAffineLocal` is an owned
/// affine value: ownership replay inserts exactly that custody at the
/// establishment, so a call consuming the local sees the same contract. The
/// establishment carries no qualification roster.
fn trivial_affine_local_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishTrivialAffineLocal {
                place: declaration,
                structural_type,
                ..
            } if declaration.id == place => Some(StructuralSourceContract {
                structural_type: structural_type.id,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                access: terminal_psi::StructuralAccess::Owned,
                qualifications: &[],
                projected_qualifications: &[],
                operation_result: None,
            }),
            _ => None,
        })
}

/// The exact result and its storage access share one operation classification.
/// Owning a result row is not sufficient: local storage and view operations
/// expose loans, while a reference result owns only its permission carrier.
pub(crate) fn structural_operation_result_contract<'a>(
    caller: &PsiOptimizationFunction,
    operation: &'a O,
) -> Option<StructuralSourceContract<'a>> {
    let (result, access) = match operation {
                // Activation-local storage supplies loans, never owned escape.
                O::EstablishPrimitiveLocal { result, .. } => {
                    (result, terminal_psi::StructuralAccess::MutableBorrow)
                }
                O::ByteSequenceSubslice { result, .. } | O::ElementViewSubslice { result, .. } => {
                    (result, view_access(caller, operation)?)
                }
                // The view's source access IS the loan's access: a mutable
                // establishment lends mutable, a shared one lends shared.
                O::EstablishElementView { result, .. } => (result, view_access(caller, operation)?),
                O::EstablishScalarArray { result, .. } | O::EstablishStructuralArray { result, .. }
                | O::EstablishCase { result, .. }
                | O::EstablishRecord { result, .. }
                | O::EstablishZeroedStructural { result, .. }
                | O::EstablishStructuralLocal { result, .. }
                | O::EstablishReference { result, .. }
                // The moved subtree leaves the window as an owned whole value;
                // the repair store consumes it with exactly that custody.
                | O::MoveStructuralField { result, .. }
                // A leaf copy establishes fresh owned storage the way an
                // establishment does: consumers must not alias it back into
                // the readable root's custody.
                | O::EstablishByteSequenceConcatenation { result, .. }
                | O::StructuralLeafCopy { result, .. }
                | O::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                }
                | O::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                } => (result, terminal_psi::StructuralAccess::Owned),
                _ => return None,
            };
    Some(StructuralSourceContract {
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access,
        qualifications: &result.qualifications,
        projected_qualifications: &result.projected_qualifications,
        operation_result: Some(result),
    })
}

/// Rejoin every descriptor with its current declaration and producer. Multiplicity
/// selects the requested loan; the exact source chain must supply it. The place
/// count bounds malformed cycles without recursive queries or guessed authority.
fn view_access(
    caller: &PsiOptimizationFunction,
    operation: &O,
) -> Option<terminal_psi::StructuralAccess> {
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};

    let (_, initial) = operation.structural_result()?;
    let mut place = initial.place;
    let mut current = Some(operation);
    let mut expected_type = Some(initial.structural_type);
    let mut requested = None;
    let mut required = Access::SharedBorrow;
    for _ in 0..caller.structural_places.len() {
        let mut declarations = caller
            .structural_places
            .iter()
            .filter(|row| row.id == place);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        if let Some(parameter) = caller.arrival_parameter(place) {
            if expected_type.is_some_and(|identity| identity != parameter.structural_type)
                || (expected_type.is_some()
                    && !matches!(
                        parameter.access,
                        Access::SharedBorrow | Access::MutableBorrow
                    ))
                || !structural_access_can_supply(parameter.access, required)
            {
                return None;
            }
            return requested;
        }
        let (producer, structural_type) = match declaration.kind {
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => (producer, structural_type),
            StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } if required == Access::SharedBorrow
                && expected_type.is_none_or(|identity| identity == structural_type) =>
            {
                return requested;
            }
            _ => return None,
        };
        let recorded = caller
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find(|node| node.operation.psi_operation() == Some(producer))?;
        let operation = current.take().unwrap_or(&recorded.operation);
        if operation != &recorded.operation {
            return None;
        }
        let (actual_producer, result) = operation.structural_result()?;
        if producer != actual_producer
            || result.place != place
            || result.structural_type != structural_type
            || expected_type.is_some_and(|identity| identity != structural_type)
        {
            return None;
        }
        match operation {
            O::ByteSequenceSubslice { source, .. } | O::ElementViewSubslice { source, .. } => {
                if !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                let access = match result.multiplicity {
                    Multiplicity::Unrestricted => Access::SharedBorrow,
                    Multiplicity::Affine => Access::MutableBorrow,
                    _ => return None,
                };
                if !structural_access_can_supply(access, required) {
                    return None;
                }
                requested.get_or_insert(access);
                required = access;
                expected_type = Some(structural_type);
                place = *source;
            }
            O::EstablishElementView {
                source,
                destination,
                ..
            } => {
                // A reference crossing needs the typed referent-permission
                // validator; ownership of the carrier grants no such access.
                if source
                    .path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                    || *destination != result.place
                    || !matches!(
                        (source.access, result.multiplicity),
                        (Access::SharedBorrow, Multiplicity::Unrestricted)
                            | (Access::MutableBorrow, Multiplicity::Affine)
                    )
                    || !structural_access_can_supply(source.access, required)
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                requested.get_or_insert(source.access);
                required = source.access;
                expected_type = None;
                place = source.place;
            }
            _ => {
                // A subslice consumes a descriptor. Only an establishment may
                // change carrier shape before reaching its owning backing.
                if expected_type.is_some() {
                    return None;
                }
                let source = structural_operation_result_contract(caller, operation)?;
                return if structural_access_can_supply(source.access, required) {
                    requested
                } else {
                    None
                };
            }
        }
    }
    None
}

pub(crate) fn structural_access_can_supply(
    source: terminal_psi::StructuralAccess,
    presented: terminal_psi::StructuralAccess,
) -> bool {
    match source {
        terminal_psi::StructuralAccess::Owned => true,
        terminal_psi::StructuralAccess::SharedBorrow => {
            presented == terminal_psi::StructuralAccess::SharedBorrow
        }
        terminal_psi::StructuralAccess::MutableBorrow => matches!(
            presented,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ),
        terminal_psi::StructuralAccess::WriteOnlyBorrow => {
            presented == terminal_psi::StructuralAccess::WriteOnlyBorrow
        }
    }
}

pub(crate) fn structural_access_is_exclusive(access: terminal_psi::StructuralAccess) -> bool {
    matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) fn is_nonempty_field_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
}

pub(crate) fn is_partial_affine_path(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> bool {
    !path.is_empty() && resolve_structural_path(types, root, path).is_some()
}
