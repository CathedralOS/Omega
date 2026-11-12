// We check that boundary requirements connected by `AssumedBindingContract`
// declarations have compatible source signatures, operational/lifetime
// envelopes, and positional lifetime subject mappings before publishing the
// shared intermediate program. We keep each compatibility verdict beside its
// contract and report it when that contract's Calling plan is validated,
// preserving diagnostic ordering without retaining the checked program.

use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AssumedBindingContract;

pub(crate) struct PreparedAssumedBindings {
    pub coordinates: Result<Vec<effects::AssumedBindingContractSelection>, Vec<Diagnostic>>,
    pub rows: Vec<PreparedAssumedBinding>,
}

pub(crate) struct PreparedAssumedBinding {
    contract: AssumedBindingContract,
    source_compatibility: Result<(), Vec<Diagnostic>>,
}

impl PreparedAssumedBindings {
    pub(crate) fn capture(typed: &TypedTrees, contracts: &[AssumedBindingContract]) -> Self {
        Self {
            coordinates: coordinates(typed, contracts),
            rows: contracts
                .iter()
                .map(|contract| PreparedAssumedBinding {
                    contract: contract.clone(),
                    source_compatibility: source_compatibility(typed, contract),
                })
                .collect(),
        }
    }
}

impl PreparedAssumedBinding {
    pub(crate) fn validate_calling(
        self,
        calling: &[BoundaryCallingPlanRealization],
    ) -> Result<(), Vec<Diagnostic>> {
        self.source_compatibility?;
        let required = calling
            .iter()
            .find(|row| row.requirement_machine == self.contract.required);
        let supplied = calling
            .iter()
            .find(|row| row.requirement_machine == self.contract.supplied);
        let compatible = match (required, supplied) {
            (None, None) => true,
            (Some(required), Some(supplied)) => {
                required.exact_boundary_entry_plan() == supplied.exact_boundary_entry_plan()
                    && required.materialized_signature() == supplied.materialized_signature()
            }
            _ => false,
        };
        if !compatible {
            return Err(vec![
                Diagnostic::error(
                    "assumed binding has incompatible validated layout or calling plan",
                )
                .with_source_span(self.contract.source_span),
            ]);
        }
        Ok(())
    }
}

fn coordinates(
    typed: &TypedTrees,
    contracts: &[AssumedBindingContract],
) -> Result<Vec<effects::AssumedBindingContractSelection>, Vec<Diagnostic>> {
    let identity = |symbol| {
        typed
            .traits()
            .iter()
            .find_map(|definition| {
                typed
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
                    .map(|signature| {
                        typed.trait_requirement_callable_identity_text(definition, signature)
                    })
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "assumed binding lost exact requirement identity",
                )]
            })
    };
    contracts
        .iter()
        .map(|contract| {
            Ok(effects::AssumedBindingContractSelection {
                required: identity(contract.required)?,
                supplied: identity(contract.supplied)?,
            })
        })
        .collect()
}

fn source_compatibility(
    typed: &TypedTrees,
    contract: &AssumedBindingContract,
) -> Result<(), Vec<Diagnostic>> {
    let signature = |symbol| {
        typed.traits().iter().find_map(|definition| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.symbol == symbol)
        })
    };
    let required_signature = signature(contract.required)
        .ok_or_else(|| vec![Diagnostic::error("assumed binding lost required signature")])?;
    let supplied_signature = signature(contract.supplied)
        .ok_or_else(|| vec![Diagnostic::error("assumed binding lost supplied signature")])?;
    let required_parameters = typed.state_signature_parameters(required_signature);
    let supplied_parameters = typed.state_signature_parameters(supplied_signature);
    if required_parameters.len() != supplied_parameters.len()
        || typed.normalized_type_identity(required_signature.return_type)
            != typed.normalized_type_identity(supplied_signature.return_type)
        || required_parameters
            .iter()
            .zip(supplied_parameters)
            .any(|(required, supplied)| {
                typed.normalized_type_identity(required.type_reference)
                    != typed.normalized_type_identity(supplied.type_reference)
            })
    {
        return Err(vec![Diagnostic::error("assumed binding has incompatible runtime carriers, parameter order or passing modes").with_source_span(contract.source_span)]);
    }
    if (!required_signature.suspends && supplied_signature.suspends)
        || (!required_signature.blocks && supplied_signature.blocks)
        || required_signature.termination_guarantee != supplied_signature.termination_guarantee
        || !required_signature.invokes.is_empty()
        || !supplied_signature.invokes.is_empty()
        || required_signature.lifetime_parameters.len()
            != supplied_signature.lifetime_parameters.len()
    {
        return Err(vec![Diagnostic::error("assumed binding has incompatible or unsupported operational/lifetime correspondence").with_source_span(contract.source_span)]);
    }

    let lifetime_mapping = required_signature
        .lifetime_parameters
        .iter()
        .zip(&supplied_signature.lifetime_parameters)
        .map(|(required, supplied)| (required.as_str().to_owned(), supplied.as_str().to_owned()))
        .collect::<Vec<_>>();
    let scopes = [
        checked_trees::typed_trees::type_identity::AppliedTypeIdentityScope {
            declaration: required_signature.symbol,
            arguments: &[],
            lifetimes: &lifetime_mapping,
        },
    ];
    let corresponding = |required, supplied| {
        let required = checked_trees::typed_trees::type_identity::applied_type_identity_in_scope(
            typed,
            required,
            required_signature.symbol,
            supplied_signature.symbol,
            &scopes,
        );
        let supplied = checked_trees::typed_trees::type_identity::applied_type_identity(
            typed,
            supplied,
            supplied_signature.symbol,
            &[],
        );
        required.is_some() && required == supplied
    };
    if !corresponding(
        required_signature.return_type,
        supplied_signature.return_type,
    ) || required_parameters
        .iter()
        .zip(supplied_parameters)
        .any(|(required, supplied)| {
            !corresponding(required.type_reference, supplied.type_reference)
        })
    {
        return Err(vec![
            Diagnostic::error("assumed binding has incompatible lifetime subject mapping")
                .with_source_span(contract.source_span),
        ]);
    }
    Ok(())
}
