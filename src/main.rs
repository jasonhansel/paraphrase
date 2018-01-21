
#![allow(dead_code)]
// ^ rls doesn't handle tests correctly





// TODO: some tests are failing (removing first character spuriously)
// TODO: Back to copying
//
//
//

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


// nb also write stdlib.

fn read_file<'s>(mut string: &'s mut String, path: &str) -> Result<Rope<'s>, Error> {
    println!("Reading...");
    std::io::stdout().flush().unwrap();
    let mut file = File::open(path)?;
    file.read_to_string(string)?;
    // TODO use Borrowed always
    Ok(Rope::Leaf(Leaf::Chr(Cow::Borrowed(&string[..]))))
}


fn eval<'c, 'v>(cmd_scope: &'v Rc<Scope>, scope: Rc<Scope>, command: Vec<CommandPart>, args: Vec<Leaf<'v>>) -> Leaf<'v> {
   match cmd_scope.commands.get(&command).unwrap() {
        &Command::InOther(ref other_scope) => {
           eval( other_scope, scope, command, args)
        },
        &Command::Rescope => {
            match (args[0].to_val(), args[1].to_val()) {
                (&Closure(ValueClosure(ref inner_scope, _)),
                &Closure(ValueClosure(_, ref contents))) => {
                    Leaf::Own(Box::from(
                         Closure(ValueClosure(inner_scope.clone(), Box::new(contents.make_static() )))
                    ))
                },
                _ => {panic!() }
            }
        },
        &Command::Expand => {
            match args[0].to_val() {
                &Closure(ValueClosure(ref scope, ref contents)) => {
                    retval_to_val(new_expand(scope, contents.make_static() ))
                },
                _ => {panic!("ARG {:?}", args[0]); }
            }
        }
        &Command::Immediate(ref x) => {
            Leaf::Val(x)
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
                Rc::get_mut(&mut new_scope)
                    .unwrap()
                    .commands
                    .insert(vec![Ident(name.to_owned() )],
                    Command::Immediate( arg.to_val().make_static() )
                );
            }
            retval_to_val(new_expand(&new_scope, contents.dupe() ))
        },
       &Command::Define => {
            // get arguments/name from param 1
            match (args[0].to_val(), args[1].to_val(), args[2].to_val()) {
                (&Str(ref name_args),
                &Closure(ref closure),
                &Closure(ValueClosure(_, ref to_expand))) => {

                    if name_args.is_empty() {
                        panic!("Empty define: {:?}", args);
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
                    Rc::get_mut(&mut new_scope)
                    .unwrap()
                    .commands
                    .insert(parts,
                    Command::User(params,
                        // TODO: fix scpoe issues
                        closure.force_clone()
                    ));
                    // TODO avoid clone here
                    retval_to_val(new_expand(&new_scope, to_expand.make_static() ))
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

pub trait TokenVisitor<'s, 't : 's> {
    fn start_command(&mut self, Cow<'s, str>);
    fn end_command(&mut self, Vec<CommandPart>);
    fn start_paren(&mut self);
    fn end_paren(&mut self);
    fn raw_param(&mut self, Rope<'s>);
    fn semi_param(&mut self, Rope<'s>) -> Rope<'s>;
    fn text(&mut self, Rope<'s>);
    fn done(&mut self);
}

enum Instr<'s> {
    Push(Rope<'s>),
    Concat(u16),
    Call(u16, Vec<CommandPart>),
    Close(Rope<'s>)
}

struct Expander<'s> {
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr<'s>>
}

// are ropes stil necessary? basically just using them as linked lists now, I think


impl<'s> Expander<'s> {
    fn new() -> Expander<'s> {
        Expander {
            parens: vec![0],
            calls: vec![],
            instr: vec![]
        }
    }
    fn do_expand(self, scope: &'s Rc<Scope>) -> Leaf<'s> {
        let mut stack : Vec<Rope<'s>> = vec![];
        for i in self.instr.into_iter() { match i {
            Instr::Push(r) => { stack.push(r); },
            Instr::Concat(cnt) => {
                let mut new_rope = Rope::new();
                let idx = stack.len() - cnt as usize;
                for item in stack.split_off(idx) {
                    new_rope = new_rope.concat(item.make_static());
                }
                stack.push(
                    new_rope
                );
            },
            Instr::Close(r) => {
                let stat = r.make_static();
                stack.push(
                    Rope::Leaf( Leaf::Own(
                            Box::new(
                                Value::Closure ( ValueClosure( scope.clone(), Box::new(stat)  )) ))
                        )
                );
            }
            Instr::Call(cnt, cmd) => {
                let idx = stack.len() - cnt as usize;
                let args = stack.drain(idx..)
                    .map(|x| { x.to_leaf(true) })
                    .collect::<Vec<_>>();
                println!("ARGDAT {:?} {:?}", cnt, cmd);
                let result = eval(scope, scope.clone(), cmd, args);
                println!("RES {:?}", result);
                stack.push(Rope::Leaf( result ));
            }

        } }
        if stack.len() != 1 {
            panic!("Wrong stack size!");
        }
        stack.remove(0).to_leaf(false)
    }

}

impl<'s,'t:'s> TokenVisitor<'s, 't> for Expander<'s> {
    fn start_command(&mut self, _: Cow<'s, str>) {
        self.calls.push(0);
    }
    fn end_command(&mut self, cmd: Vec<CommandPart>) {
        if let Some(l) = self.parens.last_mut() { *l += 1; }
        self.instr.push(Instr::Call(self.calls.pop().unwrap(), cmd));
    }
    fn start_paren(&mut self) {
        println!("START PAR");
        self.parens.push(0);
    }
    fn end_paren(&mut self) {
        *( self.calls.last_mut().unwrap() ) += 1;
        println!("END PAR");
        self.instr.push(Instr::Concat(self.parens.pop().unwrap()));
    }
    fn raw_param(&mut self, rope: Rope<'s>) {
        *( self.calls.last_mut().unwrap() ) += 1;
        self.instr.push(Instr::Close(rope));
    }
    fn semi_param(&mut self, rope: Rope<'s>) -> Rope<'s> {
        *( self.calls.last_mut().unwrap() ) += 1;
        // TODO inner expansion
        self.instr.push(Instr::Close(rope));
        println!("ATSEMI {:?} {:?}", self.calls, self.parens);
        Rope::new()
    }
    fn text(&mut self, rope: Rope<'s>) {
        if let Some(l) = self.parens.last_mut() { *l += 1; }
        println!("TXT {:?}", rope);
        self.instr.push(Instr::Push(rope));
    }
    fn done(&mut self) {
        self.instr.push(Instr::Concat(self.parens.pop().unwrap()));
        if self.calls.len() > 0 || self.parens.len() > 0 {
            panic!("Unbalanced {:?} {:?}", self.calls, self.parens);
        }
    }
}



// TODO fix perf - rem compile optimized, stop storing characters separately
// TODO note: can't parse closures in advance because of Rescope
// TODO: allow includes - will be tricky to avoid copying owned characters around
fn parse<'f, 'r, 's : 'r>(
    scope: Rc<Scope>,
    mut rope: Rope<'s>,
    visitor: &mut TokenVisitor<'s,'s>
) {
    let mut stack : Vec<ParseEntry> = vec![
        ParseEntry::Text(0, false)
    ];
    while let Some(current) = stack.pop() { match current {
        ParseEntry::Command(mut parts) => {
            // TODO: multi-part commands, variadic macros (may never impl - too error prone)
            // TODO: breaks intermacro text
            if let Some(cmd) = scope.commands.get(&parts) {
                println!("COMMAND DONE {:?}", parts);
                visitor.end_command(parts.split_off(0));
                // continue to next work item
            } else if parts.len() == 0 {
                println!("HERE");
                let mut ident = rope.split_at(false, &mut |chr : char| {
                    println!("CHECKING {:?}", chr);
                    if chr.is_alphabetic() || chr == '_' || chr == scope.sigil {
                        // dumb check for sigil /here
                        false
                    } else {
                        true
                    }
                });

                if let Some(mut id) = ident {
                    id.split_char(); // get rid of sigil
                    parts.push(Ident( id.to_str().unwrap().into_owned() ));
                    visitor.start_command(id.to_str().unwrap());
                    stack.push(ParseEntry::Command(parts));
                } else {
                    rope.split_char(); // get rid of sigil
                    parts.push(Ident( rope.to_str().unwrap().into_owned() ));
                    visitor.start_command(rope.to_str().unwrap());
                    stack.push(ParseEntry::Command(parts));
                    rope = Rope::new();
                }

                } else {
                rope.split_at(false, &mut |ch : char| {
                    println!("SCANW {:?}", ch);
                    if ch.is_whitespace() {
                        return false;
                    } else {
                        return true;
                    }
                }).unwrap();

                let chr = rope.split_char().unwrap();
                if chr == '(' {
                    visitor.start_paren();
                    stack.push(ParseEntry::Command(parts));
                    stack.push(ParseEntry::Text(0, true));
                } else if chr == ')' {
                    visitor.end_paren();
                    parts.push(Param);
                    stack.push(ParseEntry::Command(parts));
                } else if chr == ';' {
                    println!("HIT SEMI");
                    parts.push(Param);
                    let _ = visitor.semi_param(rope);
                    //stack.push(ParseEntry::Command(parts));
                    if let Some(cmd) = scope.commands.get(&parts) {
                        println!("COMMAND DONE {:?}", parts);
                        visitor.end_command(parts.split_off(0));
                        // continue to next work item
                    } 
                    break;
                } else if chr == '{' {
                    let mut raw_level = 1;
                    let param = rope.split_at(true, &mut |ch| { 
                        println!("RAW {:?} {:?}", ch, raw_level);
                        raw_level += match ch {
                            '{' => 1,
                            '}' => -1,
                            _ => 0
                        };
                        raw_level == 0
                    }).unwrap();
                    rope.split_char();
                    println!("REST {:?}", rope);
                    parts.push(Param);
                    visitor.raw_param(param);
                    stack.push(ParseEntry::Command(parts));
                } else {
                    panic!("Failed {:?} {:?} {:?}", rope, parts, chr);
                }
            }
        },
        ParseEntry::Text(mut paren_level, in_call) => {
            let mut pos = 0;
            let prefix = rope.split_at(true, &mut |x| { 
                
                println!("SCAN {:?}", x);
                match x{
                    '(' => {
                        paren_level += 1;
                        false
                    },
                    ')' => {
                        if paren_level > 0 {
                            paren_level -= 1;
                            false
                        } else if in_call {
                            println!("HAEC");
                            true
                        } else {
                            false
                        }
                    }
                    chr => { 
                        if chr == (scope.sigil) {
                            println!("HOC");
                            true
                        } else {
                            false
                        }
                    }
                } });
            if let Some(p) = prefix {
                if !p.is_empty() {
                    visitor.text(p);
                }
            
                match rope.get_char() {
                    Some(')') => {
                    },
                    Some(x) => {
                        if x != scope.sigil { panic!("Unexpected halt at: {:?}", x); }
                        stack.push(ParseEntry::Text(paren_level, in_call));
                        stack.push(ParseEntry::Command(vec![]));
                    },
                    None => {
                        println!("TEST");
                    }
                }
            } else {
                visitor.text(rope);
                break;
            }
        }
    } }
    visitor.done()
}

impl<'s> Value<'s> {
    fn to_string(&self) -> Cow<'s, str> {
        match self {
            &Str(ref x) => x.clone(),
            &Tagged(_, ref x) => x.to_string(),
            _ => {panic!("Cannot coerce value into string!")}
        }
    }
}

fn retval_to_val<'s>(rope: Leaf<'s>) -> Leaf<'static> {
    rope.make_static()
}


fn new_expand<'f, 'r : 'f>(scope: &'f Rc<Scope>, tokens: Rope<'f>) -> Leaf<'f> {
    let mut expander = Expander::new();
    parse(scope.clone(), tokens, &mut expander);
    expander.do_expand(&scope)
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

/*
impl Value {
    fn serialize(&self) -> String {
        match self {
            &Str(ref x) => x.clone(),
            &Tagged(_, ref x) => x.serialize(),
            _ => {panic!("Cannot serialize") }
        }
    }
}
impl<'s> Atom<'s> {
    fn serialize(&self) -> String {
        (match self {
            &Chars(ref x) => x.to_string(),
            &Val(ref x) => x.serialize()
        })
    }
}
*/

#[test]
fn it_works() {
    let mut s = String::new();
    let mut chars = read_file(&mut s, "tests/1-simple.pp").unwrap();
    let scope = Rc::new(default_scope());
    let results = new_expand(&scope, chars);
    println!("||\n{}||", results.to_str().unwrap());
    // ISSUE: extra whitespace at end of output
 //   assert_eq!(out, "Hello world!\n");
}

fn main() {
    // TODO cli
    println!("Hello, world!");
}
