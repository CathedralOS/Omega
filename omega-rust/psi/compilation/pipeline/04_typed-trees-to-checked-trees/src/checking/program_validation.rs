use diagnostics::Diagnostic;
use flow_effects::{OperationalPlan, ServiceReachInferencePlan};
use proof::obligations::ProofPlan;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct PreparedTypedProgram<'program> {
    pub(crate) dependency_body_judgments: Vec<validation::DependencyBodyJudgment>,
    pub(crate) integer_assignments: Vec<proof::checker::CheckedIntegerAssignment>,
    /// Comparisons the proof pass established against retained constant
    /// recipes, for the target closure to discharge.
    pub(crate) constant_recipe_obligations: Vec<checked_trees::ConstantRecipeObligation>,
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operational: OperationalPlan,
    pub(crate) service_reaches: ServiceReachInferencePlan,
    pub(crate) validation_facts: validation::ProgramValidationFacts,
}

pub(crate) fn prepare_typed_program<'program>(
    program: &'program dyn ::validation::CheckingInput,
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
    allow_pending_opaque_copy: bool,
    prior_dependency_bodies: &[validation::DependencyBodyJudgment],
    record_dependency_bodies: bool,
) -> Result<PreparedTypedProgram<'program>, Vec<Diagnostic>> {
    let opaque_properties = if allow_pending_opaque_copy {
        validation::OpaquePropertyValidation::PendingBuildSelection
    } else {
        validation::OpaquePropertyValidation::Required(opaque_property_receipts)
    };
    // Quotient requests are judged after the checked termination facts exist
    // (`lower_typed_trees` calls `admit_checked_quotient_requests` beside
    // `build_check_facts`); validation keeps formation checking here.
    let validated = phase_profile::scope("05.d.2.a.validate", || {
        validation::prepare_specialized_program(
            program,
            opaque_properties,
            prior_dependency_bodies,
            record_dependency_bodies,
        )
    })?;

    let proof_plan = phase_profile::scope("05.d.2.b.build_proof_plan", || {
        proof::obligations::build_proof_plan(program)
    });
    let proof::checker::ProofDecisions {
        integer_assignments,
        constant_recipe_obligations,
    } = phase_profile::scope("05.d.2.c.check_proof_plan", || {
        proof::checker::check_proof_plan_with_decisions(&proof_plan)
    })?;

    let operational = validated.operational;
    phase_profile::scope("05.d.2.d.bare_boundary_trait_values", || {
        validate_no_bare_boundary_trait_values(program)
    })?;

    let dependency_body_judgments = validated.dependency_body_judgments;
    Ok(PreparedTypedProgram {
        dependency_body_judgments,
        integer_assignments,
        constant_recipe_obligations,
        proof_plan,
        operational,
        validation_facts: validated.facts,
        service_reaches: validated.service_reaches,
    })
}

/// A bare boundary trait in value position does not denote a service carrier:
/// fields, parameters, signature slots and returns name `Binding<R>` instead.
/// Borrows (`&T`, `&mut T`) and container members are other positions and stay
/// out of this gate; only the outermost `Named`/`DynamicTrait`/`Generic` head
/// (through `Constrained` shells) is inspected, so `Binding<Console>` itself is
/// never mistaken for a bare trait.
fn validate_no_bare_boundary_trait_values(
    program: &dyn ::validation::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut check = |type_reference: TypeReferenceHandle, position: &dyn Fn() -> String| {
        if let Some(trait_name) = bare_boundary_trait_name(program, type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "{} names bare boundary trait `{trait_name}` in value position; the intrinsic `Binding<R>` carrier is the only service value spelling",
                position()
            )));
        }
    };
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            match member {
                DataMember::Field(field) => check(field.type_reference, &|| {
                    format!(
                        "field `{}` on data `{}`",
                        field.name.as_str(),
                        definition.name.as_str()
                    )
                }),
                DataMember::Variant(variant) => {
                    for payload in program.data_payload_fields(variant) {
                        check(payload.type_reference, &|| {
                            format!(
                                "payload field `{}` on case `{}` of data `{}`",
                                payload.name.as_str(),
                                variant.name.as_str(),
                                definition.name.as_str()
                            )
                        });
                    }
                }
            }
        }
    }
    // A satisfier has no provider-receiver slot: its parameters are exactly
    // its requirement's, so every machine parameter is an ordinary value
    // position here, including a satisfier's first.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                check(parameter.type_reference, &|| {
                    format!(
                        "parameter `{}` on `{}.{}`",
                        parameter.name.as_str(),
                        machine.name.as_str(),
                        state.name.as_str()
                    )
                });
            }
            check(state.return_type, &|| {
                format!(
                    "return type of `{}.{}`",
                    machine.name.as_str(),
                    state.name.as_str()
                )
            });
        }
    }
    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            for parameter in program.state_signature_parameters(signature) {
                check(parameter.type_reference, &|| {
                    format!(
                        "parameter `{}` on signature `{}.{}`",
                        parameter.name.as_str(),
                        trait_definition.name.as_str(),
                        signature.name.as_str()
                    )
                });
            }
            check(signature.return_type, &|| {
                format!(
                    "return type of signature `{}.{}`",
                    trait_definition.name.as_str(),
                    signature.name.as_str()
                )
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn bare_boundary_trait_name(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> Option<String> {
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(type_reference)
        {
            return None;
        }
        let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
                continue;
            }
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::DynamicTrait { symbol, name, .. } => (*symbol, name.as_str()),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => (*base_symbol, base_name.as_str()),
            _ => return None,
        };
        return program
            .trait_definition_by_symbol(symbol)
            .is_some_and(|definition| definition.is_boundary)
            .then(|| name.to_owned());
    }
}
