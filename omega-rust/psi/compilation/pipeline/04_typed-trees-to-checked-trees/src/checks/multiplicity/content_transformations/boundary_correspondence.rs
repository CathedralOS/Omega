//! Prepare a checked boundary implementation's raw-result custody relation.
//!
//! Public result authorization and a proved content equation do not identify
//! backing. Every address in the consumed owned accounts must correspond to
//! exactly one address in the raw result in the same completed observation.
//! This preparation grants no result qualification and consults no Build row.

use arena::Handle;
use checked_trees::CheckFacts;
use checked_trees::facts::{
    ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
};
use language_semantics::PermissionClaimIdentity;
use language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use validation::CanonicalPlace;

pub(super) struct PreparedCorrespondence {
    pub requirements: Vec<validation::ContentBoundaryBodyRequirement>,
    pub addresses: Vec<PreparedAddress>,
}

pub(super) struct PreparedAddress {
    pub input: Handle<ContentBodyAvailableInput>,
    pub output: Handle<ContentBodyClaimBinding>,
    pub input_segments: Vec<facts::PlaceSegment>,
    pub output_segments: Vec<facts::PlaceSegment>,
}

struct InputAddress {
    available: Handle<ContentBodyAvailableInput>,
    identity: PermissionClaimIdentity,
    relative: Vec<facts::PlaceSegment>,
    entry: CanonicalPlace,
    current: CanonicalPlace,
}

struct OutputAddress {
    binding: Handle<ContentBodyClaimBinding>,
    identity: PermissionClaimIdentity,
    relative: Vec<facts::PlaceSegment>,
    result: ContentStructuralPlace,
}

pub(super) fn prepare(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    equation: &ContentBodyConservationFact,
) -> Result<PreparedCorrespondence, &'static str> {
    let content = &facts.qualifications.content;
    let law = content
        .conservation_plans
        .get(equation.plan_index)
        .ok_or("boundary custody law is absent")?;
    let sources = validation::content_conservation_plans(program);
    let body = sources
        .iter()
        .find(|source| &source.plan == law)
        .ok_or("boundary custody law has no exact normalized source")?;
    let mut requirements = validation::boundary_body_content_requirements(program, body)?;
    // One actual applied requirement must authorize the complete output
    // roster. An authorized D result cannot donate authority to another D'.
    requirements.retain(|requirement| {
        content
            .body_claim_bindings
            .span_or_empty(equation.outputs)
            .iter()
            .all(|output| {
                body.boundary_subjects.iter().any(|subject| {
                    subject.declaring_trait == requirement.declaring_trait
                        && subject.requirement == requirement.requirement
                        && subject.realization_state == requirement.realization_state
                        && !subject.parameter.is_valid()
                        && subject.domain == output.domain
                        && subject.semantic_domain == output.semantic_domain
                        && output.segments.is_empty()
                })
            })
    });
    if requirements.is_empty() {
        return Err("boundary custody outputs have no complete exact result authorization");
    }
    let completed = facts.flow.control.operands.get(equation.completed_value);
    let machine = program
        .machine_by_symbol(equation.machine_symbol)
        .ok_or("boundary custody machine is absent")?;
    let state = program
        .state_by_symbol(equation.state_symbol)
        .ok_or("boundary custody state is absent")?;
    let ownership = &facts.flow.ownership;
    let mut inputs: Vec<InputAddress> = Vec::new();
    for handle in equation.available_inputs.handles() {
        let available = content.body_available_inputs.get(handle);
        let binding = content.body_claim_bindings.get(available.binding);
        let [entry] = content
            .body_claim_bindings
            .span_or_empty(available.entry_bindings)
        else {
            return Err(
                "boundary custody needs exact original-to-current backing correspondence; a joint ancestry roster alone does not establish it",
            );
        };
        let current = CanonicalPlace {
            root: available.root,
            segments: ownership
                .segments
                .span_or_empty(available.segments)
                .to_vec(),
        };
        let reference = validation::canonical_place_type_reference(
            program,
            state.symbol,
            completed.statement_index,
            &current,
        )
        .ok_or("boundary custody input has no exact carrier")?;
        for relative in owned_addresses(program, reference)? {
            if inputs.iter().any(|previous| {
                previous.identity == binding.claim_identity && previous.relative == relative
            }) {
                continue;
            }
            let mut entry_path = ownership.segments.span_or_empty(entry.segments).to_vec();
            entry_path.extend_from_slice(&relative);
            let mut current = current.clone();
            current.segments.extend_from_slice(&relative);
            inputs.push(InputAddress {
                available: handle,
                identity: binding.claim_identity,
                relative,
                entry: CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(entry.parameter_symbol),
                    segments: entry_path,
                },
                current,
            });
        }
    }
    let mut outputs: Vec<OutputAddress> = Vec::new();
    for handle in equation.outputs.handles() {
        let binding = content.body_claim_bindings.get(handle);
        let prefix = ownership.segments.span_or_empty(binding.segments);
        let reference =
            validation::owned_projection_type_reference(program, completed.type_reference, prefix)
                .ok_or("boundary custody result has no exact owned carrier")?;
        for relative in owned_addresses(program, reference)? {
            if outputs.iter().any(|previous| {
                previous.identity == binding.claim_identity && previous.relative == relative
            }) {
                continue;
            }
            let mut path = prefix.to_vec();
            path.extend_from_slice(&relative);
            outputs.push(OutputAddress {
                binding: handle,
                identity: binding.claim_identity,
                relative,
                result: ContentStructuralPlace {
                    root: ContentPlaceRoot::Result,
                    version: ContentPlaceVersion::Current,
                    segments: crate::checks::content::content_path(program, &path)
                        .ok_or("boundary custody result address path is not finite")?,
                },
            });
        }
    }
    if inputs.is_empty() || inputs.len() != outputs.len() {
        return Err("boundary custody requires the complete same-sized backing address roster");
    }
    let goals = inputs
        .iter()
        .flat_map(|input| {
            outputs
                .iter()
                .map(move |output| validation::ContentBackingAddressGoal {
                    entry: input.entry.clone(),
                    current: input.current.clone(),
                    result: output.result.clone(),
                })
        })
        .collect::<Vec<_>>();
    let proven = validation::completed_value_proves_backing_addresses(
        program,
        machine,
        state,
        completed.statement_index,
        completed.expression,
        completed.type_reference,
        &goals,
    )?;
    let mut used = vec![false; outputs.len()];
    let mut addresses = Vec::new();
    for (position, input) in inputs.iter().enumerate() {
        let mut matching = proven[position * outputs.len()..(position + 1) * outputs.len()]
            .iter()
            .enumerate()
            .filter_map(|(position, proven)| proven.then_some(position));
        let selected = matching
            .next()
            .ok_or("boundary custody does not prove an input's exact backing transport")?;
        if matching.next().is_some() || used[selected] {
            return Err(
                "boundary custody backing correspondence is ambiguous or repeats an address",
            );
        }
        used[selected] = true;
        let output = &outputs[selected];
        addresses.push(PreparedAddress {
            input: input.available,
            output: output.binding,
            input_segments: input.relative.clone(),
            output_segments: output.relative.clone(),
        });
    }
    Ok(PreparedCorrespondence {
        requirements,
        addresses,
    })
}

/// Enumerate the address-bearing leaves of finite owned storage. Project from
/// the actual root type at each step, retaining scoped generic substitutions.
/// A reference shell is a distinct loan, never an owned backing traversal.
fn owned_addresses(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Vec<Vec<facts::PlaceSegment>>, &'static str> {
    fn visit(
        program: &dyn validation::CheckingInput,
        root: TypeReferenceHandle,
        path: &mut Vec<facts::PlaceSegment>,
        ancestors: &mut Vec<symbols::SymbolHandle>,
        output: &mut Vec<Vec<facts::PlaceSegment>>,
    ) -> Result<(), &'static str> {
        let reference = validation::owned_projection_type_reference(program, root, path)
            .ok_or("boundary custody backing projection is unresolved")?;
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if validation::reference_shell(program, carrier).is_some() {
            return Err(
                "boundary custody cannot reinterpret a borrowed address descriptor as owned backing",
            );
        }
        if let Some(primitive) = program.primitive_type_reference(carrier) {
            if primitive == PrimitiveType::Addr {
                output.push(path.clone());
            }
            return Ok(());
        }
        if let Some(data) = program
            .closed_data_definition(carrier)
            .or_else(|| validation::field_domains::owned_nominal_data_definition(program, carrier))
        {
            if ancestors.contains(&data.symbol) {
                return Err("boundary custody backing shape is recursive");
            }
            ancestors.push(data.symbol);
            for field in program
                .data_member_fields(program.data_members(data))
                .filter(|field| !field.relevance.is_erased())
            {
                let length = path.len();
                validation::push_field_place_segments(program, path, field.symbol);
                visit(program, root, path, ancestors, output)?;
                path.truncate(length);
            }
            ancestors.pop();
            return Ok(());
        }
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Unit => Ok(()),
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } => {
                for index in 0..*length {
                    path.push(facts::PlaceSegment::FixedIndex { index });
                    visit(program, root, path, ancestors, output)?;
                    path.pop();
                }
                Ok(())
            }
            _ => Err("boundary custody backing storage has no closed finite shape"),
        }
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut output,
    )?;
    Ok(output)
}
