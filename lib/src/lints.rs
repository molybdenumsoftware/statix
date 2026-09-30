use crate::lints;

mod module_context;
mod shell_context;

lints! {
    bool_comparison,
    empty_let_in,
    manual_inherit,
    manual_inherit_from,
    legacy_let_syntax,
    collapsible_let_in,
    eta_reduction,
    useless_parens,
    // unquoted_splice,
    empty_pattern,
    redundant_pattern_bind,
    unquoted_uri,
    empty_inherit,
    deprecated_to_path,
    bool_simplification,
    useless_has_attr,
    repeated_keys,
    empty_list_concat,
    ineffective_string_escape,
    negated_is_null,
    optionals_string,
    optional_list_in_flat_context,
    deprecated_stdenv_lib,
    fetcher_hash_field,
    broad_with_lib,
    with_lexical_collision,
    module_mkif_update,
    module_optional_attrs,
    suspect_native_dependency,
    argv_multi_flag_string,
    shell_double_escaping,
    shell_unquoted_substitution,
    shell_unescaped_env,
    shell_variable_interpolation
}
