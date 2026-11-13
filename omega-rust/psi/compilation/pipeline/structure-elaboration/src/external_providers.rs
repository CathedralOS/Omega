/*
The target's primitive realization table selects the sealed numeric catalog
even when the application performs no floating operation. Native preparation
requires those selections to join exact external declaration rows in the
application's own artifact (terminal-psi/product.md).

These rows describe authenticated source declarations, not proved bodies.
The typed catalog gate fixes their complete value signature, owned satisfaction,
primitive binding and permitted effects. Their contract commitment covers both
their own declarations and the public requirement they satisfy, including its
inherited guarantees. No checker fact or old producer commitment is read.
*/

use std::hash::Hash;

use language_semantics::{TerminationGuarantee, TerminationInterface};
use optimization::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{
    ExternalProviderConformance, ExternalProviderOwner, ExternalRequirementSchema,
    NominalDeclarationIdentity, TerminalIdentityTableBuilder,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::type_identity::{IndexIdentityArgument, normalized_contract_argument_identity};
use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement;

use crate::{Unserved, declare, refuse};

const CONTRACT_DOMAIN: &[u8] = b"omega.lowered-external-public-contract.v1\0";

#[derive(Hash)]
enum Fact {
    Expression(String),
    Membership {
        value: String,
        domain: String,
        arguments: Vec<String>,
    },
    Proposition {
        declaration: String,
        binder_kinds: Vec<u8>,
        binders: Vec<String>,
        arguments: Vec<String>,
        formula: String,
    },
}

#[derive(Hash)]
enum ClauseKind {
    Requires,
    Ensures,
    EnsuresForResultCase { data: String, case: String },
    CrashesTrap,
    CrashesAbort,
}

#[derive(Hash)]
struct Clause {
    kind: ClauseKind,
    facts: Vec<Fact>,
}

/// Preserve source distinctions that do not occupy a contract-fact row.
/// Any dimension not normalized here refuses the whole compilation.
#[derive(Hash)]
struct Header {
    reach: u8,
    suspends: bool,
    explicit_suspends: bool,
    blocks: bool,
    explicit_blocks: bool,
    explicit_no_crash: bool,
    termination: u8,
}

impl Header {
    fn read(machine: &typed_trees::machine::Machine) -> Result<Self, &'static str> {
        if machine.service_reach_is_installation_bound
            || !machine.invokes.is_empty()
            || !machine.type_parameters.is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.owned_data.is_empty()
            || !machine.declared_type_equations.is_empty()
            || !machine.conformance_bounds.is_empty()
            || machine.structural_type_equations_pending
            || machine.admitted_same_stack_bytes != 0
            || machine.termination_plan.implementation_witness.is_some()
        {
            return Err(
                "external contract has an unsupported capture, effect or progress dimension",
            );
        }
        let reach = match machine.service_reach_row {
            language_semantics::ServiceReachRowId::NULL => 0,
            language_semantics::ServiceReachRowTable::EMPTY_ROW => 1,
            _ => return Err("external contract has a nonempty or unresolved service ceiling"),
        };
        let termination = match &machine.termination_plan.interface {
            TerminationInterface::InternalDerived => 0,
            TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => 1,
            TerminationInterface::Published(TerminationGuarantee::Terminates { premises })
                if premises.is_empty() =>
            {
                2
            }
            TerminationInterface::Published(TerminationGuarantee::Terminates { .. }) => {
                return Err("external contract has a progress premise");
            }
        };
        Ok(Self {
            reach,
            suspends: machine.suspends,
            explicit_suspends: !machine.suspends_keyword_source_spans.is_empty(),
            blocks: machine.blocks,
            explicit_blocks: !machine.blocks_keyword_source_spans.is_empty(),
            explicit_no_crash: machine.declares_no_crash_route,
            termination,
        })
    }
}

struct Contract<'typed, 'sources> {
    typed: &'typed TypedTrees,
    sources: &'sources [(source::SourceId, [u8; 32])],
    binders: Vec<(SymbolHandle, String)>,
    owner: SymbolHandle,
}

impl<'typed, 'sources> Contract<'typed, 'sources> {
    fn new(
        typed: &'typed TypedTrees,
        sources: &'sources [(source::SourceId, [u8; 32])],
        parameters: &[StateParameter],
        owner: SymbolHandle,
    ) -> Result<Self, &'static str> {
        if !owner.is_valid()
            || parameters
                .iter()
                .any(|parameter| !parameter.symbol.is_valid())
        {
            return Err("external contract has an unresolved binder owner");
        }
        Ok(Self {
            typed,
            sources,
            binders: parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| (parameter.symbol, format!("$V{position}")))
                .collect(),
            owner,
        })
    }

    fn argument(
        &self,
        argument: IndexIdentityArgument<'_>,
        result: bool,
    ) -> Result<String, &'static str> {
        normalized_contract_argument_identity(
            self.typed,
            argument,
            &self.binders,
            result.then_some(self.owner),
            self.sources,
        )
        .ok_or("external contract argument has no exact owned identity")
    }

    fn expression(
        &self,
        expression: ExpressionHandle,
        result: bool,
    ) -> Result<String, &'static str> {
        if !self.typed.expression_table.expression_is_valid(expression) {
            return Err("external contract has an unresolved expression");
        }
        self.argument(IndexIdentityArgument::Expression(expression), result)
    }

    fn clauses(&self, contracts: &[SignatureContract]) -> Result<Vec<Clause>, &'static str> {
        contracts
            .iter()
            .map(|contract| {
                if contract.binding.is_some() {
                    return Err("external contract introduces an unsupported evidence binder");
                }
                let (kind, result) = match &contract.kind {
                    SignatureContractKind::Requires => (ClauseKind::Requires, false),
                    SignatureContractKind::Ensures => (ClauseKind::Ensures, true),
                    SignatureContractKind::EnsuresForResultCase {
                        result_data,
                        result_case,
                    } => (
                        ClauseKind::EnsuresForResultCase {
                            data: declare::nominal(self.typed, self.sources, *result_data)?,
                            case: declare::nominal(self.typed, self.sources, *result_case)?,
                        },
                        true,
                    ),
                    SignatureContractKind::Crashes { cause } => (
                        match cause {
                            typed_trees::signature::CrashCause::Trap => ClauseKind::CrashesTrap,
                            typed_trees::signature::CrashCause::Abort => ClauseKind::CrashesAbort,
                        },
                        false,
                    ),
                };
                let facts = self
                    .typed
                    .proof_facts
                    .span(contract.facts)
                    .ok_or("external contract fact span does not resolve")?
                    .iter()
                    .map(|fact| self.fact(fact, result))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Clause { kind, facts })
            })
            .collect()
    }

    fn fact(&self, fact: &ProofFact, result: bool) -> Result<Fact, &'static str> {
        Ok(match fact {
            ProofFact::Expression(expression) => {
                Fact::Expression(self.expression(*expression, result)?)
            }
            ProofFact::Membership(membership) => {
                let arguments = self
                    .typed
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments);
                if arguments.len() != membership.domain_arguments.len() {
                    return Err("external membership argument span does not resolve");
                }
                Fact::Membership {
                    value: self.expression(membership.value, result)?,
                    domain: declare::nominal(self.typed, self.sources, membership.domain_symbol)?,
                    arguments: arguments
                        .iter()
                        .map(|argument| {
                            self.argument(IndexIdentityArgument::TypeArgument(*argument), result)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            ProofFact::Proposition(application) => {
                let mut kinds = Vec::with_capacity(application.binder_arguments.len());
                let mut binders = Vec::with_capacity(application.binder_arguments.len());
                for binder in &application.binder_arguments {
                    kinds.push(match binder.kind {
                        typed_trees::proposition::PropositionBinderArgumentKind::Type => 0,
                        typed_trees::proposition::PropositionBinderArgumentKind::Const => 1,
                        typed_trees::proposition::PropositionBinderArgumentKind::Machine => 2,
                    });
                    if binder.evidence_projection.is_some() {
                        return Err("external proposition has an unsupported evidence projection");
                    }
                    binders.push(match &binder.const_literal {
                        Some(literal)
                            if matches!(
                                binder.kind,
                                typed_trees::proposition::PropositionBinderArgumentKind::Const
                            ) =>
                        {
                            literal
                                .value_bignum()
                                .ok_or("external proposition literal is invalid")?
                                .to_string()
                        }
                        Some(_) => {
                            return Err("external proposition has a non-const literal binder");
                        }
                        None => {
                            self.argument(IndexIdentityArgument::Subject(binder.symbol), result)?
                        }
                    });
                }
                let values = self
                    .typed
                    .expression_table
                    .expression_handles(application.arguments);
                if values.len() != application.arguments.len() {
                    return Err("external proposition argument span does not resolve");
                }
                let arguments = values
                    .iter()
                    .map(|argument| self.expression(*argument, result))
                    .collect::<Result<Vec<_>, _>>()?;
                let formula = self
                    .typed
                    .normalize_proposition_application(
                        application,
                        Some(typed_trees::proposition::PropositionLabels {
                            binder_labels: &binders,
                            argument_labels: &arguments,
                        }),
                    )
                    .ok_or("external proposition cannot be normalized exactly")?;
                Fact::Proposition {
                    declaration: declare::nominal(
                        self.typed,
                        self.sources,
                        application.proposition,
                    )?,
                    binder_kinds: kinds,
                    binders,
                    arguments,
                    formula: formula.identity_label(),
                }
            }
        })
    }
}

pub(crate) fn retain(
    typed: &TypedTrees,
    module: &mut terminal_psi::TerminalModule,
    entry: MachineId,
) -> Checked<()> {
    let project = |identities: &mut TerminalIdentityTableBuilder| {
        let sources =
            source::source_identity::toolchain_source_identities(typed.symbols.source_files())
                .map_err(|_| "external declaration source identities do not resolve")?;
        let mut rows = Vec::new();
        for machine in typed
            .machines()
            .iter()
            .filter(|machine| typed_trees::float_providers::is_catalog_declaration(typed, machine))
        {
            let (required, meaning) =
                typed_trees::float_providers::authenticate_intrinsic_member(typed, machine)?;
            let requirement = IntrinsicRequirement::by_symbol(typed, required)
                .ok_or("external realization has no exact public requirement")?;
            let required_machine = typed
                .machine_by_symbol(requirement.symbol)
                .ok_or("external requirement has no closed machine header")?;
            let shape = declare::source_shape(typed, &sources, machine.symbol)?;
            let required_shape = declare::source_shape(typed, &sources, requirement.symbol)?;
            if required_shape.state.symbol != requirement.call_target
                || shape.parameter_types != required_shape.parameter_types
                || shape.result_type != required_shape.result_type
            {
                return Err(
                    "external realization and public requirement have different exact signatures",
                );
            }
            let header = Header::read(shape.declared)?;
            let required_header = Header::read(required_machine)?;
            let own_contracts = typed
                .signature_contracts
                .span(machine.contracts)
                .ok_or("external realization contract span does not resolve")?;
            let state_contracts = typed
                .signature_contracts
                .span(shape.state.contracts)
                .ok_or("external realization state contract span does not resolve")?;
            let required_contracts = typed
                .signature_contracts
                .span(required_machine.contracts)
                .ok_or("external requirement contract span does not resolve")?;
            let required_state_contracts = typed
                .signature_contracts
                .span(required_shape.state.contracts)
                .ok_or("external requirement state contract span does not resolve")?;
            let [conformance] = typed.machine_trait_conformances(machine) else {
                return Err("external realization lost its exact satisfaction");
            };
            let binding = conformance
                .external_binding
                .and_then(|binding| typed.external_bindings.identity(binding))
                .ok_or("external realization has no binding identity")?;
            let own = Contract::new(typed, &sources, shape.formals, shape.state.symbol)?;
            let inherited = Contract::new(
                typed,
                &sources,
                requirement.parameters,
                requirement.call_target,
            )?;
            let mut commitment = declare::Commitment(DomainDigestEncoder::new(CONTRACT_DOMAIN));
            shape.callable.hash(&mut commitment);
            for (formal, value_type) in shape.formals.iter().zip(&shape.parameter_types) {
                (
                    formal.is_const,
                    formal.is_mutable,
                    formal.relevance,
                    value_type,
                )
                    .hash(&mut commitment);
            }
            shape.result_type.hash(&mut commitment);
            required_shape.callable.hash(&mut commitment);
            required_shape.parameter_types.hash(&mut commitment);
            required_shape.result_type.hash(&mut commitment);
            meaning.hash(&mut commitment);
            binding.hash(&mut commitment);
            typed.machine_authored_target(machine).hash(&mut commitment);
            header.hash(&mut commitment);
            required_header.hash(&mut commitment);
            own.clauses(own_contracts)?.hash(&mut commitment);
            own.clauses(state_contracts)?.hash(&mut commitment);
            inherited.clauses(required_contracts)?.hash(&mut commitment);
            inherited
                .clauses(required_state_contracts)?
                .hash(&mut commitment);
            let realization = shape.finish(
                identities,
                commitment.0.finish(),
                header.suspends,
                header.blocks,
            );
            let provider = typed_trees::provider_declarations::satisfaction_provider_declaration(
                typed, machine,
            )
            .ok_or("external realization has no exact selecting owner")?;
            let (owned, _) = typed
                .owned_conformance_realization(machine.symbol)
                .ok_or("external realization lost its owned map row")?;
            if provider != owned.symbol {
                return Err("external realization has inconsistent selecting owners");
            }
            rows.push(ExternalProviderConformance {
                requirement: NominalDeclarationIdentity::new(identities.intern(&declare::nominal(
                    typed,
                    &sources,
                    requirement.symbol,
                )?)),
                requirement_identity: identities.intern(&requirement.requirement_identity),
                requirement_package: requirement.package_identity,
                numeric_intrinsic: Some(meaning),
                requirement_schema: Some(ExternalRequirementSchema {
                    identity: identities.intern(&requirement.schema_identity()),
                    method: identities.intern(requirement.method_name()),
                    owner: identities.intern(requirement.method_owner()),
                }),
                target: typed.machine_authored_target(machine).map(str::to_owned),
                realization,
                realization_package: typed.symbols.symbol_package_identity(machine.symbol),
                provider: Some(ExternalProviderOwner {
                    declaration: NominalDeclarationIdentity::new(
                        identities.intern(&declare::nominal(typed, &sources, provider)?),
                    ),
                    identity: identities.intern(&typed.symbols.display_path(provider, "::")),
                    package: typed.symbols.symbol_package_identity(provider),
                }),
                binding: binding.clone(),
            });
        }
        rows.sort();
        rows.dedup();
        Ok(rows)
    };
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let rows = project(&mut identities)
        .map_err(|reason| refuse(entry, Construct::Machine, Unserved::Contract(reason)));
    module.identities = identities.build();
    module.external_providers = rows?;
    Ok(())
}
