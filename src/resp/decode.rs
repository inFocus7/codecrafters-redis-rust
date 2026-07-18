use super::types::{CRLF, MultiBulk, ParseError, RESPMap, RESPSet, RESPValue, resp_prefix};
use num_bigint::BigInt;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

// Parse a RESP value from a string starting at the given position.
// Used recursively to support nested structures (e.g. arrays, maps.)
// It returns the decoded value or an error.
// There are two types of errors:
// - Invalid: Broken syntax.
// - Incomplete: Incomplete data, e.g. missing CRLF could indicate that there is still more data to be read in the case of buffering.
fn decode_value(input: &str, pos: &mut usize, depth: usize) -> Result<RESPValue, ParseError> {
    if *pos >= input.len() {
        // possible data missing
        return Err(ParseError::Incomplete);
    }

    let result: RESPValue;
    let type_char = input.as_bytes()[*pos] as char;
    match type_char {
        resp_prefix::SIMPLE_STRING => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            result = RESPValue::SimpleString(content.to_string());
            *pos += end + CRLF.len();
        }
        resp_prefix::SIMPLE_ERROR => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            result = RESPValue::SimpleError(content.to_string());
            *pos += end + CRLF.len();
        }
        resp_prefix::INTEGER => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            let number = content.parse::<i64>().map_err(|_| ParseError::Invalid)?;
            result = RESPValue::Integer(number);
            *pos += end + CRLF.len();
        }
        resp_prefix::NULL => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            if !content.is_empty() {
                // null should be empty
                return Err(ParseError::Invalid);
            }
            result = RESPValue::Null;
            *pos += end + CRLF.len();
        }
        resp_prefix::BOOLEAN => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            let boolean = match content {
                "t" => true,
                "f" => false,
                _ => return Err(ParseError::Invalid),
            };
            result = RESPValue::Boolean(boolean);
            *pos += end + CRLF.len();
        }
        resp_prefix::DOUBLE => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            let number = content.parse::<f64>().map_err(|_| ParseError::Invalid)?;
            result = RESPValue::Double(number);
            *pos += end + CRLF.len();
        }
        resp_prefix::BIG_NUM => {
            let end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let content = &input[*pos + 1..*pos + end];
            let number = BigInt::from_str(content).map_err(|_| ParseError::Invalid)?;
            result = RESPValue::BigNum(number);
            *pos += end + CRLF.len();
        }
        resp_prefix::BULK_STRING => {
            let len_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let len_str = &input[*pos + 1..*pos + len_end];
            let len = len_str.parse::<isize>().map_err(|_| ParseError::Invalid)?;
            *pos += len_end + CRLF.len();
            if len < -1 {
                return Err(ParseError::Invalid);
            } else if len == -1 {
                result = RESPValue::NullBulkString;
            } else {
                // read up to len bytes
                let end = *pos + len as usize;
                if end > input.len() {
                    return Err(ParseError::Incomplete);
                }
                if end + CRLF.len() > input.len() {
                    // partial check: if first byte exists but isn't \r, invalid
                    // while it's safe to return ::Incomplete, it would cause the server to wait for more data to buffer unecessarily
                    if end < input.len() && input.as_bytes()[end] != b'\r' {
                        return Err(ParseError::Invalid);
                    }
                    return Err(ParseError::Incomplete);
                }
                if &input[end..end + CRLF.len()] != CRLF {
                    return Err(ParseError::Invalid);
                }
                let content = &input[*pos..end];
                result = RESPValue::BulkString(content.to_string());
                *pos = end + CRLF.len();
            }
        }
        resp_prefix::ARRAY => {
            let num_elems_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let num_elems_str = &input[*pos + 1..*pos + num_elems_end];
            let num_elems = num_elems_str
                .parse::<isize>()
                .map_err(|_| ParseError::Invalid)?;
            *pos += num_elems_end + CRLF.len();
            if num_elems < -1 {
                return Err(ParseError::Invalid);
            } else if num_elems == -1 {
                result = RESPValue::NullArray;
            } else {
                let mut elements = Vec::<RESPValue>::with_capacity(num_elems as usize);
                for _ in 0..num_elems {
                    let element = decode_value(input, pos, depth + 1)?;
                    elements.push(element);
                }
                result = RESPValue::Array(MultiBulk(elements));
            }
        }
        resp_prefix::BULK_ERROR => {
            let len_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let len_str = &input[*pos + 1..*pos + len_end];
            let len = len_str.parse::<usize>().map_err(|_| ParseError::Invalid)?;
            *pos += len_end + CRLF.len();
            let end = *pos + len;
            if end > input.len() {
                return Err(ParseError::Incomplete);
            }
            if end + CRLF.len() > input.len() {
                // partial check: if first byte exists but isn't \r, invalid
                // while it's safe to return ::Incomplete, it would cause the server to wait for more data to buffer unecessarily

                if end < input.len() && input.as_bytes()[end] != b'\r' {
                    return Err(ParseError::Invalid);
                }
                return Err(ParseError::Incomplete);
            }
            if &input[end..end + CRLF.len()] != CRLF {
                return Err(ParseError::Invalid);
            }
            let content = &input[*pos..end];
            result = RESPValue::BulkError(content.to_string());
            *pos = end + CRLF.len();
        }
        resp_prefix::VERBATIM_STRING => {
            let len_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let len_str = &input[*pos + 1..*pos + len_end];
            let len = len_str.parse::<usize>().map_err(|_| ParseError::Invalid)?;
            *pos += len_end + CRLF.len();
            let end = *pos + len;
            if end > input.len() {
                return Err(ParseError::Incomplete);
            }
            if end + CRLF.len() > input.len() {
                // partial check: if first byte exists but isn't \r, invalid
                // while it's safe to return ::Incomplete, it would cause the server to wait for more data to buffer unecessarily
                if end < input.len() && input.as_bytes()[end] != b'\r' {
                    return Err(ParseError::Invalid);
                }
                return Err(ParseError::Incomplete);
            }
            if &input[end..end + CRLF.len()] != CRLF {
                return Err(ParseError::Invalid);
            }
            let content = &input[*pos..end];
            let sep = content.find(':').ok_or(ParseError::Incomplete)?;
            let enc = &content[..sep];
            let data = &content[sep + 1..];
            result = RESPValue::VerbatimString {
                encoding: enc.to_string(),
                data: data.to_string(),
            };
            *pos = end + CRLF.len();
        }
        resp_prefix::MAP => {
            let num_entries_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let num_entries_str = &input[*pos + 1..*pos + num_entries_end];
            let num_entries = num_entries_str
                .parse::<usize>()
                .map_err(|_| ParseError::Invalid)?;
            *pos += num_entries_end + CRLF.len();
            let mut entries = HashMap::<RESPValue, RESPValue>::with_capacity(num_entries);
            for _ in 0..num_entries {
                let key = decode_value(input, pos, depth + 1)?;
                let value = decode_value(input, pos, depth + 1)?;
                entries.insert(key, value);
            }
            result = RESPValue::Map(RESPMap(entries));
        }
        resp_prefix::ATTRIBUTE => {
            let num_entries_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let num_entries_str = &input[*pos + 1..*pos + num_entries_end];
            let num_entries = num_entries_str
                .parse::<usize>()
                .map_err(|_| ParseError::Invalid)?;
            *pos += num_entries_end + CRLF.len();
            let mut entries = HashMap::<RESPValue, RESPValue>::with_capacity(num_entries);
            for _ in 0..num_entries {
                let key = decode_value(input, pos, depth + 1)?;
                let value = decode_value(input, pos, depth + 1)?;
                entries.insert(key, value);
            }

            let content = decode_value(input, pos, depth + 1)?;
            result = RESPValue::Attribute {
                metadata: RESPMap(entries),
                value: Box::new(content),
            }
        }
        resp_prefix::SET => {
            let num_elems_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let num_elems_str = &input[*pos + 1..*pos + num_elems_end];
            let num_elems = num_elems_str
                .parse::<usize>()
                .map_err(|_| ParseError::Invalid)?;
            *pos += num_elems_end + CRLF.len();
            let mut elements = HashSet::<RESPValue>::with_capacity(num_elems);
            for _ in 0..num_elems {
                let element = decode_value(input, pos, depth + 1)?;
                elements.insert(element);
            }
            result = RESPValue::Set(RESPSet(elements));
        }
        resp_prefix::PUSH => {
            if depth > 0 {
                // a push must NOT be nested
                return Err(ParseError::Invalid);
            }

            let num_elems_end = input[*pos..].find(CRLF).ok_or(ParseError::Incomplete)?;
            let num_elems_str = &input[*pos + 1..*pos + num_elems_end];
            let num_elems = num_elems_str
                .parse::<usize>()
                .map_err(|_| ParseError::Invalid)?;
            *pos += num_elems_end + CRLF.len();
            let mut elements = Vec::<RESPValue>::with_capacity(num_elems);
            for _ in 0..num_elems {
                let element = decode_value(input, pos, depth + 1)?;
                elements.push(element);
            }
            result = RESPValue::Push(MultiBulk(elements));
        }
        _ => {
            return Err(ParseError::Invalid);
        }
    }

    Ok(result)
}

impl FromStr for RESPValue {
    type Err = ParseError;

    // convert raw string into appropiate RESPValue
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        return decode_value(s, &mut 0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::{INFINITY, NAN, NEG_INFINITY};

    #[test]
    fn test_simple_string() {
        let cases = vec![
            (
                "+OK\r\n",
                Ok(RESPValue::SimpleString("OK".to_string())),
                "succeeds parsing a valid simple string",
            ),
            (
                "+\r\n",
                Ok(RESPValue::SimpleString("".to_string())),
                "succeeds parsing a valid simple string (empty)",
            ),
            (
                "+OK",
                Err(ParseError::Incomplete),
                "fails parsing a simple string without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_simple_error() {
        let cases = vec![
            (
                "-Error message\r\n",
                Ok(RESPValue::SimpleError("Error message".to_string())),
                "succeeds parsing a valid simple error",
            ),
            (
                "-Error message",
                Err(ParseError::Incomplete),
                "fails parsing a simple error without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_integer() {
        let cases = vec![
            (
                ":1000\r\n",
                Ok(RESPValue::Integer(1000)),
                "succeeds parsing a valid non-signed integer",
            ),
            (
                ":+1000\r\n",
                Ok(RESPValue::Integer(1000)),
                "succeeds parsing a valid positive integer",
            ),
            (
                ":-1000\r\n",
                Ok(RESPValue::Integer(-1000)),
                "succeeds parsing a valid negative integer",
            ),
            (
                ":a1000\r\n",
                Err(ParseError::Invalid),
                "fails parsing an invalid integer (non-digit)",
            ),
            (
                ":1000",
                Err(ParseError::Incomplete),
                "fails parsing an integer without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_bulk_string() {
        let cases = vec![
            (
                "$7\r\nhello\r\n\r\n",
                Ok(RESPValue::BulkString("hello\r\n".to_string())),
                "succeeds in parsing a valid full-length string",
            ),
            (
                "$5\r\nhello\r\n",
                Ok(RESPValue::BulkString("hello".to_string())),
                "succeeds in parsing a valid full-length string with CRLF in content",
            ),
            (
                "$0\r\n\r\n",
                Ok(RESPValue::BulkString("".to_string())),
                "succeeds in parsing a valid empty string",
            ),
            (
                "$5\r\nhellomorecontent\r\n",
                Err(ParseError::Invalid),
                "fails in parsing a string that has trailing content",
            ),
            (
                "$9\r\nhello\r\n",
                Err(ParseError::Incomplete),
                "fails in parsing a string whole length definition is longer than content",
            ),
            (
                "$5\r\nhello",
                Err(ParseError::Incomplete),
                "fails in parsing a string missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_null_bulk_string() {
        let cases = vec![
            (
                "$-1\r\n",
                Ok(RESPValue::NullBulkString),
                "succeeds in parsing a valid null bulk string",
            ),
            (
                "$-2\r\n",
                Err(ParseError::Invalid),
                "fails in parsing a bulk string with length less than null-indicator",
            ),
            (
                "$-1",
                Err(ParseError::Incomplete),
                "fails in parsing a null bulk string missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_array() {
        let cases = vec![
            (
                "*0\r\n",
                Ok(RESPValue::Array(MultiBulk(vec![]))),
                "succeeds in parsing empty array",
            ),
            (
                "*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n",
                Ok(RESPValue::Array(MultiBulk(vec![
                    RESPValue::BulkString("hello".to_string()),
                    RESPValue::BulkString("world".to_string()),
                ]))),
                "succeeds in parsing array with content",
            ),
            (
                "*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n",
                Ok(RESPValue::Array(MultiBulk(vec![
                    RESPValue::Integer(1),
                    RESPValue::Integer(2),
                    RESPValue::Integer(3),
                    RESPValue::Integer(4),
                    RESPValue::BulkString("hello".to_string()),
                ]))),
                "succeeds in parsing mixed data-type array",
            ),
            (
                "*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n",
                Ok(RESPValue::Array(MultiBulk(vec![
                    RESPValue::Array(MultiBulk(vec![
                        RESPValue::Integer(1),
                        RESPValue::Integer(2),
                        RESPValue::Integer(3),
                    ])),
                    RESPValue::Array(MultiBulk(vec![
                        RESPValue::SimpleString("Hello".to_string()),
                        RESPValue::SimpleError("World".to_string()),
                    ])),
                ]))),
                "succeeds in parsing multi-dimensional array",
            ),
            (
                "*4\r\n$5\r\nhello\r\n$-1\r\n*-1\r\n$5\r\nworld\r\n",
                Ok(RESPValue::Array(MultiBulk(vec![
                    RESPValue::BulkString("hello".to_string()),
                    RESPValue::NullBulkString,
                    RESPValue::NullArray,
                    RESPValue::BulkString("world".to_string()),
                ]))),
                "suceeds processing array with null elements",
            ),
            (
                "*0",
                Err(ParseError::Incomplete),
                "fails parsing array without CRLF",
            ),
            (
                "*2\r\n$5\r\nhello\r\n",
                Err(ParseError::Incomplete),
                "fails parsing array with lenght > elements",
            ),
            (
                "*2\r\n$5\r\nhello\r\n$6\r\nworld\r\n", // err with world blk str
                // it should propagate the inner fail upwards: $6\r\nworld\r\n => string:world\r, with a remainder \n,
                // regardless of more input buffering a CRLF should have been next and because \n isn't \r, this is bad data.
                Err(ParseError::Invalid),
                "fails parsing when inner element has error",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_null_array() {
        let cases = vec![
            (
                "*-1\r\n",
                Ok(RESPValue::NullArray),
                "succeeds processing valid null array",
            ),
            (
                "*-1",
                Err(ParseError::Incomplete),
                "fails processing null array without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg)
        }
    }

    #[test]
    fn test_null() {
        let cases = vec![
            ("_\r\n", Ok(RESPValue::Null), "succeeds processing null"),
            (
                "_-\r\n",
                Err(ParseError::Invalid),
                "fails processing null with content",
            ),
            (
                "_",
                Err(ParseError::Incomplete),
                "fails processing null without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg)
        }
    }

    #[test]
    fn test_boolean() {
        let cases = vec![
            (
                "#t\r\n",
                Ok(RESPValue::Boolean(true)),
                "succeeds in processing boolean (t)",
            ),
            (
                "#f\r\n",
                Ok(RESPValue::Boolean(false)),
                "succeeds in processing boolean (f)",
            ),
            (
                "#T\r\n",
                Err(ParseError::Invalid),
                "fails processing boolean using wrong casing",
            ),
            (
                "#true\r\n",
                Err(ParseError::Invalid),
                "fails processing boolean using full-word",
            ),
            (
                "#\r\n",
                Err(ParseError::Invalid),
                "fails processing boolean without content",
            ),
            (
                "#t",
                Err(ParseError::Incomplete),
                "fails processing boolean missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_double() {
        let cases = vec![
            (
                ",1.23\r\n",
                Ok(RESPValue::Double(1.23)),
                "parses valid unsigned double",
            ),
            (
                ",+1.23\r\n",
                Ok(RESPValue::Double(1.23)),
                "parses valid positive double",
            ),
            (
                ",-1.23\r\n",
                Ok(RESPValue::Double(-1.23)),
                "parses valid negative double",
            ),
            (
                ",123\r\n",
                Ok(RESPValue::Double(123 as f64)),
                "parses valid non-fractional double",
            ),
            (
                ",123e2\r\n",
                Ok(RESPValue::Double(123e2 as f64)),
                "parses valid non-fractional, unsigned exponential, double",
            ),
            (
                ",123e-2\r\n",
                Ok(RESPValue::Double(123e-2 as f64)),
                "parses valid non-fractional, negative exponential, double",
            ),
            (
                ",123E+2\r\n",
                Ok(RESPValue::Double(123E+2 as f64)),
                "parses valid non-fractional, positive exponential, double",
            ),
            (
                ",inf\r\n",
                Ok(RESPValue::Double(INFINITY)),
                "parses valid inf",
            ),
            (
                ",-inf\r\n",
                Ok(RESPValue::Double(NEG_INFINITY)),
                "parses valid negative inf",
            ),
            (",nan\r\n", Ok(RESPValue::Double(NAN)), "parses valid nan"),
            (
                ",1.23e2.0\r\n",
                Err(ParseError::Invalid),
                "fails to parse double with bad exponential (fractional)",
            ),
            (
                ",1.23f1\r\n",
                Err(ParseError::Invalid),
                "fails to parse double with unknown content (random char)",
            ),
            (
                ",1.23",
                Err(ParseError::Incomplete),
                "fails to parse double with missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_big_number() {
        let cases = vec![
            (
                "(3492890328409238509324850943850943825024385\r\n",
                Ok(RESPValue::BigNum(
                    BigInt::from_str("3492890328409238509324850943850943825024385").unwrap(),
                )),
                "succeeds in parsing unsigned big number",
            ),
            (
                "(+3492890328409238509324850943850943825024385\r\n",
                Ok(RESPValue::BigNum(
                    BigInt::from_str("3492890328409238509324850943850943825024385").unwrap(),
                )),
                "succeeds in parsing positive big number",
            ),
            (
                "(-3492890328409238509324850943850943825024385\r\n",
                Ok(RESPValue::BigNum(
                    BigInt::from_str("-3492890328409238509324850943850943825024385").unwrap(),
                )),
                "succeeds in parsing negative big number",
            ),
            (
                "(349289032840923850932485094385094382502438.5\r\n",
                Err(ParseError::Invalid),
                "fails parsing big number with fractional",
            ),
            (
                "(-3492890328409238509324850943850943825024385",
                Err(ParseError::Incomplete),
                "fails parsing big number without CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_bulk_error() {
        let cases = vec![
            (
                "!21\r\nSYNTAX invalid syntax\r\n",
                Ok(RESPValue::BulkError("SYNTAX invalid syntax".to_string())),
                "succeeds in parsing valid bulk error",
            ),
            (
                "!0\r\n\r\n",
                Ok(RESPValue::BulkError("".to_string())),
                "succeeds in parsing valid bulk error (empty)",
            ),
            (
                "!23\r\nSYNTAX invalid syntax\r\n\r\n",
                Ok(RESPValue::BulkError(
                    "SYNTAX invalid syntax\r\n".to_string(),
                )),
                "succeeds in parsing valid bulk error with CRLF within content",
            ),
            (
                "!24\r\nSYNTAX invalid syntax\r\n",
                Err(ParseError::Incomplete),
                "fails parsing bulk error with length too long",
            ),
            (
                "!21\r\nSYNTAX invalid syntax",
                Err(ParseError::Incomplete),
                "fails parsing bulk error missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_verbatim_string() {
        let cases = vec![
            (
                "=15\r\ntxt:Some string\r\n",
                Ok(RESPValue::VerbatimString {
                    encoding: "txt".to_string(),
                    data: "Some string".to_string(),
                }),
                "succeeds processing valid verbatim string",
            ),
            (
                "=17\r\ntxt:Some string\r\n\r\n",
                Ok(RESPValue::VerbatimString {
                    encoding: "txt".to_string(),
                    data: "Some string\r\n".to_string(),
                }),
                "succeeds processing valid verbatim string with CRLF within content",
            ),
            (
                "=19\r\ntxt:Some string\r\n",
                Err(ParseError::Incomplete),
                "fails processing verbatim string with length too long",
            ),
            (
                "=15\r\ntxt:Some string",
                Err(ParseError::Incomplete),
                "fails processing verbatim string missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_map() {
        let cases = vec![
            (
                "%2\r\n+first\r\n:1\r\n+second\r\n:2\r\n",
                Ok(RESPValue::Map(RESPMap(HashMap::from([
                    (
                        RESPValue::SimpleString("first".to_string()),
                        RESPValue::Integer(1),
                    ),
                    (
                        RESPValue::SimpleString("second".to_string()),
                        RESPValue::Integer(2),
                    ),
                ])))),
                "succeeds parsing valid map",
            ),
            (
                "%0\r\n",
                Ok(RESPValue::Map(RESPMap(HashMap::from([])))),
                "succeeds parsing valid empty map",
            ),
            (
                "%2\r\n+first\r\n:1\r\n+second\r\n",
                Err(ParseError::Incomplete),
                "fails parsing map with missing kv-pair",
            ),
            (
                "%3\r\n+first\r\n:1\r\n+second\r\n:2\r\n",
                Err(ParseError::Incomplete),
                "fails parsing map with num elements exceeding content",
            ),
            (
                "%2\r\n+first\r\n:1\r\n+second\r\n:2",
                Err(ParseError::Incomplete),
                "fails parsing map missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_attribute() {
        let cases = vec![
            (
                "|1\r\n+key-popularity\r\n%2\r\n$1\r\na\r\n,0.1923\r\n$1\r\nb\r\n,0.0012\r\n+hello\r\n",
                Ok(RESPValue::Attribute {
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
                }),
                "succeeds parsing valid attribute",
            ),
            (
                "|0\r\n+hello\r\n",
                Ok(RESPValue::Attribute {
                    metadata: RESPMap(HashMap::from([])),
                    value: Box::new(RESPValue::SimpleString("hello".to_string())),
                }),
                "succeeds parsing valid empty attribute",
            ),
            (
                "|2\r\n+first\r\n:1\r\n+second\r\n+hello\r\n",
                Err(ParseError::Incomplete),
                "fails parsing attribute with missing kv-pair",
            ),
            (
                "|2\r\n+first\r\n:1\r\n+second\r\n",
                Err(ParseError::Incomplete),
                "fails parsing attribute with missing attributed content (following RESP)",
            ),
            (
                "|3\r\n+first\r\n:1\r\n+second\r\n:2\r\n+hello\r\n",
                Err(ParseError::Incomplete),
                "fails parsing attribute with num elements exceeding content",
            ),
            (
                "|2\r\n+first\r\n:1\r\n+second\r\n:2+hello\r\n",
                Err(ParseError::Invalid),
                "fails parsing attribute missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_set() {
        let cases = vec![
            (
                "~2\r\n+hello\r\n+world\r\n",
                Ok(RESPValue::Set(RESPSet(HashSet::<RESPValue>::from([
                    RESPValue::SimpleString("hello".to_string()),
                    RESPValue::SimpleString("world".to_string()),
                ])))),
                "succeeds parsing valid set",
            ),
            (
                "~0\r\n",
                Ok(RESPValue::Set(RESPSet(HashSet::<RESPValue>::from([])))),
                "succeeds parsing valid set (empty)",
            ),
            (
                "~3\r\n+hello\r\n+world\r\n",
                Err(ParseError::Incomplete),
                "fails parsing set denoting too many elements",
            ),
            (
                "~2\r\n+hello\r\n+world",
                Err(ParseError::Incomplete),
                "fails parsing set with missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }

    #[test]
    fn test_push() {
        let cases = vec![
            (
                ">2\r\n+hello\r\n+world\r\n",
                Ok(RESPValue::Push(MultiBulk(vec![
                    RESPValue::SimpleString("hello".to_string()),
                    RESPValue::SimpleString("world".to_string()),
                ]))),
                "succeeds parsing valid push",
            ),
            (
                ">0\r\n",
                Ok(RESPValue::Push(MultiBulk(vec![]))),
                "succeeds parsing valid push (empty)",
            ),
            (
                ">3\r\n+hello\r\n+world\r\n",
                Err(ParseError::Incomplete),
                "fails parsing push denoting too many elements",
            ),
            (
                "%2\r\n+first\r\n:1\r\n+second\r\n>0\r\n\r\n",
                Err(ParseError::Invalid),
                "fails parsing nested push",
            ),
            (
                ">2\r\n+hello\r\n+world",
                Err(ParseError::Incomplete),
                "fails parsing push with missing CRLF",
            ),
        ];

        for (input, expected, msg) in cases {
            assert_eq!(input.parse::<RESPValue>(), expected, "{}", msg);
        }
    }
}
