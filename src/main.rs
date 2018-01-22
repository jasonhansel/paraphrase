
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly

// CURRENT BUGS:
// - issues with if_Eq and recu'rsive defs

// (allow mutual recursion with a special 'define'? add standard library, improve testability)

// TODO: allow changing "catcodes"
// TODO: better error handling
// TODO: misc builtins or library fns (e.g. like m4, and stuff for types)
// TODO: issue trying to change 'w' back to 'world'
// TODO: cf assgbk
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
use std::rc::Rc;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use value::*;
use base::*;
use expand::*;

fn read_file<'s>(mut string: &'s mut String, path: &str) -> Result<Rope<'s>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    file.read_to_string(string)?;
    // TODO use Borrowed always
    Ok(Rope::Leaf(Leaf::Chr(Cow::Borrowed(&string[..]))))
}

#[test]
fn it_works() {
    let mut s = String::new();
    let mut chars = read_file(&mut s, "tests/1-simple.pp").unwrap();
    let scope = Rc::new(default_scope());
    let results = new_expand(scope, chars);
    println!("||\n{}||", results.to_str().unwrap());
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    // TODO cli
    println!("Hello, world!");
}
