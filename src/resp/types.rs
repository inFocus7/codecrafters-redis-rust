use num_bigint::BigInt;
use ordered_float::OrderedFloat;
use std::cmp::{Eq, PartialEq};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

pub const CRLF: &str = "\r\n";

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Invalid,
    Incomplete,
}

impl std::error::Error for ParseError {}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Invalid => {
                write!(f, "failed to parse invalid RESP value")
            }
            ParseError::Incomplete => {
                write!(f, "failed to parse RESP value, possible incomplete data")
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MultiBulk(pub Vec<RESPValue>);

impl From<Vec<String>> for MultiBulk {
    fn from(value: Vec<String>) -> Self {
        MultiBulk(
            value
                .into_iter()
                .map(|e| RESPValue::BulkString(e))
                .collect(),
        )
    }
}

impl std::ops::Deref for MultiBulk {
    type Target = Vec<RESPValue>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for MultiBulk {
    type Item = RESPValue;
    type IntoIter = std::vec::IntoIter<RESPValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a MultiBulk {
    type Item = &'a RESPValue;
    type IntoIter = std::slice::Iter<'a, RESPValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Debug, Clone)]
pub struct RESPMap(pub HashMap<RESPValue, RESPValue>);

impl Hash for RESPMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (key, value) in self.0.iter() {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl Eq for RESPMap {}
impl PartialEq for RESPMap {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[derive(Debug, Clone)]
pub struct RESPSet(pub HashSet<RESPValue>);

impl Hash for RESPSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for value in self.0.iter() {
            value.hash(state);
        }
    }
}

impl Eq for RESPSet {}
impl PartialEq for RESPSet {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

pub mod resp_prefix {
    pub const SIMPLE_STRING: char = '+';
    pub const SIMPLE_ERROR: char = '-';
    pub const INTEGER: char = ':';
    pub const NULL: char = '_';
    pub const BOOLEAN: char = '#';
    pub const DOUBLE: char = ',';
    pub const BIG_NUM: char = '(';
    pub const BULK_STRING: char = '$';
    pub const NULL_BULK_STRING: char = '$';
    pub const ARRAY: char = '*';
    pub const NULL_ARRAY: char = '*';
    pub const BULK_ERROR: char = '!';
    pub const VERBATIM_STRING: char = '=';
    pub const MAP: char = '%';
    pub const ATTRIBUTE: char = '|';
    pub const SET: char = '~';
    pub const PUSH: char = '>';
}

#[derive(Debug, Clone)]
pub enum RESPValue {
    SimpleString(String), // +<string>\r\n
    SimpleError(String), // -<string>\r\n; MAY have a prefix `-ERRPREFIX <message>\r\n` where `ERRPREFIX` is fully capitalized first word
    Integer(i64), // :<number>\r\n; unsigned 64-bit integer, but can be negative (+/- after initial :)
    Null,         // _\r\n; a null value; preferred in RESP3 for null values
    Boolean(bool), // #<t|f>\r\n; boolean value t=true, f=false
    Double(f64), // ,[+|-]<integral>[.<fractional>][<E|e>[sign]<exponent>]\r\n; a comma followed by optional sign, integer, optional decimal, and optional exponent
    BigNum(BigInt), // ([+|-]<number>\r\n; a big integer (w.c.s will conver to string)
    BulkString(String), // $<length>\r\n<data>\r\n; a single binary string
    NullBulkString, // $-1\r\n; a null bulk string
    Array(MultiBulk), // *<number of elements>\r\n<element 1>\r\n<...element n>\r\n; an array of RESP values
    NullArray,
    BulkError(String), // !<length>\r\n<error>\r\n
    VerbatimString {
        encoding: String,
        data: String,
    }, // =<length>\r\n<encoding format>:<string>\r\n; provides explicit encoding info
    Map(RESPMap), // %<number of entries>\r\n<key 1>\r\n<value 1>\r\n<...key n>\r\n<...value n>\r\n;
    Attribute {
        metadata: RESPMap,
        value: Box<RESPValue>,
    }, // |<number of entries>\r\n<key 1>\r\n<value 1>\r\n<...key n>\r\n<...value n>\r\n; a set of key-value pairs that can be used to provide additional information about the message but is not part of the message itself (client can optionally return it or show it somehow)
    Set(RESPSet), // ~<num of elements>\r\n<element 1>\r\n<...element n>\r\n; an unordered collection of RESP values; client should return its native representation of a set
    Push(MultiBulk), // ><num of elements>\r\n<element 1>\r\n<...element n>\r\n; similar to arrays; pushes may precede or follow RESP data, but must never be inside (e.g. not inside a map)
}

// HashMap and HashSet traits
impl Hash for RESPValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            RESPValue::SimpleString(s) => s.hash(state),
            RESPValue::SimpleError(s) => s.hash(state),
            RESPValue::Integer(i) => i.hash(state),
            RESPValue::Null => ().hash(state),
            RESPValue::Boolean(b) => b.hash(state),
            RESPValue::Double(d) => OrderedFloat(*d).hash(state),
            RESPValue::BigNum(bn) => bn.hash(state),
            RESPValue::BulkString(s) => s.hash(state),
            RESPValue::NullBulkString => ().hash(state),
            RESPValue::Array(arr) => arr.hash(state),
            RESPValue::NullArray => ().hash(state),
            RESPValue::BulkError(s) => s.hash(state),
            RESPValue::VerbatimString { encoding, data } => {
                encoding.hash(state);
                data.hash(state);
            }
            RESPValue::Map(map) => map.hash(state),
            RESPValue::Attribute { metadata, value } => {
                metadata.hash(state);
                value.hash(state);
            }
            RESPValue::Set(set) => set.hash(state),
            RESPValue::Push(push) => push.hash(state),
        }
    }
}

impl Eq for RESPValue {}
impl PartialEq for RESPValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RESPValue::SimpleString(s1), RESPValue::SimpleString(s2)) => s1 == s2,
            (RESPValue::SimpleError(s1), RESPValue::SimpleError(s2)) => s1 == s2,
            (RESPValue::Integer(i1), RESPValue::Integer(i2)) => i1 == i2,
            (RESPValue::Null, RESPValue::Null) => true,
            (RESPValue::Boolean(b1), RESPValue::Boolean(b2)) => b1 == b2,
            (RESPValue::Double(d1), RESPValue::Double(d2)) => {
                OrderedFloat(*d1) == OrderedFloat(*d2)
            }
            (RESPValue::BigNum(bn1), RESPValue::BigNum(bn2)) => bn1 == bn2,
            (RESPValue::BulkString(s1), RESPValue::BulkString(s2)) => s1 == s2,
            (RESPValue::NullBulkString, RESPValue::NullBulkString) => true,
            (RESPValue::Array(arr1), RESPValue::Array(arr2)) => arr1 == arr2,
            (RESPValue::NullArray, RESPValue::NullArray) => true,
            (RESPValue::BulkError(s1), RESPValue::BulkError(s2)) => s1 == s2,
            (
                RESPValue::VerbatimString {
                    encoding: e1,
                    data: d1,
                },
                RESPValue::VerbatimString {
                    encoding: e2,
                    data: d2,
                },
            ) => e1 == e2 && d1 == d2,
            (RESPValue::Map(map1), RESPValue::Map(map2)) => map1 == map2,
            (
                RESPValue::Attribute {
                    metadata: m1,
                    value: v1,
                },
                RESPValue::Attribute {
                    metadata: m2,
                    value: v2,
                },
            ) => m1 == m2 && v1 == v2,
            (RESPValue::Set(set1), RESPValue::Set(set2)) => set1 == set2,
            (RESPValue::Push(push1), RESPValue::Push(push2)) => push1 == push2,
            _ => false,
        }
    }
}
