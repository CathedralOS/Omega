//! Resolve a stored machine once, keeping its contract separate from its body.
//! Construction only records code selection. The checker later proves every
//! obligation reconstructed from that selection and the published contract.

use semantic_vocabulary::{CallableContractId, MachineDescriptorId, MachineId, ScalarType};
use symbols::SymbolKind;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalCallableTermination;
use typed_trees::callable_values::{MachineValue, RuntimeCallableContractHandle};
use typed_trees::signature::SignatureContractKind;
use typed_trees::type_identity::IndexIdentityArgument;
use typed_trees::types::TypeReferenceNode;

use super::Program;
use crate::body::{Expression, ExpressionKind, Termination, ValueType};
use crate::callables::{CallableContract, MachineSelection};
use crate::{SourceOrigin, Unserved, refuse};

fn unavailable(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine,
        concerns: Construct::Machine,
        detail: detail.into(),
    }
}

impl<'t> Program<'t> {
    pub(super) fn callable_type(
        &mut self,
        machine: MachineId,
        source: RuntimeCallableContractHandle,
    ) -> Checked<ScalarType> {
        let typed = self.typed;
        if !typed.runtime_callable_contracts.is_valid(source) {
            return Err(unavailable(
                machine,
                "callable type has no current signature",
            ));
        }
        if let Some(contract) = self
            .callables
            .source_contract(source)
            .map_err(|reason| unavailable(machine, reason))?
        {
            return Ok(ScalarType::Callable(contract));
        }
        let identity = typed
            .runtime_callable_contract_identity(source, &[])
            .map_err(|reason| unavailable(machine, reason.to_string()))?;
        if self.callables.resolving.contains(&source) {
            return Err(unavailable(machine, "recursive runtime callable signature"));
        }
        let declared = &typed.runtime_callable_contract(source).signature;
        if !declared.type_parameters.is_empty() {
            return Err(unavailable(
                machine,
                "runtime callable has open static parameters",
            ));
        }
        if !crate::reach::has_authored_ceiling(typed, machine, declared.symbol)? {
            return Err(unavailable(
                machine,
                "runtime callable has an open service ceiling; write reaches",
            ));
        }
        let envelope = self.signature_envelope(machine, declared)?;
        let clauses = typed
            .signature_contracts
            .span(declared.contracts)
            .ok_or_else(|| unavailable(machine, "runtime callable has a foreign contract span"))?;
        if !declared.declares_no_crash_route
            && !clauses
                .iter()
                .any(|clause| matches!(clause.kind, SignatureContractKind::Crashes { .. }))
        {
            return Err(unavailable(
                machine,
                "runtime callable has an open crash ceiling",
            ));
        }
        self.callables.resolving.push(source);
        let signature = self.runtime_callable_signature(machine, declared);
        self.callables.resolving.pop();
        let (mut signature, _) = signature?;
        if signature
            .parameters
            .iter()
            .any(|ty| matches!(ty, ValueType::Rigid(_)))
            || matches!(signature.result, Some(ValueType::Rigid(_)))
        {
            return Err(unavailable(
                machine,
                "runtime callable signature retains a rigid type",
            ));
        }
        signature.termination = match &envelope.termination {
            TerminalCallableTermination::NotGuaranteed => Termination::Silent,
            TerminalCallableTermination::Terminates { .. } => Termination::Promised,
        };
        // Equal semantic contracts share a row, after this occurrence has
        // met formation too. A prior alias cannot admit an unsupported field
        // that the normalized type identity does not describe.
        let existing = self
            .callables
            .contracts
            .iter()
            .find_map(|(id, row)| (row.identity == identity).then_some((id, row)));
        let contract = if let Some((id, row)) = existing {
            if row.signature != signature || row.envelope != envelope {
                return Err(Refusal {
                    reason: RefusalReason::ForeignIdentity,
                    machine,
                    concerns: Construct::Machine,
                    detail: "equal callable identities lower to different contracts".to_owned(),
                });
            }
            id
        } else {
            self.callables.contracts.push(CallableContract {
                lifetimes: self.machine_lifetimes(declared.symbol),
                identity,
                signature,
                envelope,
            })
        };
        self.callables
            .bind_source(source, contract)
            .map_err(|reason| unavailable(machine, reason))?;
        Ok(ScalarType::Callable(contract))
    }

    /// Resolve the exact selected declaration without invoking it or deriving
    /// caller facts from its body. Merely retaining code is not a call edge.
    pub(super) fn machine_value(
        &mut self,
        machine: MachineId,
        value: &MachineValue,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let typed = self.typed;
        if !typed
            .type_reference_table
            .contains_type_reference(value.type_reference)
        {
            return Err(unavailable(
                machine,
                "machine value has no current callable type",
            ));
        }
        let value_type = self.value_type(machine, value.type_reference)?;
        let ValueType::Scalar(ScalarType::Callable(contract), _) = value_type else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("machine value without a callable type"),
            ));
        };
        let selected = value.selected.symbol;
        if !typed.symbols.symbols().nodes().is_valid(selected) {
            return Err(unavailable(
                machine,
                "machine value has no exact selected declaration",
            ));
        }
        if value.selected.const_literal.is_some() || value.selected.evidence_projection.is_some() {
            return Err(unavailable(
                machine,
                "machine value is not a closed named selection",
            ));
        }
        if value.selected.application.is_some() && !value.application.is_valid() {
            return Err(unavailable(
                machine,
                "machine value lost its closed application",
            ));
        }
        let owner = match typed.symbols.get(selected).kind {
            SymbolKind::Machine => selected,
            SymbolKind::State => typed.symbols.get(selected).parent,
            _ => {
                return Err(unavailable(
                    machine,
                    "machine value selects no concrete machine",
                ));
            }
        };
        let (_, entry) = self.declaration(machine, owner)?;
        if selected != owner && selected != entry.symbol {
            return Err(unavailable(
                machine,
                "machine value selects another entry state",
            ));
        }
        self.applies_nothing(machine, owner, value.application, &[])?;
        let target = self.machine(owner)?;
        // A stored selection opens the helper's possible-use boundary: a
        // particular caller cannot narrow the callable's public inputs or
        // supply private body facts to an unknown call of the value.
        if !self.fixed_inputs.contains(&target) {
            self.fixed_inputs.push(target);
        }
        let application = typed
            .runtime_machine_value_identity(value, &[])
            .map_err(|reason| unavailable(machine, reason.to_string()))?;
        let TypeReferenceNode::Callable {
            contract: contract_source,
        } = typed
            .type_reference_table
            .type_reference(value.type_reference)
        else {
            return Err(unavailable(
                machine,
                "machine value lost its exact signature source",
            ));
        };
        let identity = |symbol| {
            typed
                .runtime_callable_index_identity(
                    *contract_source,
                    IndexIdentityArgument::Subject(symbol),
                    &[],
                )
                .map_err(|reason| unavailable(machine, reason.to_string()))
        };
        let (conformance, requirement) =
            match (value.conformance.is_valid(), value.requirement.is_valid()) {
                (true, true) => (
                    Some(identity(value.conformance)?),
                    Some(identity(value.requirement)?),
                ),
                (false, false) => (None, None),
                _ => {
                    return Err(unavailable(
                        machine,
                        "machine value lost its conformance requirement",
                    ));
                }
            };
        let descriptor = self.selection(contract, target, application, conformance, requirement);
        Ok(Expression {
            kind: ExpressionKind::MachineValue {
                contract,
                descriptor,
            },
            result: Some(value_type),
            source,
        })
    }

    fn selection(
        &mut self,
        contract: CallableContractId,
        machine: MachineId,
        application: String,
        conformance: Option<String>,
        requirement: Option<String>,
    ) -> MachineDescriptorId {
        if let Some((descriptor, _)) = self.callables.descriptors.iter().find(|(_, row)| {
            row.contract == contract
                && row.machine == machine
                && row.application == application
                && row.conformance == conformance
                && row.requirement == requirement
        }) {
            return descriptor;
        }
        self.callables.descriptors.push(MachineSelection {
            contract,
            machine,
            application,
            conformance,
            requirement,
        })
    }
}
