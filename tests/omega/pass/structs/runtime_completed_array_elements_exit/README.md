# Completed array elements

`omega run --both main.omg` must exit 70 on both engines after the package
review recorded by `omega update --project .` and
`omega update --project . --resume`.

The program moves a completed local record and an ordinary call result into the
two array elements of a record member. It observes both authored values after
construction. Array construction must evaluate each child once and preserve
its value through Terminal publication, artifact loading, and native execution.
