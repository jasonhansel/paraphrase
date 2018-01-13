

use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use std::borrow::BorrowMut;
use std::ops::{Deref, Range};
use std::rc::Rc;
use std::iter::Iterator;
use std::borrow::Cow;

#[derive(Copy, Clone, Debug)]
enum Tag {
    Num
}

#[derive(Clone, Debug)]
struct ValueList(Vec<Value>);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ValueChar(char);

// should closures "know" about their parameters?
#[derive(Clone, Debug)]
struct ValueClosure(Rc<Scope>, ValueList);

#[derive(Clone, Debug)]
enum Value {
    Char(ValueChar),
    List(ValueList),
    Tagged(Tag, ValueList),
    Closure(ValueClosure)
}

use Value::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum CommandPart {
    Ident(String),
    Param
}
use CommandPart::*;

#[derive(Clone, Debug, Default)]
struct CommandTrie {
    cmd: Option< ValueClosure >,
    next: Option< HashMap<CommandPart, Rc<CommandTrie>> >
}

#[derive(Clone, Debug)]
struct Scope {
    sigil: ValueChar,
    commands: Rc<CommandTrie>
}

impl CommandTrie {
    fn insert(&mut self, parts: &[CommandPart], cmd: ValueClosure) {
        match parts.split_first() {
            None => { self.cmd = Some(cmd); }
            Some((first, rest)) => {
                let &mut CommandTrie{ ref mut next, .. } = self;
                match next {
                    &mut Some(_)  => {},
                    &mut None => {
                        *next = Some(HashMap::new());
                    }
                }
                match next {
                    &mut Some(ref mut subtree) => { 
                        let data = subtree
                            .entry(first.clone())
                            .or_insert_with(|| { Rc::new(CommandTrie::default()) });
                        Rc::get_mut(data).unwrap().insert(rest, cmd);
                    },
                    &mut None => { panic!("Err"); }
                }
           }
        }
    }
}

fn read_file(path: &str) -> Result<Vec<Value>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut x = String::new();
    File::open(path)?.read_to_string(&mut x)?;
    Ok(x.chars().map(|x| Value::Char(ValueChar(x))).collect())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScanState {
    Text,
    Whitespace,
    Parens(u8), // <- int gives parenlevel
    Sigil,
    CommandName,
    Halt, // <- sigil inside of parameter
    Start,
    Semicolon
}
use ScanState::*;

fn expand_command(
    iter: &mut ValueList,
    cmd_here : Rc<CommandTrie>,
    scope: &Scope
) {
    // Allow nested macroexpansion (get order right -- 'inner first' for most params,
    // 'outer first' for lazy/semi params. some inner-first commands will return stuff that needs
    // to be re-expanded, if a ';'-command - but does this affect parallelism? etc)

    // tODO: this is all super slow, and has way too much copying

    let &mut ValueList(ref x) = iter;
    let test = x.clone()
        .into_iter()
        .enumerate()
        .scan(ScanState::Text, |state, (idx, v)| {
            let ch = match v {
                Value::Char(ValueChar(c)) => Some(c),
                _ => None
            };
            let ValueChar(sigil) = scope.sigil;
            let is_white = ch.map(|c| c.is_whitespace()).unwrap_or(false);
            *state = match( (*state, &v, ch) ) {
                (Text, _, Some(c)) => {
                    if c == sigil {
                        Sigil
                    } else {
                        Text
                    }
                },
                (Text, _, _) => { Text },

                (Sigil, _, _) => { CommandName },
                // todo write more tests
                (CommandName,_, Some(';')) => { Semicolon },
                (Whitespace, _, Some(';')) => { Semicolon },
                (Semicolon, _, _) => { Semicolon }
                (CommandName, _, Some('(')) => { Parens(0) },
                (Whitespace, _, Some('(')) => { Parens(0) },
                (Parens(x), _, Some('(')) => { Parens(x + 1) },
                (Parens(0), _, Some(')')) => { Whitespace },
                (Parens(x), _, Some(')')) => { Parens(x - 1) },
                (Parens(x), _, w) => {
                    if w == Some(sigil) {
                        Halt
                    } else {
                        Parens(x)
                    }
                },

                (Halt, _, _) => { Halt },

                (CommandName, _, Some(c)) => {
                    if c.is_alphabetic() {
                        CommandName
                    } else if c.is_whitespace() {
                        Whitespace
                    } else {
                        Text
                    }
                },
                (Whitespace, _, Some(c)) => {
                    if c == sigil { Sigil }
                    else if c.is_whitespace() { Whitespace }
                    else { Text }
                },

                (Whitespace, _, None) => { Text }
                (CommandName, _, None) => { Text }

                _ => {
                    panic!("Unhandled state change...");
                }
            };
            Some((*state, v, idx))
        })
        .chain(std::iter::once((Halt, Value::Char(ValueChar(' ')), 0)))
        .scan((vec! [], 0, 0, Start),
        |&mut(ref mut vec, ref mut start, ref mut end, ref mut prev_state), (state, val, idx)| {
            let matches = match(*prev_state, state) {
                (Parens(x), Parens(y)) => { true }
                (x, y) => { x == y }
            };
            let mut result = None;
            if !matches {
                *end = idx;
                result = Some((*prev_state, vec.clone(), (*start..*end)));
                *start = idx;
                vec.clear();
                *prev_state = state;
            }
            vec.push(val);
            Some(result)
        })
        .flat_map(|x| { x })
        .filter(|&(state, _, _)| {
            state != Whitespace && state != Start && state != Sigil
        })
        .map(|(state, mut vals, mut range)| {
            if let Parens(_) = state {
                vals.remove(0);
                range.start += 1;
            }
            (state, vals, range)
        });
                println!("HEY {:?}", test.collect::<Vec<(ScanState, Vec<Value>, Range<usize>)>>());
    panic!("BYE");

   let mut postwhite = {
       let ValueList(values) = iter.clone();
       values.into_iter()
    }
        .skip_while(|x| {
            match x {
                &Value::Char(ValueChar(c)) => c.is_whitespace(),
                _ => false
            }
        }).peekable();
    // TODO: 'early' expansion, expansion to chars
    if let Some(ref tree) = cmd_here.next {
       if tree.contains_key(&Param) {
               let npw = postwhite.collect::<Vec<Value>>();
            println!("Seeking param {:?}", npw);
            let param = npw
                .clone()
                .into_iter()
            .scan(0, |bal, x| {
                *bal += match x {
                    Value::Char(ValueChar('(')) => 1,
                    Value::Char(ValueChar(')')) => -1,
                    _ => 0
                };
                Some((*bal, x))
            })
            .take_while(|&(bal, _)| {
                bal > 0
            })
            .map(|(bal, x)| { x })
            .collect::<Vec<Value>>();
            println!("PARAM {:?}", param);
            // todo: strip parens
            if param.len() > 0 {
                *iter = ValueList(npw.into_iter().skip(param.len() + 1).collect());
                return expand_command(iter,
                    tree.get(&Param).unwrap().clone(), scope);
            } else {
                panic!("Empty param {:?}", npw);
            }
        }
       // Allow '##X' etc.
       if let Some(&Value::Char(c)) = postwhite.peek() {
           if c == scope.sigil {
               let npw = postwhite.collect::<Vec<Value>>();
                let cmd_name = npw
                    .clone()
                .into_iter()
                .skip(1)
                .take_while(|x| {
                    match x {
                        &Char(ValueChar(c)) => c.is_alphabetic(),
                        _ => false
                    }
                })
                .fold("".to_owned(), |mut s, x| {
                    if let Char(ValueChar(c)) = x {
                        s.push(c);
                        return s;
                    } else {
                        panic!("Err");
                    }
                });
                println!("SIGIL! {:?}", cmd_name);
                if(cmd_name.len() > 0 && tree.contains_key(&Ident(cmd_name.clone()))) {
                    *iter = ValueList(npw.into_iter().skip(1 + cmd_name.len()).collect());
                    // does string equality work as expected
                    return expand_command(iter,
                        tree.get(&Ident(cmd_name.clone())).unwrap().clone(), scope);
                } 
           } 
       }
    }
    match (iter, cmd_here.cmd.clone()) {
        ( &mut ValueList(ref mut vl), Some(ValueClosure(_, ValueList(ref mut command))) ) => {
            *vl = command.iter().chain(vl.iter()).cloned().collect::<Vec<Value>>();
            println!("Done expanding...");
        },
        _ => { panic!("Failed :("); }
    }
 
}

fn expand_text(vals: &mut ValueList, scope: Scope) {
    let ValueList(ref mut values) = vals.clone();
    match values.split_first() {
        None => {},
        Some((first, r)) => { 
            if let &Char(c) = first {
                if c == scope.sigil {
                    println!("Expanding command...");
                    std::io::stdout().flush().unwrap();
                    // expand_command will expand *a* command (maybe not this one -- e.g.
                    // it could be an inner command in one of the arguments). But it will
                    // make progress.
                    expand_command(vals, scope.commands.clone(), &scope);
                    expand_text(vals, scope);
                    return;
                }
            }
            println!("Expanding rest...");
            std::io::stdout().flush().unwrap();
            let mut rest = ValueList(r.iter().cloned().collect());
            {
                let ValueList(ref mut rest_arr) = rest;
                rest_arr.remove(0);
            }
            expand_text(&mut rest, scope);
            {
                let &mut ValueList(ref mut v) = vals;
                let ValueList(rest_arr) = rest;
                v.truncate(1);
                v.extend(rest_arr);
            }
        }
    }
}

fn expand(values: Vec<Value>) -> ValueList {
    println!("Expand...");
    std::io::stdout().flush().unwrap();
    let mut scope = Scope {
        sigil: ValueChar('#'),
        commands: Rc::new(CommandTrie::default())
    };
    // idea: source maps?
    // add 3rd param (;-kind)
    Rc::get_mut(&mut scope.commands).unwrap().insert(&(vec![ Ident("define".to_owned()), Param, Param ])[..],
        ValueClosure(
            Rc::new(Scope { sigil: ValueChar('#'), commands: Rc::new(CommandTrie::default()) }),
            ValueList( vec! [Value::Char(ValueChar('a'))] ) )
    );
    let mut vlist = ValueList(values);
    expand_text(&mut vlist, scope);
    vlist
    // note - make sure recursive macro defs work
}

impl Value {
    fn serialize(&self) -> Vec<char> {
        match self {
            &Char(ValueChar(ref x)) => vec![ *x ],
            &Tagged(ref t, ref x) => Value::List(x.clone()).serialize(),
            &List(ValueList(ref s)) => s.into_iter().flat_map(|x| { x.serialize() }).collect(),
            &Closure(_) => { panic!("Cannot serialize closures."); }
        }
    }
}


#[test]
fn it_works() {
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = expand(chars);
    assert_eq!(Value::List(results).serialize().iter().collect::<String>(), "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
