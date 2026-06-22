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
    Local(usize),                   // index into the locals frame
    SelfField(i32),                 // read self.<field> (scalar at this byte offset)
    Binary(BinaryOp, usize, usize), // op, lhs node, rhs node (indices into `expressions`)
    Call(usize, usize, usize),      // machine index, args_start (into call_args), arg_count
}

// A transition arm: a pattern over the subject and the state it jumps to.
pub enum Pattern {
    Int(i32), // matches a specific integer (true=1, false=0)
    Wild,     // `_` default
}

pub struct TransitionArm {
    pub pattern: Pattern,
    pub target: usize, // index into the machine's states
}

pub enum Statement {
    Let(usize, usize),                     // local index, init expr node
    Assign(usize, usize),                  // local index, value expr node (reassignment)
    StoreSelfField(i32, usize),            // self.<field at this byte offset> = value expr node
    Return(usize),                         // return value expr node (yields to the caller)
    Exit(usize),                           // exit-code expr node (process exit; entry machine only)
    WriteLine(usize),                      // index into Program.strings (bytes include trailing '\n')
    Transition(usize, Vec<TransitionArm>), // subject expr node, arms (jump to a state)
}

// A machine: a named, callable unit. The first `param_count` locals are its value
// parameters (filled by the caller). `entry` runs first; control then jumps among
// the named state blocks. The cross-machine call graph is a DAG.
pub struct Machine {
    pub param_count: usize,
    pub local_count: usize, // params + body locals
    pub makes_call: bool,   // calls write_line or another machine => needs ABI shadow space
    pub has_self: bool,     // has a `&mut self`/`&self` receiver => has a self-pointer slot
    pub entry: Vec<Statement>,
    pub states: Vec<Vec<Statement>>,
}

pub struct Program {
    pub machines: Vec<Machine>,
    pub entry_machine: usize,   // index of the process entry (Main::main)
    pub entry_data_size: i32,   // bytes of the entry machine's `self` data (its frame-resident instance)
    pub expressions: Vec<Expr>, // global node arena
    pub call_args: Vec<usize>,  // flat arena of call-arg node indices (Expr::Call slices into this)
    pub strings: Vec<Vec<u8>>,
}
