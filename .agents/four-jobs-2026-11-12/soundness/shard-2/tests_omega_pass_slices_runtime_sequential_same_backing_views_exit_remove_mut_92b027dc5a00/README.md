# Sequential views of one backing in one expression

`omega run --both main.omg` must exit 0 in both engines. Two calls in one
statement borrow the same array, check and replace its element, and return
scalar snapshots. Both checks succeed and the stored value is 9. Equal backing and statement
identity do not make the two loan formations the same occurrence.
