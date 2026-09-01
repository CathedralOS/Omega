//! Optimizer module role: executable entrance. Parameter-family adapters, grouped by the value-producing semantic shape.

pub(in crate::validation::catalog) mod arithmetic;
pub(in crate::validation::catalog) mod bitwise;
pub(in crate::validation::catalog) mod comparison;
pub(in crate::validation::catalog) mod direct;
pub(in crate::validation::catalog) mod shift;
pub(in crate::validation::catalog) mod unary;
