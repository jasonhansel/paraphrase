
use value::*;
use expand::*;

use std::rc::Rc;
use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::mem::replace;

pub enum Command {
    Native(Box<for<'s> fn(&Rc<Scope>, Vec<Leaf<'s>>) -> Leaf<'s>>),
    InOther(Rc<Scope>),
    User(Vec<String>, Rope<'static>),
    Immediate(Value<'static>),
}

use Command::*;

impl Debug for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &Native(_) => { write!(f, "[native code]") },
            &Immediate(ref v) => { v.fmt(f) },
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
        for<'s> fn(&Rc<Scope>, Vec<Leaf<'s>>) -> Leaf<'s>
    ) {
        self.commands.insert(parts, Command::Native(Box::new(p)));
    }

    pub fn add_user<'s>(mut this: &mut Rc<Scope>, parts: Vec<CommandPart>,
                        params: Vec<String>,
                        rope: &Rope<'s>) {
        let me = Rc::get_mut(&mut this).unwrap()
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
    for (key, _) in scope.commands.iter() {
        stat.commands.insert(key.clone(), Command::InOther(scope.clone()));
    }
    stat
}

pub fn eval<'c, 'v>(cmd_scope: &'v Rc<Scope>, scope: Rc<Scope>, command: Vec<CommandPart>, args: Vec<Leaf<'v>>) -> Leaf<'v> {
    match cmd_scope.commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope, scope, command, args)
         },
         &Command::Native(ref code) => {
             code(&scope, args )
         },
         &Command::Immediate(ref val) => {
             Leaf::Own( Box::new( val.make_static() ) )
         },
         &Command::User(ref arg_names, ref contents) => {
             // todo handle args
             //clone() scope?
             let mut new_scope = dup_scope(cmd_scope);
             if arg_names.len() != args.len() {
                 panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
             }
             for (name, arg) in arg_names.into_iter().zip( args.into_iter() ) {
                 // should it always take no arguments?
                 // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                 // or a Tagged). coerce sometimes?
                new_scope.commands
                     .insert(vec![Ident(name.to_owned() )],
                     Command::Immediate( arg.as_val().unwrap().make_static() )
                 );
             }
             let new_scope =Rc::new(new_scope);
             let out = new_expand(&new_scope, contents.make_static() );
             println!("OUTP {:?} {:?}", out, contents);
             out.make_static()
         }
     }
}



