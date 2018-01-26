use scope::*;
use value::*;
use expand::*;
use value::Value::*;
use std::rc::Rc;
use std::borrow::Cow;
use scope::EvalResult::*;
use regex::Regex;

fn get_args<'s>(mut args: Vec<Rope<'static>>) -> (Option<Value>,Option<Value>,Option<Value>,
                                              Option<Value>,Option<Value>,Option<Value>,
                                              Option<Value>) {
    let mut ait = args.drain(0..).map(|x| { x.coerce() });
    (
        ait.next(),
        ait.next(),
        ait.next(),
        ait.next(),
        ait.next(),
        ait.next(),
        ait.next(),
    )
}
// TODO: creating, removing, handling tagged values
// TODO: list stuff -- concatenate, get values/slices from, build closure
// TODO: allow defining 'constant' values
// TODO: manipulating scopes

fn head(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) { (Some(List(the_list)), None, ..) => {
        Done(the_list.into_iter().next().unwrap())
    } _ => {panic!()}}
}

fn match_regex(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) { (Some(Str(regex)), Some(Str(search_in)), None, ..) => {
        match Regex::new(regex.to_str()).unwrap().captures(search_in.to_str()) {
            None => { Done(Value::List(vec![])) },
            Some(cap) => {
                Done(Value::List(
                    cap.iter().map(|x| {
                        x.map_or_else(|| Value::Str(ArcSlice::empty()),
                            |capture| { Value::Str(search_in.index(capture.start()..capture.end()).make_static()) })
                    }).collect()
                ))
            }
        }
    } _ => {panic!()}}
}


fn list<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
   if args.len() != 1 { panic!() }
    Done(args.into_iter().next().unwrap().coerce_list())
}

fn assert<'s>(mut args: Vec<Rope<'static>>) -> EvalResult<'static> {
    let t1 = Some(args.remove(0).coerce());
    let t2 = Some(args.remove(0).coerce());
    let t3 = Some(args.remove(0).coerce());
    match (t1,t2,t3) {
        (Some(Str(mut message)), Some(val_a), Some(val_b)) => {
            // TODO fix threading issue...
            let mark = if val_a == val_b { "✓ " } else { "✗ " };
            if val_a != val_b {
                message = message + ArcSlice::from_string(format!(" - expected {:?}, found {:?}", val_a, val_b));
            }
            Done(Value::Str(
                ArcSlice::from_string(mark.to_owned()) + message
            ))
        },
        _ => { panic!() }
    }
}



fn change_char<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (Some(Str(n)), Some(Str(replacement)), Some(Closure(ValueClosure(inner_scope, mut h))), None, ..) => {
            let needle = n.to_str().chars().next().unwrap();
            let mut rest = h.make_static();
            let prefix = rest.split_at(true, false, &mut |ch| {
                ch == needle
            });
            rest.split_char(); // take the matched character out
            Expand(inner_scope,
                 prefix.unwrap().concat(
                        Rope::from_slice(replacement)
                ).concat(rest) 
            )
        },
        _ => { panic!() }
    }
}

fn if_eq<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (Some(value_a), Some(value_b), Some(Closure(if_true)), Some(Closure(if_false)), None, ..) => {
            let mut todo = if value_a == value_b { if_true } else { if_false };
            Expand(todo.0, *(todo.1))
        },
        _ => {panic!()}
    }
}


// FIXME: not working yet??

fn if_eq_then<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> { 
    match get_args(args) {
        (Some(value_a), Some(value_b), Some(Closure(if_true)), Some(Closure(if_false)), Some(Closure(finally)), None,..) => {
            let mut todo = (if value_a == value_b { if_true } else { if_false }).force_clone().1;
            Expand( finally.0, todo.concat(*finally.1))
        },
        _ => {panic!()}
    }
}


fn end_paren<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (None, ..) => {
            Done(Value::Str(ArcSlice::from_string(")".to_owned())))
        },
        _ => panic!()
    }
}

fn literal<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (Some(Closure(ValueClosure(_, closure))), None, ..) => {
           Done (Value::Str( ArcSlice::from_string( closure.to_str().unwrap().into_string()  ))) 
        },
        _ => { panic!() }
    }
}

fn define<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (Some(Str(name_args)),
        Some(Closure(ValueClosure(scope, closure_data))),
       Some(Closure(ValueClosure(_, to_expand))), None, ..) => {

            // TODO: custom arguments, more tests
            let mut parts = vec![];
            let mut params = vec![];
            let na_str = name_args;
            for part in na_str.to_str().split(' ') {
                if part.starts_with(':') {
                    parts.push(Param);
                    params.push((&part[1..]).to_owned());
                } else {
                    parts.push(Ident(part.to_owned()));
                }
            }
            let mut new_scope = dup_scope(&scope);
            Scope::add_user(&mut new_scope, parts, params, *closure_data);
            Expand(Arc::new(new_scope), *to_expand)
        },
        (Some(Str(name_args)),
        Some(imm_value),
       Some(Closure(ValueClosure(scope, to_expand))), None, ..) => {

            // TODO: custom arguments, more tests
            let mut parts = vec![];
            let mut params = vec![];
            let na_str = name_args;
            for part in na_str.to_str().split(' ') {
                if part.starts_with(':') {
                    parts.push(Param);
                    params.push((&part[1..]).to_owned());
                } else {
                    parts.push(Ident(part.to_owned()));
                }
            }
            let mut new_scope = dup_scope(&scope);
            Scope::add_user(&mut new_scope, parts, params, Rope::from_value(imm_value));
            Expand(Arc::new(new_scope), *to_expand)
        },
        args => {
            panic!("Invalid state {:?}", args);
        }

    }
}

fn expand<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
        (Some(Closure(ValueClosure(scope, contents))), None, ..) => {
            Expand(scope, *contents)
        },
        _ => {panic!("ARG"); }
    }
}

fn rescope<'s>(args: Vec<Rope<'static>>) -> EvalResult<'static> {
    match get_args(args) {
    (Some(Closure(ValueClosure(inner_scope, _))),
    Some(Closure(ValueClosure(_, contents))),None,..) => {
         Done( Closure(ValueClosure(inner_scope.clone(), contents )) )
    },
    _ => {panic!() }
    }
}

//TODO handle EOF propelry
pub fn default_scope<'c>() -> Scope<'c> {
    let mut scope = Scope::new('#');
    scope.add_native(vec![ Ident("define".to_owned()), Param, Param, Param ], define);
    scope.add_native(vec![ Ident("change_char".to_owned()), Param, Param, Param ], change_char);
    scope.add_native(vec![ Ident("end_paren".to_owned()) ], end_paren );
    scope.add_native(vec![ Ident("literal".to_owned()), Param ], literal);
    scope.add_native(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ], if_eq);
    scope.add_native(vec![ Ident("if_eq_then".to_owned()), Param, Param, Param, Param, Param ], if_eq_then);
    scope.add_native(vec![ Ident("expand".to_owned()), Param ], expand);
    scope.add_native(vec![ Ident("rescope".to_owned()), Param, Param ], rescope); 
    scope.add_native(vec![ Ident("assert".to_owned()), Param, Param, Param ], assert); 
    scope.add_native(vec![ Ident("list".to_owned()), Param ], list); 
    scope.add_native(vec![ Ident("match_regex".to_owned()), Param, Param ], match_regex); 
    scope.add_native(vec![ Ident("head".to_owned()), Param ], head); 

    scope
}
