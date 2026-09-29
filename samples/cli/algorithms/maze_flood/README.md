# Maze flood

Run breadth-first search through the authored 5×5 maze and render every cell's
shortest distance from the top-left. From the repository root:

```text
mbx run -p omega -- run samples/cli/algorithms/maze_flood/main.omg
```

Use Cargo if `mbx` is unavailable. Complete the ordinary
[package review](../../../../omega-rust/omega/application/cli/README.md#command-surface)
when requested. Leave `--target` off `run` to execute on the matching host.
Enter or EOF closes the native program.

The five rows must be exactly the following quoted strings; the quotes are
not printed. Each row is 16 ASCII bytes followed by LF, including two trailing
spaces:

```text
" 0  1  2  3  4  "
"## ## ## ##  5  "
"10  9  8  7  6  "
"11 ## ## ## ##  "
"12 13 14 15 16  "
```

The banner is `maze_flood: BFS shortest distances flooding a 5x5 maze (## = wall)`.
After the grid, print `PASS: distance map rendered, far corner is 16 away (exit 0)`
and `[press Enter to close]`, each on its own line. Native execution exits 0;
the CLI reports `native exit 0` on stderr.

Acceptance checks all 25 cells, the wall positions, padding and exit status.
The banner's hard-coded `16` does not establish that the algorithm computed
the distance. The five rows above follow independent breadth-first traversal
of the literal wall map.

This complete oracle was verified on macOS ARM64. The other declared hosted
targets still require matching-host execution.
