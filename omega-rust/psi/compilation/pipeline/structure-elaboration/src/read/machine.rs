/*
The machine family of the reader: which declarations are admitted (the subset
gate), their signatures and formals, and
the driver that reads one body statement by statement.
*/

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

use super::contracts::{ClauseScope, ResultTerm};
use super::{Program, Slot, crash_cause, span};
use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target, Termination,
    ValueType,
};
use crate::canonical::{self, Repeated};
use crate::{Binder, MachineNames, SourceOrigin, Unserved, refuse};
use source::SourceSpan;

impl<'t> Program<'t> {
    /// A helper with no written input requirement can use what every caller
    /// guarantees. A written `requires true` must still support every input.
    /// Its declaration marker survives when normalization removes the literal
    /// fact; an empty clause list alone cannot distinguish the two contracts.
    /// Every subject declaration is read; a callable used as a value or an
    /// unserved selection refuses the whole route. Only direct calls remain
    /// for those helpers. A reached dependency need not include every caller,
    /// so its helpers and static requirement implementations keep their envelope.
    pub(crate) fn private_input_contracts(&self, entry: MachineId) -> Checked<Vec<MachineId>> {
        let mut private = Vec::new();
        let mut fixed = self.symbols.map(|_, _| false);
        for id in std::iter::once(&entry)
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
                    entry,
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
    pub(crate) fn local_result_bodies(&self, entry: MachineId) -> Checked<Vec<MachineId>> {
        let mut boundary = self.symbols.map(|_, _| false);
        for id in std::iter::once(&entry)
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
            if !machine.is_public && crate::template_of(self.typed, symbol).is_none() {
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
        // ponytail: linear scan over the discovered machines; a dense table
        // indexed by machine position if closures grow past a few hundred.
        for position in 1..=self.symbols.len() as u64 {
            let id = MachineId::new(position).expect("positions start at 1");
            if self.symbols.get(id) == Some(&symbol) {
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
        let read = self.signature(id, symbol, &reading);
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

    /// Allocate the supplied contract independently of the witness entry, even
    /// when both views name the same declaration. Reusing the entry would turn
    /// the proof obligation into a recursive call to itself.
    pub fn callable_contract(
        &mut self,
        supplied: crate::CallableDeclaration<'t>,
    ) -> Checked<MachineId> {
        let id = match supplied {
            crate::CallableDeclaration::Signature(signature) => {
                self.witnessed_signature(signature, &[])?
            }
            crate::CallableDeclaration::Machine(symbol) => {
                let id = self.next_identity()?;
                self.read_machine(id, symbol, Reading::Closed)?;
                id
            }
        };
        self.requirements.push(id);
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
        // the typed trees still hold it, and refusing it here would only
        // send the same program to the current checker, which accepts it.
        //
        // What stays refused is a data type that could give the machine
        // something after all: one applied at type arguments, or a generic
        // one, whose parameters the body would be read under.
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
        // Row 1 is the interned empty row (`ServiceReachRowTable::EMPTY_ROW`).
        if !matches!(
            machine.service_reach_row,
            language_semantics::ServiceReachRowId::NULL
                | language_semantics::ServiceReachRowTable::EMPTY_ROW
        ) || machine.service_reach_is_installation_bound
        {
            return unserved("service reach");
        }
        if machine.suspends || machine.blocks {
            return unserved("suspends or blocks");
        }
        if !machine.lifetime_parameters.is_empty() {
            return unserved("generic parameters");
        }
        // An implementation parameter binds no type and no value: the body
        // only calls through it. A value parameter (`const Units: u64`) is a
        // value of a known carrier, read as a formal no caller passes
        // (`value_binders`). A type parameter would put a value of a rigid
        // type into the lowered form, which has no row for one.
        let parameters = self.typed.machine_type_parameters(machine);
        if parameters.is_empty() != machine.type_parameters.is_empty() {
            return unserved("type parameter span");
        }
        let served = match reading {
            Reading::Closed => parameters.is_empty(),
            // A type parameter is opaque here: a value of it can be named,
            // passed and returned, and nothing selects on it. One declared
            // `[linear]` must be used exactly once, which this route does
            // not judge, so only the default and `[copy]` are read.
            Reading::GenericEntry => parameters.iter().all(|parameter| match parameter.kind {
                typed_trees::data::TypeParameterKind::Implementation
                | typed_trees::data::TypeParameterKind::Const { .. } => true,
                typed_trees::data::TypeParameterKind::Type => {
                    parameter.bounds.multiplicity != language_core::Multiplicity::Linear
                }
                _ => false,
            }),
            // Applied at one type argument per parameter, all type
            // parameters; so is one alternative of a finite family.
            Reading::Applied(arguments) | Reading::Alternative(arguments) => {
                parameters.len() == arguments.len()
                    && parameters.iter().all(|parameter| {
                        parameter.kind == typed_trees::data::TypeParameterKind::Type
                    })
            }
        };
        if !served {
            return unserved("generic parameters");
        }
        if !machine.declared_type_equations.is_empty()
            || !machine.owned_data.is_empty()
            || !machine.satisfies.is_empty()
            || (!machine.conformance_bounds.is_empty() && !generic_entry)
        {
            return unserved("type equations, owned data or conformances");
        }
        if !machine.invokes.is_empty() {
            return unserved("invokes");
        }
        Ok(())
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
        // The entry of a generic check form keeps its value parameters as
        // formals; `machine_shape` refuses them on every other machine.
        let binders = match reading {
            Reading::GenericEntry => self.value_binders(id, machine)?,
            Reading::Closed | Reading::Applied(_) | Reading::Alternative(_) => Vec::new(),
        };
        self.machine_shape(id, machine, reading)?;
        // What each type parameter stands for while this machine is read.
        self.scope = match reading {
            Reading::Closed => Vec::new(),
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
        if !state.contracts.is_empty() {
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
        // Diagnostics name the declaration the author wrote, not a clone
        // stage 04 generated from it.
        let authored = crate::template_of(self.typed, symbol)
            .and_then(|template| self.typed.machine_by_symbol(template))
            .unwrap_or(machine);
        names.machine = authored.name.as_str().to_owned();
        names.state = state.name.as_str().to_owned();
        Ok((signature, names, binders.len()))
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
                return unserved("generic parameters");
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
    fn requirement_signature(
        &mut self,
        id: MachineId,
        requirement: &'t StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<(Signature, MachineNames)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        if requirement.spelling.is_some() {
            return unserved("operator spelling");
        }
        if !requirement.lifetime_parameters.is_empty() || !requirement.conformance_bounds.is_empty()
        {
            return unserved("generic parameters");
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
        if !requirement.invokes.is_empty() {
            return unserved("invokes");
        }
        if !matches!(
            requirement.service_reach_row,
            language_semantics::ServiceReachRowId::NULL
                | language_semantics::ServiceReachRowTable::EMPTY_ROW
        ) || requirement.service_reach_is_installation_bound
        {
            return unserved("service reach");
        }
        if requirement.suspends || requirement.blocks {
            return unserved("suspends or blocks");
        }
        let (signature, mut names) = self.contract(
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
                if matches!(declared, Declarer::Requirement { .. }) {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Parameter("reference parameter of a requirement"),
                    ));
                }
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
            let value_type = ValueType::Scalar(binder.scalar_type);
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
        let result = self.result_type(id, return_type)?;
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
        };
        // Only an entry `requires` may project a record formal.
        let required = ClauseScope {
            formal_fields: true,
            ..entry
        };
        let exit = ClauseScope {
            result: match result {
                Some(ValueType::Scalar(scalar_type)) => {
                    ResultTerm::Named(result_value, scalar_type)
                }
                // A clause over a record result projects it
                // (`result.field`), which a later slice serves from the
                // result place.
                Some(ValueType::Record(_)) => ResultTerm::Absent("result of a record machine"),
                Some(ValueType::Rigid(_)) => ResultTerm::Absent("result of a rigid type"),
                None => ResultTerm::Absent("result of a Unit machine"),
            },
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
                crashes,
                // A requirement's termination is its selected member's to
                // keep; a machine's is read from its plan by its caller.
                termination: Termination::Silent,
            },
            names,
        ))
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
    fn states_nothing(
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
        if named_lifetime {
            return unserved("reference parameter with a named lifetime");
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
        let value_type = if matches!(
            self.typed.type_reference_table.type_reference(referee),
            TypeReferenceNode::Slice { .. }
        ) {
            self.value_type(id, formal.type_reference)?
        } else {
            self.value_type(id, referee)?
        };
        // A data type with `requires` may be stored into by the callee, so
        // its domain is owed again where the loan returns; no slice follows
        // a domain across a call yet.
        if let ValueType::Record(record) = value_type
            && self.restricted(record)
        {
            return unserved("reference to data with requires");
        }
        let home = match value_type {
            ValueType::Rigid(_) => return unserved("reference to a value of a type parameter"),
            ValueType::Record(_) => None,
            ValueType::Scalar(_) => Some(
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
            for field in data
                .fields()
                .iter()
                .chain(data.cases().iter().flat_map(|case| &case.fields))
            {
                if let ValueType::Record(nested) = field.value_type {
                    pending.push(nested);
                }
            }
        }
        false
    }

    pub fn body(&mut self, id: MachineId) -> Checked<Body> {
        let typed = self.typed;
        let symbol = *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
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
                    let expected = if last {
                        signature.result.and_then(ValueType::scalar)
                    } else {
                        None
                    };
                    let expression = self.expression(id, &slots, *expression, expected)?;
                    match (last, signature.result) {
                        (true, Some(result)) => {
                            if expression.result != Some(result) {
                                return Err(refuse(
                                    id,
                                    Construct::Machine,
                                    Unserved::Typing("final value type"),
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
        Statement::Declare { .. }
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
        Statement::Declare { .. }
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
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                spans(field, binder, out);
            }
        }
        ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
            spans(base, binder, out);
        }
        ExpressionKind::IsCase { subject, .. } => spans(subject, binder, out),
        ExpressionKind::Borrow { place, .. } => spans(place, binder, out),
        ExpressionKind::Match {
            subject,
            arms,
            default,
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
        | ExpressionKind::Cast(operand) => {
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
        Statement::Bind { value, .. } | Statement::Store { value, .. } => {
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
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::ByteLiteral(_) => {}
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                expression_slot_uses(field, uses);
            }
        }
        ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
            expression_slot_uses(base, uses);
        }
        ExpressionKind::IsCase { subject, .. } => expression_slot_uses(subject, uses),
        ExpressionKind::Borrow { place, .. } => expression_slot_uses(place, uses),
        ExpressionKind::Match {
            subject,
            arms,
            default,
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
        | ExpressionKind::Cast(operand) => {
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
