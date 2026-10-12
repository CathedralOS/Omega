//! Directional public source judgments for actual executed registrations.
//!
//! The original endpoint applications select preparation-produced declarations.
//! The final source window classifies their public claims once; no realization
//! consumer re-enters proof. These rows are not a complete proof-input ledger,
//! semantic dependency closure, domain issuance or receiving authorization.

use arena::Handle;
use checked_trees::typed_trees::domain::ProofFact;
use checked_trees::typed_trees::signature::{SignatureContractKind, StateParameter};
use checked_trees::typed_trees::static_applications::{
    AssumedBindingContractDemands, StaticApplicationDemand,
};
use checked_trees::typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    AssumedBindingContractSide, AssumedBindingDirectionalObligations, AssumedBindingPublicClaim,
    AssumedBindingResultCase, AssumedBindingSemanticConnection, AssumedBindingSemanticDirection,
    AssumedBindingSemanticJudgment, AssumedBindingSemanticObligation, AssumedBindingSemanticSource,
    AssumedBindingSemanticSubject, CheckedAssumedBindingContracts, CheckedPublicContractOwner,
    CheckedTrees, ContractProofFactKind, ContractProofFactOwner,
};
use symbols::SymbolHandle;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

#[derive(Clone, Copy)]
enum PublicRequirement<'program> {
    Trait {
        definition: &'program checked_trees::typed_trees::trait_definition::TraitDefinition,
        signature: &'program checked_trees::typed_trees::signature::StateSignature,
    },
    Machine {
        declaration: &'program checked_trees::typed_trees::machine::Machine,
        entry: &'program checked_trees::typed_trees::state::State,
    },
}

struct DeclaredRequirement<'program> {
    declaration: PublicRequirement<'program>,
    parameters: &'program [StateParameter],
    result: TypeReferenceHandle,
}

impl DeclaredRequirement<'_> {
    fn symbol(&self) -> SymbolHandle {
        match self.declaration {
            PublicRequirement::Trait { signature, .. } => signature.symbol,
            PublicRequirement::Machine { declaration, .. } => declaration.symbol,
        }
    }
}

fn requirement<'program>(
    program: &'program CheckedTrees,
    demand: &StaticApplicationDemand,
) -> Result<DeclaredRequirement<'program>, diagnostics::Diagnostic> {
    let symbol =
        checked_trees::typed_trees::product_requirements::prepared_product_requirement_symbol(
            program, demand,
        )?;
    if let Some(declaration) = program.machine(symbol) {
        if declaration.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || declaration.body_is_present
        {
            return Err(diagnostics::Diagnostic::error(
                "assumed binding source endpoint is not a bodyless public requirement",
            ));
        }
        let [entry] = program.machine_states(declaration) else {
            return Err(diagnostics::Diagnostic::error(
                "assumed binding source endpoint needs one public entry",
            ));
        };
        return Ok(DeclaredRequirement {
            declaration: PublicRequirement::Machine { declaration, entry },
            parameters: program.state_parameters(entry),
            result: entry.return_type,
        });
    }
    let (definition, signature) = program.trait_machine_signature(symbol).ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "assumed binding source endpoint lost its public declaration",
        )
    })?;
    if !definition.is_boundary {
        return Err(diagnostics::Diagnostic::error(
            "assumed binding source endpoint is not a boundary requirement",
        ));
    }
    Ok(DeclaredRequirement {
        declaration: PublicRequirement::Trait {
            definition,
            signature,
        },
        parameters: program.state_signature_parameters(signature),
        result: signature.return_type,
    })
}

pub(crate) fn record(
    program: &CheckedTrees,
    demands: &AssumedBindingContractDemands,
) -> Result<CheckedAssumedBindingContracts, Vec<diagnostics::Diagnostic>> {
    let mut facts = CheckedAssumedBindingContracts::default();
    for demand in demands.iter() {
        let endpoints = || {
            Ok::<_, diagnostics::Diagnostic>((
                requirement(program, &demand.required)?,
                requirement(program, &demand.supplied)?,
            ))
        };
        let (required, supplied) = endpoints()
            .map_err(|diagnostic| vec![diagnostic.with_source_span(demand.source_span)])?;
        // Repeated execution of the identical registration requests the same
        // original source judgment. Other pairs or selecting sites stay distinct.
        if facts
            .connection(demand, required.symbol(), supplied.symbol())
            .is_none()
        {
            let connection = capture(program, &required, &supplied);
            facts.retain(
                demand.clone(),
                required.symbol(),
                supplied.symbol(),
                connection,
            );
        }
    }
    Ok(facts)
}

fn capture(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
) -> AssumedBindingSemanticConnection {
    let entry = direction(
        program,
        required,
        supplied,
        AssumedBindingSemanticDirection::Entry,
    );
    let exit = direction(
        program,
        required,
        supplied,
        AssumedBindingSemanticDirection::Exit,
    );
    AssumedBindingSemanticConnection::new(entry, exit)
}

fn direction(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    direction: AssumedBindingSemanticDirection,
) -> AssumedBindingDirectionalObligations {
    let (premise_side, premise_declaration, goal_side, goal_declaration, kind) = match direction {
        AssumedBindingSemanticDirection::Entry => (
            AssumedBindingContractSide::Required,
            required,
            AssumedBindingContractSide::Supplied,
            supplied,
            ContractProofFactKind::Requires,
        ),
        AssumedBindingSemanticDirection::Exit => (
            AssumedBindingContractSide::Supplied,
            supplied,
            AssumedBindingContractSide::Required,
            required,
            ContractProofFactKind::Ensures,
        ),
    };
    let premises = claims(program, premise_side, premise_declaration, kind);
    let goals = claims(program, goal_side, goal_declaration, kind);
    let mut obligations = Vec::with_capacity(goals.len());
    for goal in goals {
        let judgment = judge(program, required, supplied, &premises, &goal);
        obligations.push(AssumedBindingSemanticObligation::new(goal, judgment));
    }
    AssumedBindingDirectionalObligations::new(direction, premises, obligations)
}

fn claims(
    program: &CheckedTrees,
    side: AssumedBindingContractSide,
    declaration: &DeclaredRequirement<'_>,
    kind: ContractProofFactKind,
) -> Vec<AssumedBindingPublicClaim> {
    let mut claims = Vec::new();
    for (handle, checked) in program.facts.proof.contract_facts.iter() {
        if checked.kind != kind || !owns_contract(declaration, checked.owner) {
            continue;
        }
        claims.push(AssumedBindingPublicClaim::new(
            side,
            subject(program, declaration, checked.fact),
            AssumedBindingSemanticSource::Contract(handle),
            None,
        ));
    }
    match kind {
        ContractProofFactKind::Where => {}
        ContractProofFactKind::Requires => {
            for (ordinal, parameter) in declaration.parameters.iter().enumerate() {
                type_claims(
                    program,
                    side,
                    AssumedBindingSemanticSubject::Parameter {
                        ordinal,
                        symbol: parameter.symbol,
                    },
                    parameter.type_reference,
                    &mut claims,
                );
            }
        }
        ContractProofFactKind::Ensures => {
            type_claims(
                program,
                side,
                AssumedBindingSemanticSubject::Result,
                declaration.result,
                &mut claims,
            );
            let (first, second) = match declaration.declaration {
                PublicRequirement::Trait { signature, .. } => {
                    (signature.contracts, Default::default())
                }
                PublicRequirement::Machine { declaration, entry } => {
                    (declaration.contracts, entry.contracts)
                }
            };
            for contract_handle in first.handles().chain(second.handles()) {
                let contract = program.signature_contracts.get(contract_handle);
                let SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } = contract.kind
                else {
                    continue;
                };
                for fact in contract.facts.handles() {
                    claims.push(AssumedBindingPublicClaim::new(
                        side,
                        subject(program, declaration, fact),
                        AssumedBindingSemanticSource::ResultCase {
                            contract: contract_handle,
                            fact,
                        },
                        Some(AssumedBindingResultCase {
                            data: result_data,
                            case: result_case,
                        }),
                    ));
                }
            }
        }
    }
    claims
}

fn owns_contract(declaration: &DeclaredRequirement<'_>, owner: ContractProofFactOwner) -> bool {
    let declaration = match declaration.declaration {
        PublicRequirement::Trait {
            definition,
            signature,
        } => CheckedPublicContractOwner::Trait {
            owner: definition.symbol,
            signature: signature.symbol,
        },
        PublicRequirement::Machine { declaration, entry } => CheckedPublicContractOwner::Machine {
            machine: declaration.symbol,
            entry: entry.symbol,
        },
    };
    declaration.owns_contract(owner)
}

fn subject(
    program: &CheckedTrees,
    declaration: &DeclaredRequirement<'_>,
    fact: Handle<ProofFact>,
) -> AssumedBindingSemanticSubject {
    let ProofFact::Membership(membership) = program.proof_facts.get(fact) else {
        return AssumedBindingSemanticSubject::Predicate;
    };
    if program
        .expression_table
        .names_reserved_result(membership.value)
    {
        return AssumedBindingSemanticSubject::Result;
    }
    declaration
        .parameters
        .iter()
        .enumerate()
        .find_map(|(ordinal, parameter)| {
            program
                .expression_table
                .names_parameter(membership.value, parameter)
                .then_some(AssumedBindingSemanticSubject::Parameter {
                    ordinal,
                    symbol: parameter.symbol,
                })
        })
        .unwrap_or(AssumedBindingSemanticSubject::Predicate)
}

fn type_claims(
    program: &CheckedTrees,
    side: AssumedBindingContractSide,
    subject: AssumedBindingSemanticSubject,
    mut type_reference: TypeReferenceHandle,
    claims: &mut Vec<AssumedBindingPublicClaim>,
) {
    let mut visited = Vec::new();
    while type_reference.is_valid() && !visited.contains(&type_reference) {
        visited.push(type_reference);
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint_ordinal in
                    0..program.type_reference_table.constraints(*constraints).len()
                {
                    claims.push(AssumedBindingPublicClaim::new(
                        side,
                        subject,
                        AssumedBindingSemanticSource::DeclaredType {
                            type_reference,
                            constraint_ordinal,
                        },
                        None,
                    ));
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
}

fn judge(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    premises: &[AssumedBindingPublicClaim],
    goal: &AssumedBindingPublicClaim,
) -> AssumedBindingSemanticJudgment {
    // The scalar query has no nominal result-case admission. Preserve such
    // obligations instead of forgetting their guard or flattening an evidence row.
    if goal.result_case().is_some() {
        return AssumedBindingSemanticJudgment::Assumed;
    }
    let declaration = |side| match side {
        AssumedBindingContractSide::Required => required,
        AssumedBindingContractSide::Supplied => supplied,
    };
    let Some(goal_expression) = expression(program, goal, declaration(goal.side())) else {
        return AssumedBindingSemanticJudgment::Assumed;
    };
    let hypotheses = premises
        .iter()
        .filter(|claim| claim.result_case().is_none() || claim.result_case() == goal.result_case())
        .filter_map(|claim| {
            expression(program, claim, declaration(claim.side())).map(|proposition| {
                ScopedArithmeticHypothesis {
                    proposition,
                    holds: true,
                }
            })
        })
        .collect::<Vec<_>>();
    match validation::scoped_arithmetic_implication(
        program,
        goal_expression.owner,
        &hypotheses,
        &goal_expression,
    ) {
        StrictArithmeticImplicationJudgment::Proven => AssumedBindingSemanticJudgment::Proven,
        StrictArithmeticImplicationJudgment::Refuted => AssumedBindingSemanticJudgment::Refuted,
        StrictArithmeticImplicationJudgment::Unknown => AssumedBindingSemanticJudgment::Assumed,
    }
}

fn expression(
    program: &CheckedTrees,
    claim: &AssumedBindingPublicClaim,
    declaration: &DeclaredRequirement<'_>,
) -> Option<ScopedArithmeticExpression> {
    let fact = match claim.source() {
        AssumedBindingSemanticSource::Contract(handle) => {
            program.facts.proof.contract_facts.get(handle).fact
        }
        AssumedBindingSemanticSource::ResultCase { fact, .. } => fact,
        AssumedBindingSemanticSource::Absent
        | AssumedBindingSemanticSource::DeclaredType { .. } => return None,
    };
    let ProofFact::Expression(expression) = program.proof_facts.get(fact) else {
        return None;
    };
    let mut bindings = Vec::new();
    for (ordinal, parameter) in declaration.parameters.iter().enumerate() {
        if let Some(primitive) = integer_primitive(program, parameter.type_reference) {
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("assumed-binding-subject-{ordinal}"),
                    unsigned: !primitive.is_signed_integer(),
                },
            });
        }
    }
    if let Some(primitive) = integer_primitive(program, declaration.result) {
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Result,
            value: ScopedArithmeticValue::Atom {
                identity: "assumed-binding-result".to_owned(),
                unsigned: !primitive.is_signed_integer(),
            },
        });
    }
    Some(ScopedArithmeticExpression {
        owner: declaration.symbol(),
        expression: *expression,
        bindings,
    })
}

fn integer_primitive(
    program: &CheckedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut visited = Vec::new();
    while type_reference.is_valid() && !visited.contains(&type_reference) {
        visited.push(type_reference);
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return PrimitiveType::from_builtin_atom(
                    program.symbols.builtin_type_atom(*symbol)?,
                );
            }
            _ => return None,
        }
    }
    None
}
