//! Exact corpus inputs used by the content, text, and carrier tests.
//! Execution stages, reports, targets, and diagnostic assertions stay in the tests.

pub(crate) const UNARY_NEGATION_EXIT: &str = "operators/unary_negation_exit";
pub(crate) const UTF8_LITERAL_LEN_EXIT: &str = "domains/utf8_literal_len_exit";
pub(crate) const USER_DOMAIN_LITERAL_GRANT: &str = "domains/user_domain_literal_grant";
pub(crate) const BODYLESS_DOMAIN_DECLARATIONS_EXIT: &str =
    "domains/bodyless_domain_declarations_exit";
pub(crate) const BODYLESS_OWNER_ESTABLISHMENT: &str = "domains/bodyless_owner_establishment";
pub(crate) const EXTENT_ROOT_PROVIDER_ADAPTER: &str = "core/extent_root_provider_adapter";
pub(crate) const CONTENT_CONSERVATION_CONTRACT: &str = "core/content_conservation_contract";
pub(crate) const CARRY_PERMISSION_PROVIDER_ADAPTER: &str = "core/carry_permission_provider_adapter";
pub(crate) const VACUOUS_DOMAIN_QUALIFICATION: &str = "domains/vacuous_domain_qualification";
pub(crate) const USER_AUTHORED_PREDICATE_MACHINE: &str = "domains/user_authored_predicate_machine";
pub(crate) const DERIVES_AUTHORITY_VIA_BOUNDARY: &str =
    "capabilities/derives_authority_via_boundary";
pub(crate) const UTF8_PARAM_LEN_FIELD_EXIT: &str = "domains/utf8_param_len_field_exit";
pub(crate) const UTF8_REGULAR_CALL_LEN_EXIT: &str = "domains/utf8_regular_call_len_exit";
pub(crate) const UTF8_EQUALS_LITERAL_EXIT: &str = "domains/utf8_equals_literal_exit";
pub(crate) const UTF8_EQUALS_VIEW_EXIT: &str = "domains/utf8_equals_view_exit";
pub(crate) const UTF8_FIELD_READ_CARRIES_DOMAIN_EXIT: &str =
    "domains/utf8_field_read_carries_domain_exit";
pub(crate) const DOMAIN_FIELD_WRITE_THEN_READ_EXIT: &str =
    "domains/domain_field_write_then_read_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_WRITE_READ_EXIT: &str =
    "text/runtime_bounded_carrier_write_read_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_LENGTH_EXIT: &str =
    "text/runtime_bounded_carrier_length_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_LENGTH_FIELD_EXIT: &str =
    "text/runtime_bounded_carrier_length_field_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_BYTE_INDEX_EXIT: &str =
    "text/runtime_bounded_carrier_byte_index_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_BYTE_WIDEN_EXIT: &str =
    "text/runtime_bounded_carrier_byte_widen_exit";
pub(crate) const RUNTIME_CARRIER_INDEXED_READ_EXIT: &str = "text/runtime_carrier_indexed_read_exit";
pub(crate) const RUNTIME_NUMBER_TO_DECIMAL_EXIT: &str = "text/runtime_number_to_decimal_exit";
pub(crate) const RUNTIME_DECIMAL_TO_NUMBER_EXIT: &str = "text/runtime_decimal_to_number_exit";
pub(crate) const RUNTIME_CARRIER_INDEXED_WRITE_EXIT: &str =
    "text/runtime_carrier_indexed_write_exit";
pub(crate) const RUNTIME_CARRIER_INDEXED_READ_OPERAND_EXIT: &str =
    "text/runtime_carrier_indexed_read_operand_exit";
pub(crate) const RUNTIME_CARRIER_CIPHER_EXIT: &str = "text/runtime_carrier_cipher_exit";
pub(crate) const RUNTIME_CARRIER_INDEXED_CONST_WRITE_EXIT: &str =
    "text/runtime_carrier_indexed_const_write_exit";
pub(crate) const RUNTIME_CARRIER_LEN_GUARD_EXIT: &str = "text/runtime_carrier_len_guard_exit";
pub(crate) const RUNTIME_CARRIER_FNV_LOOP_EXIT: &str = "text/runtime_carrier_fnv_loop_exit";
pub(crate) const RUNTIME_MANDELBROT_RENDER_EXIT: &str = "text/runtime_mandelbrot_render_exit";
pub(crate) const RUNTIME_CRC32_EXIT: &str = "text/runtime_crc32_exit";
pub(crate) const RUNTIME_BASE64_ENCODE_EXIT: &str = "text/runtime_base64_encode_exit";
pub(crate) const RUNTIME_RUN_LENGTH_ENCODE_EXIT: &str = "text/runtime_run_length_encode_exit";
pub(crate) const RUNTIME_BINARY_FORMAT_EXIT: &str = "text/runtime_binary_format_exit";
pub(crate) const RUNTIME_SUBSTRING_SEARCH_EXIT: &str = "text/runtime_substring_search_exit";
pub(crate) const RUNTIME_STRING_PALINDROME_EXIT: &str = "text/runtime_string_palindrome_exit";
pub(crate) const RUNTIME_CARRIER_ITOA_EXIT: &str = "text/runtime_carrier_itoa_exit";
pub(crate) const RUNTIME_CARRIER_BYTE_WRITE_WIDTH_COERCION: &str =
    "text/runtime_carrier_byte_write_width_coercion";
pub(crate) const RUNTIME_BOUNDED_CARRIER_BYTE_WRITE_EXIT: &str =
    "text/runtime_bounded_carrier_byte_write_exit";
pub(crate) const RUNTIME_SLICE_LENGTH_FIELD_EXIT: &str = "calls/runtime_slice_length_field_exit";
pub(crate) const BODYLESS_NONOWNER_ESTABLISHMENT: &str = "domains/bodyless_nonowner_establishment";
pub(crate) const BODYFUL_OWNER_ESTABLISHMENT_BYPASS: &str =
    "domains/bodyful_owner_establishment_bypass";

pub(crate) const PASS_CANARIES: &[&str] = &[
    UNARY_NEGATION_EXIT,
    UTF8_LITERAL_LEN_EXIT,
    USER_DOMAIN_LITERAL_GRANT,
    BODYLESS_DOMAIN_DECLARATIONS_EXIT,
    BODYLESS_OWNER_ESTABLISHMENT,
    EXTENT_ROOT_PROVIDER_ADAPTER,
    CONTENT_CONSERVATION_CONTRACT,
    CARRY_PERMISSION_PROVIDER_ADAPTER,
    VACUOUS_DOMAIN_QUALIFICATION,
    USER_AUTHORED_PREDICATE_MACHINE,
    DERIVES_AUTHORITY_VIA_BOUNDARY,
    UTF8_PARAM_LEN_FIELD_EXIT,
    UTF8_REGULAR_CALL_LEN_EXIT,
    UTF8_EQUALS_LITERAL_EXIT,
    UTF8_EQUALS_VIEW_EXIT,
    UTF8_FIELD_READ_CARRIES_DOMAIN_EXIT,
    DOMAIN_FIELD_WRITE_THEN_READ_EXIT,
    RUNTIME_BOUNDED_CARRIER_WRITE_READ_EXIT,
    RUNTIME_BOUNDED_CARRIER_LENGTH_EXIT,
    RUNTIME_BOUNDED_CARRIER_LENGTH_FIELD_EXIT,
    RUNTIME_BOUNDED_CARRIER_BYTE_INDEX_EXIT,
    RUNTIME_BOUNDED_CARRIER_BYTE_WIDEN_EXIT,
    RUNTIME_CARRIER_INDEXED_READ_EXIT,
    RUNTIME_NUMBER_TO_DECIMAL_EXIT,
    RUNTIME_DECIMAL_TO_NUMBER_EXIT,
    RUNTIME_CARRIER_INDEXED_WRITE_EXIT,
    RUNTIME_CARRIER_INDEXED_READ_OPERAND_EXIT,
    RUNTIME_CARRIER_CIPHER_EXIT,
    RUNTIME_CARRIER_INDEXED_CONST_WRITE_EXIT,
    RUNTIME_CARRIER_LEN_GUARD_EXIT,
    RUNTIME_CARRIER_FNV_LOOP_EXIT,
    RUNTIME_MANDELBROT_RENDER_EXIT,
    RUNTIME_CRC32_EXIT,
    RUNTIME_BASE64_ENCODE_EXIT,
    RUNTIME_RUN_LENGTH_ENCODE_EXIT,
    RUNTIME_BINARY_FORMAT_EXIT,
    RUNTIME_SUBSTRING_SEARCH_EXIT,
    RUNTIME_STRING_PALINDROME_EXIT,
    RUNTIME_CARRIER_ITOA_EXIT,
    RUNTIME_CARRIER_BYTE_WRITE_WIDTH_COERCION,
    RUNTIME_BOUNDED_CARRIER_BYTE_WRITE_EXIT,
    RUNTIME_SLICE_LENGTH_FIELD_EXIT,
];

pub(crate) const UNAUTHORIZED_ESTABLISHMENT_FAIL_CANARIES: &[&str] = &[
    BODYLESS_NONOWNER_ESTABLISHMENT,
    BODYFUL_OWNER_ESTABLISHMENT_BYPASS,
];
