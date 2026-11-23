/// Increasing restrictions on using an established value. A product retains the strongest restriction of its components.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default)]
pub enum Multiplicity {
    Copy,
    #[default]
    Affine,
    Linear,
}
