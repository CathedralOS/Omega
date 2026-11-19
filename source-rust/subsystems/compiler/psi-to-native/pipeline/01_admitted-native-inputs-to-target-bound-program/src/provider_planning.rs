//! Native provider rows consume the source-settled canonical declarations.
#[path = "provider_rows.rs"]
pub(crate) mod provider_rows;

pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};

pub(crate) fn trait_name_text(schema: &provider_plans::ServiceSchema) -> &str {
    schema.identity_text(schema.declaration.trait_name)
}
