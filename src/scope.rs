
use value::*;
use expand::*;

use std::rc::Rc;
use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::mem::replace;
use std::borrow::Cow;

pub enum Command<'c> {
    Native(Box<for<'s> fn(Vec<Value<'s>>) -> Value<'s>>),
    InOther(Rc<Scope<'c>>),
    User(Vec<String>, Rope<'c>)
}

use Command::*;

impl<'c> Debug for Command<'c> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &Native(_) => { write!(f, "[native code]") },
            &User(ref s, ref v) => { write!(f, "params (")?; s.fmt(f)?; write!(f, ") in "); v.fmt(f) },
            &InOther(_) => { write!(f, "reference to other scope") }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CommandPart {
    Ident(String),
    Param
}
pub use CommandPart::*;

pub struct Scope<'c> {
    pub sigil: char,
    commands: HashMap<Vec<CommandPart>, Command<'c>>
}

impl<'c> Debug for Scope<'c> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let mut first = true;
        write!(f, "[scope @")?;
        for k in self.commands.keys() {
            if first { first = false; } else { write!(f, "|")?; }
            k[0].fmt(f)?;
        }
        write!(f, "]")
    }
}


impl<'c> Scope<'c> {
    pub fn new(sigil: char) -> Scope<'c> {
        Scope {
            sigil: sigil,
            commands: HashMap::new()
        }
    }

    pub fn add_native(&mut self, parts: Vec<CommandPart>, p:
        for<'s> fn(Vec<Value<'s>>) -> Value<'s>
    ) {
        self.commands.insert(parts, Command::Native(Box::new(p)));
    }

    pub fn add_user<'s>(mut this: &mut Rc<Scope>, parts: Vec<CommandPart>,
                        params: Vec<String>,
                        rope: &Rope<'s>) {
        Rc::get_mut(&mut this).unwrap()
            .commands
            .insert(parts, Command::User(params, rope.make_static()));
    }

    pub fn has_command(&self, parts: &[CommandPart]) -> bool {
        self.commands.contains_key(parts)
    }
}


pub fn dup_scope<'s>(scope : &Rc<Scope<'static>>) -> Scope<'static> {
    // does this make any sense?
    // TODO improve perf - nb InOther is more important now since it determines cope
    let mut stat = Scope { sigil: scope.sigil, commands: HashMap::new() };
    for (key, val) in scope.commands.iter() {
        let other_scope = match val {
            &InOther(ref isc) => { isc.clone() }
            _ => { scope.clone() }
        };
        stat.commands.insert(key.clone(), Command::InOther(other_scope));
    }
    stat
}

impl<'s> ValueClosure<'s> {
    pub fn force_clone(&self) -> ValueClosure<'static> {
        match self {
           &ValueClosure(ref sc, ref ro) => { ValueClosure(sc.clone(), Box::new(ro.make_static() )) },
        }
    }
}
impl<'s,'t> Value<'s> {
    fn make_static(&'t self) -> Value<'static> {
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

impl<'s> Leaf<'s> {
    fn make_static(&self) -> Leaf<'static> { match self {
        // TODO avoid this at all costs
        &Leaf::Chr(ref c) => {
            let owned = Cow::Owned(c.clone().into_owned());
            Leaf::Chr(owned)
        },
        &Leaf::Own(ref v) => { Leaf::Own( Box::new( v.make_static() ))  }
    } }
}

impl<'s> Rope<'s> {
    fn make_static(&self) -> Rope<'static> {
        match self {
            &Rope::Nil => { Rope::Nil },
            &Rope::Node(ref l, ref r) => {
                Rope::Node(
                    Box::new(l.make_static()),
                    Box::new(r.make_static())
                )
            },
            &Rope::Leaf(ref l) => { Rope::Leaf(l.make_static()) }
        }
    }
}

pub fn eval<'c, 'v>(cmd_scope: Rc<Scope<'static>>, command: Vec<CommandPart>, args: Vec<Value<'v>>) -> Value<'v> {
    match cmd_scope.commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope.clone(), command, args)
         },
         &Command::Native(ref code) => {
             code(args)
         },
         &Command::User(ref arg_names, ref contents) => {
             // todo handle args
             //clone() scope?
             let mut new_scope = dup_scope(&cmd_scope);
             if arg_names.len() != args.len() {
                 panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
             }
             let mut new_scope = Rc::new(new_scope);
             for (name, arg) in arg_names.into_iter().zip( args.into_iter() ) {
                 // should it always take no arguments?
                 // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                 // or a Tagged). coerce sometimes?
                Scope::add_user(&mut new_scope, vec![Ident(name.to_owned())], vec![], &Rope::Leaf(Leaf::Own(Box::new(arg))));
             }
             let out = new_expand(new_scope, contents.make_static() );
             println!("OUTP {:?} {:?}", out, contents);
             out.make_static()
         }
     }
}



