//! Canonical source erasure for the total direct faithful-definition bridge.

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientCongruenceCorrespondence,
    QuotientContractFactCoordinate, QuotientContractOwner, QuotientCorrespondenceOperationKind,
    QuotientCrashCertificate, QuotientDefineRuntimePosition, QuotientDirectResultFlow,
    QuotientMachineApplication, QuotientPositionalRelation, QuotientPurityCertificate,
    QuotientRelationIdentity, QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
    QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremConclusion,
    QuotientTheoremCorrespondence, QuotientTheoremEligibility, QuotientTheoremEvidence,
    QuotientTheoremParameter, QuotientTheoremParameterRole, QuotientTheoremRelationPremise,
    QuotientTheoremRole,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

use super::correspondence_certificate::{
    QuotientCorrespondenceCertificate, QuotientCorrespondenceEvidence,
};
use super::representative::RepresentativePurity;
use super::result_flow::CompleteSingleStateResultFlow;
use super::theorem_schema::{
    TheoremContractFactLocation, TheoremContractOwner, TheoremParameterRole,
};
use super::{DirectTerminalRelationPlan, ExactQuotientRelation, InputRelation};

pub(in crate::quotients) fn canonical_total_define_correspondence(
    program: &TypedTrees,
    public_machine: &Machine,
    public_state: &State,
    request_expression: ExpressionHandle,
    plan: &DirectTerminalRelationPlan,
    representative_purity: RepresentativePurity,
    result_flow: CompleteSingleStateResultFlow,
) -> Result<CanonicalQuotientCorrespondence, String> {
    let [planned_theorem] = plan.theorem_evidence.as_slice() else {
        return Err("faithful `define` requires exactly one Congruence theorem entry".to_owned());
    };
    if planned_theorem.role != psi_typed_trees::expression::QuotientTheoremRole::Congruence {
        return Err("faithful `define` theorem evidence has a noncanonical role".to_owned());
    }
    let selected_theorem = &planned_theorem.selected_application;
    require_single_entry(program, public_machine, public_state, "public operation")?;
    require_empty_owner_telescope(program, public_machine, "public operation")?;

    let (representative_machine, representative_state) = exact_machine_state(
        program,
        plan.representative.machine_symbol,
        plan.representative.state_symbol,
        "representative",
    )?;
    require_single_entry(
        program,
        representative_machine,
        representative_state,
        "representative",
    )?;
    let (theorem_machine, theorem_state) = exact_machine_state(
        program,
        selected_theorem.machine_symbol,
        selected_theorem.state_symbol,
        "selected theorem",
    )?;
    require_single_entry(program, theorem_machine, theorem_state, "selected theorem")?;

    if !plan.representative.static_application.bindings.is_empty()
        || !plan
            .representative
            .static_application
            .lifetime_arguments
            .is_empty()
        || !selected_theorem.static_application.bindings.is_empty()
        || !plan.theorem_evidence[0]
            .selected_application
            .static_application
            .lifetime_arguments
            .is_empty()
    {
        return Err("the proof-only bridge requires empty closed static applications".to_owned());
    }
    require_empty_owner_telescope(program, representative_machine, "representative")?;
    require_empty_owner_telescope(program, theorem_machine, "selected theorem")?;

    let public_parameters = program
        .state_parameters(public_state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
    require_plain_immutable_parameters(
        public_parameters
            .iter()
            .map(|parameter| (parameter.is_mutable, parameter.is_self)),
        "public operation",
    )?;
    require_plain_immutable_parameters(
        plan.representative
            .parameters
            .iter()
            .map(|parameter| (parameter.is_mutable, parameter.is_self)),
        "representative",
    )?;
    require_plain_immutable_parameters(
        selected_theorem
            .parameters
            .iter()
            .map(|parameter| (parameter.is_mutable, parameter.is_self)),
        "selected theorem",
    )?;

    let certificate = plan
        .correspondence_certificate
        .as_ref()
        .ok_or_else(|| "the complete correspondence certificate is absent".to_owned())?;
    let define_runtime = match &certificate.evidence {
        QuotientCorrespondenceEvidence::Define {
            runtime,
            precondition,
        } => {
            if !precondition.dependent.is_empty() || !precondition.fixed.is_empty() {
                return Err(
                    "the proof-only bridge admits no public or representative preconditions"
                        .to_owned(),
                );
            }
            runtime
        }
        QuotientCorrespondenceEvidence::DirectLift { .. }
        | QuotientCorrespondenceEvidence::DirectLiftWithTransport { .. } => {
            return Err("the proof-only bridge admits faithful `define` only".to_owned());
        }
    };
    require_empty_partition(plan.public_precondition.as_ref(), "public")?;
    require_empty_partition(plan.representative_precondition.as_ref(), "representative")?;
    if !plan.expected_theorem_schema.legality_premises.is_empty()
        || !certificate.theorem.legality_premises.is_empty()
    {
        return Err("the proof-only bridge admits no theorem legality premises".to_owned());
    }

    if plan.representative_termination.is_none()
        || representative_purity.machine_symbol != plan.representative.machine_symbol
        || representative_purity.state_symbol != plan.representative.state_symbol
        || planned_theorem.termination.is_none()
        || planned_theorem.purity.is_none()
        || !planned_theorem.crash_free
    {
        return Err("purity, termination, or theorem crash eligibility is incomplete".to_owned());
    }
    if result_flow.root.request_expression != request_expression
        || result_flow.root.alias_count != 0
        || result_flow.machine_symbol != public_machine.symbol
        || result_flow.state_symbol != public_state.symbol
    {
        return Err("direct result-flow certificate identity drifted".to_owned());
    }

    if public_parameters.len() != plan.representative.parameters.len()
        || define_runtime.positions.len() != public_parameters.len()
        || plan.input_relations.len() != public_parameters.len()
    {
        return Err("faithful runtime correspondence arity drifted".to_owned());
    }
    let runtime_positions = define_runtime
        .positions
        .iter()
        .enumerate()
        .map(|(position, runtime)| {
            if runtime.public_parameter != public_parameters[position].symbol
                || runtime.representative_parameter
                    != plan.representative.parameters[position].symbol
            {
                return Err("faithful runtime correspondence is not position preserving".to_owned());
            }
            Ok(QuotientDefineRuntimePosition {
                public_position: to_u32(position, "public parameter position")?,
                representative_position: to_u32(position, "representative parameter position")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let input_relations = plan
        .input_relations
        .iter()
        .enumerate()
        .map(|(position, relation)| match relation {
            InputRelation::Quotient(relation) => Ok(QuotientPositionalRelation::Quotient(
                relation_identity(program, *relation)?,
            )),
            InputRelation::ExactEquality(public_type) => {
                let public_type = canonical_type(program, *public_type)?;
                let representative_type = canonical_type(
                    program,
                    plan.representative.parameters[position].type_reference,
                )?;
                if public_type != representative_type {
                    return Err("ordinary faithful input type identity drifted".to_owned());
                }
                Ok(QuotientPositionalRelation::ExactEquality {
                    public_type,
                    representative_type,
                })
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let result_relation = relation_identity(program, plan.result_relation)?;
    let theorem = theorem_correspondence(program, plan, certificate)?;
    let statement_position = program
        .statement_table
        .statements(public_state.statement_nodes)
        .len()
        .checked_sub(1)
        .ok_or_else(|| "direct result state has no terminal statement".to_owned())?;

    Ok(CanonicalQuotientCorrespondence {
        operation_kind: QuotientCorrespondenceOperationKind::Define,
        public_operation: callable_identity(program, public_machine)?,
        representative: machine_application(program, representative_machine)?,
        input_relations,
        result_relation,
        runtime_positions,
        theorem_evidence: vec![QuotientTheoremEvidence {
            role: QuotientTheoremRole::Congruence,
            selected_application: machine_application(program, theorem_machine)?,
            correspondence: QuotientTheoremCorrespondence::Congruence(theorem),
            eligibility: QuotientTheoremEligibility {
                purity: QuotientPurityCertificate::PureClosure,
                termination: QuotientTerminationCertificate::Unconditional,
                crash: QuotientCrashCertificate::CrashFree,
            },
        }],
        representative_eligibility: QuotientRepresentativeEligibility {
            purity: QuotientPurityCertificate::PureClosure,
            termination: QuotientTerminationCertificate::Unconditional,
        },
        result_flow: QuotientDirectResultFlow {
            state_position: 0,
            statement_position: to_u32(statement_position, "result statement position")?,
        },
    })
}

fn theorem_correspondence(
    program: &TypedTrees,
    plan: &DirectTerminalRelationPlan,
    certificate: &QuotientCorrespondenceCertificate,
) -> Result<QuotientCongruenceCorrespondence, String> {
    let expected = &plan.expected_theorem_schema;
    let verified = &certificate.theorem;
    let selected_theorem = &plan.theorem_evidence[0].selected_application;
    if verified.theorem_machine_symbol != selected_theorem.machine_symbol
        || verified.theorem_state_symbol != selected_theorem.state_symbol
        || verified.parameters.len() != expected.parameters.len()
    {
        return Err("verified theorem identity or parameter roster drifted".to_owned());
    }
    let parameters = expected
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            let verified_parameter = verified
                .parameters
                .iter()
                .find(|parameter| parameter.expected_position == position)
                .ok_or_else(|| "verified theorem parameter coordinate is absent".to_owned())?;
            if verified_parameter.theorem_symbol != selected_theorem.parameters[position].symbol {
                return Err("verified theorem parameter symbol drifted".to_owned());
            }
            let input_position =
                to_u32(parameter.representative_position, "theorem input position")?;
            let role = match parameter.role {
                TheoremParameterRole::QuotientLeft => {
                    QuotientTheoremParameterRole::QuotientLeft { input_position }
                }
                TheoremParameterRole::QuotientRight => {
                    QuotientTheoremParameterRole::QuotientRight { input_position }
                }
                TheoremParameterRole::Shared => {
                    QuotientTheoremParameterRole::Shared { input_position }
                }
            };
            Ok(QuotientTheoremParameter {
                theorem_position: to_u32(position, "theorem parameter position")?,
                role,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let relation_premises = expected
        .relation_premises
        .iter()
        .enumerate()
        .map(|(position, premise)| {
            let verified_premise = verified
                .relation_premises
                .iter()
                .find(|premise| premise.expected_position == position)
                .ok_or_else(|| "verified theorem relation premise is absent".to_owned())?;
            Ok(QuotientTheoremRelationPremise {
                expected_position: to_u32(position, "relation premise position")?,
                actual: contract_coordinate(verified_premise.actual)?,
                relation: canonical_symbol(program, premise.relation.relation_symbol)?,
                left_parameter: to_u32(premise.left_parameter, "left theorem parameter")?,
                right_parameter: to_u32(premise.right_parameter, "right theorem parameter")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(QuotientCongruenceCorrespondence {
        parameters,
        relation_premises,
        legality_premises: Vec::new(),
        conclusion: QuotientTheoremConclusion {
            actual: contract_coordinate(verified.conclusion)?,
            relation: canonical_symbol(program, expected.result_relation.relation_symbol)?,
            left: QuotientRepresentativeApplication {
                arguments: expected
                    .left_application
                    .arguments
                    .iter()
                    .map(|position| to_u32(*position, "left application argument"))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            right: QuotientRepresentativeApplication {
                arguments: expected
                    .right_application
                    .arguments
                    .iter()
                    .map(|position| to_u32(*position, "right application argument"))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        },
    })
}

fn relation_identity(
    program: &TypedTrees,
    relation: ExactQuotientRelation,
) -> Result<QuotientRelationIdentity, String> {
    let quotient = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == relation.quotient_symbol)
        .ok_or_else(|| "quotient relation lost its declaration".to_owned())?;
    if !program.data_type_parameters(quotient).is_empty() {
        return Err("the proof-only bridge excludes generic quotient declarations".to_owned());
    }
    let formation = quotient
        .quotient
        .as_ref()
        .ok_or_else(|| "quotient relation lost its formation".to_owned())?;
    if formation.relation_symbol != relation.relation_symbol {
        return Err("quotient relation identity drifted from formation".to_owned());
    }
    Ok(QuotientRelationIdentity {
        quotient_declaration: canonical_symbol(program, relation.quotient_symbol)?,
        quotient_type: canonical_type(program, relation.quotient_type)?,
        carrier_type: canonical_type(program, formation.carrier)?,
        relation: canonical_symbol(program, relation.relation_symbol)?,
    })
}

fn require_empty_partition(
    partition: Option<&super::precondition::RepresentativePreconditionPartition>,
    side: &str,
) -> Result<(), String> {
    let partition = partition.ok_or_else(|| format!("{side} precondition partition is absent"))?;
    if !partition.dependent.is_empty() || !partition.fixed.is_empty() {
        return Err(format!(
            "the proof-only bridge admits no {side} preconditions"
        ));
    }
    Ok(())
}

fn exact_machine_state<'a>(
    program: &'a TypedTrees,
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    label: &str,
) -> Result<(&'a Machine, &'a State), String> {
    let matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == machine_symbol)
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter(move |state| state.symbol == state_symbol)
                .map(move |state| (machine, state))
        })
        .collect::<Vec<_>>();
    let [found] = matches.as_slice() else {
        return Err(format!("{label} does not resolve exactly"));
    };
    Ok(*found)
}

fn require_single_entry(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    label: &str,
) -> Result<(), String> {
    let [entry] = program.machine_states(machine) else {
        return Err(format!("{label} must have exactly one state"));
    };
    if entry.symbol != state.symbol {
        return Err(format!("{label} retained a non-entry state"));
    }
    Ok(())
}

fn require_empty_owner_telescope(
    program: &TypedTrees,
    machine: &Machine,
    label: &str,
) -> Result<(), String> {
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        return Err(format!("{label} must have an empty static telescope"));
    }
    Ok(())
}

fn require_plain_immutable_parameters(
    parameters: impl IntoIterator<Item = (bool, bool)>,
    label: &str,
) -> Result<(), String> {
    if parameters
        .into_iter()
        .any(|(is_mutable, is_self)| is_mutable || is_self)
    {
        return Err(format!(
            "{label} parameters must be immutable and non-attached"
        ));
    }
    Ok(())
}

fn machine_application(
    program: &TypedTrees,
    machine: &Machine,
) -> Result<QuotientMachineApplication, String> {
    Ok(QuotientMachineApplication {
        callable: callable_identity(program, machine)?,
        static_application: QuotientStaticApplication {
            bindings: Vec::new(),
        },
    })
}

fn callable_identity(
    program: &TypedTrees,
    machine: &Machine,
) -> Result<QuotientCallableIdentity, String> {
    Ok(QuotientCallableIdentity {
        declaration: canonical_symbol(program, machine.symbol)?,
        overload: program
            .normalized_machine_overload_identity(machine)
            .ok_or_else(|| "callable has no canonical overload identity".to_owned())?
            .identity(),
    })
}

fn canonical_symbol(
    program: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<String, String> {
    program.normalized_hermetic_symbol_identity(symbol)
}

fn canonical_type(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<String, String> {
    let identity = program.package_qualified_type_identity(type_reference);
    if identity.as_str().is_empty() || identity.as_str().contains("unresolved-owner") {
        return Err("type identity is not hermetic".to_owned());
    }
    Ok(identity.into_string())
}

fn contract_coordinate(
    location: TheoremContractFactLocation,
) -> Result<QuotientContractFactCoordinate, String> {
    Ok(QuotientContractFactCoordinate {
        owner: match location.owner {
            TheoremContractOwner::Machine => QuotientContractOwner::Machine,
            TheoremContractOwner::State => QuotientContractOwner::State,
        },
        contract_position: to_u32(location.contract_position, "contract position")?,
        fact_position: to_u32(location.fact_position, "contract fact position")?,
    })
}

fn to_u32(value: usize, label: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("{label} does not fit canonical identity"))
}
