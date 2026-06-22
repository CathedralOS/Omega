// AST: index-based arena nodes (port-friendly — children are `usize` indices,
// not references/Box, so this transliterates to Alpha's heap-free arenas).

#[derive(Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
}

#[derive(Clone, Copy)]
pub enum Expr {
    Int(i32),
    Local(usize),                  // index into the locals frame
    Binary(BinaryOp, usize, usize), // op, lhs node, rhs node (indices into `expressions`)
}

// A transition arm: a pattern over the subject and the state it jumps to.
pub enum Pattern {
    Int(i32), // matches a specific integer (true=1, false=0)
    Wild,     // `_` default
}

pub struct TransitionArm {
    pub pattern: Pattern,
    pub target: usize, // index into Program.states
}

pub enum Statement {
    Let(usize, usize),                     // local index, init expr node
    Assign(usize, usize),                  // local index, value expr node (reassignment)
    Exit(usize),                           // exit-code expr node
    WriteLine(usize),                      // index into Program.strings (bytes include trailing '\n')
    Transition(usize, Vec<TransitionArm>), // subject expr node, arms (jump to a state)
}

// A machine = an entry block plus named state blocks (jump targets). A transition
// in the entry (or in a state) jumps to a state; loops are backward jumps.
pub struct Program {
    pub entry: Vec<Statement>,
    pub states: Vec<Vec<Statement>>, // state index -> its statements
    pub expressions: Vec<Expr>,
    pub local_count: usize,
    pub strings: Vec<Vec<u8>>,
}
