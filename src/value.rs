
use std::fmt;
use scope::Scope;
use std::borrow::Cow;
use std::collections::LinkedList;
use std::iter;
use std::ops::Range;
use std::sync::Arc;
use std::ops::Add;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tag(u64);


// should closures "know" about their parameters?
#[derive(Clone)]
pub struct ValueClosure(pub Arc<Scope>, pub Box<Rope>);

impl ValueClosure {
    fn from(scope: Arc<Scope>, rope: Rope) -> ValueClosure {
        ValueClosure(scope, Box::new(rope))
    }
}

#[derive(Clone)]
pub struct ArcSlice {
    string: Arc<String>,
    range: Range<usize>
}
impl fmt::Debug for ArcSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_str().fmt(f)
    }
}



impl ArcSlice {
    pub fn from_string(s: String) -> ArcSlice {
        return ArcSlice {
            range: (0..(s.len())),
            string: (Arc::new(s))
        }
    }

    pub fn to_str(&self) -> &str {
        return &self.string[self.range.clone()];
    }
    
   pub fn into_string(self) -> String {
        let range = self.range.clone();
//        let res = Arc::try_unwrap(self.string.into_owned())
//            .map(|mut x| { x.split_off(range.end); x.split_off(range.start)   })
        (&self.string[range.clone()]).to_owned()
    }

    fn make_static(&self) -> ArcSlice {
        return ArcSlice {
            string: self.string.clone(),
            range: self.range.clone()
        }
    }

    fn split_first(&mut self) -> Option<char> {
        println!("SPLITTING");
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

    fn split_at<'t>(mut self, idx: usize) -> (ArcSlice, ArcSlice) {
        println!("SPLITTING B!");
        let left = ArcSlice { string: self.string.clone(), range: Range { start: self.range.start, end: self.range.start+idx } };
        self.range.start += idx;
        (left, self)
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
            _ => { panic!() }
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
    data: LinkedList<Leaf>
}

impl ValueClosure {
    pub fn force_clone(&mut self) -> ValueClosure {
        match self {
           &mut ValueClosure(ref sc, ref mut ro) => { ValueClosure(sc.clone(), Box::new(ro.make_static() )) },
        }
    }
}

impl<'s,'t> Value {
    pub fn make_static(&mut self) -> Value {
        match self {
            // FIXME: avoid using this (or clone)
            &mut Str(ref mut s) => { Str(s.make_static()) },
            &mut List(ref mut l) => { List(l.into_iter().map(|x| { x.make_static() }).collect()) },
            &mut Tagged(ref t, ref mut v) => { Tagged(*t, Box::new(v.make_static())) },
            &mut Closure(ref mut c) => { Closure(c.force_clone()) },
        }
    }
}

impl Leaf {
fn make_static(&mut self) -> Leaf { match self {
    // TODO avoid this at all costs
    &mut Leaf::Chr(ref mut c) => {
        Leaf::Chr(c.make_static())
    },
    &mut Leaf::Own(ref mut v) => { Leaf::Own( v.make_static() )  }
} }

}

impl Rope {
    pub fn make_static(&mut self) -> Rope {
        let mut new_rope = Rope::new();
        for item in self.data.iter_mut() {
            new_rope.data.push_back( item.make_static() );
        }
        new_rope
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

unsafe impl Sync for Rope {}
unsafe impl Send for Rope {}
unsafe impl Send for Value {}
unsafe impl Send for ArcSlice {}

impl Rope {
    pub fn new() -> Rope {
        return Rope {
            data: LinkedList::new()
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
        return Rope {
            data: {
                let mut l = LinkedList::new();
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
        let mut string : ArcSlice = ArcSlice {
            string: (Arc::new("".to_owned())),
            range: 0..0
        };
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
        for val in self.data.into_iter() { match val {
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
        // may want to make this stuff iterative
    pub fn split_at<'r,F : FnMut(char) -> bool>(&'r mut self, match_val : bool, match_eof: bool, matcher: &mut F)
        ->  Option<Rope>  {
        // TODO can optimize the below. would vecs be faster than linked lists?
        let mut prefix = Rope { data: LinkedList::new() };
        while !self.data.is_empty() {
            let front = self.data.pop_back().unwrap();
            match front {
                Leaf::Own(_) => {
                    if match_val {
                        prefix.data.push_front(front);
                    } else {
                        self.data.push_back(front);
                        return Some(prefix);
                    }
                },
                Leaf::Chr(mut slice) => {
                    if let Some(idx) = slice.to_str().find(|x| { matcher(x) }) {
                        if idx > 0 {
                            let (start, rest) = slice.split_at(idx);
                            prefix.data.push_front(Leaf::Chr(start));
                            self.data.push_back(Leaf::Chr(rest));
                        } else {
                            self.data.push_back(Leaf::Chr(slice));
                        }
                        return Some(prefix)
                    } else {
                        prefix.data.push_front(Leaf::Chr(slice));
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
        match self.data.back()? {
            &Leaf::Own(_) => { None },
            &Leaf::Chr(ref ch) => {
                ch.to_str().chars().next()
            }
        }
    }

    pub fn split_char(&mut self) -> Option<char> {
        println!("SPLIT CH");
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

