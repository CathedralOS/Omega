// AST: index-based arena nodes (port-friendly — children are `usize` indices,
// not references/Box, so this transliterates to Alpha's heap-free arenas).

#[derive(Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy)]
pub enum ExprKind {
    Int(i32),
    Local(usize),             // index into the locals frame
    Bin(BinOp, usize, usize), // op, lhs node, rhs node (indices into `exprs`)
}

pub enum Stmt {
    Let(usize, usize), // local index, init expr node
    Exit(usize),       // exit-code expr node
    WriteLine(usize),  // index into Main.strings (bytes already include trailing '\n')
}

pub struct Main {
    pub stmts: Vec<Stmt>,
    pub exprs: Vec<ExprKind>,
    pub n_locals: usize,
    pub strings: Vec<Vec<u8>>, // interned string-literal payloads for write_line
}
