//! Effective program settings, taking into account pyproject.toml and
//! command-line options. Structure is optimized for internal usage, as opposed
//! to external visibility or parsing.

use regex::Regex;
use rustc_hash::FxHashSet;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use types::CompiledPerFileTargetVersionList;

use ruff_macros::CacheKey;
use ruff_python_ast::PythonVersion;

use crate::line_width::LineLength;
use crate::registry::Rule;
use crate::rules::{
    flake8_annotations, flake8_bandit, flake8_boolean_trap, flake8_bugbear, flake8_builtins,
    flake8_comprehensions, flake8_copyright, flake8_errmsg, flake8_gettext,
    flake8_implicit_str_concat, flake8_import_conventions, flake8_pytest_style, flake8_quotes,
    flake8_self, flake8_tidy_imports, flake8_type_checking, flake8_unused_arguments, isort, mccabe,
    pep8_naming, pycodestyle, pydoclint, pydocstyle, pyflakes, pylint, pyupgrade, ruff,
};
use crate::settings::types::{CompiledPerFileIgnoreList, ExtensionMapping, FilePatternSet};
use crate::{RuleSelector, fs};

use super::line_width::IndentWidth;

use self::fix_safety_table::FixSafetyTable;
use self::rule_table::RuleTable;
use self::types::PreviewMode;
use crate::rule_selector::PreviewOptions;

pub mod fix_safety_table;
pub mod flags;
pub mod rule_table;
pub mod types;

/// `display_settings!` is a macro that can display and format struct fields in a readable,
/// namespaced format. It's particularly useful at generating `Display` implementations
/// for types used in settings.
///
/// # Example
/// ```
/// use std::fmt;
/// use ruff_linter::display_settings;
/// #[derive(Default)]
/// struct Settings {
///     option_a: bool,
///     sub_settings: SubSettings,
///     option_b: String,
/// }
///
/// struct SubSettings {
///     name: String
/// }
///
/// impl Default for SubSettings {
///     fn default() -> Self {
///         Self { name: "Default Name".into() }
///     }
///
/// }
///
/// impl fmt::Display for SubSettings {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         display_settings! {
///             formatter = f,
///             namespace = "sub_settings",
///             fields = [
///                 self.name | quoted
///             ]
///         }
///         Ok(())
///     }
///
/// }
///
/// impl fmt::Display for Settings {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         display_settings! {
///             formatter = f,
///             fields = [
///                 self.option_a,
///                 self.sub_settings | nested,
///                 self.option_b | quoted,
///             ]
///         }
///         Ok(())
///     }
///
/// }
///
/// const EXPECTED_OUTPUT: &str = r#"option_a = false
/// sub_settings.name = "Default Name"
/// option_b = ""
/// "#;
///
/// fn main() {
///     let settings = Settings::default();
///     assert_eq!(format!("{settings}"), EXPECTED_OUTPUT);
/// }
/// ```
#[macro_export]
macro_rules! display_settings {
    (formatter = $fmt:ident, namespace = $namespace:literal, fields = [$($settings:ident.$field:ident $(| $modifier:tt)?),* $(,)?]) => {
        {
            const _PREFIX: &str = concat!($namespace, ".");
            $(
                display_settings!(@field $fmt, _PREFIX, $settings.$field $(| $modifier)?);
            )*
        }
    };
    (formatter = $fmt:ident, fields = [$($settings:ident.$field:ident $(| $modifier:tt)?),* $(,)?]) => {
        {
            const _PREFIX: &str = "";
            $(
                display_settings!(@field $fmt, _PREFIX, $settings.$field $(| $modifier)?);
            )*
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | debug) => {
        writeln!($fmt, "{}{} = {:?}", $prefix, stringify!($field), $settings.$field)?;
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | path) => {
        writeln!($fmt, "{}{} = \"{}\"", $prefix, stringify!($field), $settings.$field.display())?;
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | quoted) => {
        writeln!($fmt, "{}{} = \"{}\"", $prefix, stringify!($field), $settings.$field)?;
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | globmatcher) => {
        writeln!($fmt, "{}{} = \"{}\"", $prefix, stringify!($field), $settings.$field.glob())?;
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | nested) => {
        write!($fmt, "{}", $settings.$field)?;
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | optional) => {
        {
            write!($fmt, "{}{} = ", $prefix, stringify!($field))?;
            match &$settings.$field {
                Some(value) => writeln!($fmt, "{}", value)?,
                None        => writeln!($fmt, "none")?
            };
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | array) => {
        {
            write!($fmt, "{}{} = ", $prefix, stringify!($field))?;
            if $settings.$field.is_empty() {
                writeln!($fmt, "[]")?;
            } else {
                writeln!($fmt, "[")?;
                for elem in &$settings.$field {
                    writeln!($fmt, "\t{elem},")?;
                }
                writeln!($fmt, "]")?;
            }
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | map) => {
        {
            use itertools::Itertools;

            write!($fmt, "{}{} = ", $prefix, stringify!($field))?;
            if $settings.$field.is_empty() {
                writeln!($fmt, "{{}}")?;
            } else {
                writeln!($fmt, "{{")?;
                for (key, value) in $settings.$field.iter().sorted_by(|(left, _), (right, _)| left.cmp(right)) {
                    writeln!($fmt, "\t{key} = {value},")?;
                }
                writeln!($fmt, "}}")?;
            }
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | set) => {
        {
            use itertools::Itertools;

            write!($fmt, "{}{} = ", $prefix, stringify!($field))?;
            if $settings.$field.is_empty() {
                writeln!($fmt, "[]")?;
            } else {
                writeln!($fmt, "[")?;
                for elem in $settings.$field.iter().sorted_by(|left, right| left.cmp(right)) {
                    writeln!($fmt, "\t{elem},")?;
                }
                writeln!($fmt, "]")?;
            }
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident | paths) => {
        {
            write!($fmt, "{}{} = ", $prefix, stringify!($field))?;
            if $settings.$field.is_empty() {
                writeln!($fmt, "[]")?;
            } else {
                writeln!($fmt, "[")?;
                for elem in &$settings.$field {
                    writeln!($fmt, "\t\"{}\",", elem.display())?;
                }
                writeln!($fmt, "]")?;
            }
        }
    };
    (@field $fmt:ident, $prefix:ident, $settings:ident.$field:ident) => {
        writeln!($fmt, "{}{} = {}", $prefix, stringify!($field), $settings.$field)?;
    };
}

#[derive(Debug, Clone, CacheKey)]
#[expect(clippy::struct_excessive_bools)]
pub struct LinterSettings {
    pub exclude: FilePatternSet,
    pub extension: ExtensionMapping,
    pub project_root: PathBuf,

    pub rules: RuleTable,
    pub per_file_ignores: CompiledPerFileIgnoreList,
    pub fix_safety: FixSafetyTable,

    /// The non-path-resolved Python version specified by the `target-version` input option.
    ///
    /// If you have a `Checker` available, see its `target_version` method instead.
    ///
    /// Otherwise, see [`LinterSettings::resolve_target_version`] for a way to obtain the Python
    /// version for a given file, while respecting the overrides in `per_file_target_version`.
    pub unresolved_target_version: TargetVersion,
    /// Path-specific overrides to `unresolved_target_version`.
    ///
    /// If you have a `Checker` available, see its `target_version` method instead.
    ///
    /// Otherwise, see [`LinterSettings::resolve_target_version`] for a way to check a given
    /// [`Path`] against these patterns, while falling back to `unresolved_target_version` if none
    /// of them match.
    pub per_file_target_version: CompiledPerFileTargetVersionList,
    pub preview: PreviewMode,
    pub explicit_preview_rules: bool,

    pub allowed_confusables: FxHashSet<char>,
    pub builtins: Vec<String>,
    pub dummy_variable_rgx: Regex,
    pub external: Vec<String>,
    pub ignore_init_module_imports: bool,
    pub logger_objects: Vec<String>,
    pub namespace_packages: Vec<PathBuf>,
    pub src: Vec<PathBuf>,
    pub tab_size: IndentWidth,
    pub line_length: LineLength,
    pub task_tags: Vec<String>,
    pub typing_modules: Vec<String>,
    pub typing_extensions: bool,
    pub future_annotations: bool,

    pub flake8_annotations: flake8_annotations::settings::Settings,
    pub flake8_bandit: flake8_bandit::settings::Settings,
    pub flake8_boolean_trap: flake8_boolean_trap::settings::Settings,
    pub flake8_bugbear: flake8_bugbear::settings::Settings,
    pub flake8_builtins: flake8_builtins::settings::Settings,
    pub flake8_comprehensions: flake8_comprehensions::settings::Settings,
    pub flake8_copyright: flake8_copyright::settings::Settings,
    pub flake8_errmsg: flake8_errmsg::settings::Settings,
    pub flake8_gettext: flake8_gettext::settings::Settings,
    pub flake8_implicit_str_concat: flake8_implicit_str_concat::settings::Settings,
    pub flake8_import_conventions: flake8_import_conventions::settings::Settings,
    pub flake8_pytest_style: flake8_pytest_style::settings::Settings,
    pub flake8_quotes: flake8_quotes::settings::Settings,
    pub flake8_self: flake8_self::settings::Settings,
    pub flake8_tidy_imports: flake8_tidy_imports::settings::Settings,
    pub flake8_type_checking: flake8_type_checking::settings::Settings,
    pub flake8_unused_arguments: flake8_unused_arguments::settings::Settings,
    pub isort: isort::settings::Settings,
    pub mccabe: mccabe::settings::Settings,
    pub pep8_naming: pep8_naming::settings::Settings,
    pub pycodestyle: pycodestyle::settings::Settings,
    pub pydoclint: pydoclint::settings::Settings,
    pub pydocstyle: pydocstyle::settings::Settings,
    pub pyflakes: pyflakes::settings::Settings,
    pub pylint: pylint::settings::Settings,
    pub pyupgrade: pyupgrade::settings::Settings,
    pub ruff: ruff::settings::Settings,
}

impl Display for LinterSettings {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\n# Linter Settings")?;
        display_settings! {
            formatter = f,
            namespace = "linter",
            fields = [
                self.exclude,
                self.project_root | path,

                self.rules | nested,
                self.per_file_ignores,
                self.fix_safety | nested,

                self.unresolved_target_version,
                self.per_file_target_version,
                self.preview,
                self.explicit_preview_rules,
                self.extension | debug,

                self.allowed_confusables | array,
                self.builtins | array,
                self.dummy_variable_rgx,
                self.external | array,
                self.ignore_init_module_imports,
                self.logger_objects | array,
                self.namespace_packages | debug,
                self.src | paths,
                self.tab_size,
                self.line_length,
                self.task_tags | array,
                self.typing_modules | array,
                self.typing_extensions,
            ]
        }
        writeln!(f, "\n# Linter Plugins")?;
        display_settings! {
            formatter = f,
            namespace = "linter",
            fields = [
                self.flake8_annotations | nested,
                self.flake8_bandit | nested,
                self.flake8_bugbear | nested,
                self.flake8_builtins | nested,
                self.flake8_comprehensions | nested,
                self.flake8_copyright | nested,
                self.flake8_errmsg | nested,
                self.flake8_gettext | nested,
                self.flake8_implicit_str_concat | nested,
                self.flake8_import_conventions | nested,
                self.flake8_pytest_style | nested,
                self.flake8_quotes | nested,
                self.flake8_self | nested,
                self.flake8_tidy_imports | nested,
                self.flake8_type_checking | nested,
                self.flake8_unused_arguments | nested,
                self.isort | nested,
                self.mccabe | nested,
                self.pep8_naming | nested,
                self.pycodestyle | nested,
                self.pyflakes | nested,
                self.pylint | nested,
                self.pyupgrade | nested,
                self.ruff | nested,
            ]
        }
        Ok(())
    }
}

#[rustfmt::skip]
pub const DEFAULT_SELECTORS: &[RuleSelector] = &[
    RuleSelector::rule(Rule::CancelScopeNoCheckpoint),
    RuleSelector::rule(Rule::TrioSyncCall),
    RuleSelector::rule(Rule::AsyncZeroSleep),
    RuleSelector::rule(Rule::LongSleepNotForever),
    RuleSelector::rule(Rule::BlockingHttpCallInAsyncFunction),
    RuleSelector::rule(Rule::CreateSubprocessInAsyncFunction),
    RuleSelector::rule(Rule::RunProcessInAsyncFunction),
    RuleSelector::rule(Rule::WaitForProcessInAsyncFunction),
    RuleSelector::rule(Rule::BlockingOpenCallInAsyncFunction),
    RuleSelector::rule(Rule::BlockingSleepInAsyncFunction),
    RuleSelector::rule(Rule::UnaryPrefixIncrementDecrement),
    RuleSelector::rule(Rule::AssignmentToOsEnviron),
    RuleSelector::rule(Rule::UnreliableCallableCheck),
    RuleSelector::rule(Rule::StripWithMultiCharacters),
    RuleSelector::rule(Rule::MutableArgumentDefault),
    RuleSelector::rule(Rule::FunctionCallInDefaultArgument),
    RuleSelector::rule(Rule::GetAttrWithConstant),
    RuleSelector::rule(Rule::SetAttrWithConstant),
    RuleSelector::rule(Rule::JumpStatementInFinally),
    RuleSelector::rule(Rule::RedundantTupleInExceptionHandler),
    RuleSelector::rule(Rule::DuplicateHandlerException),
    RuleSelector::rule(Rule::UselessComparison),
    RuleSelector::rule(Rule::RaiseLiteral),
    RuleSelector::rule(Rule::AssertRaisesException),
    RuleSelector::rule(Rule::UselessExpression),
    RuleSelector::rule(Rule::CachedInstanceMethod),
    RuleSelector::rule(Rule::LoopVariableOverridesIterator),
    RuleSelector::rule(Rule::FStringDocstring),
    RuleSelector::rule(Rule::UselessContextlibSuppress),
    RuleSelector::rule(Rule::FunctionUsesLoopVariable),
    RuleSelector::rule(Rule::DuplicateTryBlockException),
    RuleSelector::rule(Rule::StarArgUnpackingAfterKeywordArg),
    RuleSelector::rule(Rule::ExceptWithEmptyTuple),
    RuleSelector::rule(Rule::ExceptWithNonExceptionClasses),
    RuleSelector::rule(Rule::ReuseOfGroupbyGenerator),
    RuleSelector::rule(Rule::UnintentionalTypeAnnotation),
    RuleSelector::rule(Rule::DuplicateValue),
    RuleSelector::rule(Rule::StaticKeyDictComprehension),
    RuleSelector::rule(Rule::MutableContextvarDefault),
    RuleSelector::rule(Rule::BlindExcept),
    RuleSelector::rule(Rule::UnnecessaryGeneratorList),
    RuleSelector::rule(Rule::UnnecessaryGeneratorSet),
    RuleSelector::rule(Rule::UnnecessaryGeneratorDict),
    RuleSelector::rule(Rule::UnnecessaryListComprehensionSet),
    RuleSelector::rule(Rule::UnnecessaryListComprehensionDict),
    RuleSelector::rule(Rule::UnnecessaryLiteralSet),
    RuleSelector::rule(Rule::UnnecessaryLiteralDict),
    RuleSelector::rule(Rule::UnnecessaryCollectionCall),
    RuleSelector::rule(Rule::UnnecessaryLiteralWithinTupleCall),
    RuleSelector::rule(Rule::UnnecessaryLiteralWithinListCall),
    RuleSelector::rule(Rule::UnnecessaryListCall),
    RuleSelector::rule(Rule::UnnecessaryCallAroundSorted),
    RuleSelector::rule(Rule::UnnecessaryDoubleCastOrProcess),
    RuleSelector::rule(Rule::UnnecessarySubscriptReversal),
    RuleSelector::rule(Rule::UnnecessaryMap),
    RuleSelector::rule(Rule::UnnecessaryLiteralWithinDictCall),
    RuleSelector::rule(Rule::UnnecessaryComprehensionInCall),
    RuleSelector::rule(Rule::EmptyDocstring),
    RuleSelector::rule(Rule::CallDatetimeToday),
    RuleSelector::rule(Rule::CallDatetimeUtcnow),
    RuleSelector::rule(Rule::CallDatetimeUtcfromtimestamp),
    RuleSelector::rule(Rule::BareExcept),
    RuleSelector::rule(Rule::IOError),
    RuleSelector::rule(Rule::ShebangNotExecutable),
    RuleSelector::rule(Rule::ShebangMissingExecutableFile),
    RuleSelector::rule(Rule::ShebangLeadingWhitespace),
    RuleSelector::rule(Rule::ShebangNotFirstLine),
    RuleSelector::rule(Rule::UnusedImport),
    RuleSelector::rule(Rule::ImportShadowedByLoopVar),
    RuleSelector::rule(Rule::LateFutureImport),
    RuleSelector::rule(Rule::UndefinedLocalWithNestedImportStarUsage),
    RuleSelector::rule(Rule::FutureFeatureNotDefined),
    RuleSelector::rule(Rule::PercentFormatInvalidFormat),
    RuleSelector::rule(Rule::PercentFormatExpectedMapping),
    RuleSelector::rule(Rule::PercentFormatExpectedSequence),
    RuleSelector::rule(Rule::PercentFormatExtraNamedArguments),
    RuleSelector::rule(Rule::PercentFormatMissingArgument),
    RuleSelector::rule(Rule::PercentFormatMixedPositionalAndNamed),
    RuleSelector::rule(Rule::PercentFormatPositionalCountMismatch),
    RuleSelector::rule(Rule::PercentFormatStarRequiresSequence),
    RuleSelector::rule(Rule::PercentFormatUnsupportedFormatCharacter),
    RuleSelector::rule(Rule::StringDotFormatInvalidFormat),
    RuleSelector::rule(Rule::StringDotFormatExtraNamedArguments),
    RuleSelector::rule(Rule::StringDotFormatExtraPositionalArguments),
    RuleSelector::rule(Rule::StringDotFormatMissingArguments),
    RuleSelector::rule(Rule::StringDotFormatMixingAutomatic),
    RuleSelector::rule(Rule::FStringMissingPlaceholders),
    RuleSelector::rule(Rule::MultiValueRepeatedKeyLiteral),
    RuleSelector::rule(Rule::MultiValueRepeatedKeyVariable),
    RuleSelector::rule(Rule::ExpressionsInStarAssignment),
    RuleSelector::rule(Rule::MultipleStarredExpressions),
    RuleSelector::rule(Rule::AssertTuple),
    RuleSelector::rule(Rule::IsLiteral),
    RuleSelector::rule(Rule::InvalidPrintSyntax),
    RuleSelector::rule(Rule::IfTuple),
    RuleSelector::rule(Rule::BreakOutsideLoop),
    RuleSelector::rule(Rule::ContinueOutsideLoop),
    RuleSelector::rule(Rule::YieldOutsideFunction),
    RuleSelector::rule(Rule::ReturnOutsideFunction),
    RuleSelector::rule(Rule::DefaultExceptNotLast),
    RuleSelector::rule(Rule::RedefinedWhileUnused),
    RuleSelector::rule(Rule::UndefinedName),
    RuleSelector::rule(Rule::UndefinedExport),
    RuleSelector::rule(Rule::UndefinedLocal),
    RuleSelector::rule(Rule::UnusedVariable),
    RuleSelector::rule(Rule::UnusedAnnotation),
    RuleSelector::rule(Rule::RaiseNotImplemented),
    RuleSelector::rule(Rule::FutureRewritableTypeAnnotation),
    RuleSelector::rule(Rule::FutureRequiredTypeAnnotation),
    RuleSelector::rule(Rule::StaticJoinToFString),
    RuleSelector::rule(Rule::PrintEmptyString),
    RuleSelector::rule(Rule::ForLoopWrites),
    RuleSelector::rule(Rule::ReadlinesInFor),
    RuleSelector::rule(Rule::CheckAndRemoveFromSet),
    RuleSelector::rule(Rule::IfExprMinMax),
    RuleSelector::rule(Rule::VerboseDecimalConstructor),
    RuleSelector::rule(Rule::BitCount),
    RuleSelector::rule(Rule::FromisoformatReplaceZ),
    RuleSelector::rule(Rule::RedundantLogBase),
    RuleSelector::rule(Rule::IntOnSlicedStr),
    RuleSelector::rule(Rule::RegexFlagAlias),
    RuleSelector::rule(Rule::IsinstanceTypeNone),
    RuleSelector::rule(Rule::TypeNoneComparison),
    RuleSelector::rule(Rule::ImplicitCwd),
    RuleSelector::rule(Rule::HashlibDigestHex),
    RuleSelector::rule(Rule::SliceToRemovePrefixOrSuffix),
    RuleSelector::rule(Rule::SortedMinMax),
    RuleSelector::rule(Rule::LoggingWarn),
    RuleSelector::rule(Rule::LoggingExtraAttrClash),
    RuleSelector::rule(Rule::LoggingExcInfo),
    RuleSelector::rule(Rule::LoggingRedundantExcInfo),
    RuleSelector::rule(Rule::UnsortedImports),
    RuleSelector::rule(Rule::FStringInGetTextFuncCall),
    RuleSelector::rule(Rule::FormatInGetTextFuncCall),
    RuleSelector::rule(Rule::PrintfInGetTextFuncCall),
    RuleSelector::rule(Rule::ImplicitStringConcatenationInCollectionLiteral),
    RuleSelector::rule(Rule::DirectLoggerInstantiation),
    RuleSelector::rule(Rule::InvalidGetLoggerArgument),
    RuleSelector::rule(Rule::UndocumentedWarn),
    RuleSelector::rule(Rule::ExcInfoOutsideExceptHandler),
    RuleSelector::rule(Rule::RootLoggerCall),
    RuleSelector::rule(Rule::InvalidModuleName),
    RuleSelector::rule(Rule::UnnecessaryListCast),
    RuleSelector::rule(Rule::IncorrectDictIterator),
    RuleSelector::rule(Rule::ManualListCopy),
    RuleSelector::rule(Rule::InvalidMockAccess),
    RuleSelector::rule(Rule::UnnecessaryPlaceholder),
    RuleSelector::rule(Rule::DuplicateClassFieldDefinition),
    RuleSelector::rule(Rule::NonUniqueEnums),
    RuleSelector::rule(Rule::UnnecessarySpread),
    RuleSelector::rule(Rule::UnnecessaryDictKwargs),
    RuleSelector::rule(Rule::ReimplementedContainerBuiltin),
    RuleSelector::rule(Rule::UnnecessaryRangeStart),
    RuleSelector::rule(Rule::MultipleStartsEndsWith),
    RuleSelector::rule(Rule::TypeNameIncorrectVariance),
    RuleSelector::rule(Rule::TypeBivariance),
    RuleSelector::rule(Rule::TypeParamNameMismatch),
    RuleSelector::rule(Rule::SingleStringSlots),
    RuleSelector::rule(Rule::DictIndexMissingItems),
    RuleSelector::rule(Rule::IterationOverSet),
    RuleSelector::rule(Rule::UselessImportAlias),
    RuleSelector::rule(Rule::UnnecessaryDirectLambdaCall),
    RuleSelector::rule(Rule::YieldInInit),
    RuleSelector::rule(Rule::ReturnInInit),
    RuleSelector::rule(Rule::NonlocalAndGlobal),
    RuleSelector::rule(Rule::ContinueInFinally),
    RuleSelector::rule(Rule::NonlocalWithoutBinding),
    RuleSelector::rule(Rule::LoadBeforeGlobalDeclaration),
    RuleSelector::rule(Rule::InvalidLengthReturnType),
    RuleSelector::rule(Rule::InvalidBoolReturnType),
    RuleSelector::rule(Rule::InvalidIndexReturnType),
    RuleSelector::rule(Rule::InvalidStrReturnType),
    RuleSelector::rule(Rule::InvalidBytesReturnType),
    RuleSelector::rule(Rule::InvalidHashReturnType),
    RuleSelector::rule(Rule::InvalidAllObject),
    RuleSelector::rule(Rule::InvalidAllFormat),
    RuleSelector::rule(Rule::PotentialIndexError),
    RuleSelector::rule(Rule::MisplacedBareRaise),
    RuleSelector::rule(Rule::RepeatedKeywordArgument),
    RuleSelector::rule(Rule::AwaitOutsideAsync),
    RuleSelector::rule(Rule::LoggingTooManyArgs),
    RuleSelector::rule(Rule::LoggingTooFewArgs),
    RuleSelector::rule(Rule::BadStringFormatCharacter),
    RuleSelector::rule(Rule::BadStringFormatType),
    RuleSelector::rule(Rule::BadStrStripCall),
    RuleSelector::rule(Rule::InvalidEnvvarValue),
    RuleSelector::rule(Rule::SingledispatchMethod),
    RuleSelector::rule(Rule::SingledispatchmethodFunction),
    RuleSelector::rule(Rule::YieldFromInAsyncFunction),
    RuleSelector::rule(Rule::BidirectionalUnicode),
    RuleSelector::rule(Rule::InvalidCharacterBackspace),
    RuleSelector::rule(Rule::InvalidCharacterSub),
    RuleSelector::rule(Rule::InvalidCharacterEsc),
    RuleSelector::rule(Rule::InvalidCharacterNul),
    RuleSelector::rule(Rule::InvalidCharacterZeroWidthSpace),
    RuleSelector::rule(Rule::ComparisonWithItself),
    RuleSelector::rule(Rule::ComparisonOfConstant),
    RuleSelector::rule(Rule::PropertyWithParameters),
    RuleSelector::rule(Rule::ManualFromImport),
    RuleSelector::rule(Rule::RedefinedArgumentFromLocal),
    RuleSelector::rule(Rule::StopIterationReturn),
    RuleSelector::rule(Rule::UselessReturn),
    RuleSelector::rule(Rule::BooleanChainedComparison),
    RuleSelector::rule(Rule::SysExitAlias),
    RuleSelector::rule(Rule::IfStmtMinMax),
    RuleSelector::rule(Rule::UnnecessaryDictIndexLookup),
    RuleSelector::rule(Rule::UnnecessaryListIndexLookup),
    RuleSelector::rule(Rule::EmptyComment),
    RuleSelector::rule(Rule::UselessElseOnLoop),
    RuleSelector::rule(Rule::SelfAssigningVariable),
    RuleSelector::rule(Rule::RedeclaredAssignedName),
    RuleSelector::rule(Rule::AssertOnStringLiteral),
    RuleSelector::rule(Rule::NamedExprWithoutContext),
    RuleSelector::rule(Rule::UselessExceptionStatement),
    RuleSelector::rule(Rule::NanComparison),
    RuleSelector::rule(Rule::BadStaticmethodArgument),
    RuleSelector::rule(Rule::SuperWithoutBrackets),
    RuleSelector::rule(Rule::ImportSelf),
    RuleSelector::rule(Rule::GlobalVariableNotAssigned),
    RuleSelector::rule(Rule::GlobalAtModuleLevel),
    RuleSelector::rule(Rule::SelfOrClsAssignment),
    RuleSelector::rule(Rule::BinaryOpException),
    RuleSelector::rule(Rule::BadOpenMode),
    RuleSelector::rule(Rule::ShallowCopyEnviron),
    RuleSelector::rule(Rule::InvalidEnvvarDefault),
    RuleSelector::rule(Rule::SubprocessPopenPreexecFn),
    RuleSelector::rule(Rule::SubprocessRunWithoutCheck),
    RuleSelector::rule(Rule::UselessWithLock),
    RuleSelector::rule(Rule::PytestRaisesWithoutException),
    RuleSelector::rule(Rule::PytestDuplicateParametrizeTestCases),
    RuleSelector::rule(Rule::PytestDeprecatedYieldFixture),
    RuleSelector::rule(Rule::PytestErroneousUseFixturesOnFixture),
    RuleSelector::rule(Rule::PytestUseFixturesWithoutParameters),
    RuleSelector::rule(Rule::PytestWarnsWithMultipleStatements),
    RuleSelector::rule(Rule::PyPath),
    RuleSelector::rule(Rule::InvalidPathlibWithSuffix),
    RuleSelector::rule(Rule::UnprefixedTypeParam),
    RuleSelector::rule(Rule::ComplexIfStatementInStub),
    RuleSelector::rule(Rule::UnrecognizedVersionInfoCheck),
    RuleSelector::rule(Rule::PatchVersionComparison),
    RuleSelector::rule(Rule::WrongTupleLengthVersionComparison),
    RuleSelector::rule(Rule::BadVersionInfoComparison),
    RuleSelector::rule(Rule::UnrecognizedPlatformCheck),
    RuleSelector::rule(Rule::UnrecognizedPlatformName),
    RuleSelector::rule(Rule::PassStatementStubBody),
    RuleSelector::rule(Rule::NonEmptyStubBody),
    RuleSelector::rule(Rule::PassInClassBody),
    RuleSelector::rule(Rule::EllipsisInNonEmptyClassBody),
    RuleSelector::rule(Rule::AssignmentDefaultInStub),
    RuleSelector::rule(Rule::DuplicateUnionMember),
    RuleSelector::rule(Rule::ComplexAssignmentInStub),
    RuleSelector::rule(Rule::UnusedPrivateTypeVar),
    RuleSelector::rule(Rule::CustomTypeVarForSelf),
    RuleSelector::rule(Rule::QuotedAnnotationInStub),
    RuleSelector::rule(Rule::UnaliasedCollectionsAbcSetImport),
    RuleSelector::rule(Rule::TypeAliasWithoutAnnotation),
    RuleSelector::rule(Rule::StrOrReprDefinedInStub),
    RuleSelector::rule(Rule::UnnecessaryLiteralUnion),
    RuleSelector::rule(Rule::AnyEqNeAnnotation),
    RuleSelector::rule(Rule::LegacyTypeComment),
    RuleSelector::rule(Rule::NonSelfReturnType),
    RuleSelector::rule(Rule::UnassignedSpecialVariableInStub),
    RuleSelector::rule(Rule::BadExitAnnotation),
    RuleSelector::rule(Rule::RedundantNumericUnion),
    RuleSelector::rule(Rule::SnakeCaseTypeAlias),
    RuleSelector::rule(Rule::TSuffixedTypeAlias),
    RuleSelector::rule(Rule::FutureAnnotationsInStub),
    RuleSelector::rule(Rule::IterMethodReturnIterable),
    RuleSelector::rule(Rule::UnusedPrivateProtocol),
    RuleSelector::rule(Rule::UnusedPrivateTypeAlias),
    RuleSelector::rule(Rule::StubBodyMultipleStatements),
    RuleSelector::rule(Rule::UnusedPrivateTypedDict),
    RuleSelector::rule(Rule::NoReturnArgumentAnnotationInStub),
    RuleSelector::rule(Rule::UnannotatedAssignmentInStub),
    RuleSelector::rule(Rule::UnnecessaryTypeUnion),
    RuleSelector::rule(Rule::ByteStringUsage),
    RuleSelector::rule(Rule::GeneratorReturnFromIterMethod),
    RuleSelector::rule(Rule::GenericNotLastBaseClass),
    RuleSelector::rule(Rule::RedundantNoneLiteral),
    RuleSelector::rule(Rule::DuplicateLiteralMember),
    RuleSelector::rule(Rule::Pep484StylePositionalOnlyParameter),
    RuleSelector::rule(Rule::RedundantFinalLiteral),
    RuleSelector::rule(Rule::BadVersionInfoOrder),
    RuleSelector::rule(Rule::UnnecessaryReturnNone),
    RuleSelector::rule(Rule::ZipInsteadOfPairwise),
    RuleSelector::rule(Rule::MutableDataclassDefault),
    RuleSelector::rule(Rule::FunctionCallInDataclassDefaultArgument),
    RuleSelector::rule(Rule::ExplicitFStringTypeConversion),
    RuleSelector::rule(Rule::MutableClassDefault),
    RuleSelector::rule(Rule::ImplicitOptional),
    RuleSelector::rule(Rule::UnnecessaryIterableAllocationForFirstElement),
    RuleSelector::rule(Rule::InvalidIndexType),
    RuleSelector::rule(Rule::QuadraticListSummation),
    RuleSelector::rule(Rule::AssignmentInAssert),
    RuleSelector::rule(Rule::UnnecessaryKeyCheck),
    RuleSelector::rule(Rule::NeverUnion),
    RuleSelector::rule(Rule::UnsortedDunderAll),
    RuleSelector::rule(Rule::UnsortedDunderSlots),
    RuleSelector::rule(Rule::MutableFromkeysValue),
    RuleSelector::rule(Rule::DefaultFactoryKwarg),
    RuleSelector::rule(Rule::InvalidFormatterSuppressionComment),
    RuleSelector::rule(Rule::AssertWithPrintMessage),
    RuleSelector::rule(Rule::DecimalFromFloatLiteral),
    RuleSelector::rule(Rule::PostInitDefault),
    RuleSelector::rule(Rule::UselessIfElse),
    RuleSelector::rule(Rule::InvalidAssertMessageLiteralArgument),
    RuleSelector::rule(Rule::UnnecessaryNestedLiteral),
    RuleSelector::rule(Rule::UnnecessaryCastToInt),
    RuleSelector::rule(Rule::MapIntVersionParsing),
    RuleSelector::rule(Rule::DataclassEnum),
    RuleSelector::rule(Rule::IfKeyInDictDel),
    RuleSelector::rule(Rule::ClassWithMixedTypeVars),
    RuleSelector::rule(Rule::UnnecessaryRound),
    RuleSelector::rule(Rule::StarmapZip),
    RuleSelector::rule(Rule::UnusedUnpackedVariable),
    RuleSelector::rule(Rule::AccessAnnotationsFromClassDict),
    RuleSelector::rule(Rule::DuplicateEntryInDunderAll),
    RuleSelector::rule(Rule::UnusedNOQA),
    RuleSelector::rule(Rule::RedirectedNOQA),
    RuleSelector::rule(Rule::InvalidPyprojectToml),
    RuleSelector::rule(Rule::ExecBuiltin),
    RuleSelector::rule(Rule::TryExceptPass),
    RuleSelector::rule(Rule::TryExceptContinue),
    RuleSelector::rule(Rule::DuplicateIsinstanceCall),
    RuleSelector::rule(Rule::CollapsibleIf),
    RuleSelector::rule(Rule::NeedlessBool),
    RuleSelector::rule(Rule::ReturnInTryExceptFinally),
    RuleSelector::rule(Rule::EnumerateForLoop),
    RuleSelector::rule(Rule::IfWithSameArms),
    RuleSelector::rule(Rule::OpenFileWithContextHandler),
    RuleSelector::rule(Rule::MultipleWithStatements),
    RuleSelector::rule(Rule::InDictKeys),
    RuleSelector::rule(Rule::NegateEqualOp),
    RuleSelector::rule(Rule::NegateNotEqualOp),
    RuleSelector::rule(Rule::DoubleNegation),
    RuleSelector::rule(Rule::IfExprWithTrueFalse),
    RuleSelector::rule(Rule::IfExprWithFalseTrue),
    RuleSelector::rule(Rule::ExprAndNotExpr),
    RuleSelector::rule(Rule::ExprOrNotExpr),
    RuleSelector::rule(Rule::ExprOrTrue),
    RuleSelector::rule(Rule::ExprAndFalse),
    RuleSelector::rule(Rule::IfElseBlockInsteadOfDictGet),
    RuleSelector::rule(Rule::SplitStaticString),
    RuleSelector::rule(Rule::ZipDictKeysAndValues),
    RuleSelector::rule(Rule::Debugger),
    RuleSelector::rule(Rule::RuntimeImportInTypeCheckingBlock),
    RuleSelector::rule(Rule::EmptyTypeCheckingBlock),
    RuleSelector::rule(Rule::UnquotedTypeAlias),
    RuleSelector::rule(Rule::RuntimeStringUnion),
    RuleSelector::rule(Rule::RaiseVanillaClass),
    RuleSelector::rule(Rule::TypeCheckWithoutTypeError),
    RuleSelector::rule(Rule::VerboseRaise),
    RuleSelector::rule(Rule::UselessTryExcept),
    RuleSelector::rule(Rule::VerboseLogMessage),
    RuleSelector::rule(Rule::UselessMetaclassType),
    RuleSelector::rule(Rule::TypeOfPrimitive),
    RuleSelector::rule(Rule::UselessObjectInheritance),
    RuleSelector::rule(Rule::DeprecatedUnittestAlias),
    RuleSelector::rule(Rule::NonPEP585Annotation),
    RuleSelector::rule(Rule::NonPEP604AnnotationUnion),
    RuleSelector::rule(Rule::SuperCallWithParameters),
    RuleSelector::rule(Rule::UTF8EncodingDeclaration),
    RuleSelector::rule(Rule::UnnecessaryFutureImport),
    RuleSelector::rule(Rule::LRUCacheWithoutParameters),
    RuleSelector::rule(Rule::UnnecessaryEncodeUTF8),
    RuleSelector::rule(Rule::ConvertNamedTupleFunctionalToClass),
    RuleSelector::rule(Rule::DatetimeTimezoneUTC),
    RuleSelector::rule(Rule::NativeLiterals),
    RuleSelector::rule(Rule::TypingTextStrAlias),
    RuleSelector::rule(Rule::OpenAlias),
    RuleSelector::rule(Rule::ReplaceUniversalNewlines),
    RuleSelector::rule(Rule::ReplaceStdoutStderr),
    RuleSelector::rule(Rule::DeprecatedCElementTree),
    RuleSelector::rule(Rule::OSErrorAlias),
    RuleSelector::rule(Rule::UnicodeKindPrefix),
    RuleSelector::rule(Rule::DeprecatedMockImport),
    RuleSelector::rule(Rule::YieldInForLoop),
    RuleSelector::rule(Rule::UnnecessaryBuiltinImport),
    RuleSelector::rule(Rule::FormatLiterals),
    RuleSelector::rule(Rule::PrintfStringFormatting),
    RuleSelector::rule(Rule::FString),
    RuleSelector::rule(Rule::LRUCacheWithMaxsizeNone),
    RuleSelector::rule(Rule::ExtraneousParentheses),
    RuleSelector::rule(Rule::DeprecatedImport),
    RuleSelector::rule(Rule::OutdatedVersionBlock),
    RuleSelector::rule(Rule::QuotedAnnotation),
    RuleSelector::rule(Rule::UnnecessaryClassParentheses),
    RuleSelector::rule(Rule::NonPEP695TypeAlias),
    RuleSelector::rule(Rule::TimeoutErrorAlias),
    RuleSelector::rule(Rule::UnnecessaryDefaultTypeArgs),
    RuleSelector::rule(Rule::NonPEP646Unpack),
    RuleSelector::rule(Rule::NonPEP604AnnotationOptional),
    RuleSelector::rule(Rule::NonPEP695GenericClass),
    RuleSelector::rule(Rule::NonPEP695GenericFunction),
    RuleSelector::rule(Rule::PrivateTypeParameter),
    RuleSelector::rule(Rule::UselessClassMetaclassType),
    RuleSelector::rule(Rule::InvalidEscapeSequence),
    RuleSelector::rule(Rule::SysVersionSlice3),
    RuleSelector::rule(Rule::SysVersion2),
    RuleSelector::rule(Rule::SysVersionCmpStr3),
    RuleSelector::rule(Rule::SysVersionInfo0Eq3),
    RuleSelector::rule(Rule::SixPY3),
    RuleSelector::rule(Rule::SysVersionInfo1CmpInt),
    RuleSelector::rule(Rule::SysVersionInfoMinorCmpInt),
    RuleSelector::rule(Rule::SysVersion0),
    RuleSelector::rule(Rule::SysVersionCmpStr10),
    RuleSelector::rule(Rule::SysVersionSlice1),
];

pub const TASK_TAGS: &[&str] = &["TODO", "FIXME", "XXX"];

pub static DUMMY_VARIABLE_RGX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(_+|(_+[\p{XID_Continue}]*[\p{XID_Continue}--_]+?))$").unwrap());

impl LinterSettings {
    pub fn for_rule(rule_code: Rule) -> Self {
        Self {
            rules: RuleTable::from_iter([rule_code]),
            unresolved_target_version: PythonVersion::latest().into(),
            ..Self::default()
        }
    }

    pub fn for_rules(rules: impl IntoIterator<Item = Rule>) -> Self {
        Self {
            rules: RuleTable::from_iter(rules),
            unresolved_target_version: PythonVersion::latest().into(),
            ..Self::default()
        }
    }

    pub fn new(project_root: &Path) -> Self {
        Self {
            exclude: FilePatternSet::default(),
            unresolved_target_version: TargetVersion(None),
            per_file_target_version: CompiledPerFileTargetVersionList::default(),
            project_root: project_root.to_path_buf(),
            rules: DEFAULT_SELECTORS
                .iter()
                .flat_map(|selector| selector.rules(&PreviewOptions::default()))
                .collect(),
            allowed_confusables: FxHashSet::from_iter([]),

            builtins: vec![],
            dummy_variable_rgx: DUMMY_VARIABLE_RGX.clone(),

            external: vec![],
            ignore_init_module_imports: true,
            logger_objects: vec![],
            namespace_packages: vec![],

            per_file_ignores: CompiledPerFileIgnoreList::default(),
            fix_safety: FixSafetyTable::default(),

            src: vec![fs::get_cwd().to_path_buf(), fs::get_cwd().join("src")],
            tab_size: IndentWidth::default(),
            line_length: LineLength::default(),

            task_tags: TASK_TAGS.iter().map(ToString::to_string).collect(),
            typing_modules: vec![],
            flake8_annotations: flake8_annotations::settings::Settings::default(),
            flake8_bandit: flake8_bandit::settings::Settings::default(),
            flake8_boolean_trap: flake8_boolean_trap::settings::Settings::default(),
            flake8_bugbear: flake8_bugbear::settings::Settings::default(),
            flake8_builtins: flake8_builtins::settings::Settings::default(),
            flake8_comprehensions: flake8_comprehensions::settings::Settings::default(),
            flake8_copyright: flake8_copyright::settings::Settings::default(),
            flake8_errmsg: flake8_errmsg::settings::Settings::default(),
            flake8_gettext: flake8_gettext::settings::Settings::default(),
            flake8_implicit_str_concat: flake8_implicit_str_concat::settings::Settings::default(),
            flake8_import_conventions: flake8_import_conventions::settings::Settings::default(),
            flake8_pytest_style: flake8_pytest_style::settings::Settings::default(),
            flake8_quotes: flake8_quotes::settings::Settings::default(),
            flake8_self: flake8_self::settings::Settings::default(),
            flake8_tidy_imports: flake8_tidy_imports::settings::Settings::default(),
            flake8_type_checking: flake8_type_checking::settings::Settings::default(),
            flake8_unused_arguments: flake8_unused_arguments::settings::Settings::default(),
            isort: isort::settings::Settings::default(),
            mccabe: mccabe::settings::Settings::default(),
            pep8_naming: pep8_naming::settings::Settings::default(),
            pycodestyle: pycodestyle::settings::Settings::default(),
            pydoclint: pydoclint::settings::Settings::default(),
            pydocstyle: pydocstyle::settings::Settings::default(),
            pyflakes: pyflakes::settings::Settings::default(),
            pylint: pylint::settings::Settings::default(),
            pyupgrade: pyupgrade::settings::Settings::default(),
            ruff: ruff::settings::Settings::default(),
            preview: PreviewMode::default(),
            explicit_preview_rules: false,
            extension: ExtensionMapping::default(),
            typing_extensions: true,
            future_annotations: false,
        }
    }

    #[must_use]
    pub fn with_target_version(mut self, target_version: PythonVersion) -> Self {
        self.unresolved_target_version = target_version.into();
        self
    }

    #[must_use]
    pub fn with_preview_mode(mut self) -> Self {
        self.preview = PreviewMode::Enabled;
        self
    }

    #[must_use]
    pub fn with_external_rules(mut self, rules: &[&str]) -> Self {
        self.external
            .extend(rules.iter().map(std::string::ToString::to_string));
        self
    }

    /// Resolve the [`TargetVersion`] to use for linting.
    ///
    /// This method respects the per-file version overrides in
    /// [`LinterSettings::per_file_target_version`] and falls back on
    /// [`LinterSettings::unresolved_target_version`] if none of the override patterns match.
    pub fn resolve_target_version(&self, path: &Path) -> TargetVersion {
        self.per_file_target_version
            .is_match(path)
            .map_or(self.unresolved_target_version, TargetVersion::from)
    }
}

impl Default for LinterSettings {
    fn default() -> Self {
        Self::new(fs::get_cwd())
    }
}

/// A thin wrapper around `Option<PythonVersion>` to clarify the reason for different `unwrap`
/// calls in various places.
///
/// For example, we want to default to `PythonVersion::latest()` for parsing and detecting semantic
/// syntax errors because this will minimize version-related diagnostics when the Python version is
/// unset. In contrast, we want to default to `PythonVersion::default()` for lint rules. These
/// correspond to the [`TargetVersion::parser_version`] and [`TargetVersion::linter_version`]
/// methods, respectively.
#[derive(Debug, Clone, Copy, CacheKey, PartialEq, Eq)]
pub struct TargetVersion(pub Option<PythonVersion>);

impl TargetVersion {
    /// Return the [`PythonVersion`] to use for parsing.
    ///
    /// This will be either the Python version specified by the user or the latest supported
    /// version if unset.
    pub fn parser_version(&self) -> PythonVersion {
        self.0.unwrap_or_else(PythonVersion::latest)
    }

    /// Return the [`PythonVersion`] to use for version-dependent lint rules.
    ///
    /// This will either be the Python version specified by the user or the default Python version
    /// if unset.
    pub fn linter_version(&self) -> PythonVersion {
        self.0.unwrap_or_default()
    }
}

impl From<PythonVersion> for TargetVersion {
    fn from(value: PythonVersion) -> Self {
        Self(Some(value))
    }
}

impl Display for TargetVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RuleSelector;
    use crate::codes::Category;
    use crate::registry::RuleSet;
    use crate::rule_selector::PreviewOptions;
    use crate::settings::types::PreviewMode;

    use super::DEFAULT_SELECTORS;

    #[test]
    fn preview_default_rules() {
        let stable_defaults = DEFAULT_SELECTORS
            .iter()
            .flat_map(|selector| selector.rules(&PreviewOptions::default()))
            .collect::<RuleSet>();

        let preview_options = PreviewOptions {
            mode: PreviewMode::Enabled,
            require_explicit: false,
        };
        let preview_defaults = Category::default_categories()
            .map(RuleSelector::Category)
            .iter()
            .flat_map(|selector| selector.rules(&preview_options))
            .collect::<RuleSet>();

        let added = preview_defaults.clone().subtract(&stable_defaults);
        let removed = stable_defaults.subtract(&preview_defaults);

        let snapshot = format!("Added in preview:\n{added}\n\nRemoved in preview:\n{removed}");

        insta::assert_snapshot!(snapshot, @"
        Added in preview:
        [
        	airflow-variable-name-task-id-mismatch (AIR001),
        	airflow-dag-no-schedule-argument (AIR002),
        	airflow-variable-get-outside-task (AIR003),
        	airflow-task-branch-as-short-circuit (AIR004),
        	airflow-xcom-pull-in-template-string (AIR201),
        	airflow-task-implicit-multiple-outputs (AIR202),
        	airflow3-dag-dynamic-value (AIR304),
        	fast-api-redundant-response-model (FAST001),
        	fast-api-non-annotated-dependency (FAST002),
        	fast-api-unused-path-parameter (FAST003),
        	blocking-http-call-httpx-in-async-function (ASYNC212),
        	blocking-path-method-in-async-function (ASYNC240),
        	blocking-input-in-async-function (ASYNC250),
        	abstract-base-class-without-abstract-method (B024),
        	empty-method-without-abstract-decorator (B027),
        	del-attr-with-constant (B043),
        	return-in-generator (B901),
        	loop-iterator-mutation (B909),
        	trailing-comma-on-bare-tuple (COM818),
        	unnecessary-dict-comprehension-for-iterable (C420),
        	django-model-without-dunder-str (DJ008),
        	django-unordered-body-content-in-model (DJ012),
        	django-non-leading-receiver-decorator (DJ013),
        	pytest-patch-with-lambda (PT008),
        	pytest-raises-with-multiple-statements (PT012),
        	pytest-unnecessary-asyncio-mark-on-fixture (PT024),
        	pytest-parameter-with-default-argument (PT028),
        	reimplemented-builtin (SIM110),
        	lazy-import-immediately-resolved (TID255),
        	numpy-deprecated-type-alias (NPY001),
        	numpy-legacy-random (NPY002),
        	numpy-deprecated-function (NPY003),
        	numpy2-deprecation (NPY201),
        	gettext-variable (E8502),
        	gettext-placeholders (E8503),
        	gettext-repr (E8504),
        	missing-gettext (E8505),
        	raise-unlink-override (E8506),
        	n-plus-one-query (E8507),
        	onchange-domain (E8509),
        	config-chainmap-patch (E8510),
        	gettext-developer-error (E8511),
        	shadowed-definition (E8513),
        	tax-company-singular (E8514),
        	http-json-string (E8515),
        	row-counter-in-test (E8516),
        	field-redeclared (E8521),
        	default-evaluated-at-import (E8522),
        	selection-duplicate-key (E8523),
        	field-hook-prefix (E8524),
        	field-positional-argument (E8525),
        	field-attribute-order (E8526),
        	dead-field-attribute (E8527),
        	stored-related (E8529),
        	route-untyped (E8533),
        	sql-bound-placeholder (E8534),
        	abolished-method-call (E8535),
        	user-cache-without-groups (E8540),
        	http-exception-returned (E8541),
        	wsgi-environ-optional-key (E8542),
        	empty-recordset-mutation (E8543),
        	noqa-rationale (E8544),
        	isdigit-guards-int (E8545),
        	pandas-use-of-dot-is-null (PD003),
        	pandas-use-of-dot-not-null (PD004),
        	pandas-use-of-dot-read-table (PD012),
        	pandas-use-of-pd-merge (PD015),
        	escape-sequence-in-docstring (D301),
        	missing-maxsplit-arg (PLC0207),
        	unnecessary-dunder-call (PLC2801),
        	no-method-argument (PLE0211),
        	duplicate-bases (PLE0241),
        	dict-iter-missing-items (PLE1141),
        	modified-iterating-set (PLE4703),
        	no-classmethod-decorator (PLR0202),
        	no-staticmethod-decorator (PLR0203),
        	swap-with-temporary-variable (PLR1712),
        	unnecessary-lambda (PLW0108),
        	redefined-slots-in-subclass (PLW0244),
        	replace-str-enum (UP042),
        	while-one (UP048),
        	deprecated-abc-decorator (UP051),
        	context-manager-iterator (UP052),
        	if-exp-instead-of-or-operator (FURB110),
        	repeated-append (FURB113),
        	delete-full-slice (FURB131),
        	for-loop-set-mutations (FURB142),
        	slice-copy (FURB145),
        	unnecessary-enumerate (FURB148),
        	math-constant (FURB152),
        	hardcoded-string-charset (FURB156),
        	single-item-membership-test (FURB171),
        	meta-class-abc-meta (FURB180),
        	subclass-builtin (FURB189),
        	missing-f-string-syntax (RUF027),
        	none-not-at-end-of-union (RUF036),
        	unnecessary-empty-iterable-within-deque-call (RUF037),
        	redundant-bool-literal (RUF038),
        	pytest-raises-ambiguous-pattern (RUF043),
        	implicit-class-var-in-dataclass (RUF045),
        	needless-else (RUF047),
        	unnecessary-if (RUF050),
        	indented-form-feed (RUF054),
        	unnecessary-regular-expression (RUF055),
        	falsy-dict-get-fallback (RUF056),
        	in-empty-collection (RUF060),
        	legacy-form-pytest-raises (RUF061),
        	non-octal-permissions (RUF064),
        	logging-eager-conversion (RUF065),
        	property-without-return (RUF066),
        	float-equality-comparison (RUF069),
        	os-path-commonprefix (RUF071),
        	useless-finally (RUF072),
        	f-string-percent-format (RUF073),
        	incorrect-decorator-order (RUF074),
        	fallible-context-manager (RUF075),
        	method-receiver-default (RUF077),
        	invalid-rule-code (RUF102),
        	invalid-suppression-comment (RUF103),
        	unmatched-suppression-comment (RUF104),
        	rule-codes-in-suppression-comments (RUF106),
        	renamed-rule (RUF202),
        	shared-clock-mock,
        	unnecessary-regular-expression-compile,
        ]

        Removed in preview:
        [
        	exec-builtin (S102),
        ]
        ");
    }
}
