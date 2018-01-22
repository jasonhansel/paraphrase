
use value::*;
use expand::*;

use std::rc::Rc;
use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::mem::replace;

pub enum Command {
    Native(Box<for<'s> fn(Vec<Leaf<'s>>) -> Leaf<'s>>),
    InOther(Rc<Scope>),
    User(Vec<String>, Rope<'static>)
}

use Command::*;

impl Debug for Command {
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

pub struct Scope {
    pub sigil: char,
    commands: HashMap<Vec<CommandPart>, Command>
}

impl Debug for Scope {
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


impl Scope {
    pub fn new(sigil: char) -> Scope {
        Scope {
            sigil: sigil,
            commands: HashMap::new()
        }
    }

    pub fn add_native(&mut self, parts: Vec<CommandPart>, p:
        for<'s> fn(Vec<Leaf<'s>>) -> Leaf<'s>
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


pub fn dup_scope(scope : &Rc<Scope>) -> Scope {
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

pub fn eval<'c, 'v>(cmd_scope: Rc<Scope>, command: Vec<CommandPart>, args: Vec<Leaf<'v>>) -> Leaf<'v> {
    match cmd_scope.commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope.clone(), command, args)
         },
         &Command::Native(ref code) => {
             code(args )
         },
         &Command::User(ref arg_names, ref contents) => {
             // todo handle args
             //clone() scope?
             let mut new_scope = dup_scope(&cmd_scope);
             if arg_names.len() != args.len() {
                 panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
             }
             for (name, arg) in arg_names.into_iter().zip( args.into_iter() ) {
                 // should it always take no arguments?
                 // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                 // or a Tagged). coerce sometimes?
                new_scope.commands
                     .insert(vec![Ident(name.to_owned() )],
                     Command::User( vec![],
                        Rope::Leaf(arg.make_static())
                    )
                 );
             }
             let new_scope = Rc::new(new_scope);
             let out = new_expand(new_scope, contents.make_static() );
             println!("OUTP {:?} {:?}", out, contents);
             out.make_static()
         }
     }
}



