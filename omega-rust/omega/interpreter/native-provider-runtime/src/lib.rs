/*
We execute admitted fixed-frame native adapters inside a dedicated private
provider process.

Our host integration owns real native shared libraries, shared physical homes,
and process-level containment. We consume source-free compilation products and
never invoke the compiler. Terminal Psi retains semantic ownership, loan, and
completion admission; an IPC reply from our worker process alone establishes
none of those obligations.
*/

/*
Why isolate native providers in a private process with shared physical homes
instead of calling foreign libraries in-process?

Foreign C libraries can trap, corrupt process state, register atexit/TLS
callbacks, or retain raw pointers across calls. By launching a bounded worker
process (`NativeProviderProcess`) connected over an authenticated private
channel (`channel`) and sharing immutable file-backed memory mappings
(`shared_backing`), a foreign trap faults only that worker incarnation without
corrupting the host interpreter or replaying side effects.
*/

#![deny(unsafe_op_in_unsafe_fn)]

mod channel;
mod host_clock;
pub use host_clock::read_host_clock_nanoseconds;
mod host_filesystem;
pub use host_filesystem::{KernelOperand, KernelResult, LINUX_AT_FDCWD, LinuxFilesystem};
mod private_filesystem;
pub use private_filesystem::{LinuxFilesystemOperation, PrivateLinuxFilesystem};
mod native_code;
mod native_provider_runtime;
mod pointer_links;
pub use pointer_links::{NativePointerSlot, NativePointerTarget};
mod shared_backing;
mod wire;
mod worker;
pub use native_provider_runtime::{
    NativeAdapterId, NativeArgument, NativeHomeId, NativeParameterStorage, NativeProviderError,
    NativeProviderProcess,
};
pub use worker::run_worker_if_requested;

#[cfg(test)]
mod physical_tests;
