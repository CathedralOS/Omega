//! A proof header supplies a mathematical implication. Read its complete source
//! contract without registering a runtime callable or borrowing its authority.
use crate::read::refuse_at;

use super::{Program, span};
use crate::Unserved;
use crate::body::{Ceiling, CrashCeiling, Qualification, ValueType};
use crate::source_state::domain::ProofFact;
use crate::source_state::signature::{SignatureContractKind, StateSignature};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, MachineId, PlaceId, ScalarType, ValueId,
};
use std::fmt::Write;
use terminal_check_tables::Checked;
use terminal_psi::TheoremContract;
use terminal_semantics::theorems::data_subjects::{self, SourceSubject};

pub(crate) struct PreparedTheoremParameter {
    pub(crate) binder: symbols::SymbolHandle,
    pub(crate) source_position: u32,
    pub(crate) carrier_key: Vec<u8>,
    /// The contract's source inputs, one per authored argument of an
    /// application, in header order.
    pub(crate) inputs: Vec<TheoremInput>,
}

/// One source input of a logical contract: a scalar formal, which is one
/// subject, or a plain record formal, which is one subject for each of its
/// scalar fields, in declared order
/// (`terminal_semantics::theorems::data_subjects`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TheoremInput {
    Scalar(ScalarType),
    /// A sum, one subject of its inductive declaration.
    Sum(semantic_vocabulary::StructuralTypeId),
    Record {
        record: semantic_vocabulary::StructuralTypeId,
        leaves: Vec<(Vec<CanonicalStructuralPathSegment>, ScalarType)>,
    },
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
        let machine = self.required_machine(id, target, "named theorem")?;
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
        if argument.evidence_projection.is_some()
            && argument.application.is_none()
            && !argument.authored_machine_application.is_valid()
            && argument.const_literal.is_none()
        {
            return self.stored_theorem(id, argument);
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
        let machine = self.required_machine(id, target, "named theorem")?;
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

    /// `CONSTANT.field` or `local.field` cites the machine a record stores in
    /// a callable field ([callable-field evidence](wiki/spec/language/machine_values.md#invocation-storage-and-evidence)).
    /// A constant has one possible selection, the one its checked
    /// materialized value names, and an immutable local has the one its
    /// initializer names, so that selection's evidence covers every use; the
    /// citation executes no callback. Any other receiver, such as a formal or
    /// a mutable local, may hold another selection its type allows and is
    /// refused by name.
    fn stored_theorem(
        &mut self,
        id: MachineId,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Checked<terminal_psi::TheoremSource> {
        use crate::source_state::expression::ExpressionNode;
        let unavailable = |reason| refuse_at(id, Unserved::Call(reason));
        let typed = self.typed;
        let projection = argument
            .evidence_projection
            .as_ref()
            .ok_or_else(|| unavailable("stored theorem citation has no field projection"))?;
        // Follow immutable locals back to the record they were bound to.
        // An immutable local holds exactly its initializer, so its field has
        // the one selection that initializer names: a record literal, or a
        // constant whose checked materialized value names it.
        let mut receiver = projection.term_symbol;
        let literal = loop {
            if !typed.symbols.symbols().nodes().is_valid(receiver) {
                return Err(unavailable(
                    "stored theorem citation reads a field of a value with no binding",
                ));
            }
            let initializer = if let Some(constant) = typed.const_declaration_by_symbol(receiver) {
                let initializer = constant.materialized_initializer;
                (constant.evaluated_value.is_some()
                    && typed.expression_table.expression_is_valid(initializer))
                .then_some(initializer)
                .ok_or_else(|| {
                    unavailable(
                        "stored theorem citation reads a constant without its evaluated value",
                    )
                })?
            } else {
                typed
                    .immutable_local_initializer(receiver)
                    .filter(|initializer| typed.expression_table.expression_is_valid(*initializer))
                    .ok_or_else(|| {
                        unavailable(
                            "stored theorem citation reads a field of a value other than a constant or an immutable local; only those have one possible selection",
                        )
                    })?
            };
            match typed.expression_table.expression(initializer) {
                ExpressionNode::StructLiteral(literal) => break literal,
                ExpressionNode::Name(path) if path.members.len() == 1 => {
                    receiver = path.symbol;
                }
                _ => {
                    return Err(unavailable(
                        "stored theorem citation reads a field of a value whose initializer names no one record",
                    ));
                }
            }
        };
        if literal.case_name.is_some() {
            return Err(unavailable(
                "stored theorem citation reads a field of a constant that is not a record",
            ));
        }
        let mut fields = typed
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|field| field.name == projection.member);
        let (Some(field), None) = (fields.next(), fields.next()) else {
            return Err(unavailable(
                "stored theorem citation names no unique field of the constant",
            ));
        };
        let ExpressionNode::MachineValue(value) =
            (typed.expression_table.expression_is_valid(field.value))
                .then(|| typed.expression_table.expression(field.value))
                .ok_or_else(|| unavailable("stored theorem citation lost its stored value"))?
        else {
            return Err(unavailable(
                "stored theorem citation names a field that holds no machine selection",
            ));
        };
        // A machine value in a body carries the application its selection was
        // read at; one that supplies nothing is the plain selection.
        let supplies_arguments = value.application.is_valid()
            && (!typed
                .expression_table
                .static_machine_application_is_valid(value.application)
                || {
                    let application = typed
                        .expression_table
                        .static_machine_application(value.application);
                    !application.arguments.is_empty() || !application.bindings.is_empty()
                });
        if supplies_arguments
            || value.conformance.is_valid()
            || value.requirement.is_valid()
            || value.selected.application.is_some()
            || value.selected.const_literal.is_some()
            || value.selected.evidence_projection.is_some()
        {
            return Err(unavailable(
                "stored theorem citation selects a conformance member or static application",
            ));
        }
        let target = self
            .entered_machine(value.selected.symbol)
            .ok_or_else(|| unavailable("stored theorem citation selects no named theorem"))?;
        let machine = self.required_machine(id, target, "named theorem")?;
        if !typed.machine_type_parameters(machine).is_empty()
            || !machine.conformance_bounds.is_empty()
        {
            return Err(unavailable(
                "stored theorem citation selects a theorem with parameters of its own",
            ));
        }
        self.visible_to(id, target)?;
        // As for a named citation, this retains the selected body's own
        // checking obligation and produces no call or descriptor.
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
            let (contract, carrier_key, inputs) = self.theorem_contract(id, contract)?;
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
                inputs,
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
    ) -> Checked<(TheoremContract, Vec<u8>, Vec<TheoremInput>)> {
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
            let (nested, _, _) = self.theorem_contract(id, contract)?;
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
        // A clause names scalar formal `i` as value `i + 1` and the `k`th
        // record formal as place `k + 1` (`contracts::formal_place`).
        let mut inputs = Vec::with_capacity(signature.parameters.len());
        let mut sources = Vec::with_capacity(signature.parameters.len());
        for (position, parameter) in signature.parameters.iter().enumerate() {
            match parameter {
                ValueType::Scalar(scalar_type, Qualification::NONE) => {
                    sources.push(SourceSubject::Scalar {
                        id: ValueId::new(position as u64 + 1).ok_or_else(|| {
                            unavailable("logical theorem input identity overflow")
                        })?,
                        scalar_type: *scalar_type,
                    });
                    inputs.push(TheoremInput::Scalar(*scalar_type));
                }
                ValueType::Record(record, Qualification::NONE, _)
                    if matches!(
                        self.data_type(id, *record)?.shape,
                        crate::StorageShape::Sum(_)
                    ) =>
                {
                    // A sum is one subject, a value of its inductive
                    // declaration; a clause names it by its formal place.
                    let place = PlaceId::new(
                        inputs
                            .iter()
                            .filter(|input| {
                                matches!(input, TheoremInput::Record { .. } | TheoremInput::Sum(_))
                            })
                            .count() as u64
                            + 1,
                    )
                    .ok_or_else(|| unavailable("logical theorem input identity overflow"))?;
                    self.sum_shape(id, *record)?;
                    sources.push(SourceSubject::Sum {
                        place,
                        structural_type: *record,
                    });
                    inputs.push(TheoremInput::Sum(*record));
                }
                ValueType::Record(record, Qualification::NONE, _) => {
                    let data_subjects::RecordSubject { leaves, invariants } =
                        self.record_subject(id, *record)?;
                    let place = PlaceId::new(
                        inputs
                            .iter()
                            .filter(|input| {
                                matches!(input, TheoremInput::Record { .. } | TheoremInput::Sum(_))
                            })
                            .count() as u64
                            + 1,
                    )
                    .ok_or_else(|| unavailable("logical theorem input identity overflow"))?;
                    sources.push(SourceSubject::Record {
                        place,
                        leaves: leaves.clone(),
                        invariants,
                    });
                    inputs.push(TheoremInput::Record {
                        record: *record,
                        leaves,
                    });
                }
                _ => {
                    return Err(unavailable(
                        "logical theorem input needs its complete fixed scalar carrier, plain record or sum",
                    ));
                }
            }
        }
        let mut contract =
            data_subjects::flatten(&sources, &signature.requires, &signature.ensures)
                .map_err(unavailable)?;
        for formal in &contract.parameters {
            let scalar_type = formal.scalar_type;
            if !matches!(scalar_type, ScalarType::Boolean | ScalarType::IeeeFloat(_))
                && !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits().is_some())
            {
                return Err(unavailable(
                    "logical theorem input has no closed-width integer, float or Boolean denotation",
                ));
            }
        }
        contract.proof_parameters = proof_parameters;
        let declared = |sum| {
            self.sum_shape(id, sum)
                .map_err(|_| "logical contract's sum subject has no checked declaration")
        };
        let expected = terminal_semantics::theorems::contract_type(
            &contract,
            &declared,
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
        Ok((contract, carrier_key, inputs))
    }

    /// A sum's cases and their fields, as Terminal Psi will publish them
    /// (`data_subjects::sum_shape`): each a relevant plain scalar payload or
    /// the sum itself. Anything else is refused by name.
    fn sum_shape(
        &self,
        id: MachineId,
        sum: semantic_vocabulary::StructuralTypeId,
    ) -> Checked<proof_admission::DataSubjectShape> {
        let unavailable = |reason| refuse_at(id, Unserved::Declaration(reason));
        let declared = self.data_type(id, sum)?;
        let crate::StorageShape::Sum(cases) = &declared.shape else {
            return Err(unavailable("logical sum subject is not a sum"));
        };
        if !declared.requires.is_empty() || !declared.erased.is_empty() {
            return Err(unavailable(
                "logical sum subject has an invariant or an erased payload",
            ));
        }
        let mut shape = proof_admission::DataSubjectShape {
            structural_type: sum,
            cases: Vec::with_capacity(cases.len()),
        };
        for case in cases {
            let mut fields = Vec::with_capacity(case.fields.len());
            for field in &case.fields {
                let kind = match field.value_type {
                    ValueType::Scalar(scalar, Qualification::NONE) => {
                        proof_admission::DataFieldShape::Scalar(scalar)
                    }
                    ty if ty == ValueType::record(sum) => {
                        proof_admission::DataFieldShape::Recursive
                    }
                    _ => {
                        return Err(unavailable(
                            "logical sum subject has a payload that is not a scalar or itself",
                        ));
                    }
                };
                fields.push((field.id, kind));
            }
            shape.cases.push((case.id, fields));
        }
        Ok(shape)
    }

    /// A plain record's scalar fields, in declared order and with a nested
    /// plain record's fields in its place, and the data invariants of the
    /// record and each nested record, as Terminal Psi will publish them
    /// (`data_subjects::record_subject`).
    fn record_subject(
        &self,
        id: MachineId,
        record: semantic_vocabulary::StructuralTypeId,
    ) -> Checked<data_subjects::RecordSubject> {
        let unavailable = |reason| refuse_at(id, Unserved::Declaration(reason));
        let declared = self.data_type(id, record)?;
        let crate::StorageShape::Record(fields) = &declared.shape else {
            return Err(unavailable("logical data subject is not a plain record"));
        };
        if crate::body::self_place() != Some(data_subjects::invariant_self()) {
            return Err(unavailable(
                "logical data subject's invariant names another self place",
            ));
        }
        let mut subject = data_subjects::RecordSubject {
            leaves: Vec::new(),
            invariants: declared
                .requires
                .iter()
                .map(|clause| (Vec::new(), clause.clone()))
                .collect(),
        };
        for field in fields {
            let at = CanonicalStructuralPathSegment::Field(field.id);
            match field.value_type {
                ValueType::Scalar(scalar, Qualification::NONE) => {
                    subject.leaves.push((vec![at], scalar))
                }
                ValueType::Record(child, Qualification::NONE, _) => {
                    let nested = self.record_subject(id, child)?;
                    for (mut path, scalar) in nested.leaves {
                        path.insert(0, at);
                        subject.leaves.push((path, scalar));
                    }
                    for (mut path, clause) in nested.invariants {
                        path.insert(0, at);
                        subject.invariants.push((path, clause));
                    }
                }
                _ => {
                    return Err(unavailable(
                        "logical data subject has a field that is not a plain scalar",
                    ));
                }
            }
        }
        Ok(subject)
    }
}
