
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly





// TODO: some tests are failing (removing first character spuriously)
// TODO: Back to copying
//
//
//

// CURRENT BUGS:
// - issues with if_Eq and recu'rsive defs

// (allow mutual recursion with a special 'define'? add standard library, improve testability)

// for type system below:
// - make sure that we can turn a ;-param into an auto-expanding list

// TYPES - to be improved, thought through

// Argument types:
// (....) <- list<str|list<other>> gets coerced (in various ways, can preserve all) to: string, closure, list, tagged
//           - strip whitespace (unless the whole thing is whitespace); turn other unwrapped tokens
//           into strings...
// {....} <- closure
// ;....  <- closure (not necessarily expandable)

// Return types:
// ..... -> list<expchar|other> gets coerced (in various ways, can preserve all) to: string, list<Type>, tagged<Tag>,
// closure (auto expanded?)
// (....;   -> the above, or an "unexpandable" closure which will, if this is a ;-command, get used
// instead of the original text. in fact, for ;-commands in ()-context, retval *must* be such a
// closure



// TODO: auto expand Exclosures when they reach the scope that they contain (and are returned from
// a ;-command).
// TODO: test if_eq, handle recursive defs.

// TODO fix bugs in test - is newline behavior desirable?
// bigger issue: 'new world order' duplicated

// NOTE: expanding from the right  === expanding greedily

mod value;
mod scope;
mod base;
mod expand;

use scope::*;
use std::borrow::Cow;
use std::borrow::Borrow;
use std::ops::Range;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use std::rc::Rc;
use std::iter::Iterator;
use value::*;
use base::default_scope;
use expand::*;

// TODO cloneless


// nb also write stdlib.

fn read_file<'s>(mut string: &'s mut String, path: &str) -> Result<Rope<'s>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    file.read_to_string(string)?;
    // TODO use Borrowed always
    Ok(Rope::Leaf(Leaf::Chr(Cow::Borrowed(&string[..]))))
}


fn eval<'c, 'v>(cmd_scope: &'v Rc<Scope>, scope: Rc<Scope>, command: Vec<CommandPart>, args: Vec<Leaf<'v>>) -> Leaf<'v> {
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



/*
impl Value {
    fn serialize(&self) -> String {
        match self {
            &Str(ref x) => x.clone(),
            &Tagged(_, ref x) => x.serialize(),
            _ => {panic!("Cannot serialize") }
        }
    }
}
impl<'s> Atom<'s> {
    fn serialize(&self) -> String {
        (match self {
            &Chars(ref x) => x.to_string(),
            &Val(ref x) => x.serialize()
        })
    }
}
*/

#[test]
fn it_works() {
    let mut s = String::new();
    let mut chars = read_file(&mut s, "tests/1-simple.pp").unwrap();
    let scope = Rc::new(default_scope());
    let results = new_expand(&scope, chars);
    println!("||\n{}||", results.to_str().unwrap());
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    // TODO cli
    println!("Hello, world!");
}
