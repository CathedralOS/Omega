//! Typed preparation (Psi 04) takes the typed trees of a whole program and
//! returns them prepared for the checker. `preparation.rs` is the stage: one
//! entrance and the ordered list of passes every program runs. `passes/`
//! holds one module per pass; `product/` identifies designated boundary
//! requirements and retains explicit refusals for unserved applications.

#![forbid(unsafe_code)]

mod passes;
mod preparation;
mod product;

pub use preparation::{
    PreparationAdmission, PreparationRequest, TypedPreparation, prepare_typed_trees,
};
pub use product::{
    PreparedProductRequirement, prepare_product_operand_application,
    prepare_product_requirement_operand,
};
