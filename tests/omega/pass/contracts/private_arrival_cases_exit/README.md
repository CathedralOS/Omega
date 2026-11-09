# Private arrival cases

Each private helper here is reached two ways: by a direct caller and as a
callable passed through a public interface. The helper may rely on a case or a
field value only because both arrivals establish it. The rejection twins are
`fail/contracts/private_arrival_open_callable_case` and
`fail/contracts/private_arrival_field_case`.
