use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{
    ImportingFile, Module, ModuleName, ModuleNameResolutionError, ModuleResolveMode,
    resolve_module, search_paths,
};

use crate::{
    TypeQualifiers, add_inferred_python_version_hint_to_diagnostic,
    dependency::{DependencyProjectKind, missing_direct_dependency},
    place::{DefinedPlace, Definedness, Place, PlaceAndQualifiers, Provenance, TypeOrigin},
    reachability::evaluate_reachability_with_cache,
    types::{
        DeclaredType, ModuleLiteralType, Type,
        diagnostic::{
            IMPORT_NOT_FOUND, MISSING_DIRECT_DEPENDENCY, MISSING_MODULE_MEMBER,
            POSSIBLY_MISSING_IMPORT, hint_if_stdlib_attribute_exists_on_other_versions,
            hint_if_stdlib_submodule_exists_on_other_versions,
        },
        infer::TypeInferenceBuilder,
        infer_definition_types,
    },
};
use ty_python_core::definition::Definition;

impl<'db, 'ast> TypeInferenceBuilder<'db, 'ast> {
    /// Binds an imported value without declaring its type, while preserving inherited `Final`
    /// metadata.
    ///
    /// An import does not itself constrain later assignments. Retaining the source type and
    /// qualifier for imported `Final` values lets the dedicated use-def queries preserve them
    /// through re-exports and reject later reassignment:
    ///
    /// ```python
    /// # values.py
    /// from typing import Final
    /// VALUE: Final[int] = 1
    ///
    /// # consumer.py
    /// from values import VALUE
    /// VALUE = 2  # invalid-assignment
    /// ```
    fn add_imported_binding(
        &mut self,
        alias: &'ast ast::Alias,
        definition: Definition<'db>,
        ty: Type<'db>,
        qualifiers: TypeQualifiers,
        provenance: Provenance<'db>,
    ) {
        if definition.kind(self.db()).as_star_import().is_none() {
            self.check_deprecated(alias, ty);
        }

        self.add_binding(alias.into(), definition).insert(self, ty);

        if qualifiers.contains(TypeQualifiers::FINAL) {
            self.declarations.insert(
                definition,
                DeclaredType::new(Some(ty), TypeOrigin::Declared, qualifiers)
                    .with_provenance(provenance),
            );
        }
    }

    pub(super) fn infer_import_statement(&mut self, import: &ast::StmtImport) {
        let ast::StmtImport {
            names,
            is_lazy: _,
            range: _,
            node_index: _,
        } = import;

        for alias in names {
            self.infer_definition(alias);
        }
    }

    fn check_direct_dependency(&self, module: Module<'db>, range: TextRange) {
        if !self.context.is_lint_enabled(&MISSING_DIRECT_DEPENDENCY)
            || self.in_stub()
            || self.is_in_type_checking_block(self.scope(), range)
            || self
                .settings()
                .replace_imports_with_any
                .matches(module.name(self.db()))
                .is_include()
        {
            return;
        }

        let db = self.db();
        let Some(missing) = missing_direct_dependency(db, self.program_file(), module) else {
            return;
        };

        let Some(builder) = self.context.report_lint(&MISSING_DIRECT_DEPENDENCY, range) else {
            return;
        };

        let mut diagnostic = builder.into_diagnostic(format_args!(
            "Import of `{}` requires a direct dependency on `{}`",
            module.name(db),
            missing.distribution_name,
        ));
        match missing.project_kind {
            DependencyProjectKind::Project => diagnostic.help(format_args!(
                "Declare `{}` in `project.dependencies` or `project.optional-dependencies` in your `pyproject.toml`",
                missing.distribution_name,
            )),
            DependencyProjectKind::Script => diagnostic.help(format_args!(
                "Declare `{}` in the script's inline `dependencies` metadata",
                missing.distribution_name,
            )),
        }
        if missing.group_dependency {
            diagnostic.info("Dependency groups are only available to non-package files");
        }
        diagnostic.info(match missing.project_kind {
            DependencyProjectKind::Project => {
                "See https://docs.astral.sh/uv/concepts/projects/dependencies/"
            }
            DependencyProjectKind::Script => {
                "See https://docs.astral.sh/uv/guides/scripts/#declaring-script-dependencies"
            }
        });
    }

    fn report_import_not_found(
        &self,
        range: TextRange,
        level: u32,
        module: Option<&str>,
        module_name: Option<&ModuleName>,
    ) {
        let db = self.db();

        if let Some(module_name) = &module_name
            && (self
                .settings()
                .allowed_unresolved_imports
                .matches(module_name)
                .is_include()
                || self
                    .settings()
                    .replace_imports_with_any
                    .matches(module_name)
                    .is_include())
        {
            return;
        }

        let Some(builder) = self.context.report_lint(&IMPORT_NOT_FOUND, range) else {
            return;
        };

        let mut diagnostic = builder.into_diagnostic(format_args!(
            "Cannot resolve imported module `{}`",
            format_import_from_module(level, module)
        ));

        if level == 0 {
            if let Some(module_name) = module_name {
                let resolver_environment = self.program_environment().program(db);
                let typeshed_versions = resolver_environment.search_paths(db).typeshed_versions();

                for module_name in module_name.ancestors() {
                    if let Some(version_range) = typeshed_versions.exact(&module_name) {
                        let python_version = self.program_environment().python_version(db);
                        if !version_range.contains(python_version) {
                            diagnostic.info(format_args!(
                                "The stdlib module `{module_name}` is only available on Python {version_range}",
                                version_range = version_range.diagnostic_display(),
                            ));
                            add_inferred_python_version_hint_to_diagnostic(
                                db,
                                self.file(),
                                &mut diagnostic,
                                "resolving modules",
                            );
                            return;
                        }
                        break;
                    }
                }
            }
        } else {
            let importing_file = ImportingFile::File(
                self.file(),
                self.program_environment().resolver_environment(db),
            );
            if let Some(better_level) = (0..level).rev().find(|reduced_level| {
                let Ok(module_name) =
                    ModuleName::from_identifier_parts(db, importing_file, module, *reduced_level)
                else {
                    return false;
                };
                resolve_module(db, importing_file, &module_name).is_some()
            }) {
                diagnostic
                    .help("The module can be resolved if the number of leading dots is reduced");
                diagnostic.help(format_args!(
                    "Did you mean `{}`?",
                    format_import_from_module(better_level, module)
                ));
                diagnostic.set_concise_message(format_args!(
                    "Cannot resolve imported module `{}` - did you mean `{}`?",
                    format_import_from_module(level, module),
                    format_import_from_module(better_level, module)
                ));
            }
        }

        let verbose = db.verbose();
        let search_paths = search_paths(
            db,
            self.program_environment().resolver_environment(db),
            ModuleResolveMode::Typing,
        );

        diagnostic.info(format_args!(
            "Searched in the following paths during module resolution:"
        ));

        let mut search_paths = search_paths.enumerate().peekable();

        while let Some((index, path)) = search_paths.next() {
            if index > 4 && !verbose && search_paths.peek().is_some() {
                let more = search_paths.count() + 1;
                diagnostic.info(format_args!(
                    "  ... and {more} more paths. Run with `-v` to see all paths."
                ));
                break;
            }
            diagnostic.info(format_args!(
                "  {}. {} ({})",
                index + 1,
                path,
                path.describe_kind()
            ));
        }

        diagnostic.info(
            "make sure your Python environment is properly configured: \
                https://docs.astral.sh/ty/modules/#python-environment",
        );
    }

    pub(super) fn infer_import_definition(
        &mut self,
        alias: &'ast ast::Alias,
        definition: Definition<'db>,
    ) {
        let ast::Alias {
            range: _,
            node_index: _,
            name,
            asname,
        } = alias;

        let Some(full_module_name) = ModuleName::new(name) else {
            tracing::debug!("Failed to resolve import due to invalid syntax");
            self.add_binding(alias.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        if self
            .settings()
            .replace_imports_with_any
            .matches(&full_module_name)
            .is_include()
        {
            self.add_binding(alias.into(), definition)
                .insert(self, Type::any());
            return;
        }

        let Some(full_module_ty) = self.module_type_from_name(&full_module_name) else {
            self.report_import_not_found(alias.range(), 0, Some(name), Some(&full_module_name));
            self.add_binding(alias.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        if let Type::ModuleLiteral(module) = full_module_ty {
            self.check_direct_dependency(module.module(self.db()), alias.range());
        }

        let binding_ty = if asname.is_some() {
            full_module_ty
        } else if full_module_name.contains('.') {
            let topmost_parent_name =
                ModuleName::new(full_module_name.components().next().unwrap()).unwrap();
            let Some(topmost_parent_ty) = self.module_type_from_name(&topmost_parent_name) else {
                self.add_binding(alias.into(), definition)
                    .insert(self, Type::unknown());
                return;
            };
            topmost_parent_ty
        } else {
            full_module_ty
        };

        self.add_binding(alias.into(), definition)
            .insert(self, binding_ty);
    }

    pub(super) fn infer_import_from_statement(&mut self, import: &ast::StmtImportFrom) {
        let ast::StmtImportFrom {
            module: _,
            names,
            level: _,
            is_lazy: _,
            range: _,
            node_index: _,
        } = import;

        let db = self.db();

        let module = self.check_import_from_module_is_resolvable(import);
        let import_range = import.module.as_ref().map_or(import.range(), Ranged::range);

        for alias in names {
            let mut checked_dependency = false;
            for definition in self.index.definitions(alias) {
                if let Some(star_import) = definition.kind(db).as_star_import() {
                    let use_def = self.index.use_def_map(self.scope().file_scope_id(db));
                    if let Some(binding) = use_def
                        .reachable_symbol_bindings(star_import.symbol_id())
                        .find(|binding| {
                            binding
                                .binding
                                .is_defined_and(|candidate| candidate == *definition)
                        })
                        && evaluate_reachability_with_cache(
                            db,
                            Some(self.reachability_cache()),
                            use_def.reachability_constraints(),
                            use_def.predicates(),
                            binding.reachability_constraint,
                        )
                        .is_always_false()
                    {
                        continue;
                    }
                }

                let inferred = infer_definition_types(self.db(), *definition);
                if definition.kind(db).as_star_import().is_none() {
                    for (_, ty) in inferred.bindings(*definition) {
                        // decision, and do not follow values re-exported from unrelated modules.
                        if self.context.is_lint_enabled(&MISSING_DIRECT_DEPENDENCY)
                            && let Some(parent) = module
                        {
                            let imported_module = if let Type::ModuleLiteral(literal) = ty
                                && let child = literal.module(db)
                                && let child_name = child.name(db)
                                && child_name.parent().as_ref() == Some(parent.name(db))
                                && child_name.components().next_back() == Some(alias.name.as_str())
                            {
                                child
                            } else {
                                parent
                            };
                            self.check_direct_dependency(imported_module, import_range);
                            checked_dependency = true;
                        }
                    }
                }
                self.extend_definition(*definition, inferred);
            }

            if !checked_dependency && let Some(parent) = module {
                self.check_direct_dependency(parent, import_range);
            }
        }
    }

    /// Resolve and return the module referred to by the `from` clause of an
    /// [`ast::StmtImportFrom`] node. For `from package import child`, this returns
    /// `package`, not `child`. Relative imports are resolved to an absolute module name.
    ///
    /// Return `None` if the module name is invalid or the module cannot be resolved.
    /// Emit an import-not-found diagnostic for resolution failures; syntax errors are
    /// reported elsewhere.
    fn check_import_from_module_is_resolvable(
        &mut self,
        import_from: &ast::StmtImportFrom,
    ) -> Option<Module<'db>> {
        let ast::StmtImportFrom { module, level, .. } = import_from;

        let db = self.db();

        let module_ref = module
            .as_ref()
            .map(ast::AnyNodeRef::from)
            .unwrap_or_else(|| ast::AnyNodeRef::from(import_from));
        let module = module.as_deref();

        tracing::trace!(
            "Resolving import statement from module `{}` into file `{}`",
            format_import_from_module(*level, module),
            self.file().path(db),
        );
        let importing_file = ImportingFile::File(
            self.file(),
            self.program_environment().resolver_environment(db),
        );
        let module_name = ModuleName::from_import_statement(db, importing_file, import_from);

        let module_name = match module_name {
            Ok(module_name) => module_name,
            Err(ModuleNameResolutionError::InvalidSyntax) => {
                tracing::debug!("Failed to resolve import due to invalid syntax");
                return None;
            }
            Err(ModuleNameResolutionError::TooManyDots) => {
                tracing::debug!(
                    "Relative module resolution `{}` failed: too many leading dots",
                    format_import_from_module(*level, module),
                );
                self.report_import_not_found(module_ref.range(), *level, module, None);
                return None;
            }
            Err(ModuleNameResolutionError::UnknownCurrentModule) => {
                tracing::debug!(
                    "Relative module resolution `{}` failed: could not resolve file `{}` to a module \
                    (try adjusting configured search paths?)",
                    format_import_from_module(*level, module),
                    self.file().path(db)
                );
                self.report_import_not_found(module_ref.range(), *level, module, None);
                return None;
            }
        };

        let resolved = resolve_module(db, importing_file, &module_name);
        if resolved.is_none() {
            self.report_import_not_found(module_ref.range(), *level, module, Some(&module_name));
        }

        resolved
    }

    pub(super) fn infer_import_from_definition(
        &mut self,
        import_from: &ast::StmtImportFrom,
        alias: &'ast ast::Alias,
        definition: Definition<'db>,
    ) {
        let db = self.db();

        let importing_file = ImportingFile::File(
            self.file(),
            self.program_environment().resolver_environment(db),
        );
        let Ok(module_name) = ModuleName::from_import_statement(db, importing_file, import_from)
        else {
            self.add_binding(alias.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        if self
            .settings()
            .replace_imports_with_any
            .matches(&module_name)
            .is_include()
        {
            self.add_binding(alias.into(), definition)
                .insert(self, Type::any());
            return;
        }

        let Some(module) = resolve_module(db, importing_file, &module_name) else {
            self.add_binding(alias.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        let module_literal = ModuleLiteralType::new(
            db,
            module,
            module.kind(db).is_package().then_some(self.program_file()),
        );
        let module_ty = Type::ModuleLiteral(module_literal);

        let name = if let Some(star_import) = definition.kind(db).as_star_import() {
            self.index
                .place_table(self.scope().file_scope_id(db))
                .symbol(star_import.symbol_id())
                .name()
        } else {
            &alias.name.id
        };

        // at the module-global scope, where the import definition itself is one of the
        let skip_self_referential_member_lookup = Some(self.file())
            == module_literal.module(db).file(db)
            && self.scope().file_scope_id(db).is_global();

        let mut from_module_getattr = None;

        if !skip_self_referential_member_lookup {
            let result = module_literal.static_member(db, self.program_environment(), name);
            let error = result.err();
            if let PlaceAndQualifiers {
                place:
                    Place::Defined(DefinedPlace {
                        ty,
                        definedness: boundness,
                        provenance: source_provenance,
                        ..
                    }),
                qualifiers,
            } = result
                .unwrap_or_else(|error| error.fallback_member(db))
                .member(db)
            {
                if &alias.name != "*" && boundness == Definedness::PossiblyUndefined {
                    if let Some(builder) = self
                        .context
                        .report_lint(&POSSIBLY_MISSING_IMPORT, ast::AnyNodeRef::Alias(alias))
                    {
                        builder.into_diagnostic(format_args!(
                            "Member `{name}` of module `{module_name}` may be missing",
                        ));
                    }
                }
                if qualifiers.contains(TypeQualifiers::FROM_MODULE_GETATTR) {
                    from_module_getattr = Some((ty, qualifiers, source_provenance, error));
                } else {
                    self.add_imported_binding(alias, definition, ty, qualifiers, source_provenance);
                    return;
                }
            }
        }

        let full_submodule_name = ModuleName::new(name).map(|final_part| {
            let mut ret = module_name.clone();
            ret.extend(&final_part);
            ret
        });

        if let Some(submodule_type) = full_submodule_name
            .as_ref()
            .and_then(|submodule_name| self.module_type_from_name(submodule_name))
        {
            self.add_binding(alias.into(), definition)
                .insert(self, submodule_type);
            return;
        }

        if let Some((ty, qualifiers, source_provenance, error)) = from_module_getattr {
            if let Some(error) = error {
                error.report_module_getattr_import_diagnostic(
                    &self.context,
                    module_literal,
                    alias,
                    name,
                );
            }
            self.add_imported_binding(alias, definition, ty, qualifiers, source_provenance);
            return;
        }

        self.add_binding(alias.into(), definition)
            .insert(self, Type::unknown());

        if &alias.name == "*" {
            return;
        }

        if self
            .settings()
            .allowed_unresolved_imports
            .matches(full_submodule_name.as_ref().unwrap_or(&module_name))
            .is_include()
        {
            return;
        }

        let Some(builder) = self
            .context
            .report_lint(&MISSING_MODULE_MEMBER, ast::AnyNodeRef::Alias(alias))
        else {
            return;
        };

        let mut diagnostic = builder.into_diagnostic(format_args!(
            "Module `{module_name}` has no member `{name}`"
        ));

        let mut submodule_hint_added = false;

        if let Some(full_submodule_name) = full_submodule_name {
            submodule_hint_added = hint_if_stdlib_submodule_exists_on_other_versions(
                db,
                self.file(),
                self.program_environment(),
                &mut diagnostic,
                &full_submodule_name,
                module,
            );
        }

        if !submodule_hint_added {
            hint_if_stdlib_attribute_exists_on_other_versions(
                db,
                self.program_file(),
                diagnostic,
                module_ty,
                name,
                "resolving imports",
            );
        }
    }

    /// Infer the implicit local definition `x = <module 'whatever.thispackage.x'>` that
    /// `from .x.y import z` or `from whatever.thispackage.x.y` can introduce in `__init__.py(i)`.
    ///
    /// For the definition `z`, see [`TypeInferenceBuilder::infer_import_from_definition`].
    ///
    /// The runtime semantic of this kind of statement is to introduce a variable in the global
    /// scope of this module *the first time it's imported in the entire program*. This
    /// implementation just blindly introduces a local variable wherever the `from..import` is
    /// (if the imports actually resolve).
    ///
    /// That gap between the semantics and implementation are currently the responsibility of the
    /// code that actually creates these kinds of Definitions (so blindly introducing a local
    /// is all we need to be doing here).
    pub(super) fn infer_import_from_submodule_definition(
        &mut self,
        import_from: &'ast ast::StmtImportFrom,
        definition: Definition<'db>,
    ) {
        let db = self.db();
        let importing_file = ImportingFile::File(
            self.file(),
            self.program_environment().resolver_environment(db),
        );

        let Ok(thispackage_name) = ModuleName::package_for_file(db, importing_file) else {
            self.add_binding(import_from.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        let Some(module) = resolve_module(db, importing_file, &thispackage_name) else {
            self.add_binding(import_from.into(), definition)
                .insert(self, Type::unknown());
            return;
        };


        let Some(final_part) = ModuleName::from_identifier_parts(
            db,
            importing_file,
            import_from.module.as_deref(),
            import_from.level,
        )
        .ok()
        .and_then(|submodule_name| submodule_name.relative_to(&thispackage_name))
        .and_then(|relative_submodule_name| {
            relative_submodule_name
                .components()
                .next()
                .and_then(ModuleName::new)
        }) else {
            self.add_binding(import_from.into(), definition)
                .insert(self, Type::unknown());
            return;
        };

        let mut full_submodule_name = thispackage_name.clone();
        full_submodule_name.extend(&final_part);

        if let Some(submodule_type) = self.module_type_from_name(&full_submodule_name) {
            self.add_binding(import_from.into(), definition)
                .insert(self, submodule_type);
            return;
        }

        self.add_binding(import_from.into(), definition)
            .insert(self, Type::unknown());

        if self
            .settings()
            .allowed_unresolved_imports
            .matches(&full_submodule_name)
            .is_include()
        {
            return;
        }

        let Some(builder) = self.context.report_lint(
            &IMPORT_NOT_FOUND,
            ast::AnyNodeRef::StmtImportFrom(import_from),
        ) else {
            return;
        };

        let mut diagnostic = builder.into_diagnostic(format_args!(
            "Module `{thispackage_name}` has no submodule `{final_part}`"
        ));

        hint_if_stdlib_submodule_exists_on_other_versions(
            self.db(),
            self.file(),
            self.program_environment(),
            &mut diagnostic,
            &full_submodule_name,
            module,
        );
    }
}

fn format_import_from_module(level: u32, module: Option<&str>) -> String {
    format!(
        "{}{}",
        ".".repeat(level as usize),
        module.unwrap_or_default()
    )
}
