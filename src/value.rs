
use std::fmt;
use std::rc::Rc;
use scope::Scope;
use std::borrow::Cow;
use std::mem::replace;
use expand::*;
use std::collections::LinkedList;
use std::iter;
use std::ops::Range;
use std::sync::Arc;
use std::str::Chars;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tag(u64);


// should closures "know" about their parameters?
pub struct ValueClosure<'s>(pub Arc<Scope<'static>>, pub Box<Rope<'s>>);

impl<'s> ValueClosure<'s> {
    fn from(scope: Arc<Scope<'static>>, rope: Rope<'s>) -> ValueClosure<'s> {
        ValueClosure(scope, Box::new(rope))
    }
}

#[derive(Debug)]
pub struct ArcSlice<'s> {
    string: Cow<'s, Arc<String>>,
    range: Range<usize>
}

impl<'s> ArcSlice<'s> {
    pub fn to_str(&'s self) -> &'s str {
        return &self.string[self.range];
    }
    fn make_static(self) -> ArcSlice<'static> {
        return ArcSlice {
            string: Cow::Owned(self.string.into_owned()),
            range: self.range
        }
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
    fn chars(&'s self) -> Chars<'s> {
        self.to_str().chars()
    }
    fn len(&self) -> usize {
        self.range.len()
    }
    fn concat(self, other: ArcSlice<'s>) -> ArcSlice<'s> {
        if self.len() == 0 {
            self
        } else if other.len() == 0 {
            other
        } else if Arc::strong_count(self.string.as_ref()) == 1 { // should be thread-safe in this case
            self.string.push_str(other.to_str());
            self
        } else {
            let mut s = self.to_str().to_owned();
            s.push_str(other.to_str());
            self.string = Cow::Owned(Arc::new(s));
            self
        }
    }
    fn split_at(&'s self, idx: usize) -> (ArcSlice<'s>, ArcSlice<'s>) {
        let left = ArcSlice { string: self.string, range: Range { start: self.range.start, end: self.range.start+idx } };
        let right = ArcSlice { string: self.string, range: Range { start: self.range.start + idx, end: self.range.end } };
        (left, right)
    }
}



// A Value is something that can be used as a parameter to a macro,
// or as a return value.
#[derive(Debug)]
pub enum Value<'s> {
    Str(ArcSlice<'s>),
    List(Vec<&'s Value<'s>>),
    OwnedList(Vec<Value<'s>>),
    Tagged(Tag,Box<Value<'s>>),
    Closure(ValueClosure<'s>),
    Bubble(ValueClosure<'s>) // <- gets auto-expanded when it reaches its original scope
}
use Value::*;

impl<'s> PartialEq for Value<'s> {
    fn eq(&self, other: &Value<'s>) -> bool {
        match (self, other) {
            (&Str(ref a), &Str(ref b)) => { a.to_str() == b.to_str() },
            (&List(ref a), &List(ref b)) => { a == b }
            (&OwnedList(ref a), &OwnedList(ref b)) => { a == b },
            (&Tagged(ref a1, ref b1), &Tagged(ref a2, ref b2)) => { a1 == a2 && b1 == b2 },
            _ => { false }
        }
    }
}



/* UNBALANCED, BORROWED rope data structure
 * Slightly inspired by: https://github.com/mthadley/rupe/blob/master/src/lib.rs
 * (Not quite a rope at the moment) */

#[derive(Clone,Debug)]
enum Leaf<'s> {
    Own(Value<'s>),
    Chr(ArcSlice<'s>) // from/to
}

use self::Leaf::*;

#[derive(Clone,Debug)]
pub struct Rope<'s> {
    data: LinkedList<Leaf<'s>>
}

impl<'s> ValueClosure<'s> {
    pub fn force_clone(self) -> ValueClosure<'static> {
        match self {
           ValueClosure(sc, ro) => { ValueClosure(sc.clone(), Box::new(ro.make_static() )) },
        }
    }
}

impl<'s,'t> Value<'s> {
    pub fn make_static(self) -> Value<'static> {
        match self {
            // FIXME: Cow::Owned will cause excessive copying later
            Str(s) => { Str(s.make_static()) },
            List(l) => { panic!() } // FIXME OwnedList(l.into_iter().map(|x| { x.make_static() }).collect()) },
            OwnedList(l) => { OwnedList(l.into_iter().map(|x| { x.make_static() }).collect()) },
            Tagged(t, v) => { Tagged(t, Box::new(v.make_static())) },
            Closure(c) => { Closure(c.force_clone()) },
            Bubble(c) => { Bubble(c.force_clone()) },
        }
    }
/*
// TODO allow multipart macros again?
    fn dupe(&self) -> Value<'s> {
        match self {
            // FIXME: Cow::Owned will cause excessive copying later
            &Str(ref s) => { Str(Cow::Owned(s.clone().into_owned())) },
            &List(ref l) => { panic!() } // FIXME OwnedList(l.into_iter().map(|x| { x.make_static() }).collect()) },
            &OwnedList(ref l) => { OwnedList(l.iter().map(|x| { x.dupe() }).collect()) },
            &Tagged(ref t, ref v) => { Tagged(*t, Box::new(v.dupe())) },
            &Closure(ref c) => { Closure(c.force_dupe()) },
            &Bubble(ref c) => { Bubble(c.force_dupe()) },
        }
    }
*/
}

impl<'s> Leaf<'s> {
fn make_static(self) -> Leaf<'static> { match self {
    // TODO avoid this at all costs
    Leaf::Chr(c) => {
        Leaf::Chr(c.make_static())
    },
    Leaf::Own(v) => { Leaf::Own( v.make_static() )  }
} }
/*
    fn dupe(&'s self) -> Leaf<'s> { match self {
        // TODO avoid this at all costs
        &Leaf::Chr(Cow::Borrowed(ref c)) => {
            Leaf::Chr(Cow::Borrowed( &**c ))
        },
        &Leaf::Chr(Cow::Owned(ref c)) => {
            Leaf::Chr(Cow::Owned(c.clone()))
        },
        &Leaf::Own(ref v) => { Leaf::Own( v.dupe() )  }
    } }
*/
}

impl<'s> Rope<'s> {
    pub fn make_static(self) -> Rope<'static> {
        let mut new_rope = Rope::new();
        for item in self.data.into_iter() {
            new_rope.data.push_back( item.make_static() );
        }
        new_rope
    }
    /*
    pub fn dupe(&'s self) -> Rope<'s> {
        let mut new_rope = Rope::new();
        for item in self.data.iter() {
            new_rope.data.push_back( item.dupe() );
        }
        new_rope
    }
    */
}



use std::ptr;

impl<'s> Value<'s> {
    pub fn bubble(&self, scope: Arc<Scope>) -> Option<&Rope<'s>> {
            if let &Value::Bubble(ref closure) = self {
                let &ValueClosure(ref inner_scope, ref contents) = closure;
                if Arc::ptr_eq(&inner_scope, &scope) {
                    return Some(&**contents)
                } else {
                    panic!("SAD BUBBLING"); // just a test
                }
            }
        return None
    }
    pub fn bubble_move(self, scope: Arc<Scope>) -> Option<Rope<'s>> {
            if let Value::Bubble(closure) = self {
                let ValueClosure(inner_scope, contents) = closure;
                if Arc::ptr_eq(&inner_scope, &scope) {
                    return Some(*contents)
                } else {
                    panic!("SAD BUBBLING"); // just a test
                }
            }
        return None
    }

}
impl<'s> Leaf<'s> {

    pub fn to_str(self) -> Option<ArcSlice<'s>> {
        // TODO: may need to handle Bubbles here
        match self {
            Leaf::Chr(c) => { Some(c) },
            Leaf::Own(Value::Str(v)) => { Some(v) },
            _ => { None }
        }
    }
    pub fn as_val(self) -> Option<Value<'s>> {
        match self {
            Leaf::Chr(_) => { None  },
            Leaf::Own(v) => { Some(v) }
        }
    }






}



impl<'s> Rope<'s> {
    pub fn new() -> Rope<'s> {
        return Rope {
            data: LinkedList::new()
        }
    }

    pub fn from_value(value: Value<'s>) -> Rope<'s> {
        Rope {
            data: iter::once(Leaf::Own(value)).collect()
        }
    }
    pub fn from_slice(value: ArcSlice<'s>) -> Rope<'s> {
        Rope {
            data: iter::once(Leaf::Chr(value)).collect()
        }
    }


    pub fn concat(mut self, mut other: Rope<'s>) -> Rope<'s> {
        return Rope {
            data: {
                let mut l = LinkedList::new();
                l.append(&mut self.data);
                l.append(&mut other.data);
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

    fn should_be_bubble_concat(&self, scope: Arc<Scope>) -> bool {
        let mut count = 0;
        let mut nothing_else = true;
        let mut result = Rope::new();
        for leaf in self.data.iter() {
            match leaf {
                &Leaf::Chr(ref c) => {
                    if c.to_str().chars().any(|x| { !x.is_whitespace() }) { nothing_else = false; }
                },
                &Own(Bubble(ValueClosure(ref inner_scope, ref contents))) => {
                    if Arc::ptr_eq(&inner_scope, &scope) {
                        count += 1;
                    } 
                },
                _ => { nothing_else = false; }
            }
        }
        (!nothing_else && count == 1) || (count > 1)
    }

    fn to_bubble_rope(mut self) -> Rope<'s> {
        let mut new_rope = Rope::new();
        new_rope.data = self.data.into_iter().flat_map(|leaf| {
            match leaf {
                Own(Bubble(ValueClosure(inner_scope, contents))) => {
                    contents.data
                },
                leaf => {
                    let mut l = LinkedList::new();
                    l.push_back(leaf);
                    l
                }
            }
        }).collect();
        new_rope
    }

    fn should_be_string(&self) -> bool {
        let mut count = 0;
        for leaf in self.data.iter() {
            match leaf {
                &Leaf::Chr(ref c) => { if c.to_str().chars().any(|x| { !x.is_whitespace() }) { return true } }
                &Own(_) => { count += 1 }
            }
        }
        return (count != 1)
    }

    pub fn to_str(self) -> Option<ArcSlice<'s>> {
        let mut has = true;
        let mut string : ArcSlice = ArcSlice {
            string: Cow::Owned(Arc::new("".to_owned())),
            range: 0..1
        };
        for v in self.data.iter() {
            match v.to_str() {
                // TODO avoid copies
                Some(x) => { string = string.concat(x); },
                // for some reason, adding the string below doesn't work
                None => { return None }
            }
        }
        Some(string)
    }

    pub fn coerce_bubble(mut self, scope: Arc<Scope<'static>>) -> Value<'s> { 
        if self.should_be_bubble_concat(scope.clone()) {
            panic!("BUBBLE!");
            /* return new_expand(scope.clone(), self.to_bubble_rope()); */
        } else if self.should_be_string() {
            Value::Str( self.to_str().unwrap() )
        } else {
            for val in self.data.into_iter() { match val {
                Chr(_) => { },
                Own(value) => {
                    return if let Bubble(closure) = value {
                        let ValueClosure(inner_scope, contents) = closure;
                        if Arc::ptr_eq(&inner_scope, &scope) {
                            panic!("BUBBLE");
                            /* new_expand(scope.clone(), *contents ) */
                        } else {
                            Bubble(ValueClosure(inner_scope, contents))
                        }
                    } else {
                        value
                    }
                }
            } }
            panic!("Failure");
        }
    }

    pub fn coerce(self) -> Value<'s> {
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
        // may want to make this stuff iterative
    pub fn split_at<F : FnMut(char) -> bool>(mut self, match_val : bool, match_eof: bool, matcher: &mut F)
        -> (Rope<'s>, Option<Rope<'s>>)  {
        // TODO can optimize the below. would vecs be faster than linked lists?
        let mut prefix = Rope { data: LinkedList::new() };
        while !self.data.is_empty() {
            let mut done = false;
            let mut process = None;
            match self.data.front_mut().unwrap() {
                &mut Leaf::Own(ref mut v) => {
                    if match_val {
                    } else {
                        done = true;
                    }
                },
                &mut Leaf::Chr(ref mut slice) => {
                    if let Some(idx) = slice.to_str().find(|x| { matcher(x) }) {
                        if idx > 0 {
                            let pair = slice.split_at(idx);
                            prefix.data.push_back(Leaf::Chr(pair.0));
                            *slice = pair.1;
                        }
                        done = true;
                    } 
                }

            }

            if done {
                return (self, Some(prefix));
            } else {
                prefix.data.push_back(self.data.pop_front().unwrap());
            }
        }
        if match_eof {
            return (self  ,Some(prefix))
        } else {
            return (prefix, None)
        }
    }

    pub fn get_char(&self) -> Option<char> {
        for leaf in self.data.iter() {
            match leaf {
                &Leaf::Own(_) => { panic!("Unexpected value") },
                &Leaf::Chr(ref ch) => {
                    if let Some(c) = ch.chars().next() {
                        return Some(c)
                    }
                }
            }
        }
        None
    }

    pub fn split_char(&mut self) -> Option<char> {
        match self.data.front_mut() {
            Some(&mut Leaf::Chr(ref mut slice)) => { slice.split_first() }
            _ => { None }
        }
    }

}

impl<'s> fmt::Debug for ValueClosure<'s> {
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

