//! Project admitted current fragments into the ordinary image-publisher object.
//! No assigned body, legacy machine plan, or fabricated scalar result home is used.

mod attribution;
mod production;
mod source;
mod structural;
mod validation;

pub use production::build_function_fragment_object_artifact;
pub use validation::validate_function_fragment_object_artifact;

#[derive(Debug)]
pub enum FunctionFragmentObjectArtifactError {
    Source(object_file::RelocationFreeObjectContainerError),
    Unsupported(&'static str),
    Mismatch(&'static str),
    Overflow,
}

impl std::fmt::Display for FunctionFragmentObjectArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "shared object replay: {error}"),
            Self::Unsupported(reason) | Self::Mismatch(reason) => formatter.write_str(reason),
            Self::Overflow => {
                formatter.write_str("shared object coordinate exceeds supported size")
            }
        }
    }
}
impl std::error::Error for FunctionFragmentObjectArtifactError {}

use FunctionFragmentObjectArtifactError as Error;

fn host(value: u64) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::Overflow)
}
