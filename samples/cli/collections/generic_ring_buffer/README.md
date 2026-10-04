# generic_ring_buffer

Fixed-capacity ring buffer over `[i32 in Saturating; 8]` with modular head/tail indexing.

## What it demonstrates

- Invocation-local `Ring` storage, lent explicitly to static machines
- Direct runtime-indexed array reads and writes, justified by the ring's
  `head < 8, tail < 8` invariant
- Modular head/tail arithmetic (`tail mod 8`, `head mod 8`) for the ring wrap
- `Ring::pop` returns its value instead of using a persistent result field
- All three popped values, final size and remaining element sum are checked
- Console input storage exists only in the finishing state

The fixed script uses neither full-buffer pushes nor empty-buffer pops; this is
an indexing and state-passing sample, not a complete queue API.

## Scripted sequence

```
push(20)  -> slots[0]=20, tail=1, size=1
push(30)  -> slots[1]=30, tail=2, size=2
push(40)  -> slots[2]=40, tail=3, size=3
push(50)  -> slots[3]=50, tail=4, size=4
pop       -> returns 20, head=1, size=3
push(5)   -> slots[4]=5,  tail=5, size=4
push(15)  -> slots[5]=15, tail=6, size=5
pop       -> returns 30, head=2, size=4
pop       -> returns 40, head=3, size=3
```

Final: `head=3`, `tail=6`, `size=3`.  
Remaining elements: `slots[3]+slots[4]+slots[5] = 50+5+15 = 70`.

## Expected exit code

**70** — size==3, popped values 20/30/40, and remaining element sum==70 all verify correct.

## Building

```
mbx run -p omega -- --build-dir samples/cli/collections/generic_ring_buffer/build --target windows_x86_64 samples/cli/collections/generic_ring_buffer/main.omg
./samples/cli/collections/generic_ring_buffer/build/omega-program.exe
echo $?   # 70
```
