use std::collections::HashMap;

use winnow::Parser;
use winnow::Result;
use winnow::ascii::digit1;
use winnow::ascii::multispace0;
use winnow::combinator::separated;
use winnow::combinator::separated_pair;
use winnow::combinator::trace;
use winnow::combinator::{alt, delimited, opt};
use winnow::error::{ContextError, ErrMode, ParserError};
use winnow::stream::{AsChar, Stream, StreamIsPartial};
use winnow::token::take_until;

#[derive(Debug, Clone, PartialEq)]
enum Num {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Num),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

fn main() -> anyhow::Result<()> {
    let s = r#"{
        "name": "John Doe",
        "age": 30,
        "is_student": false,
        "marks": [90.0, -80.0, 85.1],
        "address": {
            "city": "New York",
            "zip": 10001
        }
    }"#;

    let input = &mut (&*s);
    let v = parse_json(input)?;
    println!("{:#?}", v);
    Ok(())
}

fn parse_json(input: &str) -> anyhow::Result<JsonValue> {
    let input = &mut (&*input);
    parse_value(input).map_err(|e: ContextError| anyhow::anyhow!("Failed to parse JSON: {}", e))
}

fn parse_null(input: &mut &str) -> Result<()> {
    "null".value(()).parse_next(input)
}

fn parse_bool(input: &mut &str) -> Result<bool> {
    alt(("true", "false")).parse_to().parse_next(input)
}

fn parse_num(input: &mut &str) -> Result<Num> {
    let sign = opt("-").map(|s| s.is_some()).parse_next(input)?;
    let num = digit1.parse_to::<i64>().parse_next(input)?;
    let ret: Result<(), ErrMode<ContextError>> = ".".value(()).parse_next(input);
    if ret.is_ok() {
        let frac = digit1.parse_to::<i64>().parse_next(input)?;
        let v = format!("{}.{}", num, frac).parse::<f64>().unwrap();
        let v = if sign { -v } else { v };

        Ok(Num::Float(v as _))
    } else {
        let v = if sign { -num } else { num };
        Ok(Num::Int(v))
    }
}

fn parse_string(input: &mut &str) -> Result<String> {
    let ret = delimited('"', take_until(0.., '"'), '"').parse_next(input)?;
    Ok(ret.to_string())
}

pub fn sep_with_space<Input, Output, Error, ParseNext>(
    mut parser: ParseNext,
) -> impl Parser<Input, (), Error>
where
    Input: Stream + StreamIsPartial,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
    ParseNext: Parser<Input, Output, Error>,
{
    trace("sep_with_space", move |input: &mut Input| {
        let _ = multispace0.parse_next(input)?;
        parser.parse_next(input)?;
        multispace0.parse_next(input)?;
        Ok(())
    })
}

fn parse_array(input: &mut &str) -> Result<Vec<JsonValue>> {
    let left = sep_with_space('[');
    let right = sep_with_space(']');
    let separator = sep_with_space(',');
    let parse_values = separated(0.., parse_value, separator);
    delimited(left, parse_values, right).parse_next(input)
}

fn parse_object(input: &mut &str) -> Result<HashMap<String, JsonValue>> {
    let left = sep_with_space('{');
    let right = sep_with_space('}');
    let pair_separator = sep_with_space(',');
    let key_value_separator = sep_with_space(':');

    let parse_kv_pair = separated_pair(parse_string, key_value_separator, parse_value);

    let parse_kv = separated(1.., parse_kv_pair, pair_separator);

    delimited(left, parse_kv, right).parse_next(input)
}

fn parse_value(input: &mut &str) -> Result<JsonValue> {
    alt((
        parse_null.value(JsonValue::Null),
        parse_bool.map(JsonValue::Bool),
        parse_num.map(JsonValue::Number),
        parse_string.map(JsonValue::String),
        parse_array.map(JsonValue::Array),
        parse_object.map(JsonValue::Object),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() -> Result<(), ContextError> {
        let s = "null";
        let input = &mut (&*s);
        parse_null(input)?;

        Ok(())
    }

    #[test]
    fn test_parse_bool() -> Result<(), ContextError> {
        let input = "true";
        let result = parse_bool(&mut (&*input))?;
        assert!(result);

        let input = "false";
        let result = parse_bool(&mut (&*input))?;
        assert!(!result);

        Ok(())
    }

    #[test]
    fn test_parse_num() -> Result<(), ContextError> {
        let input = "123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(123));

        let input = "-123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(-123));

        let input = "123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(123.456));

        let input = "-123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(-123.456));

        Ok(())
    }

    #[test]
    fn test_parse_string() -> Result<(), ContextError> {
        let input = r#""hello""#;
        let result = parse_string(&mut (&*input))?;
        assert_eq!(result, "hello");

        Ok(())
    }

    #[test]
    fn test_parse_array() -> Result<(), ContextError> {
        let input = r#"[1, 2, 3]"#;
        let result = parse_array(&mut (&*input))?;

        assert_eq!(
            result,
            vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3))
            ]
        );

        let input = r#"["a", "b", "c"]"#;
        let result = parse_array(&mut (&*input))?;
        assert_eq!(
            result,
            vec![
                JsonValue::String("a".to_string()),
                JsonValue::String("b".to_string()),
                JsonValue::String("c".to_string())
            ]
        );
        Ok(())
    }

    #[test]
    fn test_parse_object() -> Result<(), ContextError> {
        let input = r#"{"a": 1, "b": 2}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert("b".to_string(), JsonValue::Number(Num::Int(2)));
        assert_eq!(result, expected);

        let input = r#"{"a": 1, "b": [1, 2, 3]}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert(
            "b".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3)),
            ]),
        );
        assert_eq!(result, expected);

        Ok(())
    }
}
