
use std::fmt;
use std::rc::Rc;
use scope::Scope;
use std::borrow::Cow;
use std::mem::replace;
use expand::*;

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

impl<'s> PartialEq for Value<'s> {
    fn eq(&self, other: &Value<'s>) -> bool {
        match (self, other) {
            (&Str(ref a), &Str(ref b)) => { a == b  },
            (&List(ref a), &List(ref b)) => { a == b }
            (&OwnedList(ref a), &OwnedList(ref b)) => { a == b },
            (&Tagged(ref a1, ref b1), &Tagged(ref a2, ref b2)) => { a1 == a2 && b1 == b2 },
            _ => { false }
        }
    }
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
            &Bubble(ValueClosure(ref sc, ref ro)) => { Bubble(ValueClosure(sc.clone(), Box::new(ro.make_static() ))) },
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


use std::ptr;

impl<'s> Leaf<'s> {
    pub fn bubble(&self, scope: Rc<Scope>) -> Option<Rope<'s>> {
        if let Some(&Value::Bubble(ref closure)) = self.as_val() {
            let &ValueClosure(ref inner_scope, ref contents) = closure;
            if Rc::ptr_eq(inner_scope, &scope) {
                return Some(contents.make_static())
            } else {
                panic!("SAD BUBBLING"); // just a test
            }
        }
        return None
    }

    pub fn to_str(&self) -> Option<&Cow<'s, str>> {
        // TODO: may need to handle Bubbles here
        match self {
            &Leaf::Chr(ref c) => { Some(c) },

            &Leaf::Val(&Value::Str(ref v)) => { Some(v) },
            &Leaf::Own(ref v) => {
                match &**v {
                    &Value::Str(ref v) => { Some(v) },
                    _ => None
                }
            },
            _ => None
        }
    }
    pub fn as_val(&self) -> Option<&Value> {
        match self {
            &Leaf::Chr(_) => { None  },
            &Leaf::Val(ref v) => { Some(v) }
            &Leaf::Own(ref v) => { Some(v) }
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

    pub fn debubble<'t>(&mut self, scope: Rc<Scope>) {
        match self {
            &mut Rope::Leaf(ref mut l) => {
                if let Some(bubble) = l.bubble(scope.clone()) {
                    replace(l, new_expand(scope, bubble));
                }
            },
            &mut Rope::Node(ref mut l, ref mut r) => {
                l.debubble(scope.clone());
                r.debubble(scope);
            },
            &mut Rope::Nil => {}
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

    // use wlak more

    pub fn values_cnt(&self) -> u32 {
        let mut count = 0;
        self.walk(|leaf| { match leaf {
            &Chr(_) => {},
            &Val(ref v) => { count += 1 }
            &Own(ref v) => { count += 1 }
        }; true });
        count
    }
    fn move_walk<F : FnMut(Leaf<'s>) -> bool> (self, mut todo: F) {
        let mut stack : Vec<Rope<'s>> = vec![
            self
        ];
        while let Some(top) = stack.pop() { match top {
            Rope::Nil => { }
            Rope::Node(l, r) => {
                stack.push(*r);
                stack.push(*l);
            }
            Rope::Leaf(l) => { if !todo(l) { return } }
        } }
    }
    fn walk<F : FnMut(&Leaf<'s>) -> bool> (&self, mut todo: F) {
        let mut stack : Vec<&Rope<'s>> = vec![
            &self
        ];
        while let Some(top) = stack.pop() { match top {
            &Rope::Nil => { }
            &Rope::Node(ref l, ref r) => {
                stack.push(r);
                stack.push(l);
            }
            &Rope::Leaf(ref l) => { if !todo(l) { return } }
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
            true
        });
        if has { Some(string) } else { None }
    }

    pub fn to_leaf(mut self, scope: Rc<Scope>) -> Leaf<'s> {
        self.debubble(scope);
        self.get_leaf()
    }

    pub fn get_leaf(self) -> Leaf<'s> {
        match self {
            Rope::Nil => { panic!() }
           Rope::Leaf(Val(ref l)) => { return Val(l) }
            Rope::Leaf(Own(ref l)) => { return Own(Box::new(l.make_static())) }
            Rope::Leaf(Chr(c)) => { return Own(Box::new( Value::Str( Cow::Owned( c.clone().into_owned() ) ))) }
            Rope::Node(_, _) => {
                // TODO think this through a bit more..
                if !self.is_white() {
                    Leaf::Own(Box::new(Value::Str( self.to_str().unwrap() )))
                } else {
                    match self.values_cnt() {
                        1 => {
                            let mut val = None;
                            self.move_walk(|leaf| { match leaf {
                                Chr(_) => { true },
                                Val(v) => { val = Some(Leaf::Val(v)); false },
                                Own(v) => { val = Some(Leaf::Own(v)); false }
                            } });
                            val.unwrap()
                        },
                        _ => {
                            if let Some(s) = self.to_str() {
                                println!("BUILT {:?}", s);
                                Leaf::Own(Box::new(Value::Str( s )))
                            } else {
                                 panic!("Cannot make sense of: {:?}", self);
                            }
                        }
                    }
                }
            }
        }
    }
        // may want to make this stuff iterative
    pub fn split_at<F : FnMut(char) -> bool>(self, match_val : bool, matcher: &mut F)
        -> (Rope<'s>, Option<Rope<'s>>)  {
        println!("SHIELD {:?}", self);
        let mut out : Option<Rope<'s>> = None;
        match self {
            Rope::Nil => { (Rope::Nil, None) },
             Rope::Node(left, right) => {
                match left.split_at(match_val, matcher) {
                    (left_rest, Some(result)) => {
                        (Rope::Node(Box::new(left_rest), right), Some(result))
                    },
                    (left_rest, None) => {
                        match right.split_at(match_val, matcher) {
                            (rest, Some(result)) => {
                                (rest, Some( left_rest.concat(result) ))
                            },
                            (rest, None) => {
                                (Rope::Node(Box::new(left_rest), Box::new(rest)), None)
                            }
                        }
                    }
                }
            },
             Rope::Leaf(Leaf::Val(v)) => {
                if match_val {
                    (self, None)
                } else {
                    // todo fix this
                    (Rope::Nil, Some(self))
                }
            },
             Rope::Leaf(Leaf::Own(v)) => {
                if match_val {
                    (Rope::Leaf(Leaf::Own(v)), None)
                } else {
                    (Rope::Nil, Some(Rope::Leaf(Leaf::Own( Box::new( v.make_static() ))) ))
                }
            },
             Rope::Leaf(Leaf::Chr(Cow::Borrowed(cow))) => {
                if let Some(idx) = cow.find(|x| { matcher(x) }) {
                    
                    let pair = (*cow).split_at(idx);
                    // TODO copying here
                    (Rope::Leaf(Leaf::Chr(Cow::Borrowed(pair.1))),
                        Some(Rope::Leaf(Leaf::Chr(Cow::Borrowed(pair.0)))) )
                } else {
                    (self, None)
                }
            },
             Rope::Leaf(Leaf::Chr(Cow::Owned(cow))) => {
                if let Some(idx) = cow.find(|x| { matcher(x) }) {
                    let pair = (*cow).split_at(idx);
                    // TODO copying here
                    (Rope::Leaf(Leaf::Chr(Cow::Owned( pair.1.to_owned() ))),
                        Some(Rope::Leaf(Leaf::Chr(Cow::Owned( pair.0.to_owned() )))) )
                } else {
                    (Rope::Leaf(Leaf::Chr(Cow::Owned(cow))), None)
                }
            },

        }
    }

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
        scope.fmt(f)?;
        write!(f, "CODE<");
        x.walk(|i| {
            i.fmt(f);
            true
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
