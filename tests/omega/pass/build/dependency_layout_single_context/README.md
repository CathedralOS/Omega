# A layout proof with one context

The dependency's ordinary private policy calls `require_extent`, whose query
requires the Mac-only base layout to occupy eight bytes. Its materialized view
stores and reads 51. Other hosted entry bodies are empty; checking those bodies
must preserve the Mac proof without trying to evaluate its query under Linux or
Windows. `omega run --both main.omg` exits zero on the Mac host.

The [reject twin](../../../fail/build/dependency_layout_single_context/kit/geometry.omg)
changes only the base plan extent from eight bytes to four. The ordinary policy
still owes the same `size_of<BasePolicy<Payload>>() == 8` requirement and rejects.
Runtime queries stay portable; discharged static observations retain exact
qualified receiving conditions on the policy's materialized carrier.
