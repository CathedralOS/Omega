/*
The reader: one machine's frozen typed trees resolved into the closed body. This is where identity is fixed (`check_on_lowered_form.md`, "Identity
is fixed before elaboration"):

- A type is scalar only through `SymbolTable::builtin_type_atom` on its
  resolved symbol: the compiler-installed root slot, never the spelling. The
  typed trees' own `PrimitiveType::from_name` classifies by spelling and is
  not used. A `Named` type whose symbol is invalid is refused as a spelling.
- A type is a record only through its `SymbolKind::Data` symbol and that
  symbol's declaration; a field is its declaration position, found from the
  field symbol on the literal or member (`places`).
- A name is a slot only when its path has one member and a valid symbol equal
  to a formal's or local's symbol. A name with no symbol is refused.
- The reserved `result` of a contract keeps invalid symbols, but its name
  path carries `reserved_result_of`, the contract owner stage 02 recorded
  when it decided the name is the reserved result. A term is the result
  pseudo-value only through that identity; a name spelled `result` without
  it is refused as a spelling like any other unresolved name.
- A callee is a `MachineId` reached from its symbol.

Every match over a typed-tree node kind lists every kind; a kind outside the
subset is refused with `Unsupported` and its name. A field the subset does not
serve is refused when it is set, never ignored.
*/

mod arrays;
mod authority;
mod bytes;
mod calls;
mod casts;
mod constant_values;
mod contracts;
mod expressions;
mod machine;
mod places;
mod predicates;
mod reflection;
mod regions;
mod rejections;
mod services;
mod source_records;
mod statements;
mod storage_path;
mod subjects;
mod transitions;
mod types;

use arena::Arena;
use semantic_vocabulary::{BoundaryMachineId, CrashCause, IdTable, MachineId, StructuralTypeId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::TypedTrees;

use crate::body::{RecordType, Signature, ValueType};
use crate::{Unserved, refuse};

pub(crate) struct Program<'t> {
    pub publication: terminal_codec::TerminalPublication,
    typed: &'t TypedTrees,
    /// The typed-tree machine of each discovered `MachineId`.
    pub symbols: IdTable<MachineId, SymbolHandle>,
    pub signatures: IdTable<MachineId, Signature>,
    /// A subject requirement is a declaration, never an executable machine.
    pub declaration_signatures: IdTable<semantic_vocabulary::ContractId, Signature>,
    pub declaration_symbols: IdTable<semantic_vocabulary::ContractId, SymbolHandle>,
    /// Only this root of a refinement witness exposes the member's own
    /// body contract. Every named call, including recursion, uses its route.
    implementation_body: Option<MachineId>,
    /// Exact requirement declarations use their own canonical namespace and
    /// the same formal/contract reader as body-backed machine declarations.
    pub boundary_symbols: IdTable<BoundaryMachineId, SymbolHandle>,
    pub boundary_signatures: IdTable<BoundaryMachineId, Signature>,
    boundary_services: IdTable<BoundaryMachineId, semantic_vocabulary::ServiceId>,
    /// How each requirement's invocation ends, as the typed trees state it
    /// (`Signatures::completions`).
    pub boundary_completions: IdTable<BoundaryMachineId, semantic_vocabulary::BoundaryCompletion>,
    services: IdTable<
        semantic_vocabulary::ServiceId,
        (
            typed_trees::trait_definition::AppliedTraitScope<'t>,
            Vec<semantic_vocabulary::ServiceId>,
        ),
    >,
    resolving_services: Vec<SymbolHandle>,
    /// What diagnostics call each machine's constructs, pushed with its
    /// signature and completed as its body is read.
    pub names: IdTable<MachineId, crate::MachineNames>,
    /// Whether this is a generic check form: the entry's implementation
    /// parameters stay rigid, and a call through one names the requirement.
    rigid: bool,
    /// The machines that are a requirement's contract with no body.
    pub requirements: Vec<MachineId>,
    /// Entries and selected providers keep their declared input contract.
    pub fixed_inputs: Vec<MachineId>,
    /// Every declaration in these machines' verification subject is rooted;
    /// reaching one dependency helper alone does not close its possible uses.
    pub subject_machines: Vec<MachineId>,
    /// Every call through an implementation parameter read so far.
    pub selections: Vec<crate::Selected>,
    /// What each type parameter in scope stands for while one machine's
    /// signature or body is read: a type parameter of a generic check
    /// form's entry is rigid, and a type parameter of a generic machine the
    /// entry calls is the type the call applies it at. A type that names
    /// such a parameter by its symbol is that type (`value_type`). Empty
    /// while a closed machine is read.
    scope: Vec<(SymbolHandle, ValueType)>,
    /// `scope` of each machine, pushed with its signature and restored
    /// when its body is read.
    scopes: IdTable<MachineId, Vec<(SymbolHandle, ValueType)>>,
    /// The generic machines a generic check form's entry calls at its own
    /// type parameters, each read once per tuple of type arguments as its
    /// contract with no body (`Elaboration::applications`).
    pub applications: Vec<MachineId>,
    /// The type arguments each of those was read at.
    applied: Vec<(MachineId, Vec<ValueType>)>,
    /// The types a finite family's alternative fixes the entry's type
    /// parameters to, in declaration order, when this program is that
    /// alternative's check (`Reading::Alternative`).
    pub alternative: Option<Vec<ValueType>>,
    /// The locals of the body being read whose value is decided statically
    /// by closed type equations (`static_truth`), by the local's symbol.
    static_truths: Vec<(SymbolHandle, bool)>,
    /// `Elaboration::value_parameters` and `Elaboration::value_arguments`,
    /// pushed with each machine's signature.
    pub value_parameters: IdTable<MachineId, usize>,
    pub value_arguments: IdTable<MachineId, Vec<semantic_vocabulary::ScalarTerm>>,
    /// Every record type a formal, local, result, field or literal named, in
    /// discovery order; a nested record is discovered before its owner.
    pub records: IdTable<StructuralTypeId, RecordType>,
    /// The symbols each record type was resolved from, by the same identity.
    record_sources: IdTable<StructuralTypeId, places::RecordSource>,
    /// The data declarations being resolved, outermost first: a declaration
    /// reached again while on this stack contains itself.
    resolving: Vec<(SymbolHandle, Option<typed_trees::SourceTypeApplication>)>,
    /// The declaration being read, which is the lexical owner of the
    /// lifetimes its types name when the typed trees retained none for an
    /// occurrence: the data declaration whose members are read, or the
    /// machine whose signature or body is (`types`, `value_type`).
    lifetime_owner: SymbolHandle,
    /// The region tuples `body::Regions` names, by position; the first is
    /// the empty tuple (`regions`).
    region_tuples: Vec<Vec<regions::Region>>,
    /// The text identities of `regions::Region::Fixed`, by position.
    fixed_regions: Vec<String>,
    /// What each region variable was decided as, if it was (`regions`).
    region_variables: Vec<Option<regions::Region>>,
    /// What an elided lifetime stands for where a type is being read.
    elision: regions::Elision,
    /// How many fresh lifetime parameters each machine's signature was
    /// given for its elided applications.
    elided_lifetimes: Vec<(SymbolHandle, usize)>,
    /// The type the expression being read is expected to have, handed from
    /// `expression` to the call or literal it dispatches to, which takes it
    /// before reading anything nested (`regions`: a call instantiates its
    /// callee's lifetimes from it first).
    expected: Option<ValueType>,
    /// Whether the type being read may be a byte view in a domain: true
    /// while a data field's type or a cast's result type is read, the two
    /// places such a view is served (`view_type`).
    qualified_views: bool,
    /// How many `StructuralFieldId`s the records above hold.
    fields_minted: u64,
    /// How many `StructuralCaseId`s the sums above hold.
    cases_minted: u64,
    /// The subject of each value-position `match` whose arms are being
    /// read, outermost first, by the capture identity stage 03 gave the
    /// match and every `SavedSubject` node of its arms.
    captures: Vec<(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        crate::body::Expression,
    )>,
    /// The subject's generic machines, set aside by `subject_declarations`.
    pub generic_templates: Vec<SymbolHandle>,
    /// The operations the subject's implementations select, set aside by
    /// `subject_declarations`.
    pub conformance_witnesses: Vec<crate::ConformanceWitness>,
    /// The machine each chosen provider gives each requirement it supplies,
    /// recorded by `supplied_slots`.
    pub supplied: Vec<crate::SuppliedRequirement>,
}

/// One slot of a body: formals first, then locals in binding order.
struct Slot {
    symbol: SymbolHandle,
    value_type: ValueType,
    assignable: bool,
    /// A scalar local declared without an initializer: it is a storage home
    /// written by a store, not an SSA value rebound by assignment.
    homed: bool,
    /// An `[erased]` formal or local. A contract clause and an erased
    /// position may name it; a runtime expression may not.
    erased: bool,
    /// A reference formal or its captured binding, and its access. Its
    /// `value_type` is the referent's: the slot names the caller's storage, read and (for
    /// `&mut`) written in place, never an SSA value or a root of its own.
    borrowed: Option<language_core::ReferenceAccess>,
}
/// The closed crash cause of a `crashes` clause or `crash` statement.
fn crash_cause(cause: typed_trees::signature::CrashCause) -> CrashCause {
    match cause {
        typed_trees::signature::CrashCause::Trap => CrashCause::Trap,
        typed_trees::signature::CrashCause::Abort => CrashCause::Abort,
    }
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
impl<'t> Program<'t> {
    pub fn new(typed: &'t TypedTrees, rigid: bool) -> Self {
        Self {
            publication: terminal_codec::TerminalPublication::Portable,
            typed,
            symbols: IdTable::default(),
            signatures: IdTable::default(),
            declaration_signatures: IdTable::default(),
            declaration_symbols: IdTable::default(),
            implementation_body: None,
            boundary_symbols: IdTable::default(),
            boundary_signatures: IdTable::default(),
            boundary_completions: IdTable::default(),
            boundary_services: IdTable::default(),
            services: IdTable::default(),
            resolving_services: Vec::new(),
            names: IdTable::default(),
            rigid,
            requirements: Vec::new(),
            fixed_inputs: Vec::new(),
            subject_machines: Vec::new(),
            selections: Vec::new(),
            scope: Vec::new(),
            scopes: IdTable::default(),
            applications: Vec::new(),
            applied: Vec::new(),
            alternative: None,
            static_truths: Vec::new(),
            value_parameters: IdTable::default(),
            value_arguments: IdTable::default(),
            records: IdTable::default(),
            record_sources: IdTable::default(),
            resolving: Vec::new(),
            lifetime_owner: SymbolHandle::invalid(),
            region_tuples: vec![Vec::new()],
            fixed_regions: Vec::new(),
            region_variables: Vec::new(),
            elision: regions::Elision::Declaration,
            elided_lifetimes: Vec::new(),
            expected: None,
            qualified_views: false,
            fields_minted: 0,
            cases_minted: 0,
            captures: Vec::new(),
            generic_templates: Vec::new(),
            conformance_witnesses: Vec::new(),
            supplied: Vec::new(),
        }
    }
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
    use super::contracts::{ClauseScope, MathematicalTerms, ResultTerm};
    use super::*;
    use crate::body::ExpressionKind;
    use language_core::ReferenceAccess;
    use numerics::literals::{FloatFormat, IntegerLiteral};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };
    use symbols::{SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath,
    };
    use typed_trees::expression::{
        TableBinaryExpression, TableBorrowExpression, TableRangeExpression,
    };
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    fn refused(typed: &TypedTrees, node: ExpressionNode) -> String {
        let mut typed = typed.clone();
        let handle = typed.expression_table.insert(node);
        let mut program = Program::new(&typed, false);
        let refusal = program
            .expression(machine(), &[], handle, None)
            .unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        refusal.detail
    }

    #[test]
    fn each_expression_kind_outside_the_subset_is_refused_by_name() {
        let typed = TypedTrees::default();
        let invalid = ExpressionHandle::invalid();
        let cases = [
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
            (
                ExpressionNode::ZeroValue(TypeReferenceHandle::invalid()),
                "expression zero value",
            ),
            (
                ExpressionNode::TypeExpression(TypeReferenceHandle::invalid()),
                "expression type expression",
            ),
            (
                ExpressionNode::UndeterminedNaN(FloatFormat::F32),
                "expression undetermined NaN",
            ),
            (
                ExpressionNode::Integer(IntegerLiteral::from_value(3)),
                "expression anonymous integer literal without an expected type",
            ),
            (
                ExpressionNode::Name(TableNamePath::default()),
                "spelling name without a symbol",
            ),
        ];
        let wrong: Vec<_> = cases
            .into_iter()
            .map(|(node, expected)| (refused(&typed, node), expected))
            .filter(|(found, expected)| found != expected)
            .collect();
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    fn unit_machines() -> (TypedTrees, [SymbolHandle; 2]) {
        use typed_trees::machine::{Machine, MachineBodyStatus};
        use typed_trees::state::State;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machines = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("served")),
                (SymbolKind::Machine, SymbolNameRef::Static("holder")),
            ],
        );
        let symbols = [machines.handle_at(0), machines.handle_at(1)];
        let states = symbols.map(|machine| {
            builder
                .insert_children(machine, [(SymbolKind::State, SymbolNameRef::Static("run"))])
                .handle_at(0)
        });
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        for (symbol, state) in symbols.into_iter().zip(states) {
            let states = typed.machine_states.insert_many([State {
                symbol: state,
                ..Default::default()
            }]);
            typed.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        (typed, symbols)
    }

    #[test]
    fn a_demanded_machine_holding_an_unserved_request_is_refused_by_name() {
        use typed_trees::deferred_refusals::DeferredRequest;
        let (mut typed, [served, holder]) = unit_machines();
        let state = typed.machine_states(typed.machine_by_symbol(holder).unwrap())[0].symbol;
        typed.record_deferred_refusal(
            state,
            DeferredRequest::ReceiverCallSelection,
            source::SourceSpan::default(),
        );
        let mut program = Program::new(&typed, false);
        // The twin without a row is read; nothing demanded the holder yet.
        program.machine(served).expect("a machine with no row");
        let refusal = program.machine(holder).unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        assert_eq!(
            refusal.detail,
            "declaration holding a call selected by its receiver's type, which no pass serves yet"
        );
        // The same declaration without the row is read like its twin.
        typed.deferred_refusals.clear();
        Program::new(&typed, false)
            .machine(holder)
            .expect("the holder once its request is served");
    }

    #[test]
    fn a_call_bound_to_no_declaration_is_refused_before_its_target_is_read() {
        use typed_trees::expression::TableCallExpression;
        use typed_trees::name::Identifier;
        let (typed, [served, _]) = unit_machines();
        let call = |target_symbol| {
            ExpressionNode::Call(TableCallExpression {
                authored_call_selection: None,
                authored_call_binding: None,
                build_selection: arena::Handle::invalid(),
                authored_machine_application: arena::Handle::invalid(),
                exact_coercion_target: TypeReferenceHandle::invalid(),
                requirement_application: TypeReferenceHandle::invalid(),
                receiver: ExpressionHandle::invalid(),
                target_symbol,
                static_machine_parameter: SymbolHandle::invalid(),
                target: Identifier::generated("method"),
                static_requirement_dispatch: None,
                machine_arguments: Box::new([]),
                quotient_operation: None,
                private_layout_operation: None,
                installed_interrupt_operation: None,
                layout_query: None,
                native_code_layout: None,
                arguments: Default::default(),
                evidence_arguments: Box::new([]),
                operational_acknowledgement: Default::default(),
            })
        };
        assert_eq!(
            refused(&typed, call(SymbolHandle::invalid())),
            "call without a bound target"
        );
        // A handle of no symbol in this table is as unbound as an absent one.
        assert_eq!(
            refused(&typed, call(SymbolHandle::from_parts(9_999, 1))),
            "call without a bound target"
        );
        // The twin: a bound call passes this refusal and its target is
        // read. These machines have no source file, so the later visibility
        // rule is what stops it.
        assert_eq!(
            refused(&typed, call(served)),
            "call call between declarations without a source file"
        );
    }

    #[test]
    fn case_membership_is_refused_after_its_operands_read() {
        let mut typed = TypedTrees::default();
        let left = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let right = typed
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let node = ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            left,
            operator: BinaryOperator::CaseMembership,
            right,
        });
        assert_eq!(refused(&typed, node), "expression case membership");
    }

    /// Wholly anonymous arithmetic is exact (numeric_values.md: anonymous
    /// integer-literal division "is rational division, not truncation"). An
    /// `i32` context gets `(2147483647 + 1) - 1` as the literal 2147483647,
    /// where `i32` operations would overflow. `7 / 2` is `7/2`, which no
    /// `i32` holds, so it is refused rather than truncated to 3; `(10 / 4) *
    /// 2` is 5 but lands only through an `as`; and a value outside the
    /// context's type and a remainder, which has no anonymous meaning, are
    /// refused too.
    #[test]
    fn wholly_anonymous_arithmetic_lands_exactly_or_is_refused() {
        let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let mut typed = TypedTrees::default();
        let literal = |typed: &mut TypedTrees, value| {
            typed
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
        };
        let binary = |typed: &mut TypedTrees, left, operator, right| {
            typed
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left,
                    operator,
                    right,
                }))
        };
        let (maximum, one) = (
            literal(&mut typed, i64::from(i32::MAX)),
            literal(&mut typed, 1),
        );
        let past = binary(&mut typed, maximum, BinaryOperator::Add, one);
        let exact = binary(&mut typed, past, BinaryOperator::Subtract, one);
        let (ten, four, two, seven) = (
            literal(&mut typed, 10),
            literal(&mut typed, 4),
            literal(&mut typed, 2),
            literal(&mut typed, 7),
        );
        let quarter = binary(&mut typed, ten, BinaryOperator::Divide, four);
        let cancelled = binary(&mut typed, quarter, BinaryOperator::Multiply, two);
        let half = binary(&mut typed, seven, BinaryOperator::Divide, two);
        let outside = binary(&mut typed, maximum, BinaryOperator::Add, seven);
        let remainder = binary(&mut typed, seven, BinaryOperator::Modulo, two);
        let mut program = Program::new(&typed, false);
        let read = |program: &mut Program<'_>, handle| {
            program.expression(
                machine(),
                &[],
                handle,
                Some(ValueType::Scalar(
                    i32_type,
                    crate::body::Qualification::NONE,
                )),
            )
        };
        let landed = read(&mut program, exact).unwrap();
        assert!(matches!(
            landed.kind,
            ExpressionKind::Integer(IntegerValue::Signed(value)) if value == i128::from(i32::MAX)
        ));
        assert_eq!(
            landed.result,
            Some(ValueType::Scalar(
                i32_type,
                crate::body::Qualification::NONE
            ))
        );
        for (handle, expected) in [
            (half, "expression fractional anonymous value"),
            (
                cancelled,
                "expression anonymous value with a fractional intermediate",
            ),
            (outside, "expression anonymous value outside its type"),
            (
                remainder,
                "expression anonymous operation without an exact meaning",
            ),
        ] {
            let refusal = read(&mut program, handle).unwrap_err();
            assert_eq!(
                refusal.reason,
                terminal_check_tables::RefusalReason::Unsupported
            );
            assert_eq!(refusal.detail, expected);
        }
    }

    /// `+!` is refused by name before its operands are read: no type carries
    /// `Trapping` on this route. The `%` and `|` forms are read
    /// (`compiler/tests/route_probe.rs`).
    #[test]
    fn a_trapping_operator_is_refused_before_its_operands_are_read() {
        let typed = TypedTrees::default();
        let node = ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Trapping,
            left: ExpressionHandle::invalid(),
            operator: BinaryOperator::Add,
            right: ExpressionHandle::invalid(),
        });
        assert_eq!(refused(&typed, node), "expression Trapping policy operator");
    }

    /// `result` in a contract is taken by the owner identity stage 02
    /// recorded, never by its spelling: a name spelled `result` with no
    /// identity is refused as a spelling, the result of another callable and
    /// a result in an entry clause are refused, and a body cannot name it.
    #[test]
    fn result_is_taken_by_its_owner_identity_never_by_its_spelling() {
        use typed_trees::name::Identifier;
        let mut typed = TypedTrees::default();
        let owner = SymbolHandle::from_arena_index(3);
        let other = SymbolHandle::from_arena_index(4);
        let mut members = arena::HandleSpan::empty();
        typed
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("result"));
        let mut name = |reserved_result_of| {
            typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    reserved_result_of,
                    ..TableNamePath::default()
                }))
        };
        let spelled = name(SymbolHandle::invalid());
        let mine = name(owner);
        let theirs = name(other);
        let mut program = Program::new(&typed, false);
        let u32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let result = ValueId::new(2).unwrap();
        let exit = ClauseScope {
            subject: None,
            formal_fields: false,
            mathematical: MathematicalTerms::Served,
            slots: &[],
            values: &[],
            bound: &[],
            owner,
            result: ResultTerm::Named(result, u32_type, None),
        };
        let entry = ClauseScope {
            result: ResultTerm::Absent("result in an entry clause"),
            ..exit
        };
        assert_eq!(
            program.term(machine(), &exit, mine, None),
            Ok((
                ScalarTerm::Value {
                    id: result,
                    scalar_type: u32_type
                },
                u32_type
            ))
        );
        let detail = |scope: &ClauseScope<'_>, handle| {
            program
                .term(machine(), scope, handle, None)
                .unwrap_err()
                .detail
        };
        assert_eq!(detail(&exit, theirs), "contract result of another callable");
        assert_eq!(detail(&entry, mine), "contract result in an entry clause");
        assert_eq!(detail(&exit, spelled), "spelling name without a symbol");
        assert_eq!(
            program
                .expression(machine(), &[], mine, None)
                .unwrap_err()
                .detail,
            "expression reserved result outside a contract term"
        );
    }

    /// A case is named by the declaration its path ends at. `Packet::Alpha`
    /// in value position, as a data `requires` writes `self in
    /// Packet::Alpha`, carries the sum as its head and the case as its
    /// symbol: it names the case. A path that ends at the sum itself, or at
    /// a field, names none.
    #[test]
    fn a_case_is_named_by_the_last_segment_of_its_path() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let packet = builder
            .insert_children(root, [(SymbolKind::Data, SymbolNameRef::Static("Packet"))])
            .handle_at(0);
        let members = builder.insert_children(
            packet,
            [
                (SymbolKind::Variant, SymbolNameRef::Static("Alpha")),
                (SymbolKind::Field, SymbolNameRef::Static("tag")),
            ],
        );
        let (alpha, tag) = (members.handle_at(0), members.handle_at(1));
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut path = |head_symbol, symbol| {
            typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    head_symbol,
                    symbol,
                    ..TableNamePath::default()
                }))
        };
        let qualified = path(packet, alpha);
        let by_head = path(alpha, SymbolHandle::invalid());
        let the_sum = path(packet, packet);
        let a_field = path(packet, tag);
        let program = Program::new(&typed, false);
        assert_eq!(program.named_case(qualified), Some(alpha));
        assert_eq!(program.named_case(by_head), Some(alpha));
        assert_eq!(program.named_case(the_sum), None);
        assert_eq!(program.named_case(a_field), None);
    }

    /// A `mut` formal that holds its own value may be named by an entry
    /// `requires`, where it can only mean the argument. A guarantee or a
    /// crash route guard that names it is refused: by then the name also
    /// stands for whatever the body last assigned, and which of the two
    /// the clause means is not settled. The same clause over a formal that
    /// is not `mut` names its value.
    #[test]
    fn a_mut_formal_is_named_by_an_entry_requires_and_by_no_other_clause() {
        use typed_trees::name::Identifier;
        let mut typed = TypedTrees::default();
        let formal = SymbolHandle::from_arena_index(3);
        let mut members = arena::HandleSpan::empty();
        typed
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("x"));
        let named = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                symbol: formal,
                ..TableNamePath::default()
            }));
        let program = Program::new(&typed, false);
        let u32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let value = ValueId::new(1).unwrap();
        let slot = |assignable| Slot {
            symbol: formal,
            value_type: ValueType::Scalar(u32_type, crate::body::Qualification::NONE),
            assignable,
            homed: false,
            erased: false,
            borrowed: None,
        };
        let (mutable, immutable) = ([slot(true)], [slot(false)]);
        let values = [Some(value)];
        let scope = |slots, formal_fields| ClauseScope {
            subject: None,
            formal_fields,
            mathematical: MathematicalTerms::Served,
            slots,
            values: &values,
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in an entry clause"),
        };
        let term = Ok((
            ScalarTerm::Value {
                id: value,
                scalar_type: u32_type,
            },
            u32_type,
        ));
        let read = |scope: &ClauseScope<'_>| program.term(machine(), scope, named, None);
        assert_eq!(read(&scope(&mutable, true)), term);
        assert_eq!(read(&scope(&immutable, false)), term);
        assert_eq!(
            read(&scope(&mutable, false)).unwrap_err().detail,
            "contract clause that names a mutable parameter outside requires"
        );
    }

    /// Record constructs this slice does not serve are each refused under
    /// their own name, and a member or literal the typed trees hold only as a
    /// spelling is refused as a spelling, before anything is read from it.
    #[test]
    fn each_place_expression_outside_the_slice_is_refused_by_name() {
        use typed_trees::expression::{
            TableMatchExpression, TableMemberExpression, TableStructLiteral,
        };
        let typed = TypedTrees::default();
        let member = |case_variant| {
            ExpressionNode::Member(TableMemberExpression {
                receiver: ExpressionHandle::invalid(),
                selection: typed_trees::expression::MemberSelection::Unresolved,
                member: Default::default(),
                case_variant,
            })
        };
        let cases = [
            // A payload member is the payload field its symbol names; the
            // case it spells is never read, so one with no symbol is a
            // spelling like any other member.
            (
                member(Some(Default::default())),
                "spelling member without a field symbol",
            ),
            (member(None), "spelling member without a field symbol"),
            (
                ExpressionNode::StructLiteral(TableStructLiteral {
                    case_name: Some(Default::default()),
                    ..Default::default()
                }),
                "spelling case literal without a case symbol",
            ),
            (
                ExpressionNode::StructLiteral(TableStructLiteral::default()),
                "spelling record literal without a type symbol",
            ),
            (
                ExpressionNode::Match(TableMatchExpression::default()),
                "expression missing expression",
            ),
            (
                ExpressionNode::SavedSubject(Default::default()),
                "expression saved subject",
            ),
        ];
        let wrong: Vec<_> = cases
            .into_iter()
            .map(|(node, expected)| (refused(&typed, node), expected))
            .filter(|(found, expected)| found != expected)
            .collect();
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    /// Dispatch forms outside the slice are refused by name before anything
    /// is read from them: a case arm with a guard of its own and a pattern
    /// whose head names no case. A `match` on a scalar that covers nothing
    /// is rejected.
    #[test]
    fn each_dispatch_form_outside_the_slice_is_refused_by_name_or_rejected() {
        use semantic_vocabulary::StructuralTypeId;
        use typed_trees::expression::{TableDestructurePattern, TableMatchExpression};
        let mut typed = TypedTrees::default();
        let truth = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let scalar_match =
            typed
                .expression_table
                .insert(ExpressionNode::Match(TableMatchExpression {
                    subject: truth,
                    ..Default::default()
                }));
        let mut program = Program::new(&typed, false);
        // A scalar match is read; one whose arms do not cover its subject
        // is the program's error, not a form the reader cannot judge.
        let uncovered = program
            .expression(machine(), &[], scalar_match, None)
            .unwrap_err();
        assert!(matches!(
            uncovered.reason,
            terminal_check_tables::RefusalReason::Violation(_)
        ));
        assert!(uncovered.detail.contains("does not cover every value"));
        let record = StructuralTypeId::new(1).unwrap();
        let pattern = |guard, head| TableDestructurePattern {
            guard,
            head,
            ..Default::default()
        };
        let detail = |pattern: TableDestructurePattern| {
            let refusal = program
                .case_pattern(
                    machine(),
                    record,
                    &pattern,
                    crate::SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                )
                .unwrap_err();
            assert_eq!(
                refusal.reason,
                terminal_check_tables::RefusalReason::Unsupported
            );
            refusal.detail
        };
        assert_eq!(
            detail(pattern(truth, ExpressionHandle::invalid())),
            "transition case arm with a guard"
        );
        assert_eq!(
            detail(pattern(ExpressionHandle::invalid(), truth)),
            "transition pattern head that is not a case"
        );
    }

    /// A store names a field by its symbol and a declaration without an
    /// initializer names its type; what the typed trees hold only as a
    /// spelling, a value of another type, and a destructuring binding are
    /// each refused by name, as is a named constant.
    #[test]
    fn each_place_statement_outside_the_slice_is_refused_by_name() {
        use semantic_vocabulary::StructuralTypeId;
        use typed_trees::expression::TableMemberExpression;
        use typed_trees::name::Identifier;
        use typed_trees::statement::{TableAssignment, TableLocalData};
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Local, SymbolNameRef::Static("p")),
                (SymbolKind::Const, SymbolNameRef::Static("LIMIT")),
            ],
        );
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let local = children.handles().next().unwrap();
        let constant = children.handles().nth(1).unwrap();
        let name = |typed: &mut TypedTrees, symbol, spelled| {
            let mut members = arena::HandleSpan::empty();
            typed
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated(spelled));
            typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    symbol,
                    ..TableNamePath::default()
                }))
        };
        let whole = name(&mut typed, local, "p");
        let named_constant = name(&mut typed, constant, "LIMIT");
        let projected =
            typed
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: whole,
                    selection: typed_trees::expression::MemberSelection::Unresolved,
                    member: Default::default(),
                    case_variant: None,
                }));
        let value = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let slots = [Slot {
            symbol: local,
            value_type: ValueType::record(StructuralTypeId::new(1).unwrap()),
            assignable: true,
            homed: false,
            erased: false,
            borrowed: None,
        }];
        let mut program = Program::new(&typed, false);
        let mut assigned = |target| {
            program
                .assignment(machine(), &slots, &TableAssignment { target, value })
                .unwrap_err()
                .detail
        };
        assert_eq!(
            assigned(projected),
            "spelling member without a field symbol"
        );
        assert_eq!(assigned(whole), "typing assigned value type");
        assert_eq!(assigned(value), "statement assignment to a projected place");
        // A binding that is not `mut` is neither replaced nor stored into.
        let frozen = [Slot {
            symbol: local,
            value_type: ValueType::record(StructuralTypeId::new(1).unwrap()),
            assignable: false,
            homed: false,
            erased: false,
            borrowed: None,
        }];
        assert_eq!(
            program
                .assignment(
                    machine(),
                    &frozen,
                    &TableAssignment {
                        target: whole,
                        value,
                    },
                )
                .unwrap_err()
                .detail,
            "statement assignment to an immutable binding"
        );
        let mut declared = |role, initial_value| {
            program
                .local(
                    machine(),
                    &slots,
                    &TableLocalData {
                        role,
                        symbol: constant,
                        name: Identifier::generated("q"),
                        type_reference: TypeReferenceHandle::invalid(),
                        initial_value,
                        is_mutable: false,
                        type_is_inferred: false,
                        is_operand_capture: false,
                        relevance: language_core::BindingRelevance::Relevant,
                        reflected_query_origin: None,
                        ..Default::default()
                    },
                )
                .map(drop)
                .unwrap_err()
                .detail
        };
        assert_eq!(
            declared(
                language_core::LocalBindingRole::Value,
                ExpressionHandle::invalid()
            ),
            "statement uninitialized local without a declared type"
        );
        assert_eq!(
            declared(language_core::LocalBindingRole::RecordPattern, whole),
            "statement record pattern binding"
        );
        assert_eq!(
            declared(language_core::LocalBindingRole::ArmPattern, whole),
            "statement arm pattern binding"
        );
        assert_eq!(
            program
                .expression(machine(), &slots, named_constant, None)
                .unwrap_err()
                .detail,
            "expression const"
        );
        // A contract clause can neither name a record formal whole nor
        // project it: clauses over places belong to a later slice.
        let values = crate::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            None,
        );
        let scope = ClauseScope {
            subject: None,
            formal_fields: false,
            mathematical: MathematicalTerms::Served,
            slots: &slots,
            values: &values,
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in an entry clause"),
        };
        let term = |handle| {
            program
                .term(machine(), &scope, handle, None)
                .unwrap_err()
                .detail
        };
        assert_eq!(
            term(projected),
            "contract clause that projects a formal or result"
        );
        assert_eq!(
            term(whole),
            "contract record formal named whole in a clause"
        );
    }

    /// `let { a, b as _ } = p;`: the marker is served when its roster names
    /// every field of the subject's record by symbol, bound or waived alike.
    /// A roster that omits a field, a row without a field symbol, and a roster
    /// resolved against no record or another one are each refused, whatever
    /// the rows are spelled.
    #[test]
    fn a_record_pattern_is_covered_by_field_symbols_never_by_spellings() {
        use semantic_vocabulary::StructuralTypeId;
        use typed_trees::expression::{MatchFieldAction, TableMatchField};
        use typed_trees::name::Identifier;
        use typed_trees::statement::TableLocalData;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Local, SymbolNameRef::Static("p")),
                (SymbolKind::Data, SymbolNameRef::Static("Point")),
                (SymbolKind::Data, SymbolNameRef::Static("Other")),
            ],
        );
        let local = children.handles().next().unwrap();
        let point = children.handles().nth(1).unwrap();
        let other = children.handles().nth(2).unwrap();
        let fields = builder.insert_children(
            point,
            [
                (SymbolKind::Field, SymbolNameRef::Static("a")),
                (SymbolKind::Field, SymbolNameRef::Static("b")),
            ],
        );
        let (a, b) = (fields.handle_at(0), fields.handle_at(1));
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut members = arena::HandleSpan::empty();
        typed
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("p"));
        let subject = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                symbol: local,
                ..TableNamePath::default()
            }));
        let literal = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let row = |field_symbol, member: &str, bound: bool| TableMatchField {
            component: language_core::PatternComponent::Fields,
            member: Identifier::generated(member),
            field_symbol,
            action: if bound {
                MatchFieldAction::Bind(Identifier::generated(member))
            } else {
                MatchFieldAction::Waive
            },
        };
        let mut roster =
            |rows: Vec<TableMatchField>| typed.expression_table.insert_match_fields(rows);
        let full = roster(vec![row(b, "b", false), row(a, "a", true)]);
        let partial = roster(vec![row(a, "a", true)]);
        let spelled = roster(vec![
            row(a, "a", true),
            row(SymbolHandle::invalid(), "b", true),
        ]);
        let foreign = roster(vec![
            row(a, "a", true),
            row(b, "b", true),
            row(other, "c", false),
        ]);
        let slots = [Slot {
            symbol: local,
            value_type: ValueType::record(StructuralTypeId::new(1).unwrap()),
            assignable: false,
            homed: false,
            erased: false,
            borrowed: None,
        }];
        let mut program = Program::new(&typed, false);
        program.record_sources.push(places::RecordSource {
            declaration: point,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: vec![a, b],
            cases: Vec::new(),
        });
        let mut read = |pattern_subject, pattern_fields, initial_value| {
            program.record_pattern(
                machine(),
                &slots,
                &TableLocalData {
                    role: language_core::LocalBindingRole::RecordPattern,
                    initial_value,
                    pattern_fields,
                    pattern_subject,
                    ..Default::default()
                },
            )
        };
        assert_eq!(read(point, full, subject), Ok(()));
        let mut detail = |pattern_subject, pattern_fields, initial_value| {
            read(pattern_subject, pattern_fields, initial_value)
                .unwrap_err()
                .detail
        };
        assert_eq!(
            detail(point, partial, subject),
            "statement record pattern that omits a field"
        );
        assert_eq!(
            detail(point, spelled, subject),
            "spelling record pattern field without a symbol"
        );
        assert_eq!(
            detail(point, foreign, subject),
            "spelling record pattern field without a symbol"
        );
        assert_eq!(
            detail(SymbolHandle::invalid(), full, subject),
            "spelling record pattern without a resolved subject"
        );
        assert_eq!(
            detail(other, full, subject),
            "typing record pattern resolved against another record"
        );
        assert_eq!(
            detail(point, full, literal),
            "statement record pattern over a value that is not a place"
        );
    }

    // A captured scalar remains a value; a captured reference keeps the
    // original storage and access instead of snapshotting the referent.
    #[test]
    fn operand_captures_keep_their_complete_binding_type() {
        use typed_trees::statement::TableLocalData;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols().into_iter().chain([
                (SymbolKind::Local, SymbolNameRef::Static("capture")),
                (SymbolKind::Parameter, SymbolNameRef::Static("input")),
            ]),
        );
        let u32_symbol = children.handle_at(7);
        let local_symbol = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let input_symbol = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 1)
            .unwrap();
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let carrier = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: u32_symbol,
            name: Default::default(),
        });
        let reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: carrier,
                access: ReferenceAccess::Shared,
                lifetime: None,
            });
        let exclusive = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: carrier,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
        let initial = typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let mut members = arena::HandleSpan::empty();
        typed.expression_table.push_name_path_member(
            &mut members,
            typed_trees::name::Identifier::generated("input"),
        );
        let borrowed_name = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                symbol: input_symbol,
                ..Default::default()
            }));
        let borrowed = [Slot {
            symbol: input_symbol,
            value_type: ValueType::plain(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            )),
            assignable: false,
            homed: true,
            erased: false,
            borrowed: Some(ReferenceAccess::Shared),
        }];
        let local = TableLocalData {
            symbol: local_symbol,
            type_reference: carrier,
            initial_value: initial,
            is_operand_capture: true,
            relevance: language_core::BindingRelevance::Relevant,
            ..Default::default()
        };
        let mut program = Program::new(&typed, false);
        let (slot, statement) = program.local(machine(), &[], &local).unwrap();
        assert_eq!(
            slot.value_type,
            ValueType::plain(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
            ))
        );
        assert!(!slot.assignable && !slot.homed && slot.borrowed.is_none());
        assert!(matches!(statement, crate::Statement::Bind { slot: 0, .. }));
        let (captured, statement) = program
            .local(
                machine(),
                &borrowed,
                &TableLocalData {
                    type_reference: reference,
                    initial_value: borrowed_name,
                    ..local.clone()
                },
            )
            .unwrap();
        assert_eq!(captured.borrowed, Some(ReferenceAccess::Shared));
        assert!(captured.homed);
        assert!(matches!(
            statement,
            crate::Statement::CaptureReference {
                slot: 1,
                original: 0
            }
        ));
        let mut refusal = |local: &TableLocalData| {
            program
                .local(machine(), &borrowed, local)
                .map(drop)
                .unwrap_err()
                .detail
        };
        assert_eq!(
            refusal(&TableLocalData {
                type_reference: exclusive,
                initial_value: borrowed_name,
                ..local.clone()
            }),
            "statement reference capture without matching storage and access"
        );
        assert_eq!(
            refusal(&TableLocalData {
                is_mutable: true,
                ..local.clone()
            }),
            "statement operand capture without its immutable value binding"
        );
        assert_eq!(
            refusal(&TableLocalData {
                type_reference: TypeReferenceHandle::invalid(),
                ..local.clone()
            }),
            "statement operand capture without its retained initializer and type"
        );
        assert_eq!(
            refusal(&TableLocalData {
                initial_value: ExpressionHandle::invalid(),
                ..local
            }),
            "statement operand capture without its retained initializer and type"
        );
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
        let reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: builtin,
                access: ReferenceAccess::Shared,
                lifetime: None,
            });
        let mut program = Program::new(&typed, false);
        assert_eq!(
            program.scalar_type(machine(), builtin),
            Ok(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
            ))
        );
        let detail = |handle| program.scalar_type(machine(), handle).unwrap_err().detail;
        assert_eq!(detail(nominal), "type nominal type");
        assert_eq!(detail(unresolved), "spelling type without a symbol");
        assert_eq!(detail(reference), "type reference");
        assert_eq!(
            program.result_type(machine(), TypeReferenceHandle::invalid()),
            Ok(None)
        );
    }

    /// `requires embed(rest) + embed(count) * 10 <= 99` over two `u8`
    /// formals compares mathematical integers: each `embed` keeps its
    /// formal's carrier, the literals are unbounded integers, and no
    /// operation is a carrier operation that could overflow. `>=` is the
    /// same order from the other side and `==` holds its operands in the
    /// vocabulary's order.
    ///
    /// What the slice does not serve is refused by name: the same sum
    /// without `embed` (carrier arithmetic, which owes representability
    /// first), mathematical division, a formal that is not embedded, a
    /// machine a program spells `embed`, and the comparison in a clause
    /// kind that cannot restate it yet.
    #[test]
    fn a_comparison_of_embedded_integers_is_a_mathematical_proposition() {
        use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm, Proposition};
        use symbols::{BuiltinFunction, builtin_function_symbols};
        use typed_trees::expression::TableCallExpression;
        use typed_trees::name::Identifier;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain([
                    (SymbolKind::Parameter, SymbolNameRef::Static("rest")),
                    (SymbolKind::Parameter, SymbolNameRef::Static("count")),
                    (SymbolKind::Machine, SymbolNameRef::Static("embed")),
                ]),
        );
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let builtin = typed
            .symbols
            .builtin_function_symbol(BuiltinFunction::IntegerEmbed)
            .unwrap();
        let declared = symbols::BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT;
        let rest = children.handles().nth(declared).unwrap();
        let count = children.handles().nth(declared + 1).unwrap();
        let impostor = children.handles().nth(declared + 2).unwrap();
        let name = |typed: &mut TypedTrees, symbol, spelled| {
            let mut members = arena::HandleSpan::empty();
            typed
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated(spelled));
            typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    symbol,
                    ..TableNamePath::default()
                }))
        };
        let call = |typed: &mut TypedTrees, target_symbol, argument| {
            let arguments = typed.expression_table.insert_expression_handles([argument]);
            typed
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: arena::Handle::invalid(),
                    authored_machine_application: arena::Handle::invalid(),
                    exact_coercion_target: TypeReferenceHandle::invalid(),
                    requirement_application: TypeReferenceHandle::invalid(),
                    receiver: ExpressionHandle::invalid(),
                    target_symbol,
                    static_machine_parameter: SymbolHandle::invalid(),
                    target: Identifier::generated("embed"),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments,
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }))
        };
        let literal = |typed: &mut TypedTrees, value| {
            typed
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
        };
        let binary = |typed: &mut TypedTrees, left, operator, right| {
            typed
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left,
                    operator,
                    right,
                }))
        };
        let (rest_name, count_name) = (
            name(&mut typed, rest, "rest"),
            name(&mut typed, count, "count"),
        );
        let embedded_rest = call(&mut typed, builtin, rest_name);
        let embedded_count = call(&mut typed, builtin, count_name);
        let (ten, limit) = (literal(&mut typed, 10), literal(&mut typed, 99));
        let tens = binary(&mut typed, embedded_count, BinaryOperator::Multiply, ten);
        let packed = binary(&mut typed, embedded_rest, BinaryOperator::Add, tens);
        let within = binary(&mut typed, packed, BinaryOperator::LessOrEqual, limit);
        let at_least = binary(&mut typed, limit, BinaryOperator::GreaterOrEqual, packed);
        let exactly = binary(&mut typed, packed, BinaryOperator::Equal, limit);
        // Outside the slice.
        let carrier_tens = binary(&mut typed, count_name, BinaryOperator::Multiply, ten);
        let carrier_sum = binary(&mut typed, rest_name, BinaryOperator::Add, carrier_tens);
        let carrier = binary(&mut typed, carrier_sum, BinaryOperator::LessOrEqual, limit);
        let halves = binary(&mut typed, embedded_rest, BinaryOperator::Divide, ten);
        let divided = binary(&mut typed, halves, BinaryOperator::LessOrEqual, limit);
        let mixed_sum = binary(&mut typed, embedded_rest, BinaryOperator::Add, count_name);
        let mixed = binary(&mut typed, mixed_sum, BinaryOperator::LessOrEqual, limit);
        let spelled = call(&mut typed, impostor, rest_name);
        let spelled_embed = binary(&mut typed, spelled, BinaryOperator::LessOrEqual, limit);
        let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let slot = |symbol| Slot {
            symbol,
            value_type: ValueType::Scalar(u8_type, crate::body::Qualification::NONE),
            assignable: false,
            homed: false,
            erased: false,
            borrowed: None,
        };
        let slots = [slot(rest), slot(count)];
        let values = [ValueId::new(1), ValueId::new(2)];
        let scope = |mathematical| ClauseScope {
            subject: None,
            formal_fields: true,
            mathematical,
            slots: &slots,
            values: &values,
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in an entry clause"),
        };
        let program = Program::new(&typed, false);
        let served = scope(MathematicalTerms::Served);
        let embedded = |value: u64| IntegerMathTerm::MathValue {
            source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: ValueId::new(value).unwrap(),
        };
        let integer = |value: u128| {
            IntegerMathTerm::IntegerLiteral(IntegerMathLiteral::new(false, value).unwrap())
        };
        let sum = IntegerMathTerm::Add(
            Box::new(embedded(1)),
            Box::new(IntegerMathTerm::Multiply(
                Box::new(embedded(2)),
                Box::new(integer(10)),
            )),
        );
        let bounded = Proposition::IntegerMathLessOrEqual(sum.clone(), integer(99));
        assert_eq!(
            program.clause(machine(), &served, within),
            Ok(bounded.clone())
        );
        assert_eq!(program.clause(machine(), &served, at_least), Ok(bounded));
        let equal = program.clause(machine(), &served, exactly).unwrap();
        let Proposition::IntegerMathEqual(left, right) = &equal else {
            panic!("an equality of mathematical integers: {equal:?}");
        };
        assert!(left <= right, "operands in the vocabulary's order");
        assert!([left, right].contains(&&sum) && [left, right].contains(&&integer(99)));
        terminal_codec::canonical_proposition_order_key(&equal).expect("an encodable clause");
        // The authored clause, each formal left open for a call's argument.
        assert_eq!(
            program
                .clause_text(&slots, within)
                .render(&|position| ["a", "b"][position].to_owned()),
            "embed(a) + embed(b) * 10 <= 99"
        );
        let detail = |scope: &ClauseScope<'_>, handle| {
            let refusal = program.clause(machine(), scope, handle).unwrap_err();
            assert_eq!(
                refusal.reason,
                terminal_check_tables::RefusalReason::Unsupported
            );
            refusal.detail
        };
        assert_eq!(
            detail(&served, carrier),
            "contract carrier arithmetic in a clause term"
        );
        assert_eq!(
            detail(&served, divided),
            "contract mathematical division in a clause"
        );
        assert_eq!(
            detail(&served, mixed),
            "contract runtime value in a mathematical term without embed"
        );
        assert_eq!(
            detail(&served, spelled_embed),
            "contract call in a clause term"
        );
        let guard = scope(MathematicalTerms::Refused(
            "mathematical comparison in a crash route guard",
        ));
        assert_eq!(
            detail(&guard, within),
            "contract mathematical comparison in a crash route guard"
        );
    }
}
