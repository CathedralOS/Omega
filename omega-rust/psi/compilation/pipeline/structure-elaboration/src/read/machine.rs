/*
The machine family of the reader: which declarations are admitted (the subset
gate), their signatures and formals, the call-graph recursion refusal, and
the driver that reads one body statement by statement.
*/

use semantic_vocabulary::{MachineId, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashPredicateTerm, CrashRouteGuard};
use typed_trees::machine::{Machine, MachineBodyStatus, MachinePurpose};
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

use super::contracts::{ClauseScope, ResultTerm};
use super::{Program, Slot, crash_cause, span};
use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target, ValueType,
};
use crate::canonical::{self, Repeated};
use crate::{Binder, MachineNames, SourceOrigin, Unserved, refuse};
use source::SourceSpan;

impl<'t> Program<'t> {
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
        let (signature, names, value_parameters) = self.signature(id, symbol)?;
        self.symbols.push(symbol);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(value_parameters);
        self.value_arguments.push(Vec::new());
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
        let (signature, names) = self.requirement_signature(id, declared, arguments)?;
        self.symbols.push(requirement);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(0);
        self.value_arguments.push(arguments.to_vec());
        Ok(id)
    }

    /// Records that the witness `caller` calls `callee`.
    pub fn forwards(&mut self, caller: MachineId, callee: MachineId) {
        self.calls.push((caller, callee));
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
        self.requirements.push(id);
        Ok(id)
    }

    /// Refuses recursion: a cycle in the call graph read so far, including a
    /// machine that calls itself. Kahn's algorithm over the call edges; a
    /// machine left with incoming edges lies on or behind a cycle.
    pub fn refuse_recursion(&self) -> Checked<()> {
        let count = self.symbols.len();
        let index = |machine: MachineId| machine.get() as usize - 1;
        let mut incoming = vec![0_usize; count];
        for (_, callee) in &self.calls {
            incoming[index(*callee)] += 1;
        }
        let mut ready: Vec<usize> = (0..count)
            .filter(|machine| incoming[*machine] == 0)
            .collect();
        while let Some(machine) = ready.pop() {
            for (caller, callee) in &self.calls {
                if index(*caller) == machine {
                    incoming[index(*callee)] -= 1;
                    if incoming[index(*callee)] == 0 {
                        ready.push(index(*callee));
                    }
                }
            }
        }
        // Kahn's algorithm settles every machine exactly when there is no
        // cycle, so a remaining incoming edge is the whole test.
        match incoming.iter().position(|edges| *edges != 0) {
            None => Ok(()),
            Some(position) => {
                let machine = MachineId::new(position as u64 + 1).expect("positions start at 1");
                Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Call("recursion"),
                ))
            }
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

    fn machine_shape(&self, id: MachineId, machine: &Machine) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        // The entry of a generic check form keeps its implementation
        // parameters, each rigid; every other machine is closed.
        let generic_entry = self.rigid && self.symbols.is_empty();
        if machine.lexical_region_origin.is_some() {
            return unserved("lexical region origin");
        }
        if machine.assembly_target_requirement
            != language_core::inline_assembly::AsmTargetApplicability::Any
        {
            return unserved("assembly target");
        }
        if machine.attached_data_symbol.is_valid() || machine.attached_data_application.is_valid() {
            return unserved("attached data");
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
        let rigid_kinds_only = parameters.iter().all(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Implementation
                    | typed_trees::data::TypeParameterKind::Const { .. }
            )
        });
        if !parameters.is_empty() && !(generic_entry && rigid_kinds_only) {
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
    ) -> Checked<(Signature, MachineNames, usize)> {
        let (machine, state) = self.declaration(id, symbol)?;
        // The entry of a generic check form keeps its value parameters as
        // formals; `machine_shape` refuses them on every other machine.
        let binders = if self.rigid && self.symbols.is_empty() {
            self.value_binders(id, machine)?
        } else {
            Vec::new()
        };
        self.machine_shape(id, machine)?;
        if !state.contracts.is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("contracts on a state"),
            ));
        }
        let (signature, mut names) = self.contract(
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
        let mut slots = Vec::with_capacity(formals.len());
        for formal in formals {
            let value_type = self.formal(id, formal)?;
            parameters.push(value_type);
            slots.push(Slot {
                symbol: formal.symbol,
                value_type,
                assignable: false,
                homed: false,
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
            });
        }
        let result = self.result_type(id, return_type)?;
        // The result pseudo-value is the value right after the scalar
        // formals (`Signature::result`), owned by the machine's one state:
        // stage 02 records the entry state as the owner of a machine
        // contract's reserved `result`.
        let scalar_formals = parameters
            .iter()
            .filter(|parameter| parameter.scalar().is_some())
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
                // A clause that is the literal `true` states nothing: it
                // asks nothing of a caller and promises nothing, so it adds
                // no clause, as an absent `requires` or `ensures` adds none.
                SignatureContractKind::Requires => {
                    for fact in facts {
                        let expression = self.fact(id, fact)?;
                        if self.is_true(expression) {
                            continue;
                        }
                        requires.push(self.clause(id, &required, expression)?);
                        requires_text.push(self.clause_text(&slots, expression));
                    }
                }
                SignatureContractKind::Ensures => {
                    for fact in facts {
                        let expression = self.fact(id, fact)?;
                        if self.is_true(expression) {
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
                result,
                requires,
                ensures,
                crashes,
            },
            names,
        ))
    }

    /// Whether a clause's expression is the Boolean literal `true`.
    fn is_true(&self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            )
    }

    fn formal(&mut self, id: MachineId, formal: &StateParameter) -> Checked<ValueType> {
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
        if formal.is_mutable {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Parameter("mutable parameter"),
            ));
        }
        if formal.relevance != language_core::BindingRelevance::Relevant {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Parameter("erased parameter"),
            ));
        }
        self.value_type(id, formal.type_reference)
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
            .map(|formal| formal.symbol)
            .chain(binders.iter().map(|binder| binder.symbol))
            .zip(&signature.parameters)
            .map(|(symbol, value_type)| Slot {
                symbol,
                value_type: *value_type,
                assignable: false,
                homed: false,
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
        let mut position = 0;
        while let Some(statement) = statements.get(position) {
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
                    Statement::Evaluate(self.call_statement(id, &slots, call)?)
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
                        (false, _) | (true, None) => Statement::Evaluate(expression),
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
                    let (guard, target) =
                        self.transition(id, &slots, transition, signature.result)?;
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
        Ok(Body {
            slots: slots.iter().map(|slot| slot.value_type).collect(),
            statements: body,
        })
    }
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
        Statement::Declare { .. } | Statement::Store { .. } | Statement::Evaluate(_) => {}
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
        ExpressionKind::Integer(_) | ExpressionKind::Boolean(_) | ExpressionKind::Slot(_) => {}
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                spans(field, binder, out);
            }
        }
        ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
            spans(base, binder, out);
        }
        ExpressionKind::IsCase { subject, .. } => spans(subject, binder, out),
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
        ExpressionKind::Not(operand) | ExpressionKind::BitwiseNot(operand) => {
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
