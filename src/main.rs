

use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::{Read, Error};
use std::result::Result;
use std::borrow::BorrowMut;
use std::ops::Deref; 
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
        if(parts.len() == 0) {
            self.cmd = Some(cmd);
        } else {
            let mut subtree : &mut HashMap<CommandPart, Rc<CommandTrie>> = match &mut self.next {
                &mut None => { let n = HashMap::new(); self.next = Some(n); &mut n }
                &mut Some(ref mut n) => n
            };
            let mut c = subtree.entry(parts[0].clone()).or_insert(Default::default());
            c.insert(&parts[1..], cmd);
        }
    }
}

fn read_file(path: &str) -> Result<Vec<Value>, Error> {
    let mut x = String::new();
    File::open(path)?.read_to_string(&mut x)?;
    Ok(x.chars().map(|x| Value::Char(ValueChar(x))).collect())
}

fn expand_command(
    iter: &mut ValueList,
    cmd_here : Rc<CommandTrie>,
    scope: &Scope
) {
    // Allow nested macroexpansion (get order right -- 'inner first' for most params,
    // 'outer first' for lazy/semi params. some inner-first commands will return stuff that needs
    // to be re-expanded, if a ';'-command - but does this affect parallelism? etc)
   let &mut ValueList(values) = iter;
   let mut postwhite = values
        .into_iter()
        .skip_while(|x| {
            match x {
                &Value::Char(ValueChar(c)) => c.is_whitespace(),
                _ => false
            }
        })
        .peekable();
    // TODO: 'early' expansion, expansion to chars
    if let Some(ref tree) = cmd_here.next {
       if tree.contains_key(&Param) {
            let param = postwhite
            .by_ref()
            .scan(0, |bal, x| {
                *bal += match x {
                    Value::Char(ValueChar('(')) => 1,
                    Value::Char(ValueChar(')')) => -1,
                    _ => 0
                };
                Some((*bal, x))
            })
            .take_while(|&(bal, x)| {
                bal > 0
            })
            .map(|(bal, x)| { x })
            .collect::<Vec<Value>>();
            println!("PARAM {:?}", param);
            // todo: strip parens
            if param.len() > 0 {
                *iter = ValueList(postwhite.skip(param.len()).collect());
                return expand_command(iter,
                    tree.get(&Param).unwrap().clone(), scope);
            }
        }
       // Allow '##X' etc.
       if let Some(&Value::Char(c)) = postwhite.peek() {
           if c == scope.sigil {
                let cmd_name = postwhite
                .skip(1)
                .take_while(|x| {
                    match x {
                        &Char(ValueChar(c)) => c.is_alphabetic(),
                        _ => false
                    }
                })
                .fold("".to_owned(), |s, x| {
                    if let Char(ValueChar(c)) = x {
                        s.push(c);
                        return s;
                    } else {
                        panic!("Err");
                    }
                });
                if(cmd_name.len() > 0 && tree.contains_key(&Ident(cmd_name))) {
                    *iter = ValueList(postwhite.skip(1 + cmd_name.len()).collect());
                    // does string equality work as expected
                    return expand_command(iter,
                        tree.get(&Ident(cmd_name)).unwrap().clone(), scope);
                }
            }
        }
    }
    // Here, should: handle semicolons and perform expansion...
 
}

fn expand_text(vals: &mut ValueList, scope: Scope) {
    let &mut ValueList(ref mut values) = vals;
    if values.len() == 0 {
        // nothing to see here
        return;
    } else if let Char(c) = values[0] {
        if c == scope.sigil {
            // expand_command will expand *a* command (maybe not this one -- e.g.
            // it could be an inner command in one of the arguments). But it will
            // make progress.
            expand_command(vals, scope.commands.clone(), &scope);
            expand_text(vals, scope);
            return;
        }
    }
    let rest = values.clone();
    rest.remove(0);
    expand_text(&mut ValueList(rest), scope);
    values.truncate(1);
    values.extend(rest.into_iter());
}

fn expand(values: Vec<Value>) -> ValueList {
    let mut scope = Scope {
        sigil: ValueChar('#'),
        commands: Rc::new(CommandTrie::default())
    };
    scope.commands.insert(&(vec![ Ident("define".to_owned()), Param, Param, Param ])[..],
        ValueClosure( Rc::new(scope.clone()), ValueList( vec! [Value::Char(ValueChar('a'))] ) )
    );
    expand_text(&mut ValueList(values), scope);
    ValueList(values)
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
