//! Call identities, arguments, results, and target-specific encodings.

pub mod arguments;
pub mod callbacks;
pub mod dynamic;
pub mod foreign;
pub mod internal;
pub mod results;
pub mod structural;

pub use arguments::*;
pub use callbacks::*;
pub use dynamic::*;
pub use foreign::*;
pub use internal::*;
pub use results::*;
pub use structural::*;
