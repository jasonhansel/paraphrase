
use std::fmt;
use std::rc::Rc;
use std::borrow::Borrow;
use scope::Scope;
use std::borrow::Cow;

use std::result::Result;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::iter::{empty, once};
use std::mem::{swap, replace};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tag(u64);


// should closures "know" about their parameters?
#[derive(Clone)]
pub struct ValueClosure(pub Rc<Scope>, pub Box<Rope<'static>>);

// A Value is something that can be used as a parameter to a macro,
// or as a return value.
#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    List(Vec<Value>),
    Tagged(Tag,Box<Value>),
    Closure(ValueClosure),
    Bubble(ValueClosure) // <- gets auto-expanded when it reaches its original scope
}

// An Atom represents an "atomic" piece of text that is to be expanded.
#[derive(Clone, Debug)]
pub enum Atom<T: Borrow<str>> {
    Chars(T),
    Val(Value)
}



/* UNBALANCED, BORROWED rope data structure
 * Slightly inspired by: https://github.com/mthadley/rupe/blob/master/src/lib.rs
 * (Not quite a rope at the moment) */
#[derive(Clone, Debug)]
pub enum Rope<'s> {
    Node(Box<Rope<'s>>, Box<Rope<'s>>),
    Chars(Cow<'s, str>),
    Val(Value),
    Nothing
}

#[derive(Debug)]
pub enum Token<'f> {
    CommandName(Rope<'f>),
    StartParen,
    EndParen,
    RawParam(Rope<'f>),
    Semicolon(Rope<'f>),
    Text(Rope<'f>)
}

pub use Token::*;

impl<'s> Rope<'s> {

    pub fn new() -> Rope<'s> {
        return Rope::Nothing
    }

    pub fn make_static(&self) -> Rope<'static> {
        match self {
            &Rope::Nothing => { Rope::Nothing },
            &Rope::Node(ref l, ref r) => { Rope::Node(Box::new(l.make_static()), Box::new(r.make_static())) },
            &Rope::Chars(ref c) => {
                let owned = Cow::Owned((**c).to_owned());
                Rope::Chars(owned)
            },
            &Rope::Val(ref v) => { Rope::Val(v.clone()) }
        }
    }

    pub fn read_file<'r>(file: &mut File) -> Result<Rope<'r>, Error> {
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        Ok(Rope::Chars(Cow::from(s)))
    }

    pub fn concat<'q, 'r>(self, other: Rope<'r>) -> Rope<'q>
    where 'r : 'q, 's : 'q{
        Rope::Node(Box::new(self), Box::new(other))
    }

    pub fn is_empty(&self) -> bool {
         match self {
            &Rope::Nothing => { true }
            &Rope::Node(ref l, ref r) => { l.is_empty() || r.is_empty() }
            &Rope::Chars(ref c) => { c.len() == 0 }
            &Rope::Val(_) => { false }
        }
    }

    // may want to make this stuff iterative

    pub fn atomize(&self) -> Vec<Atom<Cow<str>>> {
        // SLOOOW
        match self {
            &Rope::Nothing => { vec![] },
            &Rope::Node(ref left, ref right) => {
                let mut v = left.atomize();
                v.extend(right.atomize());
                v
            },
            &Rope::Val(ref v) => { vec![ Atom::Val(v.clone()) ] },
            &Rope::Chars(ref c) => { vec![ Atom::Chars(c.clone()) ] }
        }

    }
    pub fn split_at<'r : 's, F : FnMut(char) -> bool>
        (&'r mut self, match_val : bool, mut matcher: F)
        -> Option<Rope<'s>> {
        let mut out : Option<Rope<'s>> = None;
        match self {
            &mut Rope::Nothing => { },
            &mut Rope::Node(ref mut left, ref mut right) => {
                match left.split_at(match_val, &mut matcher) {
                    Some(result) => {
                        return Some(result);
                    }
                    None => {
                        match right.split_at(match_val, &mut matcher) {
                            Some(result) => { 
                                let mut val = Box::new(Rope::Nothing);
                                swap(&mut val, left);
                                out = Some(*val);
                            },
                            None => {
                                replace(left,Box::new(Rope::Nothing));
                                replace(right, Box::new(Rope::Nothing));
                            }
                        }
                    }
                }
            },
            &mut Rope::Val(v) => {
                if match_val {
                } else {
                    out = Some(Rope::Val(v));
                    *self = Rope::Nothing
                }
            },
            &mut Rope::Chars(ref cow) => {
                if let Some(idx) = cow.find(matcher) {
                    let pair = cow.split_at(idx);
                    out = Some(Rope::Chars(Cow::from(pair.0)));
                    *self = Rope::Chars(Cow::from(pair.1))
                } 
            }
        };
        out
    }

    pub fn get_str(&self) -> Cow<str> {
        match self {
            &Rope::Nothing => { Cow::from("") },
            &Rope::Chars(ch) => { ch },
            &Rope::Val(v) => { panic!("Unexpected value!") },
            &Rope::Node(ref left, ref right) => {
               left.get_str() + right.get_str()
            }
        }
    }

    pub fn get_char(&self) -> Option<char> {
        match self {
            &Rope::Nothing => { None },
            &Rope::Node(ref left, ref right) => { left.get_char().or(right.get_char()) },
            &Rope::Val(v) => { panic!("Unexpected value!") },
            &Rope::Chars(ch) => { ch.chars().next() }
        }
    }

    pub fn split_char<'r : 's>(&'r mut self) -> Option<char> {
       self.split_at(false, |_| { true })
           .and_then(|r| { r.get_char() })
    }
}






pub use Value::*;
pub use Atom::*;
/*
impl fmt::Debug for ValueList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ValueList(ref x) => { 
                write!(f, "List<")?;
                let mut first = true;
                for item in x.iter() {
                    if first { first = false; } else { write!(f, "|")?; }
                    item.fmt(f)?;
                }
                write!(f, ">")?;

            }
        }
        Ok(())
    }
}
*/
impl fmt::Debug for ValueClosure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &ValueClosure(ref scope, ref x) = self;
        write!(f, "[@");
        let mut first = true;
        for k in scope.commands.keys() {
            if first { first = false; } else { write!(f, "|")?; }
            k.fmt(f)?;
        }
        write!(f, "]CODE<");
        first = true;
        for item in x.atomize() {
            if first { first = false; } else { write!(f, "|")?; }
            item.fmt(f)?;
        }
        write!(f, ">")?;
        Ok(())
    }
}
/*
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Char(ValueChar(x)) => { write!(f, "{}",x)?; },
            &List(ref x) => { x.fmt(f)?; },
            &Closure(ref x) => { x.fmt(f)?; },
            &Tagged(ref tag, ValueList(ref x)) => {
                write!(f,"[");
                tag.fmt(f)?; 
                write!(f,"]<")?;
                let mut first = true;

                for item in x.iter() {
                    if first { first = false; } else { write!(f, "|")?; }
                    item.fmt(f)?;
                }
                write!(f, ">")?;
            },
       }
        Ok(())
    }
}
*/
impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (&Str(ref a), &Str(ref b)) => { a == b },
            (&List(ref a), &List(ref b)) => { a == b },
            (&Tagged(ref at, ref ad), &Tagged(ref bt, ref bd)) => {
                at == bt && ad == bd
            },
            (&Closure(_), _)
            | (_, &Closure(_)) => { panic!("Cannot compare closures!"); },
            (_, _) => false
        }
    }

}
