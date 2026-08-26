# process-exit

Capability-bearing fixture. Its public API reaches the exact toolchain-owned
`Console` boundary, which includes process termination at trait granularity.

Expected package evidence:

- exported service reach and invocation include `Console`;
- compiler-owned risk metadata classifies that exact service as process
  authority;
- a package-authored `Console` lookalike cannot spoof that classification;
- initial admission and updates retaining it recommend source audit.
