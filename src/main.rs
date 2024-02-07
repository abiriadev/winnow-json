use std::collections::HashMap;

use winnow::{
	ascii::float,
	combinator::{
		alt, cut_err, delimited, preceded, repeat, separated, separated_pair,
		terminated,
	},
	error::{AddContext, InputError, ParserError},
	prelude::*,
	token::{any, none_of, take, take_while},
	PResult,
};

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
	Null,
	Boolean(bool),
	Number(f64),
	String(String),
	Array(Vec<JsonValue>),
	Object(HashMap<String, JsonValue>),
}

fn json<'i, E>(input: &mut &'i str) -> PResult<JsonValue, E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	delimited(ws, json_value, ws).parse_next(input)
}

fn json_value<'i, E>(input: &mut &'i str) -> PResult<JsonValue, E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	alt((
		null.value(JsonValue::Null),
		boolean.map(JsonValue::Boolean),
		string.map(JsonValue::String),
		float.map(JsonValue::Number),
		array.map(JsonValue::Array),
		object.map(JsonValue::Object),
	))
	.parse_next(input)
}

fn null<'i, E>(input: &mut &'i str) -> PResult<&'i str, E>
where E: ParserError<&'i str> {
	"null".parse_next(input)
}

fn boolean<'i, E>(input: &mut &'i str) -> PResult<bool, E>
where E: ParserError<&'i str> {
	let parse_true = "true".value(true);
	let parse_false = "false".value(false);

	alt((parse_true, parse_false)).parse_next(input)
}

fn string<'i, E>(input: &mut &'i str) -> PResult<String, E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	preceded(
		'\"',
		cut_err(terminated(
			repeat(0.., character).fold(String::new, |mut string, c| {
				string.push(c);
				string
			}),
			'\"',
		)),
	)
	.context("string")
	.parse_next(input)
}

fn character<'i, E>(input: &mut &'i str) -> PResult<char, E>
where E: ParserError<&'i str> {
	let c = none_of('\"').parse_next(input)?;

	if c == '\\' {
		alt((
			any.verify_map(|c| {
				Some(match c {
					'"' | '\\' | '/' => c,
					'b' => '\x08',
					'f' => '\x0C',
					'n' => '\n',
					'r' => '\r',
					't' => '\t',
					_ => return None,
				})
			}),
			preceded('u', unicode_escape),
		))
		.parse_next(input)
	} else {
		Ok(c)
	}
}

fn unicode_escape<'i, E>(input: &mut &'i str) -> PResult<char, E>
where E: ParserError<&'i str> {
	alt((
		u16_hex
			.verify(|cp| !(0xD800..0xE000).contains(cp))
			.map(|cp| cp as u32),
		separated_pair(u16_hex, "\\u", u16_hex)
			.verify(|(h, l)| {
				(0xD800..0xDC00).contains(h) && (0xDC00..0xE000).contains(l)
			})
			.map(|(h, l)| {
				let high_ten = (h as u32) - 0xD800;
				let low_ten = (l as u32) - 0xDC00;
				(high_ten << 10) + low_ten + 0x10000
			}),
	))
	.verify_map(std::char::from_u32)
	.parse_next(input)
}

fn u16_hex<'i, E>(input: &mut &'i str) -> PResult<u16, E>
where E: ParserError<&'i str> {
	take(4usize)
		.verify_map(|s| u16::from_str_radix(s, 16).ok())
		.parse_next(input)
}

fn array<'i, E>(input: &mut &'i str) -> PResult<Vec<JsonValue>, E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	preceded(
		('[', ws),
		cut_err(terminated(
			separated(0.., json_value, (ws, ',', ws)),
			(ws, ']'),
		)),
	)
	.context("array")
	.parse_next(input)
}

fn object<'i, E>(
	input: &mut &'i str,
) -> PResult<HashMap<String, JsonValue>, E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	preceded(
		('{', ws),
		cut_err(terminated(
			separated(0.., key_value, (ws, ',', ws)),
			(ws, '}'),
		)),
	)
	.context("object")
	.parse_next(input)
}

fn key_value<'i, E>(input: &mut &'i str) -> PResult<(String, JsonValue), E>
where E: ParserError<&'i str> + AddContext<&'i str, &'static str> {
	separated_pair(
		string,
		cut_err((ws, ':', ws)),
		json_value,
	)
	.parse_next(input)
}

fn ws<'i, E>(input: &mut &'i str) -> PResult<&'i str, E>
where E: ParserError<&'i str> {
	take_while(0.., WS).parse_next(input)
}

const WS: &[char] = &[' ', '\t', '\r', '\n'];

fn main() {
	let input = r#"
  {
    "null" : null,
    "true"  :true ,
    "false":  false  ,
    "number" : 123e4 ,
    "string" : " abc 123 " ,
    "array" : [ false , 1 , "two" ] ,
    "object" : { "a" : 1.0 , "b" : "c" } ,
    "empty_array" : [  ] ,
    "empty_object" : {   }
  }
  "#;

	assert_eq!(
		json::<InputError<&'_ str>>.parse_peek(input),
		Ok((
			"",
			JsonValue::Object(
				vec![
					("null".to_string(), JsonValue::Null),
					(
						"true".to_string(),
						JsonValue::Boolean(true)
					),
					(
						"false".to_string(),
						JsonValue::Boolean(false)
					),
					(
						"number".to_string(),
						JsonValue::Number(123e4)
					),
					(
						"string".to_string(),
						JsonValue::String(" abc 123 ".to_string())
					),
					(
						"array".to_string(),
						JsonValue::Array(vec![
							JsonValue::Boolean(false),
							JsonValue::Number(1.0),
							JsonValue::String("two".to_string())
						])
					),
					(
						"object".to_string(),
						JsonValue::Object(
							vec![
								("a".to_string(), JsonValue::Number(1.0)),
								(
									"b".to_string(),
									JsonValue::String("c".to_string())
								),
							]
							.into_iter()
							.collect()
						)
					),
					(
						"empty_array".to_string(),
						JsonValue::Array(vec![]),
					),
					(
						"empty_object".to_string(),
						JsonValue::Object(HashMap::new()),
					),
				]
				.into_iter()
				.collect()
			)
		))
	);
}
