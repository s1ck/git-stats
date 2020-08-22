#![allow(deprecated)]

#[macro_use]
extern crate nom;

#[derive(Debug, PartialEq)]
pub struct CoAuthor<'a> {
    pub(crate) name: &'a str,
}

pub fn get_co_author(line: &str) -> Option<CoAuthor> {
    let (_, co_author) = co_author(line.as_bytes()).ok()?;
    let co_author = std::str::from_utf8(co_author).ok()?;
    Some(CoAuthor { name: co_author })
}

// sad Ferris :*(
fn byte_string_trim(input: &[u8]) -> &[u8] {
    &input[0..input.len() - (input.iter().rev().take_while(|c| **c == b' ').count())]
}

named!(
        co_authored_by<Vec<&[u8]>>,
        many1!(ws!(tag_no_case!("co-authored-by:")))
    );
named!(
        co_author<&[u8]>,
        preceded!(
            co_authored_by,
            map!(take_till!(|c| c == b'<'), byte_string_trim)
        )
    );

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

