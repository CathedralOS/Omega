/*
    We turn checked `build.omg` declarations into an exact immutable package
    closure and independently validated compiler inputs. `source` binds declared
    local, workspace, and Git locations to source custody, `graph` follows those
    bindings, rejects conflicts, and gives the complete closure a canonical
    review subject, and `compiler_input` revalidates that custody at the
    compiler handoff.
*/

/*
    Package source and graph resolution belong to the package manager's
    resolution boundary (`resolution/mod.rs`).
*/

mod compiler_input;
pub mod graph;
pub mod source;

pub(crate) use compiler_input::PackageCompilationScope;
