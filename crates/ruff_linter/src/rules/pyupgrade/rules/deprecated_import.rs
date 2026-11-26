use itertools::Itertools;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::token::Tokens;
use ruff_python_ast::whitespace::indentation;
use ruff_python_ast::{Alias, StmtImportFrom, StmtRef};
use ruff_python_codegen::Stylist;
use ruff_text_size::Ranged;

use crate::Locator;
use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::rules::pyupgrade::fixes;
use crate::rules::pyupgrade::rules::unnecessary_future_import::is_import_required_by_isort;
use crate::{Applicability, Edit, Fix, FixAvailability, Violation};
use ruff_python_ast::PythonVersion;

use super::RequiredImports;

/// An import was moved and renamed as part of a deprecation.
/// For example, `typing.AbstractSet` was moved to `collections.abc.Set`.
#[derive(Debug, PartialEq, Eq)]
struct WithRename {
    module: String,
    member: String,
    target: String,
}

/// A series of imports from the same module were moved to another module,
/// but retain their original names.
#[derive(Debug, PartialEq, Eq)]
struct WithoutRename {
    target: String,
    members: Vec<String>,
    fixable: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Deprecation {
    WithRename(WithRename),
    WithoutRename(WithoutRename),
}

/// ## What it does
/// Checks for uses of deprecated imports based on the minimum supported
/// Python version.
///
/// ## Why is this bad?
/// Deprecated imports may be removed in future versions of Python, and
/// should be replaced with their new equivalents.
///
/// Note that, in some cases, it may be preferable to continue importing
/// members from `typing_extensions` even after they're added to the Python
/// standard library, as `typing_extensions` can backport bugfixes and
/// optimizations from later Python versions. This rule thus avoids flagging
/// imports from `typing_extensions` in such cases.
///
/// ## Example
/// ```python
/// from collections import Sequence
/// ```
///
/// Use instead:
/// ```python
/// from collections.abc import Sequence
/// ```
///
/// ## Fix safety
/// This fix is marked unsafe if the import statement contains comments, since
/// rewriting the import may delete them.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.239", category = Category::Suspicious)]
pub(crate) struct DeprecatedImport {
    deprecation: Deprecation,
}

impl Violation for DeprecatedImport {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        match &self.deprecation {
            Deprecation::WithoutRename(WithoutRename {
                members, target, ..
            }) => {
                let names = members.iter().map(|name| format!("`{name}`")).join(", ");
                format!("Import from `{target}` instead: {names}")
            }
            Deprecation::WithRename(WithRename {
                module,
                member,
                target,
            }) => {
                format!("`{module}.{member}` is deprecated, use `{target}` instead")
            }
        }
    }

    fn fix_title(&self) -> Option<String> {
        if let Deprecation::WithoutRename(WithoutRename { target, .. }) = &self.deprecation {
            Some(format!("Import from `{target}`"))
        } else {
            None
        }
    }
}

/// Returns `true` if the module may contain deprecated imports.
fn is_relevant_module(module: &str) -> bool {
    matches!(
        module,
        "collections"
            | "pipes"
            | "mypy_extensions"
            | "typing_extensions"
            | "typing"
            | "typing.io"
            | "typing.re"
            | "backports.strenum"
    )
}

const COLLECTIONS_TO_ABC: &[&str] = &[
    "AsyncGenerator",
    "AsyncIterable",
    "AsyncIterator",
    "Awaitable",
    "ByteString",
    "Callable",
    "Collection",
    "Container",
    "Coroutine",
    "Generator",
    "Hashable",
    "ItemsView",
    "Iterable",
    "Iterator",
    "KeysView",
    "Mapping",
    "MappingView",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "Reversible",
    "Sequence",
    "Set",
    "Sized",
    "ValuesView",
];

const PIPES_TO_SHLEX: &[&str] = &["quote"];

const TYPING_EXTENSIONS_TO_TYPING: &[&str] = &[
    "AbstractSet",
    "AnyStr",
    "AsyncIterable",
    "AsyncIterator",
    "Awaitable",
    "BinaryIO",
    "Callable",
    "ClassVar",
    "Collection",
    "Container",
    "Coroutine",
    "DefaultDict",
    "Dict",
    "FrozenSet",
    "Generic",
    "Hashable",
    "IO",
    "ItemsView",
    "Iterable",
    "Iterator",
    "KeysView",
    "List",
    "Mapping",
    "MappingView",
    "Match",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "Optional",
    "Pattern",
    "Reversible",
    "Sequence",
    "Set",
    "Sized",
    "TYPE_CHECKING",
    "Text",
    "TextIO",
    "Tuple",
    "Type",
    "Union",
    "ValuesView",
    "cast",
    "no_type_check",
];


const MYPY_EXTENSIONS_TO_TYPING_37: &[&str] = &["NoReturn"];

const TYPING_EXTENSIONS_TO_TYPING_37: &[&str] = &[
    "ChainMap",
    "Counter",
    "Deque",
    "ForwardRef",
    "NoReturn",
];

const TYPING_IO_TO_TYPING_37: &[&str] = &["BinaryIO", "IO", "TextIO"];

const TYPING_RE_TO_TYPING_37: &[&str] = &["Match", "Pattern"];


const MYPY_EXTENSIONS_TO_TYPING_38: &[&str] = &["TypedDict"];

const TYPING_EXTENSIONS_TO_TYPING_38: &[&str] = &[
    "Final",
    "OrderedDict",
];


const TYPING_TO_COLLECTIONS_ABC_39: &[&str] = &[
    "AsyncGenerator",
    "AsyncIterable",
    "AsyncIterator",
    "Awaitable",
    "Collection",
    "Container",
    "Coroutine",
    "Generator",
    "Hashable",
    "ItemsView",
    "Iterable",
    "Iterator",
    "KeysView",
    "Mapping",
    "MappingView",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "Reversible",
    "Sequence",
    "Sized",
    "ValuesView",
];

const TYPING_TO_COLLECTIONS_39: &[&str] = &["ChainMap", "Counter", "OrderedDict"];

const TYPING_TO_RE_39: &[&str] = &["Match", "Pattern"];

const TYPING_RE_TO_RE_39: &[&str] = &["Match", "Pattern"];

const TYPING_EXTENSIONS_TO_TYPING_39: &[&str] = &["Annotated"];

const TYPING_TO_RENAME_PY39: &[(&str, &str)] = &[
    (
        "AsyncContextManager",
        "contextlib.AbstractAsyncContextManager",
    ),
    ("ContextManager", "contextlib.AbstractContextManager"),
    ("AbstractSet", "collections.abc.Set"),
    ("Tuple", "tuple"),
    ("List", "list"),
    ("FrozenSet", "frozenset"),
    ("Dict", "dict"),
    ("Type", "type"),
    ("Set", "set"),
    ("Deque", "collections.deque"),
    ("DefaultDict", "collections.defaultdict"),
];


const TYPING_TO_COLLECTIONS_ABC_310: &[&str] = &["Callable"];

const TYPING_EXTENSIONS_TO_TYPING_310: &[&str] = &[
    "Concatenate",
    "Literal",
    "NewType",
    "ParamSpecArgs",
    "ParamSpecKwargs",
    "TypeAlias",
    "TypeGuard",
    "get_args",
    "get_origin",
];


const TYPING_EXTENSIONS_TO_TYPING_311: &[&str] = &[
    "Any",
    "LiteralString",
    "Never",
    "NotRequired",
    "Required",
    "Self",
    "assert_never",
    "assert_type",
    "clear_overloads",
    "final",
    "get_overloads",
    "overload",
    "reveal_type",
];

const BACKPORTS_STR_ENUM_TO_ENUM_311: &[&str] = &["StrEnum"];


const TYPING_EXTENSIONS_TO_TYPING_312: &[&str] = &[
    "Protocol",
    "SupportsAbs",
    "SupportsBytes",
    "SupportsComplex",
    "SupportsFloat",
    "SupportsIndex",
    "SupportsInt",
    "SupportsRound",
    "TypeAliasType",
    "Unpack",
    "NamedTuple",
    "dataclass_transform",
    "override",
];

const TYPING_EXTENSIONS_TO_COLLECTIONS_ABC_312: &[&str] = &["Buffer"];

const TYPING_EXTENSIONS_TO_TYPES_312: &[&str] = &["get_original_bases"];


const TYPING_EXTENSIONS_TO_TYPING_313: &[&str] = &[
    "get_protocol_members",
    "is_protocol",
    "NoDefault",
    "ReadOnly",
    "TypeIs",
    "get_type_hints",
    "ContextManager",
    "Generator",
    "AsyncContextManager",
    "AsyncGenerator",
    "Protocol",
    "runtime_checkable",
    "ParamSpec",
    "TypeVar",
    "TypeVarTuple",
];

const TYPING_EXTENSIONS_TO_TYPES_313: &[&str] = &["CapsuleType"];

const TYPING_EXTENSIONS_TO_WARNINGS_313: &[&str] = &["deprecated"];


const TYPING_EXTENSIONS_TO_TYPING_315: &[&str] = &["TypeForm"];

struct ImportReplacer<'a> {
    import_from_stmt: &'a StmtImportFrom,
    module: &'a str,
    locator: &'a Locator<'a>,
    stylist: &'a Stylist<'a>,
    tokens: &'a Tokens,
    version: PythonVersion,
    required_imports: &'a RequiredImports,
}

impl<'a> ImportReplacer<'a> {
    const fn new(
        import_from_stmt: &'a StmtImportFrom,
        module: &'a str,
        locator: &'a Locator<'a>,
        stylist: &'a Stylist<'a>,
        tokens: &'a Tokens,
        version: PythonVersion,
        required_imports: &'a RequiredImports,
    ) -> Self {
        Self {
            import_from_stmt,
            module,
            locator,
            stylist,
            tokens,
            version,
            required_imports,
        }
    }

    /// Return a list of deprecated imports whose members were renamed.
    fn with_renames(&self) -> Vec<WithRename> {
        let mut operations = vec![];
        if self.module == "typing" {
            if self.version >= PythonVersion::PY39 {
                for member in &self.import_from_stmt.names {
                    if is_import_required_by_isort(
                        self.required_imports,
                        StmtRef::ImportFrom(self.import_from_stmt),
                        member,
                    ) {
                        continue;
                    }
                    if let Some(target) = TYPING_TO_RENAME_PY39.iter().find_map(|(name, target)| {
                        if &member.name == *name {
                            Some(*target)
                        } else {
                            None
                        }
                    }) {
                        operations.push(WithRename {
                            module: "typing".to_string(),
                            member: member.name.to_string(),
                            target: target.to_string(),
                        });
                    }
                }
            }
        }
        operations
    }

    /// Return a list of deprecated imports whose members were moved, but not renamed.
    fn without_renames(&self) -> Vec<(WithoutRename, Option<String>)> {
        let mut operations = vec![];
        match self.module {
            "collections" => {
                if let Some(operation) = self.try_replace(COLLECTIONS_TO_ABC, "collections.abc") {
                    operations.push(operation);
                }
            }
            "pipes" => {
                if let Some(operation) = self.try_replace(PIPES_TO_SHLEX, "shlex") {
                    operations.push(operation);
                }
            }
            "typing_extensions" => {
                let mut typing_extensions_to_collections_abc = vec![];
                if self.version >= PythonVersion::PY312 {
                    typing_extensions_to_collections_abc
                        .extend(TYPING_EXTENSIONS_TO_COLLECTIONS_ABC_312);
                }
                if let Some(operation) =
                    self.try_replace(&typing_extensions_to_collections_abc, "collections.abc")
                {
                    operations.push(operation);
                }

                let mut typing_extensions_to_warnings = vec![];
                if self.version >= PythonVersion::PY313 {
                    typing_extensions_to_warnings.extend(TYPING_EXTENSIONS_TO_WARNINGS_313);
                }
                if let Some(operation) =
                    self.try_replace(&typing_extensions_to_warnings, "warnings")
                {
                    operations.push(operation);
                }

                let mut typing_extensions_to_types = vec![];
                if self.version >= PythonVersion::PY312 {
                    typing_extensions_to_types.extend(TYPING_EXTENSIONS_TO_TYPES_312);
                }
                if self.version >= PythonVersion::PY313 {
                    typing_extensions_to_types.extend(TYPING_EXTENSIONS_TO_TYPES_313);
                }
                if let Some(operation) = self.try_replace(&typing_extensions_to_types, "types") {
                    operations.push(operation);
                }

                let mut typing_extensions_to_typing = TYPING_EXTENSIONS_TO_TYPING.to_vec();
                if self.version >= PythonVersion::PY37 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_37);
                }
                if self.version >= PythonVersion::PY38 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_38);
                }
                if self.version >= PythonVersion::PY39 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_39);
                }
                if self.version >= PythonVersion::PY310 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_310);
                }
                if self.version >= PythonVersion::PY311 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_311);
                }
                if self.version >= PythonVersion::PY312 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_312);
                }
                if self.version >= PythonVersion::PY313 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_313);
                }
                if self.version >= PythonVersion::PY315 {
                    typing_extensions_to_typing.extend(TYPING_EXTENSIONS_TO_TYPING_315);
                }
                if let Some(operation) = self.try_replace(&typing_extensions_to_typing, "typing") {
                    operations.push(operation);
                }
            }
            "mypy_extensions" => {
                let mut mypy_extensions_to_typing = vec![];
                if self.version >= PythonVersion::PY37 {
                    mypy_extensions_to_typing.extend(MYPY_EXTENSIONS_TO_TYPING_37);
                }
                if self.version >= PythonVersion::PY38 {
                    mypy_extensions_to_typing.extend(MYPY_EXTENSIONS_TO_TYPING_38);
                }
                if let Some(operation) = self.try_replace(&mypy_extensions_to_typing, "typing") {
                    operations.push(operation);
                }
            }
            "typing" => {
                let mut typing_to_collections_abc = vec![];
                if self.version >= PythonVersion::PY39 {
                    typing_to_collections_abc.extend(TYPING_TO_COLLECTIONS_ABC_39);
                }
                if self.version >= PythonVersion::PY310 {
                    typing_to_collections_abc.extend(TYPING_TO_COLLECTIONS_ABC_310);
                }
                if let Some(operation) =
                    self.try_replace(&typing_to_collections_abc, "collections.abc")
                {
                    operations.push(operation);
                }

                let mut typing_to_collections = vec![];
                if self.version >= PythonVersion::PY39 {
                    typing_to_collections.extend(TYPING_TO_COLLECTIONS_39);
                }
                if let Some(operation) = self.try_replace(&typing_to_collections, "collections") {
                    operations.push(operation);
                }

                let mut typing_to_re = vec![];
                if self.version >= PythonVersion::PY39 {
                    typing_to_re.extend(TYPING_TO_RE_39);
                }
                if let Some(operation) = self.try_replace(&typing_to_re, "re") {
                    operations.push(operation);
                }
            }
            "typing.io" if self.version >= PythonVersion::PY37 => {
                if let Some(operation) = self.try_replace(TYPING_IO_TO_TYPING_37, "typing") {
                    operations.push(operation);
                }
            }
            "typing.re" => {
                if self.version >= PythonVersion::PY39 {
                    if let Some(operation) = self.try_replace(TYPING_RE_TO_RE_39, "re") {
                        operations.push(operation);
                    }
                } else if self.version >= PythonVersion::PY37 {
                    if let Some(operation) = self.try_replace(TYPING_RE_TO_TYPING_37, "typing") {
                        operations.push(operation);
                    }
                }
            }
            "backports.strenum" if self.version >= PythonVersion::PY311 => {
                if let Some(operation) = self.try_replace(BACKPORTS_STR_ENUM_TO_ENUM_311, "enum") {
                    operations.push(operation);
                }
            }
            _ => {}
        }
        operations
    }

    fn try_replace(
        &'a self,
        candidates: &[&str],
        target: &'a str,
    ) -> Option<(WithoutRename, Option<String>)> {
        if candidates.is_empty() {
            return None;
        }

        let (matched_names, unmatched_names) = self.partition_imports(candidates);

        if matched_names.is_empty() {
            return None;
        }

        if unmatched_names.is_empty() {
            let matched = self.format_import_from(&matched_names, target);
            let operation = WithoutRename {
                target: target.to_string(),
                members: matched_names
                    .iter()
                    .map(|name| name.name.to_string())
                    .collect(),
                fixable: true,
            };
            let fix = Some(matched);
            Some((operation, fix))
        } else {
            let indentation = indentation(self.locator.contents(), self.import_from_stmt);

            let Some(indentation) = indentation else {
                let operation = WithoutRename {
                    target: target.to_string(),
                    members: matched_names
                        .iter()
                        .map(|name| name.name.to_string())
                        .collect(),
                    fixable: false,
                };
                let fix = None;
                return Some((operation, fix));
            };

            let matched = self.format_import_from(&matched_names, target);
            let unmatched = fixes::remove_import_members(
                self.locator,
                self.import_from_stmt,
                self.tokens,
                &matched_names
                    .iter()
                    .map(|name| name.name.as_str())
                    .collect::<Vec<_>>(),
            );

            let operation = WithoutRename {
                target: target.to_string(),
                members: matched_names
                    .iter()
                    .map(|name| name.name.to_string())
                    .collect(),
                fixable: true,
            };
            let fix = Some(format!(
                "{unmatched}{}{}{matched}",
                self.stylist.line_ending().as_str(),
                indentation,
            ));
            Some((operation, fix))
        }
    }

    /// Partitions imports into matched and unmatched names.
    fn partition_imports(&self, candidates: &[&str]) -> (Vec<&Alias>, Vec<&Alias>) {
        let mut matched_names = vec![];
        let mut unmatched_names = vec![];
        for name in &self.import_from_stmt.names {
            if is_import_required_by_isort(
                self.required_imports,
                StmtRef::ImportFrom(self.import_from_stmt),
                name,
            ) {
                unmatched_names.push(name);
            } else if candidates.contains(&name.name.as_str()) {
                matched_names.push(name);
            } else {
                unmatched_names.push(name);
            }
        }
        (matched_names, unmatched_names)
    }

    /// Converts a list of names and a module into an `import from`-style
    /// import.
    fn format_import_from(&self, names: &[&Alias], module: &str) -> String {
        let qualified_names: String = names
            .iter()
            .map(|name| match &name.asname {
                Some(asname) => format!("{} as {}", name.name, asname),
                None => format!("{}", name.name),
            })
            .join(", ");
        let prefix = if self.import_from_stmt.is_lazy {
            "lazy "
        } else {
            ""
        };
        format!("{prefix}from {module} import {qualified_names}")
    }
}

/// UP035
pub(crate) fn deprecated_import(checker: &Checker, import_from_stmt: &StmtImportFrom) {
    if import_from_stmt.level > 0 {
        return;
    }
    if import_from_stmt
        .names
        .first()
        .is_some_and(|name| &name.name == "*")
    {
        return;
    }
    let Some(module) = import_from_stmt.module.as_deref() else {
        return;
    };

    if !is_relevant_module(module) {
        return;
    }

    let fixer = ImportReplacer::new(
        import_from_stmt,
        module,
        checker.locator(),
        checker.stylist(),
        checker.tokens(),
        checker.target_version(),
        &checker.settings().isort.required_imports,
    );

    for (operation, fix) in fixer.without_renames() {
        let preserves_laziness = import_from_stmt.is_lazy
            || checker.import_rewrite_preserves_laziness(module, &operation.target);
        let mut diagnostic = checker.report_diagnostic(
            DeprecatedImport {
                deprecation: Deprecation::WithoutRename(operation),
            },
            import_from_stmt.range(),
        );
        diagnostic.add_primary_tag(ruff_db::diagnostic::DiagnosticTag::Deprecated);
        if let Some(content) = fix
            && preserves_laziness
        {
            let applicability = if checker
                .comment_ranges()
                .intersects(import_from_stmt.range())
            {
                Applicability::Unsafe
            } else {
                Applicability::Safe
            };
            diagnostic.set_fix(Fix::applicable_edit(
                Edit::range_replacement(content, import_from_stmt.range()),
                applicability,
            ));
        }
    }

    for operation in fixer.with_renames() {
        let mut diagnostic = checker.report_diagnostic(
            DeprecatedImport {
                deprecation: Deprecation::WithRename(operation),
            },
            import_from_stmt.range(),
        );
        diagnostic.add_primary_tag(ruff_db::diagnostic::DiagnosticTag::Deprecated);
    }
}
