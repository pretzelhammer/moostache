// moostache readme rendered on docs.rs
#![doc = include_str!("../crates-io.md")]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// ignored lints
#![allow(
    clippy::needless_pass_by_value,
    clippy::enum_glob_use,
    clippy::enum_variant_names,
)]

use fnv::FnvBuildHasher;
use lru::LruCache;
use serde::Serialize;
use serde_json::json;
use winnow::{
    ascii::multispace0,
    combinator::{alt, cut_err, delimited, repeat, separated},
    error::{AddContext, ErrMode, ErrorKind, ParserError as WParserError},
    stream::{FindSlice, Stream},
    token::{literal, take_while},
    PResult,
    Parser,
    Stateful,
};
use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    fs,
    hash::{BuildHasher, BuildHasherDefault, Hash},
    io::{self, Write},
    num::NonZeroUsize,
    ops::Deref,
    path::{Path, PathBuf, MAIN_SEPARATOR_STR},
    rc::Rc,
};
use walkdir::WalkDir;

#[cfg(test)]
mod tests;

/////////////
// PARSING //
/////////////

// Source strings are parsed into template fragments.
// A "compiled" template is just a list of fragments
// and list of "section skips". See SectionSkip.
#[derive(PartialEq, Debug)]
enum Fragment<'src> {
    Literal(&'src str),
    EscapedVariable(&'src str),
    UnescapedVariable(&'src str),
    Section(&'src str),
    InvertedSection(&'src str),
    Partial(&'src str),
}

// We have a stateful parser, and that state
// is maintained in this struct.
#[derive(Debug)]
struct State<'src, 'skips> {
    fragment_index: usize,
    section_index: usize,
    section_starts: Vec<SectionMeta<'src>>,
    section_skips: &'skips mut Vec<SectionSkip>,
}

// Things our stateful parser needs to keep track of.
// Mostly this is for checking that all sections which
// are opened are correctly closed, and also for calculating
// section skips.
impl<'src, 'skips> State<'src, 'skips> {
    fn visited_fragment(&mut self) {
        self.fragment_index += 1;
    }
    fn visited_section_start(&mut self, name: &'src str) {
        self.section_starts.push(SectionMeta {
            name,
            section_index: self.section_index,
            fragment_index: self.fragment_index,
        });
        self.section_skips.push(SectionSkip {
            nested_sections: 0,
            nested_fragments: 0,
        });
        self.fragment_index += 1;
        self.section_index += 1;
    }
    fn visited_section_end(&mut self, name: &'src str) -> Result<(), ()> {
        let start = self.section_starts
            .pop()
            .ok_or(())?;
        if start.name != name {
            return Err(());
        }
        let skip = &mut self.section_skips[start.section_index];
        skip.nested_sections = u16::try_from((self.section_index - 1) - start.section_index)
            .expect("can't have more than 65k sections within a section");
        skip.nested_fragments = u16::try_from((self.fragment_index - 1) - start.fragment_index)
            .expect("can't have more than 65k fragments within a section");
        Ok(())
    }
    fn still_expecting_section_ends(&self) -> bool {
        !self.section_starts.is_empty()
    }
}

// Just data we keep track of in our parser state.
// Helps calculate section skips.
#[derive(Debug)]
struct SectionMeta<'src> {
    name: &'src str,
    section_index: usize,
    fragment_index: usize,
}

// As you may have noticed in Fragment, there's a
// Section and InvertedSection variant but there's
// no SectionEnd variant, so how do we know where
// sections end? Section ends are maintained in a list
// of SectionSkips. The first section in the list
// correponds to the first section that appears in the
// source template, and so is the same for the second,
// third, and so on. A "section skip" basically tells us
// how many nested sections and fragments are in a given
// section, so if the section value is falsy and we need
// to skip over it during render, we can do so quickly
// and efficiently since we know exactly where it ends
// in the fragment list.
#[derive(Debug, PartialEq)]
struct SectionSkip {
    nested_sections: u16,
    nested_fragments: u16,
}

type Input<'src, 'skips> = Stateful<&'src str, State<'src, 'skips>>;

// We can't do "impl Input { fn new()" because Input is a type
// alias and not a newtype.
#[inline]
fn new_input<'src, 'skips>(template: &'src str, skips: &'skips mut Vec<SectionSkip>) -> Input<'src, 'skips> {
    Input {
        input: template,
        state: State {
            fragment_index: 0,
            section_index: 0,
            section_starts: Vec::new(),
            section_skips: skips,
        },
    }
}

// We need this because we don't want to clone
// data during parsing, but if we only borrow we
// then need to store the source string somewhere
// but we can't store it within Template because
// that would make Template a self-referential type,
// e.g. template.fragments would have references that
// would point to data owned in template.source,
// and the Rust compiler doesn't like that. So if we
// receive a source string that is a String we leak it
// to turn it into a &'static str and store that instead.
// When Template is dropped StaticStr's drop impl is also
// called and it reclaims the leaked memory to free it.
// SAFETY: this can never derive Clone, if it ever needs
// to be cloned in the future it must impl Clone by hand.
#[derive(Debug, PartialEq)]
enum StaticStr {
    Real(&'static str),
    Leaked(&'static str),
}

impl StaticStr {
    // SAFETY: this method should never be made
    // public and should only be used internally
    // in this crate
    fn as_str(&self) -> &'static str {
        match self {
            StaticStr::Real(real) => real,
            StaticStr::Leaked(leaked) => leaked,
        }
    }
}

// safe Drop impl for leaked Strings
impl Drop for StaticStr {
    fn drop(&mut self) {
        if let StaticStr::Leaked(leaked) = *self {
            // SAFETY:
            // - StaticStr cannot derive a Clone impl
            // - this should only be used within Template
            //   and remain as a private field
            let _: Box<str> = unsafe {
                Box::from_raw(std::ptr::from_ref::<str>(leaked).cast_mut())
            };
        }
    }
}

impl From<&'static str> for StaticStr {
    fn from(value: &'static str) -> Self {
        StaticStr::Real(value)
    }
}

impl From<String> for StaticStr {
    fn from(value: String) -> Self {
        StaticStr::Leaked(Box::leak(value.into_boxed_str()))
    }
}

// parses a source string into a compiled Template
fn parse<S: Into<StaticStr>>(source: S) -> Result<Template, InternalError> {
    let source: StaticStr = source.into();
    let mut skips = Vec::new();
    let input = new_input(source.as_str(), &mut skips);
    match _parse.parse(input) {
        Ok(fragments) => Ok(Template {
            fragments,
            skips,
            source
        }),
        Err(err) => Err(
            err.into_inner(),
        ),
    }
}

// parses a source string into a compiled Template
#[inline]
fn _parse<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Vec<Fragment<'src>>, InternalError> {
    if input.input.is_empty() {
        return Err(ErrMode::Cut(InternalError::ParseErrorNoContent));
    }

    let frags = repeat(1.., alt((
        parse_literal.map(Some),
        parse_section_end.map(|()| None),
        parse_section_start.map(Some),
        parse_inverted_section_start.map(Some),
        parse_unescaped_variable.map(Some),
        parse_comment.map(|()| None),
        parse_partial.map(Some),
        parse_escaped_variable.map(Some),
    )))
        .fold(Vec::new, |mut acc, item: Option<Fragment>| {
            if let Some(item) = item {
                acc.push(item);
            }
            acc
        })
        .parse_next(input)?;

    // means we had unclosed sections
    if input.state.still_expecting_section_ends() {
        return Err(ErrMode::Cut(InternalError::ParseErrorUnclosedSectionTags));
    }

    Ok(frags)
}

// parses a fragment literal, i.e. anything that doesn't begin with
// {{, until it reaches a {{ or EOF
fn parse_literal<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Fragment<'src>, InternalError> {
    if input.is_empty() {
        return Err(ErrMode::Backtrack(InternalError::ParseErrorGeneric));
    }

    if let Some(range) = input.input.find_slice("{{") {
        if range.start == 0 {
            return Err(ErrMode::Backtrack(InternalError::ParseErrorGeneric));
        }
        let literal = &input.input[..range.start];
        let frag = Fragment::Literal(literal);
        input.input = &input.input[range.start..];
        input.state.visited_fragment();
        Ok(frag)
    } else {
        let frag = Fragment::Literal(input);
        input.input = &input.input[input.input.len()..];
        input.state.visited_fragment();
        Ok(frag)
    }
}

// valid variable names can only contain alphanumeric,
// dash, or underscore chars
fn is_variable_name(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_')
}

// valid variable names must be at least 1 char long, and
// must only contain valid variable chars
fn parse_variable_name<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<&'src str, InternalError> {
    take_while(1.., is_variable_name)
        .parse_next(input)
}

// a variable "path" can potentially be several variable names
// delimited by dots, e.g. some.variable.path
fn parse_variable_path<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<&'src str, InternalError> {
    delimited(
        multispace0,
        alt((
            separated(
                1..,
                parse_variable_name, 
                '.'
            ).map(|()| ()).take(),
            literal("."),
        )),
        multispace0,
    )
        .parse_next(input)
}

// parses an escaped variable, e.g. {{ some.variable }}
fn parse_escaped_variable<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Fragment<'src>, InternalError> {
    let result = delimited(
        literal("{{"),
        cut_err(parse_variable_path),
        cut_err(literal("}}"))
    )
        .context(InternalError::ParseErrorInvalidEscapedVariableTag)
        .parse_next(input)
        .map(Fragment::EscapedVariable);
    if result.is_ok() {
        input.state.visited_fragment();
    }
    result
}

// parses an unescaped variable, e.g. {{{ some.variable }}}
fn parse_unescaped_variable<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Fragment<'src>, InternalError> {
    let result = delimited(
        literal("{{{"),
        cut_err(parse_variable_path),
        cut_err(literal("}}}"))
    )
        .context(InternalError::ParseErrorInvalidUnescapedVariableTag)
        .parse_next(input)
        .map(Fragment::UnescapedVariable);
    if result.is_ok() {
        input.state.visited_fragment();
    }
    result
}

// parses a comment, e.g. {{! comment }}
fn parse_comment(
    input: &mut Input<'_, '_>
) -> PResult<(), InternalError> {
    if input.input.starts_with("{{!") {
        if let Some(range) = input.input.find_slice("}}") {
            input.input = &input.input[range.end..];
            return Ok(());
        }
        return Err(ErrMode::Cut(InternalError::ParseErrorInvalidCommentTag));
    }
    Err(ErrMode::Backtrack(InternalError::ParseErrorGeneric))
}

// parses a section start, e.g. {{# section.start }}
fn parse_section_start<'src>(
    input: &mut Input<'src, '_>
) -> PResult<Fragment<'src>, InternalError> {
    let variable = delimited(
        literal("{{#"),
        cut_err(parse_variable_path),
        cut_err(literal("}}")),
    )
        .context(InternalError::ParseErrorInvalidSectionStartTag)
        .parse_next(input)?;

    input.state.visited_section_start(variable);

    Ok(Fragment::Section(variable))
}

// parses an inverted section start, e.g. {{^ inverted.section.start }}
fn parse_inverted_section_start<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Fragment<'src>, InternalError> {
    let variable = delimited(
        literal("{{^"),
        cut_err(parse_variable_path),
        cut_err(literal("}}")),
    )
        .context(InternalError::ParseErrorInvalidInvertedSectionStartTag)
        .parse_next(input)?;

    input.state.visited_section_start(variable);

    Ok(Fragment::InvertedSection(variable))
}

// parses a section end, e.g. {{/ section.end }}
fn parse_section_end(
    input: &mut Input<'_, '_>,
) -> PResult<(), InternalError> {
    let variable = delimited(
        literal("{{/"),
        cut_err(parse_variable_path),
        cut_err(literal("}}")),
    )
        .context(InternalError::ParseErrorInvalidSectionEndTag)
        .parse_next(input)?;

    if input.state.visited_section_end(variable).is_err() {
        return Err(ErrMode::Cut(InternalError::ParseErrorMismatchedSectionEndTag));
    }

    Ok(())
}

// there's way more valid file name chars than
// variable chars, so we calculate a bitfield
// at compile time of all of the chars that
// can appear in a file name
const fn valid_file_chars() -> u128 {
    let mut bitfield = 0u128;

    let mut b = b'0';
    while b <= b'9' {
        bitfield |= 1u128 << b;
        b += 1;
    }

    b = b'a';
    while b <= b'z' {
        bitfield |= 1u128 << b;
        b += 1;
    }

    b = b'A';
    while b <= b'Z' {
        bitfield |= 1u128 << b;
        b += 1;
    }

    let bytes = b"_-.,!@#$%^&()+=[]~";
    let mut i = 0;
    while i < bytes.len() {
        b = bytes[i];
        bitfield |= 1u128 << b;
        i += 1;
    }

    bitfield
}

// store bitfield in const
const VALID_FILE_CHARS: u128 = valid_file_chars();

// checks if char is a valid file name char
#[inline]
fn is_file_name(c: char) -> bool {
    c.is_ascii() && (VALID_FILE_CHARS & (1u128 << c as u32)) != 0
}

// parses a file name, must be at least 1 char long, and
// can contain any valid file name char, with these notable
// exceptions: "{" (used for mustache tags), "}" (used for
// mustache tags), " " (whitespace, used as a delimiter)
fn parse_file_name<'src>(
    input: &mut Input<'src, '_>
) -> PResult<&'src str, InternalError> {
    take_while(1.., is_file_name)
        .parse_next(input)
}

// parses a file path, i.e. a list of file names delimited
// by slashes, e.g. some/file/path
fn parse_file_path<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<&'src str, InternalError> {
    delimited(
        multispace0,
        separated(
            1..,
            parse_file_name, 
            '/'
        ).map(|()| ()).take(),
        multispace0,
    )
        .parse_next(input)
}

// parses a partial, e.g. {{> some/file/path }}
fn parse_partial<'src>(
    input: &mut Input<'src, '_>,
) -> PResult<Fragment<'src>, InternalError> {
    let result = delimited(
        literal("{{>"),
        cut_err(parse_file_path),
        cut_err(literal("}}")),
    )
        .context(InternalError::ParseErrorInvalidPartialTag)
        .parse_next(input)
        .map(Fragment::Partial);
    if result.is_ok() {
        input.state.visited_fragment();
    }
    result
}

/// # `Template`
/// 
/// A compiled moostache template.
/// 
/// # Examples
/// 
/// ```rust
/// use moostache::Template;
/// use serde_json::json;
/// 
/// let template = Template::parse("hello {{name}}!").unwrap();
/// let data = json!({"name": "John"});
/// let rendered = template.render_no_partials_to_string(&data).unwrap();
/// assert_eq!(rendered, "hello John!");
/// ```
#[derive(Debug, PartialEq)]
pub struct Template {
    // parsed template fragments
    fragments: Vec<Fragment<'static>>,
    // parsed section skips, i.e. tell us where sections end
    skips: Vec<SectionSkip>,
    // fragments have references that point into this
    // string, which would ordinarily make this a self-referential
    // struct which Rust doesn't allow, but we workaround that
    // constraint by using this type. See StaticStr for more details.
    source: StaticStr,
}

impl Template {
    /// Parse a &'static str or String into a compiled
    /// moostache template.
    /// 
    /// # Errors
    /// 
    /// Returns a `MoostacheError` parse error enum variant
    /// if parsing fails for whatever reason.
    #[inline]
    #[allow(private_bounds)]
    pub fn parse<S: Into<StaticStr>>(source: S) -> Result<Template, MoostacheError> {
        match parse(source) {
            Err(err) => {
                Err(MoostacheError::from_internal(err, String::new()))
            },
            Ok(template) => Ok(template),
        }
    }

    /// Render this template.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    pub fn render<K: Borrow<str> + Eq + Hash, T: TemplateLoader<K> + ?Sized, W: Write>(
        &self,
        loader: &T,
        value: &serde_json::Value,
        writer: &mut W,
    ) -> Result<(), T::Error> {
        let mut scopes = Vec::new();
        scopes.push(value);
        _render(
            &self.fragments,
            &self.skips,
            loader,
            &mut scopes,
            writer
        )
    }

    /// Render this template given a type that impls `Serialize`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    pub fn render_serializable<K: Borrow<str> + Eq + Hash, T: TemplateLoader<K> + ?Sized, W: Write, S: Serialize>(
        &self,
        loader: &T,
        serializeable: &S,
        writer: &mut W,
    ) -> Result<(), T::Error> {
        let value = serde_json::to_value(serializeable)
            .map_err(|_| MoostacheError::SerializationError)?;
        self.render(
            loader,
            &value,
            writer
        )
    }

    /// Render this template, assuming it has no partial tags.
    /// 
    /// # Errors
    /// 
    /// Returns a `MoostacheError` if the template contains a
    /// partial, or serializing data during render fails for
    /// whatever reason.
    #[inline]
    pub fn render_no_partials<W: Write>(
        &self,
        value: &serde_json::Value,
        writer: &mut W,
    ) -> Result<(), MoostacheError> {
        self.render(
            &(),
            value,
            writer
        )
    }

    /// Render this template using a type which impls `Serialize`
    /// and assuming it has no partials.
    ///
    /// # Errors
    /// 
    /// Returns a `MoostacheError` if the template contains a
    /// partial, or serializing data during render fails for
    /// whatever reason.
    #[inline]
    pub fn render_serializable_no_partials<S: Serialize, W: Write>(
        &self,
        serializeable: &S,
        writer: &mut W,
    ) -> Result<(), MoostacheError> {
        self.render_serializable(
            &(),
            serializeable,
            writer
        )
    }

    /// Render this template to a `String`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    pub fn render_to_string<K: Borrow<str> + Eq + Hash, T: TemplateLoader<K> + ?Sized>(
        &self,
        loader: &T,
        value: &serde_json::Value,
    ) -> Result<String, T::Error> {
        let mut writer = Vec::<u8>::new();
        self.render(
            loader,
            value,
            &mut writer
        )?;
        let rendered = unsafe {
            // SAFETY: templates are utf8 and value
            // is utf8 so we know templates + value
            // will also be utf8
            String::from_utf8_unchecked(writer)
        };
        Ok(rendered)
    }

    /// Render this template assuming it has no partial tags
    /// and return the result as a `String`.
    /// 
    /// # Errors
    /// 
    /// Returns a `MoostacheError` if the template contains a
    /// partial, or serializing data during render fails for
    /// whatever reason.
    #[inline]
    pub fn render_no_partials_to_string(
        &self,
        value: &serde_json::Value,
    ) -> Result<String, MoostacheError> {
        self.render_to_string(
            &(),
            value,
        )
    }

    /// Render this template given a type which impls `Serialize`
    /// and return result as a `String`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    pub fn render_serializable_to_string<K: Borrow<str> + Eq + Hash, T: TemplateLoader<K> + ?Sized, S: Serialize>(
        &self,
        loader: &T,
        serializable: &S,
    ) -> Result<String, T::Error> {
        let value = serde_json::to_value(serializable)
            .map_err(|_| MoostacheError::SerializationError)?;
        self.render_to_string(
            loader,
            &value,
        )
    }

    /// Render this template given a type which impls `Serialize`,
    /// assume it has no partials, and return result as a `String`.
    /// 
    /// # Errors
    /// 
    /// Returns a `MoostacheError` if the template contains a
    /// partial, or serializing data during render fails for
    /// whatever reason.
    #[inline]
    pub fn render_serializable_no_partials_to_string<S: Serialize>(
        &self,
        serializable: &S,
    ) -> Result<String, MoostacheError> {
        let value = serde_json::to_value(serializable)
            .map_err(|_| MoostacheError::SerializationError)?;
        self.render_to_string(
            &(),
            &value,
        )
    }
}

// note: can't do impl<S: Into<StaticStr> TryFrom<S> below
// because compiler complains that generic impl overlaps
// with another generic impl in the std lib, so we do separate
// impls for &'static str and String

impl TryFrom<&'static str> for Template {
    type Error = MoostacheError;
    fn try_from(source: &'static str) -> Result<Self, Self::Error> {
        Self::parse(source)
    }
}

impl TryFrom<String> for Template {
    type Error = MoostacheError;
    fn try_from(source: String) -> Result<Self, Self::Error> {
        Self::parse(source)
    }
}

//////////////////////
// TEMPLATE LOADERS //
//////////////////////

/// # `TemplateLoader`
/// 
/// Loads templates and renders them.
pub trait TemplateLoader<K: Borrow<str> + Eq + Hash = String> {
    /// Output type of the `get` method.
    type Output<'a>: Deref<Target = Template> + 'a where Self: 'a;
    /// Error type of the `get` and render methods.
    type Error: From<MoostacheError>;
    
    /// Get a template by name.
    /// 
    /// # Errors
    /// 
    /// Returns an `Error` if getting the template fails for whatever
    /// reason. In `HashMapLoader` this would only ever return
    /// `MoostacheError::TemplateNotFound` since it either has the
    /// template or it doesn't. In `FileLoader` it can return almost
    /// any enum variant of `MoostacheError` since it lazily loads
    /// and compiles templates on-demand.
    fn get<'a>(&'a self, name: &str) -> Result<Self::Output<'a>, Self::Error>;

    /// Insert a template by name.
    fn insert(&mut self, name: K, value: Template) -> Option<Template>;

    /// Remove a template by name.
    fn remove(&mut self, name: &str) -> Option<Template>;
    
    /// Render a template by name, using a `serde_json::Value`
    /// as data and writing output to a `writer`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    fn render<W: Write>(
        &self,
        name: &str,
        value: &serde_json::Value,
        writer: &mut W,
    ) -> Result<(), Self::Error> {
        let template = self.get(name)?;
        template.render(self, value, writer)
    }

    /// Render a template by name, using a type which impls
    /// `Serialize` as data and writing output to a `writer`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    fn render_serializable<W: Write, S: Serialize>(
        &self,
        name: &str,
        serializeable: &S,
        writer: &mut W,
    ) -> Result<(), Self::Error> {
        let value = serde_json::to_value(serializeable)
            .map_err(|_| MoostacheError::SerializationError)?;
        self.render(
            name,
            &value,
            writer
        )
    }

    /// Renders a template by name, using a `serde_json::Value`
    /// as data and returning the output as a `String`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    fn render_to_string(
        &self,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<String, Self::Error> {
        let mut writer = Vec::<u8>::new();
        self.render(
            name,
            value,
            &mut writer
        )?;
        let rendered = unsafe {
            // SAFETY: templates are utf8 and value
            // is utf8 so we know templates + value
            // will also be utf8
            String::from_utf8_unchecked(writer)
        };
        Ok(rendered)
    }

    /// Renders a template by name, using a type which impls
    /// `Serialize` as data and returning the output as a `String`.
    /// 
    /// # Errors
    /// 
    /// If using `HashMapLoader` or `FileLoader` this function
    /// can return any enum variant of `MoostacheError`.
    #[inline]
    fn render_serializable_to_string<S: Serialize>(
        &self,
        name: &str,
        serializable: &S,
    ) -> Result<String, Self::Error> {
        let value = serde_json::to_value(serializable)
            .map_err(|_| MoostacheError::SerializationError)?;
        self.render_to_string(
            name,
            &value,
        )
    }
}

/// # `LoaderConfig`
/// 
/// Useful struct for creating `HashMapLoader`s or
/// `FileLoader`s.
/// 
/// # Examples
/// 
/// Creating a `HashMapLoader`:
/// 
/// ```rust
/// use moostache::{LoaderConfig, HashMapLoader};
/// 
/// let loader = HashMapLoader::try_from(LoaderConfig::default()).unwrap();
/// ```
/// 
/// Creating a `FileLoader`:
/// 
/// ```rust
/// use moostache::{LoaderConfig, FileLoader};
/// 
/// let loader = FileLoader::try_from(LoaderConfig::default()).unwrap();
/// ```
/// 
/// `LoaderConfig` default values:
/// 
/// ```rust
/// use moostache::LoaderConfig;
/// 
/// assert_eq!(
///     LoaderConfig::default(),
///     LoaderConfig {
///         templates_directory: "./templates/",
///         templates_extension: ".html",
///         cache_size: 200,
///     },
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LoaderConfig<'a> {
    /// Directory to load templates from.
    pub templates_directory: &'a str,
    /// File extension of template files.
    pub templates_extension: &'a str,
    /// Max number of compiled templates to cache in memory.
    pub cache_size: usize,
}

impl Default for LoaderConfig<'_> {
    fn default() -> Self {
        Self {
            templates_directory: "./templates/",
            templates_extension: ".html",
            cache_size: 200,
        }
    }
}

impl TryFrom<LoaderConfig<'_>> for HashMapLoader {
    type Error = MoostacheError;
    fn try_from(config: LoaderConfig<'_>) -> Result<Self, MoostacheError> {
        let mut dir: String = config.templates_directory.into();
        if !dir.ends_with(MAIN_SEPARATOR_STR) {
            dir.push_str(MAIN_SEPARATOR_STR);
        }
        let dir_path: &Path = dir.as_ref();
        let mut ext: String = config.templates_extension.into();
        if !ext.starts_with('.') {
            ext.insert(0, '.');
        }
        let max_size = NonZeroUsize::new(config.cache_size)
            .ok_or(MoostacheError::ConfigErrorNonPositiveCacheSize)?;
        let max_size: usize = max_size.into();

        if !dir_path.is_dir() {
            return Err(MoostacheError::ConfigErrorInvalidTemplatesDirectory(dir_path.into()));
        }

        let mut current_size = 0usize;
        let mut templates: HashMap<String, Template, FnvBuildHasher> = HashMap::with_hasher(BuildHasherDefault::default());
        for entry in WalkDir::new(dir_path).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() {
                let entry_path = entry.path();
                let entry_path_str = entry_path
                    .to_str()
                    .ok_or_else(|| MoostacheError::LoaderErrorNonUtf8FilePath(entry_path.into()))?;
                if entry_path_str.ends_with(&ext) {
                    let name = entry_path_str
                        .strip_prefix(&dir)
                        .and_then(|path| path.strip_suffix(&ext))
                        .unwrap()
                        .to_string();
                    let source = fs::read_to_string(entry_path)
                        .map_err(|err| MoostacheError::from_io(err, name.clone()))?;
                    let template = Template::parse(source)
                        .map_err(|err| err.set_name(&name))?;
                    templates.insert(name, template);
                    current_size += 1;
                    if current_size > max_size {
                        return Err(MoostacheError::ConfigErrorTooManyTemplates);
                    }
                }
            }
        }

        Ok(HashMapLoader {
            templates
        })
    }
}

/// # `HashMapLoader`
/// 
/// Stores all templates in memory.
/// 
/// # Examples
/// 
/// Creating a `HashMapLoader` from a hashmap:
/// 
/// ```rust
/// use moostache::HashMapLoader;
/// use maplit::hashmap;
///
/// let loader = HashMapLoader::try_from(hashmap! {
///    "greet" => "hello {{name}}!",
/// }).unwrap();
/// ```
/// 
/// Creating a `HashMapLoader` from a `LoaderConfig`:
/// 
/// ```rust
/// use moostache::{LoaderConfig, HashMapLoader};
/// 
/// let loader = HashMapLoader::try_from(LoaderConfig::default()).unwrap();
/// ```
#[derive(Debug)]
pub struct HashMapLoader<K: Borrow<str> + Eq + Hash = String, H: BuildHasher + Default = FnvBuildHasher> {
    templates: HashMap<K, Template, H>,
}

impl<K: Borrow<str> + Eq + Hash, H: BuildHasher + Default> TemplateLoader<K> for HashMapLoader<K, H> {
    type Output<'a> = &'a Template where K: 'a, H: 'a;
    type Error = MoostacheError;
    fn get(&self, name: &str) -> Result<&Template, MoostacheError> {
        self.templates.get(name)
            .ok_or_else(|| MoostacheError::LoaderErrorTemplateNotFound(name.into()))
    }
    fn insert(&mut self, name: K, value: Template) -> Option<Template> {
        self.templates.insert(name, value)
    }
    fn remove(&mut self, name: &str) -> Option<Template> {
        self.templates.remove(name)
    }
}

/// # `FileLoader`
/// 
/// Lazily loads templates on-demand during render. Caches
/// some compiled templates in memory.
/// 
/// # Examples
/// 
/// Creating a `FileLoader` from a `LoaderConfig`:
/// 
/// ```rust
/// use moostache::{LoaderConfig, FileLoader};
/// 
/// let loader = FileLoader::try_from(LoaderConfig::default()).unwrap();
/// ```
#[derive(Debug)]
pub struct FileLoader<H: BuildHasher + Default = FnvBuildHasher> {
    templates_directory: String,
    templates_extension: String,
    path_buf: RefCell<String>,
    templates: RefCell<LruCache<String, Rc<Template>, H>>,
}

impl TemplateLoader for FileLoader {
    type Output<'a> = Rc<Template>;
    type Error = MoostacheError;
    fn get(&self, name: &str) -> Result<Rc<Template>, MoostacheError> {
        let mut templates = self.templates.borrow_mut();
        let template = templates.get(name);
        if let Some(template) = template {
            return Ok(Rc::clone(template));
        }
        let mut path_buf = self.path_buf.borrow_mut();
        path_buf.clear();
        path_buf.push_str(&self.templates_directory);
        path_buf.push_str(name);
        path_buf.push_str(&self.templates_extension);
        let source = fs::read_to_string::<&Path>(path_buf.as_ref())
            .map_err(|err| MoostacheError::from_io(err, name.into()))?;
        let template = Template::parse(source)
            .map_err(|err| err.set_name(name))?;
        let template = Rc::new(template);
        templates.put(name.into(), Rc::clone(&template));
        Ok(template)
    }
    fn insert(&mut self, name: String, value: Template) -> Option<Template> {
        let option = self.templates
            .borrow_mut()
            .put(name, Rc::new(value));
        match option {
            Some(template) => {
                Rc::into_inner(template)
            },
            None => None,
        }
    }
    fn remove(&mut self, name: &str) -> Option<Template> {
        let option = self.templates
            .borrow_mut()
            .pop(name);
        match option {
            Some(template) => {
                Rc::into_inner(template)
            },
            None => None,
        }
    }
}

impl TryFrom<LoaderConfig<'_>> for FileLoader {
    type Error = MoostacheError;
    fn try_from(config: LoaderConfig<'_>) -> Result<Self, MoostacheError> {
        let mut dir: String = config.templates_directory.into();
        if !dir.ends_with(MAIN_SEPARATOR_STR) {
            dir.push_str(MAIN_SEPARATOR_STR);
        }
        let dir_path: &Path = dir.as_ref();
        let mut ext: String = config.templates_extension.into();
        if !ext.starts_with('.') {
            ext.insert(0, '.');
        }
        let max_size = NonZeroUsize::new(config.cache_size)
            .ok_or(MoostacheError::ConfigErrorNonPositiveCacheSize)?;

        if !dir_path.is_dir() {
            return Err(MoostacheError::ConfigErrorInvalidTemplatesDirectory(dir_path.into()));
        }

        let templates = RefCell::new(LruCache::with_hasher(max_size, BuildHasherDefault::default()));

        Ok(FileLoader {
            templates_directory: dir,
            templates_extension: ext,
            path_buf: RefCell::new(String::new()),
            templates,
        })
    }
}

impl<K: Borrow<str> + Eq + Hash, V: Into<StaticStr>> TryFrom<HashMap<K, V>> for HashMapLoader<K> {
    type Error = MoostacheError;
    fn try_from(map: HashMap<K, V>) -> Result<Self, Self::Error> {
        let templates = map
            .into_iter()
            .map(|(key, value)| {
                match parse(value) {
                    Ok(template) => Ok((key, template)),
                    Err(err) => Err(MoostacheError::from_internal(err, key.borrow().to_owned())),
                }
            })
            .collect::<Result<_, _>>();
        templates.map(|templates| HashMapLoader {
            templates,
        })
    }
}

impl TemplateLoader<&'static str> for () {
    type Output<'a> = &'a Template;
    type Error = MoostacheError;
    fn get(&self, name: &str) -> Result<&Template, MoostacheError> {
        Err(MoostacheError::LoaderErrorTemplateNotFound(name.into()))
    }
    fn insert(&mut self, _: &'static str, _: Template) -> Option<Template> {
        None
    }
    fn remove(&mut self, _: &str) -> Option<Template> {
        None
    }
}

///////////////
// RENDERING //
///////////////

// checks if serde_json::Value is truthy
fn is_truthy(value: &serde_json::Value) -> bool {
    use serde_json::Value;
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(_) => value != &json!(0),
        Value::String(string) => !string.is_empty(),
        Value::Array(array) => !array.is_empty(),
        Value::Object(object) => !object.is_empty(),
    }
}

// wraps a Write type and escapes HTML chars
// before writing to the inner Write
struct EscapeHtml<'a, W: Write>(&'a mut W);

// as recommended by OWASP the chars "&", "<",
// ">", "\"", and "'" are escaped
impl<W: Write> Write for EscapeHtml<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = buf.len();
        self.write_all(buf)
            .map(|()| written)
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut start = 0;
        let mut end = 0;
        for byte in buf {
            match byte {
                b'&' => {
                    if start < end {
                        self.0.write_all(&buf[start..end])?;
                    }
                    end += 1;
                    start = end;
                    self.0.write_all(b"&amp;")?;
                },
                b'<' => {
                    if start < end {
                        self.0.write_all(&buf[start..end])?;
                    }
                    end += 1;
                    start = end;
                    self.0.write_all(b"&lt;")?;
                },
                b'>' => {
                    if start < end {
                        self.0.write_all(&buf[start..end])?;
                    }
                    end += 1;
                    start = end;
                    self.0.write_all(b"&gt;")?;
                },
                b'"' => {
                    if start < end {
                        self.0.write_all(&buf[start..end])?;
                    }
                    end += 1;
                    start = end;
                    self.0.write_all(b"&quot;")?;
                },
                b'\'' => {
                    if start < end {
                        self.0.write_all(&buf[start..end])?;
                    }
                    end += 1;
                    start = end;
                    self.0.write_all(b"&#x27;")?;
                },
                _ => {
                    end += 1;
                },
            }
        }
        if start < end {
            self.0.write_all(&buf[start..end])?;
        }
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

// serializes a serde_json::Value
fn write_value<W: Write>(
    value: &serde_json::Value,
    writer: &mut W,
) -> Result<(), MoostacheError> {
    use serde_json::Value;
    match value {
        Value::Null => {
            // serde_json serializes null as
            // "null" but we want this to be
            // an empty string instead
        },
        Value::String(string) => {
            // serde_json serializes strings
            // wrapped with quotes, but we
            // want to write them without quotes
            writer.write_all(string.as_bytes())
                .map_err(|err| MoostacheError::from_io(err, String::new()))?;
        },
        // let serde_json handle the rest
        _ => {
            let mut serializer = serde_json::Serializer::new(writer);
            value.serialize(&mut serializer)
                .map_err(|_| MoostacheError::SerializationError)?;
        },
    }
    Ok(())
}

// given a variable path, e.g. variable.path, and a list of scopes,
// e.g. serde_json::Values, it resolves the path to the specific
// serde_json::Value it points to, or returns serde_json::Value::Null
// if it cannot be found
fn resolve_value<'a>(path: &str, scopes: &[&'a serde_json::Value]) -> &'a serde_json::Value {
    use serde_json::Value;
    if path == "." {
        return scopes[scopes.len() - 1];
    }
    let mut resolved_value = &Value::Null;
    'parent: for value in scopes.iter().rev() {
        resolved_value = *value;
        for (idx, key) in path.split('.').enumerate() {
            match resolved_value {
                Value::Array(array) => {
                    // if we're in this branch assume
                    // the key is an integer index
                    let parsed_index = key.parse::<usize>();
                    if let Ok(index) = parsed_index {
                        let get_option = array.get(index);
                        match get_option {
                            Some(get) => {
                                resolved_value = get;
                            },
                            None => {
                                return &Value::Null;
                            },
                        }
                    } else {
                        // key doesn't exist in this scope
                        if idx == 0 {
                            // go to parent scope
                            continue 'parent;
                        }
                        return &Value::Null;
                    }
                },
                Value::Object(object) => {
                    let get_option = object.get(key);
                    if let Some(get) = get_option {
                        resolved_value = get;
                    } else {
                        // key doesn't exist in this scope
                        if idx == 0 {
                            // go to parent scope
                            continue 'parent;
                        }
                        return &Value::Null;
                    }
                },
                // we got a null, string, or number
                // none of which are keyed, return null
                _ => {
                    // key doesn't exist in this scope
                    if idx == 0 {
                        // go to parent scope
                        continue 'parent;
                    }
                    return &Value::Null;
                }
            }
        }
        return resolved_value;
    }
    resolved_value
}

// this function iterates over a list of fragments and writes
// each one out to the writer, will call itself recursively
// to render sections and partials
fn _render<K: Borrow<str> + Eq + Hash, T: TemplateLoader<K> + ?Sized, W: Write>(
    frags: &[Fragment<'_>],
    skips: &[SectionSkip],
    loader: &T,
    scopes: &mut Vec<&serde_json::Value>,
    writer: &mut W,
) -> Result<(), T::Error> {
    use serde_json::Value;
    let mut frag_idx = 0;
    let mut section_idx = 0;
    while frag_idx < frags.len() {
        let frag = &frags[frag_idx];
        match frag {
            // write literal to writer
            Fragment::Literal(literal) => {
                writer.write_all(literal.as_bytes())
                    .map_err(|err| MoostacheError::from_io(err, String::new()))?;
                frag_idx += 1;
            },
            // write variable value to writer, escape any html chars
            Fragment::EscapedVariable(name) => {
                let resolved_value = resolve_value(name, scopes);
                write_value(resolved_value, &mut EscapeHtml(writer))?;
                frag_idx += 1;
            },
            // write variable value to writer
            Fragment::UnescapedVariable(name) => {
                let resolved_value = resolve_value(name, scopes);
                write_value(resolved_value, writer)?;
                frag_idx += 1;
            },
            // check if section value is truthy, if not skip it,
            // otherwise create an "implicit iterator" over
            // the resolved value and render the section content
            // that many times
            Fragment::Section(name) => {
                let resolved_value = resolve_value(name, scopes);
                let start_frag = frag_idx + 1;
                let end_frag = start_frag + skips[section_idx].nested_fragments as usize;
                let start_section = section_idx + 1;
                let end_section = start_section + skips[section_idx].nested_sections as usize;
                if is_truthy(resolved_value) {
                    if let Value::Array(array) = resolved_value {
                        for value in array {
                            scopes.push(value);
                            _render(
                                &frags[start_frag..end_frag],
                                &skips[start_section..end_section],
                                loader,
                                scopes,
                                writer,
                            )?;
                            scopes.pop();
                        }
                    } else {
                        scopes.push(resolved_value);
                        _render(
                            &frags[start_frag..end_frag],
                            &skips[start_section..end_section],
                            loader,
                            scopes,
                            writer,
                        )?;
                        scopes.pop();
                    }
                }
                frag_idx += 1 + skips[section_idx].nested_fragments as usize;
                section_idx += 1 + skips[section_idx].nested_sections as usize;
            },
            // check if invertedsection value is falsey, if not
            // skip it, otherwise render inner content
            Fragment::InvertedSection(name) => {
                let resolved_value = resolve_value(name, scopes);
                let start_frag = frag_idx + 1;
                let end_frag = start_frag + skips[section_idx].nested_fragments as usize;
                let start_section = section_idx + 1;
                let end_section = start_section + skips[section_idx].nested_sections as usize;
                if !is_truthy(resolved_value) {
                    scopes.push(resolved_value);
                    _render(
                        &frags[start_frag..end_frag],
                        &skips[start_section..end_section],
                        loader,
                        scopes,
                        writer,
                    )?;
                    scopes.pop();
                }
                frag_idx += 1 + skips[section_idx].nested_fragments as usize;
                section_idx += 1 + skips[section_idx].nested_sections as usize;
            },
            // render partial by loading its content via a TemplateLoader
            Fragment::Partial(path) => {
                let template = loader.get(path)?;
                _render(
                    &template.fragments,
                    &template.skips,
                    loader,
                    scopes,
                    writer,
                )?;
                frag_idx += 1;
            },
        }
    }
    Ok(())
}

////////////
// ERRORS //
////////////

/// # `MoostacheError`
/// 
/// Enum of all possible errors that
/// `moostache` can produce.
/// 
/// The `String` in almost every enum variant
/// is the name of the template which produced
/// the error.
#[derive(Debug, Clone, PartialEq)]
pub enum MoostacheError {
    /// Reading from the filesystem, or writing
    /// to a writer, failed for whatever reason.
    IoError(String, std::io::ErrorKind),
    /// Parsing failed for some generic unidentifiable
    /// reason.
    ParseErrorGeneric(String),
    /// Parsing failed because the template was empty.
    ParseErrorNoContent(String),
    /// Not all sections were properly closed
    /// in the template.
    ParseErrorUnclosedSectionTags(String),
    /// Some escaped variable tag, e.g. {{ variable }},
    /// was invalid.
    ParseErrorInvalidEscapedVariableTag(String),
    /// Some unescaped variable tag, e.g. {{{ variable }}},
    /// was invalid.
    ParseErrorInvalidUnescapedVariableTag(String),
    /// Some section end tag, e.g. {{/ section }},
    /// was invalid.
    ParseErrorInvalidSectionEndTag(String),
    /// Some section end tag doesn't match its section
    /// start tag, e.g. {{# section1 }} ... {{/ section2 }}
    ParseErrorMismatchedSectionEndTag(String),
    /// Some comment tag, e.g. {{! comment }}, is invalid.
    ParseErrorInvalidCommentTag(String),
    /// Some section start tag, e.g. {{ section }}, is invalid.
    ParseErrorInvalidSectionStartTag(String),
    /// Some inverted section start tag, e.g. {{^ section }}, is invalid.
    ParseErrorInvalidInvertedSectionStartTag(String),
    /// Some partial tag, e.g. {{> partial }}, is invalid.
    ParseErrorInvalidPartialTag(String),
    /// Loader tried to load a template but couldn't find it by
    /// its name.
    LoaderErrorTemplateNotFound(String),
    /// `FileLoader` tried to load a template but its filepath wasn't
    /// valid utf-8.
    LoaderErrorNonUtf8FilePath(PathBuf),
    /// Cache size parameter in `LoaderConfig` must be greater than zero.
    ConfigErrorNonPositiveCacheSize,
    /// Templates directory passed to `LoaderConfig` is not a directory.
    ConfigErrorInvalidTemplatesDirectory(PathBuf),
    /// Tried creating a `HashMapLoader` from a `LoaderConfig` but there were
    /// more templates in the directory than the maximum allowed by cache
    /// size so not all templates could be loaded into memory. To fix this
    /// increase your cache size or switch to `FileLoader`.
    ConfigErrorTooManyTemplates,
    /// moostache uses `serde_json` internally, and if `serde_json` fails
    /// to serialize anything for any reason this error will be returned.
    SerializationError,
}

impl MoostacheError {
    fn from_internal(internal: InternalError, s: String) -> Self {
        match internal {
            InternalError::ParseErrorGeneric => MoostacheError::ParseErrorGeneric(s),
            InternalError::ParseErrorNoContent => MoostacheError::ParseErrorNoContent(s),
            InternalError::ParseErrorUnclosedSectionTags => MoostacheError::ParseErrorUnclosedSectionTags(s),
            InternalError::ParseErrorInvalidEscapedVariableTag => MoostacheError::ParseErrorInvalidEscapedVariableTag(s),
            InternalError::ParseErrorInvalidUnescapedVariableTag => MoostacheError::ParseErrorInvalidUnescapedVariableTag(s),
            InternalError::ParseErrorInvalidSectionEndTag => MoostacheError::ParseErrorInvalidSectionEndTag(s),
            InternalError::ParseErrorMismatchedSectionEndTag => MoostacheError::ParseErrorMismatchedSectionEndTag(s),
            InternalError::ParseErrorInvalidCommentTag => MoostacheError::ParseErrorInvalidCommentTag(s),
            InternalError::ParseErrorInvalidSectionStartTag => MoostacheError::ParseErrorInvalidSectionStartTag(s),
            InternalError::ParseErrorInvalidInvertedSectionStartTag => MoostacheError::ParseErrorInvalidInvertedSectionStartTag(s),
            InternalError::ParseErrorInvalidPartialTag => MoostacheError::ParseErrorInvalidPartialTag(s),
        }
    }
    fn set_name(mut self, name: &str) -> Self {
        use MoostacheError::*;
        match &mut self {
            ParseErrorGeneric(s) |
            ParseErrorNoContent(s) |
            ParseErrorUnclosedSectionTags(s) |
            ParseErrorInvalidEscapedVariableTag(s) |
            ParseErrorInvalidUnescapedVariableTag(s) |
            ParseErrorInvalidSectionEndTag(s) |
            ParseErrorMismatchedSectionEndTag(s) |
            ParseErrorInvalidCommentTag(s) |
            ParseErrorInvalidSectionStartTag(s) |
            ParseErrorInvalidInvertedSectionStartTag(s) |
            ParseErrorInvalidPartialTag(s) |
            IoError(s, _) |
            LoaderErrorTemplateNotFound(s) => {
                s.clear();
                s.push_str(name);
            },
            _ => unreachable!("trying to set name for parse error"),
        };
        self
    }
    fn from_io(io: std::io::Error, s: String) -> Self {
        let kind = io.kind();
        MoostacheError::IoError(s, kind)
    }
}

impl std::error::Error for MoostacheError {}

impl Display for MoostacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use MoostacheError::*;
        fn template_name(name: &str) -> String {
            if name.is_empty() {
                return "anonymous".to_owned();
            }
            format!("\"{name}\"")
        }
        match self {
            ParseErrorGeneric(s) => write!(f, "error parsing {} template", template_name(s)),
            ParseErrorNoContent(s) => write!(f, "error parsing {} template: empty template", template_name(s)),
            ParseErrorUnclosedSectionTags(s) => write!(f, "error parsing {} template: unclosed section tags", template_name(s)),
            ParseErrorInvalidEscapedVariableTag(s) => write!(f, "error parsing {} template: invalid escaped variable tag, expected {{{{ variable }}}}", template_name(s)),
            ParseErrorInvalidUnescapedVariableTag(s) => write!(f, "error parsing {} template: invalid unescaped variable tag, expected {{{{{{ variable }}}}}}", template_name(s)),
            ParseErrorInvalidSectionEndTag(s) => write!(f, "error parsing {} template: invalid section eng tag, expected {{{{/ section }}}}", template_name(s)),
            ParseErrorMismatchedSectionEndTag(s) => write!(f, "error parsing {} template: mismatched section eng tag, expected {{{{# section }}}} ... {{{{/ section }}}}", template_name(s)),
            ParseErrorInvalidCommentTag(s) => write!(f, "error parsing {} template: invalid comment tag, expected {{{{! comment }}}}", template_name(s)),
            ParseErrorInvalidSectionStartTag(s) => write!(f, "error parsing {} template: invalid section start tag, expected {{{{# section }}}}", template_name(s)),
            ParseErrorInvalidInvertedSectionStartTag(s) => write!(f, "error parsing {} template: invalid inverted section start tag, expected {{{{^ section }}}}", template_name(s)),
            ParseErrorInvalidPartialTag(s) => write!(f, "error parsing {} template: invalid partial tag, expected {{{{> partial }}}}", template_name(s)),
            IoError(s, error_kind) => write!(f, "error reading {} template: {}", template_name(s), error_kind),
            LoaderErrorTemplateNotFound(s) => write!(f, "loader error: {} template not found", template_name(s)),
            LoaderErrorNonUtf8FilePath(s) => write!(f, "loader error: can't load non-utf8 file path: {}", s.display()),
            ConfigErrorNonPositiveCacheSize => write!(f, "config error: cache size must be positive"),
            ConfigErrorInvalidTemplatesDirectory(s) => write!(f, "config error: invalid templates directory: {}", s.display()),
            ConfigErrorTooManyTemplates => write!(f, "config error: templates in directory exceeds cache size"),
            SerializationError => write!(f, "serialization error: could not serialize data to serde_json::Value"),
        }
    }
}

// The reason why we have this is because our parser is a backtracking
// parser that will try to parse something, and if it fails, will
// backtrack and try another parser. This means that even parsing a
// valid mustache template will generate A LOT of errors, and we don't
// want to heap allocate a String every time the parser has to backtrack
// due to an error, so we have this InternalError type instead for parser
// errors that eventually get converted to MoostacheErrors by the time they
// make it to the user.
#[derive(Debug, Copy, Clone, PartialEq)]
enum InternalError {
    ParseErrorGeneric,
    ParseErrorNoContent,
    ParseErrorUnclosedSectionTags,
    ParseErrorInvalidEscapedVariableTag,
    ParseErrorInvalidUnescapedVariableTag,
    ParseErrorInvalidSectionEndTag,
    ParseErrorMismatchedSectionEndTag,
    ParseErrorInvalidCommentTag,
    ParseErrorInvalidSectionStartTag,
    ParseErrorInvalidInvertedSectionStartTag,
    ParseErrorInvalidPartialTag,
}

impl std::error::Error for InternalError {}

impl Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use InternalError::*;
        match self {
            ParseErrorGeneric => write!(f, "generic parse error"),
            ParseErrorNoContent => write!(f, "parse error: empty moostache template"),
            ParseErrorUnclosedSectionTags => write!(f, "parse error: unclosed section tags"),
            ParseErrorInvalidEscapedVariableTag => write!(f, "parse error: invalid escaped variable tag, expected {{{{ variable }}}}"),
            ParseErrorInvalidUnescapedVariableTag => write!(f, "parse error: invalid unescaped variable tag, expected {{{{{{ variable }}}}}}"),
            ParseErrorInvalidSectionEndTag => write!(f, "parse error: invalid section eng tag, expected {{{{/ section }}}}"),
            ParseErrorMismatchedSectionEndTag => write!(f, "parse error: mismatched section eng tag, expected {{{{# section }}}} ... {{{{/ section }}}}"),
            ParseErrorInvalidCommentTag => write!(f, "parse error: invalid comment tag, expected {{{{! comment }}}}"),
            ParseErrorInvalidSectionStartTag => write!(f, "parse error: invalid section start tag, expected {{{{# section }}}}"),
            ParseErrorInvalidInvertedSectionStartTag => write!(f, "parse error: invalid inverted section start tag, expected {{{{^ section }}}}"),
            ParseErrorInvalidPartialTag => write!(f, "parse error: invalid partial tag, expected {{{{> partial }}}}"),
        }
    }
}

// need to impl this so InternalError plays nice with winnow
impl<I: Stream> WParserError<I> for InternalError {
    #[inline]
    fn from_error_kind(_input: &I, _kind: ErrorKind) -> Self {
        InternalError::ParseErrorGeneric
    }

    #[inline]
    fn append(
        self,
        _input: &I,
        _token_start: &<I as Stream>::Checkpoint,
        _kind: ErrorKind,
    ) -> Self {
        self
    }

    #[inline]
    fn or(self, other: Self) -> Self {
        other
    }
}

// need to impl this so InternalError plays nice with winnow
impl<I: Stream> AddContext<I, Self> for InternalError {
    #[inline]
    fn add_context(
        self,
        _input: &I,
        _token_start: &<I as Stream>::Checkpoint,
        context: Self,
    ) -> Self {
        context
    }
}