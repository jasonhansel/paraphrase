
use value::*;
use expand::*;

use std::borrow::Cow;
use std::rc::Rc;
use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::ptr;

pub enum Command {
    Native(Box<for<'s> fn(&Rc<Scope>, Vec<Leaf<'s>>) -> Leaf<'s>>),
    InOther(Rc<Scope>),
    User(Vec<String>, ValueClosure),
    Immediate(Value<'static>),
}

use Command::*;

impl Debug for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &Native(_) => { write!(f, "[native code]") },
            &Immediate(ref v) => { v.fmt(f) },
            &User(ref s, ref v) => { write!(f, "params (")?; s.fmt(f)?; write!(f, ") in "); v.fmt(f) },
            &InOther(ref s) => { write!(f, "reference to other scope") }
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
            k.fmt(f)?;
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

    pub fn add_user(&mut self, parts: Vec<CommandPart>, params: Vec<String>, closure: &ValueClosure) {
        self.commands.insert(parts, Command::User(params, closure.force_clone()));
    }

    pub fn has_command(&self, parts: &[CommandPart]) -> bool {
        self.commands.contains_key(parts)
    }
}

pub fn dup_scope(scope : &Rc<Scope>) -> Rc<Scope> {
    // does this make any sense?
    // TODO improve perf
    let mut stat = Scope { sigil: scope.sigil, commands: HashMap::new() };
    for (key, _) in scope.commands.iter() {
        stat.commands.insert(key.clone(), Command::InOther(scope.clone()));
    }
    Rc::new(stat)
}

pub fn eval<'c, 'v>(cmd_scope: &'v Rc<Scope>, scope: Rc<Scope>, command: Vec<CommandPart>, args: Vec<Leaf<'v>>) -> Leaf<'v> {
    match cmd_scope.commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope, scope, command, args)
         },
         &Command::Native(ref code) => {
             code(&scope, args)
         },
         &Command::Immediate(ref val) => {
             Leaf::Own( Box::new( val.make_static() ) )
         },
         &Command::User(ref arg_names, ValueClosure(ref inner_scope, ref contents)) => {
             // todo handle args
             //clone() scope?
             let mut new_scope = dup_scope(inner_scope);
             if arg_names.len() != args.len() {
                 panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
             }
             for (name, arg) in arg_names.into_iter().zip( args.into_iter() ) {
                 // should it always take no arguments?
                 // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                 // or a Tagged). coerce sometimes?
                 Rc::get_mut(&mut new_scope)
                     .unwrap()
                     .commands
                     .insert(vec![Ident(name.to_owned() )],
                     Command::Immediate( arg.to_val().make_static() )
                 );
             }
             let out = new_expand(&new_scope, contents.make_static() );
             println!("OUTP {:?} {:?}", out, contents);
             out.make_static()
         }
     }
}



