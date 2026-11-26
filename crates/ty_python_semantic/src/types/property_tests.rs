//! This module contains quickcheck-based property tests for `Type`s.
//!
//! These tests are disabled by default, as they are non-deterministic and slow. You can
//! run them explicitly using:
//!
//! ```sh
//! cargo test -p ty_python_semantic -- --ignored types::property_tests::stable
//! ```
//!
//! The number of tests (default: 100) can be controlled by setting the `QUICKCHECK_TESTS`
//! environment variable. For example:
//!
//! ```sh
//! QUICKCHECK_TESTS=10000 cargo test …
//! ```
//!
//! If you want to run these tests for a longer period of time, it's advisable to run them
//! in release mode. As some tests are slower than others, it's advisable to run them in a
//! loop until they fail:
//!
//! ```sh
//! export QUICKCHECK_TESTS=100000
//! while cargo test --release -p ty_python_semantic -- \
//!   --ignored types::property_tests::stable; do :; done
//! ```
mod setup;
mod type_generation;

use type_generation::{intersection, union};

/// A macro to define a property test for types.
///
/// The `$test_name` identifier specifies the name of the test function. The `$env` identifier
/// is used to refer to the semantic context in the property to be tested. The actual property is
/// specified using the syntax:
///
///     forall types t1, t2, ..., tn . <property>`
///
/// where `t1`, `t2`, ..., `tn` are identifiers that represent arbitrary types, and `<property>`
/// is an expression using these identifiers.
macro_rules! type_property_test {
    (@impl $test_name:ident, $db:ident, $env:ident, $input_type:ty, $($types:ident),+ . $property:expr) => {
        #[quickcheck_macros::quickcheck]
        #[ignore]
        fn $test_name($($types: $input_type),+) -> bool {
            let $db = &get_cached_db();
            let $env = &$db.program_environment();
            $(let $types = $types.into_type($db, $env);)+
            let result = $property;

            if !result {
                println!("\nFailing types were:");
                $(println!("{}", $types.display($db, $env));)+
            }

            result
        }
    };

    ($test_name:ident, $db:ident, $env:ident, forall types $($types:ident),+ . $property:expr) => {
        type_property_test!(@impl $test_name, $db, $env, Ty, $($types),+ . $property);
    };

    ($test_name:ident, $db:ident, $env:ident, forall fully_static_types $($types:ident),+ . $property:expr) => {
        type_property_test!(@impl $test_name, $db, $env, FullyStaticTy, $($types),+ . $property);
    };

    ($name:ident, $db:ident, $env:ident, forall $typekind:ident $($types:ident),+ . $premise:expr => $conclusion:expr) => {
        type_property_test!($name, $db, $env, forall $typekind $($types),+ . !($premise) || ($conclusion));
    };
}

mod stable {
    use super::{
        setup::get_cached_db,
        type_generation::{FullyStaticTy, Ty},
        union,
    };
    use crate::types::constraints::ConstraintSetBuilder;
    use crate::types::typevar::TypeVarSet;
    use crate::types::{CallableType, IntersectionBuilder, KnownClass, Type};

    type_property_test!(
        equivalent_to_is_reflexive, db, env,
        forall types t. t.is_equivalent_to(db, env, t)
    );

    type_property_test!(
        equivalent_to_is_symmetric, db, env,
        forall types s, t. s.is_equivalent_to(db, env, t) => t.is_equivalent_to(db, env, s)
    );

    type_property_test!(
        equivalent_to_never, db, env,
        forall types t. t.is_equivalent_to(db, env, Type::Never)
            == t.when_equivalent_to(db, env, Type::Never, &ConstraintSetBuilder::new())
                .is_always_satisfied(db, env, TypeVarSet::None)
    );

    type_property_test!(
        equivalent_to_is_transitive, db, env,
        forall types s, t, u. s.is_equivalent_to(db, env, t) && t.is_equivalent_to(db, env, u) => s.is_equivalent_to(db, env, u)
    );

    type_property_test!(
        subtype_of_is_transitive, db, env,
        forall types s, t, u. s.is_subtype_of(db, env, t) && t.is_subtype_of(db, env, u) => s.is_subtype_of(db, env, u)
    );

    type_property_test!(
        subtype_of_is_antisymmetric, db, env,
        forall types s, t. s.is_subtype_of(db, env, t) && t.is_subtype_of(db, env, s) => s.is_equivalent_to(db, env, t)
    );

    type_property_test!(
        negation_subtyping_matches_materialized_negation, db, env,
        forall types s, t, u. {
            let negated = s.negate(db, env);
            let mut cache = None;
            [t, u].into_iter().all(|target| {
                s.negation_is_subtype_of_cached(db, env, target, &mut cache)
                    == negated.is_subtype_of(db, env, target)
            })
        }
    );

    type_property_test!(
        disjoint_from_is_irreflexive, db, env,
        forall types t. t.is_disjoint_from(db, env, t) => t.is_never()
    );

    type_property_test!(
        disjoint_from_is_symmetric, db, env,
        forall types s, t. s.is_disjoint_from(db, env, t) == t.is_disjoint_from(db, env, s)
    );

    type_property_test!(
        subtype_of_implies_not_disjoint_from, db, env,
        forall types s, t. s.is_subtype_of(db, env, t) => !s.is_disjoint_from(db, env, t) || s.is_never()
    );

    type_property_test!(
        subtype_of_implies_assignable_to, db, env,
        forall types s, t. s.is_subtype_of(db, env, t) => s.is_assignable_to(db, env, t)
    );

    type_property_test!(
        all_types_assignable_to_object, db, env,
        forall types t. t.is_assignable_to(db, env, Type::object())
    );

    type_property_test!(
        all_types_subtype_of_object, db, env,
        forall types t. t.is_subtype_of(db, env, Type::object())
    );

    type_property_test!(
        never_assignable_to_every_type, db, env,
        forall types t. Type::Never.is_assignable_to(db, env, t)
    );

    type_property_test!(
        never_subtype_of_every_type, db, env,
        forall types t. Type::Never.is_subtype_of(db, env, t)
    );

    type_property_test!(
        bottom_callable_is_subtype_of_all_callable, db, env,
        forall types t. t.is_callable_type()
            => Type::Callable(CallableType::bottom(db)).is_subtype_of(db, env, t)
    );

    type_property_test!(
        assignable_to_is_reflexive, db, env,
        forall types t. t.is_assignable_to(db, env, t)
    );

    type_property_test!(
        all_type_pairs_are_assignable_to_their_union, db, env,
        forall types s, t. s.is_assignable_to(db, env, union(db, env, [s, t])) && t.is_assignable_to(db, env, union(db, env, [s, t]))
    );

    type_property_test!(
        only_never_is_subtype_of_any, db, env,
        forall types s. !s.is_equivalent_to(db, env, Type::Never) => !s.is_subtype_of(db, env, Type::any())
    );

    type_property_test!(
        only_object_is_supertype_of_any, db, env,
        forall types t. !t.is_equivalent_to(db, env, Type::object()) => !Type::any().is_subtype_of(db, env, t)
    );

    type_property_test!(
        equivalent_to_is_commutative, db, env,
        forall types s, t. s.is_equivalent_to(db, env, t) == t.is_equivalent_to(db, env, s)
    );

    type_property_test!(
        subtype_of_is_reflexive_for_fully_static_types, db, env,
        forall fully_static_types t. t.is_subtype_of(db, env, t)
    );

    type_property_test!(
        all_fully_static_type_pairs_are_subtype_of_their_union, db, env,
        forall fully_static_types s, t. s.is_subtype_of(db, env, union(db, env, [s, t])) && t.is_subtype_of(db, env, union(db, env, [s, t]))
    );

    type_property_test!(
        all_types_assignable_to_iterable_are_iterable, db, env,
        forall types t. t.is_assignable_to(db, env, KnownClass::Iterable.to_specialized_instance(db, env, &[Type::object()])) => t.try_iterate(db, env).is_ok()
    );

    type_property_test!(
        all_negated_types_identical_to_intersection_with_single_negated_element, db, env,
        forall types t. t.negate(db, env) == IntersectionBuilder::new(db, env).add_negative(t).build()
    );
}

/// This module contains property tests that currently lead to many false positives.
///
/// The reason for this is our insufficient understanding of equivalence of types. For
/// example, we currently consider `int | str` and `str | int` to be different types.
/// Similar issues exist for intersection types. Once this is resolved, we can move these
/// tests to the `stable` section. In the meantime, it can still be useful to run these
/// tests (using [`types::property_tests::flaky`]), to see if there are any new obvious bugs.
mod flaky {
    use itertools::Itertools;

    use super::{
        intersection,
        setup::get_cached_db,
        type_generation::{FullyStaticTy, Ty},
        union,
    };

    type_property_test!(
        double_negation_is_identity, db, env,
        forall types t. t.negate(db, env).negate(db, env).is_equivalent_to(db, env, t)
    );

    type_property_test!(
        negation_of_fully_static_types_is_disjoint, db, env,
        forall fully_static_types t. t.negate(db, env).is_disjoint_from(db, env, t)
    );

    type_property_test!(
        all_type_pairs_are_supertypes_of_their_intersection, db, env,
        forall types s, t.
            intersection(db, env, [s, t]).is_subtype_of(db, env, s) && intersection(db, env, [s, t]).is_subtype_of(db, env, t)
    );

    type_property_test!(
        all_type_pairs_can_be_assigned_from_their_intersection, db, env,
        forall types s, t. intersection(db, env, [s, t]).is_assignable_to(db, env, s) && intersection(db, env, [s, t]).is_assignable_to(db, env, t)
    );

    type_property_test!(
        intersection_equivalence_not_order_dependent, db, env,
        forall types s, t, u.
            [s, t, u]
                .into_iter()
                .permutations(3)
                .map(|trio_of_types| intersection(db, env, trio_of_types))
                .permutations(2)
                .all(|vec_of_intersections| vec_of_intersections[0].is_equivalent_to(db, env, vec_of_intersections[1]))
    );

    type_property_test!(
        union_equivalence_not_order_dependent, db, env,
        forall types s, t, u.
            [s, t, u]
                .into_iter()
                .permutations(3)
                .map(|trio_of_types| union(db, env, trio_of_types))
                .permutations(2)
                .all(|vec_of_unions| vec_of_unions[0].is_equivalent_to(db, env, vec_of_unions[1]))
    );

    type_property_test!(
        constituent_members_of_union_is_not_disjoint_from_that_union, db, env,
        forall types s, t.
            !s.is_disjoint_from(db, env, union(db, env, [s, t])) && !t.is_disjoint_from(db, env, union(db, env, [s, t]))
    );

    type_property_test!(
        negation_reverses_subtype_order, db, env,
        forall types s, t. s.is_subtype_of(db, env, t) => t.negate(db, env).is_subtype_of(db, env, s.negate(db, env))
    );


    type_property_test!(
        top_materialization_of_type_is_assignable_to_type, db, env,
        forall types t. t.top_materialization(db, env).is_assignable_to(db, env, t)
    );

    type_property_test!(
        bottom_materialization_of_type_is_assignable_to_type, db, env,
        forall types t. t.bottom_materialization(db, env).is_assignable_to(db, env, t)
    );
}
