# build_package_names

The package identity argument to `Build::application`/`Build::package`
is a snake_case name: it must start with a lowercase letter and use
snake_case lowercase words. There is no length bound.

- `invalid_name_spaces`: "my pkg name" — spaces reject.
- `invalid_name_empty`: "" — empty rejects.
- `invalid_name_dots`: "com.example.pkg" — dot-separated rejects.
- `invalid_name_unicode`: "Ωmega" — non-ASCII rejects.
