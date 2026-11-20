#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Carry {
    pub allows_suspension: bool,
    pub requires_same_cpu: bool,
    pub requires_same_thread: bool,
    pub requires_stable_address: bool,
}

/// Increasing restrictions on using an established value. A product retains the strongest restriction of its components.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default)]
pub enum Multiplicity {
    Copy,
    #[default]
    Affine,
    Linear,
}
