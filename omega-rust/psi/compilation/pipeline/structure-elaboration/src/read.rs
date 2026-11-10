/*
The reader: one machine's frozen typed trees resolved into the closed scalar
body. This is where identity is fixed (`check_on_lowered_form.md`, "Identity
is fixed before elaboration"):

- A type is scalar only through `SymbolTable::builtin_type_atom` on its
  resolved symbol: the compiler-installed root slot, never the spelling. The
  typed trees' own `PrimitiveType::from_name` classifies by spelling and is
  not used. A `Named` type whose symbol is invalid is refused as a spelling.
- A name is a slot only when its path has one member and a valid symbol equal
  to a formal's or local's symbol. A name with no symbol is refused. The
  synthetic `result` of an `ensures` clause reaches the typed trees in exactly
  that form (`typed-trees/src/values.rs`, `names_reserved_result`: invalid
  symbols, recognised by `is_reserved_result` on the spelling), so a clause
  that mentions `result` is refused here rather than matched by text.
- A callee is a `MachineId` reached from its symbol.

Every match over a typed-tree node kind lists every kind; a kind outside the
subset is refused with `Unsupported` and its name. A field the subset does not
serve is refused when it is set, never ignored.
*/

use arena::Arena;
use semantic_vocabulary::{
    IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, Proposition,
    ScalarTerm, ScalarType, ValueId,
};
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
    UnaryOperator,
};
use typed_trees::machine::{Machine, MachineBodyStatus, MachinePurpose};
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableCall, TableLocalData, TransitionExit, TransitionGuardNode,
    TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use crate::body::{Body, Expression, ExpressionKind, Signature, Statement, Strict, Target};
use crate::{SourceOrigin, Unserved, refuse};

pub(crate) struct Program<'t> {
    typed: &'t TypedTrees,
    /// The typed-tree machine of each discovered `MachineId`.
    pub symbols: IdTable<MachineId, SymbolHandle>,
    pub signatures: IdTable<MachineId, Signature>,
    /// Every direct call read so far, as (caller, callee).
    calls: Vec<(MachineId, MachineId)>,
}

/// One slot of a body: formals first, then locals in binding order.
struct Slot {
    symbol: SymbolHandle,
    scalar_type: ScalarType,
    assignable: bool,
}

/// Reads a span of a typed-tree arena, refusing an invalid span instead of
/// reading it as empty.
fn span<'a, T: Default>(
    arena: &'a Arena<T>,
    span: arena::HandleSpan<T>,
    machine: MachineId,
    what: &'static str,
) -> Checked<&'a [T]> {
    arena
        .span(span)
        .ok_or_else(|| refuse(machine, Construct::Machine, Unserved::Machine(what)))
}

fn integer(sign: IntegerSign, bits: u16, machine: MachineId) -> Checked<ScalarType> {
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| refuse(machine, Construct::Machine, Unserved::Type("integer width")))
}

/// The scalar type of a builtin atom, by its compiler-installed slot.
fn atom_type(atom: BuiltinTypeAtom, machine: MachineId) -> Checked<ScalarType> {
    match atom {
        BuiltinTypeAtom::Bool => Ok(ScalarType::Boolean),
        BuiltinTypeAtom::I8 => integer(IntegerSign::Signed, 8, machine),
        BuiltinTypeAtom::I16 => integer(IntegerSign::Signed, 16, machine),
        BuiltinTypeAtom::I32 => integer(IntegerSign::Signed, 32, machine),
        BuiltinTypeAtom::I64 => integer(IntegerSign::Signed, 64, machine),
        BuiltinTypeAtom::U8 => integer(IntegerSign::Unsigned, 8, machine),
        BuiltinTypeAtom::U16 => integer(IntegerSign::Unsigned, 16, machine),
        BuiltinTypeAtom::U32 => integer(IntegerSign::Unsigned, 32, machine),
        BuiltinTypeAtom::U64 => integer(IntegerSign::Unsigned, 64, machine),
        BuiltinTypeAtom::Address => Err(refuse(machine, Construct::Machine, Unserved::Type("addr"))),
        BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64 => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("float")))
        }
        BuiltinTypeAtom::Slice => Err(refuse(machine, Construct::Machine, Unserved::Type("slice"))),
        BuiltinTypeAtom::SyscallResult => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("syscall result"),
        )),
        BuiltinTypeAtom::Terminal | BuiltinTypeAtom::Never => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("terminal or never")))
        }
        BuiltinTypeAtom::UInt | BuiltinTypeAtom::Int => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("anonymous integer"),
        )),
        BuiltinTypeAtom::AtomicBool | BuiltinTypeAtom::AtomicU32 | BuiltinTypeAtom::AtomicU64 => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("atomic")))
        }
    }
}

/// The integer type a literal landed on, through the same width table.
fn landed_type(landed: numerics::literals::LandedIntegerType, machine: MachineId) -> Checked<ScalarType> {
    use numerics::literals::LandedIntegerType as Landed;
    match landed {
        Landed::I8 => integer(IntegerSign::Signed, 8, machine),
        Landed::I16 => integer(IntegerSign::Signed, 16, machine),
        Landed::I32 => integer(IntegerSign::Signed, 32, machine),
        Landed::I64 => integer(IntegerSign::Signed, 64, machine),
        Landed::U8 => integer(IntegerSign::Unsigned, 8, machine),
        Landed::U16 => integer(IntegerSign::Unsigned, 16, machine),
        Landed::U32 => integer(IntegerSign::Unsigned, 32, machine),
        Landed::U64 => integer(IntegerSign::Unsigned, 64, machine),
        Landed::Addr => Err(refuse(machine, Construct::Machine, Unserved::Type("addr literal"))),
    }
}

impl<'t> Program<'t> {
    pub fn new(typed: &'t TypedTrees) -> Self {
        Self {
            typed,
            symbols: IdTable::default(),
            signatures: IdTable::default(),
            calls: Vec::new(),
        }
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
        let mut ready: Vec<usize> = (0..count).filter(|machine| incoming[*machine] == 0).collect();
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
                Err(refuse(machine, Construct::Machine, Unserved::Call("recursion")))
            }
        }
    }

    fn declaration(&self, id: MachineId, symbol: SymbolHandle) -> Checked<(&'t Machine, &'t State)> {
        let typed = self.typed;
        if !symbol.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Spelling("callee without a symbol")));
        }
        let machine = typed
            .machine_by_symbol(symbol)
            .ok_or_else(|| refuse(id, Construct::Machine, Unserved::Call("target is not a machine declaration")))?;
        let states = span(&typed.machine_states, machine.states, id, "state span")?;
        let [state] = states else {
            return Err(refuse(id, Construct::Machine, Unserved::Machine("not exactly one state")));
        };
        Ok((machine, state))
    }

    fn machine_shape(&self, id: MachineId, machine: &Machine) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        if machine.lexical_region_origin.is_some() {
            return unserved("lexical region origin");
        }
        if machine.assembly_target_requirement != language_core::inline_assembly::AsmTargetApplicability::Any {
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
        if machine.target.is_some() || machine.authored_target.is_some() || machine.target_family_interface {
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
        )
            || machine.service_reach_is_installation_bound
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
            return Err(refuse(id, Construct::Machine, Unserved::Contract("contracts on a state")));
        }
        let formals = span(&typed.state_parameters, state.parameters, id, "parameter span")?;
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
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        for contract in span(&typed.signature_contracts, machine.contracts, id, "contract span")? {
            if contract.binding.is_some() {
                return Err(refuse(id, Construct::Machine, Unserved::Contract("result binding")));
            }
            let clauses = match contract.kind {
                SignatureContractKind::Requires => &mut requires,
                SignatureContractKind::Ensures => &mut ensures,
                SignatureContractKind::EnsuresForResultCase { .. } => {
                    return Err(refuse(id, Construct::Machine, Unserved::Contract("outcome-specific ensures")));
                }
                SignatureContractKind::Crashes { .. } => {
                    return Err(refuse(id, Construct::Machine, Unserved::Contract("crashes")));
                }
            };
            for fact in span(&typed.proof_facts, contract.facts, id, "fact span")? {
                match fact {
                    ProofFact::Expression(expression) => {
                        clauses.push(self.clause(id, &slots, *expression)?);
                    }
                    ProofFact::Membership(_) => {
                        return Err(refuse(id, Construct::Machine, Unserved::Contract("domain membership")));
                    }
                    ProofFact::Proposition(_) => {
                        return Err(refuse(id, Construct::Machine, Unserved::Contract("named proposition")));
                    }
                }
            }
        }
        let requires = canonical(requires, id)?;
        Ok(Signature {
            parameters,
            result,
            requires,
            ensures,
        })
    }

    fn formal(&self, id: MachineId, formal: &StateParameter) -> Checked<ScalarType> {
        if !formal.symbol.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Spelling("parameter without a symbol")));
        }
        if formal.is_const {
            return Err(refuse(id, Construct::Machine, Unserved::Parameter("const parameter")));
        }
        if formal.is_mutable {
            return Err(refuse(id, Construct::Machine, Unserved::Parameter("mutable parameter")));
        }
        if formal.relevance != language_core::BindingRelevance::Relevant {
            return Err(refuse(id, Construct::Machine, Unserved::Parameter("erased parameter")));
        }
        self.scalar_type(id, formal.type_reference)
    }

    fn scalar_type(&self, id: MachineId, handle: TypeReferenceHandle) -> Checked<ScalarType> {
        let typed = self.typed;
        if !handle.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Type("missing type")));
        }
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
        match typed.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => {
                if !symbol.is_valid() {
                    return Err(refuse(id, Construct::Machine, Unserved::Spelling("type without a symbol")));
                }
                match typed.symbols.builtin_type_atom(*symbol) {
                    Some(atom) => atom_type(atom, id),
                    None => unserved("nominal type"),
                }
            }
            TypeReferenceNode::Reference { .. } => unserved("reference"),
            TypeReferenceNode::Constrained { .. } => unserved("constrained"),
            TypeReferenceNode::FixedArray { .. } => unserved("fixed array"),
            TypeReferenceNode::Slice { .. } => unserved("slice"),
            TypeReferenceNode::Generic { .. } => unserved("generic application"),
            TypeReferenceNode::ConstExpression(_) => unserved("const expression"),
            TypeReferenceNode::DynamicTrait { .. } => unserved("dynamic trait"),
            TypeReferenceNode::Unit => unserved("unit as a value"),
        }
    }

    /// A state's result: the invalid handle and `Unit` are both a Unit
    /// machine, because that is how the typed trees record "no result".
    fn result_type(&self, id: MachineId, handle: TypeReferenceHandle) -> Checked<Option<ScalarType>> {
        if !handle.is_valid() {
            return Ok(None);
        }
        // Every other node kind goes through `scalar_type`'s exhaustive match.
        if matches!(
            self.typed.type_reference_table.type_reference(handle),
            TypeReferenceNode::Unit
        ) {
            return Ok(None);
        }
        self.scalar_type(id, handle).map(Some)
    }

    // Contract clauses: a comparison of formals, literals and Booleans, or a
    // conjunction of those. Terms name formal `i` as value `i + 1`.

    fn clause(&self, id: MachineId, slots: &[Slot], expression: ExpressionHandle) -> Checked<Proposition> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing clause expression");
        }
        let ExpressionNode::Binary(binary) = typed.expression_table.expression(expression) else {
            return unserved("clause that is not a comparison or conjunction");
        };
        let operator = binary.operator;
        let comparison = match operator {
            BinaryOperator::And => {
                let mut conjuncts = Vec::new();
                for side in [binary.left, binary.right] {
                    match self.clause(id, slots, side)? {
                        Proposition::Conjunction(inner) => conjuncts.extend(inner),
                        other => conjuncts.push(other),
                    }
                }
                return canonical(conjuncts, id).map(Proposition::Conjunction);
            }
            BinaryOperator::Equal
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => operator,
            BinaryOperator::NotEqual => return unserved("inequality (no negated proposition)"),
            BinaryOperator::Or => return unserved("disjunction"),
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => return unserved("arithmetic clause"),
            BinaryOperator::CaseMembership => return unserved("case membership"),
        };
        // An anonymous literal lands in its partner's type.
        let ((left, left_type), (right, right_type)) = if self.anonymous_literal(binary.left) {
            let right = self.term(id, slots, binary.right, None)?;
            (self.term(id, slots, binary.left, Some(right.1))?, right)
        } else {
            let left = self.term(id, slots, binary.left, None)?;
            let right = self.term(id, slots, binary.right, Some(left.1))?;
            (left, right)
        };
        if left_type != right_type {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("clause operand types differ")));
        }
        let ordered = matches!(left_type, ScalarType::Integer(_));
        match comparison {
            BinaryOperator::Equal => {
                let key = |term: &ScalarTerm| {
                    terminal_codec::canonical_scalar_term_order_key(term).map_err(|_| {
                        refuse(id, Construct::Machine, Unserved::Contract("unencodable term"))
                    })
                };
                if key(&left)? <= key(&right)? {
                    Ok(Proposition::Equal(left, right))
                } else {
                    Ok(Proposition::Equal(right, left))
                }
            }
            _ if !ordered => unserved("ordering of Booleans"),
            BinaryOperator::Less => Ok(Proposition::LessThan(left, right)),
            BinaryOperator::LessOrEqual => Ok(Proposition::LessOrEqual(left, right)),
            BinaryOperator::Greater => Ok(Proposition::LessThan(right, left)),
            BinaryOperator::GreaterOrEqual => Ok(Proposition::LessOrEqual(right, left)),
            _ => unserved("comparison"),
        }
    }

    /// An integer literal that stage 04 left without a landed type: it lands
    /// in the type its context expects (`wiki/spec/language/domains.md`,
    /// "Literals and expected types").
    fn anonymous_literal(&self, expression: ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                ExpressionNode::Integer(literal) if literal.landing().is_none()
            )
    }

    fn term(
        &self,
        id: MachineId,
        slots: &[Slot],
        expression: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<(ScalarTerm, ScalarType)> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing term");
        }
        match typed.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                let row = &slots[slot];
                let value = ValueId::new(slot as u64 + 1)
                    .ok_or_else(|| refuse(id, Construct::Machine, Unserved::Contract("formal identity")))?;
                Ok((
                    ScalarTerm::Value {
                        id: value,
                        scalar_type: row.scalar_type,
                    },
                    row.scalar_type,
                ))
            }
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                Ok((
                    ScalarTerm::Integer {
                        scalar_type: integer,
                        value,
                    },
                    ScalarType::Integer(integer),
                ))
            }
            ExpressionNode::Boolean(value) => Ok((ScalarTerm::Boolean(*value), ScalarType::Boolean)),
            _ => unserved("term other than a formal or literal"),
        }
    }

    /// An integer literal's value and type: the type stage 04 landed it in,
    /// or else the type its context expects. Both, when present, must agree.
    fn literal(
        &self,
        id: MachineId,
        literal: &numerics::literals::IntegerLiteral,
        expected: Option<ScalarType>,
    ) -> Checked<(IntegerValue, IntegerType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let landed = match literal.landing() {
            Some(landing) => {
                if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                    return unserved("integer literal in a non-exact domain");
                }
                Some(landed_type(landing.landed_type, id)?)
            }
            None => None,
        };
        let scalar_type = match (landed, expected) {
            (Some(landed), Some(expected)) if landed != expected => {
                return Err(refuse(id, Construct::Machine, Unserved::Typing("literal landed type")));
            }
            (Some(scalar_type), _) | (None, Some(scalar_type)) => scalar_type,
            (None, None) => return unserved("anonymous integer literal without an expected type"),
        };
        let ScalarType::Integer(integer) = scalar_type else {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("integer literal in a non-integer place")));
        };
        let value = match (integer.sign(), literal.is_negative()) {
            (IntegerSign::Signed, _) => literal.value_i64().map(|value| IntegerValue::Signed(i128::from(value))),
            (IntegerSign::Unsigned, false) => literal.value_u64().map(|value| IntegerValue::Unsigned(u128::from(value))),
            (IntegerSign::Unsigned, true) => None,
        };
        match value {
            Some(value) if integer.admits(value) => Ok((value, integer)),
            Some(_) | None => unserved("integer literal outside its type"),
        }
    }

    fn slot_of(&self, id: MachineId, slots: &[Slot], path: &TableNamePath) -> Checked<usize> {
        let typed = self.typed;
        if !path.symbol.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Spelling("name without a symbol")));
        }
        if typed.expression_table.name_path_members(path.members).len() != 1 {
            return Err(refuse(id, Construct::Machine, Unserved::Expression("projected name path")));
        }
        // ponytail: linear over the body's slots; bodies in the subset hold a
        // handful of locals.
        slots
            .iter()
            .rposition(|slot| slot.symbol == path.symbol)
            .ok_or_else(|| {
                let kind = typed.symbols.get(path.symbol).kind;
                refuse(id, Construct::Machine, Unserved::Expression(symbol_kind(kind)))
            })
    }

    pub fn body(&mut self, id: MachineId) -> Checked<Body> {
        let typed = self.typed;
        let symbol = *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
        let signature = terminal_check_tables::row(&self.signatures, id, id, Construct::Machine)?.clone();
        let (_, state) = self.declaration(id, symbol)?;
        let formals = span(&typed.state_parameters, state.parameters, id, "parameter span")?;
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
                return Err(refuse(id, Construct::Machine, Unserved::Completion("statement after an unconditional arm")));
            }
            let last = position + 1 == statements.len();
            let read = match statement {
                StatementNode::Assembly(_) => {
                    return Err(refuse(id, Construct::Machine, Unserved::Statement("assembly")));
                }
                StatementNode::AssemblyFact(_) => {
                    return Err(refuse(id, Construct::Machine, Unserved::Statement("assembly fact")));
                }
                StatementNode::Assignment(assignment) => {
                    let ExpressionNode::Name(path) = self.node(id, assignment.target)? else {
                        return Err(refuse(id, Construct::Machine, Unserved::Statement("assignment to a projected place")));
                    };
                    let slot = self.slot_of(id, &slots, path)?;
                    if !slots[slot].assignable {
                        return Err(refuse(id, Construct::Machine, Unserved::Statement("assignment to an immutable binding")));
                    }
                    let value = self.expression(id, &slots, assignment.value, Some(slots[slot].scalar_type))?;
                    if value.result != Some(slots[slot].scalar_type) {
                        return Err(refuse(id, Construct::Machine, Unserved::Typing("assigned value type")));
                    }
                    Statement::Bind { slot, value }
                }
                StatementNode::Call(call) => Statement::Evaluate(self.call_statement(id, &slots, call)?),
                StatementNode::Expression(expression) => {
                    let expected = if last { signature.result } else { None };
                    let expression = self.expression(id, &slots, *expression, expected)?;
                    match (last, signature.result) {
                        (true, Some(result)) => {
                            if expression.result != Some(result) {
                                return Err(refuse(id, Construct::Machine, Unserved::Typing("final value type")));
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
                    if !self.typed.statement_table.transition_patterns(transition.patterns).is_empty() {
                        return Err(refuse(id, Construct::Machine, Unserved::Transition("pattern")));
                    }
                    if transition.continuation.is_valid() {
                        return Err(refuse(id, Construct::Machine, Unserved::Transition("continuation")));
                    }
                    if !transition.proof_selectors.is_empty() {
                        return Err(refuse(id, Construct::Machine, Unserved::Transition("proof selectors")));
                    }
                    match transition.exit {
                        TransitionExit::Ordinary => {}
                        TransitionExit::Crash(_) => {
                            return Err(refuse(id, Construct::Machine, Unserved::Transition("crash")));
                        }
                    }
                    let guard = match transition.guard {
                        TransitionGuardNode::Always => None,
                        TransitionGuardNode::When(guard) => {
                            let guard = self.expression(id, &slots, guard, Some(ScalarType::Boolean))?;
                            if guard.result != Some(ScalarType::Boolean) {
                                return Err(refuse(id, Construct::Machine, Unserved::Typing("guard is not bool")));
                            }
                            Some(guard)
                        }
                    };
                    let target = self.target(id, &slots, transition.target, signature.result)?;
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
            return Err(refuse(id, Construct::Machine, Unserved::Completion("value machine can fall off its end")));
        }
        Ok(Body {
            slots: slots.iter().map(|slot| slot.scalar_type).collect(),
            statements: body,
        })
    }

    fn local(&mut self, id: MachineId, slots: &[Slot], local: &TableLocalData) -> Checked<(Slot, Statement)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Statement(what)));
        match local.role {
            language_core::LocalBindingRole::Value | language_core::LocalBindingRole::DispatchSubject => {}
            language_core::LocalBindingRole::AtomicObservedResult => return unserved("atomic observed result"),
            language_core::LocalBindingRole::RecordPattern => return unserved("record pattern binding"),
            language_core::LocalBindingRole::ArmPattern => return unserved("arm pattern binding"),
            language_core::LocalBindingRole::DispatchObservation => return unserved("dispatch observation"),
        }
        if !local.symbol.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Spelling("local without a symbol")));
        }
        if local.is_operand_capture || local.reflected_query_origin.is_some() {
            return unserved("operand capture or reflected query");
        }
        if local.relevance != language_core::BindingRelevance::Relevant {
            return unserved("erased local");
        }
        if !local.initial_value.is_valid() {
            return unserved("uninitialized local");
        }
        // A declared type is the initializer's expected type; without one the
        // local takes the initializer's type.
        let declared = if local.type_reference.is_valid() {
            Some(self.scalar_type(id, local.type_reference)?)
        } else {
            None
        };
        let value = self.expression(id, slots, local.initial_value, declared)?;
        let Some(value_type) = value.result else {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("local bound to a Unit call")));
        };
        if declared.is_some_and(|declared| declared != value_type) {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("local initializer type")));
        }
        let slot = Slot {
            symbol: local.symbol,
            scalar_type: value_type,
            assignable: local.is_mutable,
        };
        Ok((slot, Statement::Bind { slot: slots.len(), value }))
    }

    fn target(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: typed_trees::statement::TransitionTargetHandle,
        result: Option<ScalarType>,
    ) -> Checked<Target> {
        let typed = self.typed;
        if !handle.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Transition("missing target")));
        }
        let target = match typed.statement_table.transition_target(handle) {
            TransitionTargetNode::Terminal => {
                if result.is_some() {
                    return Err(refuse(id, Construct::Machine, Unserved::Typing("Unit arm in a value machine")));
                }
                return Ok(Target::Unit);
            }
            TransitionTargetNode::Value(expression) => self.expression(id, slots, *expression, result)?,
            TransitionTargetNode::Named {
                operational_acknowledgement,
                path,
                static_machine_parameter,
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
            } => {
                if operational_acknowledgement.acknowledges_suspend || operational_acknowledgement.acknowledges_block {
                    return Err(refuse(id, Construct::Machine, Unserved::Call("suspend or block acknowledgement")));
                }
                if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                    return Err(refuse(id, Construct::Machine, Unserved::Call("static or evidence call")));
                }
                self.direct_selection(id, path.symbol, *authored_call_selection, None)?;
                if typed.statement_table.name_path_members(path.members).len() != 1 || path.receiver_symbol.is_valid() {
                    return Err(refuse(id, Construct::Machine, Unserved::Call("qualified or receiver call")));
                }
                let arguments = typed.statement_table.expression_handles(*arguments);
                self.call(id, slots, path.symbol, arguments, SourceOrigin::of(*source_span))?
            }
        };
        if target.result != result {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("arm value type")));
        }
        Ok(Target::Value(target))
    }

    fn node(&self, id: MachineId, expression: ExpressionHandle) -> Checked<&'t ExpressionNode> {
        if !expression.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Expression("missing expression")));
        }
        Ok(self.typed.expression_table.expression(expression))
    }

    /// An authored static machine application that applies nothing: no
    /// static or selected arguments, bindings, lifetimes, inputs, conformance
    /// or contract source. Stage 03 records one for every authored call; one
    /// that applies anything is a generic or static call and is refused.
    fn applies_nothing(
        &self,
        id: MachineId,
        target: SymbolHandle,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if !machine_arguments.is_empty() {
            return unserved("static machine arguments");
        }
        if !application.is_valid() {
            return Ok(());
        }
        let application = self.typed.expression_table.static_machine_application(application);
        if !application.arguments.is_empty() || !application.selected_machine_arguments.is_empty() {
            return unserved("static application arguments");
        }
        if !application.input_selections.is_empty() {
            return unserved("static application input selections");
        }
        if !application.bindings.is_empty() {
            return unserved("static application bindings");
        }
        if !application.lifetime_bindings.is_empty() {
            return unserved("static application lifetime bindings");
        }
        if application.conformance_selection.is_some() {
            return unserved("static application conformance");
        }
        if application.callable_contract_source.is_valid() {
            return unserved("static application contract source");
        }
        // The registered operation of an ordinary call is the callee: its
        // machine symbol or its one state's symbol (stage 04 records the
        // entry state). Any other declaration is a static or captured call.
        let registration = application.registration_operation;
        if registration.is_valid() && registration != target {
            let state = self
                .typed
                .machine_by_symbol(target)
                .and_then(|machine| self.typed.machine_states.span(machine.states))
                .and_then(|states| match states {
                    [state] => Some(state.symbol),
                    _ => None,
                });
            if state != Some(registration) {
                return unserved("static application registers another declaration");
            }
        }
        Ok(())
    }

    /// A call is direct only when its authored selection, if recorded,
    /// resolved to exactly the called machine, and its call binding, if
    /// recorded, neither rebinds it nor reaches it through another
    /// declaration. All comparisons are of symbols.
    fn direct_selection(
        &self,
        id: MachineId,
        target: SymbolHandle,
        selection: Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
        binding: Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
    ) -> Checked<()> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if let Some(occurrence) = selection {
            let Some(selected) = self.typed.authored_declaration_selections().get(occurrence) else {
                return unserved("authored selection missing from its ledger");
            };
            match selected.target() {
                AuthoredDeclarationSelectionTarget::Resolved(resolved) if resolved.selected_symbol() == target => {}
                AuthoredDeclarationSelectionTarget::Resolved(_) => return unserved("selection resolved to another declaration"),
                AuthoredDeclarationSelectionTarget::Intrinsic(_) => return unserved("intrinsic selection"),
                AuthoredDeclarationSelectionTarget::LateBound(_) => return unserved("late-bound selection"),
            }
        }
        match binding {
            None => Ok(()),
            Some(binding) if binding.selected == target && binding.original == target => Ok(()),
            Some(_) => unserved("rebound call"),
        }
    }

    fn call_statement(&mut self, id: MachineId, slots: &[Slot], call: &TableCall) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.installed_interrupt_operation.is_some()
            || call.static_requirement_dispatch.is_some()
            || call.reflected_field_origin.is_some()
        {
            return unserved("interrupt, requirement dispatch or reflected call");
        }
        self.applies_nothing(id, call.target_symbol, call.authored_machine_application, &call.machine_arguments)?;
        self.direct_selection(id, call.target_symbol, call.authored_call_selection, None)?;
        if call.receiver_root_symbol.is_valid() || call.receiver_symbol.is_valid() || !call.receiver.is_empty() {
            return unserved("receiver call");
        }
        if call.static_machine_parameter.is_valid() || !call.evidence_arguments.is_empty() {
            return unserved("machine parameter or evidence");
        }
        if call.operational_acknowledgement.acknowledges_suspend || call.operational_acknowledgement.acknowledges_block {
            return unserved("suspend or block acknowledgement");
        }
        let arguments = self.typed.statement_table.expression_handles(call.arguments);
        self.call(id, slots, call.target_symbol, arguments, SourceOrigin::of(call.source_span))
    }

    fn call_expression(&mut self, id: MachineId, slots: &[Slot], call: &TableCallExpression, source: SourceOrigin) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Call(what)));
        if call.installed_interrupt_operation.is_some() {
            return unserved("installed interrupt operation");
        }
        self.applies_nothing(id, call.target_symbol, call.authored_machine_application, &call.machine_arguments)?;
        if call.static_requirement_dispatch.is_some() {
            return unserved("static requirement dispatch");
        }
        if call.build_selection.is_valid() {
            return unserved("Build selection");
        }
        self.direct_selection(id, call.target_symbol, call.authored_call_selection, call.authored_call_binding)?;
        if call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return unserved("compiler operation call");
        }
        if call.receiver.is_valid()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
        {
            return unserved("receiver, requirement or coercion call");
        }
        if call.static_machine_parameter.is_valid() || !call.evidence_arguments.is_empty() {
            return unserved("machine parameter or evidence");
        }
        if call.operational_acknowledgement.acknowledges_suspend || call.operational_acknowledgement.acknowledges_block {
            return unserved("suspend or block acknowledgement");
        }
        let arguments = self.typed.expression_table.expression_handles(call.arguments);
        self.call(id, slots, call.target_symbol, arguments, source)
    }

    fn call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
    ) -> Checked<Expression> {
        if !target.is_valid() {
            return Err(refuse(id, Construct::Machine, Unserved::Spelling("callee without a symbol")));
        }
        // A call names its callee's entry state; the machine is that state's
        // symbol parent, and must have exactly that one state.
        let machine = match self.typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => {
                let parent = self.typed.symbols.get(target).parent;
                let (_, state) = self.declaration(id, parent)?;
                if state.symbol != target {
                    return Err(refuse(id, Construct::Machine, Unserved::Call("state other than the entry")));
                }
                parent
            }
            other => return Err(refuse(id, Construct::Machine, Unserved::Call(symbol_kind(other)))),
        };
        // The callee's formals are the arguments' expected types, so its
        // signature is read first; reading order is not evaluation order.
        let callee = self.machine(machine)?;
        self.calls.push((id, callee));
        let formals = terminal_check_tables::row(&self.signatures, callee, id, Construct::Machine)?
            .parameters
            .clone();
        if formals.len() != arguments.len() {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("argument count")));
        }
        let mut read = Vec::with_capacity(arguments.len());
        for (argument, formal) in arguments.iter().zip(&formals) {
            read.push(self.expression(id, slots, *argument, Some(*formal))?);
        }
        let signature = terminal_check_tables::row(&self.signatures, callee, id, Construct::Machine)?;
        if signature.parameters.len() != read.len()
            || signature
                .parameters
                .iter()
                .zip(&read)
                .any(|(formal, actual)| actual.result != Some(*formal))
        {
            return Err(refuse(id, Construct::Machine, Unserved::Typing("argument types")));
        }
        Ok(Expression {
            kind: ExpressionKind::Call {
                callee,
                arguments: read,
            },
            result: signature.result,
            source,
        })
    }

    /// Reads one expression. `expected` is the type its context expects; only
    /// an anonymous integer literal uses it, and every other node's type is
    /// checked against its context by the caller.
    fn expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<Expression> {
        let typed = self.typed;
        let node = self.node(id, handle)?;
        let source = SourceOrigin::of(typed.expression_table.source_span(handle));
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let (kind, result) = match node {
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => (ExpressionKind::Boolean(*value), ScalarType::Boolean),
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                (ExpressionKind::Slot(slot), slots[slot].scalar_type)
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(id, slots, unary.operand, expected)?;
                match (unary.operator, operand.result) {
                    (UnaryOperator::LogicalNot, Some(ScalarType::Boolean)) => {
                        (ExpressionKind::Not(Box::new(operand)), ScalarType::Boolean)
                    }
                    (UnaryOperator::BitwiseNot, Some(integer @ ScalarType::Integer(_))) => {
                        (ExpressionKind::BitwiseNot(Box::new(operand)), integer)
                    }
                    _ => return Err(refuse(id, Construct::Machine, Unserved::Typing("unary operand type"))),
                }
            }
            ExpressionNode::Binary(binary) => {
                // What an operand expects from outside: an arithmetic or shift
                // result has its (value) operand's type, `&&`/`||` operands
                // are Boolean, comparison operands expect nothing. An
                // anonymous literal operand lands in its partner's type, so
                // its partner is read first; reading order is not evaluation
                // order, which lowering takes from the operand positions.
                let hint = match binary.operator {
                    BinaryOperator::And | BinaryOperator::Or => Some(ScalarType::Boolean),
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => expected,
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::CaseMembership => None,
                };
                let (left, right) = if self.anonymous_literal(binary.left) {
                    let right = self.expression(id, slots, binary.right, hint)?;
                    let left = self.expression(id, slots, binary.left, hint.or(right.result))?;
                    (left, right)
                } else {
                    let left = self.expression(id, slots, binary.left, hint)?;
                    let right = self.expression(id, slots, binary.right, hint.or(left.result))?;
                    (left, right)
                };
                return binary_expression(id, binary.operator, left, right, source);
            }
            ExpressionNode::Call(call) => return self.call_expression(id, slots, call, source),
            ExpressionNode::TypeExpression(_) => return unserved("type expression"),
            ExpressionNode::Match(_) => return unserved("match"),
            ExpressionNode::SavedSubject(_) => return unserved("saved subject"),
            ExpressionNode::ArrayLiteral(_) => return unserved("array literal"),
            ExpressionNode::Atomic(_) => return unserved("atomic"),
            ExpressionNode::Cast(_) => return unserved("cast"),
            ExpressionNode::Float(_) => return unserved("float literal"),
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(_) => return unserved("index"),
            ExpressionNode::Member(_) => return unserved("member"),
            ExpressionNode::Borrow(_) => return unserved("borrow"),
            ExpressionNode::Range(_) => return unserved("range"),
            ExpressionNode::StructLiteral(_) => return unserved("record literal"),
            ExpressionNode::String(_) => return unserved("string literal"),
            ExpressionNode::ZeroValue(_) => return unserved("zero value"),
        };
        Ok(Expression {
            kind,
            result: Some(result),
            source,
        })
    }
}

/// Builtin operator selection: which operation a binary operator names, and
/// its operand and result types. Builtin meaning is all the subset has: an
/// operator bound to a declared machine reaches the typed trees as a call.
fn binary_expression(
    id: MachineId,
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
    source: SourceOrigin,
) -> Checked<Expression> {
    enum Operands {
        /// Same integer type; the result has it.
        Arithmetic,
        /// Integer value and integer count, each its own type; the result
        /// has the value's type.
        Shift,
        /// Same integer or Boolean type; the result is Boolean.
        Equality,
        /// Same integer type; the result is Boolean.
        Ordering,
    }
    let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
    let (Some(left_type), Some(right_type)) = (left.result, right.result) else {
        return Err(typing("Unit operand"));
    };
    let (operation, operands) = match operator {
        BinaryOperator::And | BinaryOperator::Or => {
            if left_type != ScalarType::Boolean || right_type != ScalarType::Boolean {
                return Err(typing("short-circuit operand is not bool"));
            }
            return Ok(Expression {
                kind: ExpressionKind::ShortCircuit {
                    conjunction: operator == BinaryOperator::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                result: Some(ScalarType::Boolean),
                source,
            });
        }
        BinaryOperator::CaseMembership => {
            return Err(refuse(id, Construct::Machine, Unserved::Expression("case membership")));
        }
        BinaryOperator::Add => (Strict::Add, Operands::Arithmetic),
        BinaryOperator::Subtract => (Strict::Subtract, Operands::Arithmetic),
        BinaryOperator::Multiply => (Strict::Multiply, Operands::Arithmetic),
        BinaryOperator::Divide => (Strict::Divide, Operands::Arithmetic),
        BinaryOperator::Modulo => (Strict::Remainder, Operands::Arithmetic),
        BinaryOperator::BitwiseAnd => (Strict::BitwiseAnd, Operands::Arithmetic),
        BinaryOperator::BitwiseOr => (Strict::BitwiseOr, Operands::Arithmetic),
        BinaryOperator::BitwiseXor => (Strict::BitwiseXor, Operands::Arithmetic),
        BinaryOperator::ShiftLeft => (Strict::ShiftLeft, Operands::Shift),
        BinaryOperator::ShiftRight => (Strict::ShiftRight, Operands::Shift),
        BinaryOperator::Equal => (Strict::Equal, Operands::Equality),
        BinaryOperator::NotEqual => (Strict::NotEqual, Operands::Equality),
        BinaryOperator::Less => (Strict::Less, Operands::Ordering),
        BinaryOperator::LessOrEqual => (Strict::LessOrEqual, Operands::Ordering),
        BinaryOperator::Greater => (Strict::Greater, Operands::Ordering),
        BinaryOperator::GreaterOrEqual => (Strict::GreaterOrEqual, Operands::Ordering),
    };
    let integer = |scalar_type| matches!(scalar_type, ScalarType::Integer(_));
    let result = match operands {
        Operands::Arithmetic if integer(left_type) && left_type == right_type => left_type,
        Operands::Shift if integer(left_type) && integer(right_type) => left_type,
        Operands::Equality
            if left_type == right_type && (integer(left_type) || left_type == ScalarType::Boolean) =>
        {
            ScalarType::Boolean
        }
        Operands::Ordering if integer(left_type) && left_type == right_type => ScalarType::Boolean,
        Operands::Arithmetic | Operands::Shift | Operands::Equality | Operands::Ordering => {
            return Err(typing("operand types"));
        }
    };
    Ok(Expression {
        kind: ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        result: Some(result),
        source,
    })
}

/// Sorts propositions into Terminal Psi's canonical order and refuses a
/// duplicate, which canonical order cannot hold ("strictly increase").
fn canonical(propositions: Vec<Proposition>, id: MachineId) -> Checked<Vec<Proposition>> {
    let mut keyed = Vec::with_capacity(propositions.len());
    for proposition in propositions {
        let key = terminal_codec::canonical_proposition_order_key(&proposition)
            .map_err(|_| refuse(id, Construct::Machine, Unserved::Contract("unencodable proposition")))?;
        keyed.push((key, proposition));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(refuse(id, Construct::Machine, Unserved::Contract("duplicate clause")));
    }
    Ok(keyed.into_iter().map(|(_, proposition)| proposition).collect())
}

/// The refusal label for a name or callee that is not a slot or machine.
fn symbol_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Unknown => "unknown symbol",
        SymbolKind::Root => "root",
        SymbolKind::Module => "module",
        SymbolKind::BuiltinType => "builtin type",
        SymbolKind::BuiltinFunction => "builtin function",
        SymbolKind::Data => "data",
        SymbolKind::Domain => "domain",
        SymbolKind::Field => "field",
        SymbolKind::Variant => "variant",
        SymbolKind::Machine => "machine",
        SymbolKind::Operator => "operator",
        SymbolKind::Measure => "measure",
        SymbolKind::Proposition => "proposition",
        SymbolKind::MathematicalDefinition => "mathematical definition",
        SymbolKind::State => "state",
        SymbolKind::Trait => "trait",
        SymbolKind::Conformance => "conformance",
        SymbolKind::Const => "const",
        SymbolKind::ConformanceParameter => "conformance parameter",
        SymbolKind::WireSchema => "wire schema",
        SymbolKind::Parameter => "parameter of another machine",
        SymbolKind::TypeParameter => "type parameter",
        SymbolKind::MachineParameter => "machine parameter",
        SymbolKind::PropositionParameter => "proposition parameter",
        SymbolKind::PropositionMachineParameter => "proposition machine parameter",
        SymbolKind::Local => "local of another scope",
        SymbolKind::HostCapability => "host capability",
        SymbolKind::Object => "object",
        SymbolKind::Function => "function",
        SymbolKind::Section => "section",
        SymbolKind::Import => "import",
        SymbolKind::TypeAlias => "type alias",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_core::ReferenceAccess;
    use numerics::literals::{FloatFormat, IntegerLiteral};
    use symbols::{SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};
    use typed_trees::expression::{
        TableBinaryExpression, TableBorrowExpression, TableIndexedExpression,
        TableRangeExpression,
    };

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    fn refused(typed: &TypedTrees, node: ExpressionNode) -> String {
        let mut typed = typed.clone();
        let handle = typed.expression_table.insert(node);
        let mut program = Program::new(&typed);
        let refusal = program.expression(machine(), &[], handle, None).unwrap_err();
        assert_eq!(refusal.reason, terminal_check_tables::RefusalReason::Unsupported);
        refusal.detail
    }

    #[test]
    fn each_expression_kind_outside_the_subset_is_refused_by_name() {
        let typed = TypedTrees::default();
        let invalid = ExpressionHandle::invalid();
        let cases = [
            (
                ExpressionNode::Indexed(TableIndexedExpression {
                    collection: invalid,
                    index: invalid,
                }),
                "expression index",
            ),
            (
                ExpressionNode::Borrow(TableBorrowExpression {
                    target: invalid,
                    access: ReferenceAccess::Shared,
                }),
                "expression borrow",
            ),
            (
                ExpressionNode::Range(TableRangeExpression {
                    start: invalid,
                    end: invalid,
                    end_inclusive: false,
                }),
                "expression range",
            ),
            (ExpressionNode::ArrayLiteral(arena::HandleSpan::empty()), "expression array literal"),
            (ExpressionNode::ZeroValue(TypeReferenceHandle::invalid()), "expression zero value"),
            (ExpressionNode::TypeExpression(TypeReferenceHandle::invalid()), "expression type expression"),
            (ExpressionNode::UndeterminedNaN(FloatFormat::F32), "expression undetermined NaN"),
            (
                ExpressionNode::Integer(IntegerLiteral::from_value(3)),
                "expression anonymous integer literal without an expected type",
            ),
            (ExpressionNode::Name(TableNamePath::default()), "spelling name without a symbol"),
        ];
        for (node, expected) in cases {
            assert_eq!(refused(&typed, node), expected);
        }
    }

    #[test]
    fn a_call_cycle_is_refused_as_recursion() {
        let typed = TypedTrees::default();
        let mut program = Program::new(&typed);
        for _ in 0..3 {
            program.symbols.push(SymbolHandle::invalid());
        }
        let m = |raw| MachineId::new(raw).unwrap();
        program.calls = vec![(m(1), m(2)), (m(2), m(3)), (m(1), m(3))];
        assert_eq!(program.refuse_recursion(), Ok(()));
        program.calls.push((m(3), m(2)));
        assert_eq!(program.refuse_recursion().unwrap_err().detail, "call recursion");
        program.calls = vec![(m(1), m(1))];
        assert_eq!(program.refuse_recursion().unwrap_err().machine, m(1));
    }

    #[test]
    fn case_membership_is_refused_after_its_operands_read() {
        let mut typed = TypedTrees::default();
        let left = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let right = typed.expression_table.insert(ExpressionNode::Boolean(false));
        let node = ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::CaseMembership,
            right,
        });
        assert_eq!(refused(&typed, node), "expression case membership");
    }

    /// A type is `u32` by its builtin slot. A declaration spelled `u32` is a
    /// different symbol and is refused as nominal; a `Named` type with no
    /// symbol is refused as a spelling.
    #[test]
    fn scalar_types_come_from_builtin_slots_not_spellings() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Data, SymbolNameRef::Static("u32"))]),
        );
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let u32_slot = children.handles().nth(7).unwrap();
        let impostor = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let named = |typed: &mut TypedTrees, symbol| {
            typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol,
                name: Default::default(),
            })
        };
        let builtin = named(&mut typed, u32_slot);
        let nominal = named(&mut typed, impostor);
        let unresolved = named(&mut typed, SymbolHandle::invalid());
        let reference = typed.type_reference_table.insert(TypeReferenceNode::Reference {
            referee: builtin,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
        let program = Program::new(&typed);
        assert_eq!(
            program.scalar_type(machine(), builtin),
            Ok(ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()))
        );
        let detail = |handle| program.scalar_type(machine(), handle).unwrap_err().detail;
        assert_eq!(detail(nominal), "type nominal type");
        assert_eq!(detail(unresolved), "spelling type without a symbol");
        assert_eq!(detail(reference), "type reference");
        assert_eq!(program.result_type(machine(), TypeReferenceHandle::invalid()), Ok(None));
    }
}
