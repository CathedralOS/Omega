//! Commit jointly accounted content derivations at the completed value.
//!
//! Each equation already has its completion-time proof and exact unborrowed
//! parent frontier. That is not enough to spend it: two equations can each
//! spend the same parent, or omit another content qualification on that claim.
//! Prepare the entire transaction before changing any live place. Children
//! retain their nominal identities and construction provenance; their new
//! qualification authority lives in the derivation, not in a replacement ID.

mod boundary_correspondence;

use arena::Handle;
use checked_trees::facts::{
    ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
    ContentBodyCustody, ContentBodyDerivation, ContentBodyTransformation,
    ContentClaimTransformation, ContentClaimTransformationSource,
};
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
};
use validation::CanonicalPlace;

use super::linear_obligations::LinearPlace;

/// One parent may carry several independent qualifications but is consumed
/// only once. Its original available-input row retains the exact source place.
struct Parent {
    identity: PermissionClaimIdentity,
    available: Handle<ContentBodyAvailableInput>,
    place_index: usize,
}

struct Transaction {
    completion: Handle<checked_trees::FlowOperandFact>,
    equations: Vec<PreparedDerivation>,
    parents: Vec<Parent>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
}

struct PreparedDerivation {
    conservation: Handle<ContentBodyConservationFact>,
    lineage_anchor: PermissionClaimIdentity,
    custody: PreparedCustody,
}

enum PreparedCustody {
    Derived,
    Boundary(boundary_correspondence::PreparedCorrespondence),
}

pub(super) fn commit(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    observed: &[Handle<ContentBodyConservationFact>],
    places: &mut [LinearPlace],
    events: &mut Vec<FlowPermissionEventFact>,
) -> Result<(), &'static str> {
    if observed.is_empty() {
        return Ok(());
    }
    let transaction = prepare(program, facts, observed, places)?;
    let completed = facts.flow.control.operands.get(transaction.completion);
    let expression = completed.expression;
    let source = PermissionEventSource::Statement {
        statement_index: completed.statement_index,
    };
    for derived in &transaction.equations {
        let content = &mut facts.qualifications.content;
        let equation = content.body_conservations.get(derived.conservation);
        let outputs = equation.outputs;
        let inputs = content
            .body_available_inputs
            .span_or_empty(equation.available_inputs)
            .iter()
            .map(|available| content.body_claim_bindings.get(available.binding).clone())
            .collect::<Vec<_>>();
        let inputs = content.body_claim_bindings.insert_many(inputs);
        content
            .claim_transformations
            .insert(ContentClaimTransformation {
                machine_symbol: transaction.machine,
                state_symbol: transaction.state,
                statement_index: completed.statement_index,
                expression,
                evaluation: transaction.evaluation,
                source: ContentClaimTransformationSource::Body {
                    conservation: derived.conservation,
                },
                inputs,
                outputs,
            });
    }
    // All fallible checks precede this point. Recording the relation and the
    // consumed frontier is one successful completion, with no partial grant.
    let content = &mut facts.qualifications.content;
    let mut derivations = Vec::with_capacity(transaction.equations.len());
    for prepared in transaction.equations {
        let custody = match prepared.custody {
            PreparedCustody::Derived => ContentBodyCustody::Derived,
            PreparedCustody::Boundary(prepared) => {
                let requirements = prepared
                    .requirements
                    .into_iter()
                    .map(|requirement| {
                        let conservation_plan_index = content.boundary_requirement_plans.len();
                        content
                            .boundary_requirement_plans
                            .push(requirement.conservation);
                        checked_trees::facts::ContentBoundaryBodyRequirement {
                            declaring_trait: requirement.declaring_trait,
                            requirement: requirement.requirement,
                            realization_state: requirement.realization_state,
                            conservation_plan_index,
                        }
                    })
                    .collect::<Vec<_>>();
                let requirements = content.boundary_body_requirements.insert_many(requirements);
                let addresses = prepared
                    .addresses
                    .into_iter()
                    .map(
                        |address| checked_trees::facts::ContentBackingAddressTransport {
                            input: address.input,
                            output: address.output,
                            input_segments: facts
                                .flow
                                .ownership
                                .segments
                                .insert_many(address.input_segments),
                            output_segments: facts
                                .flow
                                .ownership
                                .segments
                                .insert_many(address.output_segments),
                        },
                    )
                    .collect::<Vec<_>>();
                let addresses = content.backing_addresses.insert_many(addresses);
                ContentBodyCustody::Boundary {
                    requirements,
                    addresses,
                }
            }
        };
        derivations.push(ContentBodyDerivation {
            conservation: prepared.conservation,
            lineage_anchor: prepared.lineage_anchor,
            custody,
        });
    }
    let derivations = content.body_derivations.insert_many(derivations);
    facts
        .qualifications
        .content
        .body_transformations
        .insert(ContentBodyTransformation {
            machine_symbol: transaction.machine,
            state_symbol: transaction.state,
            completed_value: transaction.completion,
            evaluation: transaction.evaluation,
            derivations,
        });
    for parent in transaction.parents {
        let available = facts
            .qualifications
            .content
            .body_available_inputs
            .get(parent.available);
        let place = &mut places[parent.place_index];
        events.push(FlowPermissionEventFact {
            expression,
            evaluation: transaction.evaluation,
            machine_symbol: transaction.machine,
            state_symbol: transaction.state,
            source,
            kind: PermissionEventKind::Consume,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity: parent.identity,
            provenance: available.provenance,
            root: available.root,
            segments: available.segments,
            obligation_live: place.live
                && place.multiplicity == language_semantics::Multiplicity::Linear,
        });
        place.live = false;
    }
    Ok(())
}

fn prepare(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    observed: &[Handle<ContentBodyConservationFact>],
    places: &[LinearPlace],
) -> Result<Transaction, &'static str> {
    let content = &facts.qualifications.content;
    let first = content.body_conservations.get(observed[0]);
    let mut transaction = Transaction {
        completion: first.completed_value,
        equations: Vec::new(),
        parents: Vec::new(),
        machine: first.machine_symbol,
        state: first.state_symbol,
        evaluation: first.evaluation,
    };
    let mut inputs: Vec<&ContentBodyClaimBinding> = Vec::new();
    let mut outputs: Vec<&ContentBodyClaimBinding> = Vec::new();
    for handle in observed {
        let equation = content.body_conservations.get(*handle);
        if equation.completed_value != first.completed_value
            || equation.evaluation != first.evaluation
        {
            return Err("derived transaction has more than one completion");
        }
        // Several outgoing uses can discover the very same formation demand.
        // Keep that one equation once; different equations still face joint
        // accounting rather than each spending the same input independently.
        if transaction.equations.iter().any(|derived| {
            content
                .body_conservations
                .get(derived.conservation)
                .plan_index
                == equation.plan_index
        }) {
            continue;
        }
        let available = content
            .body_available_inputs
            .span_or_empty(equation.available_inputs);
        if available.is_empty()
            || equation
                .inputs
                .handles()
                .any(|entry| !available.iter().any(|input| input.entry_binding == entry))
        {
            return Err("derived transaction lacks every available parent");
        }
        let first_owner = available.first().map(|available| {
            let binding = content.body_claim_bindings.get(available.binding);
            (
                binding.domain,
                binding.semantic_domain,
                binding.projection_machine,
            )
        });
        let ordinary = available
            .iter()
            .map(|available| content.body_claim_bindings.get(available.binding))
            .chain(content.body_claim_bindings.span_or_empty(equation.outputs))
            .all(|binding| {
                Some((
                    binding.domain,
                    binding.semantic_domain,
                    binding.projection_machine,
                )) == first_owner
            });
        // Classify authority before preparing it. Failure of the ordinary
        // owner rule is not a catch-all grant: the other operation requires
        // its own exact applied public law and completed backing proof.
        let custody = if ordinary {
            PreparedCustody::Derived
        } else {
            PreparedCustody::Boundary(boundary_correspondence::prepare(program, facts, equation)?)
        };
        let mut anchor = PermissionClaimIdentity::Unknown;
        let mut owner = None;
        for input in available {
            let binding = content.body_claim_bindings.get(input.binding);
            let projection = (
                binding.domain,
                binding.semantic_domain,
                binding.projection_machine,
            );
            if ordinary && owner.is_some_and(|owner| owner != projection) {
                return Err("one derivation cannot unite distinct qualification owners");
            }
            owner = Some(projection);
            if inputs
                .iter()
                .any(|previous| same_qualification(previous, binding))
            {
                return Err("two equations spend the same parent qualification");
            }
            inputs.push(binding);
            let lineage =
                lineage_anchor(facts, content.body_claim_bindings.get(input.entry_binding))?;
            if anchor != PermissionClaimIdentity::Unknown && anchor != lineage {
                return Err("derived parents have no checked common backing lineage");
            }
            anchor = lineage;
        }
        for output in content.body_claim_bindings.span_or_empty(equation.outputs) {
            if ordinary
                && owner
                    != Some((
                        output.domain,
                        output.semantic_domain,
                        output.projection_machine,
                    ))
            {
                return Err("a conservation equation cannot introduce another qualification");
            }
            if outputs
                .iter()
                .any(|previous| same_qualification(previous, output))
            {
                return Err("two equations establish the same output qualification");
            }
            outputs.push(output);
        }
        for available_handle in equation.available_inputs.handles() {
            let available = content.body_available_inputs.get(available_handle);
            let binding = content.body_claim_bindings.get(available.binding);
            if transaction
                .parents
                .iter()
                .any(|parent| parent.identity == binding.claim_identity)
            {
                continue;
            }
            let path = facts
                .flow
                .ownership
                .segments
                .span_or_empty(available.segments);
            let mut live = places.iter().enumerate().filter(|(_, place)| {
                place.live
                    && !place.case_excluded
                    && place.root == available.root
                    && place.path == path
                    && place.claim_identity == Some(binding.claim_identity)
            });
            let (place_index, _) = live.next().ok_or("transaction parent is no longer live")?;
            if live.next().is_some() {
                return Err("transaction parent has duplicate custody");
            }
            if transaction.parents.iter().any(|parent| {
                let other = &places[parent.place_index];
                other.root == available.root
                    && (other.path.starts_with(path) || path.starts_with(&other.path))
            }) {
                return Err("transaction parent ownership regions overlap");
            }
            transaction.parents.push(Parent {
                identity: binding.claim_identity,
                available: available_handle,
                place_index,
            });
        }
        transaction.equations.push(PreparedDerivation {
            conservation: *handle,
            lineage_anchor: anchor,
            custody,
        });
    }
    if inputs.iter().any(|input| {
        outputs
            .iter()
            .any(|output| input.claim_identity == output.claim_identity)
    }) {
        return Err("identity forwarding belongs to the existing claim transfer");
    }
    let completed =
        crate::flow::CapturedOperand::recorded(&facts.flow.control, transaction.completion)
            .ok_or("missing raw output observation")?;
    let contexts = facts
        .flow
        .semantic_constraint_contexts(completed.constraints)
        .collect::<Vec<_>>();
    let state_flow = facts
        .flow
        .control
        .state(transaction.machine, transaction.state)
        .ok_or("missing transformation state flow")?;
    for output in &outputs {
        if !validation::field_domains::domain_requires_provenance(program, output.domain) {
            continue;
        }
        let values = crate::flow::captured_operand_values(
            program,
            &facts.flow,
            &facts.semantic,
            state_flow,
            completed,
            completed.type_reference,
            facts.flow.ownership.segments.span_or_empty(output.segments),
        )
        .ok_or("missing raw output observation")?;
        for value in values {
            let contexts = crate::flow::operand_contexts(
                program,
                &facts.flow.contexts,
                &facts.flow.control,
                &facts.semantic,
                value.operand.row(),
                value.type_reference,
                value.operand.constraints,
            );
            let sources = crate::flow::captured_value_places(program, &facts.flow, &value)
                .ok_or("missing raw output source")?;
            if sources.iter().any(|source| {
                crate::checks::contracts::exact_scalar_membership(
                    program,
                    facts,
                    &contexts,
                    &source.projected,
                    output.domain,
                    output.semantic_domain,
                )
            }) {
                return Err("derived output already carries independent routed authority");
            }
        }
    }
    for parent in &transaction.parents {
        let available = content.body_available_inputs.get(parent.available);
        let current = CanonicalPlace {
            root: available.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span_or_empty(available.segments)
                .to_vec(),
        };
        let reference = validation::canonical_place_type_reference(
            program,
            transaction.state,
            completed.statement_index,
            &current,
        )
        .ok_or("missing consumed parent carrier")?;
        require_accounted_subtree(program, facts, &transaction, &contexts, &current, reference)?;
    }
    Ok(transaction)
}

/// Spending a nominal owner spends its entire owned subtree, even when its
/// multiplicity hid separate child claims. Declaration obligations survive
/// missing membership facts; current facts additionally retain qualifications
/// acquired after entry. Neither scalar totals nor missing case evidence may
/// erase a nested account.
fn require_accounted_subtree(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    transaction: &Transaction,
    contexts: &[facts::FactContextHandle],
    parent: &CanonicalPlace,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Result<(), &'static str> {
    let completed = facts.flow.control.operands.get(transaction.completion);
    let inactive = |subject: &CanonicalPlace| {
        crate::flow::place_case_path_is_inactive(
            program,
            &facts.semantic,
            contexts,
            transaction.machine,
            transaction.state,
            completed.statement_index,
            subject,
        )
    };
    let subject_at = |path: &[facts::PlaceSegment]| {
        let mut subject = parent.clone();
        subject.segments.extend_from_slice(path);
        subject
    };
    let census = validation::field_domains::declared_owned_field_obligations(program, reference);
    for path in census.unresolved {
        if !inactive(&subject_at(&path)) {
            return Err("consumed parent has unresolved owned storage");
        }
    }
    let content = &facts.qualifications.content;
    for path in census.custody {
        let subject = subject_at(&path);
        if inactive(&subject) {
            continue;
        }
        let accounted = transaction.equations.iter().any(|derivation| {
            let equation = content.body_conservations.get(derivation.conservation);
            content
                .body_available_inputs
                .span_or_empty(equation.available_inputs)
                .iter()
                .any(|available| {
                    available.root == subject.root
                        && facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(available.segments)
                            == subject.segments
                })
        });
        if !accounted {
            return Err("consumed parent has an unaccounted nested custody obligation");
        }
    }
    let mut obligations =
        validation::field_domains::domain_constraint_identities(program, reference)
            .into_iter()
            .map(|(domain, instance)| (parent.clone(), domain, instance))
            .collect::<Vec<_>>();
    obligations.extend(
        census
            .identities
            .into_iter()
            .map(|(path, domain, instance)| (subject_at(&path), domain, instance)),
    );
    for context in contexts {
        for fact in facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
        {
            let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                continue;
            };
            // Predicate-only extent rollups are reusable facts, not additional
            // content accounts. Do not require a scalar custody projection for
            // them (a whole-array predicate may use an extent coordinate).
            if !content.plans.iter().any(|projection| {
                projection.domain == domain && projection.semantic_domain == instance
            }) {
                continue;
            }
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let Some(subject) = crate::flow::canonical_place_from_semantic_place(
                program,
                &facts.semantic,
                facts.semantic.places.get(place),
            ) else {
                continue;
            };
            if subject.root != parent.root || !subject.segments.starts_with(&parent.segments) {
                continue;
            }
            if inactive(&subject) {
                continue;
            }
            let path = &subject.segments[parent.segments.len()..];
            // A readable place can cross a borrowed referent. Inspect every
            // typed prefix before admitting it as part of this owned account;
            // unresolved projection is not evidence that the debt is absent.
            let mut borrowed = false;
            for length in 0..=path.len() {
                let projected = validation::project_type_reference_from_segments(
                    program,
                    reference,
                    &path[..length],
                )
                .ok_or("content qualification has unresolved owned projection")?;
                if validation::reference_shell(program, projected).is_some() {
                    borrowed = true;
                    break;
                }
            }
            if !borrowed && !obligations.contains(&(subject.clone(), domain, instance)) {
                obligations.push((subject, domain, instance));
            }
        }
    }
    for (subject, domain, instance) in obligations {
        if inactive(&subject) {
            continue;
        }
        for projection in content.plans.iter().filter(|projection| {
            projection.domain == domain && projection.semantic_domain == instance
        }) {
            let accounted = transaction.equations.iter().any(|derivation| {
                let equation = content.body_conservations.get(derivation.conservation);
                content
                    .body_available_inputs
                    .span_or_empty(equation.available_inputs)
                    .iter()
                    .any(|available| {
                        let binding = content.body_claim_bindings.get(available.binding);
                        available.root == subject.root
                            && facts
                                .flow
                                .ownership
                                .segments
                                .span_or_empty(available.segments)
                                == subject.segments
                            && binding.domain == domain
                            && binding.semantic_domain == instance
                            && binding.projection_machine == projection.machine
                    })
            });
            if !accounted {
                return Err("consumed parent has an unaccounted content qualification");
            }
        }
    }
    Ok(())
}

/// Storage consumers follow the committed operation, not a fabricated source
/// move whose expression would incorrectly name the result as its parent.
pub(super) fn consumed_places(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<CanonicalPlace> {
    let content = &facts.qualifications.content;
    let mut places = Vec::new();
    for (_, transaction) in content.body_transformations.iter() {
        let completed = facts.flow.control.operands.get(transaction.completed_value);
        if transaction.machine_symbol != machine
            || transaction.state_symbol != state
            || completed.statement_index != statement
            || completed.expression != expression
        {
            continue;
        }
        for derivation in content
            .body_derivations
            .span_or_empty(transaction.derivations)
        {
            let equation = content.body_conservations.get(derivation.conservation);
            for input in content
                .body_available_inputs
                .span_or_empty(equation.available_inputs)
            {
                let place = CanonicalPlace {
                    root: input.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(input.segments)
                        .to_vec(),
                };
                if !places.contains(&place) {
                    places.push(place);
                }
            }
        }
    }
    places
}

fn same_qualification(left: &ContentBodyClaimBinding, right: &ContentBodyClaimBinding) -> bool {
    left.claim_identity == right.claim_identity
        && left.domain == right.domain
        && left.semantic_domain == right.semantic_domain
        && left.projection_machine == right.projection_machine
}

fn lineage_anchor(
    facts: &CheckFacts,
    input: &ContentBodyClaimBinding,
) -> Result<PermissionClaimIdentity, &'static str> {
    let content = &facts.qualifications.content;
    let mut anchor = None;
    for (_, derivation) in content.body_derivations.iter() {
        if !matches!(derivation.custody, ContentBodyCustody::Derived) {
            continue;
        }
        let equation = content.body_conservations.get(derivation.conservation);
        if content
            .body_claim_bindings
            .span_or_empty(equation.outputs)
            .iter()
            .any(|output| same_qualification(input, output))
        {
            if anchor.is_some_and(|anchor| anchor != derivation.lineage_anchor) {
                return Err("ambiguous qualified ancestor");
            }
            anchor = Some(derivation.lineage_anchor);
        }
    }
    // The body binder names an exact already-qualified entry claim. With no
    // earlier derivation that entry is an opaque authority anchor, not fresh
    // issuance. Distinct entries cannot be unified by equal provenance or
    // equal geometry; call substitution must establish any relation first.
    Ok(anchor.unwrap_or(input.claim_identity))
}
