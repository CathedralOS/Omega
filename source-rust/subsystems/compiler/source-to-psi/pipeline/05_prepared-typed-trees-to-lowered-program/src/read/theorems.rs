//! A proof header supplies a mathematical implication. Read its complete source
//! contract without registering a runtime callable or borrowing its authority.
use crate::read::refuse_at;

use super::{Program, span};
use crate::Unserved;
use crate::body::{Ceiling, CrashCeiling, Qualification, ValueType};
use crate::source_state::domain::ProofFact;
use crate::source_state::signature::{SignatureContractKind, StateSignature};
use semantic_vocabulary::{MachineId, ScalarType, ValueId};
use std::fmt::Write;
use terminal_check_tables::Checked;
use terminal_psi::{TheoremContract, ValueDeclaration};

pub(crate) struct PreparedTheoremParameter {
    pub(crate) binder: symbols::SymbolHandle,
    pub(crate) source_position: u32,
    pub(crate) carrier_key: Vec<u8>,
}

impl Program<'_> {
    /// One proof argument. Stage 04 closes an instantiated citation into the
    /// machine generated for its arguments; one it leaves open, such as
    /// `lifted::<inner>`, gives the cited theorem its own proof arguments.
    /// Those are read as proof arguments of this same caller, one per proof
    /// parameter of the cited theorem, and stage 06 checks each against that
    /// theorem's own alias.
    pub(super) fn theorem_argument(
        &mut self,
        id: MachineId,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Checked<crate::body::ProofSupply> {
        if !(argument.authored_machine_application.is_valid() && argument.application.is_some()) {
            return self
                .theorem_source(id, argument)
                .map(crate::body::ProofSupply::plain);
        }
        let unavailable = |reason| refuse_at(id, Unserved::Call(reason));
        let typed = self.typed;
        if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
            return Err(unavailable(
                "proof supply needs its complete explicit theorem citation",
            ));
        }
        let selection = argument
            .authored_selection
            .and_then(|occurrence| typed.authored_declaration_selections().get(occurrence))
            .ok_or_else(|| unavailable("proof citation has no exact authored selection"))?;
        if selection.kind()
            != crate::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
        {
            return Err(unavailable("proof citation has another authored role"));
        }
        let target = self.entered_machine(argument.symbol).ok_or_else(|| {
            unavailable("logical argument is not a named theorem or proof formal")
        })?;
        let machine = typed
            .machine_by_symbol(target)
            .ok_or_else(|| unavailable("named theorem has no complete source declaration"))?;
        let handle = argument.authored_machine_application;
        if !typed
            .expression_table
            .static_machine_application_is_valid(handle)
            || typed
                .expression_table
                .static_machine_application(handle)
                .binding_declaration
                != target
        {
            return Err(unavailable(
                "instantiated theorem citation lost its exact retained application",
            ));
        }
        let parameters = typed.machine_type_parameters(machine);
        if parameters.is_empty()
            || !machine.conformance_bounds.is_empty()
            || parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    crate::source_state::data::TypeParameterKind::Proof { .. }
                )
            })
        {
            return Err(unavailable(
                "instantiated theorem citation needs its complete logical correspondence",
            ));
        }
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(handle);
        let mut instantiation = Vec::with_capacity(parameters.len());
        for parameter in parameters.iter() {
            let mut matching = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let (Some(binding), None) = (matching.next(), matching.next()) else {
                return Err(unavailable(
                    "cited theorem's proof parameter has no unique explicit actual",
                ));
            };
            let crate::source_state::static_applications::StaticMachineBindingValue::Proof(inner) =
                &binding.value
            else {
                return Err(unavailable(
                    "cited theorem's proof parameter was supplied with an executable or type binding",
                ));
            };
            // One level: a supply nested inside an instantiation is read with
            // `theorem_source`, which refuses its own instantiation by name.
            instantiation.push(self.theorem_source(id, inner)?);
        }
        self.visible_to(id, target)?;
        Ok(crate::body::ProofSupply {
            source: terminal_psi::TheoremSource::Named(self.machine(target)?),
            instantiation,
        })
    }

    pub(super) fn theorem_source(
        &mut self,
        id: MachineId,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Checked<terminal_psi::TheoremSource> {
        let unavailable = |reason| refuse_at(id, Unserved::Call(reason));
        let typed = self.typed;
        if argument.authored_machine_application.is_valid() && argument.application.is_some() {
            return Err(unavailable(
                "a theorem given its own proof arguments cannot itself be given to another inside a citation",
            ));
        }
        if !typed.symbols.symbols().nodes().is_valid(argument.symbol)
            || argument.application.is_some()
            || argument.authored_machine_application.is_valid()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(unavailable(
                "proof supply needs its complete explicit theorem citation",
            ));
        }
        if argument.type_reference.is_valid() {
            let retained = typed
                .static_operand_from_type_reference(argument.type_reference)
                .map_err(unavailable)?;
            if retained.symbol != argument.symbol
                || retained.application.is_some()
                || retained.authored_machine_application.is_valid()
                || retained.const_literal.is_some()
                || retained.evidence_projection.is_some()
            {
                return Err(unavailable(
                    "proof citation differs from its retained source operand",
                ));
            }
        }
        // A proof formal is a binder of the caller's own header, not a
        // declaration selected by name, so resolution records no selection
        // for it. Its binder identifies it exactly; a recorded selection must
        // still name that binder.
        if typed.proof_parameter_contract(argument.symbol).is_some() {
            if let Some(occurrence) = argument.authored_selection {
                let selection = typed
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or_else(|| unavailable("proof citation has no exact authored selection"))?;
                if selection.kind()
                    != crate::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
                    || selection
                        .target()
                        .as_resolved()
                        .is_none_or(|selected| selected.selected_symbol() != argument.symbol)
                {
                    return Err(unavailable(
                        "forwarded theorem selection names another parameter",
                    ));
                }
            }
            let mut formals = self
                .theorem_parameters
                .iter()
                .zip(&self.theorem_parameter_sources)
                .filter(|(formal, source)| {
                    formal.machine == id && source.binder == argument.symbol
                });
            let position = formals
                .next()
                .map(|(formal, _)| formal.proof_position)
                .ok_or_else(|| {
                    unavailable("forwarded theorem is outside the exact caller header")
                })?;
            if formals.next().is_some() {
                return Err(unavailable(
                    "forwarded theorem has ambiguous caller identity",
                ));
            }
            return Ok(terminal_psi::TheoremSource::Formal(position));
        }
        let selection = argument
            .authored_selection
            .and_then(|occurrence| typed.authored_declaration_selections().get(occurrence))
            .ok_or_else(|| unavailable("proof citation has no exact authored selection"))?;
        if selection.kind()
            != crate::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
        {
            return Err(unavailable("proof citation has another authored role"));
        }
        self.static_argument_selections(id, argument)?;
        let target = self.entered_machine(argument.symbol).ok_or_else(|| {
            unavailable("logical argument is not a named theorem or proof formal")
        })?;
        let machine = typed
            .machine_by_symbol(target)
            .ok_or_else(|| unavailable("named theorem has no complete source declaration"))?;
        // A theorem with proof parameters of its own may be cited without
        // them when the expected contract declares the same ones in its own
        // header; stage 06 checks that correspondence and refuses any other.
        if typed
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                !matches!(
                    parameter.kind,
                    crate::source_state::data::TypeParameterKind::Proof { .. }
                )
            })
            || !machine.conformance_bounds.is_empty()
        {
            return Err(unavailable(
                "instantiated theorem citation needs its complete logical correspondence",
            ));
        }
        self.visible_to(id, target)?;
        // This retains the cited body's independent checking obligation. No
        // call, descriptor or code-selection identity is produced for it.
        self.machine(target).map(terminal_psi::TheoremSource::Named)
    }

    pub(crate) fn publish_theorem_parameters(
        &self,
        module: &mut terminal_psi::TerminalModule,
    ) -> Checked<()> {
        if self.theorem_parameters.is_empty() && self.theorem_parameter_sources.is_empty() {
            return Ok(());
        }
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let published = (|| {
            if self.theorem_parameters.len() != self.theorem_parameter_sources.len() {
                return Err(refuse_at(
                    MachineId::new(1).expect("nonzero diagnostic"),
                    Unserved::Declaration(
                        "logical theorem parameters lost their source correspondence",
                    ),
                ));
            }
            for (parameter, source) in self
                .theorem_parameters
                .iter()
                .zip(&self.theorem_parameter_sources)
            {
                let unavailable =
                    |reason| refuse_at(parameter.machine, Unserved::Declaration(reason));
                // Only the complete implication identifies a Strict proof type.
                // The cited declaration and its derivation remain separate.
                let mut name = String::from("mathematical-proof:");
                for byte in &source.carrier_key {
                    write!(&mut name, "{byte:02x}").expect("writing a String cannot fail");
                }
                let identity = identities.intern(&name);
                if let Some(carrier) = module
                    .types
                    .mathematical_carriers
                    .iter()
                    .find(|row| row.type_identity == identity)
                {
                    if carrier.carrier_key != source.carrier_key {
                        return Err(unavailable(
                            "logical theorem type has another retained mathematical carrier",
                        ));
                    }
                } else {
                    module.types.mathematical_carriers.push(
                        terminal_psi::TerminalMathematicalCarrier {
                            type_identity: identity,
                            carrier_key: source.carrier_key.clone(),
                        },
                    );
                }
                let slot = usize::try_from(parameter.machine.get() - 1)
                    .map_err(|_| unavailable("logical theorem owner is outside the module"))?;
                let machine = module
                    .machines
                    .get_mut(slot)
                    .filter(|machine| machine.id == parameter.machine)
                    .ok_or_else(|| unavailable("logical theorem owner is outside the module"))?;
                if machine.contract.erased_proof_formals.len() != parameter.proof_position as usize
                {
                    return Err(unavailable(
                        "logical theorem formal differs from its complete ordered header",
                    ));
                }
                machine
                    .contract
                    .erased_proof_formals
                    .push(terminal_psi::ErasedProofFormal {
                        source: terminal_psi::ProofFormalSource::GenericParameter(
                            source.source_position,
                        ),
                        type_identity: identity,
                    });
                module
                    .proofs
                    .theorem_parameters
                    .push(terminal_psi::TheoremParameter {
                        machine: parameter.machine,
                        proof_position: parameter.proof_position,
                        contract: parameter.contract.clone(),
                    });
            }
            module
                .types
                .mathematical_carriers
                .sort_unstable_by_key(|row| row.type_identity);
            module
                .proofs
                .theorem_parameters
                .sort_unstable_by_key(|row| (row.machine, row.proof_position));
            Ok(())
        })();
        module.identities = identities.build();
        published
    }

    pub(super) fn read_theorem_parameters(
        &mut self,
        id: MachineId,
        machine: &crate::source_state::machine::Machine,
    ) -> Checked<()> {
        let typed = self.typed;
        let mut prepared = Vec::new();
        let mut parameters = Vec::new();
        for (position, parameter) in typed.machine_type_parameters(machine).iter().enumerate() {
            let crate::source_state::data::TypeParameterKind::Proof { contract } = &parameter.kind
            else {
                continue;
            };
            let (contract, carrier_key) = self.theorem_contract(id, contract)?;
            let unavailable = || {
                refuse_at(
                    id,
                    Unserved::Declaration("theorem header position overflow"),
                )
            };
            parameters.push(terminal_psi::TheoremParameter {
                machine: id,
                proof_position: u32::try_from(parameters.len()).map_err(|_| unavailable())?,
                contract,
            });
            prepared.push(PreparedTheoremParameter {
                binder: parameter.symbol,
                source_position: u32::try_from(position).map_err(|_| unavailable())?,
                carrier_key,
            });
        }
        self.theorem_parameters.extend(parameters);
        self.theorem_parameter_sources.extend(prepared);
        Ok(())
    }

    /// The initial scalar denotation admits no live storage relationship,
    /// membership authority, executable result or unread operational promise.
    /// Other theorem shapes remain unavailable until their full correspondence
    /// can be retained; a resultless signature alone is not logical evidence.
    pub(super) fn theorem_contract(
        &mut self,
        id: MachineId,
        source: &StateSignature,
    ) -> Checked<(TheoremContract, Vec<u8>)> {
        let unavailable = |reason| refuse_at(id, Unserved::Declaration(reason));
        let typed = self.typed;
        // The contract's own proof parameters belong to its header:
        // mathematical_bindings.md, "If the contract itself takes a proof
        // parameter, that parameter belongs to the contract's own header."
        // Each states a complete contract with none of its own.
        let own = typed
            .data_type_parameters
            .span_or_empty(source.type_parameters);
        if own.is_empty() != source.type_parameters.is_empty() {
            return Err(unavailable(
                "logical theorem contract has a stale header span",
            ));
        }
        let mut proof_parameters = Vec::with_capacity(own.len());
        for parameter in own {
            let crate::source_state::data::TypeParameterKind::Proof { contract } = &parameter.kind
            else {
                return Err(unavailable(
                    "logical theorem contract declares a static parameter that is not a proof",
                ));
            };
            let (nested, _) = self.theorem_contract(id, contract)?;
            if !nested.proof_parameters.is_empty() {
                return Err(unavailable(
                    "a logical contract's proof parameter takes proof parameters of its own",
                ));
            }
            proof_parameters.push(nested);
        }
        // Its scalar telescope and clauses are read without those binders,
        // which no clause can name.
        let unbound;
        let source = if source.type_parameters.is_empty() {
            source
        } else {
            unbound = StateSignature {
                type_parameters: arena::HandleSpan::empty(),
                ..source.clone()
            };
            &unbound
        };
        if !source.lifetime_parameters.is_empty()
            || !source.conformance_bounds.is_empty()
            || source.is_default
            || !source.native_callback_parameters.is_empty()
            || source.return_type.is_valid()
            || source.suspends
            || source.blocks
            || source.service_reach_is_installation_bound
            || !matches!(
                source.termination_guarantee,
                source_declarations::TerminationGuarantee::NoGuarantee
            )
        {
            return Err(unavailable(
                "logical theorem contract needs its complete non-scalar correspondence",
            ));
        }
        let parameters = typed.state_signature_parameters(source);
        if parameters.iter().any(|parameter| {
            parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased()
        }) {
            return Err(unavailable(
                "logical theorem input is not a fixed mathematical subject",
            ));
        }
        for contract in span(
            |span| typed.signature_contracts.span(span),
            source.contracts,
            id,
            "theorem contract span",
        )? {
            if contract.binding.is_some()
                || !matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                )
                || span(
                    |span| typed.proof_facts.span(span),
                    contract.facts,
                    id,
                    "theorem premise span",
                )?
                .iter()
                .any(|fact| !matches!(fact, ProofFact::Expression(_)))
            {
                return Err(unavailable(
                    "logical theorem clause carries a result, membership or operational obligation",
                ));
            }
        }
        let (signature, _) = self.declared_requirement_signature(id, source, &[])?;
        if signature.result.is_some()
            || !signature.borrowed.is_empty()
            || !signature.erased.is_empty()
            || signature.has_structural_guarantees()
            || signature.ranking.is_some()
            || matches!(&signature.reaches, Ceiling::Authored(entries) if !entries.is_empty())
            || matches!(&signature.invokes, Ceiling::Authored(entries) if !entries.is_empty())
            || matches!(&signature.crashes, CrashCeiling::Authored(routes) if !routes.is_empty())
        {
            return Err(unavailable(
                "logical evidence cannot supply executable outcomes or authority",
            ));
        }
        let mut formals = Vec::with_capacity(signature.parameters.len());
        for (position, parameter) in signature.parameters.iter().enumerate() {
            let ValueType::Scalar(scalar_type, Qualification::NONE) = parameter else {
                return Err(unavailable(
                    "logical theorem input needs its complete fixed scalar carrier",
                ));
            };
            if !matches!(scalar_type, ScalarType::Boolean)
                && !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits().is_some())
            {
                return Err(unavailable(
                    "logical theorem input has no closed-width integer or Boolean denotation",
                ));
            }
            formals.push(ValueDeclaration {
                id: ValueId::new(position as u64 + 1)
                    .ok_or_else(|| unavailable("logical theorem input identity overflow"))?,
                scalar_type: *scalar_type,
                qualifications: Default::default(),
            });
        }
        let contract = TheoremContract {
            parameters: formals,
            requires: signature.requires,
            ensures: signature.ensures,
            proof_parameters,
        };
        let expected = terminal_semantics::theorems::contract_type(
            &contract,
            &mut proof_admission::Budget::default(),
        )
        .map_err(unavailable)?;
        let carrier_key = terminal_codec::theorem_implication_key(
            &expected.arena,
            &proof_admission::MathematicalCertificate {
                signature: expected.signature,
                level_arity: 0,
                context: Vec::new(),
                term: expected.ty,
                expected: expected.ty,
            },
        )
        .map_err(|_| unavailable("logical theorem type has no canonical portable identity"))?;
        Ok((contract, carrier_key))
    }
}
