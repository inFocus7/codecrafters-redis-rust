use super::types::{CRLF, ParseError, RESPValue, resp_prefix};
use std::fmt;
use std::fmt::Write;

fn encode_value(resp_value: Option<&RESPValue>, st: &mut String) -> Result<(), ParseError> {
    if resp_value.is_none() {
        return Ok(());
    }

    let rv_result = resp_value.ok_or(ParseError::Incomplete); // is incomplete good here?
    let rv = rv_result.map_err(|_| ParseError::Invalid)?; // is invalid good here?
    match rv {
        RESPValue::SimpleString(ss) => {
            write!(st, "{}{}{}", resp_prefix::SIMPLE_STRING, ss, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::SimpleError(se) => {
            write!(st, "{}{}{}", resp_prefix::SIMPLE_ERROR, se, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Integer(i) => {
            write!(st, "{}{}{}", resp_prefix::INTEGER, i, CRLF).map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Null => {
            write!(st, "{}{}", resp_prefix::NULL, CRLF).map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Boolean(b) => {
            let mut bool_str: &str = "t";
            if *b == false {
                bool_str = "f";
            }

            write!(st, "{}{}{}", resp_prefix::BOOLEAN, bool_str, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Double(d) => {
            // Note: Does not preserve scientific notation
            let mut content = format!("{}", d);
            if d.is_nan() {
                // ensure nan is lowercase, instead of "NaN"
                content = "nan".to_string();
            }

            write!(st, "{}{}{}", resp_prefix::DOUBLE, content, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::BigNum(bn) => {
            write!(st, "{}{}{}", resp_prefix::BIG_NUM, bn, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::BulkString(bs) => {
            write!(
                st,
                "{}{}{}{}{}",
                resp_prefix::BULK_STRING,
                bs.len(),
                CRLF,
                bs,
                CRLF
            )
            .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::NullBulkString => {
            write!(st, "{}-1{}", resp_prefix::NULL_BULK_STRING, CRLF)
                .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Array(a) => {
            // initial content without elements, should append if any
            write!(st, "{}{}{}", resp_prefix::ARRAY, a.len(), CRLF)
                .map_err(|_| ParseError::Invalid)?;
            if !a.is_empty() {
                for raw_element in a {
                    encode_value(Some(raw_element), st)?;
                }
            }
        }
        RESPValue::NullArray => {
            write!(st, "{}-1{}", resp_prefix::NULL_ARRAY, CRLF).map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::BulkError(be) => {
            write!(
                st,
                "{}{}{}{}{}",
                resp_prefix::BULK_ERROR,
                be.len(),
                CRLF,
                be,
                CRLF
            )
            .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::VerbatimString {
            encoding: e,
            data: d,
        } => {
            let s = format!("{}:{}", e, d);
            write!(
                st,
                "{}{}{}{}{}",
                resp_prefix::VERBATIM_STRING,
                s.len(),
                CRLF,
                s,
                CRLF
            )
            .map_err(|_| ParseError::Invalid)?;
        }
        RESPValue::Map(m) => {
            write!(st, "{}{}{}", resp_prefix::MAP, m.0.len(), CRLF)
                .map_err(|_| ParseError::Invalid)?;

            for (raw_key, raw_value) in m.0.iter() {
                encode_value(Some(raw_key), st)?;
                encode_value(Some(raw_value), st)?;
            }
        }
        RESPValue::Attribute {
            metadata: m,
            value: v,
        } => {
            write!(st, "{}{}{}", resp_prefix::ATTRIBUTE, m.0.len(), CRLF)
                .map_err(|_| ParseError::Invalid)?;

            for (raw_key, raw_value) in m.0.iter() {
                encode_value(Some(raw_key), st)?;
                encode_value(Some(raw_value), st)?;
            }

            encode_value(Some(v), st)?;
        }
        RESPValue::Set(s) => {
            write!(st, "{}{}{}", resp_prefix::SET, s.0.len(), CRLF)
                .map_err(|_| ParseError::Invalid)?;

            for raw_elem in s.0.iter() {
                encode_value(Some(raw_elem), st)?;
            }
        }
        RESPValue::Push(p) => {
            write!(st, "{}{}{}", resp_prefix::PUSH, p.len(), CRLF)
                .map_err(|_| ParseError::Invalid)?;

            for raw_elem in p.iter() {
                encode_value(Some(raw_elem), st)?;
            }
        }
    }

    Ok(())
}

impl fmt::Display for RESPValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res: String = String::new();
        let enc = encode_value(Some(self), &mut res);
        match enc {
            Ok(()) => {
                write!(f, "{}", res)
            }
            Err(_) => return Err(fmt::Error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resp::types::{RESPMap, RESPSet};
    use num_bigint::BigInt;
    use std::collections::{HashMap, HashSet};
    use std::f64::{INFINITY, NAN, NEG_INFINITY};
    use std::str::FromStr;

    #[test]
    fn test_simple_string() {
        let cases = vec![
            (
                RESPValue::SimpleString("OK".to_string()),
                "+OK\r\n",
                "succeeds in parsing a valid SimpleString",
            ),
            (
                RESPValue::SimpleString("".to_string()),
                "+\r\n",
                "succeeds parsing a valid SimpleString (empty)",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_simple_error() {
        let cases = vec![(
            RESPValue::SimpleError("Error message".to_string()),
            "-Error message\r\n",
            "succeeds parsing a valid simple error",
        )];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_integer() {
        let cases = vec![
            (
                RESPValue::Integer(1000),
                ":1000\r\n",
                "succeeds parsing a valid positive integer",
            ),
            (
                RESPValue::Integer(-1000),
                ":-1000\r\n",
                "succeeds parsing a valid negative integer",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_null() {
        let cases = vec![(RESPValue::Null, "_\r\n", "succeeds parsing null")];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_boolean() {
        let cases = vec![
            (
                RESPValue::Boolean(true),
                "#t\r\n",
                "succeeds in processing boolean (t)",
            ),
            (
                RESPValue::Boolean(false),
                "#f\r\n",
                "succeeds in processing boolean (f)",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_double() {
        let cases = vec![
            (
                RESPValue::Double(1.23),
                ",1.23\r\n",
                "parses valid positive double",
            ),
            (
                RESPValue::Double(-1.23),
                ",-1.23\r\n",
                "parses valid negative double",
            ),
            (
                RESPValue::Double(123 as f64),
                ",123\r\n",
                "parses valid non-fractional double",
            ),
            (
                RESPValue::Double(123e2 as f64),
                ",12300\r\n", // Note: the scientific notation is not kept
                "parses valid non-fractional, unsigned exponential, double",
            ),
            (
                RESPValue::Double(123e-2 as f64),
                ",1.23\r\n",
                "parses valid non-fractional, negative exponential, double",
            ),
            (
                RESPValue::Double(123E+2 as f64),
                ",12300\r\n",
                "parses valid non-fractional, positive exponential, double",
            ),
            (RESPValue::Double(INFINITY), ",inf\r\n", "parses valid inf"),
            (
                RESPValue::Double(NEG_INFINITY),
                ",-inf\r\n",
                "parses valid negative inf",
            ),
            (RESPValue::Double(NAN), ",nan\r\n", "parses valid nan"),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_big_number() {
        let cases = vec![
            (
                RESPValue::BigNum(
                    BigInt::from_str("3492890328409238509324850943850943825024385").unwrap(),
                ),
                "(3492890328409238509324850943850943825024385\r\n",
                "succeeds in parsing positive big number",
            ),
            (
                RESPValue::BigNum(
                    BigInt::from_str("-3492890328409238509324850943850943825024385").unwrap(),
                ),
                "(-3492890328409238509324850943850943825024385\r\n",
                "succeeds in parsing negative big number",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_bulk_string() {
        let cases = vec![
            (
                RESPValue::BulkString("hello\r\n".to_string()),
                "$7\r\nhello\r\n\r\n",
                "succeeds in parsing a valid full-length string",
            ),
            (
                RESPValue::BulkString("hello".to_string()),
                "$5\r\nhello\r\n",
                "succeeds in parsing a valid full-length string with CRLF in content",
            ),
            (
                RESPValue::BulkString("".to_string()),
                "$0\r\n\r\n",
                "succeeds in parsing a valid empty string",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_null_bulk_string() {
        let cases = vec![(
            RESPValue::NullBulkString,
            "$-1\r\n",
            "succeeds in parsing a valid null bulk string",
        )];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_array() {
        let cases = vec![
            (
                RESPValue::Array(vec![]),
                "*0\r\n",
                "succeeds in parsing empty array",
            ),
            (
                RESPValue::Array(vec![
                    RESPValue::BulkString("hello".to_string()),
                    RESPValue::BulkString("world".to_string()),
                ]),
                "*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n",
                "succeeds in parsing array with content",
            ),
            (
                RESPValue::Array(vec![
                    RESPValue::Integer(1),
                    RESPValue::Integer(2),
                    RESPValue::Integer(3),
                    RESPValue::Integer(4),
                    RESPValue::BulkString("hello".to_string()),
                ]),
                "*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n",
                "succeeds in parsing mixed data-type array",
            ),
            (
                RESPValue::Array(vec![
                    RESPValue::Array(vec![
                        RESPValue::Integer(1),
                        RESPValue::Integer(2),
                        RESPValue::Integer(3),
                    ]),
                    RESPValue::Array(vec![
                        RESPValue::SimpleString("Hello".to_string()),
                        RESPValue::SimpleError("World".to_string()),
                    ]),
                ]),
                "*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n",
                "succeeds in parsing multi-dimensional array",
            ),
            (
                RESPValue::Array(vec![
                    RESPValue::BulkString("hello".to_string()),
                    RESPValue::NullBulkString,
                    RESPValue::NullArray,
                    RESPValue::BulkString("world".to_string()),
                ]),
                "*4\r\n$5\r\nhello\r\n$-1\r\n*-1\r\n$5\r\nworld\r\n",
                "suceeds processing array with null elements",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_null_array() {
        let cases = vec![(
            RESPValue::NullArray,
            "*-1\r\n",
            "succeeds processing valid null array",
        )];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg)
        }
    }

    #[test]
    fn test_bulk_error() {
        let cases = vec![
            (
                RESPValue::BulkError("SYNTAX invalid syntax".to_string()),
                "!21\r\nSYNTAX invalid syntax\r\n",
                "succeeds in parsing valid bulk error",
            ),
            (
                RESPValue::BulkError("".to_string()),
                "!0\r\n\r\n",
                "succeeds in parsing valid bulk error (empty)",
            ),
            (
                RESPValue::BulkError("SYNTAX invalid syntax\r\n".to_string()),
                "!23\r\nSYNTAX invalid syntax\r\n\r\n",
                "succeeds in parsing valid bulk error with CRLF within content",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_verbatim_string() {
        let cases = vec![
            (
                RESPValue::VerbatimString {
                    encoding: "txt".to_string(),
                    data: "Some string".to_string(),
                },
                "=15\r\ntxt:Some string\r\n",
                "succeeds processing valid verbatim string",
            ),
            (
                RESPValue::VerbatimString {
                    encoding: "txt".to_string(),
                    data: "Some string\r\n".to_string(),
                },
                "=17\r\ntxt:Some string\r\n\r\n",
                "succeeds processing valid verbatim string with CRLF within content",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_map() {
        let cases = vec![
            (
                RESPValue::Map(RESPMap(HashMap::from([
                    (
                        RESPValue::SimpleString("first".to_string()),
                        RESPValue::Integer(1),
                    ),
                    (
                        RESPValue::SimpleString("second".to_string()),
                        RESPValue::Integer(2),
                    ),
                ]))),
                "%2\r\n+first\r\n:1\r\n+second\r\n:2\r\n",
                "succeeds parsing valid map",
            ),
            (
                RESPValue::Map(RESPMap(HashMap::from([]))),
                "%0\r\n",
                "succeeds parsing valid empty map",
            ),
        ];

        // round-trip assertion because testing raw string outputs from maps is non-deterministic
        for (input, _, msg) in cases {
            let rt = input.to_string().parse::<RESPValue>().unwrap();
            assert_eq!(rt, input, "{}", msg);
        }
    }

    #[test]
    fn test_attribute() {
        let cases = vec![
            (
                RESPValue::Attribute {
                    metadata: RESPMap(HashMap::from([(
                        RESPValue::SimpleString("key-popularity".to_string()),
                        RESPValue::Map(RESPMap(HashMap::from([
                            (
                                RESPValue::BulkString("a".to_string()),
                                RESPValue::Double(0.1923),
                            ),
                            (
                                RESPValue::BulkString("b".to_string()),
                                RESPValue::Double(0.0012),
                            ),
                        ]))),
                    )])),
                    value: Box::new(RESPValue::SimpleString("hello".to_string())),
                },
                "|1\r\n+key-popularity\r\n%2\r\n$1\r\na\r\n,0.1923\r\n$1\r\nb\r\n,0.0012\r\n+hello\r\n",
                "succeeds parsing valid attribute",
            ),
            // (
            //     RESPValue::Attribute {
            //         metadata: RESPMap(HashMap::from([])),
            //         value: Box::new(RESPValue::SimpleString("hello".to_string())),
            //     },
            //     "|0\r\n+hello\r\n",
            //     "succeeds parsing valid empty attribute",
            // ),
        ];

        // round-trip assertion because testing raw string outputs from maps is non-deterministic
        for (input, _, msg) in cases {
            let rt = input.to_string().parse::<RESPValue>().unwrap();
            assert_eq!(rt, input, "{}", msg);
        }
    }

    #[test]
    fn test_set() {
        let cases = vec![
            (
                RESPValue::Set(RESPSet(HashSet::<RESPValue>::from([
                    RESPValue::SimpleString("hello".to_string()),
                    RESPValue::SimpleString("world".to_string()),
                ]))),
                "~2\r\n+hello\r\n+world\r\n",
                "succeeds parsing valid set",
            ),
            (
                RESPValue::Set(RESPSet(HashSet::<RESPValue>::from([]))),
                "~0\r\n",
                "succeeds parsing valid set (empty)",
            ),
        ];

        // round-trip because set order changes
        for (input, _, msg) in cases {
            let rt = input.to_string().parse::<RESPValue>().unwrap();
            assert_eq!(rt, input, "{}", msg);
        }
    }

    #[test]
    fn test_push() {
        let cases = vec![
            (
                RESPValue::Push(vec![
                    RESPValue::SimpleString("hello".to_string()),
                    RESPValue::SimpleString("world".to_string()),
                ]),
                ">2\r\n+hello\r\n+world\r\n",
                "succeeds parsing valid push",
            ),
            (
                RESPValue::Push(vec![]),
                ">0\r\n",
                "succeeds parsing valid push (empty)",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.to_string(), expected, "{}", msg);
        }
    }
}
