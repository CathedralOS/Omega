# Byte window dispatch

Expected exit: 0.

Run `omega run --both samples/cli/collections/byte_window_dispatch/main.omg`.
A runtime implementation selector chooses the header or trailer of the same
packet. Each decoder returns a borrowed byte window; the caller checks its
length and reads the selected byte. Rebinding the decoder changes which
window is returned without changing the packet's storage.

The expected header byte is 11 and trailer byte is 29. Either a wrong slice,
an incorrect selector, or a lost backing reference reaches `crash Trap`.
