# Access external contents without losing the storage grant

Reading or writing external words must preserve the grant for their unchanged
storage descriptor. This program allocates three words, writes through checked
access views, and uses the same granted descriptor again after each helper.
It checks both the changed middle word and its untouched neighbors, then moves
the descriptor into a record and repeats the operation without changing the
record’s surrounding fields or the descriptor’s address and length.

The helpers return and consume `AccessOutcome` through the ordinary library
wrapper. Their returned views stay tied to the exact input region and its
borrow. The compiler must carry that relationship through the wrapper without
mistaking the storage descriptor for the memory it describes.

A second path chooses between two mutable views. Both choices run with distinct
initial values and verify that only the chosen middle word changes. This checks
that resolving a selected argument still retains every possible write target.

The expected exit is 0 on both the Terminal interpreter and native engine.
Build binds all four hosted targets; retained runtime observations name the
host actually executed. Full Cathedral compilation and boot are separate
remaining acceptance.

The invalid controls change the descriptor itself, replace the call with an
ordinary mutating boundary that shares a harmless helper’s name, deny the write
in the access policy, or try to write through a shared reference. Existing
returned-view controls additionally check that the source remains borrowed and
that a reference to a local value cannot escape.

The same program also owns two qualified descriptors in alternative-case array
slots. It updates both through a runtime-selected slot and checks the three old
word values, both final contents, and the surrounding nonzero fields. The
selection lends external bytes without replacing either descriptor or dropping
its existing grant. The original single-descriptor and selected-view checks
remain part of the exit-zero oracle.

Preservation forwards an existing exact domain instance; it creates no grant or
residency state. An opaque custody qualification is separate from observations
of the bytes. Content-dependent facts, lifecycle changes, and loan/restoration
obligations still use their existing checks. In particular, `Vacant` means no
live resident object, not zero bytes. The retained 8192-slot customer reduction
checks Granted/Vacant preservation; this runnable program observes the unchanged
descriptor grants and external words. It does not establish paging backing or
boot acceptance.
