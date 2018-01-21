

use scope::*;
use value::*;
use expand::*;
use std::rc::Rc;
use std::collections::HashMap;

fn define<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
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
            new_expand(&new_scope.clone(), to_expand.dupe() ).make_static()
        },
        _ => {
            panic!("Invalid state")
        }

    }
}

fn expand<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match args[0].to_val() {
        &Closure(ValueClosure(ref scope, ref contents)) => {
            new_expand(scope, contents.dupe() ).make_static()
        },
        _ => {panic!("ARG {:?}", args[0]); }
    }
}

fn rescope<'s>(scope: &Rc<Scope>, args: Vec<Leaf<'s>>) -> Leaf<'s> {
    match (args[0].to_val(), args[1].to_val()) {
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
    let mut scope = Scope {
        sigil: '#',
        commands: HashMap::new()
    };
    // idea: source maps?
    // add 3rd param (;-kind)
    scope.add_native(vec![ Ident("define".to_owned()), Param, Param, Param ],
        define);
    /*
    scope.add_native(vec![ Ident("if_eq".to_owned()), Param, Param, Param, Param ],
        Command::IfEq
    );
    */
    scope.add_native(vec![ Ident("expand".to_owned()), Param ], expand);
    scope.add_native(vec![ Ident("rescope".to_owned()), Param, Param ], rescope); 
    scope
}
