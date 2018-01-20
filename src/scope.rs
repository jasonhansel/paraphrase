
use value::*;

use std::borrow::Cow;
use std::rc::Rc;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Command {
    Define, // add otheres, eg. expand
    IfEq,
    User(Vec<String>, ValueClosure), // arg names
    UserHere(Vec<String>, Rope<'static>), // TODO: clone UserHere's into User's
    Immediate(Cow<'static, Value>),
    Expand,
    Rescope
}


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CommandPart {
    Ident(String),
    Param
}
pub use CommandPart::*;

#[derive(Clone, Debug)]
pub struct Scope {
    pub sigil: char,
    pub commands: HashMap<Vec<CommandPart>, Command>
}


pub fn dup_scope(scope : Rc<Scope>) -> Rc<Scope> {
    // does this make any sense?
    let mut stat = (*scope).clone();
    for (_, val) in stat.commands.iter_mut() {
        let mut cmd = None;
        match val {
            &mut Command::UserHere(ref mut arg_names, ref mut list) => {
                cmd = Some(Command::User(
                    arg_names.clone(),
                    ValueClosure(scope.clone(), Box::new(list.clone()))
                ))
            },
            _ => {}
        };
        if let Some(c) = cmd {
            *val = c;
        }
    }
    Rc::new(stat)
}

