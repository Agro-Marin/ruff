//! Helpers to test if a specific preview style is enabled or not.
//!
//! The motivation for these functions isn't to avoid code duplication but to ease promoting preview behavior
//! to stable. The challenge with directly checking the `preview` attribute of [`LinterSettings`] is that it is unclear
//! which specific feature this preview check is for. Having named functions simplifies the promotion:
//! Simply delete the function and let Rust tell you which checks you have to remove.

use crate::settings::{LinterSettings, types::PreviewMode};


pub(crate) const fn is_pytest_asyncio_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_annotated_assignment_redefinition_enabled(
    settings: &LinterSettings,
) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_s103_extended_dangerous_bits_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_custom_exception_checking_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_suspicious_function_reference_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_comprehension_with_min_max_sum_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_bad_version_info_in_non_stub_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_legacy_type_comment_in_non_stub_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_runtime_string_union_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

/// <https://github.com/astral-sh/ruff/pull/19303>
pub(crate) const fn is_fix_f_string_logging_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_pytest_composite_assertion_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_manual_dict_comprehension_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_manual_list_comprehension_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_getsize_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}
pub(crate) const fn is_fix_os_path_getmtime_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_getatime_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_getctime_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_abspath_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_rmdir_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_unlink_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_remove_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_exists_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_expanduser_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_isdir_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_isfile_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_islink_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_isabs_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_readlink_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_basename_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_dirname_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_chmod_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_rename_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_replace_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_path_samefile_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_getcwd_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_mkdir_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_makedirs_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_stat_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_os_symlink_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_dunder_init_fix_unused_import_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_unicode_to_unicode_confusables_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_undefined_export_in_dunder_init_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_allow_nested_roots_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_fix_builtin_open_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_refined_submodule_import_match_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_type_var_default_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_b006_check_guaranteed_mutable_expr_enabled(
    settings: &LinterSettings,
) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_b006_unsafe_fix_preserve_assignment_expr_enabled(
    settings: &LinterSettings,
) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_enumerate_for_loop_int_index_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_mutable_default_in_dataclass_field_enabled(
    settings: &LinterSettings,
) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_standalone_mock_non_existent_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_up024_precise_highlighting_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_plural_ngettext_check_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_f811_shadowing_in_type_checking_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_resolve_string_annotation_pyi041_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_baseloader_safe_in_yaml_load_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_expanded_import_conventions_enabled(preview: PreviewMode) -> bool {
    preview.is_enabled()
}

pub(crate) const fn is_file_level_invalid_rule_code_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_redirected_suppression_code_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_incorrect_dict_iterator_comprehension_enabled(
    settings: &LinterSettings,
) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_e402_fix_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_pt020_fix_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_up006_future_annotations_fix_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_collapsible_if_fix_safe_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_pep604_future_annotations_fix_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub const fn is_human_readable_names_enabled(preview: PreviewMode) -> bool {
    preview.is_enabled()
}

pub const fn is_warn_on_unknown_selectors_enabled(preview: PreviewMode) -> bool {
    preview.is_enabled()
}

pub(crate) const fn is_b005_precise_diagnostic_enabled(settings: &LinterSettings) -> bool {
    settings.preview.is_enabled()
}

pub(crate) const fn is_rule_categories_enabled(preview: PreviewMode) -> bool {
    preview.is_enabled()
}
