# process-exit

Capability-bearing fixture. Its public API reaches the exact toolchain-owned
`Console` boundary through an explicit affine `Service<Console> in Bound`
parameter. `Console` includes process termination at trait granularity; the
invocation remains rooted at public parameter zero for package review.

Expected package evidence:

- exported service reach and invocation include `Console`;
- the public runtime carrier is the exact routed Service parameter rather than
  a transitional bare boundary-trait value;
- compiler-owned risk metadata classifies that exact service as process
  authority;
- a package-authored `Console` lookalike cannot spoof that classification;
- initial review and updates retaining it recommend source audit.
