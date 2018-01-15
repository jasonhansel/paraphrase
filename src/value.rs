
use std::fmt;
use std::rc::Rc;

use scope::Scope;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    Num
}

#[derive(Clone, PartialEq, Eq)]
pub struct ValueList(pub Vec<Value>);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ValueChar(pub char);

// should closures "know" about their parameters?
#[derive(Clone)]
pub struct ValueClosure(pub Rc<Scope>, pub Vec<Value>);


#[derive(Clone)]
pub enum Value {
    Char(ValueChar),
    List(ValueList),
    Tagged(Tag,ValueList),
    Closure(ValueClosure)
}

pub use Value::*;

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
            (&Char(a), &Char(b)) => { a == b },
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
