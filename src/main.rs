
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
// TODO: test if_eq, handle recursive defs.

// TODO fix bugs in test - is newline behavior desirable?
// bigger issue: 'new world order' duplicated

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


fn eval<'c, 'v>(scope: Rc<Scope>, command: &'c Command, args: &'v [Value]) -> Value {
   match command {
        &Command::Rescope => {
            match(&args[0], &args[1]) {
                (&Closure(ValueClosure(ref inner_scope, _)), &Closure(ValueClosure(_, ref contents))) => {
                     Closure(ValueClosure(inner_scope.clone(), contents.clone()))
                },
                _ => {panic!() }
            }
        },
        &Command::Expand => {
            match &args[0] {
                &Closure(ref c) => {
                    let rv = retval_to_val(new_expand(c));
                    println!("GIVING {:?}", rv);
                    rv
                },
                _ => {panic!("ARG {:?}", args[0]); }
            }
        }
        &Command::Immediate(ref x) => {
            x.clone()
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
            retval_to_val(new_expand(&ValueClosure(Rc::new(new_scope), contents.clone())))
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
            retval_to_val(new_expand(&ValueClosure(Rc::new(new_scope), contents.clone())))
 
            // todo handle args
            // let closure = ValueClosure(scope.clone(), cmd_data.clone());
            // aeval(scope.clone(), &Command::User(arg_names.clone(), closure), args)
        },
        &Command::Define => {
            // get arguments/name from param 1
            match (&args[0], &args[1], &args[2]) {
                (&Str(ref name_args),
                &Closure(ref closure),
                &Closure(ValueClosure(_, ref to_expand))) => {
                    if name_args.is_empty() {
                        panic!("Empty define");
                    }
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
                    new_scope.commands.insert(parts, Command::User(params,
                        // TODO: fix scpoe issues
                        closure.clone()
                    ));
                    retval_to_val(new_expand(&ValueClosure(Rc::new(new_scope), to_expand.clone())))
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
    }
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
    Text(&'f Atom),
    OwnedText(Atom)
}


#[derive(Debug)]
enum ParseEntry {
    Text(u8, bool), // bool is true if in a call
    Command(Vec<CommandPart>)
}

fn parse_text(mut values: &[Atom], call_level: u8, scope: Rc<Scope>) -> (Vec<Token>, &[Atom]) {
    let mut tokens = vec![];
    let mut stack : Vec<ParseEntry> = vec![
        ParseEntry::Text(0, false)
    ];
    while let Some(current) = stack.pop() { match current {
        ParseEntry::Command(mut parts) => {
            // TODO: multi-part commands, variadic macros (may never impl - too error prone)
            // TODO: breaks intermacro text
            if scope.commands.contains_key(&parts) { 
                // continue to next work item
            } else if parts.len() == 0 {
                println!("Hell B {:?}", values);

                let mut ident = "".to_owned();
                while let Some((next, v)) = values.split_first() {
                    if let Some(chr) = atom_to_char(next) {
                        if chr.is_alphabetic() || chr == '_' || chr == '#' {
                            println!("HERE");
                            ident.push(chr);
                            values = v;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if !ident.is_empty() {
                    println!("I think I can: {:?}", ident);
                    tokens.push(Token::Ident(ident.clone()));
                    parts.push(Ident(ident));
                    stack.push(ParseEntry::Command(parts));
                    println!("STACK: {:?}", stack);
                } else {
                    stack.push(ParseEntry::Command(parts));
                    panic!("Other Hell {:?} {:?} {:?}", values, ident, stack);
                }
            } else {
                println!("Hell A");
                let (next, v) = values.split_first().unwrap();
                let chr = atom_to_char(next);
                if chr.map(|x| x.is_whitespace()) == Some(true) {
                    println!("Boring!");
                    values = v;
                    stack.push(ParseEntry::Command(parts));
                } else if chr == Some('(') {
                    tokens.push(Token::StartParen);
                    values = v;
                    println!("Argument: X{:?}X", v);
                    stack.push(ParseEntry::Command(parts));
                    stack.push(ParseEntry::Text(0, true));
                    println!("Into the future...");
                } else if chr == Some(')') {
                    println!("hello world");
                    values = v;
                    println!("Kicking: {:?}", Token::EndParen);
                    tokens.push(Token::EndParen);
                    parts.push(Param);
                    stack.push(ParseEntry::Command(parts));
                } else if chr == Some(';') {
                    println!("hello world 2");
                    parts.push(Param);
                    tokens.push(Token::Semicolon(v));
                    values = &[];
                    break;
                } else if chr == Some('{') {
                    println!("hello world 3");
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
                    println!("RAWPAR {:?}", &values[1..pos]);
                    values = &values[(pos+1)..];
                    stack.push(ParseEntry::Command(parts));
                } else {
                    panic!("Failed {:?} {:?}", parts, scope);
                }
            }
        },
        ParseEntry::Text(paren_level, in_call) => {
            if let Some((next, v)) = values.split_first() {
                let chr = atom_to_char(next);
                if chr == Some(scope.sigil) {
                    values = v;
                    stack.push(ParseEntry::Text(paren_level, in_call));
                    stack.push(ParseEntry::Command(vec![]));
                } else if chr == Some('(') {
                    values = v;
                    stack.push(ParseEntry::Text(paren_level + 1, in_call));
                } else if chr == Some(')') {
                    if paren_level > 0 {
                        values = v;
                        stack.push(ParseEntry::Text(paren_level - 1, in_call));
                    } else if in_call {
                    } else {
                        tokens.push(Token::Text(next));
                        values = v;
                        stack.push(ParseEntry::Text(paren_level, in_call));
                    }
                } else {
                    println!("Kicking: {:?}", next);
                    tokens.push(Token::Text(next));
                    values = v;
                    stack.push(ParseEntry::Text(paren_level, in_call));
                }
            }
        }
    } }
    println!("LEFT {:?}", tokens);
    (tokens, values)
}

fn new_parse(&ValueClosure(ref scope, ref values): &ValueClosure) -> Vec<Token> {
    parse_text(values, 0, scope.clone()).0
}

impl Value {
    fn to_string(&self) -> String {
        match self {
            &Str(ref x) => x.to_owned(),
            &Tagged(_, ref x) => x.to_string(),
            _ => {panic!("Cannot coerce value into string!")}
        }
    }
}

fn atoms_to_string<'f>(atoms: &'f [Atom]) -> String {
    atoms.into_iter().fold("".to_owned(), |mut s, atom| { match atom {
        &Char(x) => { s.push(x) }
        &Val(ref x) => { s.push_str(&x.to_string()[..]) }
    }; s })
}

fn retval_to_val(atoms: Vec<Atom>) -> Value {
    let has_non_whitespace = atoms.iter().any(|x| match x {
        &Char(x) => !x.is_whitespace(),
        &Val(_) => false
    });
    let mut non_strings = atoms.iter().flat_map(|atom| { match atom {
        &Char(_) => None,
        &Val(Str(_)) => None,
        &Val(ref val) => Some(val)
    } });
    match (has_non_whitespace, non_strings.next()) {
        (true, _) | (false, None) => {
            Str(atoms_to_string(&atoms[..]))
        }
        (false, Some(x)) => {
            if non_strings.next().is_some() {
                panic!("Cannot create implicit lists in macro return values. Got: {:?}", atoms);
            } else {
                x.clone()
            }
        }
    }
}

fn parens_to_arg(tokens: Vec<Token>) -> Value {
    let test = tokens.iter().map(|x| { match x {
        &Token::Text(&Char(ref x)) => x.clone(),
        &Token::OwnedText(Char(ref x)) => x.clone(),
        _ => {'%'}
    } }).collect::<String>();
    if test.contains("%") {
       let vals = tokens.iter().flat_map(|x| { match x {
            &Token::Text(&Val(ref x)) => { Some(x.clone()) },
            &Token::OwnedText(Val(ref x)) => { Some(x.clone()) }, 
            &Token::Text(&Char(' ')) => {None },
            &Token::OwnedText(Char(' ')) => { None },
            _ => { panic!("NYI {:?}", tokens) }
        } }).collect::<Vec<Value>>();
       if vals.len() == 1 {
           vals[0].clone()
               // TODO fix this up
        } else {
            Value::List(vals)
        }
    } else {
        Value::Str(test)
    }
}

fn raw_to_arg(tokens: &[Atom], scope: Rc<Scope>) -> Value {
    return Value::Closure(ValueClosure(scope, tokens.to_vec()))
}

fn new_expand(closure: &ValueClosure) -> Vec<Atom> {
    let parsed = new_parse(closure);
    let &ValueClosure(ref scope, _) = closure;
    expand_parsed(parsed, scope.clone())
}

fn expand_parsed(mut parsed: Vec<Token>, scope: Rc<Scope>) -> Vec<Atom> {
    loop {
        let mut last = None;
        for idx in 0..(parsed.len()) {
            if let Token::Ident(_) = parsed[idx] {
                last = Some(idx);
                println!("STARTIDX {:?}", idx);
            }
        }
        match last {
            Some(start_idx) => {
                let mut in_parens : Option<Vec<Token>> = None;
                let mut parts : Vec<CommandPart> = vec![];
                let mut results : Vec<Value> = vec![];
                let mut out = None;
                for idx in start_idx..parsed.len() {
                    println!("Adding {:?}", parsed[idx]);
                    // NB cannot contain any other commands - hence no nested parens
                    match (&parsed[idx], in_parens.is_some()) {
                        (&Token::Ident(ref id), false) => { parts.push(Ident( id.clone() )) },
                        (&Token::StartParen, false) => { in_parens = Some(vec![]); },
                        (&Token::EndParen, true) => {
                            results.push(parens_to_arg(in_parens.clone().unwrap()));
                            in_parens = None;
                            parts.push(Param);
                        },
                        (x, true) => {
                            in_parens.as_mut().unwrap().push(x.clone());
                        },
                        (&Token::RawParam(c), false) => {
                            results.push(raw_to_arg(c, scope.clone() ));
                            parts.push(Param); },
                        (&Token::Semicolon(c), false) => { results.push(raw_to_arg(c, scope.clone() )); parts.push(Param); },
                        (_, _) => { panic!("Could not handle {:?} {:?} in {:?}", parsed[idx], in_parens.clone(), parsed); }
                    }
                    println!("TRYING {:?} {:?}", parts, scope);
                    if let Some(command) = scope.commands.get(&parts) {
                        let ev = eval(
                            scope.clone(),
                            command,
                            &results[..]
                        );
                        println!("EV {:?}", ev);
                        out = Some((idx, Val(ev)));
                        break;
                    }
                }
                println!("GOING FROM {:?}", parsed);
                if let Some((end_idx, result)) = out {
                    // may need to re-parse in some cases?
                    let mut new_atoms = parsed[..start_idx].to_vec();
                    new_atoms.push(Token::OwnedText(result));
                    new_atoms.extend(parsed[(end_idx+1)..].to_vec());
                    println!("GOING TO {:?}", new_atoms);
                    // NOTE: with ;-commands, must *reparse* the whole string,
                    // in case we got interrupted mid-argument TODO
                    parsed = new_atoms;
               } else {
                    panic!("Failure? {:?} {:?} {:?}", parts, results, parsed);
                }
            },
            None => {
                return parsed.into_iter().map(|x| {
                    match x {
                        Token::Text(c) => c.clone(),
                        Token::OwnedText(c) => c,
                        _ => { panic!("Failure... {:?}", x); }
                    }
                }).collect::<Vec<Atom>>()
            }
        }
    }
}

//TODO handle EOF propelry
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
    ); 
    scope
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
   
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = new_expand(&ValueClosure(Rc::new(default_scope()), chars));
    let out = results.iter().map(|x| { x.serialize() }).collect::<String>();
    println!("||\n{}||", out);
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    // TODO cli
    println!("Hello, world!");
}
