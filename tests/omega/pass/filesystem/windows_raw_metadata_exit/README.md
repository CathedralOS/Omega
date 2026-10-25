# Native Windows metadata

Expected Windows x86-64 exit: 70. This is the raw metadata prerequisite of
**FILESYSTEM-PLATFORM-SERVICES**, not acceptance of the portable provider.

The program creates a file, writes eight bytes, queries its handle and path,
checks that metadata neither moved the cursor nor closed the caller's handle,
and queries a directory. Invalid handle zero, a genuine pipe and a missing path
exercise independent failures. Untouched metadata results are errors.

The provider pins the same object across all handle queries, classifies it as
a disk file, uses exact native records and captures the primary error before
closing its private handle. A metadata-only path open asks for read attributes,
not content or namespace mutation. Several queries are not an atomic snapshot
against concurrent writers. No POSIX identity or permission is fabricated.

Windows x86-64 execution now observes native exit 70, and the retained Terminal
inspection oracle matches. Terminal execution still declines because no trusted
host implementation matches the exact foreign bindings. The portable provider's
full dual-engine acceptance remains open under **FILESYSTEM-PLATFORM-SERVICES**.
