#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}
