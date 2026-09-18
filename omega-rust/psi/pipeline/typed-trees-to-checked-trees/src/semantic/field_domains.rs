//! Declared default-domain fields become entry facts at exact storage places.
//! Ordinary mutations invalidate these facts; a nominal annotation alone does
//! not restore them. Call and state-arrival checks require the same field
//! obligations over the current incoming values before reusing an entry fact.
//!
//! Machine storage additionally requires its ZII value to satisfy the domain.
//! Nominal input storage instead relies on the checked incoming argument.
//! Write-only input views supply no readable entry facts. Independent fields
//! use separate contexts so invalidation does not erase unrelated evidence.
//!
//! Collection elements carry their declared fields at TWO coordinates: every
//! literal element at its exact `FixedIndex`, and one whole-extent row at the
//! `0..usize::MAX` window that any element index narrows from. The window row
//! is the only elementwise spelling a slice can have (no static extent exists
//! to enumerate); it is still exact evidence -- a write to any element
//! overlaps the containing window and retires it, so an unresolved index is
//! never itself a fact place and never universal coverage. Where per-element
//! rows exist (fixed arrays, machine storage) the window row is seeded under
//! `StatementTransfer` as evidence only -- the element rows already spell the
//! complete call-boundary obligation. For a slice the window row is the ONLY
//! elementwise claim, so it rides the obligation origin and every caller owes
//! the whole extent.

use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

use crate::facts::field_domain::{
    readable_fixed_array_elements, readable_nominal_definition, readable_type_reference,
};

/// The `FixedRange` spelling of "every element of this collection": the
/// half-open extent `0..usize::MAX` covers any element index a place can
/// select -- a literal `x[i]` because `i < usize::MAX` always, and a runtime
/// `x[index]` because whichever element the index names sits inside the
/// extent. An elementwise fact at this segment is one row of evidence for
/// every element; a narrower or unresolved index never mints one, and a write
/// to any element retires it through the usual overlap machinery
/// (`flow/place/comparison.rs` already overlaps `Index`/`FixedIndex` with a
/// containing `FixedRange`). The same sentinel is re-derived locally at every
/// consumer (`flow/transfers/projected.rs`, `checks/contracts/prover.rs`) so
/// no private module boundary is crossed.
const WHOLE_ELEMENT_EXTENT: PlaceSegment = PlaceSegment::FixedRange {
    start: 0,
    end: usize::MAX,
};

pub(super) fn append_machine_field_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        let Some(self_symbol) = machine_self_parameter_symbol(program, machine) else {
            continue;
        };
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == attached.as_str())
        else {
            continue;
        };

        let mut refs = arena::HandleSpan::empty();
        append_data_field_domain_facts(
            program,
            facts,
            self_symbol,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            FactOrigin::MachineFieldDomain {
                machine_symbol: machine.symbol,
            },
            QualificationEvidence::from_origin(
                language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                machine.symbol,
            ),
            data,
            &[],
            &[data.name.as_str()],
            &mut refs,
        );

        if refs.is_empty() {
            continue;
        }
        append_independent_place_contexts(
            facts,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            refs,
        );
    }
}

/// Seed the entry-invariant `self.a.b…c in Domain` fact for every domained field
/// reachable through the attached data -- ONE level or NESTED. `prefix` is the
/// place segment chain from `self` to `data`; `visited` is the data-type names
/// on the current path (a cycle guard for self-referential data). Mirrors the
/// nested resolution in `field_domain::attached_data_field_type`: the read trust
/// seeded here is sound because every write to such a field (one-level or nested)
/// is domain-enforced through the SAME multi-level resolver, and each field is
/// gated on its ZII/empty value satisfying the domain.
///
/// Fixed-array fields additionally enumerate every element at its exact
/// `FixedIndex` place: `self.rows[i].label` is live evidence for the single
/// element `i`, invalidated only by writes overlapping that index. Collection
/// fields (fixed array or slice) also carry the whole-extent `0..usize::MAX`
/// row `self.rows[*].label`, the only elementwise spelling that survives a
/// runtime selector or a slice view. Coverage is never encoded at an
/// unresolved `Index` -- a runtime selector cannot be a fact place.
///
/// `point`, `origin`, and `evidence` describe WHO is seeding: machine entry
/// invariants use `ProgramPoint::Machine`/`MachineFieldDomain`, while
/// zero-initialized locals reuse the same walker under
/// `ProgramPoint::State`/`StatementTransfer` (their binding statement's
/// zeroed storage is the evidence -- see `append_local_zii_field_domain_facts`).
#[allow(clippy::too_many_arguments)]
fn append_data_field_domain_facts(
    program: &TypedTrees,
    facts: &mut FactPlan,
    root_symbol: symbols::SymbolHandle,
    point: ProgramPoint,
    origin: FactOrigin,
    evidence: QualificationEvidence,
    data: &typed_trees::data::DataDefinition,
    prefix: &[PlaceSegment],
    visited: &[&str],
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };

        let mut field_path = prefix.to_vec();
        crate::flow::push_field_place_segments(program, &mut field_path, field.symbol);

        // SOUNDNESS GATE (per field, one-level or nested): surface the
        // entry-invariant only when the field's ZERO/ZII value provably satisfies
        // the domain -- a read-with-no-prior-write discharges against it, sound
        // only if the empty/default is in-domain. A domain the empty value
        // violates (e.g. `non_empty`) is withheld; its reads must follow a write.
        for domain_symbol in field_domain_symbols(program, field.type_reference)
            .into_iter()
            .filter(|symbol| {
                crate::facts::field_domain::domain_admits_empty_byte_sequence(program, *symbol)
            })
        {
            // Place `<root>.<prefix…>.<field>`: root the owning symbol (`self`
            // for machine storage, the local for ZII locals) + the segment
            // chain, so the canonical label matches a `root.a.b` read exactly
            // where the nested write established it.
            let place = facts.append_symbol_place(root_symbol);
            for segment in &field_path {
                facts.push_place_segment(place, *segment);
            }

            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin,
                evidence,
                payload: FactPayload::DomainMembership {
                    value: typed_trees::expression::ExpressionHandle::invalid(),
                    domain: arena::HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain: language_semantics::SemanticDomainId::NULL,
                },
            });
            facts.append_ref(refs, fact);
        }

        // Descend into a struct-typed field so its own domained fields are seeded
        // too. The cycle guard keeps a self-referential data type from looping.
        if let Some(nested) = crate::facts::field_domain::data_definition_for_field_type(
            program,
            field.type_reference,
        ) && !visited.contains(&nested.name.as_str())
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.name.as_str());
            append_data_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &field_path,
                &next_visited,
                refs,
            );
        }

        // A slice-typed field's elements carry their declared fields through
        // the whole-extent row alone: no static extent exists to enumerate.
        if let Some(element_type) = readable_slice_element(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.name.as_str())
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.name.as_str());
            let mut element_path = field_path.clone();
            element_path.push(WHOLE_ELEMENT_EXTENT);
            append_data_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }

        // A fixed array of nominal elements carries each element's declared
        // fields at `field[i]`; the same ZII gate applies to the leaf domains.
        // The whole-extent `field[0..usize::MAX]` row is seeded alongside so a
        // runtime selector or a slice view can narrow coverage from it. It is
        // EVIDENCE ONLY (`StatementTransfer`): the per-element rows already
        // spell the complete obligation, so collecting the rollup row as a
        // requirement would only double-report every corrupted element.
        let Some((element_type, length)) =
            readable_fixed_array_elements(program, field.type_reference)
        else {
            continue;
        };
        let Some(nested) = readable_nominal_definition(program, element_type) else {
            continue;
        };
        if visited.contains(&nested.name.as_str()) {
            continue;
        }
        let mut next_visited = visited.to_vec();
        next_visited.push(nested.name.as_str());
        for index in 0..length {
            let mut element_path = field_path.clone();
            element_path.push(PlaceSegment::FixedIndex { index });
            append_data_field_domain_facts(
                program,
                facts,
                root_symbol,
                point,
                origin,
                evidence,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }
        let mut element_path = field_path.clone();
        element_path.push(WHOLE_ELEMENT_EXTENT);
        append_data_field_domain_facts(
            program,
            facts,
            root_symbol,
            point,
            FactOrigin::StatementTransfer,
            evidence,
            nested,
            &element_path,
            &next_visited,
            refs,
        );
    }
}

/// #66/P1a param entry-assumption: an IMMUTABLE state parameter declared with a
/// domain qualification carries that domain throughout the state body. The
/// param's implicit `requires param in Domain` makes every caller establish
/// membership (by proof for a bodyful predicate, by retained evidence for a
/// bodyless qualification), and immutability means it can never be reassigned
/// out of the domain. Surfaced at `ProgramPoint::State` (the param belongs to the state),
/// which `build_state_flow_fact` folds into the state entry; the flow's context
/// threading then carries it to every call, including guarded-transition
/// fallthrough arms (now that a transition no longer leaks its branch-taken exit
/// context onto its sibling fallthrough -- see flow/statements.rs).
///
/// Nominal parameters additionally carry their declared field predicates on
/// entry, including readable mutable references: calls and transitions are
/// default-domain consumption points. These are live entry facts, not facts
/// restored after arbitrary writes. Ordinary storage invalidation retires them.
/// Write-only views cannot inspect the incoming value and receive no such facts.
pub(super) fn append_state_parameter_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for parameter in program.state_parameters(state) {
                if parameter.is_self {
                    continue;
                }
                let obligation_origin = FactOrigin::StateParameterDomain {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                };
                if let Some(data) = readable_nominal_definition(program, parameter.type_reference) {
                    append_state_parameter_data_field_domain_facts(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        obligation_origin,
                        data,
                        &[],
                        &[data.symbol],
                        &mut refs,
                    );
                }
                // Fixed-array collection parameters carry each element's declared
                // fields at the exact `FixedIndex` place. Every index is seeded
                // explicitly; coverage is never encoded as an unresolved `Index`.
                // The whole-extent `0..usize::MAX` row is seeded alongside so a
                // runtime `x[index]` subject or an `as_slice()` view can narrow
                // coverage from it; it is EVIDENCE ONLY (`StatementTransfer`)
                // because the per-element rows already spell the complete
                // call-boundary obligation.
                if let Some((element_type, length)) =
                    readable_fixed_array_elements(program, parameter.type_reference)
                    && let Some(data) = readable_nominal_definition(program, element_type)
                {
                    for index in 0..length {
                        let prefix = [PlaceSegment::FixedIndex { index }];
                        append_state_parameter_data_field_domain_facts(
                            program,
                            facts,
                            machine.symbol,
                            state.symbol,
                            parameter.symbol,
                            obligation_origin,
                            data,
                            &prefix,
                            &[data.symbol],
                            &mut refs,
                        );
                    }
                    append_state_parameter_data_field_domain_facts(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        FactOrigin::StatementTransfer,
                        data,
                        &[WHOLE_ELEMENT_EXTENT],
                        &[data.symbol],
                        &mut refs,
                    );
                }
                // Slice parameters have no static extent to enumerate; the
                // whole-extent row is the ONLY elementwise spelling, so it must
                // stay an obligation: the caller owes every element's declared
                // fields, which is exactly what `rooms[index]` subjects narrow
                // coverage from inside the callee.
                if let Some(element_type) =
                    readable_slice_element(program, parameter.type_reference)
                    && let Some(data) = readable_nominal_definition(program, element_type)
                {
                    append_state_parameter_data_field_domain_facts(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        obligation_origin,
                        data,
                        &[WHOLE_ELEMENT_EXTENT],
                        &[data.symbol],
                        &mut refs,
                    );
                }
                // Keep the existing root qualification/resource permission
                // rule separate from default-domain fields of nominal values.
                if parameter.is_mutable {
                    continue;
                }
                let mut has_resource_claim = false;
                for domain_symbol in crate::facts::field_domain::domain_constraint_symbols(
                    program,
                    parameter.type_reference,
                ) {
                    has_resource_claim |= state_parameter_domain_is_resource_claim(
                        program,
                        parameter.type_reference,
                        domain_symbol,
                    );
                    append_state_parameter_domain_fact(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        obligation_origin,
                        &[],
                        domain_symbol,
                        &mut refs,
                    );
                }
                if has_resource_claim {
                    append_state_parameter_carry_origin(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        &mut refs,
                    );
                }
                for permission in carry_constraint_permissions(program, parameter.type_reference) {
                    append_state_parameter_carry_fact(
                        facts,
                        machine.symbol,
                        state.symbol,
                        parameter.symbol,
                        permission,
                        &mut refs,
                    );
                }
            }

            if refs.is_empty() {
                continue;
            }
            append_independent_place_contexts(
                facts,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

fn append_independent_place_contexts(
    facts: &mut FactPlan,
    point: ProgramPoint,
    refs: arena::HandleSpan<facts::FactRef>,
) {
    // Invalidating one storage coordinate must not discard independent
    // parameter or machine-field facts. Facts over the same place stay coupled.
    let mut groups: Vec<(FactPlace, Vec<facts::FactRef>)> = Vec::new();
    for reference in facts.refs.span_or_empty(refs) {
        let place = facts.facts.get(reference.fact).place;
        if let Some((_, group)) =
            groups
                .iter_mut()
                .find(|(candidate, _)| match (*candidate, place) {
                    (FactPlace::Place(left), FactPlace::Place(right)) => {
                        facts.places_equal(left, right)
                    }
                    _ => *candidate == place,
                })
        {
            group.push(*reference);
        } else {
            groups.push((place, vec![*reference]));
        }
    }
    for (_, group) in groups {
        let mut refs = arena::HandleSpan::empty();
        for reference in group {
            facts.refs.append_to_span(&mut refs, reference);
        }
        facts.append_context(point, refs);
    }
}

fn state_parameter_domain_is_resource_claim(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    crate::checks::type_multiplicity(program, type_reference)
        == language_semantics::Multiplicity::Linear
        && program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == domain_symbol)
            .is_some_and(|domain| {
                domain.predicate_body == language_semantics::DomainPredicateBody::Bodyless
            })
}

fn append_state_parameter_carry_origin(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::CarryOrigin {
            value: typed_trees::expression::ExpressionHandle::invalid(),
        },
    });
    facts.append_ref(refs, fact);
}

fn carry_constraint_permissions(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Vec<language_semantics::CarryPermission> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            carry_constraint_permissions(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    language_semantics::CarryPermission::from_name(domain.name.as_str())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn append_state_parameter_carry_fact(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    permission: language_semantics::CarryPermission,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin: FactOrigin::StateParameterDomain {
            machine_symbol,
            state_symbol,
        },
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::CarryPermission {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            permission,
        },
    });
    facts.append_ref(refs, fact);
}

#[allow(clippy::too_many_arguments)]
fn append_state_parameter_data_field_domain_facts(
    program: &TypedTrees,
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    origin: FactOrigin,
    data: &typed_trees::data::DataDefinition,
    prefix: &[PlaceSegment],
    visited: &[SymbolHandle],
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if readable_type_reference(program, field.type_reference).is_none() {
            continue;
        }
        let mut field_path = prefix.to_vec();
        crate::flow::push_field_place_segments(program, &mut field_path, field.symbol);
        for domain_symbol in field_domain_symbols(program, field.type_reference) {
            append_state_parameter_domain_fact(
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                origin,
                &field_path,
                domain_symbol,
                refs,
            );
        }
        if let Some(nested) = readable_nominal_definition(program, field.type_reference)
            && !visited.contains(&nested.symbol)
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.symbol);
            append_state_parameter_data_field_domain_facts(
                program,
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                origin,
                nested,
                &field_path,
                &next_visited,
                refs,
            );
        }
        // A slice-typed field's elements carry their declared fields through
        // the whole-extent row alone: no static extent exists to enumerate, so
        // it stays under `origin` -- for a slice the rollup IS the obligation.
        if let Some(element_type) = readable_slice_element(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.symbol)
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.symbol);
            let mut element_path = field_path.clone();
            element_path.push(WHOLE_ELEMENT_EXTENT);
            append_state_parameter_data_field_domain_facts(
                program,
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                origin,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }
        // A fixed array field enumerates each nominal element's declared fields
        // at `field[i]` -- element coverage rides the same entry facts and the
        // same call-boundary obligations as one-level fields. The whole-extent
        // `field[0..usize::MAX]` row is seeded alongside so a runtime selector
        // or a slice view can narrow coverage from it; it is EVIDENCE ONLY
        // (`StatementTransfer`) because the per-element rows already spell the
        // complete obligation.
        if let Some((element_type, length)) =
            readable_fixed_array_elements(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.symbol)
        {
            let mut next_visited = visited.to_vec();
            next_visited.push(nested.symbol);
            for index in 0..length {
                let mut element_path = field_path.clone();
                element_path.push(PlaceSegment::FixedIndex { index });
                append_state_parameter_data_field_domain_facts(
                    program,
                    facts,
                    machine_symbol,
                    state_symbol,
                    parameter_symbol,
                    origin,
                    nested,
                    &element_path,
                    &next_visited,
                    refs,
                );
            }
            let mut element_path = field_path.clone();
            element_path.push(WHOLE_ELEMENT_EXTENT);
            append_state_parameter_data_field_domain_facts(
                program,
                facts,
                machine_symbol,
                state_symbol,
                parameter_symbol,
                FactOrigin::StatementTransfer,
                nested,
                &element_path,
                &next_visited,
                refs,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_state_parameter_domain_fact(
    facts: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    origin: FactOrigin,
    path: &[PlaceSegment],
    domain_symbol: SymbolHandle,
    refs: &mut arena::HandleSpan<facts::FactRef>,
) {
    let place = facts.append_symbol_place(parameter_symbol);
    for segment in path {
        facts.push_place_segment(place, *segment);
    }
    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        origin,
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::Propagated,
            state_symbol,
        ),
        payload: FactPayload::DomainMembership {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            domain: arena::HandleSpan::empty(),
            domain_symbol,
            semantic_domain: language_semantics::SemanticDomainId::NULL,
        },
    });
    facts.append_ref(refs, fact);
}

/// #66 case-payload forwarding: a local constructed as a sum CASE with a
/// domain-refined payload (`let cmd = Command::Say { text: "ok" }`, where
/// `Command::Say`'s `text` is `&[u8] in Utf8`) carries that payload domain on
/// `cmd.<payload>`. Construction enforcement (#60-1c, checks::contracts::writes)
/// already proved the payload value in-domain at construction, so a later read of
/// the payload -- in particular a destructured `Command::Say { text }` forwarded
/// as a `requires <arg> in D` call argument, which resolves to `cmd.<payload>` --
/// discharges with no re-proof. Surfaced at `ProgramPoint::State`, folded into the
/// state entry and threaded to the matched arm's calls; the flow's mutation
/// invalidation drops it if `cmd` is reassigned (so it is sound for mutable locals
/// too -- no immutability gate needed).
///
/// CONSERVATIVE: a sibling arm that matches a DIFFERENT case is dead when `cmd` was
/// constructed as one specific case; its `cmd.<other-payload>` obligation shares
/// the same canonical place label, so it discharges only when the domains agree
/// (a mixed-domain dead arm would be rejected -- sound, just incomplete).
pub(super) fn append_local_case_payload_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                if !local_data.initial_value.is_valid() {
                    continue;
                }
                let ExpressionNode::StructLiteral(literal) = program
                    .expression_table
                    .expression(local_data.initial_value)
                else {
                    continue;
                };
                let Some(case_name) = literal.case_name.as_ref() else {
                    continue;
                };
                let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.name.as_str() == literal.type_name.as_str())
                else {
                    continue;
                };
                let Some(variant) =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.name.as_str() == case_name.as_str() =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                else {
                    continue;
                };

                for payload_field in program.data_payload_fields(variant) {
                    for domain_symbol in field_domain_symbols(program, payload_field.type_reference)
                    {
                        // Place `cmd.<payload-field>`: root the local symbol + a Field
                        // segment for the variant's payload field, matching how a
                        // destructured payload arg resolves at the call site.
                        let place = facts.append_symbol_place(local_data.symbol);
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Case {
                                variant: variant.symbol,
                            },
                        );
                        facts.push_place_segment(
                            place,
                            PlaceSegment::Field {
                                symbol: payload_field.symbol,
                            },
                        );
                        let fact = facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point: ProgramPoint::State {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            origin: FactOrigin::LocalCasePayloadDomain {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                            },
                            evidence: QualificationEvidence::from_origin(
                                language_semantics::QualificationEvidenceOrigin::Prover,
                                state.symbol,
                            ),
                            payload: FactPayload::DomainMembership {
                                value: typed_trees::expression::ExpressionHandle::invalid(),
                                domain: arena::HandleSpan::empty(),
                                domain_symbol,
                                semantic_domain: language_semantics::SemanticDomainId::NULL,
                            },
                        });
                        facts.append_ref(&mut refs, fact);
                    }
                }
            }

            if refs.is_empty() {
                continue;
            }
            facts.append_context(
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

/// `let value: T;` binds zero-initialized storage (dependent_values.md:
/// zero-initializability is a storage representation guarantee). When `T`'s
/// declared field domains admit the zero/empty byte sequence, that zero value
/// already satisfies each such field, so `value.<field> in D` is live evidence
/// from the binding onward -- the same gate machine storage uses for its
/// entry invariants. Seeded at `ProgramPoint::State` with the
/// `StatementTransfer` origin: the binding statement is the formation point,
/// and the symbol cannot resolve before its declaration, so folding the rows
/// into state entry is exact. Ordinary mutation invalidation retires every
/// row, and domains the zero value violates stay gated -- a read of a gated
/// field still requires an establishing write first.
///
/// An initializer (`let value: T = expr;`) gets its domain evidence from the
/// assignment transfer instead; seeding the zero here too would be wrong
/// because the initializer replaces the zero before any observation.
pub(super) fn append_local_zii_field_domain_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut refs = arena::HandleSpan::empty();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                if local_data.initial_value.is_valid() {
                    continue;
                }
                let point = ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                };
                let origin = FactOrigin::StatementTransfer;
                let evidence = QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    state.symbol,
                );

                // Whole-value `let x: T in D;` rows: zero satisfies `D` only
                // when the domain admits the empty byte sequence -- the same
                // ZII gate the per-field rows below use.
                for (domain_symbol, semantic_domain) in
                    crate::facts::field_domain::domain_constraint_identities(
                        program,
                        local_data.type_reference,
                    )
                    .into_iter()
                    .filter(|(domain_symbol, _)| {
                        crate::facts::field_domain::domain_admits_empty_byte_sequence(
                            program,
                            *domain_symbol,
                        )
                    })
                {
                    let place = facts.append_symbol_place(local_data.symbol);
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin,
                        evidence,
                        payload: FactPayload::DomainMembership {
                            value: typed_trees::expression::ExpressionHandle::invalid(),
                            domain: arena::HandleSpan::empty(),
                            domain_symbol,
                            semantic_domain,
                        },
                    });
                    facts.append_ref(&mut refs, fact);
                }

                let Some(data) = crate::facts::field_domain::owned_nominal_data_definition(
                    program,
                    local_data.type_reference,
                ) else {
                    continue;
                };
                append_data_field_domain_facts(
                    program,
                    facts,
                    local_data.symbol,
                    point,
                    origin,
                    evidence,
                    data,
                    &[],
                    &[data.name.as_str()],
                    &mut refs,
                );
            }

            if refs.is_empty() {
                continue;
            }
            append_independent_place_contexts(
                facts,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                refs,
            );
        }
    }
}

/// A symbol named `self` for the machine's receiver -- the `is_self` parameter
/// of any of its states (all spelled `self`, so the canonical label is identical
/// regardless of which state's receiver symbol is used).
fn machine_self_parameter_symbol(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<SymbolHandle> {
    program.machine_states(machine).iter().find_map(|state| {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol)
    })
}

/// Every normalized predicate-domain symbol on a field type, looking through a
/// leading reference (`&[u8] in Utf8 & NoNul`). Semantic-only constraints do
/// not become flow facts.
fn field_domain_symbols(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, type_reference)
}

/// The element type of a slice (`[T]`, `&[T]`, `&mut [T]`), looking through a
/// leading reference exactly as `readable_fixed_array_elements` does for fixed
/// arrays. `None` for non-slice or write-only types.
fn readable_slice_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program
        .type_reference_table
        .type_reference(readable_type_reference(program, type_reference)?)
    {
        typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}
