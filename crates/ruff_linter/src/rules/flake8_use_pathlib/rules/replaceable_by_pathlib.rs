use ruff_diagnostics::Applicability;
use ruff_python_ast::name::QualifiedName;
use ruff_python_ast::{Expr, ExprCall};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::importer::ImportRequest;
use crate::rules::flake8_use_pathlib::helpers::{
    is_file_descriptor, is_keyword_only_argument_non_default,
};
use crate::rules::flake8_use_pathlib::{
    rules::Glob,
    violations::{Joiner, OsListdir, OsPathJoin, OsPathSplitext, PyPath},
};
use crate::{Edit, Fix};

pub(crate) fn replaceable_by_pathlib(
    checker: &Checker,
    call: &ExprCall,
    qualified_name: &QualifiedName,
) {
    let range = call.func.range();
    match qualified_name.segments() {
        ["os", module @ ("path" | "sep"), "join"] => {
            checker.report_diagnostic_if_enabled(
                OsPathJoin {
                    module: module.to_string(),
                    joiner: if call.arguments.args.iter().any(Expr::is_starred_expr) {
                        Joiner::Joinpath
                    } else {
                        Joiner::Slash
                    },
                },
                range,
            );
        }
        ["os", "path", "splitext"] => {
            checker.report_diagnostic_if_enabled(OsPathSplitext, range);
        }
        ["py", "path", "local"] => {
            checker.report_diagnostic_if_enabled(PyPath, range);
        }
        ["glob", function @ ("glob" | "iglob")] => {
            if is_keyword_only_argument_non_default(&call.arguments, "dir_fd") {
                return;
            }

            checker.report_diagnostic_if_enabled(
                Glob {
                    function: function.to_string(),
                },
                range,
            );
        }
        ["os", "listdir"] => {
            let path = call.arguments.find_argument_value("path", 0);
            if path.is_some_and(|expr| is_file_descriptor(expr, checker.semantic())) {
                return;
            }

            if let Some(mut diagnostic) = checker.report_diagnostic_if_enabled(OsListdir, range) {
                if let Some(path) = path {
                    diagnostic.try_set_fix(|| {
                        let (import_edit, binding) = checker.importer().get_or_import_symbol(
                            &ImportRequest::import("pathlib", "Path"),
                            call.start(),
                            checker.semantic(),
                        )?;

                        Ok(Fix::applicable_edits(
                            Edit::range_replacement(
                                format!("{binding}({}).iterdir()", checker.locator().slice(path)),
                                call.range(),
                            ),
                            [import_edit],
                            Applicability::DisplayOnly,
                        ))
                    });
                }
            }
        }

        _ => {}
    }
}
