

use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use std::borrow::BorrowMut;
use std::ops::{Deref, Range};
use std::rc::Rc;
use std::iter::Iterator;
use std::borrow::Cow;
use std::borrow::Borrow;

// TODO cloneless

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Tag {
    Num
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValueList<'f>(&'f [Value<'f>]);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ValueChar(char);

// should closures "know" about their parameters?
#[derive(Clone, Debug)]
struct ValueClosure<'f>(Rc<Scope<'f>>, &'f [Value<'f>]);


#[derive(Clone, Debug)]
enum Value<'f> {
    Char(ValueChar),
    List(ValueList<'f>),
    Tagged(Tag, ValueList<'f>),
    Closure(ValueClosure<'f>)
}

impl<'f> PartialEq for Value<'f> {
    fn eq(&self, other: &Value<'f>) -> bool {
        match (self, other) {
            (&Char(a), &Char(b)) => { a == b },
            (&List(ref a), &List(ref b)) => { a == b },
            (&Tagged(ref at, ref ad), &Tagged(ref bt, ref bd)) => {
                at == bt && ad == bd
            },
            (&Closure(_), _)
            | (_, &Closure(_)) => { panic!("Cannot compare closures!"); },
            (_, _) => false
        }
    }

}
impl<'f> Eq for Value<'f> {}

// nb also write stdlib.

#[derive(Clone, Debug)]
enum Command<'f> {
    Define, // add otheres, eg. expand
    IfEq,
    User(Vec<String>, ValueClosure<'f>), // arg names
    UserHere(Vec<String>, ValueList<'f>) // TODO: clone UserHere's into User's
}

use Value::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum CommandPart {
    Ident(String),
    Param
}
use CommandPart::*;

#[derive(Clone, Debug)]
struct Scope<'f> {
    sigil: ValueChar,
    commands: HashMap<Vec<CommandPart>, Command<'f>>
}

impl<'f> ValueList<'f> {
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

fn read_file<'f>(path: &str) -> Result<Vec<Value<'f>>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut x = String::new();
    File::open(path)?.read_to_string(&mut x)?;
    Ok(x.chars().map(|x| Value::Char(ValueChar(x))).collect::<Vec<Value<'f>>>())
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



fn expand_fully<'f>(c : ValueClosure<'f>)
    -> ValueList<'f> {
    let mut closure = c.clone();
    let mut out = vec![];
    loop {
        closure = {
            let (next, ValueList(slice)) = expand_command(closure.clone());
            if let Some(ValueList(c)) = next {
                out.extend_from_slice(c);
                let ValueClosure(scope, _) = closure;
                ValueClosure(scope.clone(),slice)
            } else {
                out.extend_from_slice(slice);
                return ValueList(slice);
            }
        };
    }
}

fn expand_command<'a, 'b, 'v : 'a + 'b>(ValueClosure(ref scope, ref values): ValueClosure<'v>)
 -> (Option<ValueList<'a>>, ValueList<'b>) {
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
            (RawParens(x), _, Some('{')) => { RawParens(x + 1) },
            (RawParens(0), _, Some('}')) => { CloseParen },
            (RawParens(x), _, Some('}')) => { RawParens(x - 1) },
            (RawParens(x), _, _) => { RawParens(x) },


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
        let matches = match(*prev_state, state) {
            (Parens(x), Parens(y)) => { true }
            (x, y) => { x == y }
        };
        let mut result = None;
        if state != End {
            *end = idx;
        }
        if !matches {
            result = Some((*prev_state, (*start..*end)));
            *start = idx;
            *prev_state = state;
        }
        Some(result)
    })
    .flat_map(|x| { x })
    .filter(|&(state,  _)| {
        // TODO keep whitespace at end of macro
        state != Whitespace &&
        state != Start &&
        state != Sigil &&
        state != CloseParen &&
        state != Semi
    })
    .map(|(state, r)| {

        let mut range = r.clone();
        match state {
            Parens(_) => {
                range.start += 1;
            },
            _ => {}
        }
       (state, &values[(range.start)..(range.end)], range)
    })
    .collect::<Vec<(ScanState, &[Value], Range<usize>)>>();

    // note -- only Halt if we're sure it's in the current invocation?
    // ^ and enable parallelism if not a ;-command
    // ^ this may be impossible in the general case :(

    let hpos = parsed.iter().position(|&(s, _, _)| { s == Halt }).map(|pos| { parsed[pos].2.start });
    if let Some(halter) = hpos {
        let closure = ValueClosure(scope.clone(), &(&values[halter..]));
        match expand_command(closure) {
            (None, slice) => {
                panic!("Could not get past halt!");
            },
            (Some(ValueList(expanded)), ValueList(rest)) => { 
                println!("PARTS {:?} __ EXP __ {:?} __ REST __ {:?}", &values[..(halter)], expanded, rest);
                let to_reexpand = values[..(halter)] // Hack to get rid of the '#' - fix later
                    .into_iter()
                    .chain(expanded.into_iter())
                    .chain(rest.into_iter())
                    .map(|x| { x.clone() })
                    .collect::<Vec<Value>>();

                let rest = ValueClosure(
                    scope.clone(),
                    &to_reexpand[..]
                );
                let exp_rest = expand_fully(rest);
                return (Some(exp_rest.clone()), ValueList(&[])) // TODO: why not return 'slice' here? may be a bug or sometihng here
            }
        }
    } else {

        let pos = parsed.iter().position(|&(s, _, _)| { s == CommandName });
        match pos {
            None => {
                (None, ValueList(values))
            }
            Some(pos) => {
                let idxpos = match pos {
                    0 => 0,
                    p => {
                        parsed[p - 1].2.start
                    }
                };
                let mut parts = &parsed[pos..].iter()
                    .map(|&(s, d, _)| { part_for_scan(s, &ValueList(&d[..])) }).collect::<Vec<CommandPart>>()[..];
                // note - quadratic :(
                while !scope.commands.contains_key(
                    &parts.to_vec()
                ) {
                    if let Some((_, r)) = parts.split_last() {
                        parts = r;
                    } else {
                        break;
                    }
               }
                let mut call = &parsed[pos..(pos + parts.len())];
                if parts.len() == 0 {
                    panic!("Failure {:?} {:?}", parsed, pos);
                }
                // nb: unwrap responses from ;-commands
    // nb: demo, parsed perf
                // type coerce args and retvals
                let args = call.iter().flat_map(|&(state, vals, _)| {
                    match state {
                        Parens(_) => Some(List(ValueList(&(vals)))),
                        RawParens(_) => Some(Closure(ValueClosure(scope.clone(),&(vals)))),
                        _ => None
                    }
                }).collect::<Vec<Value>>();
                let ValueList(ref mut expand_result) = match scope.commands.get(&parts.to_vec()).unwrap() {
                    &Command::User(ref args, ref closure) => {
                        // todo handle args
                        //clone() scope?
                        expand_fully(closure.clone())
                    },
                    &Command::UserHere(ref args, ValueList(cmd_data)) => {
                        // todo handle args
                        let closure = ValueClosure(scope.clone(), cmd_data);
                        expand_fully(closure.clone())
                    },
                    &Command::Define => {
                        // get arguments/name from param 1
                        match (&args[0], &args[1], &args[2]) {
                            (&List(ref name_args), &Closure(ValueClosure(_, command_text)), &Closure(ValueClosure(_, to_expand))) => {
                                // TODO: custom arguments, more tests
                                let id = vec![Ident(name_args.to_str())];
                                // make_mut clones as nec.
                                let mut new_scope = (**scope).clone();
                                // circular refs here?
                                new_scope.commands.insert(id, Command::UserHere(vec![],
                                    // TODO: fix scpoe issues
                                    ValueList(command_text)
                                ));
                                expand_fully(ValueClosure(Rc::new(new_scope), to_expand))
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
                };

               
                // TODO: actual expansion here; subtract 1 to avoid sigil
                let result = values[..(parsed[pos].2.start-1)].iter()
                .chain(expand_result.iter())
                .cloned()
                .collect::<Vec<Value>>();

                return (Some(ValueList(&result[..])), ValueList(&values[(call.last().unwrap().2.end)..]));

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

fn expand<'f>(ValueList(values): ValueList<'f>) -> ValueList<'f> {
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
    expand_fully(ValueClosure(Rc::new(scope), values))
    // note - make sure recursive macro defs work
}

impl<'f> Value<'f> {
    fn serialize(&'f self) -> String {
        (match self {
            &Char(ValueChar(x)) => x.to_string(),
            &Tagged(_, ValueList(ref s))
            | &List(ValueList(ref s)) => {
                s.iter().map(|x| { x.serialize() })
                    .fold("".to_owned(), |a, b| { a + &*b })
            },
            &Closure(_) => { panic!("Cannot serialize closures."); }
        })
    }
}


#[test]
fn it_works() {
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = expand(ValueList(&chars[..]));
    let out = Value::List(results).serialize();
    println!("X\n{}X", out);
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
