//! Non-authoritative relation and same-state result-flow planning for quotient
//! requests.
//!
//! The plan retains exact quotient TYPE identity as well as relation symbol so
//! two quotients over one carrier cannot collapse. It grants no execution
//! authority and deliberately refuses nested/adapted result flow.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    QuotientOperationKind, QuotientOperationRequest, StaticMachineArgument, TableCallExpression,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceHandle;
use std::fmt;

mod correspondence_certificate;
mod precondition;
mod proof_fact_identity;
mod representative;
mod result_flow;
mod runtime_correspondence;
mod static_application;
mod theorem;
mod theorem_schema;
mod theorem_schema_verification;

use correspondence_certificate::{
    DirectLiftPreconditionImplication, QuotientCorrespondenceCertificate,
    compose_define_correspondence_certificate, compose_lift_correspondence_certificate,
    derive_direct_lift_precondition_implication,
};
use precondition::{
    DefinePreconditionCorrespondence, RepresentativePreconditionPartition,
    derive_define_precondition_correspondence, derive_direct_lift_public_precondition_partition,
    derive_public_precondition_partition, derive_representative_precondition_partition,
};
#[cfg(test)]
use precondition::{RepresentativeContractFactLocation, RepresentativeContractOwner};
pub(super) use representative::pure_representative_effect;
#[cfg(test)]
use representative::{RepresentativePurity, RepresentativeRuntimeParameter};
use representative::{
    RepresentativeTelescope, RepresentativeTermination, derive_representative_telescope,
    representative_machine_state, unconditional_representative_termination,
};
use runtime_correspondence::DefineRuntimePosition;
use runtime_correspondence::{
    DefineRuntimeCorrespondence, DirectLiftRuntimeCorrespondence,
    closed_scalar_literal_for_representative, derive_define_runtime_correspondence,
    derive_direct_lift_runtime_correspondence,
};
#[cfg(test)]
use runtime_correspondence::{DirectLiftArgumentSource, DirectLiftRuntimePosition};
#[cfg(test)]
use theorem::derive_selected_theorem_telescope;
use theorem::{SelectedTheoremPurity, SelectedTheoremTelescope, SelectedTheoremTermination};
use theorem_schema::{ExpectedTheoremSchema, derive_expected_theorem_schema};
use theorem_schema_verification::{VerifiedTheoremSchema, verify_selected_theorem_schema};

#[cfg(test)]
use result_flow::{
    CompleteSingleStateResultFlow, CompleteStateForwardingResultFlow,
    ImmutableAliasFallthroughRoot, StateForwardingEdge, immutable_alias_fallthrough_root,
};
pub(super) use result_flow::{
    complete_single_state_result_flow, complete_state_forwarding_result_flow,
    fallthrough_result_root,
};
#[cfg(test)]
use static_application::{
    derive_exact_representative_static_application, substituted_type_matches,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactQuotientRelation {
    pub(super) quotient_type: TypeReferenceHandle,
    pub(super) quotient_symbol: SymbolHandle,
    pub(super) relation_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputRelation {
    Quotient(ExactQuotientRelation),
    /// Non-quotient operands remain part of the pointwise relation through
    /// exact equality. They must never disappear into an implicit `true`.
    ExactEquality(TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectTerminalRelationPlan {
    /// One entry per authored runtime argument. Quotient positions use their
    /// exact selected relation; ordinary positions use exact typed equality.
    pub(super) input_relations: Vec<InputRelation>,
    pub(super) result_relation: ExactQuotientRelation,
    pub(super) representative: RepresentativeTelescope,
    pub(super) representative_termination: Option<RepresentativeTermination>,
    /// Exact explicitly selected resultless theorem-machine application.
    pub(super) selected_theorem: SelectedTheoremTelescope,
    pub(super) selected_theorem_termination: Option<SelectedTheoremTermination>,
    pub(super) selected_theorem_purity: Option<SelectedTheoremPurity>,
    pub(super) selected_theorem_crash_free: bool,
    /// Exact compiler-derived contract expected from the selected theorem.
    expected_theorem_schema: ExpectedTheoremSchema,
    /// Structural verification pairs every expected row with one exact
    /// selected-theorem row. An error remains diagnostic planning state only;
    /// the correspondence rung must consume the certificate and cannot infer
    /// authority from the expected schema alone.
    pub(super) theorem_schema_verification: Result<VerifiedTheoremSchema, RelationPlanError>,
    pub(super) direct_lift_correspondence: Option<DirectLiftRuntimeCorrespondence>,
    pub(super) define_correspondence: Option<DefineRuntimeCorrespondence>,
    pub(super) public_precondition: Option<RepresentativePreconditionPartition>,
    pub(super) representative_precondition: Option<RepresentativePreconditionPartition>,
    pub(super) direct_lift_precondition_implication: Option<DirectLiftPreconditionImplication>,
    pub(super) define_precondition_correspondence: Option<DefinePreconditionCorrespondence>,
    /// Exact theorem + bounded correspondence composition only. Fixed call
    /// obligations, general implication/adaptation, and Terminal replay remain
    /// outside this non-executable certificate.
    pub(super) correspondence_certificate: Option<QuotientCorrespondenceCertificate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepresentativeStaticBindingKind {
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeStaticBinding {
    pub(super) parameter: SymbolHandle,
    pub(super) kind: RepresentativeStaticBindingKind,
    pub(super) argument: StaticMachineArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeStaticApplication {
    pub(super) lifetime_arguments: Vec<Identifier>,
    pub(super) bindings: Vec<RepresentativeStaticBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RelationPlanError {
    UnresolvedArgumentType(usize),
    UnresolvedInputRelationApplication(usize),
    ResultIsNotQuotient,
    UnresolvedResultRelationApplication,
    RepresentativeEntryDoesNotResolveExactly,
    RepresentativeResultTypeIsUnresolved,
    RepresentativeStaticArityMismatch,
    RepresentativeStaticArgumentCategoryMismatch(usize),
    RepresentativeStaticArgumentIsOpen(usize),
    RepresentativeLifetimeApplicationRequiresElision,
    RepresentativePropositionApplicationUnsupported(usize),
    TheoremEntryDoesNotResolveExactly,
    TheoremMustBeCheckedBody,
    TheoremMustBeResultless,
    TheoremStaticApplicationInvalid,
    TheoremSchemaRuntimeArityMismatch,
    TheoremSchemaParameterArityMismatch,
    TheoremSchemaConstParameter(usize),
    TheoremSchemaAttachedReceiver(usize),
    TheoremSchemaParameterModeMismatch(usize),
    TheoremSchemaParameterTypeMismatch(usize),
    TheoremSchemaNamedEvidenceLane,
    TheoremSchemaUnexpectedContractKind,
    TheoremSchemaPremiseCountMismatch,
    TheoremSchemaRelationPremiseMismatch(usize),
    TheoremSchemaLegalityPremiseMismatch(usize),
    TheoremSchemaConclusionCountMismatch,
    TheoremSchemaConclusionMismatch,
    DirectLiftOwnerRequiresSubstitution,
    DirectLiftRuntimeArityMismatch,
    DirectLiftParameterIdentityNotUnique,
    DirectLiftArgumentIsNotPublicParameter(usize),
    DirectLiftParameterModeMismatch(usize),
    DirectLiftParameterTypeMismatch(usize),
    DirectLiftLiteralTargetMismatch(usize),
    DirectLiftResultTypeMismatch,
    DirectLiftLeftPreconditionNotImplied(usize),
    DirectLiftRightPreconditionNotImplied(usize),
    DirectLiftTheoremLegalityMismatch,
    DefineOwnerRequiresSubstitution,
    DefineRuntimeArityMismatch,
    DefineParameterIdentityNotUnique,
    DefineArgumentIsNotPublicParameter(usize),
    DefineArgumentOrderMismatch(usize),
    DefineParameterModeMismatch(usize),
    DefineParameterTypeMismatch(usize),
    DefineResultTypeMismatch,
    PreconditionDependencyUnresolved,
    DefinePreconditionMismatch,
}

impl fmt::Display for RelationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedArgumentType(position) => write!(
                formatter,
                "argument {position} has no exact declared type; adapted lift arguments require later expression typing"
            ),
            Self::UnresolvedInputRelationApplication(position) => write!(
                formatter,
                "argument {position}'s quotient relation has an open binder application that requires the representative-operation telescope"
            ),
            Self::ResultIsNotQuotient => formatter
                .write_str("the enclosing state's exact result type is not a formed quotient"),
            Self::UnresolvedResultRelationApplication => formatter.write_str(
                "the result quotient relation has an open binder application that requires the representative-operation result telescope",
            ),
            Self::RepresentativeEntryDoesNotResolveExactly => formatter.write_str(
                "the retained representative entry symbol does not resolve to exactly one machine state",
            ),
            Self::RepresentativeResultTypeIsUnresolved => formatter.write_str(
                "the representative operation has no exact result type",
            ),
            Self::RepresentativeStaticArityMismatch => formatter.write_str(
                "the representative static application does not exactly match its declaration parameter arity",
            ),
            Self::RepresentativeStaticArgumentCategoryMismatch(position) => write!(
                formatter,
                "representative static argument {position} has the wrong declaration category"
            ),
            Self::RepresentativeStaticArgumentIsOpen(position) => write!(
                formatter,
                "representative static argument {position} is not one closed application"
            ),
            Self::RepresentativeLifetimeApplicationRequiresElision => formatter.write_str(
                "representative lifetime arguments require the ordinary call-site elision judgment",
            ),
            Self::RepresentativePropositionApplicationUnsupported(position) => write!(
                formatter,
                "representative proposition argument {position} has no closed application boundary yet"
            ),
            Self::TheoremEntryDoesNotResolveExactly => formatter.write_str(
                "the selected theorem does not resolve to one exact machine entry",
            ),
            Self::TheoremMustBeCheckedBody => formatter.write_str(
                "the selected theorem must be one bodyful checked machine; boundary, accepted, and external proof sources cannot license quotient substitution",
            ),
            Self::TheoremMustBeResultless => formatter.write_str(
                "the selected theorem must return Unit; a result-bearing machine is not proof-static authority",
            ),
            Self::TheoremStaticApplicationInvalid => formatter.write_str(
                "the selected theorem's complete static application is open, mismatched, or otherwise unresolved",
            ),
            Self::TheoremSchemaRuntimeArityMismatch => formatter.write_str(
                "the representative runtime telescope does not match the quotient operation argument telescope",
            ),
            Self::TheoremSchemaParameterArityMismatch => formatter.write_str(
                "the selected theorem's ordinary parameter arity does not exactly match the derived theorem schema",
            ),
            Self::TheoremSchemaConstParameter(position) => write!(
                formatter,
                "selected theorem parameter {position} is const; theorem-schema parameters must all be ordinary proof-static values"
            ),
            Self::TheoremSchemaAttachedReceiver(position) => write!(
                formatter,
                "selected theorem parameter {position} is an attached receiver; theorem-schema parameters must all be ordinary explicit values"
            ),
            Self::TheoremSchemaParameterModeMismatch(position) => write!(
                formatter,
                "selected theorem parameter {position} changes the derived representative access mode"
            ),
            Self::TheoremSchemaParameterTypeMismatch(position) => write!(
                formatter,
                "selected theorem parameter {position} does not have the exact derived type after both static applications are substituted"
            ),
            Self::TheoremSchemaNamedEvidenceLane => formatter.write_str(
                "the selected theorem schema contains a named contract binding and would expose a runtime evidence lane",
            ),
            Self::TheoremSchemaUnexpectedContractKind => formatter.write_str(
                "the selected theorem schema contains a result-case or crash contract outside the exact requires/ensures theorem shape",
            ),
            Self::TheoremSchemaPremiseCountMismatch => formatter.write_str(
                "the selected theorem's requires fact count does not exactly match all relation and representative-legality premises",
            ),
            Self::TheoremSchemaRelationPremiseMismatch(position) => write!(
                formatter,
                "selected theorem relation premise {position} is missing, duplicated, finer, or names a different relation"
            ),
            Self::TheoremSchemaLegalityPremiseMismatch(position) => write!(
                formatter,
                "selected theorem representative-legality premise {position} does not exactly match its substituted representative requires fact"
            ),
            Self::TheoremSchemaConclusionCountMismatch => formatter.write_str(
                "the selected theorem must have exactly one ordinary ensures fact and no other conclusion lane",
            ),
            Self::TheoremSchemaConclusionMismatch => formatter.write_str(
                "the selected theorem conclusion is not the exact result relation over the two exact representative applications",
            ),
            Self::DirectLiftOwnerRequiresSubstitution => formatter.write_str(
                "the direct-lift precondition rung does not yet substitute a generic quotient owner",
            ),
            Self::DirectLiftRuntimeArityMismatch => formatter.write_str(
                "the bounded direct-lift rung requires equal authored-call, representative, and relation arity; the authored call may omit or repeat public parameters",
            ),
            Self::DirectLiftParameterIdentityNotUnique => formatter.write_str(
                "the bounded direct-lift rung requires unique public and representative parameter identities",
            ),
            Self::DirectLiftArgumentIsNotPublicParameter(position) => write!(
                formatter,
                "direct-lift argument {position} is neither a direct public parameter nor an admitted closed scalar literal"
            ),
            Self::DirectLiftParameterModeMismatch(position) => write!(
                formatter,
                "direct-lift parameter {position} changes mutable/borrow mode"
            ),
            Self::DirectLiftParameterTypeMismatch(position) => write!(
                formatter,
                "direct-lift parameter {position} does not map its exact quotient carrier or ordinary type to the representative parameter"
            ),
            Self::DirectLiftLiteralTargetMismatch(position) => write!(
                formatter,
                "direct-lift literal {position} is not a closed boolean or explicitly landed, in-range integer of the representative parameter's exact primitive type and arithmetic domain"
            ),
            Self::DirectLiftResultTypeMismatch => formatter.write_str(
                "the direct-lift result quotient carrier does not match the representative result",
            ),
            Self::DirectLiftLeftPreconditionNotImplied(position) => write!(
                formatter,
                "public Q does not contain dependent representative P fact {position} after exact left-application substitution"
            ),
            Self::DirectLiftRightPreconditionNotImplied(position) => write!(
                formatter,
                "public Q does not contain dependent representative P fact {position} after exact right-application substitution"
            ),
            Self::DirectLiftTheoremLegalityMismatch => formatter.write_str(
                "the direct-lift implication row does not join to one exact verified theorem-legality coordinate",
            ),
            Self::DefineOwnerRequiresSubstitution => formatter.write_str(
                "the quotient-facing definition is generic and requires exact owner-telescope substitution",
            ),
            Self::DefineRuntimeArityMismatch => formatter.write_str(
                "the public, authored-call, and representative runtime telescopes have different arity",
            ),
            Self::DefineParameterIdentityNotUnique => formatter.write_str(
                "the public or representative runtime telescope repeats one parameter identity",
            ),
            Self::DefineArgumentIsNotPublicParameter(position) => write!(
                formatter,
                "define argument {position} is not one exact direct public parameter"
            ),
            Self::DefineArgumentOrderMismatch(position) => write!(
                formatter,
                "define argument {position} does not name the public parameter at the same position"
            ),
            Self::DefineParameterModeMismatch(position) => write!(
                formatter,
                "define parameter {position} changes mutable/borrow mode"
            ),
            Self::DefineParameterTypeMismatch(position) => write!(
                formatter,
                "define parameter {position} does not map its exact quotient carrier or ordinary type to the representative parameter"
            ),
            Self::DefineResultTypeMismatch => formatter.write_str(
                "the exact quotient result carrier does not match the representative result",
            ),
            Self::PreconditionDependencyUnresolved => formatter.write_str(
                "a quotient-facing or representative precondition contains an unresolved value identity and cannot be partitioned by quotient-bearing position",
            ),
            Self::DefinePreconditionMismatch => formatter.write_str(
                "the quotient-facing and representative preconditions are not one exact position-substituted bijection",
            ),
        }
    }
}

pub(super) fn derive_direct_terminal_plan(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    request: &QuotientOperationRequest,
) -> Result<DirectTerminalRelationPlan, RelationPlanError> {
    let mut representative = None;
    let mut input_relations = Vec::new();
    for (position, argument) in program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let argument_type =
            crate::places::declared_place_type_raw(program, machine, Some(state), *argument);
        let Some(argument_type) = argument_type else {
            let is_closed_scalar_candidate = match program.expression_table.expression(*argument) {
                psi_typed_trees::expression::ExpressionNode::Boolean(_) => true,
                psi_typed_trees::expression::ExpressionNode::Integer(_) => true,
                psi_typed_trees::expression::ExpressionNode::Float(literal) => {
                    literal.landing().is_some()
                }
                _ => false,
            };
            if request.kind == QuotientOperationKind::Lift && is_closed_scalar_candidate {
                if representative.is_none() {
                    representative = Some(derive_representative_telescope(program, request)?);
                }
                if let Some(parameter) = representative
                    .as_ref()
                    .and_then(|representative| representative.parameters.get(position))
                {
                    match closed_scalar_literal_for_representative(
                        program,
                        *argument,
                        parameter.type_reference,
                        position,
                    )? {
                        Some(_) => {
                            input_relations
                                .push(InputRelation::ExactEquality(parameter.type_reference));
                            continue;
                        }
                        None => {}
                    }
                }
            }
            return Err(RelationPlanError::UnresolvedArgumentType(position));
        };
        input_relations.push(match exact_quotient_relation(program, argument_type) {
            ExactRelationLookup::NotQuotient => InputRelation::ExactEquality(argument_type),
            ExactRelationLookup::Exact(relation) => InputRelation::Quotient(relation),
            ExactRelationLookup::OpenApplication => {
                return Err(RelationPlanError::UnresolvedInputRelationApplication(
                    position,
                ));
            }
        });
    }
    let result_relation = match exact_quotient_relation(program, state.return_type) {
        ExactRelationLookup::NotQuotient => return Err(RelationPlanError::ResultIsNotQuotient),
        ExactRelationLookup::Exact(relation) => relation,
        ExactRelationLookup::OpenApplication => {
            return Err(RelationPlanError::UnresolvedResultRelationApplication);
        }
    };
    let representative = match representative {
        Some(representative) => representative,
        None => derive_representative_telescope(program, request)?,
    };
    let representative_termination =
        unconditional_representative_termination(program, &representative);
    let selected_theorem = theorem::derive_selected_theorem_telescope(program, request)?;
    let selected_theorem_termination =
        theorem::unconditional_selected_theorem_termination(program, &selected_theorem);
    let theorem_operational = psi_effects::infer_operational_may(program);
    let theorem_reaches = psi_effects::infer_service_reaches(program, &theorem_operational);
    let selected_theorem_purity = theorem::pure_selected_theorem_effect(
        &selected_theorem,
        &theorem_operational,
        &theorem_reaches,
    );
    let selected_theorem_crash_free = crate::denotational_calls::has_no_crash_routes(
        program,
        selected_theorem.machine_symbol,
        &theorem_operational,
    );
    let expected_theorem_schema = derive_expected_theorem_schema(
        program,
        &input_relations,
        result_relation,
        &representative,
    )?;
    let theorem_schema_verification = verify_selected_theorem_schema(
        program,
        &representative,
        &selected_theorem,
        &expected_theorem_schema,
    );
    let direct_lift_correspondence = (request.kind == QuotientOperationKind::Lift)
        .then(|| {
            derive_direct_lift_runtime_correspondence(
                program,
                machine,
                state,
                call,
                &input_relations,
                result_relation,
                &representative,
            )
        })
        .transpose()?;
    let define_correspondence = (request.kind == QuotientOperationKind::Define)
        .then(|| {
            derive_define_runtime_correspondence(
                program,
                machine,
                state,
                call,
                &input_relations,
                result_relation,
                &representative,
            )
        })
        .transpose()?;
    let has_runtime_correspondence =
        direct_lift_correspondence.is_some() || define_correspondence.is_some();
    let representative_precondition = has_runtime_correspondence
        .then(|| {
            derive_representative_precondition_partition(program, &input_relations, &representative)
        })
        .transpose()?;
    let public_precondition = match (
        direct_lift_correspondence.as_ref(),
        define_correspondence.as_ref(),
    ) {
        (Some(runtime), None) => Some(derive_direct_lift_public_precondition_partition(
            program,
            machine,
            state,
            &input_relations,
            runtime,
        )?),
        (None, Some(runtime)) => Some(derive_public_precondition_partition(
            program,
            machine,
            state,
            &input_relations,
            &runtime.positions,
        )?),
        _ => None,
    };
    let define_precondition_correspondence = match (
        define_correspondence.as_ref(),
        public_precondition.as_ref(),
        representative_precondition.as_ref(),
    ) {
        (Some(runtime), Some(public), Some(representative_partition)) => {
            Some(derive_define_precondition_correspondence(
                program,
                machine,
                state,
                &representative,
                public,
                representative_partition,
                runtime,
            )?)
        }
        _ => None,
    };
    let direct_lift_precondition_implication = match (
        direct_lift_correspondence.as_ref(),
        public_precondition.as_ref(),
        representative_precondition.as_ref(),
        theorem_schema_verification.as_ref().ok(),
    ) {
        (Some(runtime), Some(public), Some(representative_partition), Some(verified_theorem)) => {
            Some(derive_direct_lift_precondition_implication(
                program,
                machine,
                state,
                &representative,
                public,
                representative_partition,
                runtime,
                &expected_theorem_schema,
                verified_theorem,
            )?)
        }
        _ => None,
    };
    let correspondence_certificate = match request.kind {
        QuotientOperationKind::Lift => direct_lift_correspondence
            .as_ref()
            .zip(direct_lift_precondition_implication.as_ref())
            .and_then(|(runtime, precondition)| {
                compose_lift_correspondence_certificate(
                    &theorem_schema_verification,
                    runtime,
                    precondition,
                )
            }),
        QuotientOperationKind::Define => define_correspondence
            .as_ref()
            .zip(define_precondition_correspondence.as_ref())
            .and_then(|(runtime, precondition)| {
                compose_define_correspondence_certificate(
                    &theorem_schema_verification,
                    runtime,
                    precondition,
                )
            }),
    };
    Ok(DirectTerminalRelationPlan {
        input_relations,
        result_relation,
        representative,
        representative_termination,
        selected_theorem,
        selected_theorem_termination,
        selected_theorem_purity,
        selected_theorem_crash_free,
        expected_theorem_schema,
        theorem_schema_verification,
        direct_lift_correspondence,
        define_correspondence,
        public_precondition,
        representative_precondition,
        direct_lift_precondition_implication,
        define_precondition_correspondence,
        correspondence_certificate,
    })
}

enum ExactRelationLookup {
    NotQuotient,
    Exact(ExactQuotientRelation),
    OpenApplication,
}

fn exact_quotient_relation(
    program: &TypedTrees,
    quotient_type: TypeReferenceHandle,
) -> ExactRelationLookup {
    let Some(quotient) = super::quotient_for_type(program, quotient_type) else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(relation) = program
        .propositions()
        .iter()
        .find(|relation| relation.symbol == metadata.relation_symbol)
    else {
        return ExactRelationLookup::OpenApplication;
    };
    if !program.proposition_binders(relation).is_empty() {
        // The quotient declaration retains the relation declaration identity,
        // but not the closed application needed for heterogeneous families.
        // That application must come from the fully instantiated
        // representative operation telescope; guessing it from the quotient
        // type would collapse independently quantified I/J/K binders.
        return ExactRelationLookup::OpenApplication;
    }
    ExactRelationLookup::Exact(ExactQuotientRelation {
        quotient_type,
        quotient_symbol: quotient.symbol,
        relation_symbol: metadata.relation_symbol,
    })
}

impl DirectTerminalRelationPlan {
    pub(super) fn render_ra(&self, program: &TypedTrees) -> String {
        let positions = self
            .input_relations
            .iter()
            .enumerate()
            .map(|(position, relation)| {
                let relation = match relation {
                    InputRelation::Quotient(relation) => {
                        relation_name(program, relation.relation_symbol)
                    }
                    InputRelation::ExactEquality(type_reference) => format!(
                        "==<{}>",
                        program.display_type_reference_with_constraints(*type_reference)
                    ),
                };
                format!("{position}:{relation}")
            })
            .collect::<Vec<_>>();
        format!("RA=[{}]", positions.join(", "))
    }

    pub(super) fn render_rr(&self, program: &TypedTrees) -> String {
        format!(
            "RR={}",
            relation_name(program, self.result_relation.relation_symbol)
        )
    }

    pub(super) fn render_representative_telescope(&self, program: &TypedTrees) -> String {
        let parameters = self
            .representative
            .parameters
            .iter()
            .map(|parameter| {
                let receiver = if parameter.is_self { "self:" } else { "" };
                format!(
                    "{receiver}{}",
                    program.display_type_reference_with_constraints(parameter.type_reference)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "F#{}({parameters})->{}",
            self.representative.state_symbol.arena_index(),
            program.display_type_reference_with_constraints(self.representative.return_type),
        )
    }

    pub(super) fn render_representative_termination(&self) -> Option<String> {
        self.representative_termination.map(|termination| {
            format!(
                "unconditional-termination=machine#{}:state#{}",
                termination.machine_symbol.arena_index(),
                termination.state_symbol.arena_index(),
            )
        })
    }

    pub(super) fn render_selected_theorem(&self, program: &TypedTrees) -> String {
        let parameters = self
            .selected_theorem
            .parameters
            .iter()
            .map(|parameter| {
                program.display_type_reference_with_constraints(parameter.type_reference)
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "theorem#{}:state#{}({parameters})[static-bindings:{}]",
            self.selected_theorem.machine_symbol.arena_index(),
            self.selected_theorem.state_symbol.arena_index(),
            self.selected_theorem.static_application.bindings.len(),
        )
    }

    pub(super) fn render_expected_theorem_schema(&self) -> String {
        // Diagnostic summary only. Canonical equality is the structural
        // `ExpectedTheoremSchema`; equal counts never imply equal schemas.
        format!(
            "theorem-schema=[parameters:{}, relations:{}, legality:{}, applications:2, conclusion:1]",
            self.expected_theorem_schema.parameters.len(),
            self.expected_theorem_schema.relation_premises.len(),
            self.expected_theorem_schema.legality_premises.len(),
        )
    }

    pub(super) fn render_define_correspondence(&self) -> Option<String> {
        self.define_correspondence.as_ref().map(|correspondence| {
            format!(
                "define-runtime=[{}]",
                correspondence
                    .positions
                    .iter()
                    .enumerate()
                    .map(|(position, _)| position.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    pub(super) fn render_direct_lift_correspondence(&self) -> Option<String> {
        self.direct_lift_correspondence
            .as_ref()
            .map(|correspondence| {
                format!(
                    "direct-lift-runtime=[{}]",
                    correspondence
                        .positions
                        .iter()
                        .enumerate()
                        .map(|(position, _)| position.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    pub(super) fn render_representative_precondition(&self) -> Option<String> {
        self.representative_precondition.as_ref().map(|partition| {
            format!(
                "P=[dependent:{}, fixed:{}]",
                partition.dependent.len(),
                partition.fixed.len()
            )
        })
    }

    pub(super) fn has_fixed_representative_preconditions(&self) -> bool {
        self.representative_precondition
            .as_ref()
            .is_some_and(|partition| !partition.fixed.is_empty())
    }

    pub(super) fn render_public_precondition(&self) -> Option<String> {
        self.public_precondition.as_ref().map(|partition| {
            format!(
                "Q=[dependent:{}, fixed:{}]",
                partition.dependent.len(),
                partition.fixed.len()
            )
        })
    }

    pub(super) fn render_define_precondition_correspondence(&self) -> Option<String> {
        self.define_precondition_correspondence
            .as_ref()
            .map(|correspondence| format!("Q<->P=[dependent:{}]", correspondence.dependent.len()))
    }

    pub(super) fn render_direct_lift_precondition_implication(&self) -> Option<String> {
        self.direct_lift_precondition_implication
            .as_ref()
            .map(|implication| {
                let left = implication
                    .rows
                    .iter()
                    .filter(|row| row.application == theorem_schema::TheoremApplicationSide::Left)
                    .count();
                let right = implication.rows.len() - left;
                format!("Q=>P=[left:{left}, right:{right}]")
            })
    }

    pub(super) fn render_correspondence_certificate(&self) -> Option<String> {
        self.correspondence_certificate.as_ref().map(|certificate| {
            let kind = match &certificate.evidence {
                correspondence_certificate::QuotientCorrespondenceEvidence::DirectLift {
                    ..
                } => "direct-lift",
                correspondence_certificate::QuotientCorrespondenceEvidence::Define { .. } => {
                    "define"
                }
            };
            format!(
                "{kind}-certificate=[theorem#{}:state#{}]",
                certificate.theorem.theorem_machine_symbol.arena_index(),
                certificate.theorem.theorem_state_symbol.arena_index(),
            )
        })
    }
}

fn relation_name(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == symbol)
        .map(|proposition| proposition.name.as_str().to_owned())
        .unwrap_or_else(|| format!("relation#{symbol:?}"))
}

#[cfg(test)]
mod tests;
