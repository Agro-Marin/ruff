use crate::{
    FxOrderSet,
    types::{
        ParamSpecAttrKind, Type,
        context::InferContext,
        diagnostic::{INVALID_PARAMSPEC, UNBOUND_TYPE_VARIABLE},
        generics::resolve_typevar_reference,
    },
};
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_core::SemanticIndex;

/// Validate the usage of `ParamSpec` components (`P.args` and `P.kwargs`) across all
/// parameters of a function.
///
/// This enforces several rules from the typing spec:
/// - `P.args` and `P.kwargs` must always be used together
/// - When `*args: P.args` is present, `**kwargs: P.kwargs` must also be present (same P)
/// - `P` must already be in scope
/// - No keyword-only parameters are allowed between `*args: P.args` and `**kwargs: P.kwargs`
pub(super) fn validate_paramspec_components<'db>(
    context: &'db InferContext<'db, '_>,
    index: &SemanticIndex<'db>,
    parameters: &ast::Parameters,
    infer_type: impl Fn(&ast::Expr) -> Type<'db>,
) {
    let db = context.db();
    let env = context.program_environment();

    let args_paramspec = parameters.vararg.as_deref().and_then(|vararg| {
        let annotation = vararg.annotation()?;
        let ty = infer_type(annotation);
        if let Type::TypeVar(typevar) = ty
            && typevar.is_paramspec(db)
            && typevar.paramspec_attr(db) == Some(ParamSpecAttrKind::Args)
        {
            Some((typevar.without_paramspec_attr(db), annotation))
        } else {
            None
        }
    });

    let kwargs_paramspec = parameters.kwarg.as_deref().and_then(|kwarg| {
        let annotation = kwarg.annotation()?;
        let ty = infer_type(annotation);
        if let Type::TypeVar(typevar) = ty
            && typevar.is_paramspec(db)
            && typevar.paramspec_attr(db) == Some(ParamSpecAttrKind::Kwargs)
        {
            Some((typevar.without_paramspec_attr(db), annotation))
        } else {
            None
        }
    });

    let vararg_name = parameters.vararg.as_deref().map(|v| v.name.as_str());
    let kwarg_name = parameters.kwarg.as_deref().map(|k| k.name.as_str());

    match (args_paramspec, kwargs_paramspec) {
        (Some((args_tv, args_annotation)), Some((kwargs_tv, kwargs_annotation))) => {
            if !args_tv.is_same_typevar_as(db, kwargs_tv) {
                let args_name = args_tv.name(db);
                let vararg = vararg_name.unwrap_or("args");
                let kwarg = kwarg_name.unwrap_or("kwargs");
                if let Some(builder) = context.report_lint(&INVALID_PARAMSPEC, kwargs_annotation) {
                    builder.into_diagnostic(format_args!(
                        "`*{vararg}: {args_name}.args` must be accompanied \
                             by `**{kwarg}: {args_name}.kwargs`",
                    ));
                }
            } else {
                let paramspec_is_bound_by_parameter = parameters
                    .iter()
                    .filter_map(ast::AnyParameterRef::annotation)
                    .map(&infer_type)
                    .filter(|ty| {
                        !matches!(
                            ty,
                            Type::TypeVar(typevar)
                                if typevar.is_paramspec(db)
                                    && typevar.paramspec_attr(db).is_some()
                        )
                    })
                    .any(|ty| {
                        let mut typevars = FxOrderSet::default();
                        ty.find_legacy_typevars(db, env, None, &mut typevars);
                        typevars
                            .iter()
                            .any(|typevar| typevar.is_same_typevar_as(db, args_tv))
                    });
                let paramspec_is_in_scope = paramspec_is_bound_by_parameter
                    || index
                        .scope(context.scope().file_scope_id(db))
                        .parent()
                        .is_some_and(|parent_scope| {
                            resolve_typevar_reference(db, index, parent_scope, args_tv.typevar(db))
                                .is_some()
                        });
                if !paramspec_is_in_scope
                    && let Some(builder) =
                        context.report_lint(&UNBOUND_TYPE_VARIABLE, args_annotation)
                {
                    let mut diagnostic = builder.into_diagnostic(format_args!(
                        "ParamSpec `{}` is not in scope",
                        args_tv.name(db),
                    ));
                    diagnostic.annotate(
                        context
                            .secondary(kwargs_annotation)
                            .message("This component uses the same out-of-scope ParamSpec"),
                    );
                }

                if !parameters.kwonlyargs.is_empty() {
                    let name = args_tv.name(db);
                    let vararg = vararg_name.unwrap_or("args");
                    let kwarg = kwarg_name.unwrap_or("kwargs");
                    if let Some(builder) =
                        context.report_lint(&INVALID_PARAMSPEC, &parameters.kwonlyargs[0])
                    {
                        builder.into_diagnostic(format_args!(
                            "No parameters may appear between \
                                 `*{vararg}: {name}.args` and `**{kwarg}: {name}.kwargs`",
                        ));
                    }
                }
            }
        }

        (Some((args_tv, args_annotation)), None) => {
            let name = args_tv.name(db);
            let vararg = vararg_name.unwrap_or("args");
            let kwarg = kwarg_name.unwrap_or("kwargs");
            let range = if let Some(kwarg_param) = parameters.kwarg.as_deref() {
                kwarg_param
                    .annotation()
                    .map(Ranged::range)
                    .unwrap_or_else(|| kwarg_param.range())
            } else {
                args_annotation.range()
            };
            if let Some(builder) = context.report_lint(&INVALID_PARAMSPEC, range) {
                builder.into_diagnostic(format_args!(
                    "`*{vararg}: {name}.args` must be accompanied by `**{kwarg}: {name}.kwargs`",
                ));
            }
        }

        (None, Some((kwargs_tv, kwargs_annotation))) => {
            let name = kwargs_tv.name(db);
            let vararg = vararg_name.unwrap_or("args");
            let kwarg = kwarg_name.unwrap_or("kwargs");
            let range = if let Some(vararg_param) = parameters.vararg.as_deref() {
                vararg_param
                    .annotation()
                    .map(Ranged::range)
                    .unwrap_or_else(|| vararg_param.range())
            } else {
                kwargs_annotation.range()
            };
            if let Some(builder) = context.report_lint(&INVALID_PARAMSPEC, range) {
                builder.into_diagnostic(format_args!(
                    "`**{kwarg}: {name}.kwargs` must be accompanied by `*{vararg}: {name}.args`",
                ));
            } else {
                if let Some(builder) = context.report_lint(&INVALID_PARAMSPEC, kwargs_annotation) {
                    builder.into_diagnostic(format_args!(
                        "`**{kwarg}: {name}.kwargs` must be accompanied by \
                             `*{kwarg}: {name}.args`",
                    ));
                }
            }
        }

        (None, None) => {}
    }
}
