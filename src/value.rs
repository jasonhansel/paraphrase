
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
pub struct ValueClosure(pub Rc<Scope>, pub Box<Rope<'static>>);

impl ValueClosure {
    fn from<'s>(scope: Rc<Scope>, rope: Rope<'s>) -> ValueClosure {
        ValueClosure(scope, Box::new(rope.make_static()))
    }
}


// A Value is something that can be used as a parameter to a macro,
// or as a return value.
#[derive(Debug)]
pub enum Value<'s> {
    Str(Cow<'s, str>),
    List(Vec<&'s Value<'s>>),
    OwnedList(Vec<Value<'s>>),
    Tagged(Tag,Box<Value<'s>>),
    Closure(ValueClosure),
    Bubble(ValueClosure) // <- gets auto-expanded when it reaches its original scope
}

pub use Value::*;

impl ValueClosure {
    pub fn force_clone(&self) -> ValueClosure {
        match self {
           &ValueClosure(ref sc, ref ro) => { ValueClosure(sc.clone(), Box::new(ro.make_static() )) },
        }
    }
}

impl<'s,'t> Value<'s> {
    pub fn make_static(&'t self) -> Value<'static> {
        match self {
            // FIXME: Cow::Owned will cause excessive copying later
            &Str(ref s) => { Str(Cow::Owned(s.clone().into_owned())) },
            &List(ref l) => { OwnedList(l.iter().map(|x| { x.make_static() }).collect()) },
            &OwnedList(ref l) => { OwnedList(l.iter().map(|x| { x.make_static() }).collect()) },
            &Tagged(ref t, ref v) => { Tagged(*t, Box::new(v.make_static())) },
            &Closure(ValueClosure(ref sc, ref ro)) => { Closure(ValueClosure(sc.clone(), Box::new(ro.make_static() ))) },
            &Bubble(ValueClosure(ref sc, ref ro)) => { Closure(ValueClosure(sc.clone(), Box::new(ro.make_static() ))) },
        }
    }
}

impl<'s> Leaf<'static> {
    pub fn dupe(&'s self) -> Leaf<'s> {
        // AVOID USING THIS
        match self {
            &Val(ref v) => Val(v),
            &Own(ref v) => Val(v),
            &Chr(ref v) => Chr(Cow::Owned(v.clone().into_owned())),
        }
    }
}

impl<'s> Rope<'static> {
    pub fn dupe(&'s self) -> Rope<'s> { 
        match self {
            &Rope::Node(ref l, ref r) => Rope::Node(Box::new(l.dupe()),Box::new(r.dupe())),
            &Rope::Leaf(ref l) => Rope::Leaf(l.dupe()),
            &Rope::Nil => Rope::Nil
        }
    }
}
/* UNBALANCED, BORROWED rope data structure
 * Slightly inspired by: https://github.com/mthadley/rupe/blob/master/src/lib.rs
 * (Not quite a rope at the moment) */

#[derive(Debug)]
pub enum Leaf<'s> {
    Val(&'s Value<'s>),
    Own(Box<Value<'s>>),
    Chr(Cow<'s, str>)
}

use Leaf::*;

#[derive(Debug)]
pub enum Rope<'s> {
    Node(Box<Rope<'s>>, Box<Rope<'s>>),
    Leaf(Leaf<'s>),
    Nil
}


impl<'s> Leaf<'s> {
    pub fn to_str(&self) -> Option<&Cow<'s, str>> {
        match self {
            &Leaf::Chr(ref c) => { Some(c) },
            &Leaf::Val(&Value::Str(ref v)) => { Some(v) }
            &Leaf::Own(ref v) => {
                match &**v {
                    &Value::Str(ref v) => { Some(v) },
                    _ => None
                }
            },
            _ => None
        }
    }
    pub fn to_val(&self) -> &Value {
        match self {
            &Leaf::Chr(_) => { panic!() },
            &Leaf::Val(ref v) => { v }
            &Leaf::Own(ref v) => { v }
        }
    }
    pub fn make_static(&self) -> Leaf<'static> { match self {
        // TODO avoid this at all costs
        &Chr(ref c) => {
            let owned = Cow::Owned(c.clone().into_owned());
            Leaf::Chr(owned)
        },
        &Val(ref v) => { Leaf::Own( Box::new( v.make_static() )) },
        &Own(ref v) => { Leaf::Own( Box::new( v.make_static() ))  }
    } }


}



impl<'s> Rope<'s> {
/*    fn force_clone(&self) -> Rope<'static> {
        match self {
            &Rope::Nil => { Rope::Nil },
            &Rope::Node(ref l, ref r) => {
                Rope::Node(
                    Box::new( l.shallow_copy() ),
                    Box::new( r.shallow_copy() )
                )
            },
            &Rope::Char(ref c) => { Rope::Char(Cow::Owned(c.clone().into_owned())) }
            &Rope::Val(ref v) => { Rope::Val(Cow::Owned(v.clone().into_owned())) }
        }
    }
*/
    pub fn new() -> Rope<'s> {
        return Rope::Nil
    }



    pub fn make_static(&self) -> Rope<'static> {
        match self {
            &Rope::Nil => { Rope::Nil },
            &Rope::Node(ref l, ref r) => {
                Rope::Node(
                    Box::new(l.make_static()),
                    Box::new(r.make_static())
                )
            },
            &Rope::Leaf(Chr(ref c)) => {
                let owned = Cow::Owned((**c).to_owned());
                Rope::Leaf(Leaf::Chr(owned))
            },
            &Rope::Leaf(Val(ref v)) => { Rope::Leaf( Leaf::Own( Box::new( v.make_static() ) )) },
            &Rope::Leaf(Own(ref v)) => { Rope::Leaf( Leaf::Own( Box::new( v.make_static() ) )) },
        }
    }


    pub fn concat(self, other: Rope<'s>) -> Rope<'s> {
        Rope::Node(Box::new(self), Box::new(other))
    }

    pub fn is_empty(&self) -> bool {
         match self {
            &Rope::Nil => { true }

            &Rope::Node(ref l, ref r) => { l.is_empty() && r.is_empty() }
            &Rope::Leaf(Leaf::Chr(ref c)) => { c.len() == 0 }
            &Rope::Leaf(Leaf::Val(_)) => { false }
            &Rope::Leaf(Leaf::Own(_)) => { false }
        }
    }

    pub fn is_white(&self) -> bool {
        match self {
           &Rope::Nil => { true }
           &Rope::Node(ref l, ref r) => { l.is_white() && r.is_white() }
           &Rope::Leaf(Leaf::Chr(ref c)) =>  { c.chars().all(|x| { x.is_whitespace() }) }
           &Rope::Leaf(Leaf::Val(_)) => { true  }
           &Rope::Leaf(Leaf::Own(_)) => { true }
        }
    }

 //   pub fn walk<T>(

    // use wlak more
    pub fn values(self) -> Vec<Value<'s>> {
        let mut values = vec![];
        self.walk(|leaf| { match leaf {
            &Chr(_) => {},
            &Val(ref v) => { values.push(v.make_static()) },
            &Own(ref v) => { values.push(v.make_static()) }
        } });
        values
    }
    pub fn values_cnt(&self) -> u32 {
        let mut count = 0;
        self.walk(|leaf| { match leaf {
            &Chr(_) => {},
            &Val(ref v) => { count += 1 }
            &Own(ref v) => { count += 1 }
        } });
        count
    }
    fn walk<F : FnMut(&Leaf<'s>)> (&self, mut todo: F) {
        let mut stack : Vec<&Rope<'s>> = vec![
            &self
        ];
        while let Some(top) = stack.pop() { match top {
            &Rope::Nil => { }
            &Rope::Node(ref l, ref r) => {
                stack.push(r);
                stack.push(l);
            }
            &Rope::Leaf(ref l) => { todo(l) }
        } }
    }

    pub fn to_str(&self) -> Option<Cow<'s, str>> {
        let mut has = true;
        let mut string : Cow<'s, str> = Cow::from("");
        self.walk(|v|{
            println!("TRYCONC {:?}", v.to_str());
            match v.to_str() {
                // TODO avoid copies
                Some(&Cow::Borrowed(ref x)) => { string += x.clone(); },
                // for some reason, adding the string below doesn't work
                Some(&Cow::Owned(ref x)) => { string.to_mut().push_str(&x[..]) },
                _ => { has = false }
            }
        });
        if has { Some(string) } else { None }
    }

    pub fn to_leaf(self, lists_allowed: bool) -> Leaf<'s> {
        match self {
            Rope::Leaf(Val(ref l)) => { return Val(l) }
            Rope::Leaf(Own(ref l)) => { return Own(Box::new(l.make_static())) }
            Rope::Leaf(Chr(_))
            | Rope::Nil => { panic!() },
            Rope::Node(_, _) => {
                // TODO think this through a bit more..
                if !self.is_white() {
                    println!("BUILTA {:?}", self.to_str());
                    Leaf::Own(Box::new(Value::Str( self.to_str().unwrap() )))
                } else {
                    match self.values_cnt() {
                        1 => {
                            Leaf::Own(Box::from( self.values().remove(0) ))
                        },
                        _ => {
                            if let Some(s) = self.to_str() {
                                println!("BUILT {:?}", s);
                                Leaf::Own(Box::new(Value::Str( s )))
                            } else {
                                 panic!("Cannot make sense!");
                            }
                        }
                    }
                }
            }
        }
    }
        // may want to make this stuff iterative
    pub fn split_at<'q, F : FnMut(char) -> bool>
        (&mut self, match_val : bool, matcher: &mut F)
        -> Option<Rope<'s>>  {
        println!("SHIELD {:?}", self);
        let mut out : Option<Rope<'s>> = None;
        let mut make_nil = false;
        match self {
            &mut Rope::Nil => { println!("THORAX"); },
            &mut Rope::Node(ref mut left, ref mut right) => {
                match left.split_at(match_val, matcher) {
                    Some(result) => {
                        out = Some(result);
                    }
                    None => {
                        match right.split_at(match_val, matcher) {
                            Some(result) => { 
                                out = Some(
                                    replace(left, Box::new(Rope::Nil))
                                        .concat(result)
                                );
                            },
                            None => {
                            }
                        }
                    }
                }
            },
            &mut Rope::Leaf(Leaf::Val(v)) => {
                if match_val {
                } else {
                    // todo fix this
                    println!("LORAX");
                    out = Some(replace(self, Rope::Nil))
                }
            },
            &mut Rope::Leaf(Leaf::Own(ref v)) => {
                if match_val {
                } else {
                    println!("ATOWN {:?}", v);
                    out = Some(Rope::Leaf(Leaf::Own( Box::new( v.make_static() ))) );
                    make_nil = true;
                }
            },
            &mut Rope::Leaf(Leaf::Chr(ref mut cow)) => {
                if let Some(idx) = cow.find(|x| { matcher(x) }) {
                    let ncow = {
                        let pair = cow.split_at(idx);
                        println!("AT: {:?}", pair);
                    // TODO copying here
                        out = Some(Rope::Leaf(Leaf::Chr(Cow::Owned(pair.0.to_owned()))).make_static());
                        Cow::Owned(pair.1.to_owned())
                    };
                    *cow = ncow;
                }
            },
        };
        if make_nil {
            *self = Rope::Nil
        }
        println!("YIELD {:?} {:?}", out, self);
        out
    }
/*
    pub fn get_str(&self) -> Cow<str> {
        match self {
            &Rope::Nil => { Cow::Borrowed("") },
            &Rope::Chars(Cow::Owned(ref ch)) => { Cow::Owned(ch.clone()) },
            &Rope::Chars(Cow::Borrowed(ch)) => { Cow::Borrowed(ch) },
            &Rope::Val(_) => { panic!("Unexpected value!") },
            &Rope::Node(ref left, ref right) => {
               left.get_str() + right.get_str()
            }
        }
    }
*/
    pub fn get_char(&self) -> Option<char> {
        match self {
            &Rope::Nil => { None },
            &Rope::Node(ref left, ref right) => { left.get_char().or(right.get_char()) },
            &Rope::Leaf(Chr(ref ch)) => { ch.chars().next() }
            &Rope::Leaf(_) => { panic!("Unexpected value!") },
        }
    }

    pub fn split_char(&mut self) -> Option<char> {

        let res = match self {
            &mut Rope::Nil => { None },
            &mut Rope::Node(ref mut left, ref mut right) => {
                left.split_char().or_else(|| { right.split_char() })
            },
            &mut Rope::Leaf(Leaf::Chr(Cow::Owned(ref mut ch))) => {
                if ch.len() > 0 {
                    let rest = ch.split_off(1);
                    let out = ch.remove(0);
                    *ch = rest;
                    Some(out)
                } else {
                    None
                }
            },
            &mut Rope::Leaf(Leaf::Chr(Cow::Borrowed(ref mut ch))) => {
                if ch.len() > 0 {
                    let (first, rest) = ch.split_at(1);
                    *ch = rest;
                    first.chars().next()
                } else {
                    None
                }
            }
            &mut Rope::Leaf(_) => { panic!("Unexpected value!") }, 
        };
        println!("SPLIT OFF {:?} {:?}", res, self);
        res
    }
}





pub use Value::*;
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
        x.walk(|i| {
            i.fmt(f);
        });
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
*/
