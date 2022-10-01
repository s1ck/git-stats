use nom::{
    bytes::complete::{is_a, tag_no_case, take_until},
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, preceded},
    IResult,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CoAuthor<'a> {
    pub name: &'a str,
}

pub fn get_co_author(line: &str) -> Option<CoAuthor> {
    let (_, name) = co_author(line.as_bytes()).ok()?;
    let name = std::str::from_utf8(name).ok()?;
    Some(CoAuthor { name })
}

fn co_author(input: &[u8]) -> IResult<&[u8], &[u8]> {
    const LEADING_ANGLE: &[u8] = b"<";
    let co_author_name = map(take_until(LEADING_ANGLE), trim_ascii_end);
    preceded(co_authored_by, co_author_name)(input)
}

// unstable feature 'byte_slice_trim_ascii'
// see issue #94035 <https://github.com/rust-lang/rust/issues/94035>
// taken from https://github.com/rust-lang/rust/blob/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/slice/ascii.rs#L125-L138
fn trim_ascii_end(input: &[u8]) -> &[u8] {
    let mut bytes = input;
    while let [rest @ .., last] = bytes {
        if last.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

fn co_authored_by(input: &[u8]) -> IResult<&[u8], Vec<()>> {
    let co_authored_by = delimited(
        opt(is_a(" \t")),
        map(tag_no_case("co-authored-by:"), |_| ()),
        opt(is_a(" \t")),
    );
    many1(co_authored_by)(input)
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
        let (result, _) = co_authored_by(input.as_bytes()).unwrap();
        assert_eq!(result, expected.as_bytes())
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>", "Alice"; "alice")]
    #[test_case("co-authored-by: Alice Bob <alice@wonderland.org>", "Alice Bob"; "alice bob")]
    fn test_co_author(input: &str, expected: &str) {
        let (_, result) = co_author(input.as_bytes()).unwrap();
        assert_eq!(result, expected.as_bytes())
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>" => Some("Alice"); "alice")]
    #[test_case("co-authored-by: Alice Keys <alice@wonderland.org>" => Some("Alice Keys"); "alice keys")]
    #[test_case("Some other content" => None; "none")]
    fn test_get_co_author(input: &str) -> Option<&str> {
        get_co_author(input).map(|co_author| co_author.name)
    }
}
