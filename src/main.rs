

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

// nb also write stdlib.

#[derive(Clone, Debug)]
enum Command {
    Define, // add otheres, eg. expand
    User(Vec<String>, ValueClosure), // arg names
    UserHere(Vec<String>, ValueList) // TODO: clone UserHere's into User's
}

use Value::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum CommandPart {
    Ident(String),
    Param
}
use CommandPart::*;

#[derive(Clone, Debug)]
struct Scope {
    sigil: ValueChar,
    commands: HashMap<Vec<CommandPart>, Command>
}

impl ValueList {
    fn to_str(&self) -> String {
        let &ValueList(ref list) = self;
        list.iter().map(|x| {
            match(x) {
                &Char(ValueChar(c)) => { c }
                _ => { panic!() }
            }
        }).collect::<String>()
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
    CloseParen,
    Halt, // <- sigil inside of parameter
    Start,
    End,
    Semi,
    Semicolon
}
use ScanState::*;


fn part_for_scan(scan: ScanState, data: &ValueList) -> CommandPart {
    match (scan, data) {
        (CommandName, _) => {
            Ident(data.to_str())
        },
        (Parens(0), _) => {
           Param
        },
        (Semicolon, _) => {
            Param
        },
        _ => Ident("INVALID".to_owned())
    }
}

fn expand_command(
    &ValueList(ref list): &ValueList,
    scope: &Scope
) -> ValueList {
    // Allow nested macroexpansion (get order right -- 'inner first' for most params,
    // 'outer first' for lazy/semi params. some inner-first commands will return stuff that needs
    // to be re-expanded, if a ';'-command - but does this affect parallelism? etc)

    // tODO: this is all super slow, and has way too much copying

    let test = list
    .clone()
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
            (CommandName,_, Some(';')) => { Semi },
            (Whitespace, _, Some(';')) => { Semi },
            (CloseParen, _, Some(';')) => { Semi },
            (Semi, _, Some(c)) => {
                if c.is_whitespace() {
                    Semi
                } else {
                    Semicolon
                }
            }
            (Semi, _, _)
            | (Semicolon, _, _) => {
                Semicolon
            }
            (CommandName, _, Some('(')) => { Parens(0) },
            (Whitespace, _, Some('(')) => { Parens(0) },
            (CloseParen, _, Some('(')) => { Parens(0) },
            (Parens(x), _, Some('(')) => { Parens(x + 1) },
            (Parens(0), _, Some(')')) => { CloseParen },
            (Parens(x), _, Some(')')) => { Parens(x - 1) },
            (Parens(x), _, w) => {
                if w == Some(sigil) {
                    Halt
                } else {
                    Parens(x)
                }
            },

            // Should semicolons swallow whitespace?

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
            (Whitespace, _, Some(c))
            | (CloseParen, _, Some(c)) => {
                if c == sigil { Sigil }
                else if c.is_whitespace() { Whitespace }
                else { Text }
            },

            (Whitespace, _, None) => { Text }
            (CloseParen, _, None) => { Text }
            (CommandName, _, None) => { Text }

            _ => {
                panic!("Unhandled state change...");
            }
        };
        Some((*state, v, idx))
    })
    .chain(std::iter::once((End, Value::Char(ValueChar(' ')), 0)))
    .scan((vec! [], 0, 0, Start),
    |&mut(ref mut vec, ref mut start, ref mut end, ref mut prev_state), (state, val, idx)| {
        if *prev_state == End {
            return None;
        }
        let matches = match(*prev_state, state) {
            (Parens(x), Parens(y)) => { true }
            (x, y) => { x == y }
        };
        let mut result = None;
        if state != End {
            *end = idx;
        }
        if !matches {
            result = Some((*prev_state, vec.clone(), (*start..*end)));
            *start = idx;
            vec.clear();
            *prev_state = state;
        }
        if state != End {
            vec.push(val);
        }
        Some(result)
    })
    .flat_map(|x| { x })
    .filter(|&(state, _, _)| {
        state != Whitespace && state != Start && state != Sigil && state != CloseParen && state != Semi
    })
    .map(|(state, mut vals, mut range)| {
        match state {
            Parens(_) => {
                vals.remove(0);
                range.start += 1;
            },
            _ => {}
        }
       (state, ValueList(vals), range)
    })
    .collect::<Vec<(ScanState, ValueList, Range<usize>)>>();

    // note -- only Halt if we're sure it's in the current invocation?
    // ^ and enable parallelism if not a ;-command
    // ^ this may be impossible in the general case :(

    let pos = test.iter().position(|&(s, _, _)| { s == CommandName });
    match pos {
        None => {
            ValueList(list.clone())
        }
        Some(pos) => {
            let idxpos = match pos {
                0 => 0,
                p => {
                    test[p - 1].2.start
                }
            };
            let mut call = &test[pos..];
            let mut parts = call.iter()
                .map(|&(s, ref d, _)| { part_for_scan(s, d) }).collect::<Vec<CommandPart>>();
            // note - quadratic :(
            while !scope.commands.contains_key(
                &parts
            ) {
                call = &call.split_last().unwrap().1;
                parts.pop();
                if call.len() == 0 {
                    panic!("Unrecognized call!");
                }
            }
// nb: demo, test perf
            let ValueList(expand_result) = match scope.commands.get(&parts).unwrap() {
                &Command::User(ref args, ValueClosure(ref inner_scope, ref cmd_data)) => {
                    // todo handle args
                    expand_command(cmd_data, &inner_scope)
                },
                &Command::UserHere(ref args, ref cmd_data) => {
                    // todo handle args
                    expand_command(cmd_data, &scope)
                },
                &Command::Define => {
                    // get arguments/name from param 1
                    return match (&call[1], &call[2], &call[3]) {
                        (
                            &(_, ref name_args, _),
                            &(_, ref command_text, _),
                            &(_, ref to_expand, _)
                        ) => {
                            // TODO: custom arguments, more tests
                            let id = vec![Ident(name_args.to_str())];
                            // make_mut clones as nec.
                            let mut new_scope = scope.clone();
                            // circular refs here?
                            new_scope.commands.insert(id, Command::UserHere(vec![],
                                // TODO: fix scpoe issues
                                command_text.clone()
                            ));
                            expand_command(to_expand, &new_scope)
                        },
                        _ => {
                            panic!("Invalid state")
                        }
                    }
                }
            };
            let ValueList(remainder) = expand_command(
                &ValueList( list[(call.last().unwrap().2.end)..].to_vec() ),
                scope
            );
           
            // TODO: actual expansion here
            let result = list[..pos].iter()
            .chain(expand_result.iter())
            .chain(remainder.iter())
            .cloned()
            .collect::<Vec<Value>>();
            ValueList(result)

        }
    }
/*
    match (iter, cmd_here.cmd.clone()) {
        ( &mut ValueList(ref mut vl), Some(ValueClosure(_, ValueList(ref mut command))) ) => {
            *vl = command.iter().chain(vl.iter()).cloned().collect::<Vec<Value>>();
            println!("Done expanding...");
        },
        _ => { panic!("Failed :("); }
    }
 */
}

fn expand_text(vals: &mut ValueList, scope: Scope) {
    *vals = expand_command(vals, &scope);
/*

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
                    expand_command(vals, &scope);
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
*/
}

fn expand(values: Vec<Value>) -> ValueList {
    println!("Expand...");
    std::io::stdout().flush().unwrap();
    let mut scope = Scope {
        sigil: ValueChar('#'),
        commands: HashMap::new()
    };
    // idea: source maps?
    // add 3rd param (;-kind)
    scope.commands.insert(vec![ Ident("define".to_owned()), Param, Param, Param ],
        Command::Define
    );
    let mut vlist = ValueList(values);
    expand_text(&mut vlist, scope);
    vlist
    // note - make sure recursive macro defs work
}

impl Value {
    fn serialize(self) -> ValueList {
        ValueList(match self {
            Char(ref x) => vec![ Char(*x) ],
            Tagged(t, x) => {
               let ValueList(vals) = Value::List(x).serialize();
               vals
            },
            List(ValueList(s)) => s.into_iter().flat_map(|x| {
                let ValueList(vals) = x.serialize();
                vals
            }).collect::<Vec<Value>>(),
            Closure(_) => { panic!("Cannot serialize closures."); }
        })
    }
}


#[test]
fn it_works() {
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = expand(chars);
    let out = Value::List(results).serialize().to_str();
    println!("{:?}", out);
    assert_eq!(out, "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
