These programs check portable filesystem values and public calls.

`run/windows_adapter_values/main.omg` checks transfer counts around the native
limit, timestamp encodings and complete UTF-8 names. The Rust test reads the
operations from their actual library files. Its negative twin changes only the
copy count from 765 to 766 and must fail that exact precondition. This program
executes pure adapter operations; it does not call the Windows filesystem.

`run/public_values/main.omg` calls the selected host filesystem service. It
checks writes, reads, positioned I/O, copy, metadata and invalid arguments while
checking bytes beside each borrowed buffer. It also enumerates a private
directory containing one 255-byte name, so it needs no enumeration-order
promise.

`run/portable_rules/build.omg` calls the public filesystem service on a POSIX
host. It checks truncate refusal, oversized reads and copy, permission changes,
long path prefixes, and recursive removal. The test starts native and Psi in
separate working directories, each with 4,200 files. Each must exit 42 and
remove both its wide and deep directory trees.
