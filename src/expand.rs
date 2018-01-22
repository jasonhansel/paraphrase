use scope::*;
use value::*;
use std::borrow::Cow;
use std::rc::Rc;

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
    fn semi_param(&mut self, &Rc<Scope>, Rope<'s>, Vec<CommandPart>) -> Rope<'s> ;
    fn text(&mut self, Rope<'s>);
    fn done(&mut self);
}

#[derive(Debug)]
enum Instr<'s> {
    Push(Rope<'s>),
    Concat(u16),
    Call(u16, Vec<CommandPart>),
    Close(Rope<'s>),
    StartCmd
}

struct Expander<'s> {
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr<'s>>
}

// are ropes stil necessary? basically just using them as linked lists now, I think

// TODO think thru bubbling behavior a bit more

fn do_expand<'s>(instr: Vec<Instr<'s>>, scope: &'s Rc<Scope>) -> Rope<'s> {
    let mut stack : Vec<Rope<'s>> = vec![];
    println!("EXPANDING {:?}", instr);
    for i in instr.into_iter() { match i {
        Instr::StartCmd => {}
        Instr::Push(r) => { stack.push(r); },
        Instr::Concat(cnt) => {
            let mut new_rope = Rope::new();
            let idx = stack.len() - cnt as usize;
            for item in stack.split_off(idx) {
                println!("CONCATTING {:?}", item);
                new_rope = new_rope.concat(item);
            }
            println!("POSTCONC {:?}", new_rope);
            stack.push(
                new_rope
            );
        },
        Instr::Close(r) => {
            println!("CLOSING {:?}", r);
            let stat = r.make_static();
            stack.push(
                Rope::Leaf( Leaf::Own(
                        Box::new(
                            Value::Closure ( ValueClosure( scope.clone(), Box::new(stat)  )) ))
                    )
            );
        }
        Instr::Call(cnt, cmd) => {
            println!("CALLING {:?} {:?}", stack.len(), cnt);
            let idx = stack.len() - cnt as usize;
            let args = stack.drain(idx..)
                .map(|x| { x.to_leaf(scope) })
                .collect::<Vec<_>>();
            println!("ARGDAT {:?} {:?}", cnt, cmd);
            // TODO: currently there are lots of nested eval() calls when working with closures --
            // e.g. with *macro definitions*
            let result = eval(scope, scope.clone(), cmd, args);
            println!("RES {:?}", result);
            stack.push( Rope::Leaf( result ) );
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
    fn do_expand(self, scope: &'s Rc<Scope>) -> Rope<'s> {
        do_expand(self.instr, scope)
    }
}

impl<'s,'t:'s> TokenVisitor<'s, 't> for Expander<'s> {
    fn start_command(&mut self, _: Cow<'s, str>) {
        println!("START CMD");
        self.instr.push(Instr::StartCmd);
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
        let num = self.parens.pop().unwrap();
        if num != 1 {
            self.instr.push(Instr::Concat(num));
        }
    }
    fn raw_param(&mut self, rope: Rope<'s>) {
        *( self.calls.last_mut().unwrap() ) += 1;
        self.instr.push(Instr::Close(rope));
    }
    fn semi_param(&mut self, scope: &Rc<Scope>, rope: Rope<'s>, parts: Vec<CommandPart>) -> Rope<'s> {
        let mut idx = self.instr.len() - 1;
        let mut level = 1;

        while idx >= 0  {
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


        if idx < 0 {
            panic!("Semi param outside of any command");
        } else if self.calls.len() > 0 {
            let file = self.instr.split_off(idx);
            // TODO: if there are no calls in progress, this should be the same
            // as the old raw_param behavior.
            let result = do_expand(file, scope).get_leaf();
            if let Some(bubble) = result.bubble(scope) {
                return bubble.make_static()
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
        println!("TXT {:?}", rope);
        self.instr.push(Instr::Push(rope));
    }
    fn done(&mut self) {
        self.instr.push(Instr::Concat(self.parens.pop().unwrap()));
        println!("COMPILED {:?}", self.instr);
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
            if scope.has_command(&parts) {
                println!("COMMAND DONE {:?}", parts);
                visitor.end_command(parts.split_off(0));
                // continue to next work item
            } else if parts.len() == 0 {
                println!("HERE");
                let (r, ident) = rope.split_at(false, &mut |chr : char| {
                    println!("CHECKING {:?}", chr);
                    if chr.is_alphabetic() || chr == '_' || chr == scope.sigil {
                        // dumb check for sigil /here
                        false
                    } else {
                        true
                    }
                });
                rope = r;

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
            println!("PREPARING {:?} {:?} {:?}", parts, rope, scope);
                let (r, _) = rope.split_at(false, &mut |ch : char| {
                    println!("SCANW {:?}", ch);
                    if ch.is_whitespace() {
                        return false;
                    } else {
                        return true;
                    }
                });
                rope = r;

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
                    // TODO get this working better
                    if !scope.has_command(&parts) {
                        panic!("Invalid semicolon");
                    }
                    rope = visitor.semi_param(&scope, rope, parts.split_off(0));
                } else if chr == '{' {
                    let mut raw_level = 1;
                    let (r, param) = rope.split_at(true, &mut |ch| { 
                        println!("RAW {:?} {:?}", ch, raw_level);
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
            let mut pos = 0;
            let (r, prefix) = rope.split_at(true, &mut |x| { 
                
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
            rope = r;
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
// TODO: make sure user can define their own bubble-related fns.
pub fn new_expand_nobubble<'f, 'r : 'f>(scope: &'f Rc<Scope>, tokens: Rope<'f>) -> Leaf<'f> {
    let mut expander = Expander::new();
    parse(scope.clone(), tokens, &mut expander);
    expander.do_expand(&scope).get_leaf()
}

// TODO: make sure user can define their own bubble-related fns.
pub fn new_expand<'f, 'r : 'f>(scope: &'f Rc<Scope>, tokens: Rope<'f>) -> Leaf<'f> {
    let mut expander = Expander::new();
    parse(scope.clone(), tokens, &mut expander);
    expander.do_expand(&scope).to_leaf(scope)
}
