//! Operand custody is independent of dispatch and result shape. Cleanup and
//! projected-move accounting use the canonical Terminal operand query for
//! direct, boundary and dynamic invocations. Callee identity remains with
//! `direct_calls`.

pub(crate) use terminal_psi::call_structural_arguments as structural_arguments;
