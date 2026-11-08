// Mathematical call application verification using the same proof checker that
// admits source contracts. Because mathematical functions may be partially applied
// or return another function, a runtime machine's parameter-count check cannot
// validate these calls.

use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

pub(crate) struct MathematicalCallQueries<'a> {
    input: &'a dyn validation::CheckingInput,
    checked: std::cell::OnceCell<
        Result<proof::mathematical_declarations::CheckedMathematics, Vec<Diagnostic>>,
    >,
}

impl<'a> MathematicalCallQueries<'a> {
    pub(crate) fn new(input: &'a dyn validation::CheckingInput) -> Self {
        Self {
            input,
            checked: std::cell::OnceCell::new(),
        }
    }

    pub(crate) fn require_application(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        let mathematics = self
            .checked
            .get_or_init(|| {
                proof::mathematical_declarations::check_mathematical_declarations(self.input)
            })
            .as_ref()
            .map_err(Clone::clone)?;
        if mathematics
            .prepared_view()
            .and_then(|view| view.application(expression))
            .is_none()
        {
            return Err(vec![Diagnostic::error(
                "reviewed mathematical call has no checked application for its exact source expression",
            )]);
        }
        Ok(())
    }
}
