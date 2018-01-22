use scope::*;
use value::*;
use expand::*;
use std::rc::Rc;
use std::borrow::Cow;

fn get_args<'s>(args: Vec<Leaf<'s>>) -> (Option<Value>,Option<Value>,Option<Value>,Option<Value>,Option<Value>,Option<Value>,Option<Value>) {
    let mut it = args.into_iter().fuse().map(|x| { x.as_val() });
    (
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
        it.next().and_then(|x|{x}),
    )
}

fn change_char<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(Str(n)), Some(Str(replacement)), Some(Closure(ValueClosure(inner_scope, h))), None, ..) => {
            let needle = n.chars().next().unwrap();
            let (mut rest, prefix) = h.make_static().split_at(true, &mut |ch| {
                ch == needle
            });
            rest.split_char(); // take the matched character out
            let new_closure = ValueClosure(inner_scope.clone(),
                Box::new( prefix.unwrap().concat(
                    Rope::Leaf(Leaf::Chr(Cow::Owned(replacement.into_owned())))
                ).concat(rest).make_static() )
            );
            Leaf::Own(Box::new(Value::Bubble(new_closure)))
        },
        _ => { panic!() }
    }
}

fn if_eq<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(value_a), Some(value_b), Some(Closure(if_true)), Some(Closure(if_false)), None, ..) => {
            let todo = if value_a == value_b { if_true } else { if_false };
            Leaf::Own(Box::new(Bubble(todo.force_clone()))) 
        },
        _ => {panic!()}
    }
}


// FIXME: not working yet??

fn if_eq_then<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> { 
    match get_args(args) {
        (Some(value_a), Some(value_b), Some(Closure(if_true)), Some(Closure(if_false)), Some(Closure(finally)), None,..) => {

            let todo = (if value_a == value_b { if_true } else { if_false }).force_clone().1;

            let rv = Leaf::Own(Box::new(Bubble(
                ValueClosure(finally.0.clone(), Box::new(Rope::Node(todo, finally.force_clone().1)))
            )));

            rv
        },
        _ => {panic!()}
    }
}

fn bubble<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(Closure(closure)), None, ..) => {
            Leaf::Own(Box::new(Bubble(closure.force_clone())))
        },
        _ => panic!()
    }
}

fn end_paren<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    Leaf::Own(Box::new(Value::Str(Cow::from(")".to_owned()))))
}

fn literal<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(Closure(ValueClosure(_, closure))), None, ..) => {
            Leaf::Own(Box::new(Value::Str( Cow::Owned( closure.to_str().unwrap().into_owned()  ))) )
        },
        _ => { panic!() }
    }
}

// NOTE: can't define inside of parentheses (intentonally -- I think this is the only sensible
// opton if we want to allow parallelism -- can this be changed?)

fn define<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(Str(name_args)),
        Some(Closure(ValueClosure(scope, closure_data))),
       Some(Closure(ValueClosure(_, to_expand))), None, ..) => {

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
            let mut new_scope = Rc::new(dup_scope(&scope));
            Scope::add_user(&mut new_scope, parts, params, &*closure_data);
            // TODO avoid clone here
            new_expand(new_scope, to_expand.make_static())
        },
        _ => {
            panic!("Invalid state");
        }

    }
}

fn expand<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
        (Some(Closure(ValueClosure(scope, contents))), None, ..) => {
            new_expand(scope.clone(), contents.make_static() ).make_static()
        },
        _ => {panic!("ARG"); }
    }
}

fn rescope<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match get_args(args) {
    (Some(Closure(ValueClosure(inner_scope, _))),
    Some(Closure(ValueClosure(_, contents))),None,..) => {
        Leaf::Own(Box::from(
             Closure(ValueClosure(inner_scope.clone(), Box::new(contents.make_static() )))
        ))
    },
    _ => {panic!() }
    }
}

//TODO handle EOF propelry
pub fn default_scope() -> Scope {
    let mut scope = Scope::new('#');
    // idea: source maps?
    // add 3rd param (;-kind)
    scope.add_native(vec![ Ident("define".to_owned()), Param, Param, Param ],
        define);

    // the below will test 
    scope.add_native(vec![ Ident("change_char".to_owned()), Param, Param, Param ],
        change_char
    );
    scope.add_native(vec![ Ident("end_paren".to_owned()) ],
        end_paren
    );
    // 
    scope.add_native(vec![ Ident("literal".to_owned()), Param ],
        literal
    );
    scope.add_native(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ], if_eq);
    scope.add_native(vec![ Ident("if_eq_then".to_owned()), Param, Param, Param, Param, Param ], if_eq_then);
    scope.add_native(vec![ Ident("bubble".to_owned()), Param ], bubble);


    /*
    scope.add_native(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ],
        Command::IfEq
    );
    */
    scope.add_native(vec![ Ident("expand".to_owned()), Param ], expand);
    scope.add_native(vec![ Ident("rescope".to_owned()), Param, Param ], rescope); 
    scope
}
