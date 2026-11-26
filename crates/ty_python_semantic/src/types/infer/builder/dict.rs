use itertools::Itertools;
use ruff_python_ast::{self as ast, HasNodeIndex};
use rustc_hash::FxHashMap;

use super::{ArgExpr, TypeInferenceBuilder};
use crate::types::typed_dict::{
    extract_unpacked_typed_dict_keys_from_value_type, infer_unpacked_keyword_types,
    validate_typed_dict_constructor,
};
use crate::types::{KnownClass, Type, TypeContext};

impl<'db> TypeInferenceBuilder<'db, '_> {
    pub(super) fn infer_keyword_only_dict_call(
        &mut self,
        func: &ast::Expr,
        arguments: &ast::Arguments,
        collection_expr: Option<ast::ExprRef<'_>>,
        call_expression_tcx: TypeContext<'db>,
    ) -> Option<Type<'db>> {
        let db = self.db();
        if !arguments.args.is_empty() {
            return None;
        }

        if let Some(tcx) = call_expression_tcx.expected_type()
            && let Some(typed_dict) = tcx
                .filter_union(db, self.program_environment(), Type::is_typed_dict)
                .as_typed_dict()
        {
            let supports_typed_dict_context = {
                let mut speculative_builder = self.speculate_without_diagnostics();
                let env = speculative_builder.program_environment();
                infer_unpacked_keyword_types(arguments, |expr, tcx| {
                    speculative_builder.infer_expression(expr, tcx)
                })
                .into_iter()
                .flatten()
                .all(|keyword_ty| {
                    keyword_ty.is_assignable_to(db, env, Type::TypedDict(typed_dict))
                        || extract_unpacked_typed_dict_keys_from_value_type(db, env, keyword_ty)
                            .is_some()
                })
            };

            if supports_typed_dict_context {
                self.infer_typed_dict_constructor_keyword_values(
                    typed_dict,
                    arguments,
                    call_expression_tcx,
                );
                validate_typed_dict_constructor(
                    &self.context,
                    typed_dict,
                    arguments,
                    func.into(),
                    |expr, _| self.expression_type(expr),
                );

                return Some(Type::TypedDict(typed_dict));
            }
        }

        if arguments
            .keywords
            .iter()
            .any(|keyword| keyword.arg.is_none())
        {
            return None;
        }

        let items = arguments
            .keywords
            .iter()
            .map(|keyword| [Some(&keyword.value), Some(&keyword.value)])
            .collect_vec();
        let keyword_names = arguments
            .keywords
            .iter()
            .filter_map(|keyword| {
                Some((
                    keyword.value.node_index().load(),
                    keyword.arg.as_ref()?.id.clone(),
                ))
            })
            .collect::<FxHashMap<_, _>>();
        let mut infer_elt_ty = |builder: &mut Self, (i, elt, tcx): ArgExpr<'db, '_>| {
            if i == 0 {
                let key = keyword_names
                    .get(&elt.node_index().load())
                    .expect("keyword-only dict() fast-path requires named keywords");
                Type::string_literal(builder.db(), key)
            } else {
                builder.infer_expression(elt, tcx)
            }
        };

        self.infer_collection_literal(
            KnownClass::Dict,
            collection_expr,
            &items,
            &mut infer_elt_ty,
            call_expression_tcx,
        )
        .or_else(|| {
            if arguments.is_empty() {
                return None;
            }

            KnownClass::Dict
                .try_to_class_literal(db, self.program_environment())
                .and_then(|class| class.generic_context(db))
                .is_none()
                .then(Type::unknown)
        })
    }
}
