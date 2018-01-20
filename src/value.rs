
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
pub enum Atom<'s> {
    Chars(Cow<'s, str>),
    Val(Cow<'s, Value>) // TODO avoid copies
}



/* UNBALANCED, BORROWED rope data structure
 * Slightly inspired by: https://github.com/mthadley/rupe/blob/master/src/lib.rs
 * (Not quite a rope at the moment) */
#[derive(Clone, Debug)]
pub enum Rope<'s> {
    Node(Box<Rope<'s>>, Box<Rope<'s>>),
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

}

impl<'s> Rope<'s> {
    pub fn shallow_copy(&'s self) -> Rope<'s> {
        // SLOOOOW may need to use refcells or something?
        match self {
            &Rope::Nothing => { Rope::Nothing },
            &Rope::Node(ref l, ref r) => {
                Rope::Node(
                    Box::new( l.shallow_copy() ),
                    Box::new( r.shallow_copy() )
                )
            },
            &Rope::Chars(ref c) => {
                Rope::Chars(Cow::Borrowed(&*c))
            },
            &Rope::Val(ref v) => { Rope::Val(Cow::Borrowed(&*v)) }
        }
    }
    pub fn new() -> Rope<'s> {
        return Rope::Nothing
    }

    pub fn make_static(&self) -> Rope<'static> {
        match self {
            &Rope::Nothing => { Rope::Nothing },
            &Rope::Node(ref l, ref r) => {
                Rope::Node(
                    Box::new(l.make_static()),
                    Box::new(r.make_static())
                )
            },
            &Rope::Chars(ref c) => {
                let owned = Cow::Owned((**c).to_owned());
                Rope::Chars(owned)
            },
            &Rope::Val(ref v) => { Rope::Val( Cow::Owned( v.clone().into_owned() ) ) }
        }
    }


    pub fn concat(self, other: Rope<'s>) -> Rope<'s> {
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

    pub fn atomize<'f>(&'f self) -> Vec<Atom<'f>> {
        // SLOOOW
        match self {
            &Rope::Nothing => { vec![] },
            &Rope::Node(ref left, ref right) => {
                let mut v = left.atomize();
                v.extend(right.atomize());
                v
            },
            &Rope::Val(ref v) => { vec![
                Atom::Val(Cow::Borrowed(&*v))
            ] },
            &Rope::Chars(ref c) => { vec![ Atom::Chars((*c).clone()) ] }
        }

    }
    pub fn split_at<'r : 's, 'q, F : FnMut(char) -> bool>
        (&mut self, match_val : bool, matcher: &mut F)
        -> Option<Rope<'s>>  {
        let mut out : Option<Rope<'s>> = None;
        match self {
            &mut Rope::Nothing => { },
            &mut Rope::Node(ref mut left, ref mut right) => {
                match left.split_at(match_val, matcher) {
                    Some(result) => {
                        return Some(result);
                    }
                    None => {
                        match right.split_at(match_val, matcher) {
                            Some(result) => { 
                                out = Some(
                                    replace(left, Box::new(Rope::Nothing))
                                        .concat(result)
                                );
                            },
                            None => {
                                replace(left,Box::new(Rope::Nothing));
                                replace(right, Box::new(Rope::Nothing));
                            }
                        }
                    }
                }
            },
            &mut Rope::Val(Cow::Owned(_)) => {
                if match_val {
                } else {
                    // todo fix this
                    out = Some(replace(self, Rope::Nothing))
                }
            },
            &mut Rope::Val(Cow::Borrowed(v)) => {
                if match_val {
                } else {
                    out = Some(Rope::Val(Cow::Borrowed(v)));
                    *self = Rope::Nothing
                }
            },
            &mut Rope::Chars(Cow::Borrowed(ref mut cow)) => {
                if let Some(idx) = cow.find(|x| { matcher(x) }) {
                    let pair = (**cow).split_at(idx);
                    out = Some(Rope::Chars(Cow::Borrowed(&*pair.0)));
                    replace(cow, &*pair.1);
                }
            },
            &mut Rope::Chars(Cow::Owned(ref mut string)) => {
                // relatively slow; hence borrowed ropes are preferred
                // TODO borrow closure ropes when possible
                if let Some(idx) = string.find(|x| { matcher(x) }) {
                    let rest = string.split_off(idx);
                    out = Some(Rope::Chars(Cow::Owned(string.clone())));
                    *string = rest
                }
            }
        };
        out
    }

    pub fn get_str(&self) -> Cow<str> {
        match self {
            &Rope::Nothing => { Cow::Borrowed("") },
            &Rope::Chars(Cow::Owned(ref ch)) => { Cow::Owned(ch.clone()) },
            &Rope::Chars(Cow::Borrowed(ch)) => { Cow::Borrowed(ch) },
            &Rope::Val(_) => { panic!("Unexpected value!") },
            &Rope::Node(ref left, ref right) => {
               left.get_str() + right.get_str()
            }
        }
    }

    pub fn get_char(&self) -> Option<char> {
        match self {
            &Rope::Nothing => { None },
            &Rope::Node(ref left, ref right) => { left.get_char().or(right.get_char()) },
            &Rope::Val(_) => { panic!("Unexpected value!") },
            &Rope::Chars(ref ch) => { ch.chars().next() }
        }
    }

    pub fn split_char(&mut self) -> Option<char> {

        match self {
            &mut Rope::Nothing => { None },
            &mut Rope::Node(ref mut left, ref mut right) => {
                left.split_char().or_else(|| { right.split_char() })
            },
            &mut Rope::Val(_) => { panic!("Unexpected value!") }, 
            &mut Rope::Chars(Cow::Owned(ref mut ch)) => {
                if ch.len() > 0 {
                    Some(ch.remove(0))
                } else {
                    None
                }
            },
            &mut Rope::Chars(Cow::Borrowed(ref mut ch)) => {
                if ch.len() > 0 {
                    let (first, rest) = ch.split_at(1);
                    *ch = rest;
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
