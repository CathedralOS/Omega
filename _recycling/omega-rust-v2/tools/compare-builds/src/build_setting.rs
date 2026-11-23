//! The ways each version is built, so that a difference has to show up in all of them to count.

/// One way of building the same code.
///
/// Two builds of identical logic can differ by several percent because of where the compiler
/// happens to put the hot loop in the binary. A change that is really in the code shows up
/// however the binary is laid out, so we build every version three ways and only believe a
/// difference that all three agree on. See experiments 8 and 9 in the lexer's EXPERIMENTS.md.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildSetting {
    /// An ordinary release build.
    Plain,

    /// The same code with every function in a different place. The flag only names the symbol
    /// scheme the compiler already uses, but passing any flag changes the hash inside every
    /// symbol name, and the functions are laid out in name order.
    Reshuffled,

    /// Every function starts on a 64-byte boundary. This costs a few percent by itself, which is
    /// why it is a check and not how we ship; what matters is whether the two versions still
    /// differ under it.
    Aligned,
}

impl BuildSetting {
    pub const ALL: [BuildSetting; 3] = [
        BuildSetting::Plain,
        BuildSetting::Reshuffled,
        BuildSetting::Aligned,
    ];

    pub fn get_label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Reshuffled => "reshuffled",
            Self::Aligned => "aligned",
        }
    }

    /// What goes in `RUSTFLAGS` for this setting. Empty for an ordinary build.
    pub fn get_rust_flags(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Reshuffled => "-C symbol-mangling-version=v0",
            Self::Aligned => "-C llvm-args=-align-all-functions=6",
        }
    }
}
