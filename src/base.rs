

use scope::*;
use value::*;
use expand::*;
use std::rc::Rc;
use std::borrow::Cow;
use std::collections::HashMap;

// TODO: allow changing "catcodes"
// TODO: better error handling
// TODO: misc builtins or library fns (e.g. like m4, and stuff for types)
// TODO: issue trying to change 'w' back to 'world'

fn change_char<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap()) {
        (&Str(ref n), &Str(ref replacement), &Closure(ValueClosure(ref inner_scope, ref h))) => {
            let needle = n.chars().next().unwrap();
            let mut haystack = h.make_static();
            let prefix = haystack.split_at(true, &mut |ch| {
                ch == needle
            }).unwrap();
            haystack.split_char(); // take the matched character out
            let new_closure = ValueClosure(inner_scope.clone(),
                Box::new( prefix.concat(
                    Rope::Leaf(Leaf::Chr(Cow::Borrowed(replacement)))
                ).concat(haystack.make_static()).make_static() )
            );
            Leaf::Own(Box::new(Value::Bubble(new_closure)))
        },
        _ => { panic!() }
    }
}

fn end_paren<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    Leaf::Own(Box::new(Value::Str(Cow::from(")".to_owned()))))
}
fn literal<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match args[0].as_val().unwrap() {
        &Closure(ValueClosure(_,ref closure)) => {
            Leaf::Own(Box::new(Value::Str(closure.to_str().unwrap())))
        },
        _ => { panic!() }
    }
}

fn define<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].as_val().unwrap(), args[1].as_val().unwrap(), args[2].as_val().unwrap()) {
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
                .add_user(parts, params, closure);
            // TODO avoid clone here
            new_expand(&new_scope.clone(), to_expand.dupe() ).make_static()
        },
        _ => {
            panic!("Invalid state")
        }

    }
}

fn expand<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match args[0].as_val().unwrap() {
        &Closure(ValueClosure(ref scope, ref contents)) => {
            new_expand(scope, contents.dupe() ).make_static()
        },
        _ => {panic!("ARG {:?}", args[0]); }
    }
}

fn rescope<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
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


    /*
    scope.add_native(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ],
        Command::IfEq
    );
    */
    scope.add_native(vec![ Ident("expand".to_owned()), Param ], expand);
    scope.add_native(vec![ Ident("rescope".to_owned()), Param, Param ], rescope); 
    scope
}
