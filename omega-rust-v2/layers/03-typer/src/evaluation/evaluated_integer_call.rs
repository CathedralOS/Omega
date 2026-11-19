use num_bigint::BigInt;
use omega_resolver::DeclarationId;
/// The result of an admitted, closed invocation, in this stage's evaluation context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluatedIntegerCall {
    pub machine: DeclarationId,
    pub inputs: Vec<BigInt>,
    pub result: BigInt,
}
