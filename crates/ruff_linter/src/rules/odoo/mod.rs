//! Odoo rules: the static checks of Odoo's `test_lint` addon, native in ruff.
pub(crate) mod helpers;
pub(crate) mod model;
pub(crate) mod rules;
pub(crate) mod strings;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Result;
    use test_case::test_case;

    use crate::registry::Rule;
    use crate::test::test_path;
    use crate::{assert_diagnostics, settings};

    #[test_case(Rule::GettextVariable, Path::new("addon/models/gettext.py"))]
    #[test_case(Rule::GettextPlaceholders, Path::new("addon/models/gettext.py"))]
    #[test_case(Rule::GettextRepr, Path::new("addon/models/gettext.py"))]
    #[test_case(Rule::MissingGettext, Path::new("addon/models/gettext.py"))]
    #[test_case(Rule::MissingGettext, Path::new("addon/tests/test_gettext.py"))]
    #[test_case(Rule::RaiseUnlinkOverride, Path::new("addon/models/E8506.py"))]
    #[test_case(Rule::NPlusOneQuery, Path::new("addon/models/E8507.py"))]
    #[test_case(Rule::NPlusOneQuery, Path::new("addon/tests/test_E8507.py"))]
    #[test_case(Rule::OrmImport, Path::new("addon/models/E8508.py"))]
    #[test_case(Rule::OrmImport, Path::new("addon/tests/test_E8508.py"))]
    #[test_case(Rule::OrmImport, Path::new("framework/E8508.py"))]
    #[test_case(Rule::OnchangeDomain, Path::new("addon/models/E8509.py"))]
    #[test_case(Rule::ConfigChainmapPatch, Path::new("addon/models/E8510.py"))]
    #[test_case(Rule::GettextDeveloperError, Path::new("addon/models/gettext.py"))]
    #[test_case(Rule::ShadowedDefinition, Path::new("addon/models/E8513.py"))]
    #[test_case(Rule::HttpJsonString, Path::new("addon/controllers.py"))]
    #[test_case(Rule::RowCounterInTest, Path::new("addon/tests/test_E8516.py"))]
    #[test_case(Rule::RowCounterInTest, Path::new("addon/models/E8516.py"))]
    #[test_case(Rule::FieldRedeclared, Path::new("addon/models/E8521.py"))]
    #[test_case(Rule::DefaultEvaluatedAtImport, Path::new("addon/models/E8522.py"))]
    #[test_case(Rule::SelectionDuplicateKey, Path::new("addon/models/E8523.py"))]
    #[test_case(Rule::FieldHookPrefix, Path::new("addon/models/E8524.py"))]
    #[test_case(Rule::FieldPositionalArgument, Path::new("addon/models/E8525.py"))]
    #[test_case(Rule::FieldPositionalArgument, Path::new("addon/tests/test_fields.py"))]
    #[test_case(Rule::FieldPositionalArgument, Path::new("framework/fields.py"))]
    #[test_case(Rule::FieldAttributeOrder, Path::new("addon/models/E8526.py"))]
    #[test_case(Rule::DeadFieldAttribute, Path::new("addon/models/E8527.py"))]
    #[test_case(Rule::ReceiverFailOpen, Path::new("addon/controllers.py"))]
    #[test_case(Rule::StoredRelated, Path::new("addon/models/E8529.py"))]
    #[test_case(Rule::CompanyFieldOutsideConfig, Path::new("addon/models/E8530.py"))]
    #[test_case(
        Rule::CompanyFieldOutsideConfig,
        Path::new("addons/base/models/E8530.py")
    )]
    #[test_case(Rule::AuthMethodOutsideOwner, Path::new("addon/models/E8531.py"))]
    #[test_case(
        Rule::AuthMethodOutsideOwner,
        Path::new("addons/integration/models/E8531.py")
    )]
    #[test_case(Rule::HandRolledRange, Path::new("addon/models/E8532.py"))]
    #[test_case(Rule::HandRolledRange, Path::new("addons/base/models/E8532.py"))]
    #[test_case(Rule::RouteUntyped, Path::new("addon/controllers_typed.py"))]
    #[test_case(Rule::SqlBoundPlaceholder, Path::new("addon/models/E8534.py"))]
    #[test_case(Rule::AbolishedMethodCall, Path::new("addon/models/E8535.py"))]
    #[test_case(Rule::MarkupPreformatted, Path::new("addon/models/E8538.py"))]
    #[test_case(Rule::UserCacheWithoutGroups, Path::new("addon/models/E8540.py"))]
    #[test_case(Rule::HttpExceptionReturned, Path::new("addon/controllers.py"))]
    #[test_case(Rule::WsgiEnvironOptionalKey, Path::new("addon/wsgi.py"))]
    #[test_case(Rule::WsgiEnvironOptionalKey, Path::new("addon/wsgi_os.py"))]
    #[test_case(Rule::EmptyRecordsetMutation, Path::new("addon/models/E8543.py"))]
    fn rules(rule_code: Rule, path: &Path) -> Result<()> {
        let snapshot = format!(
            "{}_{}",
            rule_code.name(),
            path.to_string_lossy().replace('/', "__")
        );
        let diagnostics = test_path(
            Path::new("odoo").join(path).as_path(),
            &settings::LinterSettings::for_rule(rule_code),
        )?;
        assert_diagnostics!(snapshot, diagnostics);
        Ok(())
    }
}
