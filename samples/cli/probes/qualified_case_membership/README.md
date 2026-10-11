# qualified_case_membership

Fully qualified case membership tests across a package dependency:
`value in leaf::outcomes::Outcome::Empty` names the case through the
package alias and its module. The dep is a path package
(`Build::depend_as(..., Source::Path { location: "leaf" })`) whose own
`leaf/build.omg` declares it `membership_values`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
