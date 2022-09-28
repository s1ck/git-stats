use nom::{
    bytes::complete::{is_not, tag, tag_no_case, take_until},
    character::complete::space0,
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, preceded},
    IResult,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CoAuthor<'a> {
    pub name: &'a str,
    pub mail: Option<&'a str>,
}

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
    let co_author_name = map(take_until("<"), str::trim);
    preceded(co_authored_by, co_author_name)(input)
}

fn co_authored_by(input: &str) -> IResult<&str, Vec<()>> {
    let co_authored_by = tag_no_case("co-authored-by:");
    let co_authored_by = map(co_authored_by, |_| ());
    let co_authored_by = delimited(space0, co_authored_by, space0);
    many1(co_authored_by)(input)
}

fn co_author_mail(input: &str) -> IResult<&str, Option<&str>> {
    opt(delimited(tag("<"), is_not("> \t"), tag(">")))(input)
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
    #[test_case("Some other content" => None; "none")]
    fn test_get_co_author_name(input: &str) -> Option<&str> {
        get_co_author(input).map(|co_author| co_author.name)
    }

    #[test_case("co-authored-by: Alice <alice@wonderland.org>" => Some("alice@wonderland.org"); "alice")]
    #[test_case("co-authored-by: Alice Keys <alice@wonderland.org>" => Some("alice@wonderland.org"); "alice keys")]
    #[test_case("Some other content" => None; "none")]
    fn test_get_co_author_mail(input: &str) -> Option<&str> {
        get_co_author(input).and_then(|co_author| co_author.mail)
    }
}
