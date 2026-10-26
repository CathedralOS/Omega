//! A call's qualification source is independent of its selected public clauses.
//! Checked adapters owe inherited boundary clauses at installation; calling
//! their bodies ordinarily does not turn that obligation into issuer evidence.

use checked_trees::BoundaryQualificationAuthorization;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateSignature};

pub enum CallQualificationSource<'program> {
    CheckedBody(&'program Machine),
    AdmittedRequirement(BoundaryQualificationAuthorization),
    Propagated,
}

/// Classify the actual invoked declaration before transporting a membership
/// or carry conclusion. The authorization belongs to the exact selected
/// requirement fact; it is not inferred from a matching implementation name.
pub fn call_qualification_source(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    authorization: Option<BoundaryQualificationAuthorization>,
) -> Option<CallQualificationSource<'_>> {
    CallQualificationTarget::resolve(program, target).fact_source(authorization)
}

enum CallQualificationTarget<'program> {
    CheckedBody(&'program Machine),
    AdmittedRequirement,
    /// An actual requirement declaration, distinct from a bodyless adapter
    /// that merely inherits a public signature for installation.
    BoundaryRequirement {
        invoked: SymbolHandle,
        declaration: SymbolHandle,
        signature: SymbolHandle,
    },
    Propagated,
}

impl<'program> CallQualificationTarget<'program> {
    fn resolve(program: &'program dyn crate::CheckingInput, target: SymbolHandle) -> Self {
        if let Some(machine) = program.machine_by_own_or_state_symbol(target) {
            return if machine.body_is_present
                && matches!(
                    machine.supply_mode,
                    MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
                ) {
                Self::CheckedBody(machine)
            } else {
                Self::AdmittedRequirement
            };
        }
        // A public member or evidence-binder child names its declaring trait
        // through the exact requirement query, not trait_definition_by_symbol.
        if let Some((declaration, signature)) =
            crate::selected_requirement_signature(program, target)
                .filter(|(declaration, _)| declaration.is_boundary)
        {
            return Self::BoundaryRequirement {
                invoked: target,
                declaration: declaration.symbol,
                signature: signature.symbol,
            };
        }
        if program
            .trait_definition_by_symbol(target)
            .is_some_and(|definition| definition.is_boundary)
        {
            return Self::AdmittedRequirement;
        }
        Self::Propagated
    }

    fn fact_source(
        &self,
        authorization: Option<BoundaryQualificationAuthorization>,
    ) -> Option<CallQualificationSource<'program>> {
        match self {
            Self::CheckedBody(machine) => authorization
                .is_none()
                .then_some(CallQualificationSource::CheckedBody(machine)),
            Self::AdmittedRequirement | Self::BoundaryRequirement { .. } => {
                authorization.map(CallQualificationSource::AdmittedRequirement)
            }
            Self::Propagated => Some(CallQualificationSource::Propagated),
        }
    }
}

/// Qualification clauses of one exact selected call contract. Prepare its
/// public requirement owner once, while keeping the actual invoked declaration
/// separate. Issuer evidence and conditional membership obligations are distinct:
/// an exact bodyless requirement may promise a conserved result without minting
/// its qualification. A checked body alone still cannot authenticate a public
/// issuer clause that its adapter inherits for installation.
pub struct CallQualificationContract<'program> {
    program: &'program dyn crate::CheckingInput,
    target: CallQualificationTarget<'program>,
    clauses: CallQualificationClauses<'program>,
}

enum CallQualificationClauses<'program> {
    ActualBody,
    Requirement {
        declaration: SymbolHandle,
        signature: &'program StateSignature,
    },
    UnresolvedRequirement,
}

impl<'program> CallQualificationContract<'program> {
    pub fn prepare(
        program: &'program dyn crate::CheckingInput,
        target: SymbolHandle,
        selected: Option<&'program StateSignature>,
    ) -> Self {
        let clauses = match selected {
            None => match crate::selected_requirement_signature(program, target) {
                Some((declaration, signature)) => CallQualificationClauses::Requirement {
                    declaration: declaration.symbol,
                    signature,
                },
                None => CallQualificationClauses::ActualBody,
            },
            Some(signature) => {
                match crate::selected_requirement_signature(program, signature.symbol) {
                    Some((declaration, _)) => CallQualificationClauses::Requirement {
                        declaration: declaration.symbol,
                        signature,
                    },
                    None => CallQualificationClauses::UnresolvedRequirement,
                }
            }
        };
        Self {
            program,
            target: CallQualificationTarget::resolve(program, target),
            clauses,
        }
    }

    /// An exact membership clause the invoked bodyless boundary requirement
    /// owes on normal completion. This is obligation vocabulary, not issuer
    /// evidence: consumers must still join the actual invocation, consumed
    /// claims, conservation or authorized fresh supply, and result custody.
    /// Ordinary adapters cannot obtain this route from inherited clauses.
    pub fn declares_membership_obligation(
        &self,
        kind: SignatureContractKind,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> bool {
        let (
            CallQualificationTarget::BoundaryRequirement {
                invoked,
                declaration: actual_declaration,
                signature: actual_signature,
            },
            CallQualificationClauses::Requirement {
                declaration,
                signature,
            },
        ) = (&self.target, &self.clauses)
        else {
            return false;
        };
        if kind != SignatureContractKind::Ensures
            || actual_declaration != declaration
            || !self.program.proof_facts.is_valid(fact)
            || self.program.proof_facts.get(fact).as_membership().is_none()
        {
            return false;
        }
        // A closed executable target already names the complete application.
        // Matching its template alone would borrow another instance's promise.
        // An open declaration may instead select its invocation's applied view;
        // that view is supplied by requirement_call_signature, not guessed here.
        if self
            .program
            .closed_requirement_application(*invoked)
            .is_some()
            && *actual_signature != signature.symbol
        {
            return false;
        }
        if let (Some(actual), Some(selected)) = (
            self.program
                .closed_requirement_application(*actual_signature),
            self.program
                .closed_requirement_application(signature.symbol),
        ) && actual.trait_application_identity != selected.trait_application_identity
        {
            return false;
        }
        let declaration_signature = |selected| {
            self.program
                .closed_requirement_application(selected)
                .map_or(Some(selected), |application| {
                    (application.declaring_trait == *declaration).then_some(application.template)
                })
        };
        if declaration_signature(*actual_signature).is_none()
            || declaration_signature(*actual_signature) != declaration_signature(signature.symbol)
        {
            return false;
        }
        // The fact handle must belong to this selected signature's own clause;
        // equal membership payloads from another contract are not its promise.
        self.program
            .state_signature_contracts(signature)
            .iter()
            .any(|contract| {
                contract.kind == kind && contract.facts.handles().any(|owned| owned == fact)
            })
    }

    /// Read only a clause from this selected signature (or the actual body's
    /// own contract). Withholding an unauthorized promise is not a new call
    /// refusal: the consuming membership obligation must still be proved.
    pub fn fact_source(
        &self,
        kind: SignatureContractKind,
        fact: &typed_trees::domain::ProofFact,
    ) -> Option<CallQualificationSource<'program>> {
        let authorization = match &self.clauses {
            CallQualificationClauses::ActualBody => None,
            CallQualificationClauses::Requirement {
                declaration,
                signature,
            } => super::boundary_qualification_authorization(
                self.program,
                *declaration,
                signature,
                kind,
                fact,
            ),
            // A selected public contract cannot borrow the body's authority
            // when its exact declaring requirement failed to rejoin.
            CallQualificationClauses::UnresolvedRequirement => return None,
        };
        self.target.fact_source(authorization)
    }
}
