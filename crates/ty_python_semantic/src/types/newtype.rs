use crate::Db;
use crate::ProgramEnvironment;
use crate::types::constraints::ConstraintSet;
use crate::types::relation::{DisjointnessChecker, TypeRelation, TypeRelationChecker};
use crate::types::{ClassType, KnownUnion, Type, definition_expression_type, visitor};
use ruff_db::parsed::parsed_module;
use ruff_python_ast::{self as ast};
use rustc_hash::FxHashSet;
use ty_python_core::definition::{Definition, DefinitionKind};

/// A `typing.NewType` declaration, either from the perspective of the
/// identity-callable-that-acts-like-a-subtype-in-type-expressions returned by the call to
/// `typing.NewType(...)`, or from the perspective of instances of that subtype returned by the
/// identity callable. For example:
///
/// ```py
/// import typing
/// Foo = typing.NewType("Foo", int)
/// x = Foo(42)
/// ```
///
/// The revealed types there are:
/// - `typing.NewType`: `Type::ClassLiteral(ClassLiteral)` with `KnownClass::NewType`.
/// - `Foo`: `Type::KnownInstance(KnownInstanceType::NewType(NewType { .. }))`
/// - `x`: `Type::NewTypeInstance(NewType { .. })`
#[salsa::interned(debug, heap_size=ruff_memory_usage::heap_size)]
pub struct NewType<'db> {
    /// The name of this NewType (e.g. `"Foo"`)
    #[returns(ref)]
    pub name: ast::name::Name,

    /// The binding where this NewType is first created.
    #[returns(copy)]
    pub definition: Definition<'db>,

    #[returns(copy)]
    eager_base: Option<NewTypeBase<'db>>,
}

impl get_size2::GetSize for NewType<'_> {}

#[salsa::tracked]
impl<'db> NewType<'db> {
    pub fn base(self, db: &'db dyn Db) -> NewTypeBase<'db> {
        match self.eager_base(db) {
            Some(base) => base,
            None => self.lazy_base(db),
        }
    }

    #[salsa::tracked(
        returns(copy),
        cycle_initial=|db, _, self_: NewType<'db>| NewTypeBase::ClassType(ClassType::object(
            db,
            &ProgramEnvironment::from_definition(self_.definition(db)),
        )),
        heap_size=ruff_memory_usage::heap_size
    )]
    fn lazy_base(self, db: &'db dyn Db) -> NewTypeBase<'db> {
        let definition = self.definition(db);
        let program_file = definition.program_file(db);
        let python_file = program_file.python_file(db);
        let env = ProgramEnvironment::from_file(program_file);
        let object_fallback = NewTypeBase::ClassType(ClassType::object(db, &env));
        let module = parsed_module(db, python_file).load(db);
        let DefinitionKind::Assignment(assignment) = definition.kind(db) else {
            return object_fallback;
        };
        let Some(call_expr) = assignment.value(&module).as_call_expr() else {
            return object_fallback;
        };
        let Some(second_arg) = call_expr.arguments.args.get(1) else {
            return object_fallback;
        };
        match definition_expression_type(db, definition, second_arg) {
            Type::NominalInstance(nominal_instance_type) => {
                NewTypeBase::ClassType(nominal_instance_type.class(db, &env))
            }
            Type::NewTypeInstance(newtype) => NewTypeBase::NewType(newtype),
            Type::Union(union_type) => match union_type.known(db) {
                Some(KnownUnion::Float) => NewTypeBase::Float,
                Some(KnownUnion::Complex) => NewTypeBase::Complex,
                _ => object_fallback,
            },
            _ => object_fallback,
        }
    }

    fn iter_bases(self, db: &'db dyn Db) -> NewTypeBaseIter<'db> {
        NewTypeBaseIter {
            current: Some(self),
            seen_before: FxHashSet::default(),
            db,
        }
    }

    pub fn concrete_base_type(self, db: &'db dyn Db) -> Type<'db> {
        for base in self.iter_bases(db) {
            match base {
                NewTypeBase::NewType(_) => continue,
                concrete => {
                    let env = ProgramEnvironment::from_definition(self.definition(db));
                    return concrete.instance_type(db, &env);
                }
            }
        }
        Type::object()
    }

    fn is_equivalent_to(self, db: &'db dyn Db, other: Self) -> bool {
        self.definition(db) == other.definition(db)
    }

    /// Create a new `NewType` by mapping the underlying `ClassType`. This descends through any
    /// number of nested `NewType` layers and rebuilds the whole chain. In the rare case of cyclic
    /// `NewType`s with no underlying `ClassType`, this has no effect and does not call `f`.
    fn try_map_base_class_type(
        self,
        db: &'db dyn Db,
        f: impl FnOnce(ClassType<'db>) -> Option<ClassType<'db>>,
    ) -> Option<Self> {
        let mut inner_newtype_stack = Vec::new();
        for base in self.iter_bases(db) {
            match base {
                NewTypeBase::NewType(base_newtype) => inner_newtype_stack.push(base_newtype),
                NewTypeBase::ClassType(base_class_type) => {
                    let mut mapped_base = NewTypeBase::ClassType(f(base_class_type)?);
                    for inner_newtype in inner_newtype_stack.into_iter().rev() {
                        mapped_base = NewTypeBase::NewType(NewType::new(
                            db,
                            inner_newtype.name(db),
                            inner_newtype.definition(db),
                            Some(mapped_base),
                        ));
                    }
                    return Some(NewType::new(
                        db,
                        self.name(db),
                        self.definition(db),
                        Some(mapped_base),
                    ));
                }
                NewTypeBase::Float | NewTypeBase::Complex => {}
            }
        }
        Some(self)
    }

    pub(crate) fn map_base_class_type(
        self,
        db: &'db dyn Db,
        f: impl FnOnce(ClassType<'db>) -> ClassType<'db>,
    ) -> Self {
        self.try_map_base_class_type(db, |class_type| Some(f(class_type)))
            .unwrap()
    }

    pub(super) fn recursive_type_normalized_impl(
        self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        div: Type<'db>,
        nested: bool,
    ) -> Option<Self> {
        let eager_base = match self.eager_base(db) {
            Some(base) => Some(base.recursive_type_normalized_impl(db, env, div, nested)?),
            None => None,
        };

        Some(NewType::new(
            db,
            self.name(db),
            self.definition(db),
            eager_base,
        ))
    }
}

impl<'c, 'db> TypeRelationChecker<'_, 'c, 'db> {
    pub(super) fn check_newtype_pair(
        &self,
        db: &'db dyn Db,
        source: NewType<'db>,
        target: NewType<'db>,
    ) -> ConstraintSet<'db, 'c> {
        if source.is_equivalent_to(db, target) {
            return self.always();
        }
        for base in source.iter_bases(db) {
            if let NewTypeBase::NewType(base_newtype) = base
                && base_newtype.is_equivalent_to(db, target)
            {
                return self.always();
            }
        }
        self.never()
    }
}

impl<'c, 'db> DisjointnessChecker<'_, 'c, 'db> {
    pub(super) fn check_newtype_pair(
        &self,
        db: &'db dyn Db,
        left: NewType<'db>,
        right: NewType<'db>,
    ) -> ConstraintSet<'db, 'c> {

        let relation_checker = self.as_relation_checker(TypeRelation::Subtyping);
        relation_checker
            .check_newtype_pair(db, left, right)
            .or(db, self.constraints, || {
                relation_checker.check_newtype_pair(db, right, left)
            })
            .negate(db, self.constraints)
    }
}

pub(crate) fn walk_newtype_instance_type<'db, V: visitor::TypeVisitor<'db> + ?Sized>(
    db: &'db dyn Db,
    newtype: NewType<'db>,
    visitor: &V,
) {
    let base = if visitor.should_visit_lazy_type_attributes() {
        Some(newtype.base(db))
    } else {
        let base = newtype.eager_base(db);
        if base.is_none() {
            visitor.notify_skipped_lazy_type_attributes();
        }
        base
    };
    if let Some(base) = base {
        visitor.visit_type(db, base.instance_type(db, visitor.program_environment()));
    }
}

/// Evaluate and visit the instance type of a newtype's base.
///
/// The caller must guard against recursive newtypes.
pub(super) fn walk_newtype_base<'db, V: visitor::TypeVisitor<'db> + ?Sized>(
    db: &'db dyn Db,
    newtype: NewType<'db>,
    visitor: &V,
) {
    let base = newtype.base(db);
    visitor.visit_type(db, base.instance_type(db, visitor.program_environment()));
}

/// `typing.NewType` typically wraps a class type, but it can also wrap another newtype.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, get_size2::GetSize, salsa::SalsaValue)]
pub enum NewTypeBase<'db> {
    ClassType(ClassType<'db>),
    NewType(NewType<'db>),
    Float,
    Complex,
}

impl<'db> NewTypeBase<'db> {
    pub fn instance_type(self, db: &'db dyn Db, env: &ProgramEnvironment<'db>) -> Type<'db> {
        match self {
            NewTypeBase::ClassType(class_type) => Type::instance(db, env, class_type),
            NewTypeBase::NewType(newtype) => Type::NewTypeInstance(newtype),
            NewTypeBase::Float => KnownUnion::Float.to_type(db, env),
            NewTypeBase::Complex => KnownUnion::Complex.to_type(db, env),
        }
    }

    fn recursive_type_normalized_impl(
        self,
        db: &'db dyn Db,
        env: &ProgramEnvironment<'db>,
        div: Type<'db>,
        nested: bool,
    ) -> Option<Self> {
        match self {
            NewTypeBase::ClassType(class_type) => class_type
                .recursive_type_normalized_impl(db, env, div, nested)
                .map(NewTypeBase::ClassType),
            NewTypeBase::NewType(newtype) => newtype
                .recursive_type_normalized_impl(db, env, div, nested)
                .map(NewTypeBase::NewType),
            NewTypeBase::Float | NewTypeBase::Complex => Some(self),
        }
    }
}

/// An iterator over the transitive bases of a `NewType`. In the most common case, e.g.
/// `Foo = NewType("Foo", int)`, this yields the one `NewTypeBase::ClassType` (e.g. `int`). For
/// newtypes that wrap other newtypes, this iterator yields the `NewTypeBase::NewType`s (not
/// including `self`) before finally yielding the `NewTypeBase::ClassType`. In the pathological
/// case of cyclic newtypes like `Foo = NewType("Foo", "Foo")`, this iterator yields the unique
/// `NewTypeBase::NewType`s (not including `self`), detects the cycle, and then stops.
///
/// Note that this does *not* detect indirect cycles that go through a proper class, like this:
/// ```py
/// Foo = NewType("Foo", list["Foo"])
/// ```
/// As far as this iterator is concerned, that's the "common case", and it yields the one
/// `NewTypeBase::ClassType` for `list[Foo]`. Functions like `normalize` that continue recursing
/// over the base class need to pass down a cycle-detecting visitor as usual.
struct NewTypeBaseIter<'db> {
    current: Option<NewType<'db>>,
    seen_before: FxHashSet<NewType<'db>>,
    db: &'db dyn Db,
}

impl<'db> Iterator for NewTypeBaseIter<'db> {
    type Item = NewTypeBase<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        match current.base(self.db) {
            NewTypeBase::NewType(base_newtype) => {
                self.seen_before.insert(current);
                if self.seen_before.contains(&base_newtype) {
                    self.current = None;
                    None
                } else {
                    self.current = Some(base_newtype);
                    Some(NewTypeBase::NewType(base_newtype))
                }
            }
            concrete_base => {
                self.current = None;
                Some(concrete_base)
            }
        }
    }
}
