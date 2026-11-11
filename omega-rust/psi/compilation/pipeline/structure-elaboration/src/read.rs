/*
The reader: one machine's frozen typed trees resolved into the closed scalar
body. This is where identity is fixed (`check_on_lowered_form.md`, "Identity
is fixed before elaboration"):

- A type is scalar only through `SymbolTable::builtin_type_atom` on its
  resolved symbol: the compiler-installed root slot, never the spelling. The
  typed trees' own `PrimitiveType::from_name` classifies by spelling and is
  not used. A `Named` type whose symbol is invalid is refused as a spelling.
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
mod statements;
mod transitions;
mod types;

use arena::Arena;
use semantic_vocabulary::{CrashCause, IdTable, MachineId, ScalarType};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::TypedTrees;

use crate::body::Signature;
use crate::{Unserved, refuse};

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
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ValueId};
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
        let program = Program::new(&typed);
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
