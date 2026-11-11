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

mod calls;
mod contracts;
mod expressions;
mod machine;
mod places;
mod statements;
mod transitions;
mod types;

use arena::Arena;
use semantic_vocabulary::{CrashCause, IdTable, MachineId, StructuralTypeId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::TypedTrees;

use crate::body::{RecordType, Signature, ValueType};
use crate::{Unserved, refuse};

pub(crate) struct Program<'t> {
    typed: &'t TypedTrees,
    /// The typed-tree machine of each discovered `MachineId`.
    pub symbols: IdTable<MachineId, SymbolHandle>,
    pub signatures: IdTable<MachineId, Signature>,
    /// Every direct call read so far, as (caller, callee).
    calls: Vec<(MachineId, MachineId)>,
    /// Every record type a formal, local, result, field or literal named, in
    /// discovery order; a nested record is discovered before its owner.
    pub records: IdTable<StructuralTypeId, RecordType>,
    /// The symbols each record type was resolved from, by the same identity.
    record_sources: IdTable<StructuralTypeId, places::RecordSource>,
    /// The data declarations being resolved, outermost first: a declaration
    /// reached again while on this stack contains itself.
    resolving: Vec<SymbolHandle>,
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
}

/// One slot of a body: formals first, then locals in binding order.
struct Slot {
    symbol: SymbolHandle,
    value_type: ValueType,
    assignable: bool,
    /// A scalar local declared without an initializer: it is a storage home
    /// written by a store, not an SSA value rebound by assignment.
    homed: bool,
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
    pub fn new(typed: &'t TypedTrees) -> Self {
        Self {
            typed,
            symbols: IdTable::default(),
            signatures: IdTable::default(),
            calls: Vec::new(),
            records: IdTable::default(),
            record_sources: IdTable::default(),
            resolving: Vec::new(),
            fields_minted: 0,
            cases_minted: 0,
            captures: Vec::new(),
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
    use super::contracts::{ClauseScope, ResultTerm};
    use super::*;
    use language_core::ReferenceAccess;
    use numerics::literals::{FloatFormat, IntegerLiteral};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ScalarType, ValueId};
    use symbols::{SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath,
    };
    use typed_trees::expression::{
        TableBinaryExpression, TableBorrowExpression, TableIndexedExpression, TableRangeExpression,
    };
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    fn refused(typed: &TypedTrees, node: ExpressionNode) -> String {
        let mut typed = typed.clone();
        let handle = typed.expression_table.insert(node);
        let mut program = Program::new(&typed);
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
            (
                ExpressionNode::ArrayLiteral(arena::HandleSpan::empty()),
                "expression array literal",
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
        assert_eq!(
            program.refuse_recursion().unwrap_err().detail,
            "call recursion"
        );
        program.calls = vec![(m(1), m(1))];
        assert_eq!(program.refuse_recursion().unwrap_err().machine, m(1));
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
        let mut program = Program::new(&typed);
        let u32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let result = ValueId::new(2).unwrap();
        let exit = ClauseScope {
            subject: None,
            formal_fields: false,
            slots: &[],
            owner,
            result: ResultTerm::Named(result, u32_type),
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
                member_symbol: SymbolHandle::invalid(),
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
        for (node, expected) in cases {
            assert_eq!(refused(&typed, node), expected);
        }
    }

    /// Dispatch forms outside the slice are refused by name before anything
    /// is read from them: a `match` on a scalar, a case arm with a guard of
    /// its own, and a pattern whose head names no case.
    #[test]
    fn each_dispatch_form_outside_the_slice_is_refused_by_name() {
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
        let mut program = Program::new(&typed);
        assert_eq!(
            program
                .expression(machine(), &[], scalar_match, None)
                .unwrap_err()
                .detail,
            "expression match on a scalar subject"
        );
        let record = StructuralTypeId::new(1).unwrap();
        let pattern = |guard, head| TableDestructurePattern {
            guard,
            head,
            ..Default::default()
        };
        let detail = |pattern: TableDestructurePattern| {
            let refusal = program
                .case_pattern(machine(), record, &pattern)
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
                    member_symbol: SymbolHandle::invalid(),
                    member: Default::default(),
                    case_variant: None,
                }));
        let value = typed.expression_table.insert(ExpressionNode::Boolean(true));
        let slots = [Slot {
            symbol: local,
            value_type: ValueType::Record(StructuralTypeId::new(1).unwrap()),
            assignable: true,
            homed: false,
        }];
        let mut program = Program::new(&typed);
        let mut assigned = |target| {
            program
                .assignment(
                    machine(),
                    &slots,
                    &TableAssignment {
                        target,
                        value,
                        ..Default::default()
                    },
                )
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
            value_type: ValueType::Record(StructuralTypeId::new(1).unwrap()),
            assignable: false,
            homed: false,
        }];
        assert_eq!(
            program
                .assignment(
                    machine(),
                    &frozen,
                    &TableAssignment {
                        target: whole,
                        value,
                        ..Default::default()
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
        let scope = ClauseScope {
            subject: None,
            formal_fields: false,
            slots: &slots,
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
        let mut program = Program::new(&typed);
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
}
