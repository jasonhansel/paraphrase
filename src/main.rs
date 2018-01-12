

use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::{Read, Error};
use std::result::Result;
use std::borrow::BorrowMut;
use std::ops::Deref; 
use std::rc::Rc;

#[derive(Copy, Clone, Debug)]
enum Tag {
    Num
}

#[derive(Copy, Clone, Debug)]
enum Value<'f> {
    Char(char),
    Str(&'f str),
    TaggedStr(Tag, &'f str),
    List(&'f Vec<Value<'f>>),
    Closure(&'f Scope<'f>, &'f Vec<Value<'f>>)
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum CommandPart<'f> {
    Ident(&'f str),
    Param
}

#[derive(Clone, Debug, Default)]
struct CommandTrie<'f> {
    cmd: Option<Value<'f>>,
    next: Option<HashMap<CommandPart<'f>, Rc<CommandTrie<'f>>>>
}

impl<'f> CommandTrie<'f> {
    fn insert(&mut self, parts: &[CommandPart<'f>], cmd: Value<'f>) {
        if(parts.len() == 0) {
            self.cmd = Some(cmd);
        } else {
            match self.next {
                None => { self.next = Some(HashMap::new()) },
                _ => {}
            };
            let mut subtree = self.next.as_mut().unwrap();
            let mut c = subtree.entry(parts[0].clone())
                .or_insert(Rc::new(CommandTrie::default()));
            Rc::make_mut(c).insert(&parts[1..], cmd);
        }
    }
}

#[derive(Clone, Debug)]
struct Scope<'f> {
    sigil: char,
    commands: CommandTrie<'f>
}

#[allow(unused_imports)]
use Value::*;
use CommandPart::*;

fn read_file(path: &str) -> Result<Vec<Value>,Error> {
    let mut x = "".to_owned();
    File::open(path)?.read_to_string(&mut x)?;
    Ok(x.chars().map(|x| Value::Char(x)).collect())
}

fn expand<'f>(values: Vec<Value<'f>>) -> Vec<Value<'f>> {
    let mut scope = Scope {
        sigil: '#',
        commands: CommandTrie::default()
    };
    scope.commands.insert(&(vec![ Ident("define"), Param, Param, Param ])[..], Value::Char('a'));

    let mut cmd_here : Rc<CommandTrie> = Rc::new(scope.commands);
    let mut iter = values.into_iter().peekable();

    let mut res = Vec::new();

    // note - make sure recursive macro defs work
    
    while let Some(val) = iter.next() {
        // really we should check cmd_here, then its parent, then its parent...or something
        if let Char(c) = val {
           if(c == scope.sigil) {
                let mut part = String::new();
                let alp = iter.clone().take_while(|x| {
                    if let &Char(c) = x {
                        if c.is_alphabetic() {
                            return true;
                        }
                    }
                    return false;
                });
                
                for a in alp {
                    if let Char(c) = a { part.push(c); }
                    iter.next();
                }
                let id = Ident(&part[..]);
                cmd_here = {
                    let ctree = cmd_here.next.as_ref().unwrap();
                    println!("TEST {:?}", part);
                    ctree.get(&id).unwrap().clone()
                }
            } else {
                res.push(Value::Char(c));
            }
        } else {
            panic!("Invalid state!");
        }
    }
    return res;
}


fn serialize(values: Vec<Value>) -> String {
    let mut result = "".to_owned();


    for val in values {
        match val {
            Char(x) => { result.push(x); },
            Str(x) | TaggedStr(_, x) => { result.push_str(x); },
            _ => {
                panic!("Cannot serialize a list.");
            }
        }
    }
    result
}

#[test]
fn it_works() {
    assert_eq!(serialize(expand(read_file("tests/1-simple.pp").unwrap())), "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
