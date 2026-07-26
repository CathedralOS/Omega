# Tic-Tac-Toe

A scripted tic-tac-toe game using a `[i32; 9]` fixed-array board.
Stresses const-indexed array reads and writes across 9 cells, multi-arm
dispatch chains, and comparison chains over array elements.

Board layout (row-major, 0=empty, 1=X, 2=O):

```
0 1 2
3 4 5
6 7 8
```

Scripted game — X wins the top row:

```
X plays 0  (board[0]=1)
O plays 3  (board[3]=2)
X plays 1  (board[1]=1)
O plays 4  (board[4]=2)
X plays 2  (board[2]=1)  <- X wins
```

Win detection reads `board[0]`, `board[1]`, `board[2]` and confirms all
equal 1. Exits **70** when the sequence and board state are correct.

```
omega --target windows_x64 --build-dir build samples/tic_tac_toe/main.omg
./build/omega-program.exe   # exit 70
```

Exercises: `[i32; 9]` fixed array, const-indexed writes (`cells[n] = v`),
const-indexed guard reads, multi-arm dispatch chains, `[copy]`.
