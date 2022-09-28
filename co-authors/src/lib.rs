use std::convert::{identity, TryFrom};

use nom::{
    bytes::complete::{is_not, tag, tag_no_case, take_until},
    character::complete::space0,
    combinator::{map, opt, verify},
    error::ErrorKind,
    multi::many1,
    sequence::{delimited, preceded},
    Err, IResult,
};

/// A co-author as parsed from a [Co-Authored-By trailer].
///
/// # Example
///
/// ```rust
/// # use co_authors::CoAuthor;
/// # use std::convert::TryFrom;
/// let trailer = "Co-Authored-By: Alice <alice@wonderland.org>";
/// let co_author = CoAuthor::try_from(trailer);
/// assert_eq!(co_author, Ok(CoAuthor { name: "Alice", mail: Some("alice@wonderland.org") }));
/// ```
///
/// [Co-Authored-By trailer]: https://github.blog/2018-01-29-commit-together-with-co-authors/
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CoAuthor<'a> {
    pub name: &'a str,
    pub mail: Option<&'a str>,
}

impl<'a> TryFrom<&'a str> for CoAuthor<'a> {
    type Error = CoAuthorError;

    fn try_from(line: &'a str) -> Result<Self, Self::Error> {
        match co_author(line) {
            Ok((_, (name, mail))) => Ok(CoAuthor { name, mail }),
            Err(e) => match dbg!(e) {
                Err::Incomplete(_) => Err(CoAuthorError::MissingTrailerKey),
                Err::Error(e) | Err::Failure(e) => match e.code {
                    ErrorKind::Tag => Err(CoAuthorError::MissingTrailerKey),
                    ErrorKind::TakeUntil => Err(CoAuthorError::MissingMail),
                    ErrorKind::Verify => Err(CoAuthorError::MissingName),
                    otherwise => unreachable!("Unexpected error kind: {:?}", otherwise),
                },
            },
        }
    }
}

/// Possible errors when parsing the [CoAuthor].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoAuthorError {
    /// The trailer is missing the `Co-Authored-By:` key.
    MissingTrailerKey,
    /// The name of the co-author is missing.
    MissingName,
    /// The mail of the co-author is missing.
    MissingMail,
}

impl std::fmt::Display for CoAuthorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoAuthorError::MissingTrailerKey => {
                f.write_str("The trailer is missing the `Co-Authored-By:` key.")
            }
            CoAuthorError::MissingName => f.write_str("The name of the co-author is missing."),
            CoAuthorError::MissingMail => f.write_str("The mail of the co-author is missing."),
        }
    }
}

impl std::error::Error for CoAuthorError {}

#[deprecated(since = "0.1.0", note = "Use `CoAuthor::try_from` instead")]
pub fn get_co_author(line: &str) -> Option<CoAuthor> {
    let (_, (name, mail)) = co_author(line).ok()?;
    Some(CoAuthor { name, mail })
}

fn co_author(input: &str) -> IResult<&str, (&str, Option<&str>)> {
    let (input, name) = co_author_name(input)?;
    let (input, email) = co_author_mail(input)?;
    Ok((input, (name, email)))
}

fn co_author_name(input: &str) -> IResult<&str, &str> {
    let co_author_name = take_until("<");
    let co_author_name = map(co_author_name, str::trim);
    let co_author_name = verify(co_author_name, |s: &str| !s.is_empty());
    let co_author_name = preceded(co_authored_by, co_author_name);
    identity(co_author_name)(input)
}

fn co_authored_by(input: &str) -> IResult<&str, Vec<()>> {
    let co_authored_by = tag_no_case("co-authored-by:");
    let co_authored_by = map(co_authored_by, |_| ());
    let co_authored_by = delimited(space0, co_authored_by, space0);
    let co_authored_by = many1(co_authored_by);
    identity(co_authored_by)(input)
}

fn co_author_mail(input: &str) -> IResult<&str, Option<&str>> {
    let co_author_mail = is_not("> \t");
    let co_author_mail = delimited(tag("<"), co_author_mail, tag(">"));
    let co_author_mail = opt(co_author_mail);
    identity(co_author_mail)(input)
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case("co-authored-by: Alice <alice@wonderland.org>", "Alice <alice@wonderland.org>"; "lower case")]
    #[test_case("Co-Authored-By: Alice <alice@wonderland.org>", "Alice <alice@wonderland.org>"; "camel case")]
    #[test_case("CO-AUTHORED-BY: Alice <alice@wonderland.org>", "Alice <alice@wonderland.org>"; "upper case")]
    #[test_case("Co-authored-by: Alice <alice@wonderland.org>", "Alice <alice@wonderland.org>"; "mixed case")]
    #[test_case("Co-authored-by: Co-authored-by: Alice <alice@wonderland.org>", "Alice <alice@wonderland.org>"; "florentin case")]
    fn test_co_authored_by(input: &str, expected: &str) {
        let (result, _) = co_authored_by(input).unwrap();
        assert_eq!(result, expected)
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>", "Alice"; "alice")]
    #[test_case("co-authored-by: Alice Bob <alice@wonderland.org>", "Alice Bob"; "alice bob")]
    fn test_co_author_name(input: &str, expected: &str) {
        let (_, result) = co_author_name(input).unwrap();
        assert_eq!(result, expected)
    }

    #[test_case("<alice@wonderland.org>", "alice@wonderland.org"; "alice")]
    #[test_case("<alice@wonderland.org> bob", "alice@wonderland.org"; "alice bob")]
    #[test_case("<alice@wonderland.org> <charlie@wonderland.org>", "alice@wonderland.org"; "alice charlie")]
    fn test_co_author_mail(input: &str, expected: &str) {
        let (_, result) = co_author_mail(input).unwrap();
        assert_eq!(result.unwrap(), expected)
    }

    #[test_case(""; "empty")]
    #[test_case(" <alice@wonderland.org>"; "leading space")]
    #[test_case("<alice@wonderland.org"; "missing close")]
    #[test_case("<alice@wonderland.org&gt;"; "encoded close")]
    #[test_case("alice@wonderland.org>"; "missing open")]
    #[test_case("<alice and bob@wonderland.org>"; "contains whitespace")]
    fn test_missing_co_author_mail(input: &str) {
        let (_, result) = co_author_mail(input).unwrap();
        assert_eq!(result, None)
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>" => Some("Alice"); "alice")]
    #[test_case("co-authored-by: Alice Keys <alice@wonderland.org>" => Some("Alice Keys"); "alice keys")]
    #[test_case("Co-Authored-By:Alice<alice@wonderland.org>" => Some("Alice"); "no space alice")]
    #[test_case("Some other content" => None; "none")]
    fn test_get_co_author_name(input: &str) -> Option<&str> {
        CoAuthor::try_from(input)
            .ok()
            .map(|co_author| co_author.name)
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>" => Some("alice@wonderland.org"); "alice")]
    #[test_case("co-authored-by: Alice Keys <alice@wonderland.org>" => Some("alice@wonderland.org"); "alice keys")]
    #[test_case("co-authored-by: <alice@wonderland.org>" => None; "missing name")]
    #[test_case("Some other content" => None; "none")]
    fn test_get_co_author_mail(input: &str) -> Option<&str> {
        CoAuthor::try_from(input)
            .ok()
            .and_then(|co_author| co_author.mail)
    }

    #[test_case("Alice <alice@wonderland.org>"; "missing")]
    #[test_case("co-authored-by Alice <alice@wonderland.org>"; "missing colon")]
    #[test_case("Co-Authored: Alice <alice@wonderland.org>"; "missing By")]
    #[test_case("Authored-By: Alice <alice@wonderland.org>"; "missing Co")]
    #[test_case(""; "empty input")]
    fn test_missing_trailer_key(input: &str) {
        let err = CoAuthor::try_from(input).unwrap_err();
        assert_eq!(err, CoAuthorError::MissingTrailerKey)
    }

    #[test_case("Co-Authored-By: <alice@wonderland.org>"; "missing name")]
    fn test_missing_name(input: &str) {
        let err = CoAuthor::try_from(input).unwrap_err();
        assert_eq!(err, CoAuthorError::MissingName)
    }

    #[test_case("Co-Authored-By: Alice"; "missing mail")]
    fn test_missing_mail(input: &str) {
        let err = CoAuthor::try_from(input).unwrap_err();
        assert_eq!(err, CoAuthorError::MissingMail)
    }
}
