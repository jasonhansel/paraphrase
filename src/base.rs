use scope::*;
use value::*;
use expand::*;
use std::rc::Rc;
use std::borrow::Cow;


fn change_char<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap()) {
        (&Str(ref n), &Str(ref replacement), &Closure(ValueClosure(ref inner_scope, ref h))) => {
            let needle = n.chars().next().unwrap();
            let (mut rest, prefix) = h.make_static().split_at(true, &mut |ch| {
                ch == needle
            });
            rest.split_char(); // take the matched character out
            let new_closure = ValueClosure(inner_scope.clone(),
                Box::new( prefix.unwrap().concat(
                    Rope::Leaf(Leaf::Chr(Cow::Borrowed(replacement)))
                ).concat(rest).make_static() )
            );
            Leaf::Own(Box::new(Value::Bubble(new_closure)))
        },
        _ => { panic!() }
    }
}

fn if_eq<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap(), args[3].as_val().unwrap()) {
        (value_a, value_b, &Closure(ref if_true), &Closure(ref if_false)) => {
            let todo = if value_a == value_b { if_true } else { if_false };
            Leaf::Own(Box::new(Bubble(todo.force_clone())))
        },
        _ => {panic!()}
    }
}


// FIXME: not working yet??

fn if_eq_then<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap(), args[3].as_val().unwrap(), args[4].as_val().unwrap()) {
        (value_a, value_b, &Closure(ref if_true), &Closure(ref if_false), &Closure(ref finally) ) => {
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
    match (args[0].as_val().unwrap()) {
        &Closure(ref closure) => {
            Leaf::Own(Box::new(Bubble(closure.force_clone())))
        },
        _ => panic!()
    }
}

fn end_paren<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    Leaf::Own(Box::new(Value::Str(Cow::from(")".to_owned()))))
}

fn literal<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match args[0].as_val().unwrap() {
        &Closure(ValueClosure(_,ref closure)) => {
            Leaf::Own(Box::new(Value::Str(closure.to_str().unwrap())))
        },
        _ => { panic!() }
    }
}

// NOTE: can't define inside of parentheses (intentonally -- I think this is the only sensible
// opton if we want to allow parallelism -- can this be changed?)

fn define<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap()) {
        (&Str(ref name_args),
        &Closure(ValueClosure(ref scope, ref closure_data)),
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
            let mut new_scope = Rc::new(dup_scope(scope));
            Scope::add_user(&mut new_scope, parts, params, closure_data);
            // TODO avoid clone here
            new_expand(new_scope, to_expand.dupe() ).make_static()
        },
        _ => {
            panic!("Invalid state: {:?}", args)
        }

    }
}

fn expand<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match args[0].as_val().unwrap() {
        &Closure(ValueClosure(ref scope, ref contents)) => {
            new_expand(scope.clone(), contents.dupe() ).make_static()
        },
        _ => {panic!("ARG {:?}", args[0]); }
    }
}

fn rescope<'s>(args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap()) {
    (&Closure(ValueClosure(ref inner_scope, _)),
    &Closure(ValueClosure(_, ref contents))) => {
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
