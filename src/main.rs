
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly

// CURRENT BUGS:
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

extern crate futures;
extern crate futures_cpupool;

use futures_cpupool::{CpuFuture, CpuPool};
use futures::prelude::*;
use scope::*;
use std::borrow::Cow;
use std::rc::Rc;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use value::*;
use base::*;
use expand::*;

fn read_file<'s>(mut string: String, path: &str) -> Result<Rope<'s>, Error> {
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    file.read_to_string(&mut string)?;
    Ok(Rope::from_slice(ArcSlice::from_string( string )))
}

#[test]
fn it_works() {
    // TODO: organize a real test suite
    let mut chars = read_file(String::new(), "tests/1-simple.pp").unwrap();
    let pool = CpuPool::new_num_cpus();
    let results = expand_with_pool(pool, Arc::new(default_scope()), chars)
        .wait()
        .unwrap()
        .as_str()
        .unwrap()
        .into_string();
    println!("||\n{}||", results );
}

fn main() {
    // TODO add a CLI
    println!("Hello, world!");
}
