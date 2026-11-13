//! A stored machine keeps its public contract independently of its selected code.
//! The reader registers only contracts and selections the program actually names;
//! lowering copies these rows into the module without rediscovering declarations.

use semantic_vocabulary::{
    CallableContractId, ContractId, IdTable, MachineDescriptorId, MachineId, ObligationId,
};
use terminal_check_tables::{CheckTables, Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, TerminalCallableCatalog,
    TerminalCallableContract, TerminalMachineDescriptor, TerminalMachineDescriptorOrigin,
    TerminalMachineEnvelope, TerminalMachineResult, TerminalModule,
};
use typed_trees::callable_values::RuntimeCallableContractHandle;

use crate::Signature;

#[derive(Debug, Clone, Default)]
pub(crate) struct CallableNamespace {
    pub contracts: IdTable<CallableContractId, CallableContract>,
    pub descriptors: IdTable<MachineDescriptorId, MachineSelection>,
    pub envelopes: IdTable<MachineId, Option<crate::reach::SourceEnvelope>>,
    sources: Vec<Option<(RuntimeCallableContractHandle, CallableContractId)>>,
    pub resolving: Vec<RuntimeCallableContractHandle>,
}

#[derive(Debug, Clone)]
pub(crate) struct CallableContract {
    pub identity: String,
    pub signature: Signature,
    pub envelope: crate::reach::SourceEnvelope,
}

#[derive(Debug, Clone)]
pub(crate) struct MachineSelection {
    pub contract: CallableContractId,
    pub machine: MachineId,
    pub application: String,
    pub conformance: Option<String>,
    pub requirement: Option<String>,
}

impl CallableNamespace {
    pub fn bind_machine_envelope(
        &mut self,
        machine: MachineId,
        envelope: crate::reach::SourceEnvelope,
    ) -> Result<(), &'static str> {
        while self.envelopes.len() < machine.get() as usize {
            self.envelopes.push(None);
        }
        let retained = self
            .envelopes
            .get_mut(machine)
            .ok_or("named callable envelope lost its machine coordinate")?;
        match retained {
            Some(previous) if previous != &envelope => {
                Err("named callable changes its retained operational contract")
            }
            _ => {
                *retained = Some(envelope);
                Ok(())
            }
        }
    }

    /// A source coordinate is useful only with the exact generation it names.
    /// Different source rows may share the same normalized contract identity.
    pub fn source_contract(
        &self,
        source: RuntimeCallableContractHandle,
    ) -> Result<Option<CallableContractId>, &'static str> {
        match self.sources.get(source.arena_index() as usize) {
            Some(Some((retained, contract))) if *retained == source => Ok(Some(*contract)),
            Some(Some(_)) => Err("callable type names a stale signature generation"),
            Some(None) | None => Ok(None),
        }
    }

    pub fn bind_source(
        &mut self,
        source: RuntimeCallableContractHandle,
        contract: CallableContractId,
    ) -> Result<(), &'static str> {
        let ordinal = source.arena_index() as usize;
        if self.sources.len() <= ordinal {
            self.sources.resize(ordinal + 1, None);
        }
        match self.sources[ordinal] {
            Some((retained, previous)) if retained != source || previous != contract => {
                Err("callable type changes its retained contract association")
            }
            _ => {
                self.sources[ordinal] = Some((source, contract));
                Ok(())
            }
        }
    }

    /// Copy the reader's contracts into the module using the ordinary
    /// signature lowering. The rows only name obligations here; the checker
    /// must supply their certificates before it can publish an artifact.
    pub(crate) fn publish(
        &self,
        context: MachineId,
        signatures: &IdTable<MachineId, Signature>,
        outcomes: &IdTable<MachineId, crate::Outcome>,
        records: &crate::lower::places::Records<'_>,
        boundaries: &crate::boundaries::BoundaryNamespace,
        tables: &CheckTables,
        services: &crate::reach::ServiceMap,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let refuse = |reason: &str| Refusal {
            reason: RefusalReason::Unsupported,
            machine: context,
            concerns: Construct::Machine,
            detail: reason.to_owned(),
        };
        if module.callables != TerminalCallableCatalog::default() {
            return Err(refuse("callable catalog was already published"));
        }
        let mut next_obligation = tables
            .machines
            .iter()
            .flat_map(|(_, machine)| machine.obligations.keys())
            .map(ObligationId::get)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| refuse("callable obligation identity overflow"))?;
        let mut obligation = || {
            let id = ObligationId::new(next_obligation)
                .ok_or_else(|| refuse("callable obligation identity is zero"))?;
            next_obligation = next_obligation
                .checked_add(1)
                .ok_or_else(|| refuse("callable obligation identity overflow"))?;
            Ok(id)
        };
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let machine_count = u64::try_from(module.machines.len())
            .map_err(|_| refuse("callable contract namespace exceeds its identity width"))?;
        let mut catalog = TerminalCallableCatalog::default();
        for (id, contract) in self.contracts.iter() {
            let header = crate::lower::signature_header(
                context,
                &contract.signature,
                signatures,
                outcomes,
                records,
                self,
                boundaries,
            )?;
            let mut proof = header.proof;
            proof.id = machine_count
                .checked_add(id.get())
                .and_then(ContractId::new)
                .ok_or_else(|| refuse("callable contract identity overflow"))?;
            proof.source_checked_requires = proof.requires.clone();
            for guarantee in &mut proof.ensures {
                guarantee.obligation = obligation()?;
            }
            let result = match &header.result {
                TerminalMachineResult::Unit => BoundaryMachineResult::Unit,
                TerminalMachineResult::Scalar(value) => {
                    BoundaryMachineResult::Scalar(value.value_type())
                }
                TerminalMachineResult::Structural(value) => {
                    BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                        structural_type: value.structural_type,
                        multiplicity: value.multiplicity,
                        qualifications: value.qualifications.clone(),
                        projected_qualifications: value.projected_qualifications.clone(),
                    })
                }
            };
            catalog.contracts.push(TerminalCallableContract {
                id,
                identity: identities.intern(&contract.identity),
                scalar_parameters: header.scalar_parameters,
                parameter_order: header.parameter_order,
                structural_parameters: header.structural_parameters,
                result,
                proof_result: header.result,
                // The shared reader currently serves unrestricted data and
                // erased scalar formals. It refuses owned claims, proof
                // formals and returned-reference types before this point.
                entry_claim_paths: Vec::new(),
                result_claim_paths: Vec::new(),
                result_reference_regions: Vec::new(),
                proof,
                evidence_contract: Vec::new(),
                envelope: services.envelope(context, &contract.envelope)?,
            });
        }
        catalog.machine_envelopes = self
            .envelopes
            .iter()
            .map(|(machine, envelope)| {
                envelope
                    .as_ref()
                    .map(|envelope| {
                        services
                            .envelope(context, envelope)
                            .map(|envelope| TerminalMachineEnvelope { machine, envelope })
                    })
                    .transpose()
            })
            .collect::<Checked<Vec<_>>>()?;
        for (id, selection) in self.descriptors.iter() {
            if !matches!(self.envelopes.get(selection.machine), Some(Some(_))) {
                return Err(refuse(
                    "selected machine has no retained operational contract",
                ));
            }
            catalog.descriptors.push(TerminalMachineDescriptor {
                id,
                contract: selection.contract,
                machine: selection.machine,
                origin: TerminalMachineDescriptorOrigin {
                    application: identities.intern(&selection.application),
                    conformance: selection
                        .conformance
                        .as_ref()
                        .map(|text| identities.intern(text)),
                    requirement: selection
                        .requirement
                        .as_ref()
                        .map(|text| identities.intern(text)),
                },
                refinement_obligations: Vec::new(),
                refinement_evidence: Vec::new(),
            });
        }
        module.identities = identities.build();
        module.callables = catalog;
        for (id, _) in self.descriptors.iter() {
            let selected = module
                .callables
                .descriptor(id)
                .ok_or_else(|| refuse("selected machine descriptor disappeared"))?;
            let goals =
                terminal_semantics::calls::callable_refinement::callable_contract_refinement_goals(
                    module,
                    selected.contract,
                    selected.machine,
                )
                .map_err(refuse)?;
            let obligations = goals
                .iter()
                .map(|_| obligation())
                .collect::<Checked<Vec<_>>>()?;
            let row = module
                .callables
                .descriptors
                .get_mut((id.get() - 1) as usize)
                .ok_or_else(|| refuse("selected machine descriptor lost its coordinate"))?;
            row.refinement_obligations = obligations;
        }
        Ok(())
    }
}
