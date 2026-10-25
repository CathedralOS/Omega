//! Callable type recipes use the exact signature's incoming assumptions.
//! They do not borrow guarantees or another state's same-named parameters.
//! Requirements themselves are formed independently, before their truth can
//! justify a dependent type; a partial condition cannot establish itself.

use super::TypeReferenceOwner;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::SignatureContractKind;

impl TypeReferenceOwner<'_> {
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
