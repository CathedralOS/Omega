// Windows Job Object resource limits and process-tree containment
// (`WindowsJobChild`).
//
// We create and configure an anonymous Job Object + I/O Completion Port, spawn
// the child with `CREATE_SUSPENDED`, assign the suspended process to the job,
// and resume its initial threads. `try_wait` only reports completion once the
// job posts `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO` on the completion port,
// guaranteeing that the entire descendant process tree has exited.

mod job;
mod launch;
mod lifecycle;
mod limits;
mod native;
mod termination;

pub(crate) use lifecycle::WindowsJobChild;

#[cfg(test)]
mod tests;
