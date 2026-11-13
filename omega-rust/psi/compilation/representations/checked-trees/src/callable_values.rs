//! Reserve callable contracts before lowering types that can refer to them.
//!
//! Equal complete signatures share a contract row. The source handle retains its
//! generation, and selected machine bodies never participate in this numbering.

use crate::KeyedRows;
use arena::Arena;
use semantic_vocabulary::CallableContractId;
use typed_trees::TypedTrees;
use typed_trees::callable_values::RuntimeCallableContractHandle;
use typed_trees::type_identity::TypeIdentityId;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallableContract {
    pub id: CallableContractId,
    pub source: RuntimeCallableContractHandle,
    pub identity: TypeIdentityId,
    pub signature: Option<CheckedCallableSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallableSignature {
    pub scalar_parameters: Vec<crate::CheckedStructuralScalarParameterPlan>,
    pub structural_parameters: Vec<crate::CheckedUnitStructuralParameterPlan>,
    pub erased_scalar_parameters: Vec<crate::CheckedStructuralScalarParameterPlan>,
    pub erased_proof_parameters: Vec<crate::CheckedErasedProofParameterPlan>,
    pub result: crate::CheckedBoundaryMachineResultPlan,
    pub entry_claim_paths: Vec<crate::CheckedDynamicClaimPath>,
    pub result_claim_paths: Vec<Vec<crate::CheckedUnitStructuralPathSegment>>,
    pub result_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
}

impl CheckedCallableSignature {
    pub fn structural_type_roots(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
        self.structural_parameters
            .iter()
            .map(|parameter| parameter.type_identity)
            .chain(self.result.structural_identity())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
struct CallableContractBinding {
    source: RuntimeCallableContractHandle,
    contract: arena::Handle<Option<CheckedCallableContract>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallableContracts {
    contracts: Arena<Option<CheckedCallableContract>>,
    bindings: Arena<CallableContractBinding>,
    by_source: KeyedRows<RuntimeCallableContractHandle, CallableContractBinding>,
    by_id: KeyedRows<Option<CallableContractId>, Option<CheckedCallableContract>>,
}

impl CheckedCallableContracts {
    pub fn from_typed(program: &TypedTrees) -> Result<Self, String> {
        let mut grouped = std::collections::BTreeMap::<String, Vec<_>>::new();
        for (source, _) in program.runtime_callable_contracts.iter() {
            let identity = program
                .runtime_callable_contract_identity(source, &[])
                .map_err(|reason| format!("cannot retain runtime callable contract: {reason}"))?;
            grouped.entry(identity).or_default().push(source);
        }
        let mut retained = Self::default();
        for (ordinal, (identity, sources)) in grouped.into_iter().enumerate() {
            let number = u64::try_from(ordinal)
                .ok()
                .and_then(|number| number.checked_add(1))
                .ok_or("runtime callable contract count exceeds semantic identity range")?;
            let id = CallableContractId::new(number)
                .ok_or("runtime callable contract has an invalid semantic identity")?;
            let source = *sources
                .first()
                .ok_or("callable contract has no source owner")?;
            let contract = retained.contracts.insert(Some(CheckedCallableContract {
                id,
                source,
                identity: program.type_identities.intern(&identity),
                signature: None,
            }));
            for source in sources {
                retained
                    .bindings
                    .insert(CallableContractBinding { source, contract });
            }
        }
        Ok(retained)
    }

    pub fn contracts(&self) -> impl Iterator<Item = &CheckedCallableContract> {
        self.contracts
            .iter()
            .filter_map(|(_, contract)| contract.as_ref())
    }

    pub fn contract_for_source(
        &self,
        source: RuntimeCallableContractHandle,
    ) -> Result<&CheckedCallableContract, &'static str> {
        let binding = self.by_source.with_rows(
            &self.bindings,
            |binding| binding.source,
            &source,
            |rows| match rows {
                [binding] => Ok(*binding),
                [] => Err("runtime callable has no retained contract association"),
                _ => Err("runtime callable has conflicting retained contract associations"),
            },
        )?;
        self.contracts
            .get(self.bindings.get(binding).contract)
            .as_ref()
            .ok_or("runtime callable association names an absent contract row")
    }

    pub fn contract(
        &self,
        id: CallableContractId,
    ) -> Result<&CheckedCallableContract, &'static str> {
        let contract = self.by_id.with_rows(
            &self.contracts,
            |contract| contract.as_ref().map(|row| row.id),
            &Some(id),
            |rows| match rows {
                [contract] => Ok(*contract),
                [] => Err("callable identity names no retained contract"),
                _ => Err("callable identity names conflicting contracts"),
            },
        )?;
        self.contracts
            .get(contract)
            .as_ref()
            .ok_or("callable identity names an absent contract row")
    }

    pub fn retain_signatures(
        &mut self,
        signatures: Vec<(CallableContractId, CheckedCallableSignature)>,
    ) -> Result<(), &'static str> {
        let mut pending = self
            .contracts
            .iter()
            .filter_map(|(handle, row)| row.as_ref().map(|row| (row.id, handle)))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (id, signature) in signatures {
            let handle = pending
                .remove(&id)
                .ok_or("callable signature has no unique reserved contract")?;
            let row = self
                .contracts
                .get_mut(handle)
                .as_mut()
                .ok_or("callable signature names an absent contract row")?;
            if row.signature.is_some() {
                return Err("callable signature was already retained");
            }
            row.signature = Some(signature);
        }
        if !pending.is_empty() {
            return Err("a reserved callable contract has no complete signature plan");
        }
        self.by_source
            .seal(&self.bindings, |binding| binding.source);
        self.by_id.seal(&self.contracts, |contract| {
            contract.as_ref().map(|row| row.id)
        });
        Ok(())
    }
}

impl CheckedCallableContract {
    pub fn signature(&self) -> Result<&CheckedCallableSignature, &'static str> {
        self.signature
            .as_ref()
            .ok_or("callable contract signature planning is unfinished")
    }
}
