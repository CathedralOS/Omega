/*
    Structural type, shape, claim, and return-custody helpers.
*/

use super::{
    BTreeMap, BTreeSet, CheckFacts, CheckedBoundaryMachineResultPlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, DataMember, DataShapeKind,
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PrimitiveType, ProofFact,
    SemanticDomainId, SignatureContractKind, StateParameter, SymbolHandle, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedExtent, CheckedStructuralPathQualification, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::type_identity::TypeIdentityRequest;

mod call_qualifications {
    /*
        A completed call can establish a domain independently of its storage's
        declared qualifications. Retain that selected theory on the exact field or
        referent carrier before catalog pruning. This describes checked evidence;
        it introduces no membership, entry assumption, or new storage authority.
    */
    use super::*;
    use crate::execution::terminal_unit::symbol_key;

    pub(in crate::execution::terminal_unit) fn retain_call_qualification_domains<'operation>(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        operations: impl Iterator<
            Item = (
                SymbolHandle,
                SymbolHandle,
                &'operation CheckedUnitEffectOperationPlan,
            ),
        >,
        retained: &mut BTreeSet<typed_trees::type_identity::TypeIdentityId>,
    ) {
        let coordinates = operations
            .filter_map(|(_, state, operation)| {
                let call = operation.call()?;
                Some((
                    symbol_key(state),
                    call.coordinate.statement_index as usize,
                    call.coordinate.call_ordinal as usize,
                ))
            })
            .collect::<BTreeSet<_>>();
        let mut binders = BTreeMap::new();
        for (_, fact) in facts.semantic.facts.iter() {
            let facts::ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            } = fact.point
            else {
                continue;
            };
            if fact.origin != facts::FactOrigin::CallEnsures
                || !coordinates.contains(&(symbol_key(state_symbol), statement_index, call_ordinal))
            {
                continue;
            }
            let Some((_, _, domain_symbol, domain)) = fact.payload.domain_membership() else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let place = facts.semantic.places.get(place);
            let selected = validation::canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &validation::CanonicalPlace {
                    root: place.root,
                    segments: facts
                        .semantic
                        .place_segments
                        .span_or_empty(place.segments)
                        .to_vec(),
                },
            );
            let Some((_, carrier)) = selected.and_then(|selected| {
                validation::result_membership_carrier(program, selected, domain_symbol)
            }) else {
                continue;
            };
            if program.primitive_type_reference(carrier).is_some() {
                continue;
            }
            let parameters = binders
                .entry(symbol_key(machine_symbol))
                .or_insert_with(|| {
                    program
                        .machine_by_symbol(machine_symbol)
                        .map(|machine| machine_binders(program, machine))
                        .unwrap_or_default()
                });
            if let Some(identity) =
                shapes.add_selected_domain(domain, domain_symbol, carrier, parameters, true)
            {
                retained.insert(identity);
            }
        }
    }
}
pub(super) use call_qualifications::retain_call_qualification_domains;
mod declared_result {
    /*
        Authored callable results shared by ordinary bodies and composed graphs.
        Completion plans prove actual returns; they never choose the declaration's
        scalar/structural carrier or erase it when no path returns normally.
    */
    use super::{
        Multiplicity, ShapeCollector, TypeReferenceHandle, is_reference, is_unit,
        parameter_qualifications, projected_result_qualifications,
    };
    use crate::execution::terminal_unit::types::borrowed_slice_result;
    use checked_trees::{CheckedControlResultPlan as ResultPlan, CheckedStructuralResultPlan};

    /*
        Retain the authored entry result and its domains independently of body
        routing, executed result production, and normal-return receiving bindings.
    */
    pub(in crate::execution::terminal_unit) fn declared_result_plan(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
    ) -> Option<checked_trees::CheckedControlResultPlan> {
        let entry = program.machine_states(machine).first()?;
        let result = declared_result_type(program, shapes, entry.return_type)?;
        super::register_authored_result_domains(
            program,
            shapes,
            machine,
            entry,
            &super::machine_binders(program, machine),
        )?;
        Some(result)
    }

    pub(in crate::execution::terminal_unit) fn declared_result_type(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        reference: TypeReferenceHandle,
    ) -> Option<ResultPlan> {
        if is_unit(program, reference) {
            return Some(ResultPlan::Unit);
        }
        if let Some(scalar_type) = checked_trees::CheckedScalarType::from_type_reference(
            program.typed_program(),
            reference,
        ) {
            let represented = match scalar_type {
                checked_trees::CheckedScalarType::Primitive(_) => {
                    validation::scalar_result_contract_is_represented(program, reference)
                }
                checked_trees::CheckedScalarType::Callable(_) => program
                    .runtime_callable_type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(reference),
                    )
                    .is_ok(),
            };
            return represented.then_some(ResultPlan::Scalar { scalar_type });
        }
        let multiplicity = crate::checks::type_multiplicity(program, reference);
        let pointer_result =
            validation::reference_result_custody::has_borrowed_parts(program, reference);
        let view_result = pointer_result || borrowed_slice_result(program, reference);
        let qualifications = if view_result {
            let qualifications =
                validation::view_result_carrier_qualifications(program, reference).ok()?;
            for domain in &qualifications {
                shapes.add_domain(*domain, reference, &[])?;
            }
            qualifications
        } else {
            parameter_qualifications(program, shapes, reference, &[])?
        };
        let represented_copy = multiplicity == Multiplicity::Unrestricted
            && validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::RepresentedOwned,
            );
        let owned_bytes = multiplicity == Multiplicity::Unrestricted
            && validation::bounded_byte_buffer_capacity(program, reference).is_some()
            && validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::RepresentedOwned,
            );
        let claim_bearing = !validation::linear_claim_frontier(program, reference).is_empty()
            && validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::StructuralConstruction,
            );
        if (is_reference(program, reference) && !view_result)
            || (multiplicity != Multiplicity::Linear
                && !claim_bearing
                && !owned_bytes
                && (!view_result
                    && !represented_copy
                    && !validation::reference_result_custody::is_reference_record(
                        program, reference,
                    )
                    && !validation::has_contents(
                        program,
                        reference,
                        &[],
                        validation::ContentsRequirement::BorrowFree,
                    )
                    && !validation::has_contents(
                        program,
                        reference,
                        &[],
                        validation::ContentsRequirement::OwnedOrSharedView,
                    )))
        {
            return None;
        }
        let (type_identity, multiplicity) = if borrowed_slice_result(program, reference) {
            (
                shapes.add_stored_type(reference, &[], &[])?,
                validation::reference_result_custody::binding_multiplicity(program, reference),
            )
        } else if pointer_result {
            (
                shapes.add_reference_type(reference, &[])?,
                Multiplicity::Affine,
            )
        } else {
            (
                shapes.add_type(reference, &[], &[])?,
                validation::reference_result_custody::result_multiplicity(program, reference),
            )
        };
        Some(ResultPlan::Structural(CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications: projected_result_qualifications(
                program,
                shapes,
                reference,
                &[],
            )?,
        }))
    }
}
pub(super) use declared_result::{declared_result_plan, declared_result_type};
mod domain_predicates {
    /*
        Retain complete declaration meaning, never an inferred write permission list.
        Unknown definitions remain opaque. Normalized atoms close conjunctions before
        this catalog; indexed/open definitions require their exact substituted theory.
    */
    use checked_trees::{CheckedOperatorFacts, CheckedUnitStructuralDomainPlan};
    use typed_trees::types::TypeReferenceHandle;

    pub(in crate::execution::terminal_unit) fn retain_domain_predicates(
        program: &dyn validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        plans: &mut [CheckedUnitStructuralDomainPlan],
    ) {
        for plan in plans.iter_mut() {
            let mut definitions = program
                .domain_definitions()
                .iter()
                .chain(
                    program
                        .typed_program()
                        .applied_domain_applications
                        .iter()
                        .map(|application| &application.theory),
                )
                .filter(|domain| domain.semantic_id == plan.domain);
            let Some(domain) = definitions.next() else {
                continue;
            };
            if definitions.next().is_some() || domain.alias.is_some() {
                continue;
            }
            let parameters = [typed_trees::signature::StateParameter {
                symbol: domain.subject_symbol,
                name: domain.subject_name.clone(),
                type_reference: domain.target_type,
                ..Default::default()
            }];
            let mut remaining = 4096;
            let retained_memberships = plan.required_memberships.as_ref();
            let predicates = program
                .proof_facts(domain)
                .iter()
                .map(|fact| match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        crate::values::lower_callable_scalar_contract_predicate(
                            program,
                            operators,
                            domain.symbol,
                            &parameters,
                            TypeReferenceHandle::invalid(),
                            *expression,
                            false,
                            &mut remaining,
                        )
                    }
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        if retained_memberships
                            .is_some_and(|required| required.contains(&membership.semantic_domain))
                        {
                            return Some(checked_trees::CheckedBooleanExpression::Constant(true));
                        }
                        crate::values::scalar::result_contract::lower_callable_scalar_membership(
                            program,
                            operators,
                            domain.symbol,
                            &parameters,
                            membership,
                            &mut remaining,
                        )
                    }
                    typed_trees::domain::ProofFact::Proposition(_) => None,
                })
                .collect::<Option<Vec<_>>>();
            plan.predicate = predicates.map(|predicates| {
                predicates
                    .into_iter()
                    .reduce(|left, right| checked_trees::CheckedBooleanExpression::And {
                        left: Box::new(left),
                        right: Box::new(right),
                    })
                    .unwrap_or(checked_trees::CheckedBooleanExpression::Constant(true))
            });
            if plan.required_memberships.is_none() && plan.predicate.is_some() {
                plan.required_memberships = Some(Vec::new());
            }
        }

        fn complete(
            plans: &[CheckedUnitStructuralDomainPlan],
            index: usize,
            active: &mut Vec<usize>,
        ) -> Option<bool> {
            let plan = &plans[index];
            if active.contains(&index) {
                return None;
            }
            let Some(required) = &plan.required_memberships else {
                return Some(false);
            };
            active.push(index);
            let result = required
                .iter()
                .map(|domain| {
                    let mut matches = plans.iter().enumerate().filter(|(_, candidate)| {
                        candidate.domain == *domain
                            && candidate.carrier_type_identity == plan.carrier_type_identity
                    });
                    let (parent, _) = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    complete(plans, parent, active)
                })
                .collect::<Option<Vec<_>>>();
            active.pop();
            result.map(|parents| {
                plan.complete_predicate_theory
                    && plan.predicate.is_some()
                    && parents.into_iter().all(|parent| parent)
            })
        }
        let complete = (0..plans.len())
            .map(|index| complete(plans, index, &mut Vec::new()))
            .collect::<Vec<_>>();
        for (plan, complete) in plans.iter_mut().zip(complete) {
            if complete.is_none() {
                plan.required_memberships = None;
                plan.predicate = None;
            }
            plan.complete_predicate_theory = complete == Some(true);
        }

        fn value_predicate(
            plans: &[CheckedUnitStructuralDomainPlan],
            index: usize,
            active: &mut Vec<usize>,
            values: &mut [Option<Option<checked_trees::CheckedBooleanExpression>>],
        ) -> Option<checked_trees::CheckedBooleanExpression> {
            use checked_trees::CheckedBooleanExpression as Boolean;
            if let Some(value) = &values[index] {
                return value.clone();
            }
            if active.contains(&index) {
                return None;
            }
            let plan = &plans[index];
            let mut value = plan.predicate.clone()?;
            active.push(index);
            let result = (|| {
                if let Some(required) = &plan.required_memberships {
                    for domain in required {
                        let mut matches = plans.iter().enumerate().filter(|(_, parent)| {
                            parent.domain == *domain
                                && parent.carrier_type_identity == plan.carrier_type_identity
                        });
                        let (parent, _) = matches.next()?;
                        if matches.next().is_some() {
                            return None;
                        }
                        let predicate = value_predicate(plans, parent, active, values)?;
                        value = match (value, predicate) {
                            (Boolean::Constant(true), right) => right,
                            (left, Boolean::Constant(true)) => left,
                            (Boolean::Constant(false), _) | (_, Boolean::Constant(false)) => {
                                Boolean::Constant(false)
                            }
                            (left, right) if left == right => left,
                            (left, right) => Boolean::And {
                                left: Box::new(left),
                                right: Box::new(right),
                            },
                        };
                    }
                }
                Some(value)
            })();
            active.pop();
            values[index] = Some(result.clone());
            result
        }
        let mut values = vec![None; plans.len()];
        for index in 0..plans.len() {
            value_predicate(plans, index, &mut Vec::new(), &mut values);
        }
        for (plan, value) in plans.iter_mut().zip(values) {
            plan.predicate = value.flatten();
            plan.complete_predicate_theory &= plan.predicate.is_some();
        }
    }
}
pub(super) use domain_predicates::retain_domain_predicates;
mod scalar_fields {
    /*
        Complete scalar field restrictions retained independently of the raw carrier.
        Intersect exact proof-integer endpoints before converting to the field's
        signed or unsigned carrier; an authored endpoint can exceed that carrier.
        General domain predicates use the shared declaration-owned projection and
        remain conjoined with every retained interval shell.
    */
    use super::{
        CheckedUnitStructuralFieldType as FieldType, DataMember, SymbolHandle, TypeReferenceHandle,
        TypeReferenceNode, substituted_formal_type,
    };
    use numerics::bignum::BigInt;
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue};

    /*
        Whether lending a scalar field must remain an owning-record observation.
        Exclusive access retains its complete receiving restriction, and atomic
        storage never becomes an ordinary scalar reference. A shared loan may expose
        a bounded or refined scalar's carrier: it retains the exact backing and
        lifetime but supplies no store or invariant-establishment authority.
    */
    pub(crate) fn field_requires_place_observation(
        program: &dyn ::validation::CheckingInput,
        field: SymbolHandle,
        leaf_type: TypeReferenceHandle,
        access: checked_trees::CheckedStructuralAccess,
    ) -> bool {
        let Some(primitive) = program.primitive_type_reference(leaf_type) else {
            return false;
        };
        if access != checked_trees::CheckedStructuralAccess::SharedBorrow
            && program
                .any_field_owner(field)
                .is_some_and(|(definition, field)| {
                    !validation::data_field_is_requires_independent(program, definition, field)
                })
        {
            return true;
        }
        match retain_scalar_field(
            program,
            leaf_type,
            &[],
            primitive,
            stated_field_interval(program, field),
        ) {
            Some(FieldType::Scalar(_)) => false,
            Some(FieldType::BoundedInteger(_)) | Some(FieldType::RefinedScalar { .. })
                if access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
            {
                false
            }
            _ => true,
        }
    }

    /*
        Whether primitive storage can retain the declaration's complete receiving
        restriction. Parameters and local declarations establish the same storage;
        replacements use the exact assignment judgment against its full restriction.
        A domain kept only in custody (such as Finite) is not an unrestricted store,
        Closed predicates retain their declaration-owned receiving operation.
    */
    pub(in crate::execution::terminal_unit) fn supports_primitive_storage(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let Some(primitive) = program.primitive_type_reference(type_reference) else {
            return false;
        };
        match retain_scalar_field(program, type_reference, &[], primitive, None) {
            Some(FieldType::Scalar(_)) => {
                validation::is_plain_scalar_storage(program, type_reference)
            }
            Some(FieldType::BoundedInteger(_)) | Some(FieldType::RefinedScalar { .. }) => true,
            _ => false,
        }
    }

    /*
        The owner's interval-only `requires` bound on `field`: the record's facts for
        a direct field, the selected case's facts for a payload field.
    */
    fn stated_field_interval(
        program: &dyn ::validation::CheckingInput,
        field: SymbolHandle,
    ) -> Option<(Option<i64>, Option<i64>)> {
        let (definition, _) = program.any_field_owner(field)?;
        let intervals = if program.data_field(definition, field).is_some() {
            validation::data_requires_field_intervals(program, definition)
        } else {
            let variant =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if program
                                .data_payload_fields(variant)
                                .iter()
                                .any(|payload| payload.symbol == field) =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })?;
            validation::case_requires_field_intervals(program, variant)
        }?;
        intervals
            .into_iter()
            .find(|(symbol, _, _)| *symbol == field)
            .map(|(_, low, high)| (low, high))
    }

    pub(in crate::execution::terminal_unit) fn retain_scalar_field(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scalar_type: impl Into<checked_trees::CheckedScalarType>,
        stated: Option<(Option<i64>, Option<i64>)>,
    ) -> Option<FieldType> {
        let scalar_type = scalar_type.into();
        let primitive = match scalar_type {
            checked_trees::CheckedScalarType::Primitive(primitive) => primitive,
            checked_trees::CheckedScalarType::Callable(_) => {
                let declared = checked_trees::CheckedScalarType::from_type_reference(
                    program.typed_program(),
                    type_reference,
                )?;
                if stated.is_some()
                    || !declared
                        .same_type(program.typed_program(), scalar_type)
                        .ok()?
                {
                    return None;
                }
                program
                    .runtime_callable_type_identity(
                        typed_trees::type_identity::TypeIdentityRequest {
                            substitutions,
                            ..typed_trees::type_identity::TypeIdentityRequest::ordinary(
                                type_reference,
                            )
                        },
                    )
                    .ok()?;
                return Some(FieldType::Scalar(scalar_type));
            }
        };
        let mut declared_bounds: Option<(BigInt, BigInt)> = None;
        let mut has_non_exact_domain = false;
        let mut predicate = None;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain) =
                            constraint
                        {
                            has_non_exact_domain |=
                                *domain != numerics::arithmetic::ArithmeticDomain::Exact;
                        }
                        if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                            && domain.subject
                                == typed_trees::types::DomainConstraintSubject::Value(
                                    language_semantics::value_domain::ValueDomain::Finite,
                                )
                        {
                            continue;
                        }
                        if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                            let Some((lower, upper)) =
                                validation::exact_declared_domain_carrier_interval(
                                    program, primitive, domain,
                                )
                            else {
                                let next = validation::scalar_domain_value_predicate(
                                    program,
                                    domain,
                                    checked_trees::CheckedScalarExpression::Parameter {
                                        position: 0,
                                        scalar_type: (primitive).into(),
                                    },
                                )?;
                                predicate = Some(match predicate {
                                    None => next,
                                    Some(previous) => {
                                        checked_trees::CheckedBooleanExpression::And {
                                            left: Box::new(previous),
                                            right: Box::new(next),
                                        }
                                    }
                                });
                                continue;
                            };
                            declared_bounds = Some(match declared_bounds {
                                Some((previous_lower, previous_upper)) => {
                                    (previous_lower.max(lower), previous_upper.min(upper))
                                }
                                None => (lower, upper),
                            });
                        }
                        if matches!(constraint, typed_trees::types::TypeConstraintNode::Named(_)) {
                            return None;
                        }
                    }
                    type_reference = *base_type;
                }
                TypeReferenceNode::Named { .. } => {
                    let resolved = substituted_formal_type(program, type_reference, substitutions);
                    if resolved == type_reference {
                        break;
                    }
                    type_reference = resolved;
                }
                TypeReferenceNode::DynamicTrait { .. } => break,
                _ => return None,
            }
        }
        let atomic = validation::is_atomic_type(program, type_reference);
        if atomic {
            if let Some((lower, upper)) = stated {
                let lower = lower.map_or_else(|| BigInt::from_i128(i128::MIN), BigInt::from_i64);
                let upper = upper.map_or_else(|| BigInt::from_u128(u128::MAX), BigInt::from_i64);
                declared_bounds = Some(match declared_bounds {
                    Some((previous_lower, previous_upper)) => {
                        (previous_lower.max(lower), previous_upper.min(upper))
                    }
                    None => (lower, upper),
                });
            }

            if predicate.is_some() || has_non_exact_domain {
                return None;
            }
            if declared_bounds.is_none() {
                return Some(FieldType::Atomic(primitive));
            }
        }
        if has_non_exact_domain && (declared_bounds.is_some() || predicate.is_some()) {
            return None;
        }
        let Some((minimum, maximum)) = declared_bounds else {
            return validation::scalar_field_restriction(primitive, None, predicate);
        };
        let integer_type = crate::values::integer_type(primitive)?;
        let carrier_integer = |value| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        let minimum = minimum.max(carrier_integer(integer_type.minimum_value()));
        let maximum = maximum.min(carrier_integer(integer_type.maximum_value()));
        if minimum > maximum {
            if atomic {
                return None;
            }
            return validation::scalar_field_restriction(
                primitive,
                None,
                Some(checked_trees::CheckedBooleanExpression::Constant(false)),
            );
        }
        let (minimum, maximum) = match (integer_type.minimum_value(), integer_type.maximum_value())
        {
            (IntegerValue::Signed(carrier_minimum), IntegerValue::Signed(carrier_maximum)) => (
                IntegerValue::Signed(i128::from(
                    minimum.max(BigInt::from_i128(carrier_minimum)).to_i64()?,
                )),
                IntegerValue::Signed(i128::from(
                    maximum.min(BigInt::from_i128(carrier_maximum)).to_i64()?,
                )),
            ),
            (IntegerValue::Unsigned(carrier_minimum), IntegerValue::Unsigned(carrier_maximum)) => (
                IntegerValue::Unsigned(u128::from(
                    minimum.max(BigInt::from_u128(carrier_minimum)).to_u64()?,
                )),
                IntegerValue::Unsigned(u128::from(
                    maximum.min(BigInt::from_u128(carrier_maximum)).to_u64()?,
                )),
            ),
            _ => return None,
        };
        let bounds = BoundedIntegerType::new(integer_type, minimum, maximum).ok()?;
        if atomic {
            return Some(FieldType::AtomicBoundedInteger(bounds));
        }
        validation::scalar_field_restriction(primitive, Some(bounds), predicate)
    }
}
pub(crate) use scalar_fields::field_requires_place_observation;
pub(super) use scalar_fields::{retain_scalar_field, supports_primitive_storage};

mod affine_discards {
    /*
        Affine obligations an ordinary Unit return edge owes: whole-root discards and
        the residual complement of a partially moved owned parameter.
    */
    use super::*;
    use facts::PlaceRoot;
    use language_semantics::PermissionProvenance;
    use typed_trees::type_identity::TypeIdentityId;

    /*
        Whole-root discards plus the residual complement of a partially moved
        owned parameter. Only owned transfers consume that custody. Establishing a
        reference, projected or whole, borrows backing and leaves its source owner
        on the checked cleanup roster; a returned descriptor carries its own loan.
    */
    pub(in crate::execution::terminal_unit) fn return_unit_affine_discards(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
        machine: SymbolHandle,
        state: SymbolHandle,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        source_parameters: &[StateParameter],
        operations: &[CheckedUnitEffectOperationPlan],
        admitted_local_symbols: &[SymbolHandle],
        structural_types: &crate::execution::terminal_unit::types::TypeCatalog,
    ) -> Option<(Vec<u32>, Vec<CheckedUnitPartialAffineDiscardPlan>, bool)> {
        let selected_sources = facts
            .flow
            .ownership
            .with_state_owned_selections(state, |handles| {
                handles
                    .iter()
                    .copied()
                    .map(|handle| facts.flow.ownership.owned_selections.get(handle))
                    .filter(|receipt| receipt.machine == machine && receipt.state == state)
                    .flat_map(|receipt| {
                        facts
                            .flow
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .map(|source| source.symbol)
                    })
                    .collect::<Vec<_>>()
            });
        let mut transferred_parameters = BTreeSet::new();
        let mut moved_parameter_paths =
            BTreeMap::<u32, Vec<(Vec<CheckedUnitStructuralPathSegment>, TypeIdentityId)>>::new();
        for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
            if parameter.access != CheckedStructuralAccess::Owned {
                continue;
            }
            let source = source_parameters.get(parameter.position as usize)?;
            if selected_sources.contains(&source.symbol) {
                continue;
            }
            let parameter_index = u32::try_from(parameter_index).ok()?;
            let vacancies = super::super::storage_transfers::parameter_vacancies(
                program,
                facts,
                prepared_calls,
                parameter_index,
                parameter.position,
                source.symbol,
                parameter.multiplicity,
                operations,
            )?;
            if !vacancies.maybe.is_empty() {
                if vacancies.maybe.iter().any(|path| !path.is_empty())
                    || parameter.multiplicity != Multiplicity::Affine
                    || type_graph_requires_nominal_drop(
                        program,
                        source.type_reference,
                        &[],
                        &mut BTreeSet::new(),
                    )
                {
                    return None;
                }
                transferred_parameters.insert(parameter_index);
            }
            for path in vacancies.definite {
                if path.is_empty() {
                    transferred_parameters.insert(parameter_index);
                } else {
                    let endpoint =
                        projected_type(structural_types, parameter.type_identity, &path)?;
                    moved_parameter_paths
                        .entry(parameter_index)
                        .or_default()
                        .push((path, endpoint));
                }
            }
        }
        let mut residual_roots = BTreeSet::new();
        let mut residual_affine_discards = Vec::new();
        for (parameter_index, moved_paths) in &moved_parameter_paths {
            if transferred_parameters.contains(parameter_index) {
                continue;
            }
            let parameter = structural_parameters.get(*parameter_index as usize)?;
            if parameter.access != CheckedStructuralAccess::Owned {
                continue;
            }
            if !parameter.qualifications.is_empty() {
                return None;
            }
            let rows = crate::execution::terminal_unit::cleanup::partial_affine_residuals(
                structural_types,
                &CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: *parameter_index,
                },
                parameter.type_identity,
                moved_paths,
            )?;
            if rows.is_empty() {
                continue;
            }
            if parameter.multiplicity != Multiplicity::Affine {
                return None;
            }
            if !residual_roots.is_empty() {
                return None;
            }
            residual_roots.insert(*parameter_index);
            residual_affine_discards = rows;
        }
        let has_projected_parameter_moves = moved_parameter_paths.keys().any(|index| {
            !transferred_parameters.contains(index)
                && structural_parameters
                    .get(*index as usize)
                    .is_some_and(|parameter| {
                        parameter.access == CheckedStructuralAccess::Owned
                            && parameter.qualifications.is_empty()
                            && parameter.multiplicity == Multiplicity::Affine
                    })
        });
        let events = facts
            .flow
            .ownership
            .state_exit_cleanup_roots(machine, state)?
            .into_iter()
            .filter(|event| {
                event.access == PermissionAccess::Owned
                    && event.multiplicity == Multiplicity::Affine
                    && event.claim_identity == PermissionClaimIdentity::Unknown
                    && event.provenance == PermissionProvenance::Unknown
                    && !event.obligation_live
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .is_empty()
            })
            .collect::<Vec<_>>();
        let mut output = Vec::with_capacity(events.len());
        for event in events {
            let parameter_index = structural_parameters.iter().position(|parameter| {
                source_parameters
                    .get(parameter.position as usize)
                    .is_some_and(|source| event.root == PlaceRoot::Symbol((source).symbol))
            });
            if parameter_index.is_none() {
                let root = event.root.as_symbol()?;
                let erased_formal = source_parameters.iter().any(|source| {
                    strips_erased_parameter(source) == Some(true) && root == (source).symbol
                });
                if erased_formal || admitted_local_symbols.contains(&root) {
                    continue;
                }
                return None;
            }
            let parameter_index = parameter_index?;
            let parameter = &structural_parameters[parameter_index];
            if selected_sources
                .contains(&source_parameters.get(parameter.position as usize)?.symbol)
            {
                continue;
            }
            let parameter_index = u32::try_from(parameter_index).ok()?;
            if transferred_parameters.contains(&parameter_index)
                || residual_roots.contains(&parameter_index)
            {
                continue;
            }
            if moved_parameter_paths.contains_key(&parameter_index) {
                continue;
            }
            if parameter.multiplicity != Multiplicity::Affine || output.contains(&parameter_index) {
                return None;
            }
            output.push(parameter_index);
        }
        Some((
            output,
            residual_affine_discards,
            has_projected_parameter_moves,
        ))
    }

    /*
        Resolve a moved endpoint in the same checked shape catalog used to
        reconstruct its residual. Case identities select payload namespaces; a
        bare Case is not a value. No reference or runtime-index inference occurs.
    */
    fn projected_type(
        types: &crate::execution::terminal_unit::types::TypeCatalog,
        mut current: TypeIdentityId,
        mut path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<TypeIdentityId> {
        while let Some((segment, remaining)) = path.split_first() {
            let shape = &types.get(&current)?.shape;
            match (shape, segment) {
                (
                    CheckedUnitStructuralTypeShape::Record { fields, .. }
                    | CheckedUnitStructuralTypeShape::Mixed { fields, .. },
                    CheckedUnitStructuralPathSegment::Field(identity),
                ) => {
                    current = fields
                        .iter()
                        .find(|field| &field.identity == identity)?
                        .field_type
                        .structural_type_identity()?;
                    path = remaining;
                }
                (
                    CheckedUnitStructuralTypeShape::Sum { cases }
                    | CheckedUnitStructuralTypeShape::Mixed { cases, .. },
                    CheckedUnitStructuralPathSegment::Case(identity),
                ) => {
                    let case = cases.iter().find(|case| &case.identity == identity)?;
                    let (CheckedUnitStructuralPathSegment::Field(field), suffix) =
                        remaining.split_first()?
                    else {
                        return None;
                    };
                    current = case
                        .fields
                        .iter()
                        .find(|candidate| &candidate.identity == field)?
                        .field_type
                        .structural_type_identity()?;
                    path = suffix;
                }
                (
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    },
                    CheckedUnitStructuralPathSegment::FixedIndex(index),
                ) if length.admits_fixed_index(*index) => {
                    current = *element_type_identity;
                    path = remaining;
                }
                _ => return None,
            }
        }
        Some(current)
    }
}
mod boundary_domains {
    /*
        Boundary membership admission has two value lanes, not two proof rules.
        Classify the exact subject before choosing its representation. Scalar
        predicates and nominal identity remain independent; neither provider choice
        nor projection of the numeric component can establish routed membership.
    */
    use super::signature_contracts::{
        normal_mutable_parameter_position, signature_progress_requirement,
    };
    use super::{
        CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralParameterPlan,
        ShapeCollector,
    };
    use checked_trees::{
        CheckedBooleanExpression, CheckedBoundaryScalarDomainRequirementPlan,
        CheckedStructuralScalarParameterPlan,
    };
    use symbols::SymbolHandle;
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

    pub(in crate::execution::terminal_unit) struct BoundaryDomainRequirements {
        pub structural: Vec<CheckedUnitStructuralDomainRequirementPlan>,
        pub normal: Vec<CheckedUnitStructuralDomainRequirementPlan>,
        pub scalar: Vec<CheckedBoundaryScalarDomainRequirementPlan>,
    }

    pub(in crate::execution::terminal_unit) fn boundary_domain_requirements<'a>(
        program: &dyn validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        source_parameters: &[StateParameter],
        contracts: impl Iterator<Item = &'a SignatureContract>,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        scalar_parameters: &[CheckedStructuralScalarParameterPlan],
        binders: &[(SymbolHandle, String)],
    ) -> Option<BoundaryDomainRequirements> {
        let mut structural = Vec::new();
        let mut scalar = Vec::new();
        let mut normal = Vec::new();
        for (argument_index, parameter) in structural_parameters.iter().enumerate() {
            for domain in &parameter.qualifications {
                structural.push(CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: u32::try_from(argument_index).ok()?,
                    path: Vec::new(),
                    domain: *domain,
                });
            }
            let source = source_parameters.get(parameter.position as usize)?;
            if parameter.access == checked_trees::CheckedStructuralAccess::MutableBorrow {
                let declared =
                    validation::structural_qualification_contract(program, source.type_reference)
                        .ok()?;
                for reference in declared.projection_types {
                    shapes.add_type(reference, binders, &[])?;
                }
                for (domain, carrier) in declared.domains {
                    shapes.add_domain(domain, carrier, binders)?;
                }
                for row in declared.projected_qualifications {
                    normal.push(CheckedUnitStructuralDomainRequirementPlan {
                        argument_index: u32::try_from(argument_index).ok()?,
                        path: row.path,
                        domain: row.domain,
                    });
                }
            }
        }
        for contract in contracts.filter(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            )
        }) {
            let entry = contract.kind == SignatureContractKind::Requires;
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if entry
                    && let Some(valid) =
                        signature_progress_requirement(program, source_parameters, membership)
                {
                    if !valid {
                        return None;
                    }
                    continue;
                }
                let source_position =
                    validation::single_segment_name_symbol(program, membership.value).and_then(
                        |root| {
                            source_parameters
                                .iter()
                                .position(|parameter| parameter.symbol == root)
                        },
                    );
                if !entry
                    && normal_mutable_parameter_position(
                        program,
                        source_parameters,
                        membership.value,
                    )
                    .is_none()
                {
                    continue;
                }
                let primitive_type =
                    validation::scalar_membership_value_carrier(program, membership);
                if primitive_type.is_some()
                    && !validation::field_domains::domain_requires_provenance(
                        program,
                        membership.domain_symbol,
                    )
                    && !validation::field_domains::domain_is_vacuous(
                        program,
                        membership.domain_symbol,
                        &mut Vec::new(),
                    )
                {
                    continue;
                }
                if let Some(argument_index) = source_position.and_then(|source_position| {
                    structural_parameters
                        .iter()
                        .position(|parameter| parameter.position as usize == source_position)
                }) {
                    let domain = membership.semantic_domain;
                    if !domain.is_valid() {
                        return None;
                    }
                    let carrier = program.unwrapped_type_reference(
                        source_parameters[source_position?].type_reference,
                    )?;
                    shapes.add_selected_domain(
                        domain,
                        membership.domain_symbol,
                        carrier,
                        binders,
                        false,
                    )?;
                    let row = CheckedUnitStructuralDomainRequirementPlan {
                        argument_index: u32::try_from(argument_index).ok()?,
                        path: Vec::new(),
                        domain,
                    };
                    if entry {
                        structural.push(row);
                    } else {
                        normal.push(row);
                    }
                    continue;
                }
                if !entry {
                    continue;
                }
                if let Some(primitive_type) = primitive_type {
                    let source_position = source_position?;
                    let argument_index = scalar_parameters.iter().position(|parameter| {
                        parameter.source_position as usize == source_position
                    })?;
                    if scalar_parameters[argument_index].scalar_type != primitive_type
                        || !membership.semantic_domain.is_valid()
                    {
                        return None;
                    }
                    let subject = checked_trees::CheckedScalarExpression::Parameter {
                        position: 0,
                        scalar_type: primitive_type.into(),
                    };
                    let predicate = validation::scalar_membership_value_predicates(
                        program, membership, &subject,
                    )?
                    .into_iter()
                    .reduce(|left, right| CheckedBooleanExpression::And {
                        left: Box::new(left),
                        right: Box::new(right),
                    })
                    .unwrap_or(CheckedBooleanExpression::Constant(true));
                    scalar.push(CheckedBoundaryScalarDomainRequirementPlan {
                        argument_index: u32::try_from(argument_index).ok()?,
                        domain_symbol: membership.domain_symbol,
                        domain: membership.semantic_domain,
                        primitive_type,
                        predicate,
                    });
                    continue;
                }
                return None;
            }
        }
        structural.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
        structural.dedup();
        scalar.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
        if scalar.windows(2).any(|pair| {
            pair[0].argument_index == pair[1].argument_index
                && pair[0].domain == pair[1].domain
                && pair[0] != pair[1]
        }) {
            return None;
        }
        scalar.dedup();
        normal.sort_by_key(|row| (row.argument_index, row.domain.0));
        normal.dedup();
        Some(BoundaryDomainRequirements {
            structural,
            scalar,
            normal,
        })
    }
}
mod identities {

    /*
        Type identities, structural access, and the type-shape predicates the Unit
        plans key on.
    */
    use super::*;

    pub(in crate::execution::terminal_unit) fn terminal_field_identity(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<checked_trees::CheckedMemberIdentity> {
        terminal_field_declaration(program, symbol).map(|field| {
            checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone())
        })
    }

    /*
        Common and payload fields use the same declaration identity. The path's
        Case segment, not a spelling search, supplies the payload namespace.
    */
    pub(crate) fn terminal_field_declaration(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::data::DataField> {
        if !symbol.is_valid() {
            return None;
        }
        let positions = program.data_field_positions_for_symbol(symbol);
        let &[(data_position, member_position, payload_position)] = positions.as_slice() else {
            return None;
        };
        let definition = program.data_definitions().get(data_position as usize)?;
        let member = program
            .data_members(definition)
            .get(member_position as usize)?;
        if payload_position == u32::MAX {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            Some(field)
        } else {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            program
                .data_payload_fields(variant)
                .get(payload_position as usize)
        }
    }

    pub(crate) fn is_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        validation::reference_shell(program, type_reference).is_some()
    }

    pub(in crate::execution) fn structural_access_for_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<CheckedStructuralAccess> {
        let Some(shell) = validation::reference_shell(program, type_reference) else {
            return Some(CheckedStructuralAccess::Owned);
        };
        Some(shell.access.into())
    }

    /*
        The identity a byte carrier's structural plan registers under, or `None`
        when `type_reference` is not a byte carrier. The identity rule itself —
        which shells a view, a bounded view and a bounded owner keep — belongs to
        `validation::structural_type_identity`.
    */
    pub(crate) fn byte_sequence_type_identity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        byte_sequence_carrier(program, type_reference, substitutions)?;
        Some(validation::structural_type_identity(
            program,
            type_reference,
            binders,
            substitutions,
        ))
    }

    pub(in crate::execution) use validation::type_graph_requires_nominal_drop;

    /*
        Join stored reference identity to its borrowed presentation without
        erasing the source declaration. Access and custody remain the edge's checks.
    */
    pub(in crate::execution::terminal_unit) fn stored_type_presents_borrowed(
        program: &dyn ::validation::CheckingInput,
        source: TypeReferenceHandle,
        stored: typed_trees::type_identity::TypeIdentityId,
        borrowed: typed_trees::type_identity::TypeIdentityId,
    ) -> bool {
        validation::stored_structural_type_identity(program, source, &[], &[]) == stored
            && base_type_identity(program, source, &[], &[]) == Some(borrowed)
    }

    pub(crate) fn is_unit(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> bool {
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Unit => return true,
                _ => return false,
            }
        }
    }

    /*
        The structural plan identity a call operand, formal or result of
        `type_reference` joins, offered only when the storage beneath its loan and
        qualification shells is a nominal, a slice or a literal-extent array: the
        carriers a call presents. Other storage declines. The identity itself is
        the rule `validation::structural_type_identity` owns, so a bounded byte
        owner keeps its qualification here exactly as its registered plan does.
    */

    /*
        The same identity under a call edge's checked generic substitutions: the
        callee's `Type` parameters resolve to the actuals its admitted
        specialization derived, so a requirement formal's identity compares
        against the caller's concrete presentation.
    */
    pub(in crate::execution::terminal_unit) fn base_type_identity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let storage = program
            .type_reference_table
            .peeled_type_reference(type_reference);
        matches!(
            program.type_reference_table.type_reference(storage),
            TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Generic { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(_),
                    ..
                }
        )
        .then(|| {
            validation::structural_type_identity(program, type_reference, binders, substitutions)
        })
    }

    /*
        The structural identity of the storage a type lends: a loan's referent, or
        the type itself when it is no loan. Two types lend the same storage when
        these agree, whichever shell each spells — a `&mut [u8; N]` alias and the
        `[u8; N]` it borrows, or a plain array and the `&[u8; N]` bounded view a
        formal declares over it — while a qualified byte owner still lends
        different storage from the plain array.
    */
    pub(in crate::execution::terminal_unit) fn lent_storage_identity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let storage = validation::reference_shell(program, type_reference)
            .map_or(type_reference, |shell| shell.referee);
        base_type_identity(program, storage, &[], substitutions).map(|_| {
            validation::stored_structural_type_identity(program, storage, &[], substitutions)
        })
    }

    /*
        The last checked substitution binding `parameter`, if the call edge
        supplied one.
    */
    pub(in crate::execution::terminal_unit) fn substitution_for(
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        parameter: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        substitutions
            .iter()
            .rev()
            .find(|(candidate, _)| *candidate == parameter)
            .map(|(_, actual)| *actual)
    }

    /*
        A call edge's checked specialization maps a bare `Type` formal to the
        actual its admission derived before custody or shape evidence is
        replayed against the caller's concrete values. A compound formal keeps
        its own reference: the substitution only names a top-level `Type`
        parameter, and identity-level substitution covers the generic arguments
        inside it.

        This is the one owner of that walk — the last binding wins, and a binding
        to the reference itself stops it — so a caller that must keep resolving
        past its own shells calls this instead of spelling the loop again.
    */
    pub(in crate::execution::terminal_unit) fn substituted_formal_type(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> TypeReferenceHandle {
        loop {
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return type_reference;
            };
            let Some(actual) = substitution_for(substitutions, *symbol) else {
                return type_reference;
            };
            if actual == type_reference {
                return type_reference;
            }
            type_reference = actual;
        }
    }

    pub(in crate::execution::terminal_unit) fn closed_data_identity(
        program: &dyn ::validation::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> typed_trees::type_identity::TypeIdentityId {
        if let Some(application) = data.generic_instance {
            return program.type_identity(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(application)
            });
        }
        program
            .normalized_nominal_type_identity(data.symbol)
            .unwrap_or_default()
    }

    pub(in crate::execution::terminal_unit) fn scalar_type(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<checked_trees::CheckedScalarType> {
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { name, .. } => {
                    let resolved = substituted_formal_type(program, type_reference, substitutions);
                    if resolved == type_reference {
                        return PrimitiveType::from_name(name.as_str()).map(Into::into);
                    }
                    type_reference = resolved;
                }
                TypeReferenceNode::DynamicTrait { .. } => {
                    return Some(PrimitiveType::Selector.into());
                }
                TypeReferenceNode::Callable { .. } => {
                    return checked_trees::CheckedScalarType::from_type_reference(
                        program.typed_program(),
                        type_reference,
                    );
                }
                _ => return None,
            }
        }
    }
}
mod layout_observations {
    /*
        A layout query retains its subject even when no runtime value has that type.
        Walk owned source references rather than the arena: removed declarations do
        not demand geometry, and a shared expression keeps its supplying telescope.
    */
    use super::ShapeCollector;
    use std::collections::{BTreeSet, HashSet};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::references::TypedReferences;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::TypeReferenceHandle;

    pub(in crate::execution::terminal_unit) fn retain_layout_observation_types(
        program: &dyn validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        retained: &mut BTreeSet<TypeIdentityId>,
    ) -> Vec<checked_trees::CheckedLayoutObservation> {
        let mut queries = QuerySubjects {
            program,
            owner: SymbolHandle::default(),
            outer_owners: Vec::new(),
            signatures: Vec::new(),
            expressions: HashSet::new(),
            types: HashSet::new(),
            facts: HashSet::new(),
            mathematical_types: HashSet::new(),
            subjects: Vec::new(),
        };
        typed_trees::references::visit_program(program.typed_program(), &mut queries);
        let mut observations = Vec::new();
        for subject in queries.subjects {
            if let Some(identity) = shapes.add_stored_type(subject.reference, &subject.binders, &[])
            {
                retained.insert(identity);
                observations.push(checked_trees::CheckedLayoutObservation {
                    owner: subject.owner,
                    expression: subject.expression,
                    subject: identity,
                    measurement: subject.measurement,
                });
            }
        }
        observations
    }

    struct QuerySubject {
        owner: SymbolHandle,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        binders: Vec<(SymbolHandle, String)>,
        measurement: language_core::data_layout_queries::DataLayoutQueryKind,
    }

    struct QuerySubjects<'a> {
        program: &'a dyn validation::CheckingInput,
        owner: SymbolHandle,
        outer_owners: Vec<SymbolHandle>,
        signatures: Vec<(SymbolHandle, Vec<(SymbolHandle, String)>)>,
        expressions: HashSet<(SymbolHandle, ExpressionHandle)>,
        types: HashSet<(SymbolHandle, TypeReferenceHandle)>,
        facts: HashSet<(SymbolHandle, arena::Handle<typed_trees::domain::ProofFact>)>,
        mathematical_types: HashSet<(
            SymbolHandle,
            typed_trees::mathematical::MathematicalTypeHandle,
        )>,
        subjects: Vec<QuerySubject>,
    }

    impl Extend<SymbolHandle> for QuerySubjects<'_> {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }

    impl TypedReferences for QuerySubjects<'_> {
        fn declaration(&mut self, owner: SymbolHandle) -> bool {
            self.owner = owner;
            true
        }

        fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
            self.outer_owners.push(self.owner);
            if owner.is_valid() {
                self.owner = owner;
            }
        }

        fn leave_lexical_scope(&mut self) {
            if let Some(owner) = self.outer_owners.pop() {
                self.owner = owner;
            }
        }

        fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
            self.signatures.push((
                signature.symbol,
                validation::contract_structure::signature_generic_binders(self.program, signature),
            ));
        }

        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            if !self.expressions.insert((self.owner, expression)) {
                return false;
            }
            if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
                && let Some(query) = &call.layout_query
            {
                let binders = if let Some((_, binders)) = self
                    .signatures
                    .iter()
                    .rev()
                    .find(|(owner, _)| *owner == self.owner)
                {
                    binders.clone()
                } else if let Some(scope) = validation::StaticTypeArgumentOwner::from_lexical_owner(
                    self.program,
                    self.owner,
                ) {
                    let (parameters, _, _) = scope.parameters(self.program);
                    parameters
                        .iter()
                        .enumerate()
                        .map(|(position, parameter)| (parameter.symbol, format!("$T{position}")))
                        .collect()
                } else {
                    return true;
                };
                self.subjects.push(QuerySubject {
                    owner: self.owner,
                    expression,
                    reference: query.operand.type_reference,
                    binders,
                    measurement: query.kind,
                });
            }
            true
        }

        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            self.types.insert((self.owner, reference))
        }

        fn proof_fact(&mut self, fact: arena::Handle<typed_trees::domain::ProofFact>) -> bool {
            self.facts.insert((self.owner, fact))
        }

        fn mathematical_type(
            &mut self,
            reference: typed_trees::mathematical::MathematicalTypeHandle,
        ) -> bool {
            self.mathematical_types.insert((self.owner, reference))
        }
    }
}
pub(super) use layout_observations::retain_layout_observation_types;
mod parameters {
    /*
        The authored parameter roster as the structural plans see it: roots, binders,
        ABI positions, and the erased scalar and proof formals that own no slot.
    */
    use super::*;
    use checked_trees::CheckedErasedProofParameterPlan;
    use checked_trees::CheckedStructuralScalarParameterPlan;
    use checked_trees::CheckedUnitParameterStoragePlan;
    use checked_trees::FlowStateFact;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use validation::mathematical_applications::is_source_evidence_carrier;
    use validation::proof_only_classification;

    pub(in crate::execution::terminal_unit) fn state_flow(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<&FlowStateFact> {
        facts.flow.control.state(machine, state)
    }

    /*
        Which authored parameter an argument or contract place roots at. A
        parameter owns its place root outright, so the question is its own symbol
        and nothing else.

        Before static machines this was `parameter_root_symbol`, which answered the
        attached machine's symbol for a `self` receiver, because an implicit
        receiver place rooted at the machine rather than at any parameter. Every
        caller therefore spelled the question as a disjunction over both roots. A
        qualified machine name now contributes no hidden receiver and
        `StateParameter` carries no receiver flag at all, so the machine-rooted
        alternative is not representable and the disjunction collapses to this one
        comparison. Keep the question here rather than re-deriving it at each
        place-rooting site, so a later root spelling has one place to grow.
    */
    pub(crate) fn parameter_is_root(parameter: &StateParameter, root: SymbolHandle) -> bool {
        parameter.symbol == root
    }

    pub(in crate::execution::terminal_unit) fn machine_binders(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
    ) -> Vec<(SymbolHandle, String)> {
        program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
            .collect()
    }

    /*
        Whether the calling plan strips this parameter as an `[erased]` binding
        occurrence (contracts.md#explicit-erased-bindings): it receives no
        structural or scalar plan entry and no ABI position, while the retained
        parameters keep their authored `position`/`source_position` indices. Only
        a plain immutable value binding is strippable. `const` and
        mutable bindings carry runtime custody or entry storage that erasure does
        not define away, so an erased one refuses the plan (`None`) instead of
        silently vanishing or silently keeping a slot.
    */
    pub(crate) fn strips_erased_parameter(parameter: &StateParameter) -> Option<bool> {
        if !parameter.relevance.is_erased() {
            return Some(false);
        }
        (!parameter.is_const && !parameter.is_mutable).then_some(true)
    }

    /*
        Storage for each `mut` scalar formal among `scalar_parameters`. Checking
        reads such a formal as `StorageRead` of its symbol, never as the incoming
        value, so the body must own initialized storage for it from entry. The
        storage takes the formal's declared type, as a `let mut` local's does; a
        carrier checking gives no current-value storage refuses the plan.
    */
    pub(in crate::execution::terminal_unit) fn parameter_storage_plans(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &State,
        binders: &[(SymbolHandle, String)],
        scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    ) -> Option<Vec<CheckedUnitParameterStoragePlan>> {
        let parameters = program.state_parameters(state);
        scalar_parameters
            .iter()
            .filter_map(|scalar| {
                let parameter = parameters.get(usize::try_from(scalar.source_position).ok()?);
                parameter
                    .filter(|parameter| parameter.is_mutable)
                    .map(|parameter| (scalar, parameter))
            })
            .map(|(scalar, parameter)| {
                let primitive_type =
                    crate::values::mutable_scalar_parameter_type(program, parameter)?;
                if primitive_type != scalar.scalar_type
                    || !super::supports_primitive_storage(program, parameter.type_reference)
                {
                    return None;
                }
                Some(CheckedUnitParameterStoragePlan {
                    source_position: scalar.source_position,
                    symbol: parameter.symbol,
                    type_identity: shapes.add_type(parameter.type_reference, binders, &[])?,
                    primitive_type,
                })
            })
            .collect()
    }

    /*
        The authored `[erased]` scalar formals in authored order, each retaining
        its authored parameter position. Contract-term erased formals belong to
        the proof lane (`erased_proof_parameter_plans`), so this plan skips
        them. Erased formals with any other non-primitive type refuse the plan.
    */
    pub(crate) fn erased_scalar_parameter_plans(
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> Option<Vec<CheckedStructuralScalarParameterPlan>> {
        erased_scalar_parameter_plans_for_parameters(program, program.state_parameters(state))
    }

    pub(crate) fn erased_scalar_parameter_plans_for_parameters(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
    ) -> Option<Vec<CheckedStructuralScalarParameterPlan>> {
        if !parameters
            .iter()
            .any(|parameter| parameter.relevance.is_erased())
        {
            return Some(Vec::new());
        }
        let proof_only = proof_only_classification(program);
        parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.relevance.is_erased())
            .filter(|(_, parameter)| {
                !(proof_only.contract_term_carrier(program, parameter.type_reference)
                    || is_source_evidence_carrier(program, parameter.type_reference))
            })
            .map(|(position, parameter)| {
                // One plan per erased scalar lane, in lane order: a record
                // formal repeats its source position once for each leaf.
                let lanes = program.erased_scalar_lanes(parameter.type_reference);
                if lanes.is_empty() {
                    return None;
                }
                let source_position = u32::try_from(position).ok()?;
                Some(
                    lanes
                        .into_iter()
                        .map(|lane| CheckedStructuralScalarParameterPlan {
                            source_position,
                            scalar_type: lane.primitive_type.into(),
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Option<Vec<_>>>()
            .map(|plans| plans.into_iter().flatten().collect())
    }

    /*
        The authored `[erased]` contract-term formals in authored order, each
        retaining its authored parameter position and canonical semantic type
        identity (`Nat`, `Proof`). These carriers admit no scalar lane; the
        contract term lane carries them by identity instead.
    */
    pub(crate) fn erased_proof_parameter_plans(
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> Option<Vec<CheckedErasedProofParameterPlan>> {
        erased_proof_parameter_plans_for_parameters(program, program.state_parameters(state))
    }

    pub(crate) fn erased_proof_parameter_plans_for_parameters(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
    ) -> Option<Vec<CheckedErasedProofParameterPlan>> {
        if !parameters
            .iter()
            .any(|parameter| parameter.relevance.is_erased())
        {
            return Some(Vec::new());
        }
        let proof_only = proof_only_classification(program);
        parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.relevance.is_erased())
            .filter(|(_, parameter)| {
                proof_only.contract_term_carrier(program, parameter.type_reference)
                    || is_source_evidence_carrier(program, parameter.type_reference)
            })
            .enumerate()
            .map(|(lane, (position, parameter))| {
                Some(CheckedErasedProofParameterPlan {
                    source_position: u32::try_from(position).ok()?,
                    proof_position: u32::try_from(lane).ok()?,
                    parameter_symbol: parameter.symbol,
                    type_identity: if is_source_evidence_carrier(program, parameter.type_reference)
                    {
                        program.normalized_type_identity(parameter.type_reference)
                    } else {
                        base_type_identity(program, parameter.type_reference, &[], &[])?
                    },
                })
            })
            .collect()
    }

    /*
        Resolve the authored slot before the final structural roster is available.
        Callers with a completed roster must use that roster instead.
    */
    pub(crate) fn dense_structural_parameter_index(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        authored_position: usize,
    ) -> Option<u32> {
        parameters.get(authored_position)?;
        u32::try_from(
            parameters
                .iter()
                .take(authored_position)
                .filter(|parameter| {
                    validation::reference_result_custody::is_mutable_structural_parameter(
                        program, parameter,
                    )
                })
                .count(),
        )
        .ok()
    }

    /*
        The dense scalar-namespace index of the authored parameter at
        `authored_position`: the number of earlier formals that occupy the scalar
        namespace (`values::scalar::occupies_scalar_position`), the same partition
        the signature builders record in each
        `CheckedStructuralScalarParameterPlan`'s `source_position`.
    */
    pub(crate) fn dense_scalar_parameter_index(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        authored_position: usize,
    ) -> Option<u32> {
        parameters.get(authored_position)?;
        u32::try_from(
            parameters
                .iter()
                .take(authored_position)
                .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                .count(),
        )
        .ok()
    }
}
mod shape_collector {
    /*
        The catalogs a Unit plan family publishes, collected from the types its
        signatures, locals, results and cleanup control name. Cleanup conditions
        retain their own arenas beside the type catalog; they never alter a shape's
        declaration or grant custody.

        Collection describes storage and retains checked type capabilities, not
        occurrence authority to copy, construct, or discard it. Fields and array
        elements collect the same stored value shapes,
        including provider receipts and stored reference shells. Their multiplicity
        and cleanup remain obligations of the operations that use that storage.
        Plain finite source contents additionally retain their no-code disposal
        capability. A selected move may remove a common exit disposition while the
        skipped alternative still owns the value; the graph's existing custody
        solver needs this independent authority to settle that live complement.
        Nominal hooks and linear descendants never gain this capability. Copyable
        shared-carrier aggregates can retire their own descriptors without owning
        an affine source claim or their external referents.
        Descriptors have a separate exact-type disposal judgment. A replacement
        must settle their resident cohort and preserve external loans before installing
        the incoming roster; this bit never permits ordinary resource-free cleanup.

        A stored reference owns its pointer carrier, not its referent's storage.
        Record its exact type edge without collecting the target's fields. Root
        storage demands collect their own shapes; otherwise `Nest<T>` containing
        `&Nest<[T; 2]>` would require an infinite catalog for one finite pointer.
    */
    use super::*;
    mod forks {

        /*
            Planning checked bodies on several cores without changing what they plan.

            Checked bodies are planned against one collector in roster order, and a
            body can see what earlier bodies registered: a calling parameter gains a
            callback backing only when its identity is already catalogued, and two
            presentations of one identity must agree. So a body plans against a fork:
            a copy of the collector as it stood after some prefix of the roster (the
            fork's base), which remembers every row it consulted and wrote. The merge
            walks the roster in order and keeps a log of the position at which each row
            last changed. A fork that consulted no row changed at or after its base saw
            exactly the catalog the sequential pass would have shown it, so its plan
            and its writes are the sequential ones and are replayed as they are. A
            stale fork plans again from the merged collector; see `admits`.

            Cleanup-control arenas hand out spans that the plan stores, so a fork that
            appended to them is replayed only when no other body appended since its
            base. The completed-request memo is a query cache: a fork that misses it
            recomputes the same admitted shape, and its entries are replayed.

            A fork does not copy the type catalog or the cleanup arenas. The type
            rows and recursion markers keep a shared base and a fork writes into its
            own overlay; the cleanup arenas are shared until a fork first appends.
            The merged collector holds the only reference to its bases between fork
            rounds, so it writes in place and never pays for an overlay.
        */
        use super::*;
        use std::sync::{Arc, Mutex, PoisonError};

        /*
            A keyed catalog that remembers which keys an attempt consulted, through
            shared and exclusive access alike, and the rows it wrote, with each
            row's value before its first write.

            Every body plans against its own fork, so a fork must be cheap. The rows
            live in a shared base; a collector that holds the only reference to its
            base (the merged collector) writes into it directly, and a fork, whose base
            its siblings share, writes into its own overlay instead. A key the overlay
            holds shadows the base: `Some` is the row now, `None` a removed row.
        */
        pub(crate) struct ConsultedMap<K, V> {
            base: Arc<BTreeMap<K, V>>,
            overlay: BTreeMap<K, Option<V>>,
            consulted: Mutex<Consulted<K>>,
            written: BTreeMap<K, Option<V>>,
        }

        /* The keys an attempt looked up, and whether it read the whole catalog. */
        struct Consulted<K> {
            keys: BTreeSet<K>,
            every_key: bool,
        }

        impl<K> Default for Consulted<K> {
            fn default() -> Self {
                Self {
                    keys: BTreeSet::new(),
                    every_key: false,
                }
            }
        }

        impl<K: Ord + Clone, V: Clone + PartialEq> ConsultedMap<K, V> {
            fn consult(&self, key: &K) {
                self.consulted
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .keys
                    .insert(key.clone());
            }

            /* The row as it stands now, overlay first. */
            fn current(&self, key: &K) -> Option<&V> {
                match self.overlay.get(key) {
                    Some(row) => row.as_ref(),
                    None => self.base.get(key),
                }
            }

            /*
                The base to write through, when no sibling shares it and no overlay
                row could be shadowed by the write.
            */
            fn exclusive_base(&mut self) -> Option<&mut BTreeMap<K, V>> {
                if self.overlay.is_empty() {
                    Arc::get_mut(&mut self.base)
                } else {
                    None
                }
            }

            fn write(&mut self, key: &K) {
                self.consulted
                    .get_mut()
                    .unwrap_or_else(PoisonError::into_inner)
                    .keys
                    .insert(key.clone());
                if !self.written.contains_key(key) {
                    let before = self.current(key).cloned();
                    self.written.insert(key.clone(), before);
                }
            }

            pub(crate) fn get(&self, key: &K) -> Option<&V> {
                self.consult(key);
                self.current(key)
            }

            pub(crate) fn contains_key(&self, key: &K) -> bool {
                self.consult(key);
                self.current(key).is_some()
            }

            pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
                self.write(key);
                if self.exclusive_base().is_some() {
                    return self.exclusive_base()?.get_mut(key);
                }
                if !self.overlay.contains_key(key) {
                    let row = self.base.get(key).cloned();
                    self.overlay.insert(key.clone(), row);
                }
                self.overlay.get_mut(key)?.as_mut()
            }

            pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
                self.write(&key);
                if let Some(base) = self.exclusive_base() {
                    return base.insert(key, value);
                }
                let shadowed = self.base.get(&key).cloned();
                self.overlay.insert(key, Some(value)).unwrap_or(shadowed)
            }

            fn remove(&mut self, key: &K) -> Option<V> {
                self.write(key);
                if let Some(base) = self.exclusive_base() {
                    return base.remove(key);
                }
                let shadowed = self.base.get(key).cloned();
                self.overlay.insert(key.clone(), None).unwrap_or(shadowed)
            }

            /*
                Reading every row consults the whole catalog, including rows another
                attempt may have added. Rows come in key order, overlay shadowing base.
            */
            pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
                self.consulted
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .every_key = true;
                let mut base = self.base.iter().peekable();
                let mut overlay = self.overlay.iter().peekable();
                std::iter::from_fn(move || {
                    loop {
                        let from_overlay = match (base.peek(), overlay.peek()) {
                            (None, None) => return None,
                            (Some(_), None) => false,
                            (None, Some(_)) => true,
                            (Some((base_key, _)), Some((overlay_key, _))) => {
                                match overlay_key.cmp(base_key) {
                                    std::cmp::Ordering::Less => true,
                                    std::cmp::Ordering::Equal => {
                                        base.next();
                                        true
                                    }
                                    std::cmp::Ordering::Greater => false,
                                }
                            }
                        };
                        if !from_overlay {
                            return base.next();
                        }
                        if let (key, Some(row)) = overlay.next()? {
                            return Some((key, row));
                        }
                    }
                })
            }

            /* Fold the overlay into the base, which this collector then owns. */
            fn settle(&mut self) -> &mut BTreeMap<K, V> {
                let overlay = std::mem::take(&mut self.overlay);
                let base = Arc::make_mut(&mut self.base);
                for (key, row) in overlay {
                    match row {
                        Some(row) => base.insert(key, row),
                        None => base.remove(&key),
                    };
                }
                base
            }

            pub(crate) fn retain(&mut self, keep: impl FnMut(&K, &mut V) -> bool) {
                self.settle().retain(keep);
            }

            pub(crate) fn into_values(mut self) -> impl Iterator<Item = V> {
                self.settle();
                Arc::unwrap_or_clone(self.base).into_values()
            }

            /*
                What this attempt consulted, and the rows whose value it changed, as
                they stand now.
            */
            fn forget(&mut self) -> (Consulted<K>, Vec<(K, Option<V>)>) {
                let written = std::mem::take(&mut self.written)
                    .into_iter()
                    .filter(|(key, before)| self.current(key) != before.as_ref())
                    .map(|(key, _)| {
                        let now = self.current(&key).cloned();
                        (key, now)
                    })
                    .collect();
                (
                    std::mem::take(
                        self.consulted
                            .get_mut()
                            .unwrap_or_else(PoisonError::into_inner),
                    ),
                    written,
                )
            }

            /* Set a row as a replayed attempt left it, `None` for no row. */
            fn replay(&mut self, key: K, row: Option<V>) {
                match row {
                    Some(row) => {
                        if let Some(base) = self.exclusive_base() {
                            base.insert(key, row);
                        } else {
                            self.overlay.insert(key, Some(row));
                        }
                    }
                    None => {
                        if let Some(base) = self.exclusive_base() {
                            base.remove(&key);
                        } else {
                            self.overlay.insert(key, None);
                        }
                    }
                }
            }
        }

        impl<K: Ord, V> Default for ConsultedMap<K, V> {
            fn default() -> Self {
                Self {
                    base: Arc::default(),
                    overlay: BTreeMap::new(),
                    consulted: Mutex::default(),
                    written: BTreeMap::new(),
                }
            }
        }

        /* A copy has consulted and written nothing yet, and shares the base. */
        impl<K: Ord + Clone, V: Clone> Clone for ConsultedMap<K, V> {
            fn clone(&self) -> Self {
                Self {
                    base: Arc::clone(&self.base),
                    overlay: self.overlay.clone(),
                    consulted: Mutex::default(),
                    written: BTreeMap::new(),
                }
            }
        }

        /* A recursion-marker set that remembers which members an attempt consulted. */
        pub(crate) struct ConsultedSet<K>(ConsultedMap<K, ()>);

        impl<K: Ord + Clone> Clone for ConsultedSet<K> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl<K: Ord + Clone> ConsultedSet<K> {
            pub(crate) fn new() -> Self {
                Self(ConsultedMap::default())
            }

            pub(crate) fn contains(&self, key: &K) -> bool {
                self.0.contains_key(key)
            }

            pub(crate) fn insert(&mut self, key: K) -> bool {
                self.0.insert(key, ()).is_none()
            }

            pub(crate) fn remove(&mut self, key: &K) -> bool {
                self.0.remove(key).is_some()
            }
        }

        /* Where each append-only catalog ended before an attempt. */
        #[derive(Clone, Copy)]
        pub(crate) struct RowMarks {
            domains: usize,
            opaque_representations: usize,
            cleanup_rows: usize,
        }

        /*
            One attempt's consultations, and the rows it changed as they stood when it
            finished. A replayable fork also carries what replaying it needs.
        */
        pub(crate) struct ShapeEdits {
            consulted_types: BTreeSet<TypeIdentityId>,
            consulted_every_type: bool,
            consulted_domains: BTreeSet<(u32, TypeIdentityId)>,
            types: Vec<(TypeIdentityId, Option<CheckedUnitStructuralTypePlan>)>,
            in_progress: Vec<(TypeIdentityId, bool)>,
            domains_in_progress: Vec<((u32, TypeIdentityId), bool)>,
            domains: Vec<CheckedUnitStructuralDomainPlan>,
            opaque_representations: Vec<checked_trees::CheckedOpaqueRepresentation>,
            completed_requests: Vec<(usize, TypeShapeRequest, TypeIdentityId)>,
            byte_view_elements: Vec<TypeIdentityId>,
            unselected_opaque: Option<SymbolHandle>,
            cleanup_control: Option<checked_trees::CheckedCleanupControlPlans>,
        }

        /*
            The completed-request memo, shared between a collector and its forks.

            The memo has one slot per type-reference arena row, so it grows with the
            program. Each body plans on a fork, and copying the whole memo into every
            fork made planning quadratic in the number of machines. Forks now share
            the memo and keep only what they complete themselves in `local`, which is
            also exactly what replaying the fork needs. A collector that holds the
            only reference extends the shared rows in place.
        */
        #[derive(Clone, Default)]
        pub(crate) struct CompletedRequests {
            shared: std::sync::Arc<Vec<Vec<(TypeShapeRequest, TypeIdentityId)>>>,
            /*
                Requests completed while `shared` was shared, in completion order.
                Every one of them follows every shared request of its slot.
            */
            local: Vec<(usize, TypeShapeRequest, TypeIdentityId)>,
        }

        impl CompletedRequests {
            /* The first completed request in `slot` that `matches`, in completion order. */
            pub(super) fn find(
                &self,
                slot: usize,
                mut matches: impl FnMut(&TypeShapeRequest) -> bool,
            ) -> Option<TypeIdentityId> {
                let shared = self.shared.get(slot).into_iter().flatten();
                let local = self
                    .local
                    .iter()
                    .filter(|(owner, _, _)| *owner == slot)
                    .map(|(_, request, identity)| (request, identity));
                shared
                    .map(|(request, identity)| (request, identity))
                    .chain(local)
                    .find_map(|(request, identity)| matches(request).then_some(*identity))
            }

            pub(super) fn push(
                &mut self,
                slot: usize,
                request: TypeShapeRequest,
                identity: TypeIdentityId,
            ) {
                if self.local.is_empty()
                    && let Some(rows) = std::sync::Arc::get_mut(&mut self.shared)
                {
                    if rows.len() <= slot {
                        rows.resize_with(slot + 1, Vec::new);
                    }
                    rows[slot].push((request, identity));
                } else {
                    self.local.push((slot, request, identity));
                }
            }

            fn slot_len(&self, slot: usize) -> usize {
                self.shared.get(slot).map_or(0, Vec::len)
                    + self
                        .local
                        .iter()
                        .filter(|(owner, _, _)| *owner == slot)
                        .count()
            }

            /* Every request completed here beyond what `base` holds in each slot. */
            fn since(&self, base: &Self) -> Vec<(usize, TypeShapeRequest, TypeIdentityId)> {
                if std::sync::Arc::ptr_eq(&self.shared, &base.shared)
                    && self.local.len() >= base.local.len()
                {
                    return self.local[base.local.len()..].to_vec();
                }
                let slots = self.shared.len().max(
                    self.local
                        .iter()
                        .map(|(slot, _, _)| slot + 1)
                        .max()
                        .unwrap_or(0),
                );
                let mut since = Vec::new();
                for slot in 0..slots {
                    let known = base.slot_len(slot);
                    let shared = self.shared.get(slot).into_iter().flatten();
                    let local = self
                        .local
                        .iter()
                        .filter(|(owner, _, _)| *owner == slot)
                        .map(|(_, request, identity)| (request, identity));
                    since.extend(
                        shared
                            .map(|(request, identity)| (request, identity))
                            .chain(local)
                            .skip(known)
                            .map(|(request, identity)| (slot, request.clone(), *identity)),
                    );
                }
                since
            }
        }

        /* The roster position at which each collector row last changed. */
        #[derive(Default)]
        pub(crate) struct ChangeLog {
            types: BTreeMap<TypeIdentityId, usize>,
            domains: BTreeMap<(u32, TypeIdentityId), usize>,
            last_type_change: Option<usize>,
            last_cleanup_change: Option<usize>,
        }

        impl ChangeLog {
            /*
                A fork based after the first `base` bodies is the sequential run of
                its body when nothing it consulted changed at a later position.
            */
            pub(crate) fn admits(&self, edits: &ShapeEdits, base: usize) -> bool {
                let before_base =
                    |position: Option<&usize>| position.is_none_or(|changed| *changed < base);
                (edits.cleanup_control.is_none() || before_base(self.last_cleanup_change.as_ref()))
                    && (!edits.consulted_every_type || before_base(self.last_type_change.as_ref()))
                    && edits
                        .consulted_types
                        .iter()
                        .all(|identity| before_base(self.types.get(identity)))
                    && edits
                        .consulted_domains
                        .iter()
                        .all(|key| before_base(self.domains.get(key)))
            }

            pub(crate) fn record(&mut self, edits: &ShapeEdits, position: usize) {
                let types = edits
                    .types
                    .iter()
                    .map(|(identity, _)| *identity)
                    .chain(edits.in_progress.iter().map(|(identity, _)| *identity))
                    .chain(edits.opaque_representations.iter().map(|row| row.opaque))
                    .collect::<Vec<_>>();
                if !types.is_empty() {
                    self.last_type_change = Some(position);
                }
                self.types
                    .extend(types.into_iter().map(|identity| (identity, position)));
                self.domains.extend(
                    edits
                        .domains_in_progress
                        .iter()
                        .map(|(key, _)| *key)
                        .chain(
                            edits
                                .domains
                                .iter()
                                .map(|row| (row.domain.0, row.carrier_type_identity)),
                        )
                        .map(|key| (key, position)),
                );
                if edits.cleanup_control.is_some() {
                    self.last_cleanup_change = Some(position);
                }
            }
        }

        impl ShapeCollector<'_> {
            /* Build the declaration index every fork would otherwise rebuild. */
            pub(crate) fn prepare_forks(&mut self) {
                self.canonical_carriers();
            }

            /* A copy for planning one body: the same catalogs, nothing consulted. */
            pub(crate) fn fork(&self) -> Self {
                let mut fork = self.clone();
                fork.forget_consulted();
                fork
            }

            pub(crate) fn forget_consulted(&mut self) {
                self.types.forget();
                self.in_progress.0.forget();
                self.domains_in_progress.0.forget();
                self.consulted_domains.clear();
            }

            pub(crate) fn row_marks(&self) -> RowMarks {
                RowMarks {
                    domains: self.domains.len(),
                    opaque_representations: self.opaque_representations.len(),
                    cleanup_rows: self.cleanup_rows(),
                }
            }

            /* Cleanup arenas only grow while bodies are planned. */
            fn cleanup_rows(&self) -> usize {
                let control = &self.cleanup_control;
                control.conditions.len()
                    + control.paths.len()
                    + control.normal_exit_rows.len()
                    + control.normal_result_completions.len()
                    + control.normal_result_arms.len()
                    + control.value_completions.len()
                    + control.actions.len()
                    + control.normal_exit_regions.len()
                    + control.reference_owner_cleanups.len()
            }

            /*
                What this attempt consulted and wrote since `marks`. A fork passes its
                `base` so the edits carry what replaying it needs.
            */
            pub(crate) fn edits_since(
                &mut self,
                base: Option<&Self>,
                marks: RowMarks,
            ) -> ShapeEdits {
                let (consulted_types, written_types) = self.types.forget();
                let (consulted_progress, written_progress) = self.in_progress.0.forget();
                let (consulted_domain_progress, written_domain_progress) =
                    self.domains_in_progress.0.forget();
                let mut consulted_domains = std::mem::take(&mut self.consulted_domains);
                consulted_domains.extend(consulted_domain_progress.keys);
                let cleanup_changed = self.cleanup_rows() != marks.cleanup_rows;
                let mut edits = ShapeEdits {
                    consulted_every_type: consulted_types.every_key,
                    consulted_types: consulted_types
                        .keys
                        .into_iter()
                        .chain(consulted_progress.keys)
                        .collect(),
                    consulted_domains,
                    types: written_types,
                    in_progress: written_progress
                        .into_iter()
                        .map(|(identity, now)| (identity, now.is_some()))
                        .collect(),
                    domains_in_progress: written_domain_progress
                        .into_iter()
                        .map(|(key, now)| (key, now.is_some()))
                        .collect(),
                    domains: self.domains[marks.domains..].to_vec(),
                    opaque_representations: self.opaque_representations
                        [marks.opaque_representations..]
                        .to_vec(),
                    completed_requests: Vec::new(),
                    byte_view_elements: Vec::new(),
                    unselected_opaque: self.unselected_opaque,
                    cleanup_control: cleanup_changed.then(Default::default),
                };
                let Some(base) = base else {
                    return edits;
                };
                edits.completed_requests = self.completed_requests.since(&base.completed_requests);
                edits.byte_view_elements = self
                    .byte_view_elements
                    .difference(&base.byte_view_elements)
                    .copied()
                    .collect();
                if cleanup_changed {
                    edits.cleanup_control =
                        Some(std::mem::take(&mut self.cleanup_control).into_inner());
                }
                edits
            }

            /* Replay an admitted fork's writes, as its sequential run made them. */
            pub(crate) fn apply(&mut self, edits: ShapeEdits) {
                for (identity, row) in edits.types {
                    self.types.replay(identity, row);
                }
                for (identity, present) in edits.in_progress {
                    self.in_progress.0.replay(identity, present.then_some(()));
                }
                for (key, present) in edits.domains_in_progress {
                    self.domains_in_progress
                        .0
                        .replay(key, present.then_some(()));
                }
                self.domains.extend(edits.domains);
                self.opaque_representations
                    .extend(edits.opaque_representations);
                for (slot, request, identity) in edits.completed_requests {
                    self.completed_requests.push(slot, request, identity);
                }
                self.byte_view_elements.extend(edits.byte_view_elements);
                self.unselected_opaque = edits.unselected_opaque;
                if let Some(cleanup_control) = edits.cleanup_control {
                    self.cleanup_control = cleanup_control.into();
                }
            }
        }

        /*
            A catalog a fork shares with its siblings until it first writes to it;
            the merged collector, holding the only reference, writes in place.
        */
        #[derive(Clone, Default)]
        pub(crate) struct CopyOnWrite<T>(Arc<T>);

        impl<T> std::ops::Deref for CopyOnWrite<T> {
            type Target = T;
            fn deref(&self) -> &T {
                &self.0
            }
        }

        impl<T: Clone> std::ops::DerefMut for CopyOnWrite<T> {
            fn deref_mut(&mut self) -> &mut T {
                Arc::make_mut(&mut self.0)
            }
        }

        impl<T: Clone> CopyOnWrite<T> {
            pub(crate) fn into_inner(self) -> T {
                Arc::unwrap_or_clone(self.0)
            }
        }

        impl<T> From<T> for CopyOnWrite<T> {
            fn from(value: T) -> Self {
                Self(Arc::new(value))
            }
        }
    }
    pub(crate) use forks::{ChangeLog, ConsultedSet};
    /* The structural type rows the collector has registered, by identity. */
    pub(crate) type TypeCatalog =
        forks::ConsultedMap<TypeIdentityId, CheckedUnitStructuralTypePlan>;
    use checked_trees::CheckedBooleanExpression;
    use checked_trees::CheckedByteSequenceCarrier;
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::CheckedScalarExpression;
    use checked_trees::CheckedUnitRecordStorage;
    use checked_trees::CheckedUnitStructuralCasePlan;
    use language_semantics::DataSupplyMode;
    use language_semantics::Multiplicity;
    use language_semantics::byte_predicates::ByteSequencePredicateSet;
    use typed_trees::PlanLaidLayout;
    use typed_trees::byte_predicates::domain_byte_predicate;
    use typed_trees::byte_predicates::type_reference_domain_predicates;
    use typed_trees::data::DataDefinition;
    use typed_trees::data::DataField;
    use typed_trees::domain::DomainDefinition;
    use typed_trees::domain::domain_by_symbol;
    use typed_trees::domain::required_subject_memberships;
    use typed_trees::operator::declared_domain_constraints;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::DomainConstraint;
    use typed_trees::types::FixedArrayLength;
    use validation::ContentsRequirement;
    use validation::activation_claim_byte_carrier;
    use validation::case_requires_field_intervals;
    use validation::data_has_contents;
    use validation::data_requires_field_intervals;
    use validation::field_domains::domain_predicate_theory_is_complete;
    use validation::field_domains::domain_requires_provenance;
    use validation::has_contents;
    use validation::reference_shell;
    use validation::scalar_domain_predicate;
    use validation::slice_view_shell;
    use validation::stored_structural_type_identity;
    use validation::stores_reference_shape;
    use validation::structural_storage_type;
    use validation::structural_type_identity;
    use validation::substituted_type_satisfies_copy;
    use validation::unrestricted_builtin_primitive;

    /*
        Identity alone can join different source presentations. Memoize only an
        already admitted exact request, retaining its declaration telescope and
        substitutions; another presentation still checks its complete shape.
    */
    #[derive(Clone, PartialEq, Eq)]
    struct TypeShapeRequest {
        reference: TypeReferenceHandle,
        binders: Vec<(SymbolHandle, String)>,
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    }

    #[derive(Clone)]
    pub(crate) struct ShapeCollector<'program> {
        pub(in super::super) program: &'program dyn ::validation::CheckingInput,
        pub(in super::super) types: TypeCatalog,
        pub(in super::super) domains: Vec<CheckedUnitStructuralDomainPlan>,
        pub(in super::super) opaque_representations:
            Vec<checked_trees::CheckedOpaqueRepresentation>,
        pub(in super::super) cleanup_control:
            forks::CopyOnWrite<checked_trees::CheckedCleanupControlPlans>,
        /*
            Compiler-owned handles select arena slots; the small bucket retains
            exact generation/telescope/substitution contexts for that source row.
            This is a collector-local query memo, never a semantic type identity.
        */
        completed_requests: forks::CompletedRequests,
        in_progress: ConsultedSet<TypeIdentityId>,
        domains_in_progress: ConsultedSet<(u32, TypeIdentityId)>,
        /* The domain rows this attempt looked up; see `forks`. */
        consulted_domains: BTreeSet<(u32, TypeIdentityId)>,
        /*
            The element rows registered beside borrowed byte views. A byte
            carrier's plan keeps no element field, so `retain_transitive` follows
            this edge where it follows a `BorrowedSliceView`'s element identity.
        */
        byte_view_elements: BTreeSet<TypeIdentityId>,
        /*
            The opaque boundary datum the last failed `add_type` could not shape
            because the build selects no representation for it. A builder that
            stops on that failure reads it to name the program's missing
            selection instead of the construct it was planning.
        */
        unselected_opaque: Option<SymbolHandle>,
        /*
            The symbol-owning closed declaration settled per structural identity,
            built once on first use: declaration inventory never changes during
            collection, so one index supplies every later route to any identity.
        */
        canonical_carriers: Option<CarrierIndex>,
    }

    /*
        The symbol-owning closed declaration per structural identity. It is a
        pure function of the typed program's declaration inventory, which no
        checking pass changes, so one index built from a program serves every
        collector over that same program.
    */
    pub(crate) type CarrierIndex = std::sync::Arc<BTreeMap<TypeIdentityId, DataDefinition>>;

    /* Index the declaration inventory of `program` once; see `CarrierIndex`. */
    pub(crate) fn carrier_index(program: &dyn ::validation::CheckingInput) -> CarrierIndex {
        let typed = program.typed_program();
        let mut carriers = BTreeMap::new();
        for candidate in typed.data_definitions().iter() {
            let Some(origin) = candidate.generic_instance else {
                continue;
            };
            if matches!(
                typed.type_reference_table.type_reference(origin),
                TypeReferenceNode::Generic { base_symbol, .. }
                    if *base_symbol == candidate.symbol
            ) {
                continue;
            }
            let origin_identity = typed.normalized_type_identity(origin);
            carriers
                .entry(origin_identity)
                .and_modify(|retained: &mut DataDefinition| {
                    if candidate.symbol.arena_index() < retained.symbol.arena_index() {
                        *retained = candidate.clone();
                    }
                })
                .or_insert_with(|| candidate.clone());
        }
        std::sync::Arc::new(carriers)
    }

    impl<'program> ShapeCollector<'program> {
        pub(crate) fn new(program: &'program dyn ::validation::CheckingInput) -> Self {
            Self {
                program,
                types: TypeCatalog::default(),
                domains: Vec::new(),
                opaque_representations: Vec::new(),
                cleanup_control: Default::default(),
                completed_requests: Default::default(),
                in_progress: ConsultedSet::new(),
                domains_in_progress: ConsultedSet::new(),
                consulted_domains: BTreeSet::new(),
                byte_view_elements: BTreeSet::new(),
                unselected_opaque: None,
                canonical_carriers: None,
            }
        }

        /*
            A fresh collector over `program` that starts from `carriers`, which the
            caller built with `carrier_index` from this same `program`.
        */
        pub(crate) fn with_carrier_index(
            program: &'program dyn ::validation::CheckingInput,
            carriers: CarrierIndex,
        ) -> Self {
            let mut collector = Self::new(program);
            collector.canonical_carriers = Some(carriers);
            collector
        }

        /*
            The unselected opaque behind the collector's last shape failure, if
            any, cleared for the next builder.
        */
        pub(in super::super) fn take_unselected_opaque(&mut self) -> Option<SymbolHandle> {
            self.unselected_opaque.take()
        }

        /*
            One shape per identity. An equal row is the same registration; a
            different one contradicts the identity already established.
        */
        fn register_type(
            &mut self,
            identity: TypeIdentityId,
            plan: CheckedUnitStructuralTypePlan,
        ) -> Option<()> {
            let known = self.types.get(&identity);
            if known.is_some_and(|known| known != &plan) {
                return None;
            }
            self.types.insert(identity, plan);
            Some(())
        }

        pub(in super::super) fn add_domain(
            &mut self,
            domain: SemanticDomainId,
            carrier: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
        ) -> Option<TypeIdentityId> {
            let constraints = declared_domain_constraints(self.program.typed_program(), carrier);
            if !constraints.is_empty() {
                let mut selected = constraints
                    .into_iter()
                    .filter(|constraint| constraint.semantic_id == domain);
                let application = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                return self.add_domain_application(application, carrier, binders, false);
            }
            let definition = {
                self.program
                    .domain_definitions()
                    .iter()
                    .find(|definition| definition.semantic_id == domain)
            };
            self.add_domain_definition(domain, carrier, binders, definition, false, None)
        }

        /*
            A typed application retains its closed argument roster. Its instance
            identity is not the uninstantiated declaration's identity, and its
            scalar theory uses the same projection as receiving-field replay.
            Registration records that theory; it establishes no membership.
        */
        pub(in super::super) fn add_domain_application(
            &mut self,
            application: &DomainConstraint,
            carrier: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            stored_carrier: bool,
        ) -> Option<TypeIdentityId> {
            let definition = self
                .program
                .typed_program()
                .domain_definition_for_instance(application.symbol, application.semantic_id)
                .or_else(|| domain_by_symbol(self.program, application.symbol))?;
            let predicate = if application.arguments.is_empty()
                || domain_requires_provenance(self.program, definition.symbol)
            {
                None
            } else if let Some(primitive) = self.program.primitive_type_reference(carrier) {
                let predicate = scalar_domain_predicate(
                    self.program,
                    application,
                    CheckedScalarExpression::Parameter {
                        position: 0,
                        scalar_type: (primitive).into(),
                    },
                )?;
                (!matches!(predicate, CheckedBooleanExpression::Constant(true)))
                    .then_some(predicate)
            } else {
                None
            };
            self.add_domain_definition(
                application.semantic_id,
                carrier,
                binders,
                Some(definition),
                stored_carrier,
                predicate,
            )
        }

        /*
            A contract may establish a domain independently of the qualifications
            already declared on its result carrier. Keep the declaration selected
            by that contract's authorization instead of searching those unrelated
            qualifications for it. Registration describes its theory, not evidence
            that the result satisfies it; the calling contract owns that judgment.
            Typing retains the declaration symbol beside the instantiated semantic
            identity. They are different coordinates for indexed families: the
            declaration owns the routes, while the application owns membership.
            Requiring the family's semantic ID here would erase every closed index.
        */
        pub(in super::super) fn add_selected_domain(
            &mut self,
            domain: SemanticDomainId,
            selected_symbol: SymbolHandle,
            carrier: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            stored_carrier: bool,
        ) -> Option<TypeIdentityId> {
            let definition = self
                .program
                .typed_program()
                .domain_definition_for_instance(selected_symbol, domain)
                .or_else(|| domain_by_symbol(self.program, selected_symbol))?;
            self.add_domain_definition(
                domain,
                carrier,
                binders,
                Some(definition),
                stored_carrier,
                None,
            )
        }

        fn add_domain_definition(
            &mut self,
            domain: SemanticDomainId,
            carrier: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            definition: Option<&DomainDefinition>,
            stored_carrier: bool,
            predicate: Option<CheckedBooleanExpression>,
        ) -> Option<TypeIdentityId> {
            let carrier_type_identity =
                if stored_carrier && stores_reference_shape(self.program, carrier, &[]) {
                    self.add_reference_type(carrier, binders)?
                } else {
                    self.add_type(carrier, binders, &[])?
                };
            let key = (domain.0, carrier_type_identity);
            if self.domains_in_progress.contains(&key) {
                return Some(carrier_type_identity);
            }
            let requirements = definition.and_then(|definition| {
                required_subject_memberships(self.program.typed_program(), definition)
            });
            let required_memberships = requirements.as_ref().map(|requirements| {
                let mut domains = requirements
                    .iter()
                    .map(|membership| membership.semantic_domain)
                    .collect::<Vec<_>>();
                domains.sort_by_key(|domain| domain.0);
                domains.dedup();
                domains
            });
            let identity = self.program.semantic_domains.name(domain)?.to_owned();
            let establishment_routes = definition
                .map(|definition| definition.establishment_routes.clone())
                .unwrap_or_default();
            let byte_predicate = definition
                .filter(|definition| {
                    definition.classification.is_none() && definition.semantic_roles.is_empty()
                })
                .and_then(|definition| domain_byte_predicate(self.program, definition.symbol));
            let complete_predicate_theory = definition.is_some_and(|definition| {
                domain_predicate_theory_is_complete(self.program, definition)
            });
            let plan = CheckedUnitStructuralDomainPlan {
                required_memberships,
                complete_predicate_theory,
                predicate,
                byte_predicate,
                domain,
                name: identity,
                carrier_type_identity,
                establishment_routes,
            };
            let stored_carrier_identity = if slice_view_shell(self.program, carrier, &[]).is_some()
            {
                self.add_stored_type(carrier, binders, &[])?
            } else {
                carrier_type_identity
            };
            for identity in [carrier_type_identity, stored_carrier_identity] {
                self.consulted_domains.insert((domain.0, identity));
                let mut presented = plan.clone();
                presented.carrier_type_identity = identity;
                if let Some(existing) = self.domains.iter().find(|existing| {
                    existing.domain == domain && existing.carrier_type_identity == identity
                }) {
                    if existing != &presented {
                        return None;
                    }
                } else {
                    self.domains.push(presented);
                }
            }
            self.domains_in_progress.insert(key);
            let collected = requirements.as_ref().is_none_or(|requirements| {
                requirements.iter().all(|membership| {
                    self.add_selected_domain(
                        membership.semantic_domain,
                        membership.domain_symbol,
                        carrier,
                        binders,
                        stored_carrier,
                    )
                    .is_some()
                })
            });
            self.domains_in_progress.remove(&key);
            collected.then_some(carrier_type_identity)
        }

        pub(in super::super) fn add_attached_data(
            &mut self,
            data: &DataDefinition,
            binders: &[(SymbolHandle, String)],
        ) -> Option<TypeIdentityId> {
            if let Some(identity) = self.add_accessor_attachment(data, binders) {
                return Some(identity);
            }
            if !self.program.data_type_parameters(data).is_empty() {
                return None;
            }
            let identity = closed_data_identity(self.program, data, binders);
            self.add_data_shape(
                identity,
                data.clone(),
                binders,
                Vec::new(),
                typed_trees::SourceTypeApplication::default(),
            )
        }

        /*
            A access-view plan's generated accessor shell stays `BoundaryOpaque`
            to source construction, yet each cloned `read`/`take`/`write` target
            still attaches to it. Admit that attachment through the exact plan
            row — not the generated name — as the empty nominal record the layout
            catalog already assigns the same definition. The shell keeps no
            members a source carrier could construct; this only publishes the
            attachment identity its boundary operations and their callers join.
        */
        fn add_accessor_attachment(
            &mut self,
            data: &DataDefinition,
            binders: &[(SymbolHandle, String)],
        ) -> Option<TypeIdentityId> {
            if data.supply_mode != DataSupplyMode::BoundaryOpaque
                || !self
                    .program
                    .access_view_plans
                    .iter()
                    .flat_map(|view| view.fields.iter())
                    .any(|field| field.accessor_data_symbol == data.symbol)
            {
                return None;
            }
            let (view, field) = self.program.access_view_plans.iter().find_map(|view| {
                view.fields
                    .iter()
                    .find(|field| field.accessor_data_symbol == data.symbol)
                    .map(|field| (view, field))
            })?;
            let descriptor = view.placement.access().field_descriptor(field.access_key);
            let byte_size = if view
                .placement
                .access()
                .field_region(field.access_key)
                .is_some()
            {
                let (value, _) = field.scalar_projection_leaf(self.program).ok()?;
                u64::try_from(
                    self.program
                        .primitive_type_reference(value)?
                        .scalar_byte_size()?,
                )
                .ok()?
            } else {
                descriptor.map_or(0, |descriptor| descriptor.effect_footprint().length_bytes())
            };
            let storage = semantic_vocabulary::AuthoredRecordStorage {
                byte_size,
                byte_alignment: 1,
            };
            let identity = closed_data_identity(self.program, data, binders);
            let plan = CheckedUnitStructuralTypePlan {
                descriptor_presentation: Default::default(),
                source_declaration: Default::default(),
                source_application: Default::default(),
                native_identity: None,
                native_policy_identity: None,
                native_semantic_identity: None,
                scalar_restriction: None,
                owns_linear_claim: data.properties.multiplicity == Multiplicity::Linear,
                no_code_affine_cleanup: false,
                copyable: data.properties.multiplicity == Multiplicity::Unrestricted,
                descriptor_cleanup: false,
                identity,
                shape: CheckedUnitStructuralTypeShape::Record {
                    storage: CheckedUnitRecordStorage::Accessor(storage),
                    fields: Vec::new(),
                },
            };
            if self
                .types
                .get(&identity)
                .is_some_and(|existing| *existing != plan)
            {
                return None;
            }
            self.types.insert(identity, plan);
            Some(identity)
        }

        /*
            A const actual's extent: its literal, or the recipe a still-pending
            const-generic argument retains for the target closure.
        */
        fn const_length_extent(&self, reference: TypeReferenceHandle) -> Option<CheckedExtent> {
            self.program.declared_constant_extent(reference)
        }

        /*
            Register an admitted reference carrier with its declared access.
            Access selects the loan contract, not whether its descriptor exists.
        */
        pub(in super::super) fn add_reference_type(
            &mut self,
            reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
        ) -> Option<TypeIdentityId> {
            let shell = reference_shell(self.program, reference)?;
            self.add_view_type(reference, binders, shell.access)
        }

        /*
            A shared, mutable or write-only view spells
            its own type reference: the reference shell is the loan custody the
            plan carries, so unlike `add_type` it is retained as a `Reference`
            shape over its exact stored referee rather than peeled away. Each
            reference edge retains its own access; shape collection grants neither
            a pointee read nor its ownership. Stored slice descriptors register
            through `add_stored_type`.
        */
        pub(in super::super) fn add_view_type(
            &mut self,
            reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            access: language_semantics::ReferenceAccess,
        ) -> Option<TypeIdentityId> {
            let identity = self.add_view_type_for_substitutions(reference, binders, &[], access)?;
            let shell = reference_shell(self.program, reference)?;
            self.add_stored_type(shell.referee, binders, &[])?;
            Some(identity)
        }

        fn add_view_type_for_substitutions(
            &mut self,
            reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            access: language_semantics::ReferenceAccess,
        ) -> Option<TypeIdentityId> {
            if !structural_storage_type(self.program, reference, substitutions).is_valid() {
                return None;
            }
            let shell = reference_shell(self.program, reference)?;
            if shell.access != access {
                return None;
            }
            let referee = substituted_formal_type(self.program, shell.referee, substitutions);
            let referent_identity =
                stored_structural_type_identity(self.program, referee, binders, substitutions);
            if reference_shell(self.program, referee).is_some() {
                let identity = stored_structural_type_identity(
                    self.program,
                    reference,
                    binders,
                    substitutions,
                );
                if !self.in_progress.insert(identity) {
                    return None;
                }
                let collected = self.add_stored_type(referee, binders, substitutions);
                self.in_progress.remove(&identity);
                if collected != Some(referent_identity) {
                    return None;
                }
            }
            self.add_reference_shape(
                reference,
                binders,
                substitutions,
                referent_identity,
                access.into(),
                typed_trees::boundary_values::foreign_reference_uses_descriptor(
                    self.program,
                    referee,
                ),
            )
        }

        /*
            Register the pointer carrier and its exact referent identity. The edge
            does not demand a referent storage definition. One identity names one
            shape: a conflicting earlier plan declines the registration.
        */
        fn add_reference_shape(
            &mut self,
            reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            referent_identity: TypeIdentityId,
            access: CheckedStructuralAccess,
            foreign_descriptor: bool,
        ) -> Option<TypeIdentityId> {
            let identity =
                stored_structural_type_identity(self.program, reference, binders, substitutions);
            let referee = substituted_formal_type(
                self.program,
                reference_shell(self.program, reference)?.referee,
                substitutions,
            );
            let abstract_referent = self
                .program
                .closed_data_definition(referee)
                .is_some_and(|data| data.supply_mode == DataSupplyMode::BoundaryOpaque);
            let plan = CheckedUnitStructuralTypePlan {
                descriptor_presentation: Default::default(),
                source_declaration: Default::default(),
                source_application: Default::default(),
                native_identity: None,
                native_policy_identity: None,
                native_semantic_identity: None,
                scalar_restriction: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: access == CheckedStructuralAccess::SharedBorrow,
                descriptor_cleanup: access != CheckedStructuralAccess::Owned,
                identity,
                shape: CheckedUnitStructuralTypeShape::Reference {
                    referent_identity,
                    access,
                    foreign_descriptor,
                    abstract_referent,
                },
            };
            self.register_type(identity, plan)?;
            Some(identity)
        }

        /*
            A checked reference formation demands the storage of its exact source
            leaf. An equal stored carrier transports the existing descriptor.
            Otherwise peel only the newly formed loan: an inner reference remains a
            stored descriptor. This is an operation dependency, not permission to
            expand the pointees of every reference field in the type catalog.
        */
        pub(in super::super) fn add_reference_backing(
            &mut self,
            reference: TypeReferenceHandle,
            source_identity: TypeIdentityId,
            binders: &[(SymbolHandle, String)],
        ) -> Option<()> {
            if stored_structural_type_identity(self.program, reference, binders, &[])
                == source_identity
            {
                return (self.add_stored_type(reference, binders, &[])? == source_identity)
                    .then_some(());
            }
            let identity = self.add_type(reference, binders, &[])?;
            (identity == source_identity).then_some(())
        }

        /*
            Keep every source-contract carrier and theory through catalog pruning.
            The requirement record survives even when a carrier cannot be shaped;
            an actual Terminal consumer still refuses that missing declaration.
        */
        pub(in super::super) fn retain_reference_contract_types(
            &mut self,
            contract: &checked_trees::CheckedReferenceSourceContract,
            roots: &mut impl Extend<TypeIdentityId>,
        ) {
            let checked_trees::CheckedReferenceSourceContract::Standing { contract, .. } = contract
            else {
                return;
            };
            for (domain, reference) in &contract.domains {
                if let Some(identity) = self.add_domain(*domain, *reference, &[]) {
                    roots.extend(std::iter::once(identity));
                }
            }
            for reference in &contract.projection_types {
                if let Some(identity) = self.add_stored_type(*reference, &[], &[]) {
                    roots.extend(std::iter::once(identity));
                }
            }
        }

        /*
            The primitive a registered identity names when its catalog row is a
            primitive scalar. A borrowed formal registers its peeled referent, so
            this is the primitive a `&u8` or `&mut i64` formal lends.
        */
        pub(in super::super) fn primitive_scalar(
            &self,
            identity: TypeIdentityId,
        ) -> Option<PrimitiveType> {
            match self.types.get(&identity).map(|plan| &plan.shape) {
                Some(CheckedUnitStructuralTypeShape::Scalar(scalar_type)) => {
                    scalar_type.primitive()
                }
                _ => None,
            }
        }

        /*
            Boundary formals transport their actual carrier. Only represented
            backing supplies an inline callback path; an abstract foreign pointee
            has neither fields nor private slots to materialize.
        */
        pub(in super::super) fn add_calling_parameter(
            &mut self,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<checked_trees::CheckedCallingParameter> {
            let identity = self.add_type(reference, &[], substitutions);
            let logical = self.add_logical_type(reference, &[], substitutions);
            let mut reference_custody = None;
            if logical.is_none() {
                let mut referee = substituted_formal_type(self.program, reference, substitutions);
                let mut borrowed = false;
                let mut visited = Vec::new();
                while let Some(shell) = reference_shell(self.program, referee) {
                    if visited.contains(&referee) {
                        return None;
                    }
                    visited.push(referee);
                    borrowed = true;
                    referee = substituted_formal_type(self.program, shell.referee, substitutions);
                }
                let symbol = match self.program.type_reference_table.type_reference(referee) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => SymbolHandle::invalid(),
                };
                if !borrowed || self.unselected_opaque != Some(symbol) {
                    return None;
                }
                reference_custody = Some(self.add_stored_type(reference, &[], substitutions)?);
                self.unselected_opaque = None;
            }
            let identity = identity.unwrap_or_else(|| {
                structural_type_identity(self.program, reference, &[], substitutions)
            });
            let callback_backing = self.types.contains_key(&identity).then_some(identity);
            if callback_backing.is_none() && logical.is_some() {
                return None;
            }
            let effective = substituted_formal_type(self.program, reference, substitutions);
            let access = super::structural_access_for_type_reference(self.program, effective)?;
            Some(checked_trees::CheckedCallingParameter {
                type_identity: identity,
                callback_backing,
                reference_custody,
                access,
            })
        }

        /*
            Collect the value held in a field or element, rather than the storage
            borrowed by an argument. Sequence shells retain their descriptor route;
            every other stored loan must retain a Reference shape. Keeping this
            distinction here prevents an aggregate boundary from inlining a referent
            or making an unsupported loan look like owned storage.
        */
        pub(in super::super) fn add_stored_type(
            &mut self,
            reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeIdentityId> {
            let reference = substituted_formal_type(self.program, reference, substitutions);
            if let Some(shell) = slice_view_shell(self.program, reference, substitutions) {
                let canonical = self.add_type(reference, binders, substitutions)?;
                if !canonical.is_valid() {
                    return None;
                }
                let identity = stored_structural_type_identity(
                    self.program,
                    reference,
                    binders,
                    substitutions,
                );
                let mut stored = self.types.get(&canonical)?.clone();
                let access = match &mut stored.shape {
                    CheckedUnitStructuralTypeShape::BorrowedSliceView { access, .. }
                    | CheckedUnitStructuralTypeShape::ByteSequence(
                        CheckedByteSequenceCarrier::BorrowedView { access }
                        | CheckedByteSequenceCarrier::BoundedView { access, .. },
                    ) => access,
                    _ => return None,
                };
                *access = Some(shell.access.into());
                stored.copyable = shell.access == language_semantics::ReferenceAccess::Shared;
                stored.identity = identity;
                stored.descriptor_presentation = canonical;
                if self
                    .types
                    .get(&identity)
                    .is_some_and(|existing| existing != &stored)
                {
                    return None;
                }
                self.types.insert(identity, stored);
                return Some(identity);
            }
            if stores_reference_shape(self.program, reference, substitutions) {
                let shell = reference_shell(self.program, reference)?;
                return self.add_view_type_for_substitutions(
                    reference,
                    binders,
                    substitutions,
                    shell.access,
                );
            }
            self.add_type(reference, binders, substitutions)
        }

        /*
            Demand a source root's logical presentation without treating a view
            descriptor as by-value unsized backing. Stored pointers contribute
            physical crossings; a view already presents its elements implicitly.
            The finite type graph bounds this geometry walk and grants no access.
        */
        pub(in super::super) fn add_logical_type(
            &mut self,
            mut reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeIdentityId> {
            if !structural_storage_type(self.program, reference, substitutions).is_valid() {
                return None;
            }
            for _ in 0..=self.program.type_reference_table.type_reference_count() {
                reference = substituted_formal_type(self.program, reference, substitutions);
                let Some(shell) = reference_shell(self.program, reference) else {
                    return self.add_type(reference, binders, substitutions);
                };
                if !stores_reference_shape(self.program, reference, substitutions) {
                    return self.add_type(reference, binders, substitutions);
                }
                reference = shell.referee;
            }
            None
        }

        pub(in super::super) fn add_type(
            &mut self,
            type_reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeIdentityId> {
            let slot = type_reference.arena_index() as usize;
            if let Some(identity) = self.completed_requests.find(slot, |candidate| {
                candidate.reference == type_reference
                    && candidate.binders == binders
                    && candidate.substitutions == substitutions
            }) && self.types.contains_key(&identity)
            {
                return (!self.in_progress.contains(&identity)).then_some(identity);
            }
            let identity = self.collect_type(type_reference, binders, substitutions)?;
            let request = TypeShapeRequest {
                reference: type_reference,
                binders: binders.to_vec(),
                substitutions: substitutions.to_vec(),
            };
            self.completed_requests.push(slot, request, identity);
            Some(identity)
        }

        fn collect_type(
            &mut self,
            type_reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeIdentityId> {
            let storage = structural_storage_type(self.program, type_reference, substitutions);
            if !storage.is_valid() {
                return None;
            }
            if reference_shell(self.program, storage).is_some() {
                let identity = self.add_stored_type(storage, binders, substitutions)?;
                self.add_logical_type(storage, binders, substitutions)?;
                return Some(identity);
            }
            let identity =
                structural_type_identity(self.program, type_reference, binders, substitutions);
            let program = self.program;
            let storage_properties = || {
                let copyable = substituted_type_satisfies_copy(program, storage, substitutions);
                let no_code_affine_cleanup = has_contents(
                    program,
                    storage,
                    substitutions,
                    ContentsRequirement::RepresentedOwned,
                );
                let descriptor_cleanup = has_contents(
                    program,
                    storage,
                    substitutions,
                    ContentsRequirement::DescriptorCleanup,
                );
                let no_code_affine_cleanup =
                    no_code_affine_cleanup || (descriptor_cleanup && copyable);
                (copyable, no_code_affine_cleanup, descriptor_cleanup)
            };
            if let Some(carrier) =
                byte_sequence_carrier(self.program, type_reference, substitutions)
            {
                let (copyable, no_code_affine_cleanup, _) = storage_properties();
                if matches!(
                    carrier,
                    CheckedByteSequenceCarrier::BorrowedView { .. }
                        | CheckedByteSequenceCarrier::BoundedView { .. }
                        | CheckedByteSequenceCarrier::BoundedOwned { .. }
                ) {
                    let referee = substituted_formal_type(
                        self.program,
                        self.program
                            .type_reference_table
                            .peeled_type_reference(type_reference),
                        substitutions,
                    );
                    if let TypeReferenceNode::Slice { element_type }
                    | TypeReferenceNode::FixedArray { element_type, .. } =
                        self.program.type_reference_table.type_reference(referee)
                    {
                        let element = self.add_type(*element_type, binders, substitutions)?;
                        self.byte_view_elements.insert(element);
                    }
                }
                let plan = CheckedUnitStructuralTypePlan {
                    descriptor_presentation: Default::default(),
                    source_declaration: Default::default(),
                    source_application: Default::default(),
                    native_identity: None,
                    native_policy_identity: None,
                    native_semantic_identity: None,
                    scalar_restriction: None,
                    copyable: copyable && !carrier.is_borrowed(),
                    descriptor_cleanup: carrier.is_borrowed(),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: no_code_affine_cleanup
                        && matches!(carrier, CheckedByteSequenceCarrier::BoundedOwned { .. }),
                    identity,
                    shape: CheckedUnitStructuralTypeShape::ByteSequence(carrier.without_access()),
                };
                self.register_type(identity, plan)?;
                return Some(identity);
            }
            if let Some(element) =
                borrowed_slice_view_element(self.program, type_reference, substitutions)
            {
                if !self.in_progress.insert(identity) {
                    return None;
                }
                let element_type_identity = self.add_type(element, binders, substitutions);
                self.in_progress.remove(&identity);
                let plan = CheckedUnitStructuralTypePlan {
                    descriptor_presentation: Default::default(),
                    source_declaration: Default::default(),
                    source_application: Default::default(),
                    native_identity: None,
                    native_policy_identity: None,
                    native_semantic_identity: None,
                    scalar_restriction: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: true,
                    identity,
                    shape: CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity: element_type_identity?,
                        access: None,
                    },
                };
                self.register_type(identity, plan)?;
                return Some(identity);
            }
            let type_reference = storage;
            if self.types.contains_key(&identity) {
                return Some(identity);
            }
            if let Some(scalar_type) = checked_trees::CheckedScalarType::from_type_reference(
                self.program.typed_program(),
                type_reference,
            ) {
                let (copyable, no_code_affine_cleanup, descriptor_cleanup) = storage_properties();
                let (shape, scalar_restriction) = match scalar_fields::retain_scalar_field(
                    self.program,
                    type_reference,
                    substitutions,
                    scalar_type,
                    None,
                )? {
                    CheckedUnitStructuralFieldType::Scalar(_) => {
                        (CheckedUnitStructuralTypeShape::Scalar(scalar_type), None)
                    }
                    restriction @ (CheckedUnitStructuralFieldType::BoundedInteger(_)
                    | CheckedUnitStructuralFieldType::RefinedScalar { .. }) => (
                        CheckedUnitStructuralTypeShape::Scalar(scalar_type),
                        Some(restriction),
                    ),
                    CheckedUnitStructuralFieldType::Atomic(primitive) => {
                        let cell = if primitive == typed_trees::types::PrimitiveType::Bool {
                            semantic_vocabulary::AtomicCellType::Boolean
                        } else {
                            let (sign, bits) = primitive.integer_sign_bits()?;
                            semantic_vocabulary::AtomicCellType::Integer(
                                semantic_vocabulary::IntegerType::new(sign, bits).ok()?,
                            )
                        };
                        cell.is_supported().then_some(())?;
                        (CheckedUnitStructuralTypeShape::AtomicCell(cell), None)
                    }
                    CheckedUnitStructuralFieldType::AtomicBoundedInteger(bounds) => {
                        let cell = semantic_vocabulary::AtomicCellType::BoundedInteger(bounds);
                        cell.is_supported().then_some(())?;
                        (CheckedUnitStructuralTypeShape::AtomicCell(cell), None)
                    }
                    _ => return None,
                };
                self.types.insert(
                    identity,
                    CheckedUnitStructuralTypePlan {
                        descriptor_presentation: Default::default(),
                        source_declaration: Default::default(),
                        source_application: Default::default(),
                        native_identity: None,
                        native_policy_identity: None,
                        native_semantic_identity: None,
                        scalar_restriction,
                        owns_linear_claim: false,
                        no_code_affine_cleanup,
                        copyable,
                        descriptor_cleanup,
                        identity,
                        shape,
                    },
                );
                return Some(identity);
            }
            if let TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let (copyable, no_code_affine_cleanup, descriptor_cleanup) = storage_properties();
                let length = (match length {
                    FixedArrayLength::Literal(value) => {
                        u64::try_from(*value).ok().map(CheckedExtent::Literal)
                    }
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        substitution_for(substitutions, *symbol)
                            .and_then(|replacement| self.const_length_extent(replacement))
                    }
                    FixedArrayLength::Expression(argument) => self.const_length_extent(*argument),
                    FixedArrayLength::ConstCall { .. } => {
                        let recipe = self
                            .program
                            .constant_recipe_for_array_length(type_reference);
                        recipe.is_valid().then_some(CheckedExtent::Recipe(recipe))
                    }
                })?;
                if !self.in_progress.insert(identity) {
                    return None;
                }
                let element = substituted_formal_type(self.program, *element_type, substitutions);
                let missing_membership = self
                    .program
                    .primitive_type_reference(element)
                    .is_some_and(|primitive| {
                        matches!(
                            scalar_fields::retain_scalar_field(
                                self.program,
                                element,
                                substitutions,
                                primitive,
                                None
                            ),
                            Some(CheckedUnitStructuralFieldType::Scalar(_))
                        ) && unrestricted_builtin_primitive(self.program, element)
                            != Some(primitive)
                    });
                let element_type_identity = if missing_membership {
                    None
                } else {
                    self.add_stored_type(element, binders, substitutions)
                };
                let Some(element_type_identity) = element_type_identity else {
                    self.in_progress.remove(&identity);
                    return None;
                };
                self.types.insert(
                    identity,
                    CheckedUnitStructuralTypePlan {
                        descriptor_presentation: Default::default(),
                        source_declaration: Default::default(),
                        source_application: Default::default(),
                        native_identity: None,
                        native_policy_identity: None,
                        native_semantic_identity: None,
                        scalar_restriction: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup,
                        copyable,
                        descriptor_cleanup,
                        identity,
                        shape: CheckedUnitStructuralTypeShape::FixedArray {
                            element_type_identity,
                            length,
                        },
                    },
                );
                self.in_progress.remove(&identity);
                return Some(identity);
            }
            if let Some(data) = self.program.closed_data_definition(type_reference) {
                return self.add_data_shape(
                    identity,
                    data.clone(),
                    binders,
                    Vec::new(),
                    typed_trees::SourceTypeApplication {
                        reference: type_reference,
                        substitutions: substitutions.to_vec(),
                    },
                );
            }
            let (data_symbol, arguments) = match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Named { symbol, name }
                    if PrimitiveType::from_name(name.as_str()).is_none() =>
                {
                    (*symbol, Vec::new())
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => (
                    *base_symbol,
                    self.program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec(),
                ),
                _ => {
                    return None;
                }
            };
            let data = self.program.data_definition_by_symbol(data_symbol)?.clone();
            let members = self.program.data_members(&data);
            if data.supply_mode != DataSupplyMode::CheckedShape
                || !matches!(
                    DataDefinition::shape_kind_from_members(members),
                    DataShapeKind::Empty
                        | DataShapeKind::Record
                        | DataShapeKind::Enum
                        | DataShapeKind::Mixed
                )
            {
                return None;
            }
            let data_parameters = self.program.data_type_parameters(&data);
            if data_parameters.len() != arguments.len() {
                return None;
            }
            let mut local_substitutions = substitutions.to_vec();
            local_substitutions.extend(
                data_parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, argument)),
            );
            self.add_data_shape(
                identity,
                data,
                binders,
                local_substitutions,
                typed_trees::SourceTypeApplication {
                    reference: type_reference,
                    substitutions: substitutions.to_vec(),
                },
            )
        }

        pub(in super::super) fn add_affine_array_construction_type(
            &mut self,
            type_reference: TypeReferenceHandle,
            binders: &[(SymbolHandle, String)],
        ) -> Option<TypeIdentityId> {
            self.add_type(type_reference, binders, &[])
        }

        /*
            Resolve the one closed declaration that owns a minted symbol for this
            structural identity. Preparation can retain a per-occurrence
            application view that borrows the authored declaration's symbol beside
            the generated carrier; an origin whose `base_symbol` is still the
            candidate's own symbol marks that borrowed view, never the carrier.
        */
        fn canonical_closed_carrier(&mut self, identity: TypeIdentityId) -> Option<DataDefinition> {
            self.canonical_carriers().get(&identity).cloned()
        }

        /*
            The symbol-owning closed declaration per structural identity, indexed
            once from the declaration inventory.
        */
        fn canonical_carriers(&mut self) -> &BTreeMap<TypeIdentityId, DataDefinition> {
            let program = self.program;
            self.canonical_carriers
                .get_or_insert_with(|| carrier_index(program))
        }

        pub(in super::super) fn add_data_shape(
            &mut self,
            identity: TypeIdentityId,
            data: DataDefinition,
            binders: &[(SymbolHandle, String)],
            substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            source_application: typed_trees::SourceTypeApplication,
        ) -> Option<TypeIdentityId> {
            let already_in_progress = self.in_progress.contains(&identity);
            let result = self.collect_data_shape(identity, data, binders, substitutions);
            if let Some(plan) = result.and_then(|identity| self.types.get_mut(&identity)) {
                plan.source_application = source_application;
            }
            if !already_in_progress {
                self.in_progress.remove(&identity);
            }
            result
        }

        fn collect_data_shape(
            &mut self,
            identity: TypeIdentityId,
            data: DataDefinition,
            binders: &[(SymbolHandle, String)],
            substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        ) -> Option<TypeIdentityId> {
            let (mut data, substitutions) = match self.canonical_closed_carrier(identity) {
                Some(canonical)
                    if canonical.symbol != data.symbol
                        || canonical.generic_instance != data.generic_instance =>
                {
                    (canonical, Vec::new())
                }
                _ => (data, substitutions),
            };
            let copyable = data.properties.multiplicity == Multiplicity::Unrestricted;
            if let Some(accessor) = self.add_accessor_attachment(&data, binders) {
                return (accessor == identity).then_some(identity);
            }
            if self.types.contains_key(&identity) {
                return Some(identity);
            }
            if !self.in_progress.insert(identity) {
                return None;
            }
            if let Some(quotient) = data.quotient.as_ref() {
                let carrier = self.add_type(quotient.carrier, binders, &substitutions);
                let plan = carrier.and_then(|carrier| self.types.get(&carrier).cloned());
                self.in_progress.remove(&identity);
                let mut plan = plan?;
                plan.identity = identity;
                plan.copyable = copyable;
                self.types.insert(identity, plan);
                return Some(identity);
            }
            let owns_linear_claim = data.properties.multiplicity == Multiplicity::Linear;
            if data.supply_mode == DataSupplyMode::BoundaryOpaque {
                if self.program.core_layout_declaration(
                    data.symbol,
                    language_core::sealed_intrinsics::SealedIntrinsic::LayoutTargetData,
                ) {
                    self.in_progress.remove(&identity);
                    return None;
                }
                if activation_claim_byte_carrier(self.program, data.symbol).is_some() {
                    self.types.insert(
                        identity,
                        CheckedUnitStructuralTypePlan {
                            descriptor_presentation: Default::default(),
                            source_declaration: Default::default(),
                            source_application: Default::default(),
                            native_identity: None,
                            native_policy_identity: None,
                            native_semantic_identity: None,
                            scalar_restriction: None,
                            owns_linear_claim: true,
                            no_code_affine_cleanup: false,
                            copyable: false,
                            descriptor_cleanup: false,
                            identity,
                            shape: CheckedUnitStructuralTypeShape::ByteSequence(
                                CheckedByteSequenceCarrier::BorrowedView {
                                    access: Some(CheckedStructuralAccess::MutableBorrow),
                                },
                            ),
                        },
                    );
                    self.in_progress.remove(&identity);
                    return Some(identity);
                }
                let Some(selection) = representation_selections::selection_for_opaque(
                    self.program.opaque_representation_selections(),
                    data.symbol,
                ) else {
                    self.in_progress.remove(&identity);
                    self.unselected_opaque = Some(data.symbol);
                    return None;
                };
                let Some(carrier) = self
                    .program
                    .closed_data_definition(selection.carrier_type())
                else {
                    self.in_progress.remove(&identity);
                    return None;
                };
                let application = selection
                    .portable_application(self.program.typed_program())
                    .ok()?;
                let carrier = carrier.clone();
                let carrier_identity = self.add_type(selection.carrier_type(), &[], &[])?;
                let relation = checked_trees::CheckedOpaqueRepresentation {
                    opaque: identity,
                    carrier: carrier_identity,
                    application,
                };
                if let Some(existing) = self
                    .opaque_representations
                    .iter()
                    .find(|row| row.opaque == identity)
                {
                    if existing != &relation {
                        return None;
                    }
                } else {
                    self.opaque_representations.push(relation);
                }
                data = carrier;
            }
            let no_code_affine_cleanup = !owns_linear_claim
                && data_has_contents(
                    self.program,
                    &data,
                    &substitutions,
                    ContentsRequirement::RepresentedOwned,
                );
            let descriptor_cleanup = data_has_contents(
                self.program,
                &data,
                &substitutions,
                ContentsRequirement::DescriptorCleanup,
            );
            let no_code_affine_cleanup =
                no_code_affine_cleanup || (descriptor_cleanup && !owns_linear_claim && copyable);
            let members = self.program.data_members(&data).to_vec();
            if data.supply_mode != DataSupplyMode::CheckedShape
                || !matches!(
                    DataDefinition::shape_kind_from_members(&members),
                    DataShapeKind::Empty
                        | DataShapeKind::Record
                        | DataShapeKind::Enum
                        | DataShapeKind::Mixed
                )
            {
                self.in_progress.remove(&identity);
                return None;
            }
            let shape_kind = DataDefinition::shape_kind_from_members(&members);
            let stated_intervals =
                data_requires_field_intervals(self.program, &data).unwrap_or_default();
            let stated = |field: &DataField| {
                stated_intervals
                    .iter()
                    .find(|(symbol, _, _)| *symbol == field.symbol)
                    .map(|(_, low, high)| (*low, *high))
            };
            if matches!(shape_kind, DataShapeKind::Enum | DataShapeKind::Mixed) {
                let mut fields = Vec::new();
                let mut cases = Vec::with_capacity(members.len());
                for member in &members {
                    match member {
                        DataMember::Field(field) => {
                            let Some(field) = self.structural_field_plan(
                                field,
                                binders,
                                &substitutions,
                                identity,
                                stated(field),
                            ) else {
                                self.in_progress.remove(&identity);
                                return None;
                            };
                            fields.push(field);
                        }
                        DataMember::Variant(variant) => {
                            if ::validation::impossible_variant(self.program, variant) {
                                continue;
                            }
                            let intervals = case_requires_field_intervals(self.program, variant)
                                .unwrap_or_default();
                            let mut payload_fields = Vec::new();
                            for field in self.program.data_payload_fields(variant) {
                                let bounds = intervals
                                    .iter()
                                    .find(|(symbol, _, _)| *symbol == field.symbol)
                                    .map(|(_, low, high)| (*low, *high));
                                let Some(field) = self.structural_field_plan(
                                    field,
                                    binders,
                                    &substitutions,
                                    identity,
                                    bounds,
                                ) else {
                                    self.in_progress.remove(&identity);
                                    return None;
                                };
                                payload_fields.push(field);
                            }
                            cases.push(CheckedUnitStructuralCasePlan {
                                identity: CheckedMemberIdentity::of_member(
                                    variant.identity,
                                    variant.name.clone(),
                                ),
                                fields: payload_fields,
                            });
                        }
                    }
                }
                self.types.insert(
                    identity,
                    CheckedUnitStructuralTypePlan {
                        descriptor_presentation: Default::default(),
                        source_declaration: Default::default(),
                        source_application: Default::default(),
                        native_identity: None,
                        native_policy_identity: None,
                        native_semantic_identity: None,
                        scalar_restriction: None,
                        owns_linear_claim,
                        no_code_affine_cleanup,
                        copyable,
                        descriptor_cleanup,
                        identity,
                        shape: if matches!(shape_kind, DataShapeKind::Mixed) {
                            CheckedUnitStructuralTypeShape::Mixed { fields, cases }
                        } else {
                            CheckedUnitStructuralTypeShape::Sum { cases }
                        },
                    },
                );
                self.in_progress.remove(&identity);
                return Some(identity);
            }

            let program = self.program;
            let authored_layout = program
                .plan_laid_layouts
                .iter()
                .find(|plan| program.normalized_type_identity(plan.application) == identity);
            let access_view = program.access_view_plans.iter().find_map(|view| {
                let definition = program.data_definition_by_symbol(view.data_symbol)?;
                (closed_data_identity(program, definition, binders) == identity).then_some(view)
            });
            let record_storage = if let Some(view) = access_view {
                let schema_symbol = view.schema_symbol;
                let schema = program.data_definition_by_symbol(schema_symbol)?.clone();
                CheckedUnitRecordStorage::AccessView {
                    schema_type_identity: self.add_attached_data(&schema, binders)?,
                    storage: semantic_vocabulary::AuthoredRecordStorage {
                        byte_size: view.placement.layout().size?,
                        byte_alignment: view.placement.layout().align,
                    },
                }
            } else {
                match authored_layout {
                    Some(layout) => match layout.invariant_geometry() {
                        Some(geometry) => CheckedUnitRecordStorage::Authored(
                            semantic_vocabulary::AuthoredRecordStorage {
                                byte_size: u64::try_from(geometry.size).ok()?,
                                byte_alignment: u64::try_from(geometry.align).ok()?,
                            },
                        ),
                        None => {
                            let schema_type_identity =
                                self.add_type(layout.semantic_type, binders, &[])?;
                            CheckedUnitRecordStorage::TargetLayout {
                                application: layout.application,
                                application_identity: program
                                    .normalized_type_identity(layout.application),
                                schema_type_identity,
                            }
                        }
                    },
                    None => CheckedUnitRecordStorage::Packed,
                }
            };
            let mut fields = Vec::new();
            for member in &members {
                let Some(field) = member.as_field() else {
                    self.in_progress.remove(&identity);
                    return None;
                };
                let bounds = stated(field);
                let Some(mut field_plan) =
                    self.structural_field_plan(field, binders, &substitutions, identity, bounds)
                else {
                    self.in_progress.remove(&identity);
                    return None;
                };
                if let Some(layout) = authored_layout
                    && !field.relevance.is_erased()
                    && layout.invariant_geometry().is_some()
                {
                    field_plan.storage = Some(authored_field_storage(
                        layout,
                        field.symbol,
                        &field_plan.field_type,
                    )?);
                }
                if let Some(view) = access_view {
                    let access_field = view
                        .fields
                        .iter()
                        .find(|candidate| candidate.view_field_symbol == field.symbol)?;
                    if let Some(descriptor) = view
                        .placement
                        .access()
                        .field_descriptors()
                        .iter()
                        .find(|descriptor| descriptor.key() == access_field.access_key)
                    {
                        field_plan.storage =
                            Some(semantic_vocabulary::AuthoredFieldStorage::Direct {
                                byte_offset: descriptor.container_byte_offset(),
                            });
                    } else if let Some(region) = view
                        .placement
                        .access()
                        .field_region(access_field.access_key)
                    {
                        field_plan.storage =
                            Some(semantic_vocabulary::AuthoredFieldStorage::Direct {
                                byte_offset: region.byte_offset(),
                            });
                    }
                }
                fields.push(field_plan);
            }
            let native_policy_identity = authored_layout
                .and_then(|plan| plan.native_policy_identity(program.typed_program()).ok());
            self.types.insert(
                identity,
                CheckedUnitStructuralTypePlan {
                    descriptor_presentation: Default::default(),
                    source_declaration: data.symbol,
                    source_application: Default::default(),
                    native_policy_identity: native_policy_identity.clone(),
                    native_semantic_identity: authored_layout.and_then(|plan| {
                        plan.native_semantic_identity(program.typed_program()).ok()
                    }),
                    native_identity: match authored_layout {
                        Some(plan) => native_policy_identity.as_deref().and_then(|policy| {
                            plan.native_data_identity(program.typed_program(), &data, policy)
                                .ok()
                        }),
                        None => program
                            .typed_program()
                            .unique_data_definition_by_symbol(data.symbol)
                            .and_then(|declaration| {
                                program
                                    .typed_program()
                                    .native_data_declaration_identity(declaration)
                                    .ok()
                            }),
                    },
                    scalar_restriction: None,
                    owns_linear_claim,
                    no_code_affine_cleanup,
                    copyable,
                    descriptor_cleanup,
                    identity,
                    shape: CheckedUnitStructuralTypeShape::Record {
                        fields,
                        storage: record_storage,
                    },
                },
            );
            self.in_progress.remove(&identity);
            Some(identity)
        }

        fn structural_field_plan(
            &mut self,
            field: &DataField,
            binders: &[(SymbolHandle, String)],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            owner_identity: TypeIdentityId,
            stated_bounds: Option<(Option<i64>, Option<i64>)>,
        ) -> Option<CheckedUnitStructuralFieldPlan> {
            let field_type_reference =
                substituted_formal_type(self.program, field.type_reference, substitutions);
            let receiving_type_identity = self.program.type_identity(TypeIdentityRequest {
                binders,
                substitutions,
                ..TypeIdentityRequest::ordinary(field_type_reference)
            });
            if !receiving_type_identity.is_valid() {
                return None;
            }
            let mut byte_sequence_type_identity = Default::default();
            let field_type = if field.relevance.is_erased() {
                CheckedUnitStructuralFieldType::Erased {
                    type_identity: receiving_type_identity,
                }
            } else if let Some(carrier) =
                byte_sequence_carrier(self.program, field_type_reference, substitutions).filter(
                    |_| !stores_reference_shape(self.program, field_type_reference, substitutions),
                )
            {
                byte_sequence_type_identity =
                    self.add_stored_type(field_type_reference, binders, substitutions)?;
                let declared = type_reference_domain_predicates(self.program, field_type_reference);
                let mut predicates = ByteSequencePredicateSet::EMPTY;
                for (_, predicate) in declared {
                    predicates.insert(predicate?);
                }
                if predicates.is_empty() {
                    CheckedUnitStructuralFieldType::ByteSequence(carrier)
                } else {
                    CheckedUnitStructuralFieldType::PredicateByteSequence {
                        carrier,
                        predicates,
                    }
                }
            } else if is_reference(self.program, field_type_reference) {
                CheckedUnitStructuralFieldType::Structural {
                    type_identity: self.add_stored_type(
                        field_type_reference,
                        binders,
                        substitutions,
                    )?,
                }
            } else {
                match scalar_type(self.program, field_type_reference, substitutions) {
                    Some(primitive) => scalar_fields::retain_scalar_field(
                        self.program,
                        field_type_reference,
                        substitutions,
                        primitive,
                        stated_bounds,
                    )?,
                    None => {
                        let nested =
                            self.add_stored_type(field_type_reference, binders, substitutions)?;
                        if nested == owner_identity {
                            return None;
                        }
                        CheckedUnitStructuralFieldType::Structural {
                            type_identity: nested,
                        }
                    }
                }
            };
            let mut qualification_carriers = Vec::new();
            let scalar_carrier_identity = if !field.relevance.is_erased()
                && matches!(
                    field_type,
                    CheckedUnitStructuralFieldType::Scalar(_)
                        | CheckedUnitStructuralFieldType::BoundedInteger(_)
                        | CheckedUnitStructuralFieldType::RefinedScalar { .. }
                ) {
                self.add_type(field_type_reference, binders, substitutions)?
            } else {
                Default::default()
            };
            if !field.relevance.is_erased()
                && matches!(
                    field_type,
                    CheckedUnitStructuralFieldType::Structural { .. }
                        | CheckedUnitStructuralFieldType::BoundedInteger(_)
                        | CheckedUnitStructuralFieldType::RefinedScalar { .. }
                        | CheckedUnitStructuralFieldType::ByteSequence(_)
                        | CheckedUnitStructuralFieldType::PredicateByteSequence { .. }
                )
            {
                let carriers = super::signature_contracts::qualification_carriers(
                    self.program,
                    self,
                    field_type_reference,
                    binders,
                )?;
                if !matches!(
                    field_type,
                    CheckedUnitStructuralFieldType::Structural { .. }
                ) {
                    for (_, carrier) in carriers {
                        crate::shared::collections::push_unique(
                            &mut qualification_carriers,
                            carrier,
                        );
                    }
                }
            }
            Some(CheckedUnitStructuralFieldPlan {
                source_declaration: field.symbol,
                scalar_carrier_identity,
                native_identity: self
                    .program
                    .typed_program()
                    .normalized_hermetic_symbol_identity(field.symbol)
                    .ok(),
                type_reference: field_type_reference,
                receiving_type_identity,
                identity: CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                relevance: field.relevance,
                field_type,
                byte_sequence_type_identity,
                storage: None,
                qualification_carriers,
            })
        }

        pub(in super::super) fn retain_transitive(&mut self, roots: &BTreeSet<TypeIdentityId>) {
            let mut retained = roots.iter().copied().collect::<BTreeSet<_>>();
            loop {
                let old_len = retained.len();
                for identity in retained.clone() {
                    if let Some(representation) = self
                        .opaque_representations
                        .iter()
                        .find(|row| row.opaque == identity)
                    {
                        retained.insert(representation.carrier);
                    }
                    let Some(plan) = self.types.get(&identity) else {
                        continue;
                    };
                    if plan.descriptor_presentation.is_valid() {
                        retained.insert(plan.descriptor_presentation);
                    }
                    match &plan.shape {
                        CheckedUnitStructuralTypeShape::Reference {
                            referent_identity, ..
                        } => {
                            retained.insert(*referent_identity);
                        }
                        CheckedUnitStructuralTypeShape::ByteSequence(_) => {
                            retained.extend(self.byte_view_elements.iter().cloned());
                        }
                        CheckedUnitStructuralTypeShape::Scalar(_)
                        | CheckedUnitStructuralTypeShape::AtomicCell(_) => {}
                        CheckedUnitStructuralTypeShape::Record { fields, storage } => {
                            if let CheckedUnitRecordStorage::TargetLayout {
                                schema_type_identity,
                                ..
                            }
                            | CheckedUnitRecordStorage::AccessView {
                                schema_type_identity,
                                ..
                            } = storage
                            {
                                retained.insert(*schema_type_identity);
                            }
                            retained.extend(fields.iter().flat_map(
                                CheckedUnitStructuralFieldPlan::retained_type_identities,
                            ));
                        }
                        CheckedUnitStructuralTypeShape::FixedArray {
                            element_type_identity,
                            ..
                        }
                        | CheckedUnitStructuralTypeShape::BorrowedSliceView {
                            element_type_identity,
                            ..
                        } => {
                            retained.insert(*element_type_identity);
                        }
                        CheckedUnitStructuralTypeShape::Sum { cases } => {
                            retained.extend(cases.iter().flat_map(|case| &case.fields).flat_map(
                                CheckedUnitStructuralFieldPlan::retained_type_identities,
                            ));
                        }
                        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                            retained.extend(
                                fields
                                    .iter()
                                    .chain(cases.iter().flat_map(|case| &case.fields))
                                    .flat_map(
                                        CheckedUnitStructuralFieldPlan::retained_type_identities,
                                    ),
                            );
                        }
                    }
                }
                if retained.len() == old_len {
                    break;
                }
            }
            self.completed_requests = Default::default();
            self.types.retain(|identity, _| retained.contains(identity));
            self.domains
                .retain(|domain| retained.contains(&domain.carrier_type_identity));
        }
    }

    /* Join the admitted plan to the exact synthesized field, not its spelling. */
    fn authored_field_storage(
        layout: &PlanLaidLayout,
        symbol: SymbolHandle,
        field_type: &CheckedUnitStructuralFieldType,
    ) -> Option<semantic_vocabulary::AuthoredFieldStorage> {
        let field_index = layout
            .field_symbols
            .iter()
            .position(|candidate| *candidate == symbol)?;
        let scalar_field = matches!(
            field_type,
            CheckedUnitStructuralFieldType::Scalar(_)
                | CheckedUnitStructuralFieldType::Atomic(_)
                | CheckedUnitStructuralFieldType::AtomicBoundedInteger(_)
                | CheckedUnitStructuralFieldType::BoundedInteger(_)
                | CheckedUnitStructuralFieldType::RefinedScalar { .. }
        );
        layout
            .invariant_geometry()?
            .authored_field_storage(field_index, scalar_field)
    }
}
mod signature_contracts {
    /*
        Signature contracts and domain qualifications a checked structural parameter
        or result carries, and the domain requirements boundary machines publish.
    */
    use super::*;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::calls::projected_argument_path;
    use crate::flow::canonical_place_from_semantic_place;
    use crate::published::places::place_at;
    use crate::shared::collections::push_unique;

    /*
        Invocation clauses retain their own represented erased lanes. Their
        presence is independent of a scalar body's result operation. Internal
        state hypotheses are not invocation inputs and remain under the separate
        state/arrival owner; this ordinary body cannot promote their bindings.
    */
    pub(in crate::execution::terminal_unit) fn checked_machine_signature_has_plan(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> bool {
        if program.machine_contracts(machine).len() != machine.contracts.len()
            || program.state_contracts(state).len() != state.contracts.len()
        {
            return false;
        }
        let ordinary_kind = |contract: &typed_trees::signature::SignatureContract| {
            matches!(
                contract.kind,
                SignatureContractKind::Crashes { .. }
                    | SignatureContractKind::Requires
                    | SignatureContractKind::Ensures
            )
        };
        program.machine_contracts(machine).iter().all(|contract| {
            if !ordinary_kind(contract) {
                return false;
            }
            if contract.binding.is_none() {
                return true;
            }
            let declared = program.proof_facts.span_or_empty(contract.facts);
            if declared.is_empty() || declared.len() != contract.facts.len() {
                return false;
            }
            contract.facts.handles().all(|fact| {
                let recorded = match contract.kind {
                    SignatureContractKind::Requires => facts.proof.machine_requires_fact(
                        program.typed_program(),
                        machine.symbol,
                        fact,
                    ),
                    SignatureContractKind::Ensures => facts.proof.machine_ensures_fact(
                        program.typed_program(),
                        machine.symbol,
                        fact,
                    ),
                    _ => None,
                };
                recorded.is_some_and(|row| row.evidence_term.is_some())
            })
        }) && program
            .state_contracts(state)
            .iter()
            .all(|contract| ordinary_kind(contract) && contract.binding.is_none())
    }

    pub(in crate::execution::terminal_unit) fn checked_state_contracts_have_plan(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
    ) -> bool {
        let Some(plan) = facts.contract_plans.for_machine(machine) else {
            return false;
        };
        program.state_contracts(state).iter().all(|contract| {
            if contract.kind == SignatureContractKind::Requires {
                return contract.binding.is_none()
                    && plan.crash.structural_runtime_requirements().is_some();
            }
            checked_structural_signature_has_plan(program, state, structural_parameters, contract)
        })
    }

    pub(in crate::execution::terminal_unit) fn checked_structural_signature_has_plan(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        contract: &typed_trees::signature::SignatureContract,
    ) -> bool {
        let source_parameters = program.state_parameters(state);
        program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .all(|fact| match (&contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Membership(membership)) => {
                    let Some(place) = place_at(program, state.symbol, 0, membership.value) else {
                        return false;
                    };
                    if let Some(valid) =
                        signature_progress_requirement(program, source_parameters, membership)
                    {
                        return valid;
                    }
                    if !place.segments.is_empty() {
                        return false;
                    }
                    let Some(root) = place.root.as_symbol() else {
                        return false;
                    };
                    let Some(position) = source_parameters
                        .iter()
                        .position(|parameter| parameter_is_root(parameter, root))
                    else {
                        return false;
                    };
                    let Some(domain) = type_domain_semantic_id(
                        program,
                        source_parameters[position].type_reference,
                        membership.domain_symbol,
                    ) else {
                        return false;
                    };
                    structural_parameters.iter().any(|parameter| {
                        parameter.position as usize == position
                            && parameter.qualifications.contains(&domain)
                    })
                }
                (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => matches!(
                    program.expression_table.expression(*expression),
                    typed_trees::expression::ExpressionNode::Boolean(true)
                ),
                _ => false,
            })
    }

    /*
        Progress profiles retain exact checked termination subjects and supply no
        runtime predicates. Structural signature publication independently retains
        an authored formal's existing membership, without introducing an issuer or
        deriving a scalar predicate. Arbitrary projected memberships still refuse.
        Canonical termination.md 6a14afff75d92a16d3e6a0adfeee18f0fbbe8727:
        "Progress evidence follows exact identity-preserving lineage, explicit contract
        substitution, or checked qualification-preserving transitions."
    */
    pub(in crate::execution::terminal_unit) fn signature_progress_requirement(
        program: &dyn ::validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        membership: &typed_trees::domain::ProofMembershipFact,
    ) -> Option<bool> {
        if !program.domain_definitions().iter().any(|domain| {
            domain.symbol == membership.domain_symbol
                && domain.classification
                    == Some(language_semantics::DomainClassification::ProgressProfile)
        }) {
            return None;
        }
        Some(
            validation::canonical_place_from_expression(program, membership.value).is_some_and(
                |place| {
                    place.root.as_symbol().is_some_and(|root| {
                        parameters
                            .iter()
                            .any(|parameter| parameter_is_root(parameter, root))
                    }) && place
                        .segments
                        .iter()
                        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
                },
            ),
        )
    }

    pub(in crate::execution::terminal_unit) fn parameter_qualifications(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<Vec<SemanticDomainId>> {
        let mut output = qualification_carriers(program, shapes, type_reference, binders)?
            .into_iter()
            .map(|(domain, _)| domain)
            .collect::<Vec<_>>();
        output.sort_by_key(|domain| domain.0);
        output.dedup();
        Some(output)
    }

    /*
        Register each domain qualification on `type_reference` and pair it with
        the type identity of the carrier it was registered against, in authored
        order.
    */
    pub(in crate::execution::terminal_unit) fn qualification_carriers(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        mut type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<Vec<(SemanticDomainId, typed_trees::type_identity::TypeIdentityId)>> {
        let borrowed_view = matches!(
            byte_sequence_carrier(program, type_reference, &[]),
            Some(
                checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
            )
        )
        .then_some(type_reference);
        let mut output = Vec::new();
        let mut represented_carrier = None;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        let domain = match constraint {
                            TypeConstraintNode::ArithmeticDomain(_) => continue,

                            TypeConstraintNode::Domain(domain)
                                if matches!(
                                    domain.subject,
                                    typed_trees::types::DomainConstraintSubject::Carry(_)
                                ) =>
                            {
                                continue;
                            }
                            TypeConstraintNode::Domain(domain) => domain,
                            _ => return None,
                        };
                        if !domain.semantic_id.is_valid() {
                            return None;
                        }
                        let carrier = if matches!(
                            byte_sequence_carrier(program, type_reference, &[]),
                            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
                        ) || (program
                            .primitive_type_reference(type_reference)
                            .is_some()
                            && validation::structural_identity_type(program, type_reference, &[])
                                == type_reference)
                        {
                            type_reference
                        } else {
                            represented_carrier.or(borrowed_view).unwrap_or(*base_type)
                        };
                        let carrier_identity =
                            shapes.add_domain_application(domain, carrier, binders, false)?;
                        output.push((domain.semantic_id, carrier_identity));
                    }
                    type_reference = *base_type;
                }
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                _ => {
                    if let Some(storage) = program.layout_storage_arguments(type_reference) {
                        represented_carrier.get_or_insert(type_reference);
                        type_reference = storage.semantic_type;
                    } else {
                        break;
                    }
                }
            }
        }
        Some(output)
    }

    /*
        Retain a boundary's exact result-membership contract on its carrier. This
        is signature transport, not issuance: a crossing may forward existing
        custody, derive it by conservation, or introduce it through an authorized
        route. The call-result custody checker owns that distinction and provider
        installation retains its separate admission obligation. Requiring an
        issuance route here incorrectly rejected preserved input qualifications.
        Exact mutable-formal guarantees belong to the normal-parameter roster;
        never move their obligations onto a different returned value. Other
        non-result outputs and nonstructural carriers still need their own plans.
    */
    pub(in crate::execution::terminal_unit) fn fold_boundary_result_domains(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        signature: &typed_trees::signature::StateSignature,
        result_type: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        result: CheckedBoundaryMachineResultPlan,
    ) -> Option<CheckedBoundaryMachineResultPlan> {
        let mut guaranteed = Vec::new();
        for contract in program.state_signature_contracts(signature) {
            if contract.kind != SignatureContractKind::Ensures {
                continue;
            }
            for fact in contract.facts.handles() {
                let Some(membership) = program.proof_facts.get(fact).as_membership() else {
                    continue;
                };
                if validation::reserved_result_owner(program, membership.value)
                    .is_none_or(|(owner, _)| owner != signature.symbol)
                {
                    if normal_mutable_parameter_position(
                        program,
                        program.state_signature_parameters(signature),
                        membership.value,
                    )
                    .is_some()
                    {
                        continue;
                    }
                    return None;
                }
                let Some(domain) =
                    crate::published::facts::qualification_evidence::domain_definition(
                        program,
                        membership.domain_symbol,
                    )
                else {
                    continue;
                };
                guaranteed.push((membership.semantic_domain, domain.symbol));
            }
        }
        guaranteed
            .sort_by_key(|(domain, symbol)| (domain.0, symbol.arena_index(), symbol.generation()));
        guaranteed.dedup();
        if guaranteed.is_empty() {
            return Some(result);
        }
        let CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            mut qualifications,
            projected_qualifications,
        } = result
        else {
            return None;
        };
        for (domain, selected_symbol) in guaranteed {
            shapes.add_selected_domain(domain, selected_symbol, result_type, binders, false)?;
            push_unique(&mut qualifications, domain);
        }
        qualifications.sort_by_key(|domain| domain.0);
        Some(CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications,
        })
    }

    /*
        A normal guarantee can preserve an existing qualification on this exact
        mutable formal. Projected subjects and shared/value inputs have different
        owners and cannot borrow that restoration authority.
    */
    pub(super) fn normal_mutable_parameter_position(
        program: &dyn ::validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        value: typed_trees::expression::ExpressionHandle,
    ) -> Option<usize> {
        let root = validation::single_segment_name_symbol(program, value)?;
        parameters.iter().position(|parameter| {
            parameter.symbol == root
                && validation::reference_shell(program, parameter.type_reference).is_some_and(
                    |shell| shell.access == language_semantics::ReferenceAccess::Mutable,
                )
        })
    }

    /*
        Register the carrier an authored `ensures result in <domain>` clause
        qualifies on an ordinary or state-graph machine. The result declaration
        and every receiving call publish that membership as a qualification of
        the returned carrier, and the structural domain catalog can only lower a
        domain whose declaration this collector holds. A boundary requirement
        registers through `fold_boundary_result_domains`; the call and installation
        owners retain its authority obligations. A checked body proved its own clause. A membership
        over another value names a different carrier and is skipped, and a scalar,
        unit result carries no structural qualification. Borrowed record guarantees
        qualify their exact referent path; a domain on the shell retains that shell.
    */
    pub(in crate::execution::terminal_unit) fn register_authored_result_domains(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        let return_type = state.return_type;
        if is_unit(program, return_type) || program.primitive_type_reference(return_type).is_some()
        {
            return Some(());
        }
        for contract in program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
        {
            if contract.kind != SignatureContractKind::Ensures {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                let Some((owner, carrier)) =
                    validation::reserved_result_owner(program, membership.value)
                else {
                    continue;
                };
                if owner != machine.symbol && owner != state.symbol {
                    continue;
                }
                let (_, carrier) = validation::result_membership_carrier(
                    program,
                    carrier,
                    membership.domain_symbol,
                )?;
                let Some(definition) = program.domain_definition(membership.domain_symbol) else {
                    continue;
                };
                shapes.add_selected_domain(
                    membership.semantic_domain,
                    definition.symbol,
                    carrier,
                    binders,
                    true,
                )?;
            }
        }
        Some(())
    }

    /*
        Freeze each authored `requires <formal> in <domain>` entry membership on the
        structural formal it names. A bare annotation does not erase a requires
        membership: the scalar formal route publishes the same requirement on its
        declaration, and this is that rule's structural sibling. The body assumes
        the proposition its own entry clause states, while the caller's arrival still
        owes that domain's establishment route, which its checked call admitted.
        Registration describes the selected domain's theory; it establishes no
        membership and grants no custody. Resolve the state through its actual
        typed owner: a fused graph's root does not own a callee's entry clauses.
    */
    pub(in crate::execution::terminal_unit) fn fold_parameter_entry_domains(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: SymbolHandle,
        binders: &[(SymbolHandle, String)],
        structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    ) -> Option<()> {
        let (machine, state) = validation::find_state(program, state)?;
        let parameters = program.state_parameters(state);
        let entry_state = program
            .machine_states(machine)
            .first()
            .map(|state| state.symbol);
        let contracts = program.state_contracts(state).iter().chain(
            program
                .machine_contracts(machine)
                .iter()
                .filter(|_| entry_state == Some(state.symbol)),
        );
        for contract in contracts {
            if contract.kind != SignatureContractKind::Requires || contract.binding.is_some() {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if let Some(valid) = signature_progress_requirement(program, parameters, membership)
                {
                    if !valid {
                        return None;
                    }
                    continue;
                }
                let Some(root) = validation::single_segment_name_symbol(program, membership.value)
                else {
                    continue;
                };
                let Some(source_position) = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == root)
                else {
                    continue;
                };
                let Some(parameter) = structural_parameters
                    .iter_mut()
                    .find(|parameter| parameter.position as usize == source_position)
                else {
                    continue;
                };
                let routed = validation::field_domains::domain_requires_provenance(
                    program,
                    membership.domain_symbol,
                );
                let vacuous = validation::field_domains::domain_is_vacuous(
                    program,
                    membership.domain_symbol,
                    &mut Vec::new(),
                );
                if !routed && !vacuous {
                    continue;
                }
                let domain = crate::published::facts::qualification_evidence::domain_definition(
                    program,
                    membership.domain_symbol,
                )?;
                if !membership.semantic_domain.is_valid() {
                    return None;
                }
                shapes.add_selected_domain(
                    membership.semantic_domain,
                    domain.symbol,
                    parameters[source_position].type_reference,
                    binders,
                    false,
                )?;
                if !parameter
                    .qualifications
                    .contains(&membership.semantic_domain)
                {
                    parameter.qualifications.push(membership.semantic_domain);
                    parameter.qualifications.sort_by_key(|domain| domain.0);
                }
            }
        }
        Some(())
    }

    /*
        Internal state signatures retain the qualifications proved on every arrival,
        including facts not spelled in a parameter's nominal type. Copies and later
        projections consume this same contract. Never strengthen an invocation entry
        from the particular callers encountered while compiling its body.
    */
    pub(in crate::execution::terminal_unit) fn retain_parameter_arrival_domains(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    ) -> Result<(), &'static str> {
        if structural_parameters.is_empty()
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
        {
            return Ok(());
        }
        let flow = state_flow(facts, machine.symbol, state.symbol)
            .ok_or("state graph: arrival domains: missing checked flow")?;
        let parameters = program.state_parameters(state);
        let declared_qualifications = parameters
            .iter()
            .map(|parameter| {
                validation::field_domains::declared_type_domain_rows(
                    program,
                    parameter.type_reference,
                )
            })
            .collect::<Vec<_>>();
        for context in facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(flow.entry_semantic_contexts)
        {
            for fact in facts
                .semantic
                .context_view(facts.semantic.contexts.get(context.context))
                .facts()
            {
                if fact.origin != facts::FactOrigin::StatementTransfer {
                    continue;
                }
                let Some((_, _, domain_symbol, domain)) = fact.payload.domain_membership() else {
                    continue;
                };
                let place = fact.place.as_place().and_then(|place| {
                    canonical_place_from_semantic_place(
                        program,
                        &facts.semantic,
                        facts.semantic.places.get(place),
                    )
                });
                let Some(place) = place else {
                    continue;
                };
                let Some(position) = parameters
                    .iter()
                    .position(|parameter| place.root == facts::PlaceRoot::Symbol(parameter.symbol))
                else {
                    continue;
                };
                if declared_qualifications[position].iter().any(|declared| {
                    declared.describes_projection(&place.segments, |expression| {
                        expression.is_valid()
                            && !matches!(
                                program.expression_table.expression(expression),
                                typed_trees::expression::ExpressionNode::Range(_)
                            )
                    }) && declared.domain_symbol == domain_symbol
                        && declared.semantic_domain == domain
                }) {
                    continue;
                }
                let Some(parameter) = structural_parameters
                    .iter_mut()
                    .find(|parameter| parameter.position as usize == position)
                else {
                    continue;
                };
                let ProjectedArgumentPath {
                    storage_type: carrier,
                    path,
                    ..
                } = projected_argument_path(program, state.symbol, 0, &place, None, None)
                    .ok_or("state graph: arrival domains: unsupported exact subject path")?;
                if program.primitive_type_reference(carrier).is_some() {
                    continue;
                }
                if path.is_empty() && parameter.qualifications.contains(&domain)
                    || parameter
                        .projected_qualifications
                        .iter()
                        .any(|row| row.path == path && row.domain == domain)
                {
                    continue;
                }
                shapes
                    .add_selected_domain(domain, domain_symbol, carrier, &[], false)
                    .ok_or("state graph: arrival domains: selected theory has no carrier plan")?;
                if path.is_empty() {
                    parameter.qualifications.push(domain);
                    parameter.qualifications.sort_by_key(|domain| domain.0);
                } else {
                    parameter
                        .projected_qualifications
                        .push(CheckedStructuralPathQualification { path, domain });
                }
            }
        }
        Ok(())
    }

    fn cast_is_borrowed_in_statement(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        cast_expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        let Some(state_node) = program.state_by_symbol(state) else {
            return false;
        };
        let Some(statement) =
            crate::flow::state_statements(program, state_node.statement_nodes).get(statement_index)
        else {
            return false;
        };
        let mut is_borrowed = false;
        ::validation::visit_statement_value_occurrences(
            program,
            machine,
            state,
            statement_index,
            statement,
            &mut |expr, _origin, _primitive| {
                if is_borrowed {
                    return;
                }
                if let Some(borrow) = program.expression_table.borrow_expression(expr) {
                    let mut target = borrow.target;
                    while target.is_valid() {
                        if target == cast_expression {
                            is_borrowed = true;
                            break;
                        }
                        if let Some(inner_cast) = program.expression_table.cast_node(target) {
                            if inner_cast.form.is_recast() {
                                break;
                            }
                            target = inner_cast.value;
                        } else {
                            break;
                        }
                    }
                }
            },
        );
        is_borrowed
    }

    /*
        Body commitments whose theory belongs to structural carriers. Classify
        normalized domain instances by their carrier, not by their predicates or
        the arrangement of an authored cast. Scalar computations retain scalar
        membership and its proof; a structural request would duplicate that owner.
        This query assigns ownership only and establishes no membership evidence.
    */
    pub(in crate::execution::terminal_unit) fn body_structural_qualifications(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
    ) -> Vec<SemanticDomainId> {
        let mut domains = facts
            .qualifications
            .for_machine(machine)
            .map(|fact| {
                fact.body_committed
                    .iter()
                    .copied()
                    .filter(|domain| {
                        !matches!(
                            *domain,
                            language_semantics::SemanticDomainTable::WRAPPING
                                | language_semantics::SemanticDomainTable::SATURATING
                                | language_semantics::SemanticDomainTable::TRAPPING
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if domains.is_empty() {
            return domains;
        }
        let scalar_domains = program
            .type_reference_table
            .constrained_type_references()
            .into_iter()
            .filter(|(carrier, _)| program.primitive_type_reference(*carrier).is_some())
            .flat_map(|(_, constraints)| program.type_reference_table.constraints(constraints))
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Domain(domain) => Some(domain.semantic_id.0),
                _ => None,
            })
            .chain(program.domain_definitions().iter().filter_map(|domain| {
                program
                    .primitive_type_reference(domain.target_type)
                    .map(|_| domain.semantic_id.0)
            }))
            .collect::<BTreeSet<_>>();
        domains.retain(|domain| !scalar_domains.contains(&domain.0));
        let decided = facts
            .qualifications
            .vacuous_uses
            .iter()
            .map(|use_| {
                (
                    use_.machine,
                    use_.state,
                    use_.statement_index,
                    use_.expression,
                    use_.semantic_domain,
                )
            })
            .chain(
                facts
                    .qualifications
                    .value_formations
                    .iter()
                    .map(|(_, decision)| {
                        (
                            decision.machine,
                            decision.state,
                            decision.statement_index,
                            decision.expression,
                            decision.semantic_domain,
                        )
                    }),
            )
            .filter(|(owner, ..)| *owner == machine)
            .filter(|(_, _, _, expression, _)| {
                program.expression_table.cast_node(*expression).is_some()
            });
        let (owned, unowned): (Vec<_>, Vec<_>) =
            decided.partition(|(_, state, statement, expression, _)| {
                facts
                    .values
                    .structural_values
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.machine == machine
                            && node.state == *state
                            && node.statement_index == *statement
                            && node.expression == *expression
                    })
                    || cast_is_borrowed_in_statement(
                        program,
                        machine,
                        *state,
                        *statement as usize,
                        *expression,
                    )
            });
        domains.retain(|domain| {
            !owned
                .iter()
                .any(|(.., semantic_domain)| semantic_domain == domain)
                || unowned
                    .iter()
                    .any(|(.., semantic_domain)| semantic_domain == domain)
        });
        domains.sort_by_key(|domain| domain.0);
        domains.dedup();
        domains
    }

    /*
        Collect domain constraints below a structural root without treating them
        as root qualifications. Fixed-array indexes are the canonical source path
        for each occurrence; unsupported dynamic shapes remain absent from plans.
    */
    pub(in crate::execution::terminal_unit) fn projected_parameter_qualifications(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<Vec<CheckedStructuralPathQualification>> {
        let contract =
            validation::structural_qualification_contract(program, type_reference).ok()?;
        for reference in contract.projection_types {
            shapes.add_type(reference, binders, &[])?;
        }
        for (domain, carrier) in contract.domains {
            shapes.add_domain(domain, carrier, binders)?;
        }
        Some(contract.projected_qualifications)
    }

    /*
        Returned values retain reference carriers; their projected memberships
        use the same explicit pointer paths as the emitted result declaration.
    */
    pub(in crate::execution::terminal_unit) fn projected_result_qualifications(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<Vec<CheckedStructuralPathQualification>> {
        let contract =
            validation::structural_result_qualification_contract(program, type_reference).ok()?;
        for reference in contract.projection_types {
            shapes.add_type(reference, binders, &[])?;
        }
        for (domain, carrier) in contract.domains {
            shapes.add_domain(domain, carrier, binders)?;
        }
        Some(contract.projected_qualifications)
    }

    pub(super) fn type_domain_semantic_id(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        domain: SymbolHandle,
    ) -> Option<SemanticDomainId> {
        fn collect(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
            domain: SymbolHandle,
            matches: &mut Vec<SemanticDomainId>,
        ) {
            if !type_reference.is_valid() {
                return;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    collect(program, *referee, domain, matches);
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    collect(program, *base_type, domain, matches);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        let TypeConstraintNode::Domain(candidate) = constraint else {
                            continue;
                        };
                        if candidate.symbol == domain
                            && candidate.semantic_id.is_valid()
                            && !matches.contains(&candidate.semantic_id)
                        {
                            matches.push(candidate.semantic_id);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut matches = Vec::new();
        collect(program, type_reference, domain, &mut matches);
        match matches.as_slice() {
            [semantic_id] => Some(*semantic_id),
            [] => program
                .domain_definition(domain)
                .filter(|definition| definition.index_arguments.is_empty())
                .map(|definition| definition.semantic_id)
                .filter(|semantic_id| semantic_id.is_valid()),
            _ => None,
        }
    }
}
pub(super) use boundary_domains::boundary_domain_requirements;
mod views {
    /*
        Borrowed views and byte-sequence carriers: which borrow shells present as a
        slice or stored reference, and the carrier each byte view realizes as.
    */
    use super::*;

    /*
        A borrowed `[T]` element view: one reference shell over a non-byte slice.
        Fixed arrays and vectors own contiguous storage and a slice borrows it,
        carrying its own extent as a stored runtime length rather than a declared
        constant, so the view's shape names only its element. Arithmetic-domain
        and other constraint shells qualify the carrier without changing it. The
        byte element keeps its established `ByteSequence` carrier, whose length,
        read and subslice obligations are already reconstructed. The shell's
        access is reported, not required: a shared `&[T]` and an exclusive
        `&mut [T]` view share this carrier, and each consumer admits the loan
        its own custody needs.
    */
    pub(crate) fn element_view_shell(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<validation::SliceViewShell> {
        validation::slice_view_shell(program, type_reference, substitutions).filter(|view| {
            program.primitive_type_reference(view.element) != Some(PrimitiveType::U8)
        })
    }

    /* The element of a borrowed `[T]` element view under any access. */
    pub(crate) fn borrowed_slice_view_element(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<TypeReferenceHandle> {
        element_view_shell(program, type_reference, substitutions).map(|view| view.element)
    }

    /*
        A `&[T]` declared result type, either view family. The byte element's
        `BorrowedView` carrier and the non-byte element's identity share the same
        reference-peeling shape. This shared-only query serves immutable view routes.
    */
    pub(crate) fn borrowed_slice_view(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        structural_access_for_type_reference(program, type_reference)
            == Some(CheckedStructuralAccess::SharedBorrow)
            && slice_view_carrier(program, type_reference)
    }

    /*
        A readable borrowed slice result transports its descriptor and preserves
        the source access. Shared custody copies the descriptor; mutable custody is
        affine. A write-only result keeps its separate, unsupported result contract.
    */
    pub(crate) fn borrowed_slice_result(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        validation::borrowed_slice_view_result(program, type_reference)
    }

    /*
        A `&[T]` or `&[u8]` descriptor carrier under either loan access; the
        reference shell's access is the loan's, not the carrier's.
    */
    pub(crate) fn slice_view_carrier(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        borrowed_slice_view_element(program, type_reference, &[]).is_some()
            || matches!(
                byte_sequence_carrier(program, type_reference, &[]),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            )
    }

    /*
        A shared stored-reference result, independent of its referent family.
        The referent's storage
        lives in the caller's frame, so the owned-contents custody clause does not
        apply. `&mut`/`&write` named results stay with their own custody families.
    */
    pub(crate) fn borrowed_named_view(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        validation::reference_result_custody::stored_loan_parts(program, type_reference)
            .is_some_and(|(_, access)| access == CheckedStructuralAccess::SharedBorrow)
    }

    pub(crate) use validation::byte_sequence_carrier;
}

pub(super) use affine_discards::*;
pub(crate) use identities::*;
pub(crate) use parameters::*;
pub(crate) use shape_collector::*;
pub(super) use signature_contracts::*;
pub(crate) use views::*;
