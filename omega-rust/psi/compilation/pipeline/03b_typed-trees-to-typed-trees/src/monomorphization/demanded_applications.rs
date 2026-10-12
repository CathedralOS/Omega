//! Explicit source application demands use ordinary specialization storage.
//!
//! A non-executing Build reference keeps its original immutable payload owner.
//! This owner copies payloads through the existing table copier, instantiates
//! the source declaration through the ordinary clone producer, and retains the
//! exact source-to-instance association. It creates no execution call, checked
//! conformance or provider choice. Immutable checking still judges every clone.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::static_applications::{
    InstantiatedStaticApplication, StaticApplicationDemand, StaticMachineBinding,
    StaticMachineBindingValue, static_application_identities_equal, static_application_identity,
};
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

use super::body_cloning::{clone_specialized_machine, copy_binding_type_payloads};
use super::{Candidate, candidate};

pub(crate) fn instantiate(
    program: &mut TypedTrees,
    demands: &[StaticApplicationDemand],
) -> Result<(), Vec<Diagnostic>> {
    for demand in demands {
        instantiate_one(program, demand)?;
    }
    Ok(())
}

pub(super) fn instantiate_one(
    program: &mut TypedTrees,
    demand: &StaticApplicationDemand,
) -> Result<(), Vec<Diagnostic>> {
    let payload = demand.owner.as_ref();
    let application = payload
        .expression_table
        .static_machine_application(demand.application);
    if !demand.declaration.is_valid()
        || application.binding_declaration != demand.declaration
        || application.registration_operation != demand.declaration
        || program
            .symbols
            .symbol_provenance_source_span(demand.declaration)
            != payload
                .symbols
                .symbol_provenance_source_span(demand.declaration)
        || program.symbols.symbol_package_identity(demand.declaration)
            != payload.symbols.symbol_package_identity(demand.declaration)
        || program.symbols.get(demand.declaration).kind
            != payload.symbols.get(demand.declaration).kind
    {
        return Err(vec![Diagnostic::error(
            "source application demand lost its original declaration correspondence",
        )]);
    }
    let identity = static_application_identity(payload, demand.application)
        .map_err(|diagnostic| vec![diagnostic])?;
    let previous = program
        .instantiated_static_applications()
        .find(|(_, produced)| {
            produced.demand.declaration == demand.declaration
                && std::sync::Arc::ptr_eq(&produced.demand.owner, &demand.owner)
                && static_application_identity(&produced.demand.owner, produced.demand.application)
                    .is_ok_and(|previous| previous == identity)
        })
        .map(|(_, produced)| (produced.demand.application, produced.instance));
    if let Some((application, instance)) = previous {
        if application != demand.application {
            // Distinct source occurrences can share a public instance while
            // retaining their own exact input receipt and lexical custody.
            program.retain_instantiated_static_application(InstantiatedStaticApplication {
                demand: demand.clone(),
                instance,
            });
        }
        return Ok(());
    }
    let bindings = payload
        .expression_table
        .static_machine_application_bindings(demand.application);
    if !super::application_arguments::static_application_is_closed(payload, demand.application) {
        return Err(vec![incomplete()]);
    }
    let applied_trait = program
        .trait_machine_signature(demand.declaration)
        .is_some_and(|(definition, signature)| {
            (application.selected_interface.is_valid()
                && application.selected_interface != definition.symbol)
                || !bindings.is_empty()
                || !program.trait_type_parameters(definition).is_empty()
                || !program
                    .state_signature_type_parameters(signature)
                    .is_empty()
                || !definition.conformance_bounds.is_empty()
                || !definition.lifetime_parameters.is_empty()
        });
    let instance = if applied_trait {
        let mut matching = Vec::new();
        for (_, row) in program.instantiated_requirement_signatures.iter() {
            if row.requirement_template == demand.declaration
                && static_application_identities_equal(
                    payload,
                    demand.application,
                    program,
                    row.binding_application,
                )
                .map_err(|diagnostic| vec![diagnostic])?
            {
                matching.push(program.trait_machine_signatures.get(row.signature).symbol);
            }
        }
        match matching.as_slice() {
            [instance] => *instance,
            [] => super::requirement_signatures::produce(program, payload, demand, bindings)
                .map_err(|diagnostic| vec![diagnostic])?,
            _ => {
                return Err(vec![Diagnostic::error(
                    "source application demand has more than one exact public header",
                )]);
            }
        }
    } else if bindings.is_empty() {
        let open = program
            .machine(demand.declaration)
            .is_some_and(|machine| machine.has_specialization_parameters());
        if open {
            return Err(vec![incomplete()]);
        }
        demand.declaration
    } else {
        let Some(machine_index) = program.machine_position_by_symbol(demand.declaration) else {
            return Err(vec![Diagnostic::error(
                "source application demand has no original callable declaration",
            )]);
        };
        let machine = &program.machines()[machine_index];
        if !machine.has_specialization_parameters() {
            return Err(vec![Diagnostic::error(
                "source application demand supplied a tuple to a nongeneric declaration",
            )]);
        }
        // Both owners retain the same source declarations, but their type
        // interner slots are independent after copying or live graph mutation.
        // Compare through the issuing owners before reusing an exact instance.
        let mut matching = Vec::new();
        for receipt in &program.machine_specializations {
            if receipt.template != demand.declaration || !receipt.binding_application.is_valid() {
                continue;
            }
            if static_application_identities_equal(
                payload,
                demand.application,
                program,
                receipt.binding_application,
            )
            .map_err(|diagnostic| vec![diagnostic])?
            {
                matching.push(receipt.instance);
            }
        }
        match matching.as_slice() {
            [instance] => *instance,
            [] => produce_machine(program, payload, demand, machine_index, bindings)?,
            _ => {
                return Err(vec![Diagnostic::error(
                    "source application demand has more than one exact produced instance",
                )]);
            }
        }
    };
    program.retain_instantiated_static_application(InstantiatedStaticApplication {
        demand: demand.clone(),
        instance,
    });
    Ok(())
}

fn produce_machine(
    program: &mut TypedTrees,
    payload: &TypedTrees,
    demand: &StaticApplicationDemand,
    machine_index: usize,
    bindings: &[StaticMachineBinding],
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    // Source and executable payload handles never alias by arena number.
    // Work against an immutable declaration snapshot using the existing copier.
    let mut source = program.clone();
    let (bindings, application) = copy_application_payload(
        &mut source,
        payload,
        demand.declaration,
        demand.application,
        bindings,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let mut candidate = candidate::from_machine(&source, machine_index);
    candidate.binding_application = application;
    bind_candidate(&mut candidate, &bindings).map_err(|diagnostic| vec![diagnostic])?;
    let machine = &source.machines()[machine_index];
    let identity = normalized_machine_identity(&source, machine).ok_or_else(|| {
        vec![Diagnostic::error(
            "source application template has no normalized declaration identity",
        )]
    })?;
    let commitment = accepted_template_commitment(&source, machine_index);

    let specialization =
        clone_specialized_machine(Some(&source), program, &candidate, identity, commitment)?;
    // A family closes every member. Retain the exact requested declaration
    // returned by that producer, never a member chosen by arena append order.
    Ok(specialization.instance)
}

pub(super) fn copy_application_payload(
    source: &mut TypedTrees,
    payload: &TypedTrees,
    declaration: SymbolHandle,
    original_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    bindings: &[StaticMachineBinding],
) -> Result<
    (
        Vec<StaticMachineBinding>,
        typed_trees::static_applications::StaticMachineApplicationHandle,
    ),
    Diagnostic,
> {
    let mut bindings = bindings.to_vec();
    copy_binding_type_payloads(Some(payload), source, &mut bindings, &[]);
    let application = source.expression_table.retain_static_machine_application(
        declaration,
        payload
            .expression_table
            .static_machine_application(original_application)
            .target
            .clone(),
        &[],
    );
    let selected = source
        .expression_table
        .static_machine_application_mut(application);
    selected.binding_declaration = declaration;
    selected.selected_interface = payload
        .expression_table
        .static_machine_application(original_application)
        .selected_interface;
    selected.lexical_owner = payload
        .expression_table
        .static_machine_application(original_application)
        .lexical_owner;
    let captured = payload
        .expression_table
        .static_machine_application(original_application)
        .captured_lexical_scope;
    if captured.is_valid() {
        let scope = payload
            .captured_static_lexical_scope(captured)
            .ok_or_else(|| {
                Diagnostic::error("source application lost its original lexical capture")
            })?;
        let captured = super::captured_lexical_scopes::retain(source, scope)?;
        source
            .expression_table
            .static_machine_application_mut(application)
            .captured_lexical_scope = captured;
    }
    source
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    source
        .expression_table
        .set_static_machine_application_lifetimes(
            application,
            payload
                .expression_table
                .static_machine_application_lifetimes(original_application),
        );
    Ok((bindings, application))
}

fn bind_candidate(
    candidate: &mut Candidate<'_>,
    bindings: &[StaticMachineBinding],
) -> Result<(), Diagnostic> {
    let find = |parameter| {
        bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
            .map(|binding| &binding.value)
    };
    for ((parameter, _), slot) in candidate
        .template
        .type_parameters
        .iter()
        .zip(&mut candidate.type_bindings)
    {
        let Some(StaticMachineBindingValue::Type(reference)) = find(*parameter) else {
            return Err(incomplete());
        };
        *slot = Some(*reference);
    }
    for ((parameter, _, _), slot) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&mut candidate.const_bindings)
    {
        let Some(StaticMachineBindingValue::Const(reference)) = find(*parameter) else {
            return Err(incomplete());
        };
        *slot = Some(*reference);
    }
    for ((parameter, _, _), slot) in candidate
        .template
        .machine_parameters
        .iter()
        .zip(&mut candidate.machine_bindings)
    {
        let Some(StaticMachineBindingValue::Machine(argument)) = find(*parameter) else {
            return Err(incomplete());
        };
        *slot = Some(argument.clone());
    }
    for (parameter, slot) in candidate
        .template
        .evidence_parameters
        .iter()
        .zip(&mut candidate.evidence_bindings)
    {
        let Some(StaticMachineBindingValue::Conformance(argument)) = find(parameter.binder) else {
            return Err(incomplete());
        };
        *slot = Some(argument.clone());
    }
    if bindings.len()
        != candidate.type_bindings.len()
            + candidate.const_bindings.len()
            + candidate.machine_bindings.len()
            + candidate.evidence_bindings.len()
    {
        return Err(incomplete());
    }
    Ok(())
}

fn incomplete() -> Diagnostic {
    Diagnostic::error(
        "source application demand does not own one complete declared static substitution",
    )
}
