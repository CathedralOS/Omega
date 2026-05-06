# Dungeon Crawler CLI Sample

This sample explores Omega as a console program rather than a windowed game.

What it is trying to prove:

- `state entry` can drive a terminal application
- console input/output is isolated behind a platform machine
- room entry is explicit and visible in the state graph
- level data can be hardcoded without hardcoding traversal logic
- parent-to-child machine flow can be expressed with nested arrows
- user input drives ordered transitions
- invalid input can loop back without hidden branches

Current loop:

- enter a room
- print the current cell, such as `A1`
- print adjacent rooms
- read a user command
- classify the command into owned navigation data
- move if the command names an adjacent room
- otherwise print an invalid-command message and ask again
- return to parent control flow from the terminal dungeon state

Sample layout:

- `main.omg`: process-level runner
- `dungeon/`: generic dungeon flow, room movement, and command classification
- `levels/`: hardcoded sample level data
- `data/`: shared dungeon data
- `platform/`: console boundary
