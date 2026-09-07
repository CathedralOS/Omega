//! Explicit access to fixtures that require a direct conditional legal body.

pub(crate) trait ConditionalFixture {
    fn conditional(&self) -> &legalized_operations::LegalizedConditionalFunction;
    fn conditional_mut(&mut self) -> &mut legalized_operations::LegalizedConditionalFunction;
}

impl ConditionalFixture for legalized_operations::LegalizedFunction {
    fn conditional(&self) -> &legalized_operations::LegalizedConditionalFunction {
        let Self::Conditional(function) = self;
        function
    }

    fn conditional_mut(&mut self) -> &mut legalized_operations::LegalizedConditionalFunction {
        let Self::Conditional(function) = self;
        function
    }
}
