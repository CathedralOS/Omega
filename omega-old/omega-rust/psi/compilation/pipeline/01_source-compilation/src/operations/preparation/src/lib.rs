//! Typed preparation (Psi 04) takes the typed trees of a whole program and
//! returns them prepared for the checker. `driver.rs` is the stage: one
//! entrance and the ordered list of passes every program runs. `passes/`
//! holds one module per pass; `product/` holds the Product operand
//! entrances: slots, providers and requirements retain the complete static
//! applications their Build requests select.

#![forbid(unsafe_code)]

mod driver;
mod passes;
mod product;

#[cfg(any(test, feature = "test-support"))]
pub use driver::{PreparationAdmission, PreparationRequest, TypedPreparation, prepare_typed_trees};
#[cfg(not(any(test, feature = "test-support")))]
pub use driver::{PreparationRequest, prepare_typed_trees};
#[cfg(any(test, feature = "test-support"))]
pub use product::PreparedProductRequirement;
pub use product::{
    prepare_product_operand_application, prepare_product_requirement_operand,
    prepare_retained_product_application,
};
