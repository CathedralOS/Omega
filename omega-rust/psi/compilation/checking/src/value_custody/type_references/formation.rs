//! Callable type recipes use the exact signature's incoming assumptions.
//! They do not borrow guarantees or another state's same-named parameters.
//! Requirements themselves are formed independently, before their truth can
//! justify a dependent type; a partial condition cannot establish itself.

use super::*;
use crate::proof_contracts::arithmetic_domains::LocalValueFormation;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::SignatureContractKind;

impl<'program> TypeReferenceOwner<'program> {
    pub(super) fn local_value_formation(self) -> Option<LocalValueFormation<'program>> {
        match self {
            Self::Expression {
                machine,
                current_state,
                value_environment,
                ..
            }
            | Self::StateLocalData {
                machine,
                current_state,
                value_environment,
                ..
            } => Some(LocalValueFormation {
                machine,
                state: current_state,
                environment: value_environment,
            }),
            _ => None,
        }
    }

    pub(super) fn value_formation_facts(
        self,
        program: &dyn crate::CheckingInput,
    ) -> Vec<ExpressionHandle> {
        let signature = match self {
            Self::StateParameter { signature, .. } | Self::StateReturn { signature, .. } => {
                signature
            }
            _ => return Vec::new(),
        };
        let requires = SignatureContractKind::Requires;
        if let Some(machine) = program.machine_holding_state(signature)
            && let Some(state) = program.state_in_machine(machine, signature)
        {
            let mut facts = program
                .contract_expressions(program.state_contracts(state).iter(), requires.clone())
                .collect::<Vec<_>>();
            if program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == signature)
            {
                facts.extend(
                    program
                        .contract_expressions(program.machine_contracts(machine).iter(), requires),
                );
            }
            return facts;
        }
        let signature = program
            .trait_machine_signature(signature)
            .map(|(_, signature)| signature)
            .or_else(|| {
                program
                    .retained_static_machine_contract(signature)
                    .map(|contract| contract.signature())
            });
        signature.map_or_else(Vec::new, |signature| {
            program
                .contract_expressions(
                    program.state_signature_contracts(signature).iter(),
                    requires,
                )
                .collect()
        })
    }
}

pub(crate) fn validate_expression_type_application(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    if !reference.is_valid() {
        return;
    }
    let (parameters, lifetimes) =
        program.machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol);
    validate_type_reference_handle(
        program,
        reference,
        symbols,
        diagnostics,
        TypeReferenceOwner::Expression {
            machine,
            current_state: Some(state),
            value_environment: environment,
            generic_depth: 0,
        },
        TypeParameterScope::authored(machine.symbol, &parameters, &lifetimes),
        obligations,
    );
}
