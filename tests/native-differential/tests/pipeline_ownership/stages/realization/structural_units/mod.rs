//! Optimizer module role: stage group. Structural-unit realization coverage by custody shape.

mod disconnected_functions;
mod leaf_object;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod native_calls;
mod publication;
mod static_attachment;
mod structural_call;
mod structural_return;
mod zero_vreg_return;
