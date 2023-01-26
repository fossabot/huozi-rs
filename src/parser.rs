use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{char, multispace0, one_of},
    combinator::{cut, eof, map, not, verify},
    error::{context, VerboseError},
    multi::{many0, many_till},
    sequence::{preceded, separated_pair, terminated, tuple},
    IResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    Text(&'a str),
    Block(Block<'a>),
    EOF,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    inner: Vec<Element<'a>>,
    tag: &'a str,
    value: Option<&'a str>,
}

type ParseResult<'a, T, E = VerboseError<&'a str>> = IResult<&'a str, T, E>;

fn escaped_str(input: &str) -> ParseResult<&str> {
    let chars = r#""\[]/="#;

    escaped(is_not(chars), '\\', one_of(r#""\n[]/="#))(input)
}

fn string_quoted(input: &str) -> ParseResult<&str> {
    context(
        "string_quoted",
        preceded(char('\"'), cut(terminated(escaped_str, char('\"')))),
    )(input)
}

fn string_without_space(input: &str) -> ParseResult<&str> {
    let chars = "\"\\[]/= \t\n\r";
    context("string_without_space", preceded(multispace0, is_not(chars)))(input)
}

fn plain_text(input: &str) -> ParseResult<Element> {
    context("plain_text", map(escaped_str, |s: &str| Element::Text(s)))(input)
}

fn tag_head_keypair(input: &str) -> ParseResult<(&str, Option<&str>)> {
    context(
        "tag_head_keypair",
        alt((
            map(
                separated_pair(
                    preceded(multispace0, tag_key),
                    preceded(multispace0, char('=')),
                    preceded(multispace0, tag_value),
                ),
                |(k, v)| (k, Some(v)),
            ),
            map(preceded(multispace0, tag_key), |s| (s, None)),
        )),
    )(input)
}

fn tag_key(input: &str) -> ParseResult<&str> {
    context("tag_key", string_without_space)(input)
}

fn tag_value(input: &str) -> ParseResult<&str> {
    context("tag_value", alt((string_without_space, string_quoted)))(input)
}

fn tag_head(input: &str) -> ParseResult<(&str, Option<&str>)> {
    context(
        "tag_head",
        preceded(
            tuple((char('['), not(char('/')))),
            cut(terminated(
                tag_head_keypair,
                preceded(multispace0, char(']')),
            )),
        ),
    )(input)
}

fn tag_end(input: &str) -> ParseResult<&str> {
    context(
        "tag_end",
        preceded(
            tag("[/"),
            cut(terminated(tag_value, preceded(multispace0, char(']')))),
        ),
    )(input)
}

fn closed_tag(input: &str) -> ParseResult<(&str, Option<&str>, Vec<Element>)> {
    context(
        "closed_tag",
        map(
            verify(
                tuple((tag_head, elements, tag_end)),
                |&((head_key, _), _, end_key)| head_key == end_key,
            ),
            |((key, value), inner, _)| (key, value, inner),
        ),
    )(input)
}

fn block(input: &str) -> ParseResult<Element> {
    context(
        "block",
        map(closed_tag, |(key, value, inner)| {
            Element::Block(Block {
                inner,
                tag: key,
                value: value.and_then(|s| Some(s)),
            })
        }),
    )(input)
}

fn element(input: &str) -> ParseResult<Element> {
    context(
        "element",
        alt((map(eof, |_| Element::EOF), plain_text, block)),
    )(input)
}

fn elements(input: &str) -> ParseResult<Vec<Element>> {
    context("elements", many0(element))(input)
}

pub fn parse(input: &str) -> ParseResult<Vec<Element>> {
    let (input, (r, _)) = context("elements", many_till(element, eof))(input)?;
    Ok((input, r))
}

#[test]
fn parse_text() {
    use nom::error::convert_error;
    // let input = "泽材[fillColor=0xff6600]灭[bold]逐[/bold][/fillColor]莫笔[strokeEnable=true]亡[/strokeEnable]鲜，[strokeEnable=true][strokeColor=black][fillColor=red][fontSize=64]如何[/fontSize][fillColor=orange][italic]气[/italic][fillColor=yellow][bold]死[/bold][fillColor=green]你的[fillColor=0xff6600]设[fillColor=blue]计师[fillColor=magenta][fontSize=28]朋[/fontSize]友[/fillColor][/fillColor][/fillColor][/fillColor][/fillColor][/fillColor][/fillColor][/strokeColor][/strokeEnable]";

    let input = r#"ssf[xx="123"]aaa[/xx]"#;

    match parse(input) {
        Ok(r) => println!("{:#?}", r.1),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            println!("{}", e);
            println!("{}", convert_error(input, e));
        }
        Err(nom::Err::Incomplete(n)) => {
            println!("{:?}", n);
        }
    }
}

#[test]
fn test_plain_text() {
    assert_eq!(
        parse(" some text ").unwrap().1,
        vec![Element::Text(" some text ")]
    );
}

#[test]
fn test_plain_text_escaped() {
    assert_eq!(
        parse(r" some \n \[text ").unwrap().1,
        vec![Element::Text(r" some \n \[text ")]
    );
}

#[test]
fn test_single_block_without_value() {
    assert_eq!(
        parse(r"[foo]text[/foo]").unwrap().1,
        vec![Element::Block(Block {
            inner: vec![Element::Text("text")],
            tag: "foo",
            value: None
        })]
    );
}

#[test]
fn test_single_block_with_value() {
    assert_eq!(
        parse(r"[foo=bar]text[/foo]").unwrap().1,
        vec![Element::Block(Block {
            inner: vec![Element::Text("text")],
            tag: "foo",
            value: Some("bar")
        })]
    );
}

#[test]
fn test_single_block_with_value_quoted() {
    assert_eq!(
        parse(r#"[foo="bar "]text[/foo]"#).unwrap().1,
        vec![Element::Block(Block {
            inner: vec![Element::Text("text")],
            tag: "foo",
            value: Some("bar ")
        })]
    );
}

#[test]
fn test_single_block_multiline() {
    assert_eq!(
        parse(
            r"[foo=bar]
text
  
[/foo]"
        )
        .unwrap()
        .1,
        vec![Element::Block(Block {
            inner: vec![Element::Text("\ntext\n  \n")],
            tag: "foo",
            value: Some("bar")
        })]
    );
}

#[test]
fn test_mixed_text_and_block() {
    assert_eq!(
        parse(r" some text [foo=bar]text[/foo]").unwrap().1,
        vec![
            Element::Text(" some text "),
            Element::Block(Block {
                inner: vec![Element::Text("text")],
                tag: "foo",
                value: Some("bar")
            })
        ]
    );
}

#[test]
fn test_nested_blocks() {
    assert_eq!(
        parse(r"[foo=bar][xx=123][/xx][/foo]").unwrap().1,
        vec![Element::Block(Block {
            inner: vec![Element::Block(Block {
                inner: vec![],
                tag: "xx",
                value: Some("123")
            })],
            tag: "foo",
            value: Some("bar")
        })]
    );
}

#[test]
fn test_complex_elements() {
    assert_eq!(
        parse(r"a\n[foo=bar]q[xx=123][/xx]x[/foo][yy][/yy]")
            .unwrap()
            .1,
        vec![
            Element::Text("a\\n"),
            Element::Block(Block {
                inner: vec![
                    Element::Text("q"),
                    Element::Block(Block {
                        inner: vec![],
                        tag: "xx",
                        value: Some("123")
                    }),
                    Element::Text("x")
                ],
                tag: "foo",
                value: Some("bar")
            }),
            Element::Block(Block {
                inner: vec![],
                tag: "yy",
                value: None
            })
        ]
    );
}

#[test]
fn test_tagpair_with_spaces() {
    assert_eq!(
        parse(r#"[ foo = "bar " ]text[/ foo  ]"#).unwrap().1,
        vec![Element::Block(Block {
            inner: vec![Element::Text("text")],
            tag: "foo",
            value: Some("bar ")
        })]
    );
}
