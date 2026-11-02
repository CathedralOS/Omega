//! A selected generic application closes an entire portable family, not one
//! target body. The ordinary clone producer retains each declaration's own
//! substitution and custody records; this owner publishes their association.

use super::{Candidate, Diagnostic, SymbolHandle, TypedTrees};
use typed_trees::machine::TargetMachineFamilyMember;
use validation::CheckingInput;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(super) fn clone_specialization(
    source: Option<&dyn CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    ordinal: usize,
    template_identity: String,
    template_commitment: Option<String>,
) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Vec<Diagnostic>> {
    let input = source.unwrap_or(program);
    let template = candidate.template.template_symbol;
    let mut families = input.target_machine_families.iter().filter(|family| {
        family.canonical == template
            || family
                .members
                .iter()
                .any(|member| member.machine == template)
    });
    let family = families.next().cloned();
    if families.next().is_some() {
        return Err(failure(
            "generic machine belongs to multiple target families",
        ));
    }
    let Some(mut family) = family else {
        if input.machines()[candidate.template.machine_index].target_family_interface {
            return Err(failure(
                "generic target family interface has no retained family roster",
            ));
        }
        return super::body_cloning::clone_single_specialized_machine(
            source,
            program,
            candidate,
            ordinal,
            template_identity,
            template_commitment,
            None,
        );
    };
    if family.members.is_empty() {
        return Err(failure("generic target family has no retained members"));
    }

    // Capture all original declarations before the mutable arena gains clones.
    // Binder identities are declaration-local; only the admitted application
    // tuple is positional across the family telescope.
    let mut templates = Vec::with_capacity(family.members.len() + 1);
    templates.push(family.canonical);
    templates.extend(family.members.iter().map(|member| member.machine));
    let mut plans = Vec::with_capacity(templates.len());
    for symbol in templates {
        if plans.iter().any(|(retained, _, _, _)| *retained == symbol) {
            return Err(failure("generic target family repeats a declaration"));
        }
        let Some(machine_index) = input
            .machines()
            .iter()
            .position(|machine| machine.symbol == symbol)
        else {
            return Err(failure("generic target family declaration is absent"));
        };
        let mut concrete = super::candidate::from_machine(input, machine_index);
        if concrete.type_bindings.len() != candidate.type_bindings.len()
            || concrete.const_bindings.len() != candidate.const_bindings.len()
            || concrete.runtime_value_bindings.len() != candidate.runtime_value_bindings.len()
            || concrete.machine_bindings.len() != candidate.machine_bindings.len()
            || concrete.evidence_bindings.len() != candidate.evidence_bindings.len()
            || concrete.template.value_const_parameters != candidate.template.value_const_parameters
        {
            return Err(failure(
                "generic target family application has incompatible binder positions",
            ));
        }
        concrete.binding_application = candidate.binding_application;
        concrete.type_bindings.clone_from(&candidate.type_bindings);
        concrete
            .const_bindings
            .clone_from(&candidate.const_bindings);
        concrete
            .runtime_value_bindings
            .clone_from(&candidate.runtime_value_bindings);
        concrete
            .machine_bindings
            .clone_from(&candidate.machine_bindings);
        concrete
            .evidence_bindings
            .clone_from(&candidate.evidence_bindings);
        let machine = &input.machines()[machine_index];
        if symbol == family.canonical && !machine.target_family_interface {
            return Err(failure(
                "generic target family has no independent interface declaration",
            ));
        }
        let identity = normalized_machine_identity(input, machine).ok_or_else(|| {
            failure("generic target family declaration has no normalized identity")
        })?;
        let commitment = accepted_template_commitment(input, machine_index);
        plans.push((symbol, concrete, identity, commitment));
    }

    let mut instances = Vec::with_capacity(plans.len());
    let mut requested_states = None;
    for (symbol, concrete, identity, commitment) in plans {
        let existing =
            super::saved_calls::candidate_instance(source.unwrap_or(program), program, &concrete)
                .map(|instance| instance.instance);
        let states = if let Some(instance) = existing {
            let machine = program
                .machine_by_symbol(instance)
                .ok_or_else(|| failure("retained specialization has no machine declaration"))?;
            let states = program.machine_states(machine);
            if states.len() != concrete.template.state_symbols.len() {
                return Err(failure("retained specialization changes its state roster"));
            }
            concrete
                .template
                .state_symbols
                .iter()
                .copied()
                .zip(states.iter().map(|state| state.symbol))
                .collect()
        } else {
            let instance_ordinal = if symbol == template {
                ordinal
            } else {
                program
                    .machine_specializations
                    .iter()
                    .filter(|instance| instance.template == symbol)
                    .count()
            };
            super::body_cloning::clone_single_specialized_machine(
                source,
                program,
                &concrete,
                instance_ordinal,
                identity,
                commitment,
                None,
            )?
        };
        let instance =
            super::saved_calls::candidate_instance(source.unwrap_or(program), program, &concrete)
                .ok_or_else(|| {
                    failure("closed target family application has no retained specialization")
                })?
                .instance;
        instances.push((symbol, instance));
        if symbol == template {
            requested_states = Some(states);
        }
    }
    let closed_symbol = |symbol| {
        instances
            .iter()
            .find(|(original, _)| *original == symbol)
            .map(|(_, instance)| *instance)
            .ok_or_else(|| failure("closed target family is missing a declaration"))
    };
    family.canonical = closed_symbol(family.canonical)?;
    family.members = family
        .members
        .into_iter()
        .map(|member| {
            Ok(TargetMachineFamilyMember {
                target: member.target,
                machine: closed_symbol(member.machine)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    if let Some(existing) = program
        .target_machine_families
        .iter()
        .find(|existing| existing.canonical == family.canonical)
    {
        if existing.members != family.members {
            return Err(failure(
                "closed target family application disagrees with its retained roster",
            ));
        }
    } else {
        program.target_machine_families.push(family);
    }
    requested_states
        .ok_or_else(|| failure("closed target family omitted the requested declaration"))
}

fn failure(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}
