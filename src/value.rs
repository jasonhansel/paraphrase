
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
    Node(Box<Cow<'s, Rope<'s>>>, Box<Cow<'s, Rope<'s>>>),
    Chars(Cow<'s, str>),
    Val(Cow<'s, Value>),
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

impl Rope<'static> {
    pub fn shallow_copy<'t, 'r : 't>(&'r self) -> &'r mut Rope<'t> {
        match self {
            &Rope::Nothing => { Rope::Nothing },
            &Rope::Node(_, _) => {
                if let &Rope::Node(l, r) = (self as &Rope<'t>) {
                    let nl = (&*l) as &Rope<'t>;
                    let nr = (&*r) as &Rope<'t>;
                    Rope::Node(
                        Box::new(Cow::Borrowed(nl)),
                        Box::new(Cow::Borrowed(nr))
                    ) }
                }
            &Rope::Chars(ref c) => {
                Rope::Chars(Cow::Borrowed(&*c))
            },
            &Rope::Val(ref v) => { Rope::Val(Cow::Borrowed(&*v)) }
        }
    }
}

impl<'s> Rope<'s> {
    pub fn new() -> Rope<'s> {
        return Rope::Nothing
    }

    pub fn make_static(&self) -> Rope<'static> {
        match self {
            &Rope::Nothing => { Rope::Nothing },
            &Rope::Node(ref l, ref r) => { Rope::Node(
                    Box::new(Cow::Owned(l.make_static())),
                    Box::new(Cow::Owned(r.make_static())))
            },
            &Rope::Chars(ref c) => {
                let owned = Cow::Owned((**c).to_owned());
                Rope::Chars(owned)
            },
            &Rope::Val(ref v) => { Rope::Val( Cow::Owned( v.into_owned() ) ) }
        }
    }

    pub fn read_file<'r>(file: &mut File) -> Result<Rope<'r>, Error> {
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        Ok(Rope::Chars(Cow::from(s)))
    }

    pub fn concat<'q, 'r>(self, other: Rope<'s>) -> Rope<'s> {
        Rope::Node(Box::new(Cow::Owned(self)), Box::new(Cow::Owned(other)))
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
            &Rope::Val(ref v) => { vec![
                Atom::Val(v.into_owned())
            ] },
            &Rope::Chars(ref c) => { vec![ Atom::Chars((*c).clone()) ] }
        }

    }
    pub fn split_at<'r : 's, 'q, F : FnMut(char) -> bool>
        (&'s mut self, match_val : bool, mut matcher: F)
        -> Option<Rope<'s>>  {
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
                                let val = left;
                                out = Some(
                                    **replace(left, Box::new(Cow::Owned(Rope::Nothing)))
                                );
                            },
                            None => {
                                replace(left,Box::new(Cow::Owned(Rope::Nothing)));
                                replace(right, Box::new(Cow::Owned(Rope::Nothing)));
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
                    let pair = (**cow).split_at(idx);
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

    pub fn split_char(&mut self) -> Option<char> {

        match self {
            &mut Rope::Nothing => { None },
            &mut Rope::Node(ref left, ref right) => {
                left.split_char().or_else(|| { right.split_char() })
            },
            &mut Rope::Val(v) => { panic!("Unexpected value!") },
            &mut Rope::Chars(ch) => {
                if ch.len() > 0 {
                    let (first, rest) = ch.split_at(1);
                    *self = Rope::Chars(Cow::Borrowed(rest));
                    first.chars().next()
                } else {
                    None
                }
            }
        }
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
