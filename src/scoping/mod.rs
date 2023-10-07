//! Items for defining the scope actions are applied within.

use self::literal::LiteralError;
use self::regex::RegexError;
use itertools::Itertools;
use log::{debug, trace};
use std::fmt;
use std::{borrow::Cow, ops::Range};

pub mod langs;
pub mod literal;
pub mod regex;

#[derive(Debug)]
pub enum ScoperBuildError {
    EmptyScope,
    RegexError(RegexError),
    LiteralError(LiteralError),
}

impl From<LiteralError> for ScoperBuildError {
    fn from(e: LiteralError) -> Self {
        Self::LiteralError(e)
    }
}

impl From<RegexError> for ScoperBuildError {
    fn from(e: RegexError) -> Self {
        Self::RegexError(e)
    }
}

pub trait ScopedViewBuildStep {
    fn scope<'a>(&self, input: &'a str) -> ScopedViewBuilder<'a>;
}

impl fmt::Debug for dyn ScopedViewBuildStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scoper").finish()
    }
}

/// Indicates whether a given string part is in scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope<'a, T> {
    /// The given part is in scope for processing.
    In(T),
    /// The given part is out of scope for processing.
    ///
    /// Treated as immutable, view-only.
    Out(&'a str),
}

type ROScope<'a> = Scope<'a, &'a str>;
type ROScopes<'a> = Vec<ROScope<'a>>;

type RWScope<'a> = Scope<'a, Cow<'a, str>>;
type RWScopes<'a> = Vec<RWScope<'a>>;

impl<'a> ROScope<'a> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let s: &str = self.into();
        s.is_empty()
    }
}

impl<'a> From<&'a ROScope<'a>> for &'a str {
    /// Get the underlying string slice of a [`ScopeStatus`].
    ///
    /// All variants contain such a slice, so this is a convenient method.
    fn from(s: &'a ROScope) -> Self {
        match s {
            Scope::In(s) | Scope::Out(s) => s,
        }
    }
}

impl<'a> From<ROScope<'a>> for RWScope<'a> {
    fn from(s: ROScope<'a>) -> Self {
        match s {
            Scope::In(s) => RWScope::In(Cow::Borrowed(s)),
            Scope::Out(s) => RWScope::Out(s),
        }
    }
}

impl<'a> From<&'a RWScope<'a>> for &'a str {
    /// Get the underlying string slice of a [`ScopeStatus`].
    ///
    /// All variants contain such a slice, so this is a convenient method.
    fn from(s: &'a RWScope) -> Self {
        match s {
            Scope::In(s) => s,
            Scope::Out(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedViewBuilder<'a> {
    scopes: ROScopes<'a>,
}

impl<'a> ScopedViewBuilder<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            scopes: vec![Scope::In(input)],
        }
    }

    #[must_use]
    pub fn build(self) -> ScopedView<'a> {
        ScopedView {
            scopes: self
                .scopes
                .into_iter()
                .map(std::convert::Into::into)
                .collect(),
        }
    }
}

impl<'a> IntoIterator for ScopedViewBuilder<'a> {
    type Item = ROScope<'a>;

    type IntoIter = std::vec::IntoIter<ROScope<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.scopes.into_iter()
    }
}

impl<'a> ScopedViewBuilder<'a> {
    #[must_use]
    pub fn explode_from_ranges(self, exploder: impl Fn(&str) -> Vec<Range<usize>>) -> Self {
        self.explode(|s| {
            trace!("Exploding from ranges: {:?}", s);

            let ranges = exploder(s);
            trace!("Raw ranges after exploding: {:?}", ranges);

            let mut scopes = Vec::new();

            let mut last_end = 0;
            for Range { start, end } in ranges.into_iter().sorted_by_key(|r| r.start) {
                scopes.push(Scope::Out(&s[last_end..start]));
                scopes.push(Scope::In(&s[start..end]));
                last_end = end;
            }

            if last_end < s.len() {
                scopes.push(Scope::Out(&s[last_end..]));
            }

            scopes.retain(|s| !s.is_empty());

            debug!("Scopes: {:?}", scopes);

            ScopedViewBuilder { scopes }
        })
    }

    #[must_use]
    pub fn explode_from_scoper(self, scoper: &impl ScopedViewBuildStep) -> Self {
        self.explode(|s| scoper.scope(s))
    }

    #[must_use]
    pub fn explode<F>(mut self, exploder: F) -> Self
    where
        F: Fn(&'a str) -> Self,
    {
        trace!("Exploding scopes: {:?}", self.scopes);
        let mut new = Vec::with_capacity(self.scopes.len());
        for scope in self.scopes.drain(..) {
            trace!("Exploding scope: {:?}", scope);

            if scope.is_empty() {
                trace!("Skipping empty scope");
                continue;
            }

            match scope {
                Scope::In(s) => {
                    let mut new_scopes = exploder(s).scopes;
                    new_scopes.retain(|s| !s.is_empty());
                    new.extend(new_scopes);
                }
                // Be explicit about the `Out(_)` case, so changing the enum is a
                // compile error
                Scope::Out("") => {}
                out @ Scope::Out(_) => new.push(out),
            }

            trace!("Exploded scope, new scopes are: {:?}", new);
        }
        trace!("Done exploding scopes.");

        ScopedViewBuilder { scopes: new }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedView<'a> {
    scopes: RWScopes<'a>,
}

impl<'a> ScopedView<'a> {
    #[must_use]
    pub fn new(scopes: RWScopes<'a>) -> Self {
        Self { scopes }
    }

    /// For API discoverability.
    #[must_use]
    pub fn builder(input: &'a str) -> ScopedViewBuilder<'a> {
        ScopedViewBuilder::new(input)
    }

    /// submit a function to be applied to each in-scope, returning out-scopes unchanged
    pub fn map<F>(&mut self, f: &F) -> &mut Self
    where
        F: Fn(&str) -> <str as ToOwned>::Owned,
    {
        for scope in &mut self.scopes {
            match scope {
                Scope::In(s) => {
                    let res = f(s);
                    debug!(
                        "Replacing '{}' with '{}'",
                        s.escape_debug(),
                        res.escape_debug()
                    );
                    *scope = Scope::In(Cow::Owned(res));
                }
                Scope::Out(s) => {
                    debug!("Appending '{}'", s.escape_debug());
                }
            }
        }

        self
    }

    pub fn into_inner_mut(&mut self) -> &mut RWScopes<'a> {
        self.scopes.as_mut()
    }
}

impl fmt::Display for ScopedView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for scope in &self.scopes {
            let s: &str = scope.into();
            write!(f, "{s}")?;
        }
        Ok(())
    }
}
