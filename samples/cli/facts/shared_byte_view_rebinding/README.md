# Shared byte-view rebinding

Rebinding a shared byte-view parameter retains its declared descriptor identity and observes replacement backing without mutating either backing.

Parent: **BORROWED-STORAGE-RESTORATION** in [TASKS.md](../../../../TASKS.md),
including its borrowed byte-storage acceptance. See [source ownership](../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
and [descriptor custody](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).

Each positive borrows separate byte arrays containing 5 and 29, rebinds the
shared parameter twice, reads 29 through the resulting descriptor, and observes
both unchanged backing arrays. The field place also retains its original view
and marker 73. Local, field, parameter, call result, match join, constant,
another state, another machine and target-body transports keep this same
operation. Public forwarding contracts explicitly retain the one-element bound;
the constant supplies a byte value rather than owning a runtime descriptor.

Each reject twin adds one write through the rebound shared descriptor. Mutable
binding permission allows rebinding; it does not grant mutable backing access.
The intended rejection requires mutable access through every crossed reference.

The frozen pre-prerequisite compiler source-checks the focused local program but
refuses executable production at `shared view rebind changes its declared carrier`.
The existing `pass/drops/reference_rebinding_preserves_pointee_exit` has the same
failure on both the frozen and pre-prerequisite candidate runners. Its catalog
producer adds stored access and shared copy permission. The rebind join must
reverse both transformations before comparing the complete canonical declaration.
It retains exact declared type/access, claim, cleanup and geometry checks. This
path admits only its existing element-view and borrowed byte-view families;
bounded byte owners remain owners.

All nine development positives execute native 0 and Terminal 0 on Linux x86-64,
and all nine shared-write twins reject. Final qualification and independent
receipt review accompany the PR. Bindings for other hosts do not claim execution
there; the whole TASKS.md item remains open.
