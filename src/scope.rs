
use value::*;

use std::rc::Rc;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Command {
    Define, // add otheres, eg. expand
    IfEq,
    User(Vec<String>, ValueClosure), // arg names
    UserHere(Vec<String>, Vec<Atom>), // TODO: clone UserHere's into User's
    Immediate(Value),
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

pub fn dup_scope(scope : Rc<Scope>) -> Scope {
    let fixed_commands = scope.commands.iter()
        .map(|(key, val)| {
            (key.clone(), match val {
                // avoid circular refs? cloning a lot, also...
                &Command::UserHere(ref arg_names, ref list) => Command::User(arg_names.clone(), ValueClosure(scope.clone(), list.clone())),
                x => x.clone()
            })
        })
        .collect::<HashMap<Vec<CommandPart>,Command>>();
    Scope { sigil: scope.sigil.clone(), commands: fixed_commands }
}

