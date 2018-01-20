
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
use std::borrow::Cow;
use std::borrow::Borrow;
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

fn read_file<'s>(path: &str) -> Result<Rope<'s>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    Rope::read_file(&mut file)
}


fn eval<'c, 'v>(scope: Rc<Scope>, command: &'c Command, args: &'v [Value]) -> Value {
   match command {
        &Command::Rescope => {
            match(&args[0], &args[1]) {
                (&Closure(ValueClosure(ref inner_scope, _)), &Closure(ValueClosure(_, ref contents))) => {
                     Closure(ValueClosure(inner_scope.clone(), contents.clone() ))
                },
                _ => {panic!() }
            }
        },
        &Command::Expand => {
            match &args[0] {
                &Closure(ValueClosure(ref scope, ref contents)) => {
                    retval_to_val(new_expand(scope.clone(), &mut contents.shallow_copy() ))
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
            for (name, arg) in arg_names.into_iter().zip( args.into_iter() ) {
                // should it always take no arguments?
                // sometimes it shouldn't be a <Vec>, at least (rather, it should be e.g. a closure
                // or a Tagged). coerce sometimes?
                new_scope.commands.insert(vec![Ident(name.to_owned() )], Command::Immediate( *arg ) );
            }
            retval_to_val(new_expand(Rc::new(new_scope), &mut contents.shallow_copy() ))
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
                new_scope.commands.insert(vec![Ident(name.to_owned())], Command::Immediate( *arg ) );
            }
            retval_to_val(new_expand(Rc::new(new_scope), &mut contents.shallow_copy() ))
 
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
                    // make_mut clones as nec.
                    let mut new_scope = dup_scope(scope);
                    // circular refs here?
                    new_scope.commands.insert(parts, Command::User(params,
                        // TODO: fix scpoe issues
                        closure.clone()
                    ));
                    // TODO avoid clone here
                    retval_to_val(new_expand(Rc::new(new_scope), &mut to_expand.shallow_copy() ))
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








#[derive(Debug)]
enum ParseEntry {
    Text(u8, bool), // bool is true if in a call
    Command(Vec<CommandPart>)
}



// TODO fix perf - rem compile optimized, stop storing characters separately
// TODO note: can't parse closures in advance because of Rescope

fn new_parse<'r, 's : 'r>(scope: Rc<Scope>, rope: &'s mut Rope<'s>) -> Vec<Token<'s>> {
    let mut tokens : Vec<Token<'s>> = vec![];
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
                let ident : Rope<'s> = rope.split_at(false,  &mut |chr : char| {
                    if chr.is_alphabetic() || chr == '_' || chr == scope.sigil {
                        // dumb check for sigil here
                        false
                    } else {
                        true
                    }
                }).unwrap();

     //           if !ident.is_empty() {
                    parts.push(Ident( ident.get_str().into_owned() ));
                    tokens.push(Token::CommandName( ident ));
                    stack.push(ParseEntry::Command(parts));
       //         } else {
         //           panic!("Could not identify invocation...");
           //     }
            } else {
                let search_result = rope.split_at(false, |ch : char| {
                    if ch.is_whitespace() {
                        return false;
                    } else {
                        return true;
                    }
                }).unwrap();
                let chr = rope.split_char().unwrap();
                if chr == '(' {
                    tokens.push(Token::StartParen);
                    stack.push(ParseEntry::Command(parts));
                    stack.push(ParseEntry::Text(0, true));
                } else if chr == ')' {
                    tokens.push(Token::EndParen);
                    parts.push(Param);
                    stack.push(ParseEntry::Command(parts));
                } else if chr == ';' {
                    parts.push(Param);
                    tokens.push(Token::Semicolon( rope.clone() ));
                    return tokens;
                } else if chr == '{' {
                    let mut raw_level = 0;
                    
                    let param = rope.split_at(true, |ch| { 
                        raw_level += match ch {
                            '{' => 1,
                            '}' => -1,
                            _ => 0
                        };
                        raw_level == 0
                    }).unwrap();
                    rope.split_char();
                    parts.push(Param);
                    tokens.push(Token::RawParam(
                        param
                    ));
                    stack.push(ParseEntry::Command(parts));
                } else {
                    panic!("Failed {:?} {:?}", parts, scope);
                }
            }
        },
        ParseEntry::Text(mut paren_level, in_call) => {
            let mut pos = 0;
            let prefix = rope.split_at(true, &mut |x| { match x{
                    '(' => {
                        paren_level += 1;
                        false
                    },
                    ')' => {
                        if paren_level > 0 {
                            paren_level -= 1;
                            false
                        } else if in_call {
                            true
                        } else {
                            false
                        }
                    }
                    chr => { 
                        if chr == (scope.sigil) {
                                                true
                        } else {
                            false
                        }
                    }
                } });
            if let Some(p) = prefix {
                if !p.is_empty() {
                    tokens.push(Token::Text(p));
                }
            }
            match rope.get_char() {
                Some(')') => {
                },
                Some(x) => {
                    if x != scope.sigil { panic!() }
                    stack.push(ParseEntry::Text(paren_level, in_call));
                    stack.push(ParseEntry::Command(vec![]));
                },
                None => {
                }
            }
        }
    } }
    tokens
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

fn atoms_to_string<'f, T: Borrow<str>, U: Borrow<Atom<T>>>(atoms: &'f [U]) -> String {
    atoms.into_iter().fold("".to_owned(), |mut s, atom| { match atom.borrow() {
        &Chars(ref x) => { s.push_str(x.borrow()) }
        &Val(ref x) => { s.push_str(&x.to_string()[..]) }
    }; s })
}

fn atoms_to_string_inner<'f, T: Borrow<str> + 'f, U: Borrow<&'f Atom<T>>>(atoms: &'f [U]) -> String {
    atoms.into_iter().fold("".to_owned(), |mut s, atom| { match atom.borrow() {
        &&Chars(ref x) => { s.push_str(x.borrow()) }
        &&Val(ref x) => { s.push_str(&x.to_string()[..]) }
    }; s })
}



fn retval_to_val<'s>(rope: Rope<'s>) -> Value {
    return rope_to_val(rope, false);
}
fn rope_to_val<'s>(rope: Rope<'s>, lists_allowed: bool) -> Value {
    let atoms = rope.atomize();
    let has_non_whitespace = atoms.iter().any(|x : &Atom<Cow<str>>| match x.borrow() {
        &Chars(ref x) => (x.trim_left().len() > 0),
        &Val(_) => false
    });
    let val_cnt : u32 = atoms.iter().map(|atom| { match atom {
        &Chars(_) => 0,
        &Val(_) => 1
    } }).sum();
    if has_non_whitespace || val_cnt == 0 {
        Str(atoms_to_string(&atoms[..]))
    } else if val_cnt > 1 {
        if lists_allowed {
            Value::List(atoms.into_iter().flat_map(|atom| { match atom {
                Chars(_) => None,
                Val(val) => Some(val)
            } }).collect())
        } else {
            panic!("Cannot create implicit lists in macro return values");
        }
    } else {
        for a in atoms { match a {
            Val(val) => { return val },
            _ => {}
        } }
        panic!()
    }
}

// TODO: can i use 'tagged' rope segments instead of token lists?
fn parens_to_arg<'f>(tokens: Vec<Token<'f>>) -> Value {
    let mut rope = Rope::new();
    for token in tokens { match token {
        Token::Text(x) => {
            rope = rope.concat(x)
        },
        _ => { panic!("Err") }
    } };
    rope_to_val(rope, true)
}

fn raw_to_arg<'f>(tokens: &Rope<'f>, scope: Rc<Scope>) -> Value {
    // Cloning is OK here
    return Value::Closure(ValueClosure(scope, Box::new(tokens.make_static())))
}

fn new_expand<'f>(scope: Rc<Scope>, tokens: &'f mut Rope<'f>) -> Rope<'f> {
    let parsed = new_parse(scope.clone(), tokens);
    expand_parsed(parsed, scope.clone())
}

// TODO decrease recursion, do more things purely in terms of ropes
fn expand_parsed<'f>(mut parsed: Vec<Token<'f>>, scope: Rc<Scope>) -> Rope<'f> {
    loop {
        let mut last = None;
        for idx in 0..(parsed.len()) {
            if let Token::CommandName(_) = parsed[idx] {
                last = Some(idx);
            }
        }
        match last {
            Some(start_idx) => {
                let mut in_parens : Option<Vec<Token>> = None;
                let mut parts : Vec<CommandPart> = vec![];
                let mut results : Vec<Value> = vec![];
                let mut out = None;
                for idx in start_idx..parsed.len() {
                    // NB cannot contain any other commands - hence no nested parens
                    match (&parsed[idx], in_parens.is_some()) {
                        (&Token::CommandName(ref id), false) => { parts.push(Ident( id.get_str().into_owned() )) },
                        (&Token::StartParen, false) => { in_parens = Some(vec![]); },
                        (&Token::EndParen, true) => {
                            results.push(parens_to_arg( in_parens.unwrap()  ));
                            in_parens = None;
                            parts.push(Param);
                        },
                        (&x, true) => {
                            in_parens.as_mut().unwrap().push(x);
                        },
                        (&Token::RawParam(ref c), false) => {
                            results.push(raw_to_arg(c, scope.clone() ));
                            parts.push(Param); },
                        (&Token::Semicolon(ref c), false) => { results.push(raw_to_arg(c, scope.clone() )); parts.push(Param); },
                        (_, _) => { panic!("Could not handle {:?} {:?} in {:?}", parsed[idx], in_parens, parsed); }
                    }
                    if let Some(command) = scope.commands.get(&parts) {
                        let ev = eval(
                            scope.clone(),
                            command,
                            &results[..]
                        );
                        out = Some((idx, ev));
                        break;
                    }
                }
                if let Some((end_idx, result)) = out {
                    // may need to re-parse in some cases? slow
                    parsed.drain(start_idx..(end_idx+1));
                    let r = Token::Text(Rope::Val(Cow::Owned(result)));
                    parsed.insert(start_idx, r);

                   // NOTE: with ;-commands, must *reparse* the whole string,
                    // in case we got interrupted mid-argument TODO
               } else {
                    panic!("Failure? {:?} {:?} {:?}", parts, results, parsed);
                }
            },
            None => {
                let mut result = Rope::new();
                for part in parsed {
                    match part {
                        Token::Text(x) => {
                            result = result.concat(x)
                        },
                        _ => { panic!() }
                    }
                }
                return result
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
impl Value {
    fn serialize(&self) -> String {
        match self {
            &Str(ref x) => x.clone(),
            &Tagged(_, ref x) => x.serialize(),
            _ => {panic!("Cannot serialize") }
        }
    }
}
impl<T : Borrow<str>> Atom<T> {
    fn serialize(&self) -> String {
        (match self {
            &Chars(ref x) => x.borrow().to_string(),
            &Val(ref x) => x.serialize()
        })
    }
}


#[test]
fn it_works() {
   
    let chars = read_file("tests/1-simple.pp").unwrap();
    let results = new_expand(Rc::new(default_scope()), &mut chars);
    let out = results.atomize().iter().map(|x| { x.serialize() }).collect::<String>();
    println!("||\n{}||", out);
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    // TODO cli
    println!("Hello, world!");
}
