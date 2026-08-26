# generic_ring_buffer

Fixed-capacity ring buffer over `[i32; 8]` with modular head/tail indexing.

## What it demonstrates

- `[i32; 8]` fixed array field in a `[copy]` data type
- Runtime-indexed array write via const-dispatch chain (the proven pattern):
  `write_slot(idx, value)` dispatches `idx == 0 | 1 | … | 7` to const-indexed writes
- Runtime-indexed array read via parallel `read_slot(idx)` dispatch
- Modular head/tail arithmetic (`tail mod 8`, `head mod 8`) for the ring wrap
- Sequential sub-machine calls that each mutate `self` fields
- Multi-level guard ladders to verify `size`, `last_pop`, and element sums

## Scripted sequence

```
push(20)  -> slots[0]=20, tail=1, size=1
push(30)  -> slots[1]=30, tail=2, size=2
push(40)  -> slots[2]=40, tail=3, size=3
push(50)  -> slots[3]=50, tail=4, size=4
pop       -> last_pop=20, head=1, size=3
push(5)   -> slots[4]=5,  tail=5, size=4
push(15)  -> slots[5]=15, tail=6, size=5
pop       -> last_pop=30, head=2, size=4
pop       -> last_pop=40, head=3, size=3
```

Final: `head=3`, `tail=6`, `size=3`.  
Remaining elements: `slots[3]+slots[4]+slots[5] = 50+5+15 = 70`.

## Expected exit code

**70** — size==3, last_pop==40, and remaining element sum==70 all verify correct.

## Building

```
cargo run -p omega-cli -- --build-dir samples/generic_ring_buffer/build --target windows_x64 samples/generic_ring_buffer/main.omg
./samples/generic_ring_buffer/build/omega-program.exe
echo $?   # 70
```
