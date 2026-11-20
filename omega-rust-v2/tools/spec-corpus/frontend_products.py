"""The layer tests and reviewed comparisons behind the frontend report's product column."""

PRODUCTS = {
    "L0": ("byte_cases", "line_comment_with_crlf_keeps_the_carriage_return", "token kinds and original byte ranges around CRLF and LF line comments"),
    "L1": ("operator_grouping", "every_operator_pair_has_the_specified_grouping", "grouping for each operator pair; comparison chains refused at the second operator"),
    "L2": ("stage", "ranking_order_resolves_to_its_declaration_and_bound_to_its_parameter", "both order forms select the Down declaration and bind the trailing limit to its parameter"),
    "L3": ("contextual_examples", "published_cleanup_clauses_select_owned_hooks", "both published cleanup clauses select release_guard with one owned parameter; borrowed twins refused"),
    "L4": ("target_layout", "a_selected_layout_survives_constant_aliases_and_type_formation", "ALIAS frozen to 8, consume parameter closed to [u8; 8], and the selected Size layout retained"),
    "L5": ("declarations", "transparent_aliases_share_the_underlying_schema_and_field_carrier", "private aliases retain their direct targets; both Container fields use Original’s nominal carrier"),
}


def comparison(stage):
    # The L0 CRLF equality also serves as the direct R6 handoff comparison.
    return "line-comments/crlf-product" if stage == "L0" else f"product/{stage.lower()}"
