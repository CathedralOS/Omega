/*
The machine family of the reader: which declarations are admitted (the subset
gate), their signatures and formals, and
the driver that reads one body statement by statement.
*/

use crate::body::Qualification;
use language_semantics::{TerminationGuarantee, TerminationInterface};
use semantic_vocabulary::{MachineId, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashPredicateTerm, CrashRouteGuard};
use typed_trees::machine::{Machine, MachineBodyStatus, MachinePurpose};
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

use super::contracts::{ClauseScope, MathematicalTerms, ResultTerm};
use super::{Program, Slot, crash_cause, span};
use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target, Termination,
    ValueType,
};
use crate::canonical::{self, Repeated};
use crate::{Binder, MachineNames, SourceOrigin, Unserved, refuse};
use source::SourceSpan;

/// A refusal of what `declaration` writes between `<` and `>`, by the rule
/// it broke and the declaration's name.
fn refuse_parameters(
    id: MachineId,
    what: &'static str,
    declaration: &str,
) -> terminal_check_tables::Refusal {
    let mut refusal = refuse(id, Construct::Machine, Unserved::Machine(what));
    refusal.detail = format!("{}: `{declaration}`", refusal.detail);
    refusal
}

impl<'t> Program<'t> {
    /// A helper with no written input requirement can use what every caller
    /// guarantees. A written `requires true` must still support every input.
    /// Its declaration marker survives when normalization removes the literal
    /// fact; an empty clause list alone cannot distinguish the two contracts.
    /// Every subject declaration is read; a callable used as a value or an
    /// unserved selection refuses the whole route. Only direct calls remain
    /// for those helpers. A reached dependency need not include every caller,
    /// so its helpers and static requirement implementations keep their envelope.
    pub(crate) fn private_input_contracts(
        &self,
        entry: Option<MachineId>,
        diagnostic: MachineId,
    ) -> Checked<Vec<MachineId>> {
        let mut private = Vec::new();
        let mut fixed = self.symbols.map(|_, _| false);
        for id in entry
            .iter()
            .chain(&self.fixed_inputs)
            .chain(&self.requirements)
            .chain(
                self.selections
                    .iter()
                    .map(|selection| &selection.implementation),
            )
        {
            *fixed.get_mut(*id).ok_or_else(|| {
                refuse(
                    *id,
                    Construct::Machine,
                    Unserved::Machine("input boundary without a machine"),
                )
            })? = true;
        }
        for &id in &self.subject_machines {
            if fixed.get(id) == Some(&true) {
                continue;
            }
            let symbol = terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
            let Some(machine) = self.typed.machine_by_symbol(*symbol) else {
                continue;
            };
            let contracts = span(
                &self.typed.signature_contracts,
                machine.contracts,
                id,
                "contract span",
            )?;
            if !machine.is_public
                && self.typed.owned_conformance_realization(*symbol).is_none()
                && machine.satisfies.is_empty()
                && !contracts.iter().any(|clause| {
                    clause.kind == SignatureContractKind::Requires
                        && clause.keyword_source_span.is_some()
                })
            {
                private.push(id);
            }
        }
        // Build/preparation code and declaration sidecars can refer to a
        // subject helper without becoming one of this module's callers.
        // Any such use keeps the input boundary open. A reached subset is
        // never mistaken for the helper's complete possible-use set.
        if private.is_empty() {
            return Ok(private);
        }
        let mut targets = Vec::with_capacity(private.len() * 2);
        for id in &private {
            let symbol = *terminal_check_tables::row(&self.symbols, *id, *id, Construct::Machine)?;
            targets.push(symbol);
            let (_, state) = self.declaration(*id, symbol)?;
            targets.push(state.symbol);
        }
        let uses =
            InputSymbolUses::new(self.symbols.values().copied(), targets).ok_or_else(|| {
                refuse(
                    diagnostic,
                    Construct::Machine,
                    Unserved::Machine("private input use with an invalid symbol"),
                )
            })?;
        let mut outside = OutsideInputUses { uses };
        typed_trees::references::visit_program(self.typed, &mut outside);
        private.retain(|id| {
            let symbol = *self.symbols.get(*id).expect("candidate was read above");
            let state = self
                .declaration(*id, symbol)
                .expect("candidate declaration was checked")
                .1;
            [symbol, state.symbol]
                .iter()
                .all(|symbol| outside.uses.get(*symbol).is_some_and(|row| !row.outside))
        });
        Ok(private)
    }

    /// The machines whose callers in this compilation may rely on what the
    /// body returns beyond its written `ensures`. state_contracts.md, "Local
    /// call summaries": "An exact checked call inside the same fingerprinted
    /// verification unit may use the callee's written contract plus checked
    /// body-derived result and state facts", and across that boundary
    /// "ordinary calls use the published contract".
    ///
    /// The unit here is the subject package. A private subject machine can
    /// only be called from it (`visible_to`), so a fact derived from its body
    /// reaches no other unit. A public machine is left out: the route states
    /// a summary as a clause of the machine's contract, and a public
    /// contract is what independent clients are given. A reached dependency
    /// is another unit. An entry, a Build selection, a requirement and a
    /// selected implementation are boundaries ("An abstract requirement,
    /// unknown callable or replaceable boundary uses its interface
    /// contract"), and a machine stage 04 generated keeps exactly the
    /// contract its generic form was checked against.
    pub(crate) fn local_result_bodies(&self, entry: Option<MachineId>) -> Checked<Vec<MachineId>> {
        let mut boundary = self.symbols.map(|_, _| false);
        for id in entry
            .iter()
            .chain(&self.fixed_inputs)
            .chain(&self.requirements)
            .chain(
                self.selections
                    .iter()
                    .map(|selection| &selection.implementation),
            )
        {
            *boundary.get_mut(*id).ok_or_else(|| {
                refuse(
                    *id,
                    Construct::Machine,
                    Unserved::Machine("result boundary without a machine"),
                )
            })? = true;
        }
        let mut local = Vec::new();
        for &id in &self.subject_machines {
            if boundary.get(id) == Some(&true) {
                continue;
            }
            let symbol = *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
            let Some(machine) = self.typed.machine_by_symbol(symbol) else {
                continue;
            };
            if !machine.is_public
                && machine.satisfies.is_empty()
                && self.typed.owned_conformance_realization(symbol).is_none()
                && crate::template_of(self.typed, symbol).is_none()
            {
                local.push(id);
            }
        }
        Ok(local)
    }

    fn next_identity(&self) -> Checked<MachineId> {
        MachineId::new(self.symbols.len() as u64 + 1).ok_or_else(|| {
            refuse(
                MachineId::new(1).expect("1 is nonzero"),
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })
    }

    /// The `MachineId` of a callee, reading its signature the first time.
    pub fn machine(&mut self, symbol: SymbolHandle) -> Checked<MachineId> {
        let diagnostic = self.next_identity()?;
        let (declared, _) = self.declaration(diagnostic, symbol)?;
        if let Some((owner, _)) = self.conformer_requirement(diagnostic, declared)?
            && self
                .typed
                .trait_definition_by_symbol(owner)
                .is_some_and(|owner| owner.is_boundary)
        {
            return Err(refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine(
                    "a boundary conformer is callable only through its provider slot",
                ),
            ));
        }
        // ponytail: linear scan over the discovered machines; a dense table
        // indexed by machine position if closures grow past a few hundred.
        for position in 1..=self.symbols.len() as u64 {
            let id = MachineId::new(position).expect("positions start at 1");
            if self.implementation_body != Some(id) && self.symbols.get(id) == Some(&symbol) {
                return Ok(id);
            }
        }
        let id = self.next_identity()?;
        let reading = match (&self.alternative, self.symbols.is_empty()) {
            (Some(arguments), true) => Reading::Alternative(arguments.clone()),
            _ if self.rigid && self.symbols.is_empty() => Reading::GenericEntry,
            _ => Reading::Closed,
        };
        self.read_machine(id, symbol, reading)?;
        Ok(id)
    }

    /// The member's body contract is an independent obligation of the
    /// refinement witness. Its identity is excluded from named-call lookup,
    /// so a recursive call cannot discover an unadvertised direct entry.
    pub fn implementation(
        &mut self,
        symbol: SymbolHandle,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Checked<MachineId> {
        self.requirement_body(
            symbol,
            declaring_trait,
            requirement,
            Reading::Implementation,
        )
    }

    /// A supplied realization has the same advertised contract as its
    /// ordinary routed entry. The exact slot additionally admits a boundary
    /// implementation as a root, without making its name callable.
    pub fn provider(
        &mut self,
        symbol: SymbolHandle,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Checked<MachineId> {
        self.requirement_body(symbol, declaring_trait, requirement, Reading::Provider)
    }

    fn requirement_body(
        &mut self,
        symbol: SymbolHandle,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
        reading: Reading,
    ) -> Checked<MachineId> {
        let id = self.next_identity()?;
        let (machine, _) = self.declaration(id, symbol)?;
        let bound = self
            .conformer_requirement(id, machine)?
            .map(|(owner, required)| (owner, required.symbol));
        if matches!(reading, Reading::Provider) && bound.is_none() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Machine("provider body needs its exact supplied requirement"),
            ));
        }
        if let Some(bound) = bound
            && bound != (declaring_trait, requirement)
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Machine("conformance witness cannot rebind a declared requirement"),
            ));
        }
        if bound.is_some() {
            if matches!(reading, Reading::Provider)
                && let Some((known, _)) = self.symbols.iter().find(|(known, source)| {
                    Some(*known) != self.implementation_body && **source == symbol
                })
            {
                return Ok(known);
            }
            let own_witness = matches!(reading, Reading::Implementation);
            self.read_machine(id, symbol, reading)?;
            if own_witness {
                self.implementation_body = Some(id);
            }
            Ok(id)
        } else {
            self.machine(symbol)
        }
    }

    /// Reads the signature of `symbol` as machine `id` and records it. The
    /// scope of whichever machine was being read is put back afterwards: a
    /// callee is discovered in the middle of its caller's body.
    fn read_machine(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
        reading: Reading,
    ) -> Checked<()> {
        let outer = std::mem::take(&mut self.scope);
        // An elided lifetime in the signature is a fresh parameter of this
        // machine, which each call instantiates (`regions::Elision`).
        let elision = std::mem::replace(&mut self.elision, super::regions::Elision::Inputs(symbol));
        let owner = std::mem::replace(&mut self.lifetime_owner, symbol);
        let read = self.signature(id, symbol, &reading);
        self.lifetime_owner = owner;
        self.elision = elision;
        let scope = std::mem::replace(&mut self.scope, outer);
        let (signature, names, value_parameters) = read?;
        self.symbols.push(symbol);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(value_parameters);
        self.value_arguments.push(Vec::new());
        self.scopes.push(scope);
        Ok(())
    }

    /// The `MachineId` that stands for the generic machine `symbol` applied
    /// at `arguments`, the types a call in a generic check form's entry
    /// gives its type parameters, in declaration order: its signature and
    /// contract at those types, and no body. The machine's own body is
    /// checked once as the entry of its own generic check form, so here it
    /// is only what its callers are held to, like a requirement.
    pub(super) fn application(
        &mut self,
        symbol: SymbolHandle,
        arguments: Vec<ValueType>,
    ) -> Checked<MachineId> {
        if let Some((id, _)) = self
            .applied
            .iter()
            .find(|(id, applied)| self.symbols.get(*id) == Some(&symbol) && *applied == arguments)
        {
            return Ok(*id);
        }
        let id = self.next_identity()?;
        self.read_machine(id, symbol, Reading::Applied(arguments.clone()))?;
        self.applied.push((id, arguments));
        self.requirements.push(id);
        self.applications.push(id);
        Ok(id)
    }

    /// The requirement `requirement` as `declaring_trait` declares it.
    pub(super) fn declared_requirement(
        &self,
        id: MachineId,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Checked<&'t StateSignature> {
        let typed = self.typed;
        typed
            .trait_definition_by_symbol(declaring_trait)
            .and_then(|declaration| {
                typed
                    .trait_machine_signatures(declaration)
                    .iter()
                    .find(|signature| signature.symbol == requirement)
            })
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Machine("requirement that its trait does not declare"),
                )
            })
    }

    /// The first machine of a conformance witness: the requirement's
    /// signature and contract at `arguments`, the literals of its value
    /// parameters, on a body the caller supplies.
    pub fn witnessed(
        &mut self,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<MachineId> {
        let id = self.next_identity()?;
        let declared = self.declared_requirement(id, declaring_trait, requirement)?;
        self.witnessed_signature(declared, arguments)
    }

    pub fn witnessed_signature(
        &mut self,
        declared: &'t StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<MachineId> {
        let id = self.next_identity()?;
        let (signature, names) = self.requirement_signature(id, declared, arguments)?;
        self.symbols.push(declared.symbol);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(0);
        self.value_arguments.push(arguments.to_vec());
        self.scopes.push(Vec::new());
        Ok(id)
    }

    /// The `MachineId` that stands for a trait requirement in a generic
    /// check form: its signature and contract, read as any machine's, and
    /// no body. A requirement with value parameters is one machine per
    /// tuple of literals a call binds them to (`arguments`), because its
    /// contract differs with them.
    pub(super) fn requirement(
        &mut self,
        declared: &'t StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<MachineId> {
        let symbol = declared.symbol;
        for position in 1..=self.symbols.len() as u64 {
            let id = MachineId::new(position).expect("positions start at 1");
            if self.symbols.get(id) == Some(&symbol)
                && self.value_arguments.get(id).map(Vec::as_slice) == Some(arguments)
            {
                return Ok(id);
            }
        }
        let id = self.next_identity()?;
        let (signature, names) = self.requirement_signature(id, declared, arguments)?;
        self.symbols.push(symbol);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(0);
        self.value_arguments.push(arguments.to_vec());
        self.scopes.push(Vec::new());
        self.requirements.push(id);
        Ok(id)
    }

    /// Refuses a demanded declaration that holds a request no pass served
    /// (`typed_trees::deferred_refusals`). Typing and typed preparation see
    /// the whole program with every library and record such a request on its
    /// declaration instead of refusing programs that never reach it; this is
    /// where it is raised. Every machine read passes here, as an entry, a
    /// Build selection, a subject declaration or a callee, so a declaration
    /// with a row is never read through.
    /// Whether every contract on `state` only restates a domain one of its
    /// formals' declared types carries: `requires distance in Km` for
    /// `distance: i32 in Km`, which typing writes on the state for each
    /// qualified formal. domains.md, "Carrier-exact membership": a
    /// parameter `value: T in D` "gives the body the fact `value in D`".
    ///
    /// The formal's type already says it, so such a contract adds nothing
    /// to read: the type is what a caller must supply, and the domain's
    /// predicates join the entry clauses from the type (`predicates`). Any
    /// other contract on a state stays refused.
    fn contracts_restate_formal_domains(&self, state: &typed_trees::state::State) -> bool {
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode};
        let typed = self.typed;
        let carries = |mut reference: typed_trees::types::TypeReferenceHandle,
                       domain: SymbolHandle| {
            // A reference formal's domains are its referent's: `seal: &Seal in
            // Issued` lends a `Seal in Issued`.
            while reference.is_valid()
                && let TypeReferenceNode::Reference { referee, .. } =
                    typed.type_reference_table.type_reference(reference)
            {
                reference = *referee;
            }
            while reference.is_valid()
                && let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = typed.type_reference_table.type_reference(reference)
            {
                if typed
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, TypeConstraintNode::Domain(listed)
                            if listed.subject == DomainConstraintSubject::Declared
                                && listed.symbol == domain)
                    })
                {
                    return true;
                }
                reference = *base_type;
            }
            false
        };
        typed.state_contracts(state).iter().all(|contract| {
            contract.kind == SignatureContractKind::Requires
                && typed
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .all(|fact| {
                        let ProofFact::Membership(membership) = fact else {
                            return false;
                        };
                        if !membership.domain_symbol.is_valid()
                            || !membership.domain_arguments.is_empty()
                            || !typed.expression_table.expression_is_valid(membership.value)
                        {
                            return false;
                        }
                        let ExpressionNode::Name(path) =
                            typed.expression_table.expression(membership.value)
                        else {
                            return false;
                        };
                        let Some(formal) = path.resolved_symbol() else {
                            return false;
                        };
                        typed.expression_table.name_path_members(path.members).len() == 1
                            && typed.state_parameters(state).iter().any(|parameter| {
                                parameter.symbol == formal
                                    && carries(parameter.type_reference, membership.domain_symbol)
                            })
                    })
        })
    }

    pub(super) fn served(&self, id: MachineId, declaration: SymbolHandle) -> Checked<()> {
        match self.typed.deferred_refusal(declaration) {
            None => Ok(()),
            Some(row) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Declaration(row.request.construct()),
            )),
        }
    }

    pub(super) fn declaration(
        &self,
        id: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<(&'t Machine, &'t State)> {
        let typed = self.typed;
        if !symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("callee without a symbol"),
            ));
        }
        let machine = typed.machine_by_symbol(symbol).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Call("target is not a machine declaration"),
            )
        })?;
        let states = span(&typed.machine_states, machine.states, id, "state span")?;
        let [state] = states else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Machine("not exactly one state"),
            ));
        };
        Ok((machine, state))
    }

    fn machine_shape(&self, id: MachineId, machine: &Machine, reading: &Reading) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        // The entry of a generic check form keeps its parameters, each
        // rigid; a generic machine it calls is read at the call's type
        // arguments; every other machine is closed.
        let generic_entry = matches!(reading, Reading::GenericEntry);
        if machine.lexical_region_origin.is_some() {
            return unserved("lexical region origin");
        }
        if machine.assembly_target_requirement
            != language_core::inline_assembly::AsmTargetApplicability::Any
        {
            return unserved("assembly target");
        }
        // A machine declared under a data type's name, `machine
        // Counter::clear(counter: &mut Counter)`, is an ordinary static
        // machine whose name has two parts. machines.md, "Static
        // declarations and explicit state": "All machines are static
        // declarations independent of data", and qualification "never binds
        // code to a data type". So the data type gives the machine no
        // parameter, no storage and no `self`: its formals and body are read
        // like any other machine's, and a caller passes every argument
        // (`calls`, `names_carrier`). That page also retires the spelling;
        // the typed trees still hold it. It was served while a refusal here
        // sent the program to the deleted checker, which accepted it; no
        // one has ruled since whether this reader should refuse it.
        //
        // What stays refused is a data type that could give the machine
        // something after all: one applied at type arguments, or a generic
        // one, whose parameters the body would be read under. Lifetime
        // parameters of the data type give it nothing either: the machine
        // is read under its own lifetime binders, and a call gives a region
        // only to those (`calls`, `view_regions`).
        if machine.attached_data_application.is_valid() {
            return unserved("attached data applied at type arguments");
        }
        if machine.attached_data_symbol.is_valid() {
            let plain = self
                .data_declaration(machine.attached_data_symbol)
                .is_some_and(|data| {
                    data.type_parameters.is_empty()
                        && data.generic_instance.is_none()
                        && data.conformance_bounds.is_empty()
                });
            if !plain {
                return unserved("attached generic data");
            }
        }
        if machine.generic_data_template.is_valid() {
            return unserved("generic data template");
        }
        if machine.spelling.is_some() {
            return unserved("operator spelling");
        }
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            return unserved("supply mode other than a checked body");
        }
        if machine.purpose != MachinePurpose::Runtime {
            return unserved("provider-default purpose");
        }
        if machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.target_family_interface
        {
            return unserved("target selection");
        }
        if !machine.body_is_present || machine.body_status != MachineBodyStatus::Complete {
            return unserved("no complete body");
        }
        if machine.structural_type_equations_pending {
            return unserved("pending type equations");
        }
        // A written `reaches` ceiling is read with the signature
        // (`reach_ceiling`). A ceiling bound by an installation is an
        // abstract row with no concrete services to compare.
        if machine.service_reach_is_installation_bound
            || self
                .typed
                .service_reach_rows
                .is_abstract(machine.service_reach_row)
        {
            return unserved("service reach bound by an installation");
        }
        if machine.suspends || machine.blocks {
            return unserved("suspends or blocks");
        }
        // A lifetime parameter selects no type, layout or monomorphized
        // copy (lifetimes.md: "Lifetime arguments are erased regions,
        // separate from runtime generic arity"): the body is read once with
        // each parameter as a region of its own, and a call instantiates them
        // with the caller's (`regions`, `call_of`). Its binders are checked
        // as a data declaration's are.
        super::places::lifetime_binders(id, &machine.lifetime_parameters)?;
        // An implementation parameter binds no type and no value: the body
        // only calls through it. A value parameter (`const Units: u64`) is a
        // value of a known carrier, read as a formal no caller passes
        // (`value_binders`). A type parameter would put a value of a rigid
        // type into the lowered form, which has no row for one.
        let parameters = self.typed.machine_type_parameters(machine);
        if parameters.is_empty() != machine.type_parameters.is_empty() {
            return unserved("type parameter span");
        }
        // Each refusal says which rule the parameters broke and names the
        // declaration: they are different constructs with different work
        // behind them, and one shared text sent a reader to the wrong one.
        let unserved_parameters = |what| Err(refuse_parameters(id, what, machine.name.as_str()));
        match reading {
            Reading::Closed | Reading::Implementation | Reading::Provider
                if parameters.is_empty() => {}
            Reading::Closed | Reading::Implementation | Reading::Provider => {
                return unserved_parameters("generic machine read without arguments");
            }
            // A type parameter is opaque here: a value of it can be named,
            // passed and returned, and nothing selects on it. One declared
            // `[linear]` must be used exactly once, which this route does
            // not judge, so only the default and `[copy]` are read.
            Reading::GenericEntry => {
                for parameter in parameters {
                    match parameter.kind {
                        typed_trees::data::TypeParameterKind::Implementation
                        | typed_trees::data::TypeParameterKind::Const { .. } => {}
                        typed_trees::data::TypeParameterKind::Type
                            if parameter.bounds.multiplicity
                                != language_core::Multiplicity::Linear => {}
                        typed_trees::data::TypeParameterKind::Type => {
                            return unserved_parameters("linear type parameter");
                        }
                        _ => {
                            return unserved_parameters(
                                "parameter that is no type, value or implementation",
                            );
                        }
                    }
                }
            }
            // Applied at one type argument per parameter, all type
            // parameters; so is one alternative of a finite family.
            Reading::Applied(arguments) | Reading::Alternative(arguments) => {
                if parameters.len() != arguments.len()
                    || parameters.iter().any(|parameter| {
                        parameter.kind != typed_trees::data::TypeParameterKind::Type
                    })
                {
                    return unserved_parameters(
                        "arguments that are not one type for each type parameter",
                    );
                }
            }
        }
        if !machine.declared_type_equations.is_empty()
            || !machine.owned_data.is_empty()
            || (!machine.conformance_bounds.is_empty() && !generic_entry)
        {
            return unserved("type equations, owned data or conformances");
        }
        // A `satisfies` clause is an obligation of the machine that states
        // it (`satisfied_requirements`); only a closed machine's is read.
        if !machine.satisfies.is_empty()
            && !matches!(
                reading,
                Reading::Closed | Reading::Implementation | Reading::Provider
            )
        {
            return unserved("`satisfies` clause on a generic machine");
        }
        if matches!(reading, Reading::Closed)
            && let Some((owner, _)) = self.conformer_requirement(id, machine)?
            && self
                .typed
                .trait_definition_by_symbol(owner)
                .is_some_and(|owner| owner.is_boundary)
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Machine(
                    "a boundary conformer is callable only through its provider slot",
                ),
            ));
        }
        Ok(())
    }

    /// The statement each `satisfies` clause of a closed machine makes: the
    /// machine refines that requirement of that trait. conformances.md,
    /// "Binding a machine to a requirement": the machine "refines the
    /// complete requirement", and "A matching signature is insufficient."
    /// So wherever the machine is judged, each clause is owed as a
    /// conformance witness, whether the machine is a chosen provider, a
    /// subject declaration or a callee. A clause the route does not read
    /// (one of an applied trait, of an operator or a top-level requirement,
    /// or one carrying a native binding) refuses the machine by name.
    fn satisfied_requirements(
        &self,
        id: MachineId,
        machine: &'t Machine,
    ) -> Checked<Vec<crate::ConformanceWitness>> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        let mut witnesses = Vec::new();
        for clause in typed.machine_trait_conformances(machine) {
            if clause.external_binding.is_some()
                || clause.external_binding_source_span.is_some()
                || clause.via_expression.is_valid()
                || clause.alias.is_some()
            {
                return unserved("`satisfies` clause with a native binding or an alias");
            }
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(typed, machine, clause)
            else {
                return unserved("`satisfies` clause that names no requirement of a trait");
            };
            if requirement.symbol != clause.requirement_symbol
                || !clause.trait_lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
                || !definition.type_parameters.is_empty()
            {
                return unserved("`satisfies` clause of a trait with parameters");
            }
            witnesses.push(crate::ConformanceWitness {
                declaring_trait: definition.symbol,
                requirement: requirement.symbol,
                implementation: machine.symbol,
            });
        }
        Ok(witnesses)
    }

    /// The value parameters of a generic check form's entry, in declaration
    /// order: each `const` binder with its carrier type. generics.md, "Value
    /// binders and const requirements": a `const Count: u32` binds a value
    /// that "must be statically known within the compiled specialization",
    /// so the unspecialized body knows only its carrier and what the
    /// `requires` say of it. Only an integer carrier is served.
    fn value_binders(&self, id: MachineId, machine: &Machine) -> Checked<Vec<ValueBinder>> {
        let mut binders = Vec::new();
        for parameter in self.typed.machine_type_parameters(machine) {
            let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
            else {
                continue;
            };
            if !parameter.symbol.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("value parameter without a symbol"),
                ));
            }
            let scalar_type = self.scalar_type(id, type_reference)?;
            if !matches!(scalar_type, semantic_vocabulary::ScalarType::Integer(_)) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Parameter("value parameter that is not an integer"),
                ));
            }
            binders.push(ValueBinder {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                scalar_type,
            });
        }
        Ok(binders)
    }

    fn signature(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
        reading: &Reading,
    ) -> Checked<(Signature, MachineNames, usize)> {
        let (machine, state) = self.declaration(id, symbol)?;
        self.served(id, symbol)?;
        // The entry of a generic check form keeps its value parameters as
        // formals; `machine_shape` refuses them on every other machine.
        let binders = match reading {
            Reading::GenericEntry => self.value_binders(id, machine)?,
            Reading::Closed
            | Reading::Implementation
            | Reading::Provider
            | Reading::Applied(_)
            | Reading::Alternative(_) => Vec::new(),
        };
        self.machine_shape(id, machine, reading)?;
        // What each type parameter stands for while this machine is read.
        self.scope = match reading {
            Reading::Closed | Reading::Implementation | Reading::Provider => Vec::new(),
            Reading::GenericEntry => self
                .typed
                .machine_type_parameters(machine)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameter.kind == typed_trees::data::TypeParameterKind::Type
                })
                .map(|(position, parameter)| {
                    let rigid = terminal_check_tables::values::RigidType(position as u32);
                    (parameter.symbol, ValueType::Rigid(rigid))
                })
                .collect(),
            Reading::Applied(arguments) | Reading::Alternative(arguments) => self
                .typed
                .machine_type_parameters(machine)
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(arguments.iter().copied())
                .collect(),
        };
        if self.scope.iter().any(|(binder, _)| !binder.is_valid()) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("type parameter without a symbol"),
            ));
        }
        if !self.contracts_restate_formal_domains(state) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("contracts on a state"),
            ));
        }
        let (mut signature, mut names) = self.contract(
            id,
            state.symbol,
            state.parameters,
            state.return_type,
            machine.contracts,
            Declarer::Machine {
                declares_no_crash_route: machine.declares_no_crash_route,
                is_public: machine.is_public,
            },
            &binders,
            &[],
        )?;
        signature.termination = self.termination(id, machine)?;
        self.inherit_requirement_contract(id, machine, &mut signature, &mut names)?;
        // A machine's header clauses belong to the machine declaration; its
        // formals belong to its one state, which `contract` was given.
        signature.reaches = self.reach_ceiling(id, machine.symbol)?;
        signature.invokes = self.entry_ceiling(id, self.typed.machine_invokes(machine), false)?;
        // Diagnostics name the declaration the author wrote, not a clone
        // stage 04 generated from it.
        let authored = crate::template_of(self.typed, symbol)
            .and_then(|template| self.typed.machine_by_symbol(template))
            .unwrap_or(machine);
        names.machine = authored.name.as_str().to_owned();
        names.state = state.name.as_str().to_owned();
        if matches!(reading, Reading::Closed | Reading::Provider)
            && let Some((owner, requirement)) = self.conformer_requirement(id, machine)?
        {
            let (routed, routed_names) = self.requirement_signature(id, requirement, &[])?;
            self.same_conformer_shape(id, &signature, &routed, machine.symbol)?;
            // There is one executable entry: its caller-facing contract is
            // the requirement's. The member's own clauses remain owed by
            // the independent root-scoped conformance witness.
            signature.requires = routed.requires;
            signature.ensures = routed.ensures;
            names.requires = routed_names.requires;
            names.ensures = routed_names.ensures;
            if !matches!(routed.reaches, crate::Ceiling::Inferred) {
                signature.reaches = routed.reaches;
            }
            signature.invokes = routed.invokes;
            signature.termination = routed.termination;
            if !matches!(routed.crashes, CrashCeiling::Inferred) {
                signature.crashes = routed.crashes;
            }
            let witness = crate::ConformanceWitness {
                declaring_trait: owner,
                requirement: requirement.symbol,
                implementation: machine.symbol,
            };
            if !self.conformance_witnesses.contains(&witness) {
                self.conformance_witnesses.push(witness);
            }
        }
        Ok((signature, names, binders.len()))
    }

    /// The one requirement that owns this body. A reference row owns no
    /// body and supplies no context here. Both source spellings therefore
    /// share the same advertised entry and the same root-only body check.
    fn conformer_requirement(
        &self,
        id: MachineId,
        machine: &'t Machine,
    ) -> Checked<Option<(SymbolHandle, &'t StateSignature)>> {
        let context = match self.declared_requirement_origin(id, machine)? {
            Some(context) => context,
            None => {
                let Some((_, row)) = self.typed.owned_conformance_realization(machine.symbol)
                else {
                    return Ok(None);
                };
                (
                    row.declaring_trait,
                    self.declared_requirement(id, row.declaring_trait, row.requirement)?,
                )
            }
        };
        let (owner, requirement) = context;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let definition = self
            .typed
            .trait_definition_by_symbol(owner)
            .ok_or_else(|| unserved("conformer lost its originating trait declaration"))?;
        self.requirement_termination(id, requirement)?;
        if !definition.type_parameters.is_empty()
            || !definition.lifetime_parameters.is_empty()
            || !definition.conformance_bounds.is_empty()
            || !definition.requires.is_empty()
            || definition.refines.is_some()
            || !definition.refinement_clauses.is_empty()
        {
            return Err(unserved(
                "declared requirement needs its complete trait application",
            ));
        }
        if machine.is_public && !definition.is_public {
            return Err(unserved("conformer is more visible than its requirement"));
        }
        Ok(Some(context))
    }

    fn declared_requirement_origin(
        &self,
        id: MachineId,
        machine: &'t Machine,
    ) -> Checked<Option<(SymbolHandle, &'t StateSignature)>> {
        if machine.satisfies.is_empty() {
            return Ok(None);
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let [edge] = self.typed.machine_trait_conformances(machine) else {
            return Err(unserved(
                "conformer without exactly one declared requirement",
            ));
        };
        if !edge.arguments.is_empty()
            || !edge.trait_lifetime_arguments.is_empty()
            || edge.external_binding.is_some()
            || edge.via_expression.is_valid()
        {
            return Err(unserved(
                "declared requirement with arguments or external supply",
            ));
        }
        let witnesses = self.satisfied_requirements(id, machine)?;
        let [binding] = witnesses.as_slice() else {
            return Err(unserved(
                "declared edge without one exact trait requirement",
            ));
        };
        let definition = self
            .typed
            .trait_definition_by_symbol(binding.declaring_trait)
            .ok_or_else(|| unserved("declared edge lost its trait declaration"))?;
        let requirement =
            self.declared_requirement(id, binding.declaring_trait, binding.requirement)?;
        if edge.requirement_symbol != requirement.symbol || edge.symbol != definition.symbol {
            return Err(unserved(
                "declared requirement identity differs from its resolved edge",
            ));
        }
        if let Some((_, row)) = self.typed.owned_conformance_realization(machine.symbol)
            && (row.declaring_trait != definition.symbol || row.requirement != requirement.symbol)
        {
            return Err(unserved(
                "inline and declared edges name different requirements",
            ));
        }
        Ok(Some((definition.symbol, requirement)))
    }

    fn same_conformer_shape(
        &mut self,
        id: MachineId,
        body: &Signature,
        required: &Signature,
        machine: SymbolHandle,
    ) -> Checked<()> {
        // Distinct declarations own distinct lifetime binders. Compare the
        // supplied signature after instantiating it in the requirement's
        // regions, for both inherited clauses and the advertised entry.
        if !self.same_signature(id, required, body, machine)?
            || body.erased != required.erased
            || body.borrowed != required.borrowed
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("member signature differs from its requirement's"),
            ));
        }
        Ok(())
    }

    /// An owning body inherits missing logical clauses and progress from
    /// its requirement. A selection row supplies no body context
    /// (conformances.md, "Binding a machine to a requirement"). Logical
    /// clauses retain formal positions; progress is owed by the actual body.
    /// Written clauses still owe refinement, and omitted operational
    /// ceilings retain their separate inference rules.
    fn inherit_requirement_contract(
        &mut self,
        id: MachineId,
        machine: &'t Machine,
        signature: &mut Signature,
        names: &mut MachineNames,
    ) -> Checked<()> {
        let Some((_, requirement)) = self.conformer_requirement(id, machine)? else {
            return Ok(());
        };
        if matches!(signature.termination, Termination::Silent) {
            signature.termination = self.requirement_termination(id, requirement)?;
        }
        let clauses = span(
            &self.typed.signature_contracts,
            machine.contracts,
            id,
            "contract span",
        )?;
        let authored = |kind| {
            clauses
                .iter()
                .any(|clause| clause.kind == kind && clause.keyword_source_span.is_some())
        };
        let requires = !authored(SignatureContractKind::Requires);
        let ensures = !authored(SignatureContractKind::Ensures);
        if !requires && !ensures {
            return Ok(());
        }
        let (inherited, inherited_names) = self.requirement_signature(id, requirement, &[])?;
        self.same_conformer_shape(id, signature, &inherited, machine.symbol)?;
        if requires {
            // Implicit formal-domain obligations remain alongside inherited
            // inputs. Canonicalization may merge an obligation both declare.
            let mut clauses = std::mem::take(&mut signature.requires);
            clauses.extend(inherited.requires);
            let mut texts = std::mem::take(&mut names.requires);
            texts.extend(inherited_names.requires);
            signature.requires = canonical::clauses(clauses.clone(), Repeated::Merge, id)?;
            names.requires = signature
                .requires
                .iter()
                .map(|clause| {
                    let position = clauses.iter().position(|candidate| candidate == clause);
                    position
                        .and_then(|position| texts.get(position).cloned())
                        .unwrap_or_default()
                })
                .collect();
        }
        if ensures {
            for (position, clause) in inherited.ensures.into_iter().enumerate() {
                if !signature.ensures.contains(&clause) {
                    signature.ensures.push(clause);
                    names.ensures.push(
                        inherited_names
                            .ensures
                            .get(position)
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
            }
        }
        Ok(())
    }

    /// The value parameters of a trait requirement, in declaration order,
    /// each with its carrier. Any other kind of parameter is refused: a
    /// requirement over a type parameter has no lowered form yet.
    pub(super) fn requirement_binders(
        &self,
        id: MachineId,
        requirement: &StateSignature,
    ) -> Checked<Vec<(SymbolHandle, semantic_vocabulary::ScalarType)>> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        let parameters = self
            .typed
            .data_type_parameters
            .span_or_empty(requirement.type_parameters);
        if parameters.is_empty() != requirement.type_parameters.is_empty() {
            return unserved("type parameter span");
        }
        let mut binders = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
            else {
                return Err(refuse_parameters(
                    id,
                    "requirement parameter that is not a value",
                    requirement.name.as_str(),
                ));
            };
            if !parameter.symbol.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("value parameter without a symbol"),
                ));
            }
            let scalar_type = self.scalar_type(id, type_reference)?;
            if !matches!(scalar_type, semantic_vocabulary::ScalarType::Integer(_)) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Parameter("value parameter that is not an integer"),
                ));
            }
            binders.push((parameter.symbol, scalar_type));
        }
        Ok(binders)
    }

    /// A trait requirement's signature and contract. The typed trees hold a
    /// requirement as a bare signature, not as a machine. `arguments` are
    /// the literals of its value parameters, in declaration order: its
    /// clauses are read with each binder standing for its literal, so the
    /// contract is the one a call with those static arguments is held to.
    pub(super) fn requirement_signature(
        &mut self,
        id: MachineId,
        requirement: &'t StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<(Signature, MachineNames)> {
        // As a machine's: an elided lifetime is a fresh parameter of the
        // requirement, which each call instantiates (`regions::Elision`).
        let elision = std::mem::replace(
            &mut self.elision,
            super::regions::Elision::Inputs(requirement.symbol),
        );
        let read = self.read_requirement_signature(id, requirement, arguments);
        self.elision = elision;
        let (mut signature, names) = read?;
        signature.termination = self.requirement_termination(id, requirement)?;
        Ok((signature, names))
    }

    fn read_requirement_signature(
        &mut self,
        id: MachineId,
        requirement: &'t StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<(Signature, MachineNames)> {
        self.served(id, requirement.symbol)?;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        // Closing a signature substitutes its types; it does not prove the
        // original binders' storage or copying requirements.
        if self
            .typed
            .closed_requirement_application(requirement.symbol)
            .is_some_and(|closed| {
                closed
                    .invocation_contract_parameters
                    .iter()
                    .any(|parameter| {
                        parameter.bounds != typed_trees::data::DataProperties::default()
                    })
            })
        {
            return unserved("closed requirement parameter eligibility");
        }
        if requirement.spelling.is_some() {
            return unserved("operator spelling");
        }
        if !requirement.lifetime_parameters.is_empty() {
            return Err(refuse_parameters(
                id,
                "requirement with lifetime parameters",
                requirement.name.as_str(),
            ));
        }
        if !requirement.conformance_bounds.is_empty() {
            return Err(refuse_parameters(
                id,
                "requirement with conformance bounds",
                requirement.name.as_str(),
            ));
        }
        let binders = self.requirement_binders(id, requirement)?;
        if binders.len() != arguments.len()
            || binders
                .iter()
                .zip(arguments)
                .any(|((_, scalar_type), literal)| literal.scalar_type() != *scalar_type)
        {
            return unserved("value arguments that are not the requirement's parameters");
        }
        let bound: Vec<_> = binders
            .iter()
            .map(|(symbol, _)| *symbol)
            .zip(arguments.iter().cloned())
            .collect();
        if requirement.is_default || !requirement.native_callback_parameters.is_empty() {
            return unserved("default or native-callback requirement");
        }
        if requirement.service_reach_is_installation_bound
            || self
                .typed
                .service_reach_rows
                .is_abstract(requirement.service_reach_row)
        {
            return unserved("service reach bound by an installation");
        }
        if requirement.suspends || requirement.blocks {
            return unserved("suspends or blocks");
        }
        let (mut signature, mut names) = self.contract(
            id,
            requirement.symbol,
            requirement.parameters,
            requirement.return_type,
            requirement.contracts,
            Declarer::Requirement {
                declares_no_crash_route: requirement.declares_no_crash_route,
            },
            &[],
            &bound,
        )?;
        signature.invokes =
            self.entry_ceiling(id, self.typed.state_signature_invokes(requirement), true)?;
        names.machine = requirement.name.as_str().to_owned();
        Ok((signature, names))
    }

    /// The formals, result and clauses of one callable. `owner` is the
    /// symbol its reserved `result` is recorded against. `declared` says
    /// whose clauses they are, for what an absent `crashes` row means.
    /// `binders` are the value parameters a generic check form leaves rigid:
    /// each is a scalar formal after the authored ones, which a clause names
    /// by the binder's symbol like any formal. `bound` are the value
    /// parameters this reading has bound to literals instead.
    fn contract(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        formals: arena::HandleSpan<StateParameter>,
        return_type: typed_trees::types::TypeReferenceHandle,
        contracts: arena::HandleSpan<typed_trees::signature::SignatureContract>,
        declared: Declarer,
        binders: &[ValueBinder],
        bound: &[(SymbolHandle, semantic_vocabulary::ScalarTerm)],
    ) -> Checked<(Signature, MachineNames)> {
        let typed = self.typed;
        let formals = span(&typed.state_parameters, formals, id, "parameter span")?;
        let mut parameters = Vec::with_capacity(formals.len());
        let mut erased = Vec::new();
        let mut borrowed = Vec::new();
        let mut slots = Vec::with_capacity(formals.len());
        for (position, formal) in formals.iter().enumerate() {
            let (value_type, is_erased, reference) = self.formal(id, formal)?;
            parameters.push(value_type);
            if is_erased {
                erased.push(position);
            }
            let access = reference.as_ref().map(|reference| reference.access);
            if let Some(reference) = reference {
                borrowed.push(crate::body::Borrowed {
                    position,
                    ..reference
                });
            }
            slots.push(Slot {
                symbol: formal.symbol,
                value_type,
                // What a clause reader needs to know of the binding mode: a
                // guarantee or a crash route guard may not name a formal
                // the body can replace (`ClauseScope::formal_fields`).
                assignable: mutable_formal(formal, access),
                homed: false,
                erased: is_erased,
                borrowed: access,
            });
        }
        for binder in binders {
            let value_type = ValueType::Scalar(binder.scalar_type, Qualification::NONE);
            parameters.push(value_type);
            slots.push(Slot {
                symbol: binder.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
            });
        }
        // An elided lifetime in the result is the one its inputs carry
        // (`regions::Elision::Result`).
        let inputs = self.elision;
        if let super::regions::Elision::Inputs(owner) = inputs {
            self.elision = super::regions::Elision::Result(owner);
        }
        let result = self.result_type(id, return_type);
        self.elision = inputs;
        let result = result?;
        let values = crate::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            result,
        );
        // The result pseudo-value is the value right after the scalar
        // formals (`Signature::result`), owned by the machine's one state:
        // stage 02 records the entry state as the owner of a machine
        // contract's reserved `result`.
        let scalar_formals = slots
            .iter()
            .filter(|slot| {
                slot.value_type.declared().is_some() && !slot.erased && slot.borrowed.is_none()
            })
            .count();
        let result_value = ValueId::new(scalar_formals as u64 + 1).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("result identity"),
            )
        })?;
        let entry = ClauseScope {
            slots: &slots,
            values: &values,
            bound,
            owner,
            result: ResultTerm::Absent("result in an entry clause"),
            subject: None,
            formal_fields: false,
            mathematical: MathematicalTerms::Served,
        };
        // Only an entry `requires` may project a record formal.
        let required = ClauseScope {
            formal_fields: true,
            mathematical: MathematicalTerms::Served,
            ..entry
        };
        let exit = ClauseScope {
            result: match result {
                // A policy-qualified result is named as its stored value.
                Some(ValueType::Scalar(scalar_type, qualification))
                    if qualification.domains.is_empty() =>
                {
                    ResultTerm::Named(result_value, scalar_type, qualification.policy)
                }
                // domains.md, "Operands": a builtin operator or comparison
                // takes unqualified operands, in `ensures` too.
                Some(ValueType::Scalar(..)) => {
                    ResultTerm::Absent("result that carries a declared domain")
                }
                // A clause over a record result projects it
                // (`result.field`), which a later slice serves from the
                // result place.
                Some(ValueType::Record(_, _, _)) => {
                    ResultTerm::Absent("result of a record machine")
                }
                Some(ValueType::Rigid(_)) => ResultTerm::Absent("result of a rigid type"),
                None => ResultTerm::Absent("result of a Unit machine"),
            },
            mathematical: MathematicalTerms::Served,
            ..entry
        };
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut requires_text = Vec::new();
        let mut ensures_text = Vec::new();
        let mut routes = Vec::new();
        let mut crash_clauses = false;
        for contract in span(&typed.signature_contracts, contracts, id, "contract span")? {
            if contract.binding.is_some() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("result binding"),
                ));
            }
            let facts = span(&typed.proof_facts, contract.facts, id, "fact span")?;
            match contract.kind {
                // A literal `true` asks nothing of a caller and promises
                // nothing, so it adds no hypothesis. A written `requires`
                // still fixes the complete input contract; the declaration
                // marker is read separately by `private_input_contracts`.
                // So does a clause closed type equations decide as true:
                // `requires T == u32 || T == u64` in the machine generated
                // for `u32`.
                SignatureContractKind::Requires => {
                    for fact in facts {
                        let expression = self.fact(id, fact)?;
                        if self.states_nothing(id, expression)? {
                            continue;
                        }
                        requires.push(self.clause(id, &required, expression)?);
                        requires_text.push(self.clause_text(&slots, expression));
                    }
                }
                SignatureContractKind::Ensures => {
                    for fact in facts {
                        let expression = self.fact(id, fact)?;
                        if self.states_nothing(id, expression)? {
                            continue;
                        }
                        ensures.push(self.clause(id, &exit, expression)?);
                        ensures_text.push(self.clause_text(&slots, expression));
                    }
                }
                SignatureContractKind::EnsuresForResultCase { .. } => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("outcome-specific ensures"),
                    ));
                }
                // effects.md, "Guarded crashes": "Each `crashes` clause names
                // one cause (`Trap` or `Abort`); its route guards are
                // alternatives, and an empty route list means `true`." Route
                // guards describe invocation entry, so they cannot name
                // `result`.
                SignatureContractKind::Crashes { cause } => {
                    crash_clauses = true;
                    let cause = crash_cause(cause);
                    if facts.is_empty() {
                        routes.push((cause, CrashRouteGuard::Truth));
                    }
                    for fact in facts {
                        let guard = self.clause(id, &entry, self.fact(id, fact)?)?;
                        routes.push((
                            cause,
                            CrashRouteGuard::Predicate(CrashPredicateTerm::new(guard)),
                        ));
                    }
                }
            }
        }
        // A formal or a result in a domain with predicates states them as
        // entry clauses and guarantees (`predicates`); a clause the
        // callable also wrote itself is stated once.
        let domains = self.signature_predicates(
            id,
            &required,
            &exit,
            result.map(|result| (result, result_value)),
        )?;
        for (clause, text) in domains.requires {
            if !requires.contains(&clause) {
                requires.push(clause);
                requires_text.push(text);
            }
        }
        for (clause, text) in domains.ensures {
            if !ensures.contains(&clause) {
                ensures.push(clause);
                ensures_text.push(text);
            }
        }
        // effects.md: "On a public checked body omitted `crashes` means no
        // crash route, exactly as `crashes none`. Private checked bodies may
        // omit any of the three to request inference." An authored
        // `crashes none` leaves no contract row; the typed trees record it
        // as `declares_no_crash_route`, so it is told from an omission by
        // that fact and never by the missing rows.
        let declares_none = match declared {
            Declarer::Machine {
                declares_no_crash_route,
                ..
            }
            | Declarer::Requirement {
                declares_no_crash_route,
            } => declares_no_crash_route,
        };
        let crashes = match (declared, crash_clauses) {
            _ if declares_none && !crash_clauses => CrashCeiling::DeclaredNone,
            // Stage 01 rejects this header; refused here too rather than
            // trusted to have been.
            _ if declares_none => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("`crashes none` beside a `crashes Cause` clause"),
                ));
            }
            (_, true)
            | (
                Declarer::Machine {
                    is_public: true, ..
                },
                false,
            ) => CrashCeiling::Authored(canonical::routes(routes)),
            (Declarer::Machine { .. }, false) => CrashCeiling::Inferred,
            // generics.md, "Requirements and conformance evidence": on a
            // bodyless requirement "omitted `crashes` defers to its published
            // routes", the selected member's, while `crashes none` admits
            // only crash-free members.
            (Declarer::Requirement { .. }, false) => CrashCeiling::Inferred,
        };
        // A requirement closed at an application keeps the reach its
        // requirement wrote: the typed trees hold authored reach rows
        // against the declaration the author wrote them on.
        let reach_author = self
            .typed
            .closed_requirement_application(owner)
            .map_or(owner, |closed| closed.template);
        let reaches = self.reach_ceiling(id, reach_author)?;
        let authored = requires.clone();
        let requires = canonical::clauses(requires, Repeated::Refuse, id)?;
        // Each clause's text follows it into canonical order. A repeated
        // clause was refused above, so each canonical clause is exactly one
        // authored clause.
        let requires_text = requires
            .iter()
            .map(|clause| {
                authored
                    .iter()
                    .position(|candidate| candidate == clause)
                    .and_then(|position| requires_text.get(position).cloned())
                    .unwrap_or_default()
            })
            .collect();
        let names = MachineNames {
            formals: formals
                .iter()
                .map(|formal| formal.name.as_str().to_owned())
                .chain(binders.iter().map(|binder| binder.name.clone()))
                .collect(),
            requires: requires_text,
            ensures: ensures_text,
            ..MachineNames::default()
        };
        Ok((
            Signature {
                parameters,
                erased,
                borrowed,
                result,
                requires,
                ensures,
                reaches,
                // The declaration's own reader states this axis: a machine
                // and a requirement keep their `invokes` rows in different
                // tables and read omission differently.
                invokes: crate::Ceiling::Inferred,
                crashes,
                // A requirement's termination is its selected member's to
                // keep; a machine's is read from its plan by its caller.
                termination: Termination::Silent,
            },
            names,
        ))
    }

    /// A requirement publishes its authored promise even when the selected
    /// body omits the clause (termination.md). Conditional promises need
    /// their actual premise evidence; existing body checks prove a bare promise.
    fn requirement_termination(
        &self,
        id: MachineId,
        requirement: &StateSignature,
    ) -> Checked<Termination> {
        match &requirement.termination_guarantee {
            TerminationGuarantee::NoGuarantee => Ok(Termination::Silent),
            TerminationGuarantee::Terminates { premises } if premises.is_empty() => {
                Ok(Termination::Promised)
            }
            TerminationGuarantee::Terminates { .. } => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("requirement termination needs its actual progress premises"),
            )),
        }
    }

    /// What the header says about ending, from the plan stage 02 fills once
    /// (`MachineTerminationPlan`). Only the presence of a promise and of a
    /// witness is read here: the plan's `RankingWitness` holds its subjects
    /// as rendered text, which this reader never resolves.
    fn termination(&self, id: MachineId, machine: &Machine) -> Checked<Termination> {
        let plan = &machine.termination_plan;
        let promised = match &plan.interface {
            TerminationInterface::InternalDerived
            | TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => false,
            TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
                // termination.md, "Opaque progress profiles": a premise
                // attaches a provider's progress profile to a subject.
                if !premises.is_empty() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("termination under a progress premise"),
                    ));
                }
                true
            }
        };
        Ok(match (&plan.implementation_witness, promised) {
            (Some(_), _) => Termination::Witnessed,
            (None, true) => Termination::Promised,
            (None, false) => Termination::Silent,
        })
    }

    /// Whether the type parameter `rigid` of `machine` is declared `[copy]`.
    fn copies(&self, machine: &Machine, rigid: terminal_check_tables::values::RigidType) -> bool {
        self.typed
            .machine_type_parameters(machine)
            .get(rigid.0 as usize)
            .is_some_and(|parameter| {
                parameter.bounds.multiplicity == language_core::Multiplicity::Unrestricted
            })
    }

    /// Whether a clause states nothing: it is the Boolean literal `true`,
    /// or closed type equations decide it as true. One they decide as false
    /// is refused: the application is outside the family its declaration
    /// admits, and nothing about its body would mean anything.
    pub(super) fn states_nothing(
        &self,
        id: MachineId,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Checked<bool> {
        if expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            )
        {
            return Ok(true);
        }
        match self.static_truth(expression) {
            Some(true) => Ok(true),
            Some(false) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("type equation that is false for this application"),
            )),
            None => Ok(false),
        }
    }

    /// A formal's type, whether it is `[erased]`, and the reference it is
    /// passed by. An erased formal is scalar: an erased record travels in
    /// Terminal Psi's proof-term lane (`erased_proof_formals`), which a later
    /// slice serves.
    ///
    /// A formal of type `&T` or `&mut T` is read as its referent type `T`
    /// with the reference beside it (`Borrowed`), for a builtin scalar or a
    /// plain record `T`. Stage 01 marks `x: &mut T` mutable for the
    /// reference's exclusivity; that is the reference's access, not a `mut`
    /// binding. A `&write T` formal, a reference with a named lifetime and
    /// a reference to anything else stay refused.
    ///
    /// A `mut` formal that holds its own value is read like any other: the
    /// binding mode says only that the body may assign and store into its
    /// own copy (`mutable_formal`), so it changes nothing a caller sees.
    /// Two kinds stay refused by name: an `[erased]` one, which names a
    /// term and has no storage to assign, and one of a type parameter,
    /// whose replaced value no slice disposes of yet.
    fn formal(
        &mut self,
        id: MachineId,
        formal: &StateParameter,
    ) -> Checked<(ValueType, bool, Option<crate::body::Borrowed>)> {
        if !formal.symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("parameter without a symbol"),
            ));
        }
        if formal.is_const {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Parameter("const parameter"),
            ));
        }
        let reference = match formal.type_reference.is_valid().then(|| {
            self.typed
                .type_reference_table
                .type_reference(formal.type_reference)
        }) {
            Some(TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            }) => Some((*referee, *access, lifetime.is_some())),
            _ => None,
        };
        let erased = match formal.relevance {
            language_core::BindingRelevance::Relevant => false,
            language_core::BindingRelevance::Erased => true,
        };
        let Some((referee, access, named_lifetime)) = reference else {
            let value_type = self.value_type(id, formal.type_reference)?;
            if formal.is_mutable {
                let unserved =
                    |what| Err(refuse(id, Construct::Machine, Unserved::Parameter(what)));
                if erased {
                    return unserved("mutable erased parameter");
                }
                if matches!(value_type, ValueType::Rigid(_)) {
                    return unserved("mutable parameter of a type parameter");
                }
            }
            if erased && value_type.policy().is_some() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Parameter("erased parameter of a policy-qualified type"),
                ));
            }
            if erased && value_type.scalar().is_none() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Parameter("erased parameter of a data type"),
                ));
            }
            return Ok((value_type, erased, None));
        };
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Parameter(what)));
        if erased {
            return unserved("erased reference parameter");
        }
        // A byte view keeps the region it names in its type (`bytes`). A
        // reference to anything else is a loan of one place, and what its
        // lifetime relates it to has no reader yet.
        let is_view = matches!(
            self.typed.type_reference_table.type_reference(referee),
            TypeReferenceNode::Slice { .. }
        );
        if named_lifetime && !is_view {
            return unserved("reference parameter with a named lifetime");
        }
        // The loan itself is an unnamed carried reference beside whatever
        // its referent carries, so it is one more input lifetime an elided
        // result could have meant (lifetimes.md: "Several unnamed carried
        // references do not become one source merely because they occupy
        // one parameter"). No type names it; it is counted, not read.
        if !is_view {
            self.elided_region(id)?;
        }
        // Stage 01 sets `is_mutable` for an exclusive reference; a shared
        // one marked mutable would be a `mut` binding of the carrier.
        match access {
            language_core::ReferenceAccess::Shared if !formal.is_mutable => {}
            language_core::ReferenceAccess::Mutable => {}
            language_core::ReferenceAccess::Shared => {
                return unserved("mutable binding of a shared reference");
            }
            language_core::ReferenceAccess::WriteOnly => {
                return unserved("write-only reference parameter");
            }
        }
        let value_type = if is_view {
            self.value_type(id, formal.type_reference)?
        } else {
            self.value_type(id, referee)?
        };
        // A data type with `requires` may be stored into by the callee of an
        // exclusive loan, so its domain is owed again where the loan
        // returns; no slice follows a domain across a call yet. A shared
        // loan admits no store, so the value behind it is the one its owner
        // established.
        if let ValueType::Record(record, _, _) = value_type
            && access == language_core::ReferenceAccess::Mutable
            && self.restricted(record)
        {
            return unserved("reference to data with requires");
        }
        let home = match value_type {
            ValueType::Rigid(_) => return unserved("reference to a value of a type parameter"),
            // Its place would be declared with the identity of a primitive
            // type the lowered form has no row for.
            ValueType::Scalar(_, qualification) if !qualification.is_none() => {
                return unserved("reference to a qualified scalar");
            }
            ValueType::Record(_, _, _) => None,
            ValueType::Scalar(..) => Some(
                Some(self.typed.normalized_type_identity_text(referee))
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Spelling("referent type without an identity"),
                        )
                    })?,
            ),
        };
        Ok((
            value_type,
            false,
            Some(crate::body::Borrowed {
                position: 0,
                access,
                home,
            }),
        ))
    }

    /// Whether a data type or any data type inside it states `requires`.
    fn restricted(&self, record: semantic_vocabulary::StructuralTypeId) -> bool {
        let mut pending = vec![record];
        while let Some(next) = pending.pop() {
            let Some(data) = self.records.get(next) else {
                return true;
            };
            if !data.requires.is_empty() {
                return true;
            }
            if let crate::StorageShape::FixedArray { element, .. } = data.shape {
                pending.push(element);
            }
            for field in data
                .fields()
                .iter()
                .chain(data.cases().iter().flat_map(|case| &case.fields))
            {
                if let ValueType::Record(nested, _, _) = field.value_type {
                    pending.push(nested);
                }
            }
        }
        false
    }

    /// Machine `id`'s body, read with the machine as the owner of the
    /// lifetimes its types name (lifetimes.md: a named data application
    /// must "use binders declared in the lexical owner").
    pub fn body(&mut self, id: MachineId) -> Checked<Body> {
        let symbol = *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
        let owner = std::mem::replace(&mut self.lifetime_owner, symbol);
        let read = self.read_body(id, symbol);
        self.lifetime_owner = owner;
        read
    }

    fn read_body(&mut self, id: MachineId, symbol: SymbolHandle) -> Checked<Body> {
        let typed = self.typed;
        let signature =
            terminal_check_tables::row(&self.signatures, id, id, Construct::Machine)?.clone();
        // A requirement has no statement to read: its formals and nothing.
        if self.requirements.contains(&id) {
            return Ok(Body {
                slots: signature.parameters,
                statements: Vec::new(),
            });
        }
        let (machine, state) = self.declaration(id, symbol)?;
        self.scope = self.scopes.get(id).cloned().unwrap_or_default();
        let formals = span(
            &typed.state_parameters,
            state.parameters,
            id,
            "parameter span",
        )?;
        // A generic check form's value parameters are the formals after the
        // authored ones, as its signature was read.
        let binders = match self.value_parameters.get(id) {
            Some(0) | None => Vec::new(),
            Some(_) => self.value_binders(id, machine)?,
        };
        if formals.len() + binders.len() != signature.parameters.len() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Machine("formals that differ from the signature's"),
            ));
        }
        let mut slot_names: Vec<String> = formals
            .iter()
            .map(|formal| formal.name.as_str().to_owned())
            .chain(binders.iter().map(|binder| binder.name.clone()))
            .collect();
        let mut slots: Vec<Slot> = formals
            .iter()
            .map(|formal| (formal.symbol, formal.is_mutable))
            .chain(binders.iter().map(|binder| (binder.symbol, false)))
            .zip(&signature.parameters)
            .enumerate()
            .map(|(position, ((symbol, is_mutable), value_type))| {
                // A borrowed formal names the caller's storage: a scalar
                // referent is read and stored in place like a home, a record
                // referent like a record formal's root; `&mut` lets the body
                // store into it, `&` does not.
                //
                // A `mut` formal that holds its own value is the callee's
                // own storage, initialised from the argument, and is
                // assigned and stored into as a `mut` local is: a scalar is
                // rebound to the assigned value, so the formal's entry
                // value keeps its identity and every entry fact stays a
                // fact about that value only (fact_validity.md: "Current
                // body values are not entry snapshots"); a record is stored
                // into by field in its own parameter place, which holds the
                // copy the call passed.
                let borrowed = signature
                    .borrowed(position)
                    .map(|reference| reference.access);
                Slot {
                    symbol,
                    value_type: *value_type,
                    assignable: match borrowed {
                        Some(access) => access == language_core::ReferenceAccess::Mutable,
                        None => is_mutable,
                    },
                    homed: borrowed.is_some() && value_type.scalar().is_some(),
                    erased: signature.is_erased(position),
                    borrowed,
                }
            })
            .collect();
        let statements = typed.statement_table.statements(state.statement_nodes);
        if let Some(names) = self.names.get_mut(id) {
            for (position, statement) in statements.iter().enumerate() {
                if let StatementNode::Transition(transition) = statement {
                    names.exits.push((transition.source_span, position));
                }
            }
            names.fall_off = match statements.last() {
                Some(StatementNode::Expression(_)) => statements.len() - 1,
                _ => statements.len(),
            };
        }
        let mut body = Vec::with_capacity(statements.len());
        let mut completed = false;
        // Set once an arm whose guard closed type equations decide as true
        // has completed the machine: the arms after it are the branches of
        // other types, which no value of this application reaches. They are
        // left unread, because they are typed under an equation that is
        // false here (`threshold<u32>` still holds the arm that returns a
        // `u64`).
        let mut statically_completed = false;
        self.static_truths.clear();
        let mut position = 0;
        while let Some(statement) = statements.get(position) {
            if statically_completed && matches!(statement, StatementNode::Transition(_)) {
                position += 1;
                continue;
            }
            if completed {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Completion("statement after an unconditional arm"),
                ));
            }
            let last = position + 1 == statements.len();
            let read = match statement {
                StatementNode::Assembly(_) => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Statement("assembly"),
                    ));
                }
                StatementNode::AssemblyFact(_) => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Statement("assembly fact"),
                    ));
                }
                StatementNode::Assignment(assignment) => self.assignment(id, &slots, assignment)?,
                StatementNode::Call(call) => {
                    let called = self.call_statement(id, &slots, call)?;
                    if !call.discards_result {
                        undiscarded(id, &called)?;
                    }
                    Statement::Evaluate(called)
                }
                StatementNode::Expression(expression) => {
                    let expected = if last { signature.result } else { None };
                    let expression = self.expression(id, &slots, *expression, expected)?;
                    match (last, signature.result) {
                        (true, Some(result)) => {
                            if !self.agrees(expression.result, result) {
                                return Err(self.mistyped(
                                    id,
                                    expression.source,
                                    super::rejections::Destination::Result,
                                    Some(result),
                                    expression.result,
                                    "final value type",
                                ));
                            }
                            completed = true;
                            if let (Some(names), SourceOrigin::Authored(span)) =
                                (self.names.get_mut(id), expression.source)
                            {
                                names.exits.push((span, position));
                            }
                            Statement::Arm {
                                guard: None,
                                source: expression.source,
                                target: Target::Value(expression),
                            }
                        }
                        (false, _) | (true, None) => {
                            undiscarded(id, &expression)?;
                            Statement::Evaluate(expression)
                        }
                    }
                }
                StatementNode::LocalData(local)
                    if local.role == language_core::LocalBindingRole::RecordPattern =>
                {
                    // The marker reads nothing and binds nothing, so it
                    // leaves no statement; only the cursor moves on.
                    self.record_pattern(id, &slots, local)?;
                    position += 1;
                    continue;
                }
                StatementNode::LocalData(local) => {
                    let (slot, statement) = self.local(id, &slots, local)?;
                    if let Some(truth) = self.static_truth(local.initial_value) {
                        self.static_truths.push((local.symbol, truth));
                    }
                    slots.push(slot);
                    slot_names.push(local.name.as_str().to_owned());
                    statement
                }
                // A case arm starts a dispatch over a sum, which takes the
                // whole run of arms and completes the machine.
                StatementNode::Transition(transition) if self.case_arm(transition) => {
                    let (dispatch, taken) =
                        self.dispatch(id, &slots, &statements[position..], signature.result)?;
                    completed = true;
                    position += taken.saturating_sub(1);
                    dispatch
                }
                StatementNode::Transition(transition) => {
                    // An arm whose guard closed type equations decide: a
                    // false one is the branch of another type and is left
                    // unread; a true one is taken unconditionally.
                    let decided = match transition.guard {
                        typed_trees::statement::TransitionGuardNode::When(guard) => {
                            self.static_truth(guard)
                        }
                        typed_trees::statement::TransitionGuardNode::Always => None,
                    };
                    if decided == Some(false) {
                        position += 1;
                        continue;
                    }
                    let (guard, target) =
                        self.transition(id, &slots, transition, signature.result)?;
                    let guard = if decided == Some(true) {
                        statically_completed = true;
                        None
                    } else {
                        guard
                    };
                    completed = guard.is_none();
                    Statement::Arm {
                        guard,
                        target,
                        source: SourceOrigin::of(transition.source_span),
                    }
                }
            };
            body.push(read);
            position += 1;
        }
        if !completed && signature.result.is_some() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Completion("value machine can fall off its end"),
            ));
        }
        if let Some(names) = self.names.get_mut(id) {
            for statement in &body {
                bindings(statement, &slot_names, &mut names.bindings);
                value_exits(statement, &mut names.exits);
            }
        }
        // ownership.md, "Property declarations": "every use of `T` receives
        // the property fixed by `T`'s declaration". A value of a type
        // parameter not declared `[copy]` may be used at most once, and
        // this route has no judgment of moves, so a body that names such a
        // slot twice anywhere is refused, whichever paths the two uses are
        // on.
        let mut uses = vec![0_usize; slots.len()];
        for statement in &body {
            statement_slot_uses(statement, &mut uses);
        }
        for (slot, uses) in slots.iter().zip(&uses) {
            if let ValueType::Rigid(rigid) = slot.value_type
                && *uses > 1
                && !self.copies(machine, rigid)
            {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Expression("second use of a value of a type parameter"),
                ));
            }
        }
        Ok(Body {
            slots: slots.iter().map(|slot| slot.value_type).collect(),
            statements: body,
        })
    }
}

/// References outside the machines whose complete bodies the route read.
/// Text and source locations do not decide whether an entry can be invoked.
struct OutsideInputUses {
    uses: InputSymbolUses,
}

impl Extend<SymbolHandle> for OutsideInputUses {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            if let Some(row) = self.uses.get_mut(symbol)
                && row.target
            {
                row.outside = true;
            }
        }
    }
}

impl typed_trees::references::TypedReferences for OutsideInputUses {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        !self.uses.get(symbol).is_some_and(|row| row.lowered)
    }
}

#[derive(Clone, Copy, Default)]
struct InputSymbolUse {
    lowered: bool,
    target: bool,
    outside: bool,
}

/// Slots are dense within an arena page. Keep each generation separate so
/// equal slot numbers in independently loaded symbol pages never alias.
struct InputSymbolUses {
    pages: Vec<(u32, Vec<InputSymbolUse>)>,
}

impl InputSymbolUses {
    fn new(
        lowered: impl IntoIterator<Item = SymbolHandle>,
        targets: impl IntoIterator<Item = SymbolHandle>,
    ) -> Option<Self> {
        let mut symbols: Vec<_> = lowered
            .into_iter()
            .map(|symbol| {
                (
                    symbol,
                    InputSymbolUse {
                        lowered: true,
                        ..InputSymbolUse::default()
                    },
                )
            })
            .chain(targets.into_iter().map(|symbol| {
                (
                    symbol,
                    InputSymbolUse {
                        target: true,
                        ..InputSymbolUse::default()
                    },
                )
            }))
            .collect();
        symbols.sort_unstable_by_key(|(symbol, _)| (symbol.generation(), symbol.arena_index()));
        let mut pages: Vec<(u32, Vec<InputSymbolUse>)> = Vec::new();
        for (symbol, roles) in symbols {
            if !symbol.is_valid() {
                return None;
            }
            if pages
                .last()
                .is_none_or(|(generation, _)| *generation != symbol.generation())
            {
                pages.push((symbol.generation(), Vec::new()));
            }
            let slots = &mut pages.last_mut()?.1;
            let index = symbol.arena_index() as usize;
            if slots.len() <= index {
                slots.resize(index + 1, InputSymbolUse::default());
            }
            slots[index].lowered |= roles.lowered;
            slots[index].target |= roles.target;
        }
        Some(Self { pages })
    }

    fn get(&self, symbol: SymbolHandle) -> Option<&InputSymbolUse> {
        if !symbol.is_valid() {
            return None;
        }
        let page = self
            .pages
            .binary_search_by_key(&symbol.generation(), |(generation, _)| *generation)
            .ok()?;
        self.pages[page].1.get(symbol.arena_index() as usize)
    }

    fn get_mut(&mut self, symbol: SymbolHandle) -> Option<&mut InputSymbolUse> {
        if !symbol.is_valid() {
            return None;
        }
        let page = self
            .pages
            .binary_search_by_key(&symbol.generation(), |(generation, _)| *generation)
            .ok()?;
        self.pages[page].1.get_mut(symbol.arena_index() as usize)
    }
}

#[cfg(test)]
mod input_symbol_uses {
    use super::*;
    use typed_trees::references::TypedReferences;

    #[test]
    fn a_foreign_page_with_the_same_slot_cannot_hide_an_outside_caller() {
        let helper = SymbolHandle::from_parts(1, 7);
        let foreign_caller = SymbolHandle::from_parts(1, 8);
        let mut outside = OutsideInputUses {
            uses: InputSymbolUses::new([helper], [helper]).unwrap(),
        };
        assert!(!outside.declaration(helper));
        assert!(outside.declaration(foreign_caller));
        outside.extend([foreign_caller]);
        assert!(!outside.uses.get(helper).unwrap().outside);
        // Visiting that foreign declaration reveals its call of the helper.
        outside.extend([helper]);
        assert!(outside.uses.get(helper).unwrap().outside);
    }
}

/// Whether a formal is a `mut` binding of its own value. Stage 01 also marks
/// `x: &mut T` mutable, for the reference's exclusivity; that formal names
/// the caller's storage and is not a binding the body may replace.
fn mutable_formal(
    formal: &StateParameter,
    borrowed: Option<language_core::ReferenceAccess>,
) -> bool {
    formal.is_mutable && borrowed.is_none()
}

/// Every span of a value expression whose result a binder receives, with
/// that binder (`MachineNames::bindings`): a whole-value binding receives
/// its expression into the slot's name, and an arm's value is the return
/// value. A guard, a store and an expression evaluated for effect have no
/// binder.
fn bindings(statement: &Statement, slot_names: &[String], out: &mut Vec<(SourceSpan, Binder)>) {
    match statement {
        Statement::Bind { slot, value } => {
            if let Some(name) = slot_names.get(*slot) {
                spans(value, &Binder::Local(name.clone()), out);
            }
        }
        Statement::Arm { target, .. } => target_spans(target, out),
        Statement::Dispatch { arms, default, .. } => {
            for (_, target, _) in arms {
                target_spans(target, out);
            }
            if let Some((target, _)) = default {
                target_spans(target, out);
            }
        }
        Statement::CaptureReference { .. }
        | Statement::Declare { .. }
        | Statement::Witness { .. }
        | Statement::Store { .. }
        | Statement::Evaluate(_) => {}
    }
}

/// A value exit completes at its value expression's span rather than its
/// arm's, so each one takes the statement position of the arm it ends
/// (`MachineNames::exits`).
fn value_exits(statement: &Statement, exits: &mut Vec<(SourceSpan, usize)>) {
    let arms: Vec<(&Target, SourceOrigin)> = match statement {
        Statement::Arm { target, source, .. } => vec![(target, *source)],
        Statement::Dispatch { arms, default, .. } => arms
            .iter()
            .map(|(_, target, source)| (target, *source))
            .chain(default.iter().map(|(target, source)| (target, *source)))
            .collect(),
        Statement::CaptureReference { .. }
        | Statement::Declare { .. }
        | Statement::Witness { .. }
        | Statement::Bind { .. }
        | Statement::Store { .. }
        | Statement::Evaluate(_) => Vec::new(),
    };
    for (target, arm) in arms {
        let (Target::Value(value), SourceOrigin::Authored(arm)) = (target, arm) else {
            continue;
        };
        let SourceOrigin::Authored(span) = value.source else {
            continue;
        };
        let position = exits
            .iter()
            .find(|(exit, _)| *exit == arm)
            .map(|(_, position)| *position);
        if let Some(position) = position {
            exits.push((span, position));
        }
    }
}

fn target_spans(target: &Target, out: &mut Vec<(SourceSpan, Binder)>) {
    if let Target::Value(value) = target {
        spans(value, &Binder::ReturnValue, out);
    }
}

fn spans(expression: &Expression, binder: &Binder, out: &mut Vec<(SourceSpan, Binder)>) {
    if let SourceOrigin::Authored(span) = expression.source {
        out.push((span, binder.clone()));
    }
    match &expression.kind {
        ExpressionKind::SavedSubject(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Array { elements } => {
            for element in elements {
                spans(element, binder, out);
            }
        }
        ExpressionKind::Index { base, index } => {
            spans(base, binder, out);
            spans(index, binder, out);
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                spans(field, binder, out);
            }
        }
        ExpressionKind::Field { base, .. }
        | ExpressionKind::Payload { base, .. }
        | ExpressionKind::CollectionMeasure { base, .. } => {
            spans(base, binder, out);
        }
        ExpressionKind::IsCase { subject, .. } => spans(subject, binder, out),
        ExpressionKind::Borrow { place, .. } => spans(place, binder, out),
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
        } => {
            spans(subject, binder, out);
            for (_, arm) in arms {
                spans(arm, binder, out);
            }
            if let Some(default) = default {
                spans(default, binder, out);
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            spans(left, binder, out);
            spans(right, binder, out);
        }
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. } => {
            spans(operand, binder, out);
        }
        ExpressionKind::Call { arguments, .. } => {
            for argument in arguments {
                spans(argument, binder, out);
            }
        }
    }
}

/// Whose clauses a contract read is reading, for what an absent `crashes` row
/// means: on a machine, inference when private and no route when public; on
/// a requirement, deferral to the selected member. An authored `crashes
/// none` leaves no row on either, so it is carried beside them.
#[derive(Clone, Copy)]
enum Declarer {
    Machine {
        declares_no_crash_route: bool,
        is_public: bool,
    },
    Requirement {
        declares_no_crash_route: bool,
    },
}

/// One value parameter of a generic check form: the binder's symbol, which
/// is what a name in the body or a clause resolves to, its spelling for
/// diagnostics, and its carrier.
struct ValueBinder {
    symbol: SymbolHandle,
    name: String,
    scalar_type: semantic_vocabulary::ScalarType,
}

/// How a machine's declaration is read.
enum Reading {
    /// No generic parameter: every machine of a concrete module.
    Closed,
    /// Only the root implementation body of a conformance witness, under
    /// its own authored contract. Its callees still use `Closed` routes.
    Implementation,
    /// A body rooted by its exact supplied requirement. It has the same
    /// advertised runtime contract; boundary bodies remain uncallable by name.
    Provider,
    /// The entry of a generic check form, its parameters rigid.
    GenericEntry,
    /// A generic machine at the type arguments of one call in a generic
    /// check form's entry, in declaration order.
    Applied(Vec<ValueType>),
    /// The entry of one alternative's check of a finite type family: the
    /// generic machine with its body, each type parameter fixed to that
    /// alternative's type (`elaborate_alternatives`).
    Alternative(Vec<ValueType>),
}

/// Counts, per slot, how often `statement` names it.
fn statement_slot_uses(statement: &Statement, uses: &mut [usize]) {
    let target = |target: &Target, uses: &mut [usize]| {
        if let Target::Value(value) = target {
            expression_slot_uses(value, uses);
        }
    };
    match statement {
        Statement::Bind { value, .. } => expression_slot_uses(value, uses),
        Statement::Store {
            destination, value, ..
        } => {
            expression_slot_uses(destination, uses);
            expression_slot_uses(value, uses);
        }
        Statement::Evaluate(value) => expression_slot_uses(value, uses),
        // Counted although nothing evaluates it: a type parameter's value is
        // never an erased term, so counting it can only refuse more.
        Statement::Witness { term, .. } => expression_slot_uses(term, uses),
        Statement::Arm {
            guard, target: arm, ..
        } => {
            if let Some(guard) = guard {
                expression_slot_uses(guard, uses);
            }
            target(arm, uses);
        }
        Statement::Dispatch { arms, default, .. } => {
            for (_, arm, _) in arms {
                target(arm, uses);
            }
            if let Some((arm, _)) = default {
                target(arm, uses);
            }
        }
        Statement::CaptureReference { original, .. } => uses[*original] += 1,
        Statement::Declare { .. } => {}
    }
}

fn expression_slot_uses(expression: &Expression, uses: &mut [usize]) {
    match &expression.kind {
        ExpressionKind::Slot(slot) => {
            if let Some(count) = uses.get_mut(*slot) {
                *count += 1;
            }
        }
        ExpressionKind::SavedSubject(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Boolean(_) => {}
        ExpressionKind::Array { elements } => {
            for element in elements {
                expression_slot_uses(element, uses);
            }
        }
        ExpressionKind::Index { base, index } => {
            expression_slot_uses(base, uses);
            expression_slot_uses(index, uses);
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                expression_slot_uses(field, uses);
            }
        }
        ExpressionKind::Field { base, .. }
        | ExpressionKind::Payload { base, .. }
        | ExpressionKind::CollectionMeasure { base, .. } => {
            expression_slot_uses(base, uses);
        }
        ExpressionKind::IsCase { subject, .. } => expression_slot_uses(subject, uses),
        ExpressionKind::Borrow { place, .. } => expression_slot_uses(place, uses),
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
        } => {
            expression_slot_uses(subject, uses);
            for (_, arm) in arms {
                expression_slot_uses(arm, uses);
            }
            if let Some(default) = default {
                expression_slot_uses(default, uses);
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression_slot_uses(left, uses);
            expression_slot_uses(right, uses);
        }
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. } => {
            expression_slot_uses(operand, uses);
        }
        ExpressionKind::Call { arguments, .. } => {
            for argument in arguments {
                expression_slot_uses(argument, uses);
            }
        }
    }
}

/// A statement evaluated for effect leaves no value behind. One that has a
/// value and is not the machine's final value is a defect the route hands
/// back, never a statement it reads past. `wiki/spec/language/effects.md`:
/// "A non-Unit call result cannot be silently discarded. Intentional discard
/// is `_ = call();`".
fn undiscarded(id: MachineId, statement: &Expression) -> Checked<()> {
    if statement.result.is_some() {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Statement("expression statement that discards its value"),
        ));
    }
    Ok(())
}
