// AST: index-based arena nodes (port-friendly — children are `usize` indices,
// not references/Box, so this transliterates to Alpha's heap-free arenas).

#[derive(Clone, Copy)]
pub enum BinOp {
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
pub enum ExprKind {
    Int(i32),
    Local(usize),             // index into the locals frame
    Bin(BinOp, usize, usize), // op, lhs node, rhs node (indices into `exprs`)
}

// A transition arm: a pattern over the subject and the state it jumps to.
pub enum Pat {
    Int(i32), // matches a specific integer (true=1, false=0)
    Wild,     // `_` default
}

pub struct Arm {
    pub pat: Pat,
    pub target: usize, // index into Main.states
}

pub enum Stmt {
    Let(usize, usize),         // local index, init expr node
    Exit(usize),               // exit-code expr node
    WriteLine(usize),          // index into Main.strings (bytes include trailing '\n')
    Transition(usize, Vec<Arm>), // subject expr node, arms (jump to a state)
}

// A machine = an entry block plus named state blocks (jump targets). A transition
// in the entry (or in a state) jumps to a state; loops are backward jumps.
pub struct Main {
    pub entry: Vec<Stmt>,
    pub states: Vec<Vec<Stmt>>, // state index -> its statements
    pub exprs: Vec<ExprKind>,
    pub n_locals: usize,
    pub strings: Vec<Vec<u8>>,
}
