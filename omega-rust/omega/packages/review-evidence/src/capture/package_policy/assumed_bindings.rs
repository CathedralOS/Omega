//! Root relationships are risk decisions, independent of checked provider rows.
//! The full consumed source commitment includes private supplier bodies and
//! transitive semantic dependencies. Keeping it in each accepted meaning makes
//! a resolution-only update unable to silently preserve changed trust.
use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;

pub(super) fn project(input: &PackageReviewInput<'_>) -> Result<Vec<String>, Vec<Diagnostic>> {
    let facts = &input.custody.execution().selected_provider_plan_facts;
    if facts.assumed_bindings().is_empty() {
        return Ok(Vec::new());
    }
    let sources = input.custody.sources();
    let source = sources.source_consumption_commitment().ok_or_else(|| {
        super::rejected("assumed binding risk has no exact package source closure")
    })?;
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let contract = |identity: &str| -> Result<
        Vec<crate::record::PackageReviewCallableContract>,
        Vec<Diagnostic>,
    > {
        for definition in input.traits() {
            for signature in input.trait_machine_signatures(definition) {
                if input.trait_requirement_callable_identity_text(definition, signature) == identity
                {
                    let mut lifetimes = definition.lifetime_parameters.clone();
                    lifetimes.extend(signature.lifetime_parameters.iter().cloned());
                    let context = crate::capture::contracts::facts::ContractProjectionContext {
                        subject_kind: "assumed binding requirement",
                        subject_name: identity,
                        owner: checked_trees::ContractProofFactOwner::StateSignature {
                            owner_symbol: definition.symbol,
                            state_symbol: signature.symbol,
                        },
                        point: facts::ProgramPoint::State {
                            machine_symbol: definition.symbol,
                            state_symbol: signature.symbol,
                        },
                        parameters: input.state_signature_parameters(signature),
                        domain_symbol: None,
                        data_symbol: None,
                        lifetime_binders: &lifetimes,
                        lifetime_substitutions: &[],
                        selection_exposure: if definition.is_public {
                            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
                        } else {
                            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                        },
                    };
                    let contracts =
                        crate::capture::contracts::facts::project_trait_requirement_contracts(
                            input,
                            signature,
                            &context,
                            &[],
                        )?;
                    return Ok(contracts);
                }
            }
        }
        Err(super::rejected(
            "assumed binding contract lost its exact requirement declaration",
        ))
    };
    let relationships = facts
        .assumed_bindings()
        .iter()
        .map(|row| format!("{} -> {}", row.required, row.supplied))
        .collect::<Vec<_>>()
        .join("; ");
    let implementations = facts
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().map(move |row| {
                format!(
                    "{}: {}",
                    plan.identity_text(row.requirement_identity),
                    match &row.binding {
                        effects::provider_plan::ProviderBinding::CheckedAdapter {
                            machine_identity,
                            ..
                        } => plan.identity_text(*machine_identity).to_owned(),
                        other => format!("{other:?}"),
                    }
                )
            })
        })
        .collect::<Vec<_>>()
        .join("; ");
    let render = |contracts: &[crate::record::PackageReviewCallableContract]| {
        crate::record::PackageReviewCallableContract::canonical_set_text(contracts)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])
    };
    let mut rows = Vec::new();
    for row in facts.assumed_bindings() {
        let required = contract(&row.required)?;
        let supplied = contract(&row.supplied)?;
        let missing_entry = supplied
            .iter()
            .filter(|contract| {
                contract.kind() == crate::record::PackageReviewContractKind::Requires
                    && !required.contains(contract)
            })
            .cloned()
            .collect::<Vec<_>>();
        let missing_exit = required
            .iter()
            .filter(|contract| {
                contract.kind() == crate::record::PackageReviewContractKind::Ensures
                    && !supplied.contains(contract)
            })
            .cloned()
            .collect::<Vec<_>>();
        rows.push(format!(
            "Extremely high risk - assumed binding contract\nroot Product: {}; target: {}\nrequired: {} [{}]\nsupplied: {} [{}]\nsubject mapping: positional, runtime-only; no additional domain establishment authority\nassumed entry difference (supplier requires absent from caller): {}\nassumed exit difference (caller ensures absent from supplier): {}\nconnection assessment: {}\nselected implementations: {}\ntransitive relationships: {}\nselected closure: {}\nconsumed semantic source closure: {}\ndependency path: {:?}\n",
            sources.package_identity().map(|identity| hex(&identity.digest())).unwrap_or_default(),
            input.custody.execution().selected_target_profile.map(|target| target.target_name()).unwrap_or(""),
            row.required, render(&required)?, row.supplied, render(&supplied)?,
            render(&missing_entry)?, render(&missing_exit)?, assessment(&required, &supplied),
            implementations, relationships, hex(facts.identity_digest().as_bytes()), hex(&source.digest()), sources.dependency_closure()
        ));
    }
    Ok(rows)
}

/// Audit classification only: a recognized contradiction does not veto an
/// explicitly accepted semantic link or grant any proposition. Unknown clauses
/// retain their complete canonical text above and establish no consistency.
fn assessment(
    required: &[crate::record::PackageReviewCallableContract],
    supplied: &[crate::record::PackageReviewCallableContract],
) -> String {
    let required = result_interval(required);
    let supplied = result_interval(supplied);
    if required.0 <= required.1
        && supplied.0 <= supplied.1
        && (required.1 < supplied.0 || supplied.1 < required.0)
    {
        format!(
            "refuted exit connection: disjoint declared integer result bounds caller {required:?}, supplier {supplied:?}"
        )
    } else {
        "unproved connection; no contradiction established by the retained literal-result comparison audit".to_owned()
    }
}

fn result_interval(contracts: &[crate::record::PackageReviewCallableContract]) -> (i128, i128) {
    use crate::record::{
        PackageReviewContractBinaryOperator as Operator,
        PackageReviewContractExpression as Expression, PackageReviewContractFact as Fact,
        PackageReviewContractKind as Kind, PackageReviewContractOperatorMeaning as Meaning,
    };
    fn visit(expression: &Expression, bounds: &mut (i128, i128)) {
        let Expression::Binary {
            meaning: Meaning::Builtin,
            operator,
            left,
            right,
        } = expression
        else {
            return;
        };
        if *operator == Operator::And {
            visit(left, bounds);
            visit(right, bounds);
            return;
        }
        let (Expression::Result, Expression::Integer(value)) = (left.as_ref(), right.as_ref())
        else {
            return;
        };
        let Ok(value) = value.parse::<i128>() else {
            return;
        };
        match operator {
            Operator::Equal => {
                bounds.0 = bounds.0.max(value);
                bounds.1 = bounds.1.min(value);
            }
            Operator::LessOrEqual => bounds.1 = bounds.1.min(value),
            Operator::GreaterOrEqual => bounds.0 = bounds.0.max(value),
            Operator::Less => {
                if let Some(value) = value.checked_sub(1) {
                    bounds.1 = bounds.1.min(value);
                }
            }
            Operator::Greater => {
                if let Some(value) = value.checked_add(1) {
                    bounds.0 = bounds.0.max(value);
                }
            }
            _ => {}
        }
    }
    let mut bounds = (i128::MIN, i128::MAX);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind() == Kind::Ensures && contract.result_case().is_none())
    {
        if let Fact::Expression(expression) = contract.fact() {
            visit(expression, &mut bounds);
        }
    }
    bounds
}
