//! Canonical normalized type identity text of each closed hosted entry
//! contract.
//!
//! These strings are the type identities a closed entry package commits its
//! visible parameters and result to. They are data, not behaviour, so they live
//! in one table: a package that gains an entry gains a line here, not a module
//! of its own.

/// Canonical normalized type identity of the process-completion status.
pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";

/// Canonical normalized type identity of the `argc` and result integers.
pub const MACOS_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
/// Canonical normalized type identity of the `argv`, `envp`, and `apple`
/// raw process-arrival addresses.
pub const MACOS_ARM64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";

/// Canonical normalized type identity of the `argc` and result integers.
pub const MACOS_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
/// Canonical normalized type identity of the `argv`, `envp`, and `apple`
/// raw process-arrival addresses.
pub const MACOS_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";

/// Canonical normalized type identity of the `initial_stack` arrival address.
pub const LINUX_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

/// Canonical normalized type identity of the `argc` arrival count delivered as
/// the head word of the initial process-stack image.
pub const LINUX_ARM64_U64_TYPE_IDENTITY: &str = "named(name(u64))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
