# Table report call survival

A complete native/Terminal regression for repeated console calls while table,
configuration and row-position values remain live. Formatting divisions impose
fixed-register/unit writes; projections after each call must retain the original
objects. All eight rows and the final configuration read are checked exactly.

Expected rows are independently calculated as cost = position + 2, score =
3 * position + 5, feasible = position + 1, code = 4 * position for positions
0..7; final minimum is 12. These exact integer oracles were authored before
execution. Only the linux_x86_64 entry target is declared.

This is a compiler composition regression, separate from the complete spatial
sensor deployment application. Pressure recovery first shortens a value in an
earlier function, then discovers an empty common register domain in a later
function: repeated fixed uses and intervening call clobbers cannot share one
home. Recovery shortens only that named original runtime value through the
existing admitted spill rewrite. Pressure scoring, finite victim retirement,
semantic rewrite validation and final allocation checks remain in force.
The failure was independently reproduced and repaired before integration.
Stable main subsequently landed a general interval-home recovery capability;
this candidate uses that existing facility and carries no duplicate compiler
change. Native and Terminal still must match the unchanged authored oracle.
