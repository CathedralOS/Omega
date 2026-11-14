/*
What a diagnostic may call the constructs of one machine: its name, its
state's name, the authored text of its contract clauses, the binder each
evaluated value lands in, the text of each call's arguments, and the statement
position of each exit.

The lowered form carries identities and spans but no binder names, so a
rejection on the lowered route could say "requires clause 1 of the called
machine" but not "requires contract for call halve from run: count > 1". The
reader has the typed trees, where every name is an `Identifier` and every
expression renders through `ExpressionTable::display_name`, the same
rendering the deleted checker's messages use. So the reader records the
names here as it reads, and the checker renders its messages from this table
and the lowered form together. Nothing here is read back from source text
(`tools/source_text_reads.txt`), and nothing here decides anything: no
identity, verdict or obligation depends on it.

Bindings and calls are keyed by the source span the lowered form already
gives each construct (`MachineSources`), because that is the one
correspondence an obligation carries back to the source: an operation's site
is its operator, which lies inside exactly one binding's expression tree.
*/

use source::SourceSpan;

/// One machine's names, for diagnostics only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineNames {
    /// The declaration's name as authored; for a machine stage 04 generated
    /// to close a static application, its template's.
    pub machine: String,
    /// The name of the machine's one state.
    pub state: String,
    /// The formals' names in position order.
    pub formals: Vec<String>,
    /// The authored text of each `requires` clause, in the canonical order
    /// `Signature::requires` holds them: a `CallRequirement`'s `clause`
    /// indexes this.
    pub requires: Vec<ClauseText>,
    /// The authored text of each `ensures` clause, in authored order, as
    /// `Signature::ensures`.
    pub ensures: Vec<ClauseText>,
    /// The span of every node of a value expression whose result a binder
    /// receives: a local's initializer or an assignment of a whole value,
    /// and the value an arm completes the machine with.
    pub bindings: Vec<(SourceSpan, Binder)>,
    /// The span of each call this machine makes and the authored text of
    /// its arguments in position order.
    pub calls: Vec<(SourceSpan, Vec<String>)>,
    /// The span of each call that lends storage, the position among the
    /// call's runtime actuals of each argument that is a loan, and the
    /// binding at the root of the lent place as authored.
    pub loans: Vec<(SourceSpan, usize, String)>,
    /// The span of each exit, an arm's transition or the final value
    /// expression, and the position of its statement in the state's body,
    /// counted from 0 as the deleted checker counted "at statement N".
    pub exits: Vec<(SourceSpan, usize)>,
    /// The statement position of the exit that falls off the end of a unit
    /// body: the last statement when it is an expression, else one past it.
    pub fall_off: usize,
}

/// What receives the value an operation computes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binder {
    /// A formal or local, by its authored name.
    Local(String),
    /// The value the machine completes with.
    ReturnValue,
}

/// A clause's authored text with each formal left open, so it can be stated
/// over a call's actuals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClauseText(pub Vec<Piece>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    /// The formal at this position.
    Formal(usize),
}

impl ClauseText {
    pub fn push_text(&mut self, text: &str) {
        match self.0.last_mut() {
            Some(Piece::Text(last)) => last.push_str(text),
            _ => self.0.push(Piece::Text(text.to_owned())),
        }
    }

    /// The text with each formal replaced by `formal(position)`.
    pub fn render(&self, formal: &dyn Fn(usize) -> String) -> String {
        let mut rendered = String::new();
        for piece in &self.0 {
            match piece {
                Piece::Text(text) => rendered.push_str(text),
                Piece::Formal(position) => rendered.push_str(&formal(*position)),
            }
        }
        rendered
    }
}

impl MachineNames {
    /// The clause as authored, over this machine's own formals.
    pub fn authored(&self, clause: &ClauseText) -> String {
        clause.render(&|position| {
            self.formals
                .get(position)
                .cloned()
                .unwrap_or_else(|| String::from("?"))
        })
    }

    /// What receives the value computed at `site`.
    pub fn binder(&self, site: SourceSpan) -> Option<&Binder> {
        self.bindings
            .iter()
            .find(|(span, _)| *span == site)
            .map(|(_, binder)| binder)
    }

    /// The statement position of the exit at `site`.
    pub fn exit_statement(&self, site: crate::program::SourceOrigin) -> Option<usize> {
        match site {
            crate::program::SourceOrigin::Authored(site) => self
                .exits
                .iter()
                .find(|(span, _)| *span == site)
                .map(|(_, position)| *position),
            crate::program::SourceOrigin::Unpositioned(
                crate::program::Unpositioned::ImplicitReturn,
            ) => Some(self.fall_off),
            crate::program::SourceOrigin::Unpositioned(_) => None,
        }
    }

    /// The binding at the root of the place the call at `site` lends at
    /// argument `position`.
    pub fn lender(&self, site: SourceSpan, position: usize) -> Option<&str> {
        self.loans
            .iter()
            .find(|(span, at, _)| *span == site && *at == position)
            .map(|(_, _, name)| name.as_str())
    }

    /// The argument texts of the call at `site`.
    pub fn arguments(&self, site: SourceSpan) -> Option<&[String]> {
        self.calls
            .iter()
            .find(|(span, _)| *span == site)
            .map(|(_, arguments)| arguments.as_slice())
    }
}
