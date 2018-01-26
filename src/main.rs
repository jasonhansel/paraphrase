
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
// TODO: improve perf. of PPM demo; currently concurrency *decreases* perf. (prob because of
// evaluation-order or communication issues) -- much better with optimization on (~10x)
// TODO: redirections &c.


mod value;
mod scope;
mod base;
mod expand;

extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use structopt::StructOpt;
extern crate futures;
extern crate futures_cpupool;
extern crate rand;
extern crate regex;

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

fn read_file(mut string: String, path: &str) -> Result<Rope, Error> {
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    file.read_to_string(&mut string)?;
    Ok(Rope::from_slice(ArcSlice::from_string( string )))
}

#[derive(StructOpt,Debug)]
#[structopt(name="paraphrase")]
struct CLIOptions {
    #[structopt(help="Input file")]
    input: String,
    #[structopt(short="j", long="json", help="JSON file with definitions")]
    json_file: Option<String>
}

fn main() {
    // use tests/1-simple.pp to assert correctness
    let opts = CLIOptions::from_args();
    let chars = read_file(String::new(), &opts.input[..]).unwrap();
    let pool = CpuPool::new_num_cpus();
    let pool2 = pool.clone();

    let results = pool.spawn_fn(move ||{ 
        let mut scope = default_scope();
        if let Some(json_path) = opts.json_file {
            let mut s = String::new();
            let mut file = File::open(json_path).unwrap();
            file.read_to_string(&mut s);
            scope.add_json(serde_json::from_str(&s[..]).unwrap());
        }
        expand_with_pool(pool2, Arc::new(scope), chars)
            .map(|x| { Rope::from_value(x).to_str().unwrap().into_string() })
    }).wait();
    match results {
        Ok(result) => { println!("{}", result); },
        Err(err) => { println!("{:?}", err); assert!(false); }
    }
}
