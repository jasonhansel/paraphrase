use scope::*;
use value::*;
use futures::future::{ok,join_all,loop_fn,Loop};
use futures::prelude::*;
use futures_cpupool::*;
use futures::stream;
use rand;
use std::panic::UnwindSafe;


// TODO: clone() less

#[derive(Clone,Debug)]
enum ParseEntry {
    Text(u8, bool), // bool is true if in a call
    Command(Vec<CommandPart>)
}

pub trait TokenVisitor<'s, 't : 's> {
    fn start_command(&mut self);
    fn end_command(&mut self, Vec<CommandPart>, Arc<Scope<'static>>);
    fn start_paren(&mut self);
    fn end_paren(&mut self);
    fn raw_param(&mut self, Rope<'s>);
    fn semi_param(&mut self, Arc<Scope<'static>>, Rope<'s>, Vec<CommandPart>) -> Rope<'s> ;
    fn text(&mut self, Rope<'s>);
    fn done(&mut self);
}

#[derive(Clone,Debug)]
enum Instr {
    Push(Rope<'static>),
    Concat(u16),
    Call(u16, Vec<CommandPart>),
    Close(Rope<'static>),
    ClosePartial(Rope<'static>, UnfinishedParse),
    StartCmd
}

#[derive(Clone,Debug)]
pub struct UnfinishedParse {
    stack: Vec<ParseEntry>,
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr>
}

pub type Fut<T> = Box<Future<Item=T,Error=()> + Send>;

struct Expander<'s> {
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr>,
    joins: Vec<Fut<Rope<'s>>>,
    final_join: Option<Fut<EvalResult<'s>>>,

    pool: CpuPool,
    scope: Arc<Scope<'static>>
}


// TODO: is this really concurrent?
impl<'s> Expander<'s> {
    fn new(pool: CpuPool, scope:Arc<Scope<'static>>) -> Expander<'s> {
        Expander {
            parens: vec![0],
            calls: vec![],
            instr: vec![],

            pool: pool,
            scope: scope,
            joins: vec![],
            final_join: None
        }
    }
    fn get_call(&mut self) -> Vec<Instr> {
        let mut level = 1;
        let mut idx = self.instr.len() - 1;
        loop {
            if let Instr::StartCmd = self.instr[idx] {
                level -= 1;
                if level == 0 { break; }
            } else if let Instr::Call(_,_) = self.instr[idx] {
                level += 1;
            }
            idx -= 1;
        }
        self.instr.split_off(idx)
    }
    fn handle_call(&mut self, cmd: Vec<CommandPart>, instr: Vec<Instr>) -> Fut<EvalResult<'static>> {
        let pool = self.pool.clone();
        let pool3 = pool.clone();
        let scope = self.scope.clone();
        Box::new(stream::iter_ok(instr.into_iter()).fold((vec![], scope.clone()), move |(mut stack, scope): (Vec<Fut<Rope<'static>>>, Arc<Scope<'static>>), i| {
            match i {
            Instr::StartCmd => {
            }
            Instr::Push(r) => {
                stack.push(Box::new(ok(r)) as Fut<Rope<'static>>);
            },
            Instr::Concat(cnt) => {
                let idx = stack.len() - cnt as usize;
                let items = stack.split_off(idx);
                let to_push = 
                    join_all(items)
                    .map(|vec| {
                        let mut rope = Rope::new();
                        for r in vec { rope = rope.concat(r); }
                        rope
                    })
                ;
                stack.push(Box::new(to_push) as Fut<Rope<'static>>);
            },
            Instr::ClosePartial(r, unf) => {
                let mut new_scope = dup_scope(&scope);
                new_scope.part_done(unf);
                let v = Rope::from_value(  Value::Closure ( ValueClosure( Arc::new(new_scope), Box::new(r)  )) ) ;
                stack.push(Box::new(ok(v)));
            },
            Instr::Close(r) => {
                let stat = r;
                let v = Rope::from_value(  Value::Closure ( ValueClosure( scope.clone(), Box::new(stat)  )) ) ;
                stack.push(Box::new(ok(v)));
            }
            Instr::Call(cnt, inner_cmd) => {
                let idx = stack.len() - cnt as usize;
                let scope = scope.clone();
                let items = stack.split_off(idx);
                let ipool = pool3.clone();
                let to_push = join_all(items).and_then(move |args| {
                        println!("HERE {:?}", args);
                        match eval(scope, inner_cmd, args) {
                            // TODO decrease pointless recursion
                            EvalResult::Expand(s, r) => expand_with_pool(ipool, s, r) as Fut<Value<'static>>,
                            EvalResult::Done(v) => Box::new(ok(v)) as Fut<Value<'static>>
                        }
                    })
                    .map(|v| { Rope::from_value(v) });
                stack.push(Box::new(to_push) as Fut<Rope<'static>>);
            }
        } ok((stack as Vec<Fut<Rope<'static>>>, scope)) }).and_then(move |(vec, scope2)| {
            let r = Box::new( 
                join_all(vec).map(move |args| {
                    eval(scope2, cmd, args)
                }));
 /*               .and_then(move |args| { 
                    match eval(scope2, cmd, args) {
                        EvalResult::Expand(s, mut r) => expand_with_pool(pool2, s, r.make_static()),
                        EvalResult::Done(v) => Box::new(ok(v))
                    }
                })
                .map(Rope::from_value); */
            return r as Fut<EvalResult<'static>>
        }))
    }

    fn start_command(&mut self) {
        self.instr.push(Instr::StartCmd);
        self.calls.push(0);
    }
    fn end_command(&mut self, cmd: Vec<CommandPart>, scope: Arc<Scope<'static>>) {
        let call_len = self.calls.pop().unwrap();
        if self.calls.len() > 0 {
            self.instr.push(Instr::Call(call_len, cmd));
            if let Some(l) = self.parens.last_mut() { *l += 1; }
        } else {
            let spl = self.instr.split_off(0);
            let scope = self.scope.clone();
            let pool2 = self.pool.clone();
            let pool3 = self.pool.clone();
            let join = Box::new(self.handle_call(cmd,spl).and_then(move |ev| { 
                match ev {
                    EvalResult::Expand(s, r) => Box::new(expand_with_pool(pool3, s, r)) as Fut<_>,
                    EvalResult::Done(v) => Box::new(ok(v)) as Fut<_>
                }
            }).map(Rope::from_value));
            self.joins.push(join);
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
    fn raw_param(&mut self, mut rope: Rope<'s>) {
        *( self.calls.last_mut().unwrap() ) += 1;
        // TODO avoid clones here
        self.instr.push(Instr::Close(rope.make_static()));
    }
    fn semi_param(&mut self, stack: Vec<ParseEntry>, mut rope: Rope<'s>, cmd: Vec<CommandPart>) {
        let mut call = self.get_call();
        self.calls.pop();
        let unfinished = UnfinishedParse {
            stack: stack,
            calls: self.calls.clone(),
            parens: self.parens.clone(),
            instr: self.instr.split_off(0)
        };
        call.push(Instr::ClosePartial( rope.make_static(), unfinished));
        self.final_join = Some(Box::new((self.handle_call(cmd, call))));
    }
    fn text(&mut self, mut rope: Rope<'s>) {
        if self.calls.len() == 0 {
            self.joins.push(Box::new(ok(rope.make_static())));
        } else {
            if let Some(l) = self.parens.last_mut() { *l += 1; }
            self.instr.push(Instr::Push(rope.make_static()));
        }
    }
}





// TODO fix perf - rem compile optimized, stop storing characters separately
/// TODO note: can't parse closures in advance because of Rescope
// TODO: allow includes - will be tricky to avoid copying owned characters around
fn parse<'f, 'r, 's : 'r>(
    mut stack: Vec<ParseEntry>,
    scope: Arc<Scope<'static>>,
    mut rope: Rope<'s>,
    visitor: &mut Expander<'s>
){
    while let Some(current) = stack.pop() { match current {
        ParseEntry::Command(mut parts) => {
            // TODO: multi-part commands, variadic macros (may never impl - too error prone)
            // TODO: breaks intermacro text
            if scope.has_command(&parts) {
                visitor.end_command(parts.split_off(0), scope.clone());
                // continue to next work item
            }  else {
                rope.split_at(false, false, &mut |ch : char| {
                    if ch.is_whitespace() {
                        return false;
                    } else {
                        return true;
                    }
                });
                let chr = rope.split_char().unwrap();
                if chr == scope.sigil {
                    let s = rope.split_at(false, true, &mut |chr : char| {
                        if chr.is_alphabetic() || chr == '_' {
                            false
                        } else {
                            true
                        }
                    }).and_then(|x| { x.to_str() }).unwrap().into_string();
                    parts.push(Ident( s.clone() ));
                    visitor.start_command();
                    stack.push(ParseEntry::Command(parts));
               } else if chr == '(' {
                    visitor.start_paren();
                    stack.push(ParseEntry::Command(parts));
                    stack.push(ParseEntry::Text(0, true));
                } else if chr == ')' {

                    visitor.end_paren();
                    parts.push(Param);
                    stack.push(ParseEntry::Command(parts));
                } else if chr == ';' {
                    parts.push(Param);
                    // TODO get this working better
                    if !scope.has_command(&parts) {

                        panic!("Invalid semicolon");
                    }
                    let new_scope = scope.clone();
                    return visitor.semi_param(stack, rope, parts);
                } else if chr == '{' {
                    let mut raw_level = 1;
                    let param = rope.split_at(true, false, &mut |ch| { 
                        raw_level += match ch {
                            '{' => 1,
                            '}' => -1,
                            _ => 0
                        };
                        raw_level == 0
                    }).unwrap();
                    rope.split_char();
                    parts.push(Param);
                    visitor.raw_param(param);
                    stack.push(ParseEntry::Command(parts));
                } else {
                    panic!("Failed {:?} {:?} {:?}", rope, parts, chr);
                }
            }
        },
        ParseEntry::Text(mut paren_level, in_call) => {
            // TODO make more things (e.g. sigil character) customizable
            let prefix = rope.split_at(true, true, &mut |x| { 
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
}

// TODO: make sure user can define their own bubble-related fns.


// TODO: make sure user can define their own bubble-related fns.



pub fn expand_with_pool<'f>(
    pool: CpuPool,
    mut _scope: Arc<Scope<'static>>,
    mut _rope: Rope<'static>
) -> Fut<Value<'static>> {
    // TODO: why do we need a new CPUPOOL here:
    let id = rand::random::<u64>();
    let ipool = pool.clone();
    Box::new(
        loop_fn(( (vec![] as Vec<Fut<Rope<'static>>>), _scope, _rope), move |(mut joins, mut scope, mut rope)| {
            let UnfinishedParse {parens, calls, stack, instr} = Arc::make_mut(&mut scope).part_done.take().unwrap_or_else(|| {
                UnfinishedParse {
                    parens: vec![],
                    calls: vec![],
                    stack: vec![ParseEntry::Text(0, false)],
                    instr: vec![]
                }
            });
            let mut expander = Expander::new(ipool.clone(), scope.clone());
            expander.parens = parens; expander.calls = calls; expander.instr = instr;
            parse(stack, scope.clone(), rope, &mut expander);
            joins.extend( expander.joins.into_iter().map(|j| {
                Box::new(ipool.spawn(j)) as Fut<_>
            }) );
            if let Some(final_join) = expander.final_join {
                // TODO: allow this sort of thing in other cases? may be needed to prevent deadlock-y
                // situations. Do I need it everywhere?
                Box::new((final_join.map(|w| {
                    match w {
                        EvalResult::Expand(new_scope, new_rope) => Loop::Continue((joins, new_scope, new_rope)),
                        EvalResult::Done(val) => {
                            joins.push(Box::new(ok(Rope::from_value(val))));
                            Loop::Break(joins)
                        }
                    }
                })))  as Fut<Loop<_,_>>
            } else {
                Box::new(ok(Loop::Break(joins))) as Fut<Loop<_,_>>
            }
        })
        .and_then(|joins| { join_all(joins) })
        .map(move |joins : Vec<_>| {
            let mut vec : Vec<Rope<'static>> = vec![];
            for j in joins {
                vec.push(j); //wait().unwrap());
            }
            let mut res = Rope::new();
            for v in vec.into_iter() { res = res.concat(v); }
            let resc = res.coerce();
            resc
        })
    )
}

