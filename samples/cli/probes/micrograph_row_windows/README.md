# Micrograph row windows

A label image lends either half of its first strip as a mutable window.
Both windows have length 2. The caller writes the window from the lending
call, then stores that same window in a record and writes it again through
the field. The backing row shows the write: the first pair becomes 9 and 8,
and the second pair stays 3 and 4.

Expected exit: 0. There is no console output. Windows, macOS, and QEMU are
not executed here.
