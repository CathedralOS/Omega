//! Source judgments for exact executed Product endpoint pairs.
//!
//! These rows classify public entry and exit claims. They do not establish a
//! complete dependency closure, domain issuance, root consent or installation.
//! Claim handles belong to the enclosing CheckedTrees, while each demand keeps
//! its original frozen application issuer. Absence rows never supply evidence.

use crate::{ContractProofFact, ContractProofFactOwner};
use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::signature::SignatureContract;
use typed_trees::static_applications::AssumedBindingContractDemand;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssumedBindingContractSide {
    #[default]
    Required,
    Supplied,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssumedBindingSemanticDirection {
    #[default]
    Entry,
    Exit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssumedBindingSemanticSubject {
    Parameter {
        ordinal: usize,
        symbol: SymbolHandle,
    },
    Result,
    #[default]
    Predicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingResultCase {
    pub data: SymbolHandle,
    pub case: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssumedBindingSemanticSource {
    #[default]
    Absent,
    Contract(Handle<ContractProofFact>),
    ResultCase {
        contract: Handle<SignatureContract>,
        fact: Handle<ProofFact>,
    },
    DeclaredType {
        type_reference: TypeReferenceHandle,
        constraint_ordinal: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssumedBindingPublicClaim {
    side: AssumedBindingContractSide,
    subject: AssumedBindingSemanticSubject,
    source: AssumedBindingSemanticSource,
    result_case: Option<AssumedBindingResultCase>,
}

impl AssumedBindingPublicClaim {
    pub const fn new(
        side: AssumedBindingContractSide,
        subject: AssumedBindingSemanticSubject,
        source: AssumedBindingSemanticSource,
        result_case: Option<AssumedBindingResultCase>,
    ) -> Self {
        Self {
            side,
            subject,
            source,
            result_case,
        }
    }
    pub const fn side(&self) -> AssumedBindingContractSide {
        self.side
    }
    pub const fn subject(&self) -> AssumedBindingSemanticSubject {
        self.subject
    }
    pub const fn source(&self) -> AssumedBindingSemanticSource {
        self.source
    }
    pub const fn result_case(&self) -> Option<AssumedBindingResultCase> {
        self.result_case
    }
}

/// Refutation is relative to the readable premises, not a consistency claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssumedBindingSemanticJudgment {
    Proven,
    #[default]
    Assumed,
    Refuted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssumedBindingSemanticObligation {
    goal: AssumedBindingPublicClaim,
    judgment: AssumedBindingSemanticJudgment,
}

impl AssumedBindingSemanticObligation {
    pub const fn new(
        goal: AssumedBindingPublicClaim,
        judgment: AssumedBindingSemanticJudgment,
    ) -> Self {
        Self { goal, judgment }
    }
    pub const fn goal(&self) -> &AssumedBindingPublicClaim {
        &self.goal
    }
    pub const fn judgment(&self) -> AssumedBindingSemanticJudgment {
        self.judgment
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingDirectionalObligations {
    direction: AssumedBindingSemanticDirection,
    claims: Arena<AssumedBindingPublicClaim>,
    premise_span: HandleSpan<AssumedBindingPublicClaim>,
    judgments: Arena<AssumedBindingSemanticObligation>,
    obligation_span: HandleSpan<AssumedBindingSemanticObligation>,
}

impl AssumedBindingDirectionalObligations {
    pub fn new(
        direction: AssumedBindingSemanticDirection,
        premises: Vec<AssumedBindingPublicClaim>,
        obligations: Vec<AssumedBindingSemanticObligation>,
    ) -> Self {
        let mut claims = Arena::new();
        let premise_span = claims.insert_many(premises);
        let mut judgments = Arena::new();
        let obligation_span = judgments.insert_many(obligations);
        Self {
            direction,
            claims,
            premise_span,
            judgments,
            obligation_span,
        }
    }
    pub const fn direction(&self) -> AssumedBindingSemanticDirection {
        self.direction
    }
    pub fn premises(&self) -> &[AssumedBindingPublicClaim] {
        self.claims.span_or_empty(self.premise_span)
    }
    pub fn obligations(&self) -> &[AssumedBindingSemanticObligation] {
        self.judgments.span_or_empty(self.obligation_span)
    }
    pub fn premises_for<'connection>(
        &'connection self,
        goal: &AssumedBindingPublicClaim,
    ) -> impl Iterator<Item = &'connection AssumedBindingPublicClaim> {
        let result_case = goal.result_case();
        self.premises().iter().filter(move |premise| {
            premise.result_case().is_none() || premise.result_case() == result_case
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingSemanticConnection {
    entry: AssumedBindingDirectionalObligations,
    exit: AssumedBindingDirectionalObligations,
}

impl AssumedBindingSemanticConnection {
    pub const fn new(
        entry: AssumedBindingDirectionalObligations,
        exit: AssumedBindingDirectionalObligations,
    ) -> Self {
        Self { entry, exit }
    }
    pub const fn entry(&self) -> &AssumedBindingDirectionalObligations {
        &self.entry
    }
    pub const fn exit(&self) -> &AssumedBindingDirectionalObligations {
        &self.exit
    }
}

/// Public fact ownership is independent of native schema or selected providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPublicContractOwner {
    Trait {
        owner: SymbolHandle,
        signature: SymbolHandle,
    },
    Machine {
        machine: SymbolHandle,
        entry: SymbolHandle,
    },
}

impl CheckedPublicContractOwner {
    pub fn owns_contract(self, fact: ContractProofFactOwner) -> bool {
        match (self, fact) {
            (
                Self::Trait { owner, signature },
                ContractProofFactOwner::StateSignature {
                    owner_symbol,
                    state_symbol,
                },
            ) => owner == owner_symbol && signature == state_symbol,
            (Self::Machine { machine, .. }, ContractProofFactOwner::Machine { machine_symbol }) => {
                machine == machine_symbol
            }
            (
                Self::Machine { machine, entry },
                ContractProofFactOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                },
            ) => machine == machine_symbol && entry == state_symbol,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedAssumedBindingContractJudgment {
    demand: AssumedBindingContractDemand,
    required: SymbolHandle,
    supplied: SymbolHandle,
    connection: AssumedBindingSemanticConnection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedAssumedBindingContracts {
    rows: Arena<Option<CheckedAssumedBindingContractJudgment>>,
}

impl CheckedAssumedBindingContracts {
    pub fn retain(
        &mut self,
        demand: AssumedBindingContractDemand,
        required: SymbolHandle,
        supplied: SymbolHandle,
        connection: AssumedBindingSemanticConnection,
    ) {
        self.rows
            .append(Some(CheckedAssumedBindingContractJudgment {
                demand,
                required,
                supplied,
                connection,
            }));
    }
    pub fn connection(
        &self,
        demand: &AssumedBindingContractDemand,
        required: SymbolHandle,
        supplied: SymbolHandle,
    ) -> Option<&AssumedBindingSemanticConnection> {
        if !required.is_valid() || !supplied.is_valid() {
            return None;
        }
        let mut rows = self
            .rows
            .iter()
            .filter_map(|(_, row)| row.as_ref())
            .filter(|row| {
                row.demand == *demand && row.required == required && row.supplied == supplied
            });
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        Some(&row.connection)
    }
}
