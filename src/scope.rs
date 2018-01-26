
use value::*;
use expand::*;

use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::mem::replace;
use std::borrow::Cow;

pub use std::sync::Arc;
type NativeFn = for<'s> fn(Vec<Rope<'static>>) -> EvalResult<'static>;

#[derive(Clone)]
enum Command<'c> {
    Native(NativeFn),
    InOther(Arc<Scope<'c>>),
    User(Vec<String>, Rope<'c>)
}

use self::Command::*;

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

#[derive(Clone)]
pub struct Scope<'c> {
    pub sigil: char,
    commands: HashMap<Vec<CommandPart>, Command<'c>>,
    pub part_done: Option<UnfinishedParse>
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
            commands: HashMap::new(),
            part_done: None
        }
    }
    pub fn part_done(&mut self, part: UnfinishedParse) {
        self.part_done = Some(part);
    }

    pub fn add_native(&mut self, parts: Vec<CommandPart>, p:NativeFn
    ) {
        self.commands.insert(parts, Command::Native(p));
    }

    pub fn add_user(mut this: &mut Scope<'c>, parts: Vec<CommandPart>,
                        params: Vec<String>,
                        rope: Rope<'c>) {
        this.commands
            .insert(parts, Command::User(params, rope));
    }

    pub fn has_command(&self, parts: &[CommandPart]) -> bool {
        self.commands.contains_key(parts)
    }
}


pub fn dup_scope<'s>(scope : &Arc<Scope<'static>>) -> Scope<'static> {
    // does this make any sense?
    // TODO improve perf - nb InOther is more important now since it determines cope
    if let Some(ref up) = scope.part_done {
        if !up.is_empty() {
            panic!("Cannot define in partially-evaluated scopes!");
        }
    }
    let mut stat = Scope { sigil: scope.sigil, commands: HashMap::new(), part_done: None };
    for (key, val) in scope.commands.iter() {
        let other_scope = match val {
            &InOther(ref isc) => { isc.clone() }
            _ => { scope.clone() }
        };
        stat.commands.insert(key.clone(), Command::InOther(other_scope));
    }
    stat
}

#[derive(Clone,Debug)]
pub enum EvalResult<'v> {
    Expand(Arc<Scope<'static>>, Rope<'v>),
    Done(Value<'v>)
}
use EvalResult::*;

pub fn eval<'c, 'v>(cmd_scope: Arc<Scope<'static>>, command: Vec<CommandPart>, args: Vec<Rope<'v>>) -> EvalResult<'v> {
    match cmd_scope.clone().commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope.clone(), command, args)
         },
         &Command::Native(ref code) => {
             code(args.into_iter().map(|mut x| { x.make_static() }).collect())
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
                Scope::add_user(
                    &mut new_scope,
                    vec![Ident(name.to_owned())],
                    vec![],
                    Rope::from_value(arg.coerce()).make_static()
                );
             }
             Expand(Arc::new(new_scope), contents.clone().make_static())
         }
     }
}



