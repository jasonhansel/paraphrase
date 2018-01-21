
use value::*;

use std::borrow::Cow;
use std::rc::Rc;
use std::collections::HashMap;

#[derive(Debug)]
pub enum Command {
    Define, // add otheres, eg. expand
    IfEq,
    InOther(Rc<Scope>),
    User(Vec<String>, ValueClosure), // arg names
    Immediate(Value<'static>),
    Expand,
    Rescope
}


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CommandPart {
    Ident(String),
    Param
}
pub use CommandPart::*;

#[derive(Debug)]
pub struct Scope {
    pub sigil: char,
    pub commands: HashMap<Vec<CommandPart>, Command>
}


pub fn dup_scope(scope : Rc<Scope>) -> Rc<Scope> {
    // does this make any sense?
    // TODO improve perf
    let mut stat = Scope { sigil: scope.sigil, commands: HashMap::new() };
    for (key, _) in scope.commands.iter() {
        stat.commands.insert(key.clone(), Command::InOther(scope.clone()));
    }
    Rc::new(stat)
}

