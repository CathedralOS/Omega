#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Carry {
    pub allows_suspension: bool,
    pub requires_same_cpu: bool,
    pub requires_same_thread: bool,
    pub requires_stable_address: bool,
}
