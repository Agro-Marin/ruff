use super::{
    Binding, Bindings, CallableBinding, CallableItem, CheckTypesMode, generic_context_has_paramspec,
};
use crate::Db;
use crate::ProgramEnvironment;
use crate::types::call::arguments::CallArguments;
use crate::types::constraints::ConstraintSetBuilder;
use crate::types::generics::{GenericContext, Specialization};
use crate::types::signatures::Parameter;
use crate::types::typevar::TypeVarNonceGenerator;
use crate::types::{
    ApplyTypeMappingVisitor, BoundTypeVarInstance, ClassLiteral, DynamicType, Type, TypeContext,
    TypeMapping, UnionType,
};

impl<'db> CallableBinding<'db> {
    pub(crate) fn typing_self_type(&self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.bound_type.map(|bound_type| match self.signature_type {
            Type::BoundMethod(method) => method.typing_self_type(db),
            _ => bound_type,
        })
    }

    pub(crate) fn bind_unused_self(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        self_type: Type<'db>,
    ) {
        for overload in &mut self.overloads {
            if let Some(signature) = overload.signature.bind_unused_self(db, env, self_type) {
                overload.signature = signature;
                overload.return_ty = overload.initial_return_type(db);
            }
        }
    }
}

/// Bindings for a constructor call.
///
/// The `entry` is the first-called constructor method (could be a metaclass `__call__`, a
/// `__new__`, or an `__init__`, depending what is present on the constructed class). Its
/// `downstream_constructor` may link to the next downstream constructor, if present (e.g.
/// metaclass `__call__` could have `__new__` or `__init__` as downstream; `__new__` could have
/// `__init__` as downstream; `__init__` cannot have a downstream). The downstream constructor is
/// only checked if the upstream returns an instance of the class being constructed. (A downstream
/// constructor may itself have a downstream constructor, in the case where metaclass `__call__`,
/// `__new__`, and `__init__` are all present.)
#[derive(Debug, Clone, get_size2::GetSize, salsa::SalsaValue)]
pub(super) struct ConstructorBinding<'db> {
    /// The `CallableBinding` for this individual constructor method.
    pub(super) entry: CallableBinding<'db>,
    /// Context for the constructor callable: the instance type being constructed and the kind of
    /// constructor method.
    pub(super) constructor_context: ConstructorContext<'db>,
    /// The next downstream constructor method, if any, to be (conditionally) checked after this
    /// one.
    pub(super) downstream_constructor: Option<Box<Bindings<'db>>>,
}

impl<'db> ConstructorBinding<'db> {
    pub(super) fn new(
        entry: CallableBinding<'db>,
        constructor_context: ConstructorContext<'db>,
    ) -> Self {
        Self {
            entry,
            constructor_context,
            downstream_constructor: None,
        }
    }

    pub(super) fn context(&self) -> ConstructorContext<'db> {
        self.constructor_context
    }

    pub(super) fn constructed_instance_type(&self) -> Type<'db> {
        self.constructor_context.instance_type()
    }

    pub(super) fn callable(&self) -> &CallableBinding<'db> {
        &self.entry
    }

    pub(super) fn callable_mut(&mut self) -> &mut CallableBinding<'db> {
        &mut self.entry
    }

    pub(super) fn set_constructed_instance_type(&mut self, instance_type: Type<'db>) {
        self.constructor_context = self.constructor_context.with_instance_type(instance_type);
    }

    pub(super) fn set_downstream_constructor(&mut self, bindings: Bindings<'db>) {
        self.downstream_constructor = Some(Box::new(bindings));
    }

    pub(super) fn freshen_generic_contexts_in_place(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        nonce_generator: &TypeVarNonceGenerator<'db>,
    ) {
        let instance_type = self.constructed_instance_type();
        let Some((_, specialization)) = instance_type.class_specialization(db, env) else {
            return;
        };
        let generic_context = specialization.generic_context(db);
        let owns_generic_context = self.entry.overloads.iter().any(|overload| {
            overload
                .signature
                .generic_context
                .is_some_and(|owned| generic_context.is_subset_of(db, owned))
        });
        if !owns_generic_context
            || generic_context_has_paramspec(db, generic_context)
            || !nonce_generator.should_freshen(db, generic_context)
        {
            return;
        }

        let delta = nonce_generator.next().value();
        let type_mapping = TypeMapping::FreshenBoundTypeVars {
            generic_context,
            delta,
        };
        let fresh_instance_type =
            instance_type.apply_type_mapping(db, env, &type_mapping, TypeContext::default());
        if fresh_instance_type == instance_type {
            return;
        }
        self.freshen_class_typevars(db, env, generic_context, delta, fresh_instance_type);
    }

    fn freshen_class_typevars(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        generic_context: GenericContext<'db>,
        delta: u32,
        fresh_instance_type: Type<'db>,
    ) {
        let type_mapping = TypeMapping::FreshenBoundTypeVars {
            generic_context,
            delta,
        };

        let constructor_context = self.context().with_instance_type(fresh_instance_type);
        let visitor = ApplyTypeMappingVisitor::new(env);
        self.entry.bound_type = self.entry.bound_type.map(|bound_type| {
            bound_type.apply_type_mapping_impl(db, &type_mapping, TypeContext::default(), &visitor)
        });
        for overload in &mut self.entry.overloads {
            let signature_context = GenericContext::from_typevar_instances(
                db,
                env,
                generic_context.variables(db).chain(
                    overload
                        .signature
                        .generic_context
                        .into_iter()
                        .flat_map(|context| context.variables(db))
                        .filter(|typevar| typevar.typevar(db).is_self(db)),
                ),
            );
            overload.signature = overload.signature.apply_type_mapping_impl(
                db,
                &TypeMapping::FreshenBoundTypeVars {
                    generic_context: signature_context,
                    delta,
                },
                TypeContext::default(),
                &ApplyTypeMappingVisitor::new(env),
            );
            overload.set_constructor_context(db, constructor_context);
        }

        if let Some(downstream) = self.downstream_constructor_mut() {
            for downstream_binding in downstream
                .iter_callable_items_mut()
                .filter_map(CallableItem::as_constructor_mut)
            {
                downstream_binding.freshen_class_typevars(
                    db,
                    env,
                    generic_context,
                    delta,
                    fresh_instance_type,
                );
            }
        }
    }

    /// Match parameters for this constructor method and downstream constructors.
    pub(super) fn match_parameters(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        arguments: &CallArguments<'_, 'db>,
    ) {
        self.entry.match_parameters(db, env, arguments);

        if let Some(downstream) = self.downstream_constructor.as_mut() {
            downstream.match_parameters_in_place(db, env, arguments);
        }
    }

    /// Check types for all bindings in this constructor.
    ///
    /// Provisional checks preserve downstream constructors. Final checks discard inactive ones.
    pub(super) fn check_types(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        constraints: &ConstraintSetBuilder<'db>,
        argument_types: &CallArguments<'_, 'db>,
        call_expression_tcx: TypeContext<'db>,
        mode: CheckTypesMode,
    ) {
        self.entry.check_types(
            db,
            env,
            constraints,
            argument_types,
            call_expression_tcx,
            mode,
        );

        if mode.is_provisional() {
            if let Some(downstream) = self.downstream_constructor_mut() {
                let _ = downstream.check_types_impl(
                    db,
                    env,
                    constraints,
                    argument_types,
                    call_expression_tcx,
                    &[],
                    mode,
                );
            }
        } else if !self.should_check_downstream(db, env) {
            self.downstream_constructor = None;
        }
    }

    /// For constructors which may have downstreams (that is, metaclass `__call__` or `__new__`),
    /// analyze their overloads to determine whether to check downstream constructors.
    ///
    /// We analyze overloads individually rather than just relying on the resolved return type of
    /// the overall callable, because in multiple-matching-overload cases where the overload
    /// resolution algorithm might just collapse to `Unknown`, we want to make a more informed
    /// decision based on whether all overloads return instance types, or not.
    fn should_check_downstream(&self, db: &'db dyn Db, env: &ProgramEnvironment<'db>) -> bool {
        let constructor_kind = self.constructor_kind();
        if constructor_kind.is_init() || self.downstream_constructor().is_none() {
            return false;
        }

        let callable = self.callable();
        if callable.as_result().is_err() {
            return false;
        }

        let constructed_instance_type = self.constructed_instance_type();
        let constructor_class_literal = self.constructed_class_literal(db, env);

        callable.matching_overloads().any(|(_, overload)| {
            overload.return_ty == constructed_instance_type
                || constructor_class_literal.is_some_and(|class_literal| {
                    constructor_returns_instance(db, env, class_literal, overload.return_ty)
                })
        })
    }

    /// Discards an inactive downstream constructor.
    pub(super) fn discard_downstream_constructor(
        &mut self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
    ) -> bool {
        if self.should_check_downstream(db, env) {
            true
        } else {
            self.downstream_constructor = None;
            false
        }
    }

    pub(super) fn downstream_constructor(&self) -> Option<&Bindings<'db>> {
        self.downstream_constructor.as_deref()
    }

    pub(super) fn downstream_constructor_mut(&mut self) -> Option<&mut Bindings<'db>> {
        self.downstream_constructor.as_deref_mut()
    }

    pub(super) fn init_argument_matches_keyword_variadic(&self, argument_index: usize) -> bool {
        if self.constructor_kind().is_init() {
            self.entry.argument_matches_keyword_variadic(argument_index)
        } else {
            self.downstream_constructor().is_some_and(|downstream| {
                downstream.constructor_init_argument_matches_keyword_variadic(argument_index)
            })
        }
    }

    pub(super) fn map<F>(self, f: &F) -> ConstructorBinding<'db>
    where
        F: Fn(CallableBinding<'db>) -> CallableBinding<'db>,
    {
        ConstructorBinding {
            entry: f(self.entry),
            constructor_context: self.constructor_context,
            downstream_constructor: None,
        }
    }

    /// Compute the overall effective return type of this `ConstructorBinding`.
    pub(super) fn return_type(&self, db: &'db dyn Db, env: &ProgramEnvironment<'db>) -> Type<'db> {
        let constructed_instance_type = self.constructed_instance_type();

        if let Some(downstream) = self.downstream_constructor()
            && let Some(constructor_class_literal) = self.constructed_class_literal(db, env)
        {
            let downstream_return = downstream.return_type(db, env);
            if !constructor_returns_instance(db, env, constructor_class_literal, downstream_return)
            {
                return downstream_return;
            }
        }

        if let Some(return_ty) = self.explicit_return_type(db, env) {
            return return_ty;
        }

        constructed_instance_type
            .apply_optional_specialization(db, self.instance_return_specialization(db, env, None))
    }

    fn first_matching_overload(&self) -> Option<&Binding<'db>> {
        self.callable()
            .matching_overloads()
            .map(|(_, overload)| overload)
            .next()
    }

    /// Combine inferred specializations from this constructor and downstream constructors. The
    /// resulting specialization can be applied either to the constructed instance type or to an
    /// explicit `__new__` / `__call__` return annotation that is an instance of the constructed
    /// type or a subclass.
    fn instance_return_specialization(
        &self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        explicit_return: Option<&Binding<'db>>,
    ) -> Option<Specialization<'db>> {
        let constructed_instance_type = self.constructed_instance_type();
        let (class_literal, class_specialization) =
            constructed_instance_type.class_specialization(db, env)?;
        let class_context = class_specialization.generic_context(db);

        let mut combined: Option<Vec<Option<Type<'db>>>> = None;
        let mut combine_binding_types = |binding: &Self, explicit_return: Option<&Binding<'db>>| {
            let Some(overload) = explicit_return
                .or_else(|| binding.first_matching_overload())
                .or_else(|| binding.callable().unambiguous_failing_overload())
            else {
                return;
            };
            let partial_specialization = overload.inference.and_then(|inference| {
                let (_, class_specialization) = overload
                    .constructor_context?
                    .instance_type()
                    .class_specialization(db, env)?;
                Some(
                    inference.merged_specialization_with(db, |typevar, inferred| {
                        (inferred.is_none()
                            && class_specialization
                                .types(db)
                                .contains(&Type::TypeVar(typevar)))
                        .then_some(Type::Dynamic(DynamicType::UnspecializedTypeVar))
                    }),
                )
            });
            let inferred_types = |partial: Specialization<'db>, resolved: Specialization<'db>| {
                partial
                    .types(db)
                    .iter()
                    .zip(resolved.types(db))
                    .map(|(partial, resolved)| {
                        (*partial != Type::Dynamic(DynamicType::UnspecializedTypeVar))
                            .then_some(*resolved)
                    })
                    .collect::<Vec<_>>()
            };
            let project_class_arguments =
                |ty: Type<'db>, resolved_ty: Type<'db>| -> Option<Vec<Option<Type<'db>>>> {
                    let partial = ty
                        .apply_optional_specialization(db, partial_specialization)
                        .specialization_of(db, env, class_literal)?;
                    let resolved = resolved_ty.specialization_of(db, env, class_literal)?;
                    Some(inferred_types(partial, resolved))
                };
            let return_types = if explicit_return.is_some() {
                let instance = overload
                    .constructor_context
                    .unwrap_or(binding.context())
                    .instance_type();
                project_class_arguments(
                    instance,
                    instance.apply_optional_specialization(db, overload.merged_specialization(db)),
                )
            } else {
                project_class_arguments(overload.unspecialized_return_type(db), overload.return_ty)
            };

            let return_types_are_informative = return_types.as_ref().is_some_and(|types| {
                types
                    .iter()
                    .flatten()
                    .any(|mapped_ty| !mapped_ty.is_unknown())
            });
            let types = if return_types_are_informative {
                return_types
            } else {
                let self_parameter_types = (|| {
                    let self_param_ty = overload.signature.parameters().get(0)?.annotated_type();
                    let resolved_self_param_ty = self_param_ty
                        .apply_optional_specialization(db, overload.merged_specialization(db));
                    let mut types = project_class_arguments(self_param_ty, resolved_self_param_ty)?;
                    for mapped_ty in types.iter_mut().flatten() {
                        let without_unknown =
                            mapped_ty.filter_union(db, env, |element| !element.is_unknown());
                        if !without_unknown.is_never() {
                            *mapped_ty = without_unknown;
                        }
                        *mapped_ty = mapped_ty.promote(db, env);
                    }
                    Some(types)
                })();
                self_parameter_types.or(return_types).or_else(|| {
                    let resolved = overload.merged_specialization(db)?;
                    Some(inferred_types(
                        partial_specialization
                            .unwrap_or(resolved)
                            .restrict(db, class_context)?,
                        resolved.restrict(db, class_context)?,
                    ))
                })
            };

            let Some(types) = types else {
                return;
            };
            if let Some(previous) = &mut combined {
                for (previous, inferred) in previous.iter_mut().zip(types) {
                    *previous = match (*previous, inferred) {
                        (Some(previous), Some(inferred)) => {
                            Some(UnionType::from_two_elements(db, env, previous, inferred))
                        }
                        (previous, inferred) => previous.or(inferred),
                    };
                }
            } else {
                combined = Some(types);
            }
        };

        combine_binding_types(self, explicit_return);

        if let Some(downstream) = self.downstream_constructor() {
            for downstream_binding in downstream
                .iter_callable_items()
                .filter_map(CallableItem::as_constructor)
            {
                combine_binding_types(downstream_binding, None);
            }
        }

        combined.map(|types| {
            class_context
                .specialize_partial(db, types)
                .apply_optional_specialization(db, Some(class_specialization))
        })
    }

    /// Compute the explicit return type from a `__new__` or metaclass `__call__`.
    ///
    /// This method is only used for `__new__` and metaclass `__call__`, which (unlike `__init__`)
    /// can have explicit return types that determine the result of the constructor call.
    ///
    /// Returning `None` means "no explicit return type override, just construct an instance of the
    /// constructed class; default constructor behavior."
    ///
    /// This must be called only after downstream constructor bindings have been type-checked,
    /// because instance-returning constructor paths may incorporate downstream specializations.
    fn explicit_return_type(
        &self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
    ) -> Option<Type<'db>> {
        if self.constructor_kind().is_init() || self.constructed_class_literal(db, env).is_none() {
            return None;
        }

        let matching_overloads = self
            .callable()
            .matching_overloads()
            .map(|(_, overload)| overload);

        if matching_overloads.clone().next().is_none() {
            self.analyze_overload_returns(db, env, self.callable().overloads().iter())
        } else {
            self.analyze_overload_returns(db, env, matching_overloads)
        }
    }

    /// Combine return types from an iterator of overloads to determine the effective explicit
    /// return type of the constructor call. See `explicit_return_type` for details.
    fn analyze_overload_returns<'a>(
        &self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        overloads: impl IntoIterator<Item = &'a Binding<'db>>,
    ) -> Option<Type<'db>>
    where
        'db: 'a,
    {
        let mut sole_instance_return = None;
        let mut saw_instance_return = false;
        let mut non_instance_return = None;
        for overload in overloads {
            let (return_ty, is_instance_return) = self.single_overload_return(db, env, overload);
            if is_instance_return {
                if saw_instance_return {
                    sole_instance_return = None;
                } else {
                    sole_instance_return = Some(return_ty);
                    saw_instance_return = true;
                }
            } else {
                non_instance_return = Some(match non_instance_return {
                    None => return_ty,
                    Some(previous) if previous == return_ty => return_ty,
                    Some(_) => Type::unknown(),
                });
            }
        }
        if let Some(non_instance_return) = non_instance_return {
            if saw_instance_return {
                Some(Type::unknown())
            } else {
                Some(non_instance_return)
            }
        } else {
            sole_instance_return
        }
    }

    /// Compute the effective return type for the given constructor overload. This differs from the
    /// ordinary return type in that, if the overload returns an instance type, we apply a broader
    /// specialization derived (possibly) also from downstream constructors.
    ///
    /// Return a tuple of `(return_type, is_instance_return)`.
    fn single_overload_return(
        &self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        overload: &Binding<'db>,
    ) -> (Type<'db>, bool) {
        let return_ty = overload
            .unspecialized_return_type(db)
            .apply_optional_specialization(
                db,
                overload.merged_specialization(db).map(|specialization| {
                    Self::unspecialize_class_type_variables(db, env, overload, specialization)
                }),
            );
        if self
            .constructed_class_literal(db, env)
            .is_some_and(|class_literal| {
                constructor_returns_instance(db, env, class_literal, return_ty)
            })
        {
            return (
                return_ty.apply_optional_specialization(
                    db,
                    self.instance_return_specialization(db, env, Some(overload)),
                ),
                true,
            );
        }

        (overload.return_ty, false)
    }

    /// "Un-specialize" class-level type variables in an overload specialization.
    ///
    /// Per-overload specialization may contain defaulted (typically `Unknown`) solutions for the
    /// constructed class's own type variables. That is fine for parameter checking, but when
    /// inferring a type for a constructed instance, we need to also consider other sources of
    /// specialization, such as downstream constructors, but we lose the class type variables
    /// before the constructor-wide specialization can refine them. To avoid that, this helper
    /// maps the call's class type variables back to their declaration occurrences, where the
    /// constructor-wide specialization can replace them, while preserving specializations of
    /// method-level type parameters.
    ///
    /// TODO: This could be made simpler if we more clearly marked unsolved typevars in a
    /// specialization; we could probably avoid this entirely and just combine the specializations.
    fn unspecialize_class_type_variables(
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        overload: &Binding<'db>,
        specialization: Specialization<'db>,
    ) -> Specialization<'db> {
        let Some((_, class_specialization)) = overload
            .constructor_context
            .and_then(|context| context.instance_type().class_specialization(db, env))
        else {
            return specialization;
        };

        let types: Box<[_]> = specialization
            .types(db)
            .iter()
            .copied()
            .zip(specialization.generic_context(db).variables(db))
            .map(|(mapped_ty, typevar)| {
                class_specialization
                    .types(db)
                    .iter()
                    .zip(class_specialization.generic_context(db).variables(db))
                    .find_map(|(argument, parameter)| {
                        (argument.as_typevar()?.identity(db) == typevar.identity(db))
                            .then_some(Type::TypeVar(parameter))
                    })
                    .unwrap_or(mapped_ty)
            })
            .collect();

        Specialization::new(
            db,
            specialization.generic_context(db),
            types,
            specialization.materialization_kind(db),
            None,
        )
    }

    fn constructed_class_literal(
        &self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
    ) -> Option<ClassLiteral<'db>> {
        let instance_type = self.constructed_instance_type();
        let lookup_instance = match instance_type {
            Type::Intersection(_) => instance_type.flatten_typevars(db, env),
            _ => instance_type,
        };
        lookup_instance
            .as_nominal_instance()
            .map(|instance| instance.class(db, env).class_literal(db))
    }

    fn constructor_kind(&self) -> ConstructorCallableKind {
        self.constructor_context.kind()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, get_size2::GetSize, salsa::SalsaValue)]
pub(super) struct ConstructorContext<'db> {
    instance_type: Type<'db>,
    kind: ConstructorCallableKind,
}

impl<'db> ConstructorContext<'db> {
    pub(super) fn new(instance_type: Type<'db>, kind: ConstructorCallableKind) -> Self {
        Self {
            instance_type,
            kind,
        }
    }

    fn with_instance_type(self, instance_type: Type<'db>) -> Self {
        Self {
            instance_type,
            ..self
        }
    }

    fn instance_type(self) -> Type<'db> {
        self.instance_type
    }

    pub(super) fn kind(self) -> ConstructorCallableKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, get_size2::GetSize)]
pub(crate) enum ConstructorCallableKind {
    /// A metaclass `__call__` method.
    MetaclassCall,
    /// A `__new__` constructor.
    New,
    /// An `__init__` method.
    Init,
}

impl ConstructorCallableKind {
    fn is_init(self) -> bool {
        matches!(self, ConstructorCallableKind::Init)
    }
}

/// Classify a return type as either being an instance of the given `class_literal` or not, for
/// purposes of deciding whether downstream constructors should be checked. Some cases are obvious,
/// some are judgment calls (and we follow the judgment of the typing spec). For example, an
/// explicit `Any` is considered "not an instance", but an `Unknown` is considered "an instance".
fn constructor_returns_instance<'db>(
    db: &'db dyn Db,
    env: &ProgramEnvironment<'db>,
    class_literal: ClassLiteral<'db>,
    return_ty: Type<'db>,
) -> bool {
    match return_ty.resolve_type_alias(db) {
        Type::Union(union) => union
            .elements(db)
            .iter()
            .all(|element| constructor_returns_instance(db, env, class_literal, *element)),
        Type::Intersection(intersection) => intersection
            .iter_positive(db)
            .any(|element| constructor_returns_instance(db, env, class_literal, element)),
        Type::Dynamic(DynamicType::Any) => false,
        Type::Dynamic(_) => true,
        Type::Never => false,
        Type::NominalInstance(instance) => instance
            .class(db, env)
            .is_subtype_of_class_literal(db, class_literal),
        _ => false,
    }
}

impl<'db> Binding<'db> {
    /// Is a type variable returned from a constructor method a representation of the self type?
    ///
    /// Handles `typing.Self` annotations and `__new__` methods returning `T` where `self:
    /// type[T]`.
    fn is_self_like_constructor_return_typevar(
        &self,
        db: &'db dyn Db,
        return_typevar: BoundTypeVarInstance<'db>,
    ) -> bool {
        if return_typevar.typevar(db).is_self(db) {
            return true;
        }

        let Some(cls_parameter_ty) = self
            .signature
            .parameters()
            .get(0)
            .map(Parameter::annotated_type)
        else {
            return false;
        };

        let Type::SubclassOf(subclass_of) = cls_parameter_ty.resolve_type_alias(db) else {
            return false;
        };
        let Some(cls_typevar) = subclass_of.into_type_var() else {
            return false;
        };

        cls_typevar.typevar(db).identity(db) == return_typevar.typevar(db).identity(db)
    }

    pub(super) fn set_constructor_context(
        &mut self,
        db: &'db dyn Db,
        constructor_context: ConstructorContext<'db>,
    ) {
        self.constructor_context = Some(constructor_context);
        self.return_ty = self.initial_return_type(db);
    }

    pub(super) fn initial_return_type(&self, db: &'db dyn Db) -> Type<'db> {
        self.unspecialized_return_type(db)
    }

    /// Return the declared return type after constructor normalization, but before applying any
    /// specialization inferred for this overload.
    pub(super) fn unspecialized_return_type(&self, db: &'db dyn Db) -> Type<'db> {
        self.normalized_constructor_return(db)
            .unwrap_or(self.signature.return_ty)
    }

    /// Normalize constructor return type. There are a few special cases we have to handle for
    /// constructors:
    ///
    ///   * `__init__` methods always return `None`, but for the purposes of type inference we want
    ///     to treat them as returning the constructed instance type.
    ///
    ///   * If a `__new__` method (or metaclass `__call__`) has no annotated return type (or is
    ///     annotated with an unknown return type), treat it as returning the constructed instance
    ///     type.
    ///
    ///   * If a `__new__` method returns `typing.Self` or `T` where the first parameter to
    ///     `__new__` is annotated as `type[T]`, replace it with the instance type.
    ///
    /// Although these cases should be resolved correctly later by the specialization machinery, we
    /// need to unwrap these early in case the constructed instance type is generic. Literal
    /// promotion and reverse inference from type context need to be able to see into the generic
    /// instance type.
    ///
    /// Return `None` if this is not a constructor call.
    fn normalized_constructor_return(&self, db: &'db dyn Db) -> Option<Type<'db>> {
        let constructor_context = self.constructor_context?;
        let instance_type = constructor_context.instance_type();

        match (
            constructor_context.kind(),
            self.signature.return_ty.resolve_type_alias(db),
        ) {
            (ConstructorCallableKind::Init, _) => Some(instance_type),
            (_, ty) if ty.is_unknown() => Some(instance_type),
            (ConstructorCallableKind::New, Type::TypeVar(typevar))
                if self.is_self_like_constructor_return_typevar(db, typevar) =>
            {
                Some(instance_type)
            }
            _ => Some(self.signature.return_ty),
        }
    }
}
