/*
We keep the requests a whole-program pass could not serve as rows on the
declarations that hold them (`DeferredRefusal`, `record_deferred_refusal`), and
answer whether a declaration holds one (`deferred_refusal`).
*/

/*
Typing and typed preparation see a program together with every bundled
library. A request neither can serve yet (a call selected by the value it is
called on, for one) is very often written in a library declaration the program
never reaches. Refusing the program there would stop every program for a
construct nobody asked for, so the pass records the refusal on the declaration
instead and goes on.

The row is raised by whoever demands the declaration: the reader of a machine
reached from an entry or a Build selection, and the full check of a package,
which demands every declaration of its subject whether or not anything uses
it. A declaration with a row was never served, so no reader may treat it as
checked, published or callable, and none may read through it.
*/

use crate::TypedTrees;
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};

/*
Most rows are requests no pass serves yet: the program may be valid, and the
reader says it is not supported. A few are errors in every program of their
form, found where the whole program is seen but raised, like the others, only
where a reader demands the declaration. Such a row is a violation, and the
reader rejects the program for it (`DeferredRequest::is_violation`).

A violation row is raised even where the construct is never lowered, as a
statement after a call that never returns is not:
`wiki/spec/language/state_contracts.md`, "Statements after a call that never
returns": "Unreachable statements are still source ... an unknown machine name
or malformed type is an error wherever it appears."
*/

/// What a pass was asked for and does not serve yet, or a form that is an
/// error in every program (`is_violation`). Closed: a pass that starts
/// recording a new kind of refusal names it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum DeferredRequest {
    /// A call on a value, `value.method(..)`: which machine it enters is
    /// chosen from the receiver's type, and no pass makes that choice yet.
    ReceiverCallSelection,
    /// A call whose written name no declaration in scope answers, though
    /// something in the program has that name (a local holding a callable,
    /// an ambiguous import, a qualified path resolution does not follow).
    UnresolvedCallTarget,
    /// A call naming something that is not called this way: a machine reached
    /// through a value, or a declaration of a kind with no call form.
    UncallableCallTarget,
    /// A literal assigned to a place whose declared type typing does not
    /// compute: the literal is left without the type it must be built at.
    AssignmentLiteralDestination,
    /// A constructor whose field types typing does not select, such as one
    /// of generic data applied at arguments before that data is specialized.
    ConstructorFieldSelection,
    /// A type applied through the sealed layout-storage declaration: its
    /// layout is the result of evaluating a policy, and nothing evaluates
    /// one yet.
    LayoutPolicyStorage,
    /// A type written as an expression to evaluate.
    TypeExpressionEvaluation,
    /// An array whose extent is an expression or a compile-time call.
    ArrayExtentEvaluation,
    /// An expression that stands for the evaluated form of another one.
    MaterializedExpression,
    /// A domain whose `access` policy is applied at static arguments
    /// (`access IssuedBy<..>`): no pass resolves those arguments yet.
    AccessPolicyApplication,
    /// A call that applies static arguments (`relay<u8>(..)`) which typed
    /// preparation's specialization pass does not bind or does not generate
    /// a machine for yet.
    StaticApplication,
    /// A domain cast, `value as =A -B +C`, whose operand's declared type
    /// typing does not read: a call result, a match or an unannotated local.
    DomainCastOperandType,
    /// A domain cast applied through a reference, which accounts for the
    /// referent's domains and may owe a removed one back at the loan's end.
    DomainCastThroughReference,
    /// A domain cast whose operand carries a compiler-owned domain or a
    /// constraint no declaration answered, which no entry can name yet.
    DomainCastUnnamedConstraint,
    /// A machine with one body per target that binds type or lifetime
    /// parameters or conformance bounds: holding each body to the shared
    /// signature means matching the two telescopes binder for binder, and
    /// typing has no such comparison yet.
    TargetFamilyGenericSignature,
    /// A call whose bare written name nothing in the program has: no
    /// declaration, parameter, local or compiler operation of any kind. A
    /// violation: "an unknown machine name ... is an error wherever it
    /// appears" (`state_contracts.md`).
    UndeclaredCallName,
}

impl DeferredRequest {
    /// Whether the row is an error in every program of its form, so the
    /// reader that demands its declaration rejects the program rather than
    /// calling it unsupported.
    pub fn is_violation(self) -> bool {
        matches!(self, Self::UndeclaredCallName)
    }

    /// The construct as a refusal names it, after the word "declaration";
    /// for a violation, the rule the program breaks.
    pub fn construct(self) -> &'static str {
        match self {
            Self::ReceiverCallSelection => {
                "holding a call selected by its receiver's type, which no pass serves yet"
            }
            Self::UnresolvedCallTarget => "holding a call whose name resolves to no declaration",
            Self::UncallableCallTarget => "holding a call of a target that is not called this way",
            Self::AssignmentLiteralDestination => {
                "holding a literal assigned to a place whose declared type is not computed yet"
            }
            Self::ConstructorFieldSelection => {
                "holding a constructor whose field types no pass selects yet"
            }
            Self::LayoutPolicyStorage => {
                "using a layout-policy storage type, which no pass evaluates yet"
            }
            Self::TypeExpressionEvaluation => {
                "using a type written as an expression, which no pass evaluates yet"
            }
            Self::ArrayExtentEvaluation => {
                "using an array with a computed extent, which no pass evaluates yet"
            }
            Self::MaterializedExpression => {
                "holding a materialized expression, whose evaluation no pass records yet"
            }
            Self::AccessPolicyApplication => {
                "selecting an access policy applied at static arguments, which no pass resolves yet"
            }
            Self::StaticApplication => {
                "holding a call whose static arguments no pass specializes yet"
            }
            Self::DomainCastOperandType => {
                "holding a domain cast whose operand's declared type typing does not read yet"
            }
            Self::DomainCastThroughReference => {
                "holding a domain cast through a reference, which no pass accounts for yet"
            }
            Self::DomainCastUnnamedConstraint => {
                "holding a domain cast over a compiler-owned or unresolved domain, which no pass accounts for yet"
            }
            Self::TargetFamilyGenericSignature => {
                "of a generic machine with one body per target, whose bodies typing does not compare with their shared signature yet"
            }
            Self::UndeclaredCallName => {
                "this call names nothing the program declares: an unknown machine name is an \
                 error wherever it appears, in a statement that never executes as in any other"
            }
        }
    }
}

/// One unserved request and the declaration that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct DeferredRefusal {
    /// The machine, trait requirement, data type or other declaration a
    /// reader demands as one unit.
    pub declaration: SymbolHandle,
    pub request: DeferredRequest,
    /// Where the request is written.
    pub source_span: SourceSpan,
}

impl TypedTrees {
    /// Records that `lexical_owner` holds a request no pass served. The row
    /// lands on the declaration a reader demands: a machine for anything
    /// inside one of its states, the lexical owner itself otherwise. Rows
    /// stay ordered by declaration, and a declaration keeps one row per kind
    /// of request, the first one recorded.
    pub fn record_deferred_refusal(
        &mut self,
        lexical_owner: SymbolHandle,
        request: DeferredRequest,
        source_span: SourceSpan,
    ) {
        let declaration = self.refusal_declaration(lexical_owner);
        let key = (declaration.arena_index(), declaration.generation(), request);
        let position = self.deferred_refusals.binary_search_by_key(&key, |row| {
            (
                row.declaration.arena_index(),
                row.declaration.generation(),
                row.request,
            )
        });
        if let Err(position) = position {
            self.deferred_refusals.insert(
                position,
                DeferredRefusal {
                    declaration,
                    request,
                    source_span,
                },
            );
        }
    }

    /// The row a reader raises for `declaration`, if it holds any: a
    /// violation when it holds one, because the program is then wrong
    /// whatever else is not served, else the first refusal recorded.
    pub fn deferred_refusal(&self, declaration: SymbolHandle) -> Option<&DeferredRefusal> {
        self.deferred_refusals_of(declaration)
            .find(|row| row.request.is_violation())
            .or_else(|| self.deferred_refusals_of(declaration).next())
    }

    /// Every refusal recorded on `declaration`, ordered by request.
    pub fn deferred_refusals_of(
        &self,
        declaration: SymbolHandle,
    ) -> impl Iterator<Item = &DeferredRefusal> {
        let start = self.deferred_refusals.partition_point(|row| {
            (row.declaration.arena_index(), row.declaration.generation())
                < (declaration.arena_index(), declaration.generation())
        });
        self.deferred_refusals[start..]
            .iter()
            .take_while(move |row| row.declaration == declaration)
    }

    /// Of the written names `names`, those nothing in the program has, in
    /// sorted order: no symbol of any kind, a declaration, parameter, local,
    /// state or builtin, in this source or any library, carries one as the
    /// last segment of its name. A name nothing has is an error wherever it
    /// is written; a name something has may name that thing validly where a
    /// reader does not follow it yet.
    pub fn names_nothing_has<'a>(&self, mut names: Vec<&'a str>) -> Vec<&'a str> {
        names.retain(|name| !name.is_empty());
        names.sort_unstable();
        names.dedup();
        if names.is_empty() {
            return names;
        }
        let mut named = vec![false; names.len()];
        for (symbol, _) in self.symbols.symbols().nodes().iter() {
            // A declaration may keep its qualified spelling as its name
            // (`machine kit::work`); a use writes the last segment.
            let name = self.symbols.name(symbol);
            let leaf = name.rsplit("::").next().unwrap_or(name);
            if let Ok(position) = names.binary_search(&leaf) {
                named[position] = true;
            }
        }
        names
            .into_iter()
            .zip(named)
            .filter_map(|(name, named)| (!named).then_some(name))
            .collect()
    }

    /// The declaration a reader demands as one unit for anything owned by
    /// `lexical_owner`: the machine of a state, the owner itself otherwise.
    pub fn refusal_declaration(&self, lexical_owner: SymbolHandle) -> SymbolHandle {
        if !lexical_owner.is_valid() {
            return lexical_owner;
        }
        let symbol = self.symbols.get(lexical_owner);
        if symbol.kind == SymbolKind::State
            && symbol.parent.is_valid()
            && self.symbols.get(symbol.parent).kind == SymbolKind::Machine
        {
            return symbol.parent;
        }
        lexical_owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolNameRef, SymbolTableBuilder};

    fn program() -> (TypedTrees, [SymbolHandle; 4]) {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("served")),
                (SymbolKind::Machine, SymbolNameRef::Static("holder")),
                (SymbolKind::Trait, SymbolNameRef::Static("Api")),
            ],
        );
        let holder = declarations.handle_at(1);
        let state = symbols
            .insert_children(holder, [(SymbolKind::State, SymbolNameRef::Static("run"))])
            .handle_at(0);
        let requirement = symbols
            .insert_children(
                declarations.handle_at(2),
                [(SymbolKind::State, SymbolNameRef::Static("call"))],
            )
            .handle_at(0);
        let program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        (
            program,
            [declarations.handle_at(0), holder, state, requirement],
        )
    }

    #[test]
    fn a_request_inside_a_state_refuses_its_machine_and_no_other_declaration() {
        let (mut program, [served, holder, state, _]) = program();
        program.record_deferred_refusal(
            state,
            DeferredRequest::ReceiverCallSelection,
            SourceSpan::default(),
        );
        let row = program.deferred_refusal(holder).expect("the holder's row");
        assert_eq!(row.request, DeferredRequest::ReceiverCallSelection);
        assert_eq!(row.declaration, holder);
        // The failing twin: a declaration without a request stays served,
        // and the state is not a declaration of its own.
        assert!(program.deferred_refusal(served).is_none());
        assert!(program.deferred_refusal(state).is_none());
    }

    #[test]
    fn a_requirement_keeps_its_own_row_and_repeats_record_once() {
        let (mut program, [_, holder, state, requirement]) = program();
        for _ in 0..2 {
            program.record_deferred_refusal(
                requirement,
                DeferredRequest::UnresolvedCallTarget,
                SourceSpan::default(),
            );
            program.record_deferred_refusal(
                state,
                DeferredRequest::UnresolvedCallTarget,
                SourceSpan::default(),
            );
            program.record_deferred_refusal(
                holder,
                DeferredRequest::ReceiverCallSelection,
                SourceSpan::default(),
            );
        }
        assert_eq!(program.deferred_refusals.len(), 3);
        assert_eq!(
            program.deferred_refusal(requirement).map(|row| row.request),
            Some(DeferredRequest::UnresolvedCallTarget)
        );
        // Rows of one declaration are ordered by request; the first is raised.
        assert_eq!(
            program.deferred_refusal(holder).map(|row| row.request),
            Some(DeferredRequest::ReceiverCallSelection)
        );
    }

    #[test]
    fn only_a_name_no_symbol_of_any_kind_has_names_nothing() {
        let (program, _) = program();
        // A machine, a state and a trait each have their name; an empty
        // name was written by no one.
        assert_eq!(
            program.names_nothing_has(vec!["run", "nowhere", "holder", "", "Api", "nowhere"]),
            ["nowhere"]
        );
        assert!(program.names_nothing_has(vec!["served", "call"]).is_empty());
    }

    #[test]
    fn a_violation_is_raised_before_any_unserved_request_of_its_declaration() {
        let (mut program, [served, holder, state, _]) = program();
        for request in [
            DeferredRequest::ReceiverCallSelection,
            DeferredRequest::UndeclaredCallName,
            DeferredRequest::StaticApplication,
        ] {
            program.record_deferred_refusal(state, request, SourceSpan::default());
        }
        let raised = program.deferred_refusal(holder).expect("the holder's row");
        assert_eq!(raised.request, DeferredRequest::UndeclaredCallName);
        assert!(raised.request.is_violation());
        // The twin: a declaration holding only unserved requests raises the
        // first, which is no violation.
        program.record_deferred_refusal(
            served,
            DeferredRequest::UnresolvedCallTarget,
            SourceSpan::default(),
        );
        let unserved = program.deferred_refusal(served).expect("the served row");
        assert_eq!(unserved.request, DeferredRequest::UnresolvedCallTarget);
        assert!(!unserved.request.is_violation());
    }
}
