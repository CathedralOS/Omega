#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum Multiplicity {
    Copy,
    #[default]
    Affine,
    Linear,
}
