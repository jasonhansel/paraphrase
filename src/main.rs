
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly

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


// NOTE: expanding from the right  === expanding greedily

mod value;
mod scope;

use scope::*;

use std::ops::Range;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Error, Write};
use std::result::Result;
use std::rc::Rc;
use std::iter::Iterator;
use value::*;

// TODO cloneless

impl Eq for Value {}

// nb also write stdlib.

fn read_file(path: &str) -> Result<Vec<Atom>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut x = String::new();
    File::open(path)?.read_to_string(&mut x)?;
    Ok(x.chars().map(|x| Char(x)).collect::<Vec<Atom>>())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScanState {
    Text,
    Whitespace,
    Parens(u8), // <- int gives parenlevel
    RawParens(u8),
    StartSigil,
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


fn part_for_scan(scan: ScanState, data: &[Atom]) -> Option<CommandPart> {
    match (scan, data) {
        (CommandName, _) => {
            Some(Ident(data.iter().fold("".to_owned(), |mut s, x| {
                s.push(match x {
                    &Char(x) => { x },
                    _ => {panic!() }
                });
                s
            })))
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



fn eval(scope: Rc<Scope>, command: &Command, args: &[Value]) -> Value {
   let atoms : Vec<Atom> = match command {
        &Command::Rescope => {
            match(&args[0], &args[1]) {
                (&List(ref v), &Closure(ValueClosure(_, ref contents))) => {
                    match v.first() {
                        Some(&Closure(ValueClosure(ref inner_scope, _))) => { vec![
                            Val( Closure(ValueClosure(inner_scope.clone(), contents.clone())) )
                        ] },
                        _ => {panic!() }
                    }
                },
                _ => {panic!() }
            }
        },
        &Command::Expand => {
            match &args[0] {
                &List(ref v) => {
                    match v.first() {
                        Some(&Closure(ref c)) => {
                           expand_fully(c)
                        },
                        _ => {panic!(); }
                    }
                },
                _ => {panic!(); }
            }
        }
        &Command::Immediate(ref x) => {
            println!("IMMED {:?}", x);
            vec![ Val(x.clone()) ]
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
            expand_fully(&ValueClosure(Rc::new(new_scope), contents.clone()))
        },
        &Command::UserHere(ref arg_names, ref contents) => { 
            let inner_scope = scope;
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
            expand_fully(&ValueClosure(Rc::new(new_scope), contents.clone()))
 
            // todo handle args
            // let closure = ValueClosure(scope.clone(), cmd_data.clone());
            // aeval(scope.clone(), &Command::User(arg_names.clone(), closure), args)
        },
        &Command::Define => {
            // get arguments/name from param 1
            match (&args[0], &args[1], &args[2]) {
                (&Str(ref name_args),
                &Closure(ValueClosure(_, ref command_text)),
                &Closure(ValueClosure(_, ref to_expand))) => {
                    // TODO: custom arguments, more tests
                    let mut parts = vec![];
                    let mut params = vec![];
                    let na_str = name_args;
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
                        command_text.clone()
                    ));
                    expand_fully(&ValueClosure(Rc::new(new_scope), to_expand.clone()))
                },
                _ => {
                    panic!("Invalid state")
                }
            }
        },
        &Command::IfEq => {
            match (&args[0], &args[1], &args[2], &args[3]) {
                /*(ref a, ref b, &Closure(ref if_true), &Closure(ref if_false)) => {
                     if a == b { expand_fully(if_true.clone()) }
                     else { expand_fully(if_false.clone()) }
                },*/
                _ => { panic!("Invalid :("); }
            }
        }
    };
   return List(atoms.into_iter().map(|x| {
       match x {
           Char(x) => Str(x.to_string()),
           Val(x) => x
        }
    }).collect())
}


fn atom_to_char(atom: &Atom) -> Option<char> {
    match atom {
        &Char(c) => Some(c),
        _ => None
    }
}

#[derive(Clone, Debug)]
enum Token<'f> {
    Ident(String),
    StartParen,
    EndParen,
    RawParam(&'f [Atom]),
    Semicolon(&'f [Atom]),
    Text(&'f Atom)
}


fn parse_command<'f>(mut values: &'f [Atom], scope: Rc<Scope>) -> (Vec<Token<'f>>, &'f [Atom]) {
    let mut tokens = vec![];
    let mut parts : Vec<CommandPart> = vec![];
    let mut paren_level : u8 = 0;
    let mut ident = "".to_owned();
    // TODO: multi-part commands, variadic macros (may never impl - too error prone)
    while let Some((next, v)) = values.split_first() {
        if let Some(chr) = atom_to_char(next) {
            if chr.is_alphabetic() || chr == '_' {
                ident.push(chr);
                values = v;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    tokens.push(Token::Ident(ident.clone()));
    println!("AT {:?} REST {:?}", ident, values);
    parts.push(Ident(ident));
    while !scope.commands.contains_key(&parts) {
        let (next, v) = values.split_first().unwrap();
        let chr = atom_to_char(next);
        if chr.map(|x| x.is_whitespace()) == Some(true) {
            values = v;
        } else if chr == Some('(') {
            tokens.push(Token::StartParen);
            let (inner_tokens, future_atoms) = parse_text(v, 1, scope.clone());
            parts.push(Param);
            tokens.extend(inner_tokens.into_iter());
            tokens.push(Token::EndParen);
            values = future_atoms;

        } else if chr == Some(';') {
            parts.push(Param);
            tokens.push(Token::Semicolon(v));
            values = v;
            break;
        } else if chr == Some('{') {
            let mut raw_level = 0;
            let mut pos = 0;
            while let Some(next) = values.get(pos) {
                match (raw_level, atom_to_char(next)) {
                    (_, Some('{')) => { pos += 1; raw_level += 1 }
                    (1, Some('}')) => { break; }
                    (_, Some('}')) => { pos += 1; raw_level -= 1; }
                    (_, _) => { pos += 1; }
                }
            }
            parts.push(Param);
            tokens.push(Token::RawParam(&values[1..pos]));
            values = &values[(pos+1)..];
        }
        println!("LOOKING... {:?}", parts);
    }
    (tokens, values)
}

fn parse_text(mut values: &[Atom], call_level: u8, scope: Rc<Scope>) -> (Vec<Token>, &[Atom]) {
    let mut tokens = vec![];
    let mut paren_level : u8 = 0;
    while let Some((next, v)) = values.split_first() {
        let chr = atom_to_char(next);
        if chr == Some(scope.sigil) {
            let (cmd, rest) = parse_command(&values[1..], scope.clone());
            println!("PARSED {:?} {:?} WITH {:?}", paren_level, cmd, rest);
            tokens.extend(cmd.into_iter());
            values = rest;
        } else if chr == Some('(') {
            paren_level += 1;
            values = v;
        } else if chr == Some(')') {
            if paren_level > 0 {
                paren_level -= 1;
                values = v;
            } else if call_level > 0 {
                values = v;
                break;
            } else {
                panic!("Weird end paren");
                tokens.push(Token::Text(next));
                values = v;
            }
        } else {
            tokens.push(Token::Text(next));
            values = v;
        }
    }
    (tokens, values)
}

fn new_parse(&ValueClosure(ref scope, ref values): &ValueClosure)
        -> Vec<Token> {
    let call_stack : Vec<Option<Vec<CommandPart>>> = vec! [];
    parse_text(values, 0, scope.clone()).0
}

fn parse(&ValueClosure(ref scope, ref values): &ValueClosure) -> Vec<(ScanState, Range<usize>)> {

    // Allow nested macroexpansion (get order right -- 'inner first' for most params,
    // 'outer first' for lazy/semi params. some inner-first commands will return stuff that needs
    // to be re-expanded, if a ';'-command - but does this affect parallelism? etc)

    // tODO: this is all super slow, and has way too much copying

    let parsed = values
    .iter()
    .enumerate()
    .scan(ScanState::Text, |state, (idx, v)| {
        let ch = match v {
            &Char(c) => Some(c),
            _ => None
        };
        let sigil = scope.sigil;
        let is_white = ch.map(|c| c.is_whitespace()).unwrap_or(false);
        *state = match (*state, false, ch) {
            (Text, _, Some(c)) => {
                if c == sigil {
                    StartSigil
                } else {
                    Text
                }
            },
            (Text, _, _) => { Text },

            (Sigil, _, Some(_))
            | (StartSigil, _, Some(_)) => { CommandName },
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
    .chain(std::iter::once((End, &Char(' '), 0)))
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
    parsed
}

enum Chunk<'f> {
    CommandChunk(Vec<(ScanState, &'f [Atom])>),
    TextChunk(&'f Atom)
}
use Chunk::*;

fn arg_for_chunk<'f>(chunk: &Chunk<'f>, scope: Rc<Scope>) -> Vec<Value> {
    match chunk {
        &CommandChunk(ref parts) => { 
            parts.iter().flat_map(|&(ref state, ref vals)| {
                match *state {
                    Parens(_) => Some(match &vals[0] {
                        &Val(ref x) => x.clone(),
                        &Char(_) => { Str(vals.iter().map(|x| { x.serialize() }).collect::<String>()  ) }
                    }),
                    RawParens(_)
                    | Semicolon => Some(Closure(ValueClosure(scope.clone(),vals.to_vec()))),
                    _ => None
                }
            }).collect()
        },
        _ => {panic!() }
    }
}

fn get_chunks<'f>(parsed : &'f Vec<(ScanState, Range<usize>)>, values: &'f Vec<Atom>, scope: Rc<Scope>) -> Vec<Chunk<'f>> {
    parsed.split(|&(s, _)| { s == StartSigil })
          .map(|part| { part.iter().map(|&(ref s, ref x)| { (*s, &values[x.clone()]) }).collect::<Vec<(ScanState, &[Atom])>>() })
            .flat_map(|parts| {
                let mut chunks = vec![];
                let mut pos = 0;
                if parts[pos].0 == Start {
                    while pos < parts.len() && parts[pos].0 != StartSigil && parts[pos].0 != Sigil {
                        chunks.push(TextChunk(&values[pos]));
                        pos += 1;
                    }
                }
                while pos < parts.len() {
                    let mut current_slice = &parts[pos..];

                     // note - quadratic :(
                    let oldparts = parts.clone();
                    if(!scope.commands.contains_key(&
                            current_slice.iter().flat_map(|&(ref i, ref x)| { part_for_scan(*i, x) }).collect::<Vec<CommandPart>>()
                    )) {
                        while !current_slice.is_empty() { 
                            current_slice = current_slice.split_last().unwrap().1;
                            if(scope.commands.contains_key(&
                                current_slice.iter().flat_map(|&(ref i, ref x)| { part_for_scan(*i, x) }).collect::<Vec<CommandPart>>()
                            )) {
                                break;
                            }                           
                        }
                        
                        if current_slice.is_empty() {
                            panic!("Could not find command... {:?} IE {:?} IN {:?}", oldparts, oldparts.iter().flat_map(|&(ref i, ref x)| { part_for_scan(*i, x) }).collect::<Vec<CommandPart>>(), scope);
                        } else {
                            current_slice = current_slice.split_last().unwrap().1;
                        }
                    }
                    // Hacky hacky hack
                    while {
                        match current_slice[current_slice.len()-1].0 {
                            CloseParen
                            | CommandName
                            | Semicolon
                            => {
                                false
                            },
                            Halt => {panic!() },
                            x => {
                                println!("STRIPPING {:?}", x);
                                current_slice = current_slice.split_last().unwrap().1;
                                true
                            }
                        }
                    } {}
                    chunks.push(CommandChunk(current_slice
                        .iter()
                        .map(|&(ref s, x)| { (*s, x) })
                        .collect()));
                    
                    pos += current_slice.len();
                    while pos < parts.len() && parts[pos].0 != Sigil {
                        chunks.push(TextChunk(&values[pos]));
                        pos += 1;
                    }
                }
                return chunks;
            })
    .collect()
}

fn expand_chunk<'f>(chunk: &Chunk<'f>, scope: Rc<Scope>) -> Atom {
    match chunk {
        &TextChunk(v) => { v.clone() },
        &CommandChunk(ref parts) => {
            let p = parts.iter().flat_map(|&(ref i, ref x)|{ part_for_scan(*i, x) }).collect::<Vec<CommandPart>>();
                println!("P {:?}", p);
            Val(
                eval(
                    scope.clone(),
                    scope.commands.get(
                        &(
                            p
                        )[..]
                    ).unwrap(),
                    &arg_for_chunk(&chunk, scope.clone())[..]
                )
            )
        }
    }
}

fn expand_fully(closure: &ValueClosure)
    -> Vec<Atom> {
    let mut parsed = parse(closure);
    let &ValueClosure(ref scope, ref vold) = closure;
    let mut values = vold.clone();

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
    while let Some(halter) = parsed.iter().position(|&(s, _)| { s == Halt }).map(|pos| { parsed[pos].1.start }) {
        std::mem::drop(parsed); // a test
        let mut v = values.clone();
        let mut after = v.split_off(halter);
        println!("HERE {:?} {:?}", v, after);
        let closure = ValueClosure(scope.clone(), after.clone());
        let iparse = parse(&closure);
        let new_chunks = get_chunks(&iparse, &after, scope.clone());
        // FIXME: expand_chunk runs even if halt received
        v.push(expand_chunk(&new_chunks[0], scope.clone()));
        v.extend_from_slice((&after[(match &new_chunks[0]{
            &CommandChunk(ref x) => x.len(),
            _ => { panic!() }
        })..]));
        values = v.clone();
        parsed = parse(&ValueClosure(scope.clone(), v));
    }
    return get_chunks(&parsed, &values, scope.clone()).into_iter().map(|x| {
        expand_chunk(&x, scope.clone())
    }).collect();
    
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

fn default_scope() -> Scope {
    let mut scope = Scope {
        sigil: '#',
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
    );scope.commands.insert(vec![ Ident("z".to_owned())],
        Command::Rescope // a test
    );
    scope
}

fn expand(atoms : Vec<Atom>) -> Vec<Atom> {
    println!("Expand...");
    std::io::stdout().flush().unwrap();
    expand_fully(&ValueClosure(Rc::new(default_scope()), atoms))
    // note - make sure recursive macro defs work
}

impl Atom {
    fn serialize(&self) -> String {
        (match self {
            &Char(x) => x.to_string(),
            &Val(Str(ref x)) => x.clone(),
            &Val(Tagged(_, ref x)) => {
                Val(*(x.clone())).serialize()
            },
            &Val(ref x) => { panic!("Cannot serialize closure: {:?}", x); }
        })
    }
}


#[test]
fn it_works() {
    println!("{:?}", new_parse(&ValueClosure(Rc::new(default_scope()),
            "(#expand(#expand(#z))#expand{#expand{#z}})".chars().map(|x| { Char(x) })
            .collect::<Vec<Atom>>()
    )));
    /*
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = expand(chars);
    let out = results.iter().map(|x| { x.serialize() }).collect::<String>();
    println!("||\n{}||", out);
    */
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    println!("Hello, world!");
}
