
use std::fmt;
use std::rc::Rc;

use scope::Scope;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tag(u64);


// should closures "know" about their parameters?
#[derive(Clone)]
pub struct ValueClosure(pub Rc<Scope>, pub Vec<Atom>);

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
pub enum Atom {
    Char(char),
    Val(Value)
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
        for item in x {
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
