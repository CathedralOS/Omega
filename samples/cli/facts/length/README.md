# Runtime window length

A runtime window length names the last byte of that window: for a window
`items[0..count]` with `count > 0`, indexing at `length - 1` reads the byte at
`count - 1`, provided `length` is proved to be that window's length.

Every place copies that fact and performs the same last-byte read on a
four-byte `[4, 7, 8, 9]` array with `count == 3`; the oracle is exit 0 only
when the byte is 8. Only the transport of the window length changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the length is copied into a local |
| field | `field/` | the length is stored in a record field |
| parameter | `parameter/` | the length arrives as an explicit parameter |
| call result | `call_result/` | a call result is the window length |
| match join | `match_join/` | both arms yield the window length |
| const name | `const_name/` | a named constant is the window length |
| another state | `other_state/` | a successor state receives the length |
| another machine | `other_machine/` | a forwarding machine supplies the length |
| another target body | `target_body/` | each of four target bodies uses the length |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/length/local/main.omg
omega run samples/cli/facts/length/field/main.omg
omega run samples/cli/facts/length/parameter/main.omg
omega run samples/cli/facts/length/call_result/main.omg
omega run samples/cli/facts/length/match_join/main.omg
omega run samples/cli/facts/length/const_name/main.omg
omega run samples/cli/facts/length/other_state/main.omg
omega run samples/cli/facts/length/other_machine/main.omg
omega run samples/cli/facts/length/target_body/main.omg
```

Each place has an `invalid/` twin whose transported length names the backing
view rather than the runtime window. The last-byte index is then no longer
proved: the twin must reject with `cannot prove index ...` (the const-name twin
reports `cannot prove index ... is within length 2`). Check a twin with
`omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/{pass,fail}/facts/length_through_*`
fixtures; every positive records native exit 0.
