// AST: index-based arena nodes (port-friendly — children are `usize` indices,
// not references/Box, so this transliterates to Alpha's heap-free arenas).

#[derive(Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
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
    SelfIndex(i32, i32, i32, usize), // read self.<array>[index]; (byte offset, count, element_bytes, index node)
    ReadByte,                       // next byte from stdin, or -1 at EOF
    Min(usize, usize),              // min(a, b) builtin: the smaller of two i32s (signed); a, b node indices
    Max(usize, usize),              // max(a, b) builtin: the larger of two i32s (signed)
    Binary(BinaryOp, usize, usize), // op, lhs node, rhs node (indices into `expressions`)
    Call(usize, usize, usize),      // free call: machine index, args_start (into call_args), arg_count
    SelfCall(usize, usize, usize),  // method call self.m(args): passes self in rcx + args; same triple
}

// A transition arm: a pattern over the subject and the state it jumps to.
pub enum Pattern {
    Int(i32), // matches a specific integer (true=1, false=0)
    Wild,     // `_` default
}

pub struct TransitionArm {
    pub pattern: Pattern,
    pub target: usize,     // index into the machine's states
    pub args: Vec<usize>,  // arg expr nodes passed to the target state's parameters (empty = none)
}

pub enum Statement {
    Let(usize, usize, u8),                 // local index, init expr node, arithmetic domain: 0 Trapping (default), 1 Wrapping (no trap), 2 Saturating (clamp to i32 MIN/MAX)
    Assign(usize, usize),                  // local index, value expr node (reassignment)
    StoreSelfField(i32, usize, u8),        // self.<field at this byte offset> = value expr node; u8 = field arithmetic domain (0 trap, 1 wrap, 2 saturate)
    StoreSelfIndex(i32, i32, i32, usize, usize), // self.<array>[index] = value; (offset, count, element_bytes, index, value)
    Eval(usize),                           // evaluate an expr (a call) for effect, discard the result
    Return(usize),                         // return value expr node (yields to the caller)
    Exit(usize),                           // exit-code expr node (process exit; entry machine only)
    WriteByte(usize),                      // write the low byte of the value expr node to stdout
    WriteLine(usize),                      // index into Program.strings (bytes include trailing '\n')
    Transition(usize, Vec<TransitionArm>), // subject expr node, arms (jump to a state)
    Block(Vec<Statement>),                 // a sequence lowered in order (enum construction = tag + payload stores)
    Assert(usize),                         // runtime contract: trap if the condition expr is 0 (false)
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
    // Contract residue retained for STATIC discharge (compiler-generated proof certificates),
    // captured before `ensures` is desugared to runtime asserts. `result_local` is the slot
    // `result` names in a postcondition; `postconditions` are the raw cond expr nodes;
    // `return_exprs` are the body's returned-value nodes. See discharge.rs.
    pub result_local: Option<usize>,
    pub postconditions: Vec<usize>,
    pub return_exprs: Vec<usize>,
    // Preconditions (`requires`) retained for static CALL-SITE discharge: at a call to this
    // machine, the caller must prove these hold for the actual arguments. Each is a raw cond
    // expr node over this machine's parameters (locals 0..param_count). See discharge.rs.
    pub preconditions: Vec<usize>,
    // Bounded-STORE obligations from range-refined fields (`self.f = v` where `f: i32 in lo..hi`):
    // each (value node, lo, hi) owes `value in [lo, hi)`, discharged statically. See discharge.rs.
    pub store_obligations: Vec<(usize, i32, i32)>,
    // Range-typed lets that are NEVER reassigned: (local index, initializer node). Such a local equals its
    // initializer throughout, so `self.arr[x]` discharges by substituting `x` -> its init. See discharge.rs.
    pub local_inits: Vec<(usize, usize)>,
}

pub struct Program {
    pub machines: Vec<Machine>,
    pub entry_machine: usize,   // index of the process entry (Main::main)
    pub entry_data_size: i32,   // bytes of the entry machine's `self` data (its frame-resident instance)
    pub uses_imports: bool,     // uses any host op (write_line/write_byte/read_byte) => needs the import table
    pub expressions: Vec<Expr>, // global node arena
    pub call_args: Vec<usize>,  // flat arena of call-arg node indices (Expr::Call slices into this)
    pub strings: Vec<Vec<u8>>,
}
