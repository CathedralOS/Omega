use crate::StaticIntegerRequest;
use num_bigint::BigInt;
/// Implementations compute the expression in the same resolved stage. L3 rechecks
/// its carrier and range before returning a completed typed stage.
pub trait StaticIntegerEvaluator {
    fn evaluate(
        &self,
        request: &StaticIntegerRequest,
    ) -> Result<BigInt, String>;
}
impl<F: Fn(&StaticIntegerRequest) -> Result<BigInt, String>> StaticIntegerEvaluator for F {
    fn evaluate(
        &self,
        request: &StaticIntegerRequest,
    ) -> Result<BigInt, String> {
        self(request)
    }
}
