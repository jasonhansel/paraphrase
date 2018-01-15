
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly

// CURRENT BUGS:
// - issues with if_Eq and recu'rsive defs
// - issue with using macros in defs - basic problem relates to whitespace

mod value;
mod scope;

use scope::*;
use scope::CommandPart::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;


use std::borrow::BorrowMut;
use std::ops::{Deref, Range};
use std::rc::Rc;
use std::iter::Iterator;

use std::fmt;

use std::borrow::Cow;
use std::borrow::Borrow;

use value::*;
use value::Value::*;

// TODO cloneless

impl Eq for Value {}

// nb also write stdlib.

impl ValueList {
    fn to_str(&self) -> String {
        let &ValueList(ref list) = self;
        (&list).iter().map(|x| {
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
    Ok(x.chars().map(|x| Value::Char(ValueChar(x))).collect::<Vec<Value>>())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScanState {
    Text,
    Whitespace,
    Parens(u8), // <- int gives parenlevel
    RawParens(u8),
    Sigil,
    CommandName,
    CloseParen,
    Halt, // <- sigil inside of parameter
    Start,
    End,
    Semi,
    OpenParen,
    Semicolon
}
use ScanState::*;


fn part_for_scan(scan: ScanState, data: &ValueList) -> Option<CommandPart> {
    match (scan, data) {
        (CommandName, _) => {
            Some(Ident(data.to_str()))
        },
        (Parens(_), _)
        | (RawParens(_), _) => {
           Some(Param)
        },
        (Semicolon, _) => {
            Some(Param)
        },
        _ => {
            // TODO: Text should stop scanning altogether, whereas eg. whitespace can continue it
            None
        }
    }
}



fn expand_fully(ValueClosure(scope, values) : ValueClosure)
    -> ValueList {
    let mut closure = values.clone();
    let mut out = vec![];
    loop {
        closure = {
            println!("Exp {:?}", scope.commands.keys());
            let (next, ValueList(slice)) = expand_command(ValueClosure(scope.clone(), closure));
            if let Some(ValueList(c)) = next {
                out.extend(c);
                slice
            } else {
                out.extend(slice);
                return ValueList(out);
            }
        };
    }
}

// todo: allow it to return a 'replacement string'
fn eval(command : &Command, args: Vec<Value>, scope: Rc<Scope>) -> ValueList {
    match command {
        &Command::Rescope => {
            panic!("RESCOPE");
            match(&args[0], &args[1]) {
                (&List(ValueList(ref v)), &Closure(ValueClosure(_, ref contents))) => {
                    match (v.first()) {
                        Some(&Closure(ValueClosure(ref inner_scope, _))) => ValueList(vec![
                            Closure(ValueClosure(inner_scope.clone(), contents.clone()))
                        ]),
                        _ => {panic!(); }
                    }
                },
                _ => {panic!(); }
            }
        },
        &Command::Expand => {
            panic!();
            match(&args[0]) {
                &List(ValueList(ref v)) => {

                    match (v.first()) {
                        Some(&Closure(ref c)) => {
                            
                           let expanded = List(expand_fully(c.clone())); // should i wrap this in another valuelist?
                            panic!("Test {:?}", expanded);
                            ValueList(vec![ expanded  ])
                        },
                        _ => {panic!(); }
                    }
                },
                _ => {panic!(); }
            }
        }
        &Command::Immediate(ref x) => {
            println!("IMMED {:?}", x);
            ValueList(vec![ x.clone() ])
        },
        &Command::User(ref arg_names, ValueClosure(ref inner_scope, ref contents)) => {
            // todo handle args
            //clone() scope?
            let mut new_scope = dup_scope(inner_scope.clone());
            if arg_names.len() != args.len() {
                panic!("Wrong number of arguments supplied to evaluator {:?} {:?}", command, args);
            }
            for (name, arg) in arg_names.iter().zip( args.iter() ) {
                // should it always take no arguments?
                // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                // or a Tagged). coerce sometimes?
                new_scope.commands.insert(vec![Ident(name.to_owned())], Command::Immediate(arg.clone()) );
            }
            expand_fully(ValueClosure(Rc::new(new_scope), contents.clone()))
        },
        &Command::UserHere(ref arg_names, ValueList(ref cmd_data)) => {
            // todo handle args
            let closure = ValueClosure(scope.clone(), cmd_data.clone());
            eval(&Command::User(arg_names.clone(), closure), args, scope)
        },
        &Command::Define => {
            // get arguments/name from param 1
            match (&args[0], &args[1], &args[2]) {
                (&List(ref name_args), &Closure(ValueClosure(_, ref command_text)), &Closure(ValueClosure(_, ref to_expand))) => {
                    // TODO: custom arguments, more tests
                    let mut parts = vec![];
                    let mut params = vec![];
                    let na_str = name_args.to_str();
                    for part in na_str.split(' ') {
                        if part.starts_with(':') {
                            parts.push(Param);
                            params.push((&part[1..]).to_owned());
                        } else {
                            parts.push(Ident(part.to_owned()));
                        }
                    }
                    println!("Definining {:?}", parts);
                    // make_mut clones as nec.
                    let mut new_scope = dup_scope(scope);
                    // circular refs here?
                    new_scope.commands.insert(parts, Command::UserHere(params,
                        // TODO: fix scpoe issues
                        ValueList(command_text.clone())
                    ));
                    expand_fully(ValueClosure(Rc::new(new_scope), to_expand.clone()))
                },
                _ => {
                    panic!("Invalid state")
                }
            }
        },
        &Command::IfEq => {
            match (&args[0], &args[1], &args[2], &args[3]) {
                (ref a, ref b, &Closure(ref if_true), &Closure(ref if_false)) => {
                     if a == b { expand_fully(if_true.clone()) }
                     else { expand_fully(if_false.clone()) }
                },
                _ => { panic!("Invalid :("); }
            }
        }
    }
}

fn expand_command<'a, 'b, 'v : 'a + 'b>(ValueClosure(scope, values): ValueClosure)
 -> (Option<ValueList>, ValueList) {
    // Allow nested macroexpansion (get order right -- 'inner first' for most params,
    // 'outer first' for lazy/semi params. some inner-first commands will return stuff that needs
    // to be re-expanded, if a ';'-command - but does this affect parallelism? etc)

    // tODO: this is all super slow, and has way too much copying

    let parsed = values
    .iter()
    .enumerate()
    .scan(ScanState::Text, |state, (idx, v)| {
        let ch = match v {
            &Value::Char(ValueChar(c)) => Some(c),
            _ => None
        };
        let ValueChar(sigil) = scope.sigil;
        let is_white = ch.map(|c| c.is_whitespace()).unwrap_or(false);
        *state = match( (*state, false, ch) ) {
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
            },
            (CommandName, _, Some('('))
            | (Whitespace, _, Some('('))
            | (CloseParen, _, Some('(')) => { Parens(0) },
            (Parens(x), _, Some('(')) => { Parens(x + 1) },
            (Parens(0), _, Some(')')) => { CloseParen },
            (Parens(x), _, Some(')')) => { Parens(x - 1) },
            
            (CommandName, _, Some('{'))
            | (Whitespace, _, Some('{'))
            | (CloseParen, _, Some('{')) => { RawParens(0) },
            (RawParens(x), _, Some('{')) => { println!("OPEN {:?}", x); RawParens(x + 1) },
            (RawParens(0), _, Some('}')) => { CloseParen },
            (RawParens(x), _, Some('}')) => { println!("CLOSE {:?}", x); RawParens(x - 1) },
            (RawParens(x), _, Some(c)) => { RawParens(x) },


            (Parens(x), _, w) => {
                if w == Some(sigil) {
                    Halt
                } else {
                    Parens(x)
                }
            },

            (Parens(x), _, _) => { Parens(x) },
            (RawParens(x), _, _) => { RawParens(x) },


            (Halt, _, _) => { Halt },

            (CommandName, _, Some(c)) => {
                if c.is_alphabetic() || c == '_' {
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
    .chain(std::iter::once((End, &Value::Char(ValueChar(' ')), 0)))
    .scan((0, 0, Start),
    |&mut(ref mut start, ref mut end, ref mut prev_state), (state, val, idx)| {
        if *prev_state == End {
            return None;
        }
        // get a proper debugger?
        let matches = match(*prev_state, state) {
            (Parens(x), Parens(y)) => { true }
            (RawParens(x), RawParens(y)) => { true }
            (x, y) => { x == y }
        };
        let mut result = None;
        if state != End {
            *end = idx;
        } else {
            *end += 1;
        }
        if !matches {
            result = Some((*prev_state, (*start..*end)));
            *start = idx;
            *prev_state = state;
        }
        Some(result)
    })
    .flat_map(|x| { x })
    .flat_map(|(state, r)| {
        let mut range = r.clone();
        let s = match state {
            Parens(_) => Parens(0),
            RawParens(_) => RawParens(0),
            x => x
        };
        match s {
            Parens(_)
            | RawParens(_) => {
                vec![(OpenParen, ((range.start)..(range.start+1))),
                        (s, ((range.start+1)..(range.end))) ]
            },
            _ => {
                vec![(state, range)]
            }
        }
    })
    .collect::<Vec<(ScanState, Range<usize>)>>();
 
 //   println!("PARSED {:?}", parsed.clone().iter().map(|&(ref s, ref x)| (s.clone(), values[x.clone().start .. x.clone().end].to_vec())).collect::<Vec<(ScanState, Vec<Value>)>>() ) ;

    /*.filter(|&(state,  _)| {
        // TODO keep whitespace at end of macro
        state != Whitespace &&
        state != Start &&
        state != Sigil &&
        state != CloseParen &&
        state != Semi
    }) */
    // note -- only Halt if we're sure it's in the current invocation?
    // ^ and enable parallelism if not a ;-command
    // ^ this may be impossible in the general case :(

    let hpos = parsed.iter().position(|&(s, _)| { s == Halt }).map(|pos| { parsed[pos].1.start });
    if let Some(halter) = hpos {
        std::mem::drop(parsed); // a test
        let mut v = values.clone();
        let after = v.split_off(halter);
        println!("HERE {:?} {:?}", v, after);
        let closure = ValueClosure(scope.clone(), after);
        // expand one command
        match expand_command(closure) {
            (None, slice) => {
                panic!("Could not get past halt!");
            },
            (Some(expanded), rest) => { 
                println!("REST {:?} THENTHEN {:?}", expanded, rest.to_str());
                v.push(List(expanded)); // testing instead of extend

                return (Some(ValueList(v)), rest ) // TODO: why not return 'slice' here? may be a bug or sometihng here
            }
        }
    } else {

        let pos = parsed.iter().position(|&(s, _)| { s == Sigil });
        match pos {
            None => {
                (None, ValueList(values))
            }
            Some(pos) => {
                let mut parts = parsed
                    .iter()
                    .enumerate()
                    .skip(pos)
                    .by_ref()
                    .map(|(idx, &(ref s, ref r))| {
                        println!("Part {:?} {:?}", s, r);
                        (idx, part_for_scan(*s, &ValueList(values[r.clone()].to_vec())))
                    }).collect::<Vec<(usize, Option<CommandPart>)>>();
                // note - quadratic :(
                let oldparts = parts.clone();
                while { !scope.commands.contains_key(&parts.iter().flat_map(|&(ref i, ref x)| { x }).cloned().collect::<Vec<CommandPart>>())
                    && parts.pop() != None  } {
                    }
                if parts.len() == 0 {
                    panic!("Could not find command... {:?} IN {:?}", oldparts, scope);
                }
                // Hacky hacky hack
                while parts.last().unwrap().1 == None
                    && parsed[parts.last().unwrap().0].0 != CloseParen {
                    parts.pop();
                }
               
                // nb: unwrap responses from ;-commands
    // nb: demo, parsed perf
                // type coerce args and retvals
                let pos_end = parts.last().unwrap().0;
                let args = parsed[(parts.first().unwrap().0)..(parts.last().unwrap().0+ 1)]
                    .to_vec()
                    .iter()
                    .flat_map(|&(ref state, ref range)| {
                        let vals = &values[range.clone()];
                        match *state {
                            Parens(_) => Some(List(ValueList(vals.to_vec()))),
                            RawParens(_) => Some(Closure(ValueClosure(scope.clone(),vals.to_vec()))),
                            Semicolon => Some(Closure(ValueClosure(scope.clone(),vals.to_vec()))),
                            _ => None
                        }
                    }).collect::<Vec<Value>>();
                let ValueList(ref mut expand_result) = eval(scope.commands.get(&parts.iter().flat_map(|&(ref i, ref x)|{ x}).cloned().collect::<Vec<CommandPart>>()).unwrap(), args, scope.clone());
                              
                // TODO: actual expansion here; subtract 1 to avoid sigil
                let result = values
                .iter()
                .take(parsed[parts.first().unwrap().0].1.start)
                .chain(expand_result.iter())
                .cloned()
                .collect::<Vec<Value>>();

                let end = {
                    let rest = values.clone().split_off(parsed[parts.last().unwrap().0].1.end);
                    ValueList(rest)
                };


                return (Some(ValueList(result)), end);

            }
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

fn expand(ValueList(values): ValueList) -> ValueList {
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
    scope.commands.insert(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ],
        Command::IfEq
    );
    scope.commands.insert(vec![ Ident("expand".to_owned()), Param ],
        Command::Expand
    );
    scope.commands.insert(vec![ Ident("rescope".to_owned()), Param, Param ],
        Command::Rescope
    );
    expand_fully(ValueClosure(Rc::new(scope), values))
    // note - make sure recursive macro defs work
}

impl Value {
    fn serialize(&self) -> String {
        (match self {
            &Char(ValueChar(x)) => x.to_string(),
            &Tagged(_, ValueList(ref s))
            | &List(ValueList(ref s)) => {
                s.iter().map(|x| { x.serialize() })
                    .fold("".to_owned(), |a, b| { a + &*b })
            },
            &Closure(ref x) => { panic!("Cannot serialize closure: {:?}", x); }
        })
    }
}


#[test]
fn it_works() {
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = expand(ValueList(chars));
    let out = Value::List(results).serialize();
    println!("||\n{}||", out);
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
