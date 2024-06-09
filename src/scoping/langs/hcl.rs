use super::{CodeQuery, Language, LanguageScoper, TSLanguage, TSQuery};
use crate::scoping::{langs::IGNORE, ROScopes, Scoper};
use clap::ValueEnum;
use const_format::formatcp;
use std::{fmt::Debug, str::FromStr};
use tree_sitter::QueryError;

/// The Hashicorp Configuration Language.
pub type Hcl = Language<HclQuery>;
/// A query for HCL.
pub type HclQuery = CodeQuery<CustomHclQuery, PremadeHclQuery>;

/// Premade tree-sitter queries for Hcl.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PremadeHclQuery {
    /// Variable declarations and usages.
    Variables,
    /// `resource` name declarations and usages.
    ///
    /// In `resource "a" "b"`, only "b" is matched.
    ResourceNames,
    /// `resource` type declarations and usages.
    ///
    /// In `resource "a" "b"`, only "a" is matched.
    ResourceTypes,
    /// `data` name declarations and usages.
    ///
    /// In `data "a" "b"`, only "b" is matched.
    DataNames,
    /// `data` source declarations and usages.
    ///
    /// In `data "a" "b"`, only "a" is matched.
    DataSources,
    /// Comments.
    Comments,
    /// Literal strings.
    ///
    /// Excluding resource, variable, ... names as well as interpolation parts.
    Strings,
}

impl From<PremadeHclQuery> for TSQuery {
    #[allow(clippy::too_many_lines)] // No good way to avoid
    fn from(value: PremadeHclQuery) -> Self {
        TSQuery::new(
            &Hcl::lang(),
            // Seems to not play nice with the macro. Put up here, else interpolation is
            // affected.
            #[allow(clippy::needless_raw_string_hashes)]
            match value {
                PremadeHclQuery::Variables => {
                    // Capturing nodes with names, such as `@id`, requires names to be
                    // unique across the *entire* query, else things break. Hence, us
                    // `@a.b` syntax (which seems undocumented).
                    formatcp!(
                        r#"
                            [
                                (block
                                    (identifier) @{0}.declaration
                                    (string_lit) @name.declaration
                                    (#match? @{0}.declaration "variable")
                                )
                                (
                                    (variable_expr
                                        (identifier) @{0}.usage
                                        (#match? @{0}.usage "var")
                                    )
                                    .
                                    (get_attr
                                        (identifier) @name.usage
                                    )
                                )
                            ]
                        "#,
                        IGNORE
                    )
                }
                PremadeHclQuery::ResourceNames => {
                    // Capturing nodes with names, such as `@id`, requires names to be
                    // unique across the *entire* query, else things break. Hence, us
                    // `@a.b` syntax (which seems undocumented).
                    formatcp!(
                        r#"
                            [
                                (block
                                    (identifier) @{0}.declaration
                                    (string_lit)
                                    (string_lit) @name.declaration
                                    (#match? @{0}.declaration "resource")
                                )
                                (
                                    (variable_expr
                                        (identifier) @{0}.usage
                                        (#not-any-of? @{0}.usage
                                            "var"
                                            "data"
                                            "module"
                                            "local"
                                        )
                                    )
                                    .
                                    (get_attr
                                        (identifier) @name.usage
                                    )
                                )
                            ]
                        "#,
                        IGNORE
                    )
                }
                PremadeHclQuery::ResourceTypes => {
                    // Capturing nodes with names, such as `@id`, requires names to be
                    // unique across the *entire* query, else things break. Hence, us
                    // `@a.b` syntax (which seems undocumented).
                    formatcp!(
                        r#"
                            [
                                (block
                                    (identifier) @{0}.declaration
                                    (string_lit) @name.type
                                    (string_lit)
                                    (#match? @{0}.declaration "resource")
                                )
                                (
                                    (variable_expr
                                        .
                                        (identifier) @name.usage
                                        (#not-any-of? @name.usage
                                            "var"
                                            "data"
                                            "module"
                                            "local"
                                        )
                                    )
                                    .
                                    (get_attr
                                        (identifier)
                                    )
                                )
                            ]
                        "#,
                        IGNORE
                    )
                }
                PremadeHclQuery::DataNames => {
                    // Capturing nodes with names, such as `@id`, requires names to be
                    // unique across the *entire* query, else things break. Hence, us
                    // `@a.b` syntax (which seems undocumented).
                    formatcp!(
                        r#"
                            [
                                (block
                                    (identifier) @{0}.declaration
                                    (string_lit)
                                    (string_lit) @name.declaration
                                    (#match? @{0}.declaration "data")
                                )
                                (
                                    (variable_expr
                                        (identifier) @{0}.usage
                                        (#match? @{0}.usage "data")
                                    )
                                    .
                                    (get_attr
                                        (identifier)
                                    )
                                    .
                                    (get_attr
                                        (identifier) @name.usage
                                    )
                                )
                            ]
                        "#,
                        IGNORE
                    )
                }
                PremadeHclQuery::DataSources => {
                    // Capturing nodes with names, such as `@id`, requires names to be
                    // unique across the *entire* query, else things break. Hence, us
                    // `@a.b` syntax (which seems undocumented).
                    formatcp!(
                        r#"
                            [
                                (block
                                    (identifier) @{0}.declaration
                                    (string_lit) @name.provider
                                    (string_lit)
                                    (#match? @{0}.declaration "data")
                                )
                                (
                                    (variable_expr
                                        (identifier) @{0}.usage
                                        (#match? @{0}.usage "data")
                                    )
                                    .
                                    (get_attr
                                        (identifier) @name.provider
                                    )
                                    .
                                    (get_attr
                                        (identifier)
                                    )
                                )
                            ]
                        "#,
                        IGNORE
                    )
                }
                PremadeHclQuery::Comments => "(comment) @comment",
                PremadeHclQuery::Strings => {
                    r"
                    [
                        (literal_value (string_lit) @string.literal)
                        (quoted_template
                            (template_literal) @string.template_literal
                        )
                        (heredoc_template
                            (template_literal) @string.heredoc_literal
                        )
                    ]
                    "
                }
            },
        )
        .expect("Premade queries to be valid")
    }
}

/// A custom tree-sitter query for HCL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomHclQuery(String);

impl FromStr for CustomHclQuery {
    type Err = QueryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match TSQuery::new(&Hcl::lang(), s) {
            Ok(_) => Ok(Self(s.to_string())),
            Err(e) => Err(e),
        }
    }
}

impl From<CustomHclQuery> for TSQuery {
    fn from(value: CustomHclQuery) -> Self {
        TSQuery::new(&Hcl::lang(), &value.0)
            .expect("Valid query, as object cannot be constructed otherwise")
    }
}

impl Scoper for Hcl {
    fn scope<'viewee>(&self, input: &'viewee str) -> ROScopes<'viewee> {
        ROScopes::from_raw_ranges(
            input,
            Self::scope_via_query(&mut self.query(), input).into(),
        )
    }
}

impl LanguageScoper for Hcl {
    fn lang() -> TSLanguage {
        tree_sitter_hcl::language()
    }

    fn query(&self) -> TSQuery {
        self.query.clone().into()
    }
}