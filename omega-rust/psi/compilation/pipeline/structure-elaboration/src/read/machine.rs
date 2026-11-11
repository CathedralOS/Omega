/*
The machine family of the reader: which declarations are admitted (the subset
gate), their signatures and formals, the call-graph recursion refusal, and
the driver that reads one body statement by statement.
*/

use semantic_vocabulary::{MachineId, ScalarType, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashPredicateTerm, CrashRouteGuard};
use typed_trees::machine::{Machine, MachineBodyStatus, MachinePurpose};
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

use super::contracts::{ClauseScope, ResultTerm};
use super::{Program, Slot, crash_cause, span};
use crate::body::{Body, CrashCeiling, Signature, Statement, Target};
use crate::canonical::{self, Repeated};
use crate::{SourceOrigin, Unserved, refuse};

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
        let signature = self.signature(id, symbol)?;
        self.symbols.push(symbol);
        self.signatures.push(signature);
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
        if !machine.lifetime_parameters.is_empty() || !machine.type_parameters.is_empty() {
            return unserved("generic parameters");
        }
        if !machine.declared_type_equations.is_empty()
            || !machine.owned_data.is_empty()
            || !machine.satisfies.is_empty()
            || !machine.conformance_bounds.is_empty()
        {
            return unserved("type equations, owned data or conformances");
        }
        if !machine.invokes.is_empty() {
            return unserved("invokes");
        }
        Ok(())
    }

    fn signature(&self, id: MachineId, symbol: SymbolHandle) -> Checked<Signature> {
        let typed = self.typed;
        let (machine, state) = self.declaration(id, symbol)?;
        self.machine_shape(id, machine)?;
        if !state.contracts.is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("contracts on a state"),
            ));
        }
        let formals = span(
            &typed.state_parameters,
            state.parameters,
            id,
            "parameter span",
        )?;
        let mut parameters = Vec::with_capacity(formals.len());
        let mut slots = Vec::with_capacity(formals.len());
        for formal in formals {
            let scalar_type = self.formal(id, formal)?;
            parameters.push(scalar_type);
            slots.push(Slot {
                symbol: formal.symbol,
                scalar_type,
                assignable: false,
            });
        }
        let result = self.result_type(id, state.return_type)?;
        // The result pseudo-value is the value right after the formals
        // (`Signature::result`), owned by the machine's one state: stage 02
        // records the entry state as the owner of a machine contract's
        // reserved `result`.
        let result_value = ValueId::new(formals.len() as u64 + 1).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("result identity"),
            )
        })?;
        let entry = ClauseScope {
            slots: &slots,
            owner: state.symbol,
            result: ResultTerm::Absent("result in an entry clause"),
        };
        let exit = ClauseScope {
            result: match result {
                Some(scalar_type) => ResultTerm::Named(result_value, scalar_type),
                None => ResultTerm::Absent("result of a Unit machine"),
            },
            ..entry
        };
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut routes = Vec::new();
        let mut crash_clauses = false;
        for contract in span(
            &typed.signature_contracts,
            machine.contracts,
            id,
            "contract span",
        )? {
            if contract.binding.is_some() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("result binding"),
                ));
            }
            let facts = span(&typed.proof_facts, contract.facts, id, "fact span")?;
            match contract.kind {
                SignatureContractKind::Requires => {
                    for fact in facts {
                        requires.push(self.clause(id, &entry, self.fact(id, fact)?)?);
                    }
                }
                SignatureContractKind::Ensures => {
                    for fact in facts {
                        ensures.push(self.clause(id, &exit, self.fact(id, fact)?)?);
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
        let crashes = match (
            machine.declares_no_crash_route,
            crash_clauses,
            machine.is_public,
        ) {
            (true, false, _) => CrashCeiling::DeclaredNone,
            // Stage 01 rejects this header; refused here too rather than
            // trusted to have been.
            (true, true, _) => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("`crashes none` beside a `crashes Cause` clause"),
                ));
            }
            (false, true, _) | (false, false, true) => {
                CrashCeiling::Authored(canonical::routes(routes))
            }
            (false, false, false) => CrashCeiling::Inferred,
        };
        let requires = canonical::clauses(requires, Repeated::Refuse, id)?;
        Ok(Signature {
            parameters,
            result,
            requires,
            ensures,
            crashes,
        })
    }

    fn formal(&self, id: MachineId, formal: &StateParameter) -> Checked<ScalarType> {
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
        self.scalar_type(id, formal.type_reference)
    }

    pub fn body(&mut self, id: MachineId) -> Checked<Body> {
        let typed = self.typed;
        let symbol = *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
        let signature =
            terminal_check_tables::row(&self.signatures, id, id, Construct::Machine)?.clone();
        let (_, state) = self.declaration(id, symbol)?;
        let formals = span(
            &typed.state_parameters,
            state.parameters,
            id,
            "parameter span",
        )?;
        let mut slots: Vec<Slot> = formals
            .iter()
            .zip(&signature.parameters)
            .map(|(formal, scalar_type)| Slot {
                symbol: formal.symbol,
                scalar_type: *scalar_type,
                assignable: false,
            })
            .collect();
        let statements = typed.statement_table.statements(state.statement_nodes);
        let mut body = Vec::with_capacity(statements.len());
        let mut completed = false;
        for (position, statement) in statements.iter().enumerate() {
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
                    let expected = if last { signature.result } else { None };
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
                            Statement::Arm {
                                guard: None,
                                source: expression.source,
                                target: Target::Value(expression),
                            }
                        }
                        (false, _) | (true, None) => Statement::Evaluate(expression),
                    }
                }
                StatementNode::LocalData(local) => {
                    let (slot, statement) = self.local(id, &slots, local)?;
                    slots.push(slot);
                    statement
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
        }
        if !completed && signature.result.is_some() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Completion("value machine can fall off its end"),
            ));
        }
        Ok(Body {
            slots: slots.iter().map(|slot| slot.scalar_type).collect(),
            statements: body,
        })
    }
}
