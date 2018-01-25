use scope::*;
use value::*;
use futures::future::{ok,join_all,loop_fn,Loop};
use futures::prelude::*;
use futures_cpupool::*;
use futures::stream;
use rand;
use std::panic::UnwindSafe;
use std::thread;
use std::thread::JoinHandle;


// TODO: clone() less
// TODO: Revert to futures

#[derive(Clone,Debug)]
enum ParseEntry {
    Text(u8, bool), // bool is true if in a call
    Command(Vec<CommandPart>)
}

pub trait TokenVisitor<'s, 't : 's> {
    fn start_command(&mut self);
    fn end_command(&mut self, Vec<CommandPart>, Arc<Scope>);
    fn start_paren(&mut self);
    fn end_paren(&mut self);
    fn raw_param(&mut self, Rope);
    fn semi_param(&mut self, Arc<Scope>, Rope, Vec<CommandPart>) -> Rope ;
    fn text(&mut self, Rope);
    fn done(&mut self);
}

#[derive(Clone,Debug)]
enum Instr {
    Push(Rope),
    Concat(u16),
    Call(u16, Vec<CommandPart>),
    Close(Rope),
    ClosePartial(Rope, UnfinishedParse),
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

#[derive(Debug)]
enum Chunk<T> {
    Text(Rope),
    Join(JoinHandle<T>)
}

struct Expander {
    calls: Vec<u16>,
    parens: Vec<u16>,
    instr: Vec<Instr>,
    joins: Vec<Chunk<Rope>>,
    final_join: Option<JoinHandle<EvalResult>>,

    pool: CpuPool,
    scope: Arc<Scope>
}


// TODO: is this really concurrent?
impl Expander {
    fn new(pool: CpuPool, scope:Arc<Scope>) -> Expander {
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
    fn handle_call(&mut self, cmd: Vec<CommandPart>, instr: Vec<Instr>) -> JoinHandle<EvalResult> {
        let scope = self.scope.clone();
        let instr = instr.clone();
        thread::spawn(move ||{
            let mut stack : Vec<Chunk<Rope>> = vec![]; 
            for i in instr { match i {
                Instr::StartCmd => {
                }
                Instr::Push(r) => {
                    stack.push(
                        Chunk::Text(r)
                    );
                },
                Instr::Concat(cnt) => {
                    let idx = stack.len() - cnt as usize;
                    let scope = scope.clone();
                    let items = stack.split_off(idx);
                // TODO: fix deadlocks, add thread pool, then check concurrency
                    stack.push(Chunk::Join(thread::spawn(move || {
                        items
                            .into_iter()
                            .map(|x| {
                                match x {
                                    Chunk::Text(r) => r,
                                    Chunk::Join(j) => j.join().unwrap()
                                }
                            })
                            .fold(Rope::new(), |a,b| { a.concat(b) })
                    })));
                },
                Instr::ClosePartial(r, unf) => {
                    let mut new_scope = dup_scope(&scope);
                    new_scope.part_done(unf);
                    let v = Rope::from_value(  Value::Closure ( ValueClosure( Arc::new(new_scope), Box::new(r)  )) ) ;
                    stack.push(
                        Chunk::Text(v)
                    );
                },
                Instr::Close(r) => {
                    let stat = r;
                    let v = Rope::from_value(  Value::Closure ( ValueClosure( scope.clone(), Box::new(stat)  )) ) ;
                    stack.push(Chunk::Text(v));
                }
                Instr::Call(cnt, inner_cmd) => {
                    let idx = stack.len() - cnt as usize;
                    let scope = scope.clone();
                    let items = stack.split_off(idx);
                    stack.push(Chunk::Join(thread::spawn(|| { 
                       let coerced = items.into_iter().map(|x| {
                           match x {
                               Chunk::Text(r) => r,
                               Chunk::Join(j) => j.join().unwrap()
                           }
                       }).collect::<Vec<Rope>>();

                        match eval(scope, inner_cmd, coerced) {
                            // TODO decrease pointless recursion
                            EvalResult::Expand(s, r) => Rope::from_value(expand_with_pool(CpuPool::new_num_cpus(), s, r).join().unwrap()),
                            EvalResult::Done(v) => Rope::from_value(v)
                        }
                    })));
                }
            } }
            eval(scope, cmd, stack.into_iter()
                .map(|x| {
                    match x {
                        Chunk::Text(r) => r,
                        Chunk::Join(j) => j.join().unwrap()
                    }
                })
                .collect()
            )
        })
    }

    fn start_command(&mut self) {
        self.instr.push(Instr::StartCmd);
        self.calls.push(0);
    }
    fn end_command(&mut self, cmd: Vec<CommandPart>) {
        let call_len = self.calls.pop().unwrap();
        if self.calls.len() > 0 {
            self.instr.push(Instr::Call(call_len, cmd));
            if let Some(l) = self.parens.last_mut() { *l += 1; }
        } else {
            let spl = self.instr.split_off(0);
            let scope = self.scope.clone();
            let pool2 = self.pool.clone();
            let pool3 = self.pool.clone();
            let join = self.handle_call(cmd,spl);
            self.joins.push(Chunk::Join(thread::spawn(||{  
                match join.join().unwrap() {
                    EvalResult::Expand(s, r) => {
                        Rope::from_value(expand_with_pool(CpuPool::new_num_cpus(), s, r).join().unwrap())
                    },
                    EvalResult::Done(v) => {
                        Rope::from_value(v)
                    }
                }
            })));
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
    fn raw_param(&mut self, mut rope: Rope) {
        *( self.calls.last_mut().unwrap() ) += 1;
        // TODO avoid clones here
        self.instr.push(Instr::Close(rope.make_static()));
    }
    fn semi_param(&mut self, stack: Vec<ParseEntry>, mut rope: Rope, cmd: Vec<CommandPart>) {
        let mut call = self.get_call();
        self.calls.pop();
        let unfinished = UnfinishedParse {
            stack: stack,
            calls: self.calls.clone(),
            parens: self.parens.clone(),
            instr: self.instr.split_off(0)
        };
        call.push(Instr::ClosePartial( rope.make_static(), unfinished));
        self.final_join = Some(self.handle_call(cmd, call));
    }
    fn text(&mut self, mut rope: Rope) {
        if self.calls.len() == 0 {
            self.joins.push(Chunk::Text(rope.make_static()));
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
    scope: Arc<Scope>,
    mut rope: Rope,
    visitor: &mut Expander
){
    while let Some(current) = stack.pop() { match current {
        ParseEntry::Command(mut parts) => {
            // TODO: multi-part commands, variadic macros (may never impl - too error prone)
            // TODO: breaks intermacro text
            if scope.has_command(&parts) {
                visitor.end_command(parts.split_off(0));
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
    mut _scope: Arc<Scope>,
    mut _rope: Rope
) -> JoinHandle<Value> {
    // TODO: why do we need a new CPUPOOL here:
    let ipool = pool.clone();
    let rope = _rope.clone();
    thread::spawn(move || {
        let mut scope = _scope.clone();
        let mut rope = rope;
        let mut joins : Vec<Chunk<Rope>> = vec![];
        let mut last : Option<Value> = None;
        loop {
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

            // TODO avoid all these joins
            joins.extend(expander.joins);

            if let Some(final_join) = expander.final_join {
                match final_join.join().unwrap() {
                    EvalResult::Expand(new_scope, new_rope) => {
                        scope = new_scope;
                        rope = new_rope;
                    }
                    EvalResult::Done(val) => {
                        last = Some( val );
                        break;
                    }
                }
            } else {
                break;
            }
        }
        let mut re = joins.into_iter().fold(Rope::new(), |a, mut to_join| {
            a.concat(match to_join {
                Chunk::Text(r) => {
                    r
                }
                Chunk::Join(j) => {
                    j.join().unwrap()
                }
            })
        });
        if let Some(f) = last {
            re = re.concat(Rope::from_value(f));
        }
        re.coerce()
    })
}

