use scope::*;
use value::*;
use std::borrow::Cow;
use std::rc::Rc;
use std::thread::{spawn,JoinHandle};
use std::sync::atomic::{AtomicUsize,Ordering};

#[derive(Debug)]
enum ParseEntry {
    Text(u8, bool), // bool is true if in a call
    Command(Vec<CommandPart>)
}

pub trait TokenVisitor<'s, 't : 's> {
    fn start_command(&mut self, Cow<'s, str>);
    fn end_command(&mut self, Vec<CommandPart>, Arc<Scope<'static>>);
    fn start_paren(&mut self);
    fn end_paren(&mut self);
    fn raw_param(&mut self, Rope<'s>);
    fn semi_param(&mut self, Arc<Scope<'static>>, Rope<'s>, Vec<CommandPart>) -> Rope<'s> ;
    fn text(&mut self, Rope<'s>);
    fn done(&mut self);
}

#[derive(Debug)]
enum Instr<'s> {
    Push(Rope<'s>),
    Concat(u16),
    Call(u16, Vec<CommandPart>),
    Close(Rope<'s>),
    Join(JoinHandle<Rope<'static>>),
    StartCmd
}
use self::Instr::*;

struct Expander<'s> {
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr<'s>>
}

// are ropes stil necessary? basically just using them as linked lists now, I think

// TODO think thru bubbling behavior a bit more

fn do_expand<'s>(instr: Vec<Instr<'s>>, scope: Arc<Scope<'static>>) -> Rope<'s> {
    let mut stack : Vec<Rope<'s>> = vec![];
    for i in instr.into_iter() { match i {
        Instr::StartCmd => {}
        Instr::Push(r) => { stack.push(r); },
        Instr::Join(j) => {
                        ACTIVE.fetch_sub(1,Ordering::SeqCst);
            println!("ACTIVE {:?}", ACTIVE);
            

            stack.push(j.join().unwrap());
                },
        Instr::Concat(cnt) => {
            let mut new_rope = Rope::new();
            let idx = stack.len() - cnt as usize;
            for item in stack.split_off(idx) {
                new_rope = new_rope.concat(item);
            }
            stack.push(
                new_rope
            );
        },
        Instr::Close(r) => {
            let stat = r;
            let v = 
            Rope::from_value(  Value::Closure ( ValueClosure( scope.clone(), Box::new(stat)  )) ) ;
            stack.push(
                v
            );
        }
        Instr::Call(cnt, cmd) => {
            let idx = stack.len() - cnt as usize;
            let args = stack.drain(idx..)
                .map(|x| { x.coerce_bubble(scope.clone()) })
                .collect::<Vec<_>>();
            // TODO: currently there are lots of nested eval() calls when working with closures --
            // e.g. with *macro definitions*
            let result = eval(scope.clone(), cmd, args);
            stack.push( Rope::from_value(result));
        }

    } }
    if stack.len() != 1 {
        panic!("Wrong stack size!");
    }
    stack.remove(0)
}

impl<'s> Expander<'s> {
    fn new() -> Expander<'s> {
        Expander {
            parens: vec![0],
            calls: vec![],
            instr: vec![]
        }
    }
    fn do_expand(self, scope: Arc<Scope<'static>>) -> Rope<'s> {
        do_expand(self.instr, scope)
    }
}

impl<'s,'t:'s> TokenVisitor<'s, 't> for Expander<'s> {
    fn start_command(&mut self, _: Cow<'s, str>) {
        self.instr.push(Instr::StartCmd);
        self.calls.push(0);
    }
    fn end_command(&mut self, cmd: Vec<CommandPart>, scope: Arc<Scope<'static>>) {
        if let Some(l) = self.parens.last_mut() { *l += 1; }
        self.instr.push(Instr::Call(self.calls.pop().unwrap(), cmd));

        if self.calls.len() == 0 {
            let mut idx = self.instr.len() - 2;
            let mut level = 1;

            loop {
                if let Instr::StartCmd = self.instr[idx] {
                    level -= 1;
                    if level == 0 { break; }
                } else if let Instr::Call(_,_) = self.instr[idx] {
                    level += 1;
                }
                idx -= 1;
            }
            let to_run = self.instr.split_off(idx);
            // TODO let ropes use Arc internally
            let mut static_run : Vec<Instr<'static>> = vec![];
            for val in to_run.into_iter() {
                static_run.push(match val {
                    Push(r) =>  { Push(r.make_static()) }
                    Concat(u) => { Concat(u) }
                    Call(u, cp) => { Call(u, cp) }
                    Close(r) => { Close(r.make_static()) }
                    Join(j) => { panic!("Invalid join") }
                    StartCmd => { StartCmd }
                });
            }
            ACTIVE.fetch_add(1,Ordering::SeqCst);
            println!("ACTIVE {:?}", ACTIVE);
            let join = spawn(move || {
                do_expand(static_run, scope)
            });
            self.instr.push(Instr::Join(join));

        }

    }
    fn start_paren(&mut self) {
        self.parens.push(0);
    }
    fn end_paren(&mut self) {
        *( self.calls.last_mut().unwrap() ) += 1;
        let num = self.parens.pop().unwrap();
        if num != 1 {
            self.instr.push(Instr::Concat(num));
        }
    }
    fn raw_param(&mut self, rope: Rope<'s>) {
        *( self.calls.last_mut().unwrap() ) += 1;
        self.instr.push(Instr::Close(rope));
    }
    fn semi_param(&mut self, scope: Arc<Scope<'static>>, rope: Rope<'s>, parts: Vec<CommandPart>) -> Rope<'s> {
        let mut idx = self.instr.len() - 1;
        let mut level = 1;

        loop {
            if let Instr::StartCmd = self.instr[idx] {
                level -= 1;
                if level == 0 { break; }
            } else if let Instr::Call(_,_) = self.instr[idx] {
                level += 1;
            }
            idx -= 1;
        }
        // TODO: excessive recursion here

        self.instr.push(Instr::Close(rope));
        self.instr.push(Instr::Call(self.calls.pop().unwrap() + 1, parts));


        if self.calls.len() > 0 {
            let file = self.instr.split_off(idx);
            // TODO: if there are no calls in progress, this should be the same
            // as the old raw_param behavior.
            let result = do_expand(file, scope.clone()).coerce();
            if let Some(bubble) = result.bubble_move(scope) {
                return bubble
            } else {
                panic!("Hit an in-call semiparameter, but wasn't a bubble");
            }
        } else {
            if let Some(l) = self.parens.last_mut() { *l += 1; }
           // we can just finish the call
            return Rope::new()
        }
    }
    fn text(&mut self, rope: Rope<'s>) {
        if let Some(l) = self.parens.last_mut() { *l += 1; }
        self.instr.push(Instr::Push(rope));
    }
    fn done(&mut self) {
        self.instr.push(Instr::Concat(self.parens.pop().unwrap()));
        if self.calls.len() > 0 || self.parens.len() > 0 {
            panic!("Unbalanced {:?} {:?}", self.calls, self.parens);
        }
    }
}


static ACTIVE : AtomicUsize = AtomicUsize::new(0);



// TODO fix perf - rem compile optimized, stop storing characters separately
/// TODO note: can't parse closures in advance because of Rescope
// TODO: allow includes - will be tricky to avoid copying owned characters around
fn parse<'f, 'r, 's : 'r>(
    scope: Arc<Scope<'static>>,
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
            println!("CHECKING {:?}", parts);
            if scope.has_command(&parts) {
                visitor.end_command(parts.split_off(0), scope.clone());
                // continue to next work item
            }  else {
                let (r, _) = rope.split_at(false, false, &mut |ch : char| {
                    if ch.is_whitespace() {
                        return false;
                    } else {
                        return true;
                    }
                });
                rope = r;
                let chr = rope.split_char().unwrap();
                if chr == scope.sigil {
                    let (r, ident) = rope.split_at(false, true, &mut |chr : char| {
                        if chr.is_alphabetic() || chr == '_' {
                            false
                        } else {
                            true
                        }
                    });
                    let id = ident.unwrap();
                    rope = r;
                    parts.push(Ident( id.to_str().unwrap().into_owned() ));
                    visitor.start_command(id.to_str().unwrap());
                    stack.push(ParseEntry::Command(parts));
               } else if chr == '(' {
                    visitor.start_paren();
                    stack.push(ParseEntry::Command(parts));
                    stack.push(ParseEntry::Text(0, true));
                } else if chr == ')' {

                    println!("HERE {:?} {:?} {:?}", parts, stack, rope);
                    visitor.end_paren();
                    parts.push(Param);
                    stack.push(ParseEntry::Command(parts));
                } else if chr == ';' {
                    parts.push(Param);
                    // TODO get this working better
                    if !scope.has_command(&parts) {

                        panic!("Invalid semicolon");
                    }
                    rope = visitor.semi_param(scope.clone(), rope, parts.split_off(0));
                } else if chr == '{' {
                    let mut raw_level = 1;
                    let (r, param) = rope.split_at(true, false, &mut |ch| { 
                        raw_level += match ch {
                            '{' => 1,
                            '}' => -1,
                            _ => 0
                        };
                        raw_level == 0
                    });
                    rope = r;
                    rope.split_char();
                    parts.push(Param);
                    visitor.raw_param(param.unwrap());
                    stack.push(ParseEntry::Command(parts));
                } else {
                    panic!("Failed {:?} {:?} {:?}", rope, parts, chr);
                }
            }
        },
        ParseEntry::Text(mut paren_level, in_call) => {
            // TODO make more things (e.g. sigil character) customizable
            let (r, prefix) = rope.split_at(true, true, &mut |x| { 
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
            rope = r;
            let p = prefix.unwrap();
            visitor.text(p);
        
            match rope.get_char() {
                Some(')') => {
                },
                Some(x) => {
                    if x != scope.sigil { panic!("Unexpected halt at: {:?}", x); }
                    stack.push(ParseEntry::Text(paren_level, in_call));
                    stack.push(ParseEntry::Command(vec![]));
                },
                None => {
                }
            }

        }
    } }
    visitor.done()
}

// TODO: make sure user can define their own bubble-related fns.


// TODO: make sure user can define their own bubble-related fns.
pub fn new_expand<'f>(scope: Arc<Scope<'static>>, tokens: Rope<'f>) -> Value<'f> {
    let mut expander = Expander::new();
    parse(scope.clone(), tokens, &mut expander);
    expander.do_expand(scope.clone()).coerce_bubble(scope.clone())
}
