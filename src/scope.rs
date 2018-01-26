
// Tools for managing scopes and commands.


use value::*;
use expand::*;
use serde_json::Value as JValue;

use std::collections::HashMap;
use std::fmt::{Debug,Formatter,Result};
use std::sync::atomic::{AtomicUsize,Ordering};


static latest_tag : AtomicUsize = AtomicUsize::new(0);

pub use std::sync::Arc;
pub type NativeFn = fn(Vec<Rope>) -> EvalResult;

#[derive(Clone,Debug)]
pub struct ParamInfo {
    pub kind: ParamKind,
    pub name: String
}

#[derive(Clone)]
enum Command {
    Native(NativeFn),
    InOther(Arc<Scope>),
    User(Vec<ParamInfo>, Tag, Rope),
    Tagger(Tag)
}

use self::Command::*;

impl Debug for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &Native(_) => { write!(f, "[native code]") },
            &Tagger(t) => { write!(f, "[tagger {:?}]", t.0) },
            &User(ref s, _, ref v) => { write!(f, "params (")?; s.fmt(f)?; write!(f, ") in "); v.fmt(f) },
            &InOther(_) => { write!(f, "reference to other scope") }
        }
    }
}

#[derive(Copy,Clone,Debug)]
pub enum ParamKind {
    Any,
    Closure,
    List,
    Str,
    Tag(Tag)
}

use ParamKind as P;
use Value as V;

impl ParamKind {
    fn match_rope(&self, rope: Rope) -> Option<Value> {
        let value = rope.coerce();
        match (self, value) {
            (&P::Any, v) => { Some(v) },
            (&P::Closure, V::Closure(v)) => { Some(V::Closure(v)) }
            (&P::List, V::List(v)) => { Some(V::List(v)) }
            (&P::Str, V::Str(v)) => { Some(V::Str(v)) }
            (&P::Tag(tag_a), V::Tagged(tag_b, ival)) => { 
                if tag_a == tag_b { Some(V::Tagged(tag_b, ival)) }
                else { None }
            },
            _ => { None }
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
pub struct Scope {
    pub sigil: char,
    commands: HashMap<Vec<CommandPart>, Command>,
    pub part_done: Option<UnfinishedParse>
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

    pub fn add_user(&mut self, parts: Vec<CommandPart>,
                        params: Vec<ParamInfo>,
                        rope: Rope) {
        let tag = Tag(latest_tag.fetch_add(1, Ordering::SeqCst));
        self.commands
            .insert(parts, Command::User(params, tag, rope));
    }
    pub fn add_tag(&mut self, tag: Tag) {
        self.commands.insert(vec![Ident("tag".to_owned()), Param], Command::Tagger(tag));
    }
    pub fn has_command(&self, parts: &[CommandPart]) -> bool {
        self.commands.contains_key(parts)
    }


    pub fn add_json(&mut self, json: JValue) {
        match json {
            JValue::Object(map) => {
                for (k, v) in map {
                    self.add_user(vec![ Ident(k) ], vec![],
                        Rope::from_value( Value::from(v) )
                    )
                }
            },
            _ => panic!()
        }
    }

    pub fn get_tag(&self, ident: &str) -> Option<Tag> { 
        let mut parts = vec![ Ident(ident.to_owned()) ];
        // TODO: make this less inefficient
        while !self.has_command(&parts[..]) && parts.len() < 20 {
            parts.push(Param);
        }
        match self.commands.get(&parts[..]) {
            Some(&User(_, tag, _)) => Some(tag),
            Some(&InOther(ref s)) => { s.get_tag(ident) },
            _ => None
        }
    }
}


pub fn dup_scope(scope : &Arc<Scope>) -> Scope {
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
pub enum EvalResult {
    Expand(Arc<Scope>, Rope),
    Done(Value)
}
use EvalResult::*;



pub fn eval<'c, 'v>(cmd_scope: Arc<Scope>, command: Vec<CommandPart>, mut args: Vec<Rope>) -> EvalResult {
    match cmd_scope.clone().commands.get(&command).unwrap() {
         &Command::InOther(ref other_scope) => {
            eval( other_scope.clone(), command, args)
         },
         &Command::Native(ref code) => {
             code(args.into_iter().map(|mut x| { x }).collect())
         }, 
         &Command::Tagger(tag) => {
             let val = args.into_iter().next().unwrap().coerce();
             Done(Value::Tagged(tag, Box::new(val)))
         },
         &Command::User(ref arg_names, tag, ref contents) => {
             // todo handle args
             //clone() scope?
             let mut new_scope = dup_scope(&cmd_scope);
             if arg_names.len() != args.len() {
                 panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
             }
             for &ParamInfo{ref kind,ref name} in arg_names.into_iter().rev() {
                 // should it always take no arguments?
                 // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                 // or a Tagged). coerce sometimes?
                match args.pop().map(|x| { kind.match_rope(x) }) {
                    Some(Some(value)) => {
                        Scope::add_user(
                            &mut new_scope,
                            vec![Ident(name.to_owned())],
                            vec![],
                            Rope::from_value(value)
                        );
                    },
                    Some(None) => {
                        println!("Error: expected {:?} in {:?}", kind, command);
                        panic!("Error: expected {:?} in {:?}", kind, command);
                    }
                    None => {
                        panic!("Error: expected {:?} in {:?}, but no argument was provided", kind, command);
                    }
                }
             }
             Scope::add_tag(&mut new_scope, tag);
             Expand(Arc::new(new_scope), contents.clone())
         }
     }
}



