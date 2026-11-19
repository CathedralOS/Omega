use crate::ValueType;
/// Type information from the target admitted by the caller. Each instruction
/// variant lists its explicit operand types in source order.
#[derive(Debug, Default)]
pub struct AssemblyTypes {
    pub target: String,
    pub registers: std::collections::HashMap<String, ValueType>,
    pub instructions: std::collections::HashMap<String, Vec<Vec<ValueType>>>,
}
