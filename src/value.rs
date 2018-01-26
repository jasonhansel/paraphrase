

// Various helper data types and associated trait implementations.
// Main TODOs here include:
// - Improve performance by avoiding clone()s as much as possible.
// - Simplify things. This is far more complex than necessary.

use std::fmt;
use scope::Scope;
use std::iter;
use std::ops::Range;
use std::sync::Arc;
use std::ops::{Add,Index,IndexMut};

use serde_json::Value as JValue;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tag(pub usize);

#[derive(Clone)]
pub struct ValueClosure(pub Arc<Scope>, pub Box<Rope>);

impl ValueClosure {
    fn from(scope: Arc<Scope>, rope: Rope) -> ValueClosure {
        ValueClosure(scope, Box::new(rope))
    }
}

// A slice of an atomically reference-counted string.
pub struct ArcSlice {
    string: Arc<String>,
    range: Range<usize>
}

impl ArcSlice {
    pub fn from_string(s: String) -> ArcSlice {
        return ArcSlice {
            range: (0..(s.len())),
            string: Arc::new(s)
        }
    }

    pub fn empty() -> ArcSlice {
        return ArcSlice::from_string("".to_owned())
    }

    pub fn to_str(& self) -> & str {
        return &self.string[self.range.clone()];
    }
    
   pub fn into_string(self) -> String {

        return self.to_str().to_owned();

        let range = self.range.clone();
        let res = Arc::try_unwrap(self.string)
            .map(|mut x| { x.split_off(range.end); x.split_off(range.start)   })
            .unwrap_or_else(|x| { (&x[range.clone()]).to_owned()  });
        res
    }

    fn split_first(&mut self) -> Option<char> {
        if self.range.start != self.range.end {
            let ch = self.to_str().chars().next();
            self.range.start += 1;
            ch
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.range.len()
    }

    fn split_at<'t>(&'t mut self, idx: usize) -> (ArcSlice) {
        let mut left = ArcSlice::from_string(self.to_str().to_owned());
        left.range = Range{ start: 0, end: idx};
            // ArcSlice { string: self.string.clone(), range: Range { start: self.range.start, end: self.range.start+idx } };
        (*self).range.start += idx;
        left
    }

    pub fn index(&self, range: Range<usize>) -> ArcSlice {
        return ArcSlice {
            string: self.string.clone(),
            range: Range { start: self.range.start + range.start, end: self.range.start + range.end }
        }
    }
}

impl fmt::Debug for ArcSlice {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.to_str().fmt(f)
	}
}

impl Add for ArcSlice {
    type Output = ArcSlice;
    fn add(self, other: ArcSlice) -> ArcSlice {
        if self.len() == 0 {
            other
        } else if other.len() == 0 {
            self
        } else {
            let mut s = self.into_string();
            s.push_str(other.to_str());
            ArcSlice::from_string(s)
        }
    }
}


impl Clone for ArcSlice {
    fn clone(&self) -> ArcSlice {
		// Generally Arc is faster than cloning the string
		let b = ArcSlice {
			string: self.string.clone(),
			range: self.range.clone()
		};
		b
    }
}



// A Value is something that can be used as a parameter to a macro,
// or as a return value.
#[derive(Clone,Debug)]
pub enum Value {
    Str(ArcSlice),
    List(Vec<Value>),
    Tagged(Tag,Box<Value>),
    Closure(ValueClosure),
}
use Value::*;

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (&Str(ref a), &Str(ref b)) => { a.to_str() == b.to_str() },
            (&List(ref a), &List(ref b)) => { a == b }
            (&Tagged(ref a1, ref b1), &Tagged(ref a2, ref b2)) => { a1 == a2 && b1 == b2 },
            (&Closure(_), _)
            | (_, &Closure(_)) => { panic!() },
            _ => false
        }
    }
}

impl From<JValue> for Value {
	fn from(val: JValue) -> Value {
		match val {
			JValue::String(s) => { Value::Str(ArcSlice::from_string(s)) },
			JValue::Array(a) => { Value::List(a.into_iter().map(Value::from).collect()) },
			_ => { panic!("Only strings are supported in JSON"); }
		}
	}
}


/* UNBALANCED, BORROWED rope data structure
 * Slightly inspired by: https://github.com/mthadley/rupe/blob/master/src/lib.rs
 * (Not quite a rope at the moment) */

#[derive(Clone,Debug)]
enum Leaf {
    Own(Value),
    Chr(ArcSlice) // from/to
}

use self::Leaf::*;

#[derive(Clone,Debug)]
pub struct Rope {
    data: Vec<Leaf>
}

impl ValueClosure {
    pub fn force_clone(&mut self) -> ValueClosure {
        match self {
           &mut ValueClosure(ref sc, ref mut ro) => { ValueClosure(sc.clone(), ro.clone() ) },
        }
    }
}
impl Value {
   pub fn as_str(self) -> Option<ArcSlice> {
        match self {
            Str(s) => Some(s),
            _ => None
        }
    }
}

impl Leaf {
    pub fn to_str(self) -> Option<ArcSlice> {
        match self {
            Leaf::Chr(c) => { Some(c) },
            Leaf::Own(Value::Str(v)) => { Some(v) },
            _ => { None }
        }
    }
}



impl Rope {
    pub fn new() -> Rope {
        return Rope {
            data: Vec::new()
        }
    }

    pub fn from_value(value: Value) -> Rope {
        Rope {
            data: iter::once(Leaf::Own(value)).collect()
        }
    }
    pub fn from_slice(value: ArcSlice) -> Rope {
        Rope {
            data: iter::once(Leaf::Chr(value)).collect()
        }
    }


    pub fn concat(mut self, mut other: Rope) -> Rope {
		// TODO: merge strings together to improve perf.
        return Rope {
            data: {
                let mut l = Vec::new();
                l.append(&mut other.data);
                l.append(&mut self.data);
                l
            }
        }
    }

    fn is_empty(&self) -> bool {
        for leaf in self.data.iter() {
            match leaf {
                &Chr(ref c) => { if c.range.len() != 0 { return false } },
                &Own(_) => { return false }
            }
        }
        return true
    }
    fn should_be_string(&self) -> bool {
        let mut count = 0;
        for leaf in self.data.iter().rev() {
            match leaf {
                &Leaf::Chr(ref c) => { if c.to_str().chars().any(|x| { !x.is_whitespace() }) { return true } }
                &Own(_) => { count += 1 }
            }
        }
        count != 1
    }

    pub fn to_str(self) -> Option<ArcSlice> {
        let mut string : ArcSlice = ArcSlice::from_string("".to_owned());
		for v in self.data.into_iter().rev() {
            match v.to_str() {
                // TODO avoid copies
                Some(x) => { string = string + x; },
                // for some reason, adding the string below doesn't work
                None => { return None }
            }
        }
        Some(string)
    }

    pub fn coerce_list(self) -> Value {
        let mut vec = vec![];
        for val in self.data.into_iter().rev() { match val {
            Chr(c) => {
                if c.to_str().chars().any(|x| { !x.is_whitespace() }) { panic!(); }
            },
            Own(v) => { vec.push(v) }
        } }
        Value::List(vec)
    }

    pub fn coerce(self) -> Value {
       if self.should_be_string() {
            Value::Str( self.to_str().unwrap() )
        } else {
            for val in self.data.into_iter() { match val {
                Chr(_) => { },
                Own(v) => { return v }
            } }
            panic!("Failure");
       }
    }
    pub fn split_at<'r,F : FnMut(char) -> bool>(&'r mut self, match_val : bool, match_eof: bool, matcher: &mut F)
        ->  Option<Rope>  {
        let mut prefix = Rope { data: Vec::new() };
        while !self.data.is_empty() {
            let front = self.data.pop().unwrap();
            match front {
                Leaf::Own(_) => {
                    if match_val {
                        prefix.data.insert(0,front);
                    } else {
                        self.data.push(front);
                        return Some(prefix);
                    }
                },
                Leaf::Chr(mut slice) => {
                    if let Some(idx) = slice.to_str().find(|x| { matcher(x) }) {
                        if idx > 0 {
                            let start = slice.split_at(idx);
                            prefix.data.insert(0,Leaf::Chr(start));
                        }
                        self.data.push(Leaf::Chr(slice));
                        return Some(prefix)
                    } else {
                        prefix.data.push(Leaf::Chr(slice));
                    }
                }
            };
        }
        if match_eof {
            return Some(prefix)
        } else {
            *self = prefix;
            return None
        }
    }

    pub fn get_char(&self) -> Option<char> {
        match self.data.last()? {
            &Leaf::Own(_) => { None },
            &Leaf::Chr(ref ch) => {
                ch.to_str().chars().next()
            }
        }
    }

    pub fn split_char(&mut self) -> Option<char> {
        let mut first = true;
        return self.split_at(false, true, &mut |_| {
            if first == true {
                first = false;
                false
            } else {
                true
            }
        }).and_then(|x| x.get_char());
    }

}

impl fmt::Debug for ValueClosure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &ValueClosure(ref scope, ref x) = self;
        scope.fmt(f)?;
        write!(f, "CODE<");
        for leaf in x.data.iter() {
            leaf.fmt(f)?
        }
        write!(f, ">")?;
        Ok(())
    }
}

